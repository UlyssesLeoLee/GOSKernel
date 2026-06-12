#!/usr/bin/env python3
"""
GOS Graphics Bridge — tools/gfx-bridge.py
=========================================
Host-side visual watcher for the GOS kernel's graph-native render bridge.

This mirrors `tools/chat-bridge.py`: it connects to a dedicated QEMU serial
port (exposed as a TCP server by QEMU) and consumes a stream of newline-framed
*display-list* commands emitted by the in-kernel `k-vk-host` node. Each command
describes the graph to draw — nodes are render units, edges are conduits — and
the host renders them into a window via tkinter (Python stdlib only).

This is the V0 "host-bridged surface" spike for the visual track (Phase I,
Vulkan Gen-1). The wire protocol below is the architectural seam: V0/V1 render
it with this simple tkinter rasteriser; a later Vulkan host backend can consume
the exact same display-list with no kernel-side change.

Wire protocol (@gos.vk display list, COM-over-TCP, newline-terminated ASCII)
---------------------------------------------------------------------------
  Kernel -> Host   VKHEL:<kernelid>                         handshake request
  Host   -> Kernel VKOKY:<ver>                              handshake ack
  Kernel -> Host   VKDIM:<w>x<h>                            logical surface size
  Kernel -> Host   VKCLR:<rrggbb>                           clear surface to color
  Kernel -> Host   VKNOD:<id>:<x>:<y>:<w>:<h>:<rrggbb>:<label>   node (render unit)
  Kernel -> Host   VKEDG:<from_id>:<to_id>:<rrggbb>         edge (conduit)
  Kernel -> Host   VKPIX:<x>:<y>:<rrggbb>                   single pixel
  Kernel -> Host   VKEND:                                   present (swap buffers)

Colors are 6 hex digits (rrggbb). The display list is immediate-mode: every
frame restates the full scene, terminated by VKEND. A VKCLR resets the frame.

Usage
-----
  python tools/gfx-bridge.py                 connect to QEMU, render live
  python tools/gfx-bridge.py --port 14445    use a specific COM-over-TCP port
  python tools/gfx-bridge.py --selftest      draw a sample graph (no QEMU)
  python tools/gfx-bridge.py --replay FILE    render a captured frame dump
  python tools/gfx-bridge.py --check          headless parse self-test (CI)
  python tools/gfx-bridge.py --dump-ppm out.ppm                  sample frame -> host PPM
  python tools/gfx-bridge.py --dump-ppm out.ppm --replay FILE    captured frame -> host PPM

`--check` needs neither QEMU nor a display: it runs the built-in sample
display-list through a headless renderer and asserts the resulting op counts,
exiting non-zero on mismatch. This is the verifiable goal for the protocol.

`--dump-ppm FILE` rasterises the display list (sample, or `--replay`'s
captured frames) into FILE as a binary PPM (P6) — zero dependencies,
byte-diffable. This is the host-side "golden frame" capture for V2.5's gfx
golden-frame/gfx-fuzz harness: the screenshot lands on the *host* filesystem
via this COM3 bridge, independent of the kernel's FAT32/F.5 write path (see
doc/ADR-009-f5-screenshot-scope.md).
"""

import argparse
import os
import socket
import sys
import tempfile
import threading
import queue

VERSION = "gfx-bridge-v0"
DEFAULT_PORT = 14445  # dedicated visual COM-over-TCP port (chat uses 14444)

# ── ANSI colours for host-side logging ───────────────────────────────────────
RESET, CYAN, YELLOW, GREEN, RED, GREY = (
    "\033[0m", "\033[36m", "\033[33m", "\033[32m", "\033[31m", "\033[90m",
)


def log(colour: str, tag: str, msg: str) -> None:
    print(f"{colour}[{tag}]{RESET} {msg}", flush=True)


# ── Display-list parsing ──────────────────────────────────────────────────────
# A frame parses to a tuple: (op, *args). Unknown/garbage frames -> None.

def _hexcolor(s: str) -> str:
    """Normalise a 6-hex-digit color to a tkinter '#rrggbb' string."""
    s = s.strip().lstrip("#")
    if len(s) != 6 or any(c not in "0123456789abcdefABCDEF" for c in s):
        return "#000000"
    return "#" + s.lower()


def parse_frame(line: str):
    """Parse one wire frame into an op tuple, or None if not a @gos.vk frame."""
    line = line.rstrip("\r\n")
    if not line:
        return None
    try:
        if line.startswith("VKHEL:"):
            return ("hel", line[6:])
        if line.startswith("VKOKY:"):
            return ("oky", line[6:])
        if line.startswith("VKDIM:"):
            w, h = line[6:].lower().split("x", 1)
            return ("dim", int(w), int(h))
        if line.startswith("VKCLR:"):
            return ("clr", _hexcolor(line[6:]))
        if line.startswith("VKNOD:"):
            # id:x:y:w:h:rgb:label  (label is the remainder; may contain colons)
            parts = line[6:].split(":", 6)
            if len(parts) < 7:
                return None
            nid, x, y, w, h, rgb, label = parts
            return ("nod", nid, int(x), int(y), int(w), int(h),
                    _hexcolor(rgb), label)
        if line.startswith("VKEDG:"):
            a, b, rgb = line[6:].split(":", 2)
            return ("edg", a, b, _hexcolor(rgb))
        if line.startswith("VKPIX:"):
            x, y, rgb = line[6:].split(":", 2)
            return ("pix", int(x), int(y), _hexcolor(rgb))
        if line.startswith("VKEND:"):
            return ("end",)
    except (ValueError, IndexError):
        return None
    return None


# ── Renderers ───────────────────────────────────────────────────────────────
# Renderers consume parsed op tuples. The headless one records counts for the
# --check self-test; the tkinter one actually draws.

class HeadlessRenderer:
    """Records op counts without a display. Used by --check."""

    def __init__(self):
        self.counts = {"dim": 0, "clr": 0, "nod": 0, "edg": 0, "pix": 0, "end": 0}
        self.nodes = {}        # id -> (cx, cy) for edge endpoint validation
        self.dangling_edges = 0
        self.dim = (0, 0)

    def apply(self, op):
        kind = op[0]
        if kind == "dim":
            self.counts["dim"] += 1
            self.dim = (op[1], op[2])
        elif kind == "clr":
            self.counts["clr"] += 1
            self.nodes.clear()
        elif kind == "nod":
            self.counts["nod"] += 1
            _, nid, x, y, w, h, _rgb, _label = op
            self.nodes[nid] = (x + w // 2, y + h // 2)
        elif kind == "edg":
            self.counts["edg"] += 1
            _, a, b, _rgb = op
            if a not in self.nodes or b not in self.nodes:
                self.dangling_edges += 1
        elif kind == "pix":
            self.counts["pix"] += 1
        elif kind == "end":
            self.counts["end"] += 1


class TkRenderer:
    """Draws the display list into a tkinter Canvas."""

    def __init__(self, width=800, height=600):
        import tkinter as tk
        self._tk = tk
        self.root = tk.Tk()
        self.root.title("GOS · graph surface (gfx-bridge v0)")
        self.width, self.height = width, height
        self.canvas = tk.Canvas(self.root, width=width, height=height,
                                bg="#000000", highlightthickness=0)
        self.canvas.pack(fill="both", expand=True)
        self.nodes = {}        # id -> (cx, cy)
        self._pending_clear = "#000000"

    def apply(self, op):
        kind = op[0]
        c = self.canvas
        if kind == "dim":
            self.width, self.height = op[1], op[2]
            c.config(width=self.width, height=self.height)
        elif kind == "clr":
            c.delete("all")
            c.config(bg=op[1])
            self.nodes.clear()
        elif kind == "nod":
            _, nid, x, y, w, h, rgb, label = op
            c.create_rectangle(x, y, x + w, y + h, fill=rgb, outline="#e8e8f0",
                               width=2)
            if label:
                c.create_text(x + w // 2, y + h // 2, text=label,
                              fill="#f4f4ff", font=("Consolas", 10, "bold"))
            self.nodes[nid] = (x + w // 2, y + h // 2)
        elif kind == "edg":
            _, a, b, rgb = op
            if a in self.nodes and b in self.nodes:
                (x1, y1), (x2, y2) = self.nodes[a], self.nodes[b]
                c.create_line(x1, y1, x2, y2, fill=rgb, width=2)
        elif kind == "pix":
            _, x, y, rgb = op
            c.create_rectangle(x, y, x + 1, y + 1, fill=rgb, outline=rgb)
        elif kind == "end":
            self.root.update_idletasks()
            self.root.update()


def _rgb_bytes(hexcolor: str):
    """'#rrggbb' -> (r, g, b) ints. Malformed input -> black."""
    s = hexcolor.lstrip("#")
    if len(s) != 6:
        return (0, 0, 0)
    try:
        return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16))
    except ValueError:
        return (0, 0, 0)


class FrameBuffer:
    """Rasterises the display list into an RGB byte buffer — the host-side
    pixel sink for `--dump-ppm` golden-frame capture. No tkinter, no display;
    same drawing primitives as TkRenderer (filled rects, lines, pixels)."""

    def __init__(self, width=800, height=600):
        self.width, self.height = width, height
        self.buf = bytearray(width * height * 3)
        self.nodes = {}        # id -> (cx, cy)

    def _resize(self, width, height):
        self.width, self.height = width, height
        self.buf = bytearray(width * height * 3)

    def _set(self, x, y, rgb):
        if 0 <= x < self.width and 0 <= y < self.height:
            i = (y * self.width + x) * 3
            self.buf[i:i + 3] = bytes(_rgb_bytes(rgb))

    def _fill_rect(self, x, y, w, h, rgb):
        rgb_bytes = bytes(_rgb_bytes(rgb))
        for yy in range(max(0, y), min(self.height, y + h)):
            row = (yy * self.width + max(0, x)) * 3
            for _ in range(max(0, x), min(self.width, x + w)):
                self.buf[row:row + 3] = rgb_bytes
                row += 3

    def _draw_line(self, x1, y1, x2, y2, rgb):
        # Bresenham
        dx, dy = abs(x2 - x1), -abs(y2 - y1)
        sx = 1 if x1 < x2 else -1
        sy = 1 if y1 < y2 else -1
        err = dx + dy
        x, y = x1, y1
        while True:
            self._set(x, y, rgb)
            if x == x2 and y == y2:
                break
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x += sx
            if e2 <= dx:
                err += dx
                y += sy

    def _clear(self, rgb):
        rgb_bytes = bytes(_rgb_bytes(rgb))
        self.buf[:] = rgb_bytes * (self.width * self.height)
        self.nodes.clear()

    def apply(self, op):
        kind = op[0]
        if kind == "dim":
            self._resize(op[1], op[2])
        elif kind == "clr":
            self._clear(op[1])
        elif kind == "nod":
            _, nid, x, y, w, h, rgb, _label = op
            self._fill_rect(x, y, w, h, rgb)
            self.nodes[nid] = (x + w // 2, y + h // 2)
        elif kind == "edg":
            _, a, b, rgb = op
            if a in self.nodes and b in self.nodes:
                (x1, y1), (x2, y2) = self.nodes[a], self.nodes[b]
                self._draw_line(x1, y1, x2, y2, rgb)
        elif kind == "pix":
            _, x, y, rgb = op
            self._set(x, y, rgb)
        # "end" (present) is a no-op for a single-frame capture.


def write_ppm(path: str, fb: "FrameBuffer") -> None:
    """Write `fb` as a binary PPM (P6) — zero dependencies, byte-diffable
    golden-frame format."""
    header = f"P6\n{fb.width} {fb.height}\n255\n".encode("ascii")
    with open(path, "wb") as f:
        f.write(header)
        f.write(bytes(fb.buf))


# ── Built-in sample display list (graph-native demo / self-test) ──────────────
# A tiny slice of the GOS graph: kernel core wired to a VGA render unit and a
# shell unit. Restated as one immediate-mode frame.
SAMPLE_SCRIPT = [
    "VKDIM:800x600",
    "VKCLR:101018",
    "VKNOD:1:320:60:160:64:2a6cff:gos-kernel",
    "VKNOD:2:120:300:140:56:23a06a:k-vga",
    "VKNOD:3:540:300:140:56:c2410c:k-shell",
    "VKEDG:1:2:5566aa",
    "VKEDG:1:3:5566aa",
    "VKEND:",
]


def run_check() -> int:
    """Headless parse + render self-test. Returns process exit code."""
    r = HeadlessRenderer()
    parsed = 0
    for line in SAMPLE_SCRIPT:
        op = parse_frame(line)
        if op is None:
            log(RED, "CHECK", f"failed to parse frame: {line!r}")
            return 1
        parsed += 1
        r.apply(op)

    expected = {"dim": 1, "clr": 1, "nod": 3, "edg": 2, "pix": 0, "end": 1}
    ok = True
    for k, v in expected.items():
        got = r.counts[k]
        mark = GREEN + "ok" + RESET if got == v else RED + "MISMATCH" + RESET
        if got != v:
            ok = False
        log(GREY, "CHECK", f"  {k}: expected {v}, got {got}  [{mark}]")

    if r.dangling_edges != 0:
        ok = False
        log(RED, "CHECK", f"  {r.dangling_edges} edge(s) reference unknown nodes")
    else:
        log(GREY, "CHECK", f"  edges resolve to known nodes  [{GREEN}ok{RESET}]")

    log(GREY, "CHECK", f"  surface dim parsed as {r.dim[0]}x{r.dim[1]}")

    # PPM dump round trip — the host-side golden-frame path (no kernel
    # FAT32/F.5 needed; see doc/ADR-009-f5-screenshot-scope.md).
    fb = FrameBuffer()
    for line in SAMPLE_SCRIPT:
        op = parse_frame(line)
        if op:
            fb.apply(op)
    fd, tmp_path = tempfile.mkstemp(suffix=".ppm")
    os.close(fd)
    try:
        write_ppm(tmp_path, fb)
        with open(tmp_path, "rb") as f:
            data = f.read()
        expected_header = f"P6\n{fb.width} {fb.height}\n255\n".encode("ascii")
        expected_len = len(expected_header) + fb.width * fb.height * 3
        ppm_ok = data.startswith(expected_header) and len(data) == expected_len
        mark = GREEN + "ok" + RESET if ppm_ok else RED + "MISMATCH" + RESET
        if not ppm_ok:
            ok = False
        log(GREY, "CHECK", f"  ppm dump: {len(data)} bytes (expected {expected_len})  [{mark}]")
    finally:
        os.unlink(tmp_path)

    if ok:
        log(GREEN, "CHECK", f"PASS - {parsed} frames parsed, all op counts match")
        return 0
    log(RED, "CHECK", "FAIL — display-list contract violated")
    return 1


def run_selftest() -> int:
    """Open a window and render the built-in sample graph (needs a display)."""
    try:
        r = TkRenderer()
    except Exception as exc:  # tkinter missing / no display
        log(RED, "SELFTEST", f"cannot open window: {exc}")
        return 1
    for line in SAMPLE_SCRIPT:
        op = parse_frame(line)
        if op:
            r.apply(op)
    log(GREEN, "SELFTEST", "sample graph drawn — close the window to exit")
    r.root.mainloop()
    return 0


def run_replay(path: str) -> int:
    """Render a captured frame dump from a file into a window."""
    try:
        r = TkRenderer()
    except Exception as exc:
        log(RED, "REPLAY", f"cannot open window: {exc}")
        return 1
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            op = parse_frame(line)
            if op:
                r.apply(op)
    log(GREEN, "REPLAY", f"replayed {path} — close the window to exit")
    r.root.mainloop()
    return 0


def run_dump_ppm(path: str, replay_path) -> int:
    """Rasterise the sample (or --replay'd) display list to `path` as a PPM.
    Host-side golden-frame capture — no display, no kernel FAT32/F.5."""
    fb = FrameBuffer()
    if replay_path:
        with open(replay_path, "r", encoding="utf-8", errors="replace") as fh:
            lines = fh.readlines()
    else:
        lines = SAMPLE_SCRIPT
    parsed = 0
    for line in lines:
        op = parse_frame(line)
        if op:
            fb.apply(op)
            parsed += 1
    write_ppm(path, fb)
    log(GREEN, "DUMP", f"wrote {fb.width}x{fb.height} PPM ({parsed} ops) to {path}")
    return 0


# ── Live bridge (socket reader thread + tkinter render loop) ──────────────────

def run_live(port: int) -> int:
    try:
        r = TkRenderer()
    except Exception as exc:
        log(RED, "BRIDGE", f"cannot open window: {exc}")
        return 1

    frames: "queue.Queue[str]" = queue.Queue()
    stop = threading.Event()

    def reader():
        """Connect to QEMU COM-over-TCP, push raw frames onto the queue."""
        while not stop.is_set():
            try:
                log(CYAN, "BRIDGE", f"connecting to 127.0.0.1:{port} ...")
                with socket.create_connection(("127.0.0.1", port)) as sock:
                    fh = sock.makefile("rwb", buffering=0)
                    log(GREEN, "BRIDGE", "connected; waiting for VKHEL handshake")
                    for raw in fh:
                        if stop.is_set():
                            break
                        line = raw.decode("utf-8", errors="replace")
                        if line.startswith("VKHEL:"):
                            fh.write(f"VKOKY:{VERSION}\n".encode())
                            fh.flush()
                            log(GREEN, "HELO", f"kernel: {line[6:].strip()!r}")
                        else:
                            frames.put(line)
            except ConnectionRefusedError:
                log(RED, "BRIDGE",
                    f"refused — is QEMU up with -serial tcp:127.0.0.1:{port},server,nowait?")
                stop.set()
                r.root.after(0, r.root.destroy)
                return
            except OSError:
                log(YELLOW, "BRIDGE", "disconnected; waiting for next boot ...")

    def pump():
        """tkinter-thread: drain frames and apply them to the canvas."""
        try:
            while True:
                line = frames.get_nowait()
                op = parse_frame(line)
                if op and op[0] not in ("hel", "oky"):
                    r.apply(op)
        except queue.Empty:
            pass
        if not stop.is_set():
            r.root.after(15, pump)

    t = threading.Thread(target=reader, daemon=True)
    t.start()
    r.root.after(15, pump)
    try:
        r.root.mainloop()
    finally:
        stop.set()
    return 0


def main() -> int:
    p = argparse.ArgumentParser(description="GOS Graphics Bridge (gfx-bridge v0)")
    p.add_argument("--port", type=int, default=DEFAULT_PORT,
                   help=f"QEMU COM-over-TCP port (default {DEFAULT_PORT})")
    p.add_argument("--selftest", action="store_true",
                   help="draw the built-in sample graph in a window (no QEMU)")
    p.add_argument("--replay", metavar="FILE",
                   help="render a captured frame dump from FILE")
    p.add_argument("--check", action="store_true",
                   help="headless parse self-test; exit non-zero on contract failure")
    p.add_argument("--dump-ppm", metavar="FILE",
                   help="rasterise the sample (or --replay'd) display list to FILE as a "
                        "PPM (host-side golden-frame capture; no kernel FAT32 needed)")
    args = p.parse_args()

    if args.check:
        return run_check()
    if args.dump_ppm:
        return run_dump_ppm(args.dump_ppm, args.replay)
    if args.selftest:
        return run_selftest()
    if args.replay:
        return run_replay(args.replay)
    return run_live(args.port)


if __name__ == "__main__":
    sys.exit(main())
