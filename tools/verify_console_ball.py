#!/usr/bin/env python3
"""Empirical verification for the Console <-> 3D "minimized red sphere" feature
(2026-06-07 session): the kernel console can now be closed back to the 3D
desktop via ESC or a top-right X button, and while closed it appears in the
3D scene as its own standalone red sphere; clicking that sphere reopens the
console.

Checks, against a freshly booted headless QEMU instance:
  1. The red sphere is visible in the 3D scene at its expected projected
     position (CONSOLE_BALL_POS=(0,5.4,0) sits exactly on the camera's yaw
     pivot (the Y axis), so its screen X is W/2 and its screen Y depends only
     on the (constant, never auto-animated) initial pitch -- independent of
     however long auto-spin has been running).
  2. Clicking that sphere opens the console (color/usable-region switches).
  3. ESC closes the console back to the 3D scene (sphere reappears).
  4. Right-click reopens the console (regression: old toggle still works).
  5. Clicking the top-right X button closes the console back to the 3D scene.

All paths are Windows-native (E:/GOSKernel/...) since QEMU is a native Windows
binary and cannot resolve MSYS /tmp/... paths for `screendump`.
"""
import math
import socket
import subprocess
import threading
import time

QEMU = r"C:\Program Files\qemu\qemu-system-x86_64.exe"
BIN = r"E:\DevCache\cargo\target\x86_64-gos-kernel\debug\bootimage-gos-kernel.bin"
MON_ADDR = ("127.0.0.1", 45556)
SER_ADDR = ("127.0.0.1", 14446)
SHOT_DIR = "E:/GOSKernel"

W, H = 1920, 1080
FOCAL = 724.0
CAM_D = 11.0
SENS = 3.0                      # MOUSE_SENSITIVITY in fbtest.rs
EDGE_RGB = (0x10, 0x10, 0x14)   # cursor apex edge color

# Console-ball world position & radius (CONSOLE_BALL_POS / _R in fbtest.rs).
BALL_POS = (0.0, 5.4, 0.0)
BALL_R = 0.55
INIT_PITCH = 0.35               # Desktop::default pitch (yaw auto-spins, pitch does not)

# Close-button rect (CLOSE_BTN_* in fbtest.rs): X=W-12-34=1874, Y=12, 34x26.
CLOSE_BTN_X, CLOSE_BTN_Y, CLOSE_BTN_W, CLOSE_BTN_H = W - 12 - 34, 12, 34, 26


def project_pt(x, y, z, yaw, pitch):
    """Mirrors fbtest.rs::project_pt exactly."""
    cyw, syw = math.cos(yaw), math.sin(yaw)
    cp, sp = math.cos(pitch), math.sin(pitch)
    x1 = x * cyw + z * syw
    z1 = -x * syw + z * cyw
    y2 = y * cp - z1 * sp
    z2 = y * sp + z1 * cp
    d = CAM_D - z2
    inv = FOCAL / d
    return (W * 0.5 + x1 * inv, H * 0.5 - y2 * inv, d)


def connect(addr, timeout=30, drain=True):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.socket()
            s.settimeout(2)
            s.connect(addr)
            if drain:
                s.recv(4096)
            return s
        except Exception:
            time.sleep(0.5)
    return None


def mon_cmd(s, cmd, wait=0.15):
    s.sendall((cmd + "\n").encode())
    time.sleep(wait)
    try:
        s.settimeout(0.3)
        return s.recv(8192).decode(errors="replace")
    except Exception:
        return ""


def read_ppm(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:2] == b"P6"
    idx = 2
    fields = []
    while len(fields) < 3:
        while data[idx] in b" \t\r\n":
            idx += 1
        start = idx
        while data[idx] not in b" \t\r\n":
            idx += 1
        fields.append(int(data[start:idx]))
    idx += 1
    w, h, _maxval = fields
    pixels = data[idx:idx + w * h * 3]
    return w, h, pixels


def find_first_match(path, target_rgb, tol=2):
    w, h, px = read_ppm(path)
    tr, tg, tb = target_rgb
    min_x = min_y = None
    for y in range(h):
        row = y * w * 3
        for x in range(w):
            o = row + x * 3
            if abs(px[o] - tr) <= tol and abs(px[o+1] - tg) <= tol and abs(px[o+2] - tb) <= tol:
                if min_x is None or x < min_x:
                    min_x = x
                if min_y is None or y < min_y:
                    min_y = y
    return min_x, min_y


def count_reddish_in_disc(path, cx, cy, r):
    """Count "reddish" pixels within radius r of (cx,cy).

    draw_sphere applies Phong shading: rr = base.0*it + 0.6*sp (and similarly
    g/b), where `it` (the diffuse term) sweeps ~0.15..1.0 across the visible
    disc and `sp` (a sharp specular highlight) is ~0 except in one small spot.
    Exact-RGB matching would therefore only catch one brightness band; instead
    test the *hue ratio*, which `it` scales uniformly and so cancels out:
    PAL_RGB[0] = (0.86, 0.11, 0.13) -> R is ~6-8x G and ~6-8x B everywhere
    on the lit body (the only places that ratio collapses are the small
    specular spot, which trends toward white/pink, and the background/UI,
    which are all roughly neutral or differently-hued -- see comments at
    the call site for why neither produces a false positive here).
    """
    w, h, px = read_ppm(path)
    n = 0
    x0, x1 = max(0, int(cx - r)), min(w, int(cx + r) + 1)
    y0, y1 = max(0, int(cy - r)), min(h, int(cy + r) + 1)
    for y in range(y0, y1):
        row = y * w * 3
        for x in range(x0, x1):
            if (x - cx) ** 2 + (y - cy) ** 2 > r * r:
                continue
            o = row + x * 3
            rr, gg, bb = px[o], px[o+1], px[o+2]
            if rr > 25 and rr > 1.8 * gg and rr > 1.8 * bb:
                n += 1
    return n


def shot(mon, name):
    path = f"{SHOT_DIR}/{name}.ppm"
    mon_cmd(mon, f"screendump {path}")
    time.sleep(0.35)
    return path


def click_at(mon, last_shot, tx, ty, button=1, label=""):
    """Locate the cursor in `last_shot`, move it to (tx,ty) via mouse_move
    (raw delta = desired screen delta / SENS, per the empirically-verified
    mapping in tools/verify_phase4.py), then press+release `button` -- a
    QEMU monitor `mouse_button` state value, which is NOT X11-style: 1=left,
    2=right, 4=middle (verified in tools/debug_rclick.py against the wire-bit
    positions the kernel's PS2 driver actually checks) -- with no movement
    in between -> registers as a click.

    Timing here is generous on purpose. Temporary serial instrumentation
    this session (DBG[frame] prints on every observed button-state change,
    since removed) measured the headless QEMU monitor's command -> guest
    -observed latency directly: a `mouse_button` sent at T is typically not
    seen by the kernel's PS/2 driver until ~T+0.9..1.1s, and that latency
    visibly jitters run to run. A short (~0.3s) press/release gap can
    therefore collapse below the kernel's ~150ms input-poll granularity --
    the kernel then observes only the *net* button state (no transition ->
    nothing happens), which is exactly the "clicking the ball does nothing"
    symptom this rewrite fixes (it is a test-timing artifact, not a kernel
    bug -- the very same click, held longer, opens the console correctly
    with `ball_hit=true` every time). Holding well past the measured latency
    makes the press and release edges land in clearly separate polls.
    """
    bx, by = find_first_match(last_shot, EDGE_RGB)
    print(f"  [{label}] cursor before: ({bx},{by})  ->  target: ({tx},{ty})")
    if bx is not None:
        rdx, rdy = round((tx - bx) / SENS), round((ty - by) / SENS)
        mon_cmd(mon, f"mouse_move {rdx} {rdy}")
        time.sleep(0.6)
    mon_cmd(mon, f"mouse_button {button}")
    time.sleep(1.6)
    mon_cmd(mon, "mouse_button 0")
    time.sleep(1.2)


def wait_for(mon, name, predicate, timeout=15.0, interval=1.5):
    """Poll fresh screendumps until `predicate(path)` is true or `timeout`
    elapses. Returns (ok, last_screenshot_path).

    Same root cause as click_at's generous timing: with ~1s of click-to
    -effect latency in this headless setup, a state change triggered by a
    click can land more than a second after click_at() returns. Rather than
    guess a single fixed post-click delay (fragile -- exactly the kind of
    assumption that hid the real story above), keep sampling actual pixels
    until the expected state appears or we give up. Same "trust the
    measurement, not the guess" spirit as this script's yaw-invariant
    position prediction and Phong-robust hue-ratio color test.
    """
    deadline = time.time() + timeout
    path = shot(mon, name)
    while True:
        if predicate(path):
            return True, path
        if time.time() >= deadline:
            return False, path
        time.sleep(interval)
        path = shot(mon, name)


def main():
    mon = None
    print("Launching QEMU (headless)...")
    # raw_serial_println (which prints "boot: ..." / "desktop: graph n=" /
    # "LOOP #N") writes to port 0x3F8 = COM1 = the FIRST -serial backend,
    # i.e. "stdio" -> this process's stdout. (COM2/COM3, the later -serial
    # args, carry separate protocols -- the k-chat bridge and @gos.vk -- not
    # this banner; SER_ADDR is unused for boot detection because of this.)
    proc = subprocess.Popen([
        QEMU,
        "-drive", f"format=raw,file={BIN}",
        "-serial", "stdio",
        "-serial", f"tcp:{SER_ADDR[0]}:{SER_ADDR[1]},server,nowait",
        "-no-reboot",
        "-display", "none",
        "-monitor", f"telnet:{MON_ADDR[0]}:{MON_ADDR[1]},server,nowait",
        "-accel", "whpx", "-accel", "tcg",
    ], stdout=subprocess.PIPE, stderr=subprocess.STDOUT)

    boot_lines = []

    def read_stdout():
        for raw in iter(proc.stdout.readline, b""):
            boot_lines.append(raw.decode(errors="replace").rstrip("\r\n"))

    reader = threading.Thread(target=read_stdout, daemon=True)
    reader.start()

    try:
        mon = connect(MON_ADDR)
        if mon is None:
            print("FAIL: could not connect to QEMU monitor")
            return 1
        print("connected to monitor")

        # Wait for the desktop to come up by polling captured stdout (COM1)
        # for the "desktop: graph n=" banner (same signal used earlier this
        # session, e.g. "desktop: graph n=26 edges=50 lfb=true").
        deadline = time.time() + 90
        booted = False
        while time.time() < deadline:
            if any("desktop: graph n=" in ln for ln in boot_lines):
                booted = True
                break
            time.sleep(0.5)
        print(f"booted: {booted}")
        if not booted:
            print("FAIL: desktop never came up; last captured lines:")
            for ln in boot_lines[-15:]:
                print("   ", ln)
            return 1
        for ln in boot_lines:
            if "desktop: graph n=" in ln or "kernel_main entered" in ln:
                print("   ", ln)

        # Let a few frames render and the cursor settle.
        time.sleep(3.0)

        # ---- Expected ball screen position -------------------------------
        # The ball sits at world (0, 5.4, 0) -- exactly on the Y axis, which
        # is the camera's yaw-pivot, so x1=z1=0 for *any* yaw: its projection
        # is yaw-invariant and depends only on pitch (which auto-spin never
        # touches). Compute it at the known initial pitch=0.35.
        bsx, bsy, bd = project_pt(*BALL_POS, yaw=0.0, pitch=INIT_PITCH)
        bsr = BALL_R * FOCAL / bd
        area = math.pi * bsr * bsr
        print(f"expected ball: center=({bsx:.1f},{bsy:.1f}) r={bsr:.1f} area~={area:.0f}px")

        def ball_present(path, label):
            n = count_reddish_in_disc(path, bsx, bsy, bsr + 4)
            frac = n / area
            print(f"    [{label}] reddish pixels inside expected ball disc: {n}/{area:.0f} ({frac:.0%})")
            return frac > 0.5

        # ---- 1. Sphere visible in the 3D scene ---------------------------
        s1 = shot(mon, "ball_3d")
        t1 = ball_present(s1, "1: initial 3D scene")
        print("    BALL VISIBLE:", "PASS" if t1 else "FAIL")

        # The console overlay fills the screen with a dark background and
        # draws the X button at (CLOSE_BTN_X..+W, CLOSE_BTN_Y..+H); the 3D
        # scene never puts opaque UI there. Detect "console is up" by the
        # presence of the close button's bright top/bottom border accents
        # (0x00aa5060 -> RGB (0xaa,0x50,0x60)) inside that thin rect, which
        # only draw_close_button ever paints.
        BORDER = (0xaa, 0x50, 0x60)
        cx0, cy0 = CLOSE_BTN_X - 2, CLOSE_BTN_Y - 2
        cx1, cy1 = CLOSE_BTN_X + CLOSE_BTN_W + 2, CLOSE_BTN_Y + CLOSE_BTN_H + 2

        def console_open(path):
            w, h, px = read_ppm(path)
            for yy in range(max(0, cy0), min(h, cy1)):
                row = yy * w * 3
                for xx in range(max(0, cx0), min(w, cx1)):
                    o = row + xx * 3
                    if abs(px[o]-BORDER[0]) <= 30 and abs(px[o+1]-BORDER[1]) <= 30 and abs(px[o+2]-BORDER[2]) <= 30:
                        return True
            return False

        def ball_quiet(path):
            return count_reddish_in_disc(path, bsx, bsy, bsr + 4) / area > 0.5

        # ---- 2. Click the ball -> console opens ---------------------------
        click_at(mon, s1, bsx, bsy, button=1, label="click ball")
        t2, _ = wait_for(mon, "console_via_ball", console_open)
        print("    CONSOLE OPENED VIA BALL CLICK:", "PASS" if t2 else "FAIL", f"(close-button border detected: {t2})")

        # ---- 3. ESC -> back to the 3D scene (ball reappears) -------------
        mon_cmd(mon, "sendkey esc")
        t3, s3 = wait_for(mon, "after_esc", ball_quiet)
        ball_present(s3, "3: after ESC")
        print("    ESC RETURNS TO 3D:", "PASS" if t3 else "FAIL")

        # ---- 4. Right-click reopens the console (regression) -------------
        # QEMU's legacy monitor `mouse_button <state>` is NOT X11-style --
        # empirically verified (tools/debug_rclick.py, single-boot A/B against
        # the proven console_open signal): state=2 toggles the console (it
        # maps to INPUT_BUTTON_RIGHT -> PS2 wire bit 0x02, exactly the
        # `btn & 0x02` the kernel checks -- see the toggle above) while
        # state=4 lands on INPUT_BUTTON_MIDDLE and the kernel correctly
        # ignores it. So 2 is "right" here, matching the wire-bit positions
        # (left=0x01, right=0x02, middle=0x04) rather than X11 button masks.
        click_at(mon, s3, W // 2, H // 2, button=2, label="right-click center")
        t4, s4 = wait_for(mon, "console_via_rclick", console_open)
        print("    RIGHT-CLICK REOPENS CONSOLE (regression):", "PASS" if t4 else "FAIL")
        if not t4:
            # Either a real regression, or this script's guess at QEMU's
            # right-button code is off -- either way, step 5 below needs the
            # console actually open to mean anything, so fall back to the
            # already-proven ball click rather than let a wrong guess here
            # cascade into a misleading step 5.
            print("    (falling back to ball-click to reopen the console for step 5)")
            click_at(mon, s4, bsx, bsy, button=1, label="fallback: click ball")
            ok, s4 = wait_for(mon, "console_reopened_fallback", console_open)
            print(f"    console open after fallback: {ok}")

        # ---- 5. Click the X button -> back to the 3D scene ---------------
        bx_cx = CLOSE_BTN_X + CLOSE_BTN_W / 2
        bx_cy = CLOSE_BTN_Y + CLOSE_BTN_H / 2
        click_at(mon, s4, bx_cx, bx_cy, button=1, label="click X button")
        t5, s5 = wait_for(mon, "after_xbutton", ball_quiet)
        ball_present(s5, "5: after X-button click")
        print("    X BUTTON RETURNS TO 3D:", "PASS" if t5 else "FAIL")

        print()
        all_pass = t1 and t2 and t3 and t4 and t5
        print("OVERALL:", "ALL PASS" if all_pass else "SOME CHECKS FAILED -- inspect screenshots in", SHOT_DIR)
        return 0 if all_pass else 1
    finally:
        if mon is not None:
            try:
                mon_cmd(mon, "quit")
            except Exception:
                pass
        try:
            proc.terminate()
            proc.wait(timeout=10)
        except Exception:
            try:
                proc.kill()
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
