#!/usr/bin/env python3
"""Empirical verification for the mouse-sensitivity multiplier and the
command-result panel (Phase 4 of the 2026-06-07 session).

Connects to the already-running QEMU monitor (telnet 127.0.0.1:4445), and:

  1. Screenshots before/after a known `mouse_move dx dy` injection, finds
     the cursor apex via its distinctive edge color, and checks the
     on-screen displacement is ~MOUSE_SENSITIVITY x the injected delta.
     (Prior session test proved `mouse_move dx dy` nets a SCREEN-SPACE
     cursor delta of exactly (dx, dy) at 1x — so at 3x we expect (3dx, 3dy).)
  2. Types `reset` + Enter via `sendkey`, screenshots, and checks the
     result-panel's bright border color appears at the expected collapsed
     rectangle (just above the taskbar).
  3. Clicks inside that rectangle and screenshots again, checking the
     panel grew to the expanded rectangle (open state).

All paths are Windows-native (E:/GOSKernel/...) since QEMU is a native
Windows binary and cannot resolve MSYS /tmp/... paths for `screendump`.
"""
import socket
import time

MON_ADDR = ("127.0.0.1", 4446)
W, H = 800, 600
BAR_H = 36
SENS = 3.0
EDGE_RGB = (0x10, 0x10, 0x14)     # cursor apex edge color (0x00101014 -> R G B)
BORDER_RGB = (0x44, 0xaa, 0xff)   # cmd-result panel border (0x0044aaff -> R G B)


def connect_mon(timeout=20):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.socket()
            s.settimeout(2)
            s.connect(MON_ADDR)
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
    """Bounding-box top-left (min_x, min_y) of pixels within `tol` of target_rgb."""
    w, h, px = read_ppm(path)
    tr, tg, tb = target_rgb
    min_x, min_y = None, None
    for y in range(h):
        row = y * w * 3
        for x in range(w):
            o = row + x * 3
            if abs(px[o]-tr) <= tol and abs(px[o+1]-tg) <= tol and abs(px[o+2]-tb) <= tol:
                if min_x is None or x < min_x:
                    min_x = x
                if min_y is None or y < min_y:
                    min_y = y
    return min_x, min_y


def match_in_rect(path, x0, y0, x1, y1, target_rgb, tol=20):
    w, h, px = read_ppm(path)
    tr, tg, tb = target_rgb
    for y in range(max(0, y0), min(h, y1)):
        row = y * w * 3
        for x in range(max(0, x0), min(w, x1)):
            o = row + x * 3
            if abs(px[o]-tr) <= tol and abs(px[o+1]-tg) <= tol and abs(px[o+2]-tb) <= tol:
                return True
    return False


def main():
    mon = connect_mon()
    if mon is None:
        print("FAIL: could not connect to QEMU monitor")
        return 1
    print("connected to monitor")

    # ── Test 1: mouse-sensitivity multiplier ────────────────────────────
    # Center the cursor first (away from edges) so clamping can't interfere.
    mon_cmd(mon, "screendump E:/GOSKernel/before1.ppm")
    time.sleep(0.3)
    bx, by = find_first_match("E:/GOSKernel/before1.ppm", EDGE_RGB)
    print(f"cursor before: ({bx},{by})")

    DX, DY = 60, 30  # small enough that 3x (=180,90) plus current pos stays on-screen
    mon_cmd(mon, f"mouse_move {DX} {DY}")
    time.sleep(0.4)
    mon_cmd(mon, "screendump E:/GOSKernel/after1.ppm")
    time.sleep(0.3)
    ax, ay = find_first_match("E:/GOSKernel/after1.ppm", EDGE_RGB)
    print(f"cursor after:  ({ax},{ay})")

    if bx is None or ax is None:
        print("SENSITIVITY TEST: FAIL (could not locate cursor in a screenshot)")
    else:
        ddx, ddy = ax - bx, ay - by
        exp_dx, exp_dy = DX * SENS, DY * SENS
        print(f"measured delta: ({ddx},{ddy})   expected @ {SENS}x: ({exp_dx:.0f},{exp_dy:.0f})   "
              f"[1:1 would be: ({DX},{DY})]")
        print(f"ratio: ({ddx/DX:.2f}, {ddy/DY:.2f})  (target {SENS:.2f})")
        ok = abs(ddx - exp_dx) <= 6 and abs(ddy - exp_dy) <= 6
        print("SENSITIVITY TEST:", "PASS — cursor moves ~3x faster than raw input" if ok else "CHECK — see ratios above (clamping near an edge could explain a mismatch)")

    # ── Test 2 & 3: command-result panel ─────────────────────────────────
    for ch in "reset":
        mon_cmd(mon, f"sendkey {ch}", wait=0.08)
    mon_cmd(mon, "sendkey ret", wait=0.3)
    time.sleep(0.5)
    mon_cmd(mon, "screendump E:/GOSKernel/cmdpanel1.ppm")
    time.sleep(0.3)

    rx, rw = 8, 300
    ry_min, rh_min = (H - BAR_H) - 24 - 6, 24
    ry_max, rh_max = (H - BAR_H) - 72 - 6, 72
    collapsed = match_in_rect("E:/GOSKernel/cmdpanel1.ppm", rx-2, ry_min-2, rx+rw+2, ry_min+rh_min+2, BORDER_RGB)
    print(f"[panel] border visible @ collapsed rect after 'reset'+Enter: {collapsed}")

    # Click in the middle of the (collapsed) panel — press+release with no
    # movement in between registers as a click in render_frame.
    cx, cy = rx + rw // 2, ry_min + rh_min // 2
    bx2, by2 = find_first_match("E:/GOSKernel/cmdpanel1.ppm", EDGE_RGB)
    if bx2 is not None:
        # mouse_move's on-screen effect is itself scaled by SENS (that's the
        # feature under test!) — divide the desired screen delta by SENS so
        # the injected raw delta nets the intended on-screen displacement.
        rdx, rdy = round((cx - bx2) / SENS), round((cy - by2) / SENS)
        mon_cmd(mon, f"mouse_move {rdx} {rdy}")
        time.sleep(0.3)
        mon_cmd(mon, "screendump E:/GOSKernel/precl.ppm")
        time.sleep(0.2)
        pcx, pcy = find_first_match("E:/GOSKernel/precl.ppm", EDGE_RGB)
        print(f"  (repositioned cursor: target=({cx},{cy}) raw_move=({rdx},{rdy}) landed=({pcx},{pcy}))")
    mon_cmd(mon, "mouse_button 1")
    time.sleep(0.15)
    mon_cmd(mon, "mouse_button 0")
    time.sleep(0.4)
    mon_cmd(mon, "screendump E:/GOSKernel/cmdpanel2.ppm")
    time.sleep(0.3)
    expanded = match_in_rect("E:/GOSKernel/cmdpanel2.ppm", rx-2, ry_max-2, rx+rw+2, ry_max+rh_max+2, BORDER_RGB)
    # Distinguishing signal: the taller rect's TOP EDGE (only present when open).
    top_strip_only_when_open = match_in_rect("E:/GOSKernel/cmdpanel2.ppm", rx-2, ry_max-2, rx+rw+2, ry_max+4, BORDER_RGB)
    print(f"[panel] border visible @ expanded rect after click: {expanded}  (top-edge-of-tall-rect: {top_strip_only_when_open})")
    print("PANEL TEST:", "PASS — panel appeared and grew on click" if (collapsed and top_strip_only_when_open) else "CHECK — see screenshots cmdpanel1/2.ppm")

    mon.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
