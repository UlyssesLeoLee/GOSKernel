#!/usr/bin/env python3
"""
FPS + mouse responsiveness test with WHPX hardware acceleration.
Measures FPS before and after WHPX, then checks cursor movement.
"""
import subprocess, time, os, sys, threading

QEMU = r"C:\Program Files\qemu\qemu-system-x86_64.exe"
BIN  = r"E:\DevCache\cargo\target\x86_64-gos-kernel\debug\bootimage-gos-kernel.bin"
MON_ADDR = ("127.0.0.1", 45555)
SER_ADDR = ("127.0.0.1", 14444)

def qemu_cmd(accel):
    base = [
        QEMU,
        "-drive", f"format=raw,file={BIN}",
        "-serial", "stdio",
        "-serial", f"tcp:{SER_ADDR[0]}:{SER_ADDR[1]},server,nowait",
        "-no-reboot",
        "-display", "none",
        "-monitor", f"telnet:{MON_ADDR[0]}:{MON_ADDR[1]},server,nowait",
    ]
    if accel == "whpx":
        base += ["-accel", "whpx", "-accel", "tcg"]
    else:
        base += ["-accel", "tcg"]
    return base

import socket

def connect_mon(timeout=30):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            s = socket.socket()
            s.settimeout(2)
            s.connect(MON_ADDR)
            s.recv(1024)  # drain banner
            return s
        except Exception:
            time.sleep(0.5)
    return None

def mon_cmd(s, cmd):
    s.sendall((cmd + "\n").encode())
    time.sleep(0.05)
    try:
        s.settimeout(0.2)
        return s.recv(4096).decode(errors="replace")
    except Exception:
        return ""

def run_test(accel_name, accel_arg, test_duration=45):
    print(f"\n{'='*60}")
    print(f"  Testing with accel={accel_name}")
    print(f"{'='*60}")

    serial_lines = []
    proc = None

    def read_serial():
        import socket as _s
        deadline = time.time() + 10
        sock = None
        while time.time() < deadline:
            try:
                sock = _s.socket()
                sock.settimeout(3)
                sock.connect(SER_ADDR)
                break
            except Exception:
                time.sleep(0.5)
        if not sock:
            return
        sock.settimeout(0.5)
        buf = b""
        while True:
            try:
                chunk = sock.recv(4096)
                if not chunk:
                    break
                buf += chunk
                while b"\n" in buf:
                    line, buf = buf.split(b"\n", 1)
                    serial_lines.append(line.decode(errors="replace"))
            except Exception:
                if proc and proc.poll() is not None:
                    break

    try:
        proc = subprocess.Popen(
            qemu_cmd(accel_arg),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
        )
        t = threading.Thread(target=read_serial, daemon=True)
        t.start()

        # Wait for "desktop: graph" to appear (kernel is running)
        deadline = time.time() + 60
        booted = False
        while time.time() < deadline:
            if any("desktop: graph" in l for l in serial_lines):
                booted = True
                boot_time = time.time()
                print(f"  Boot complete at +{boot_time - start:.1f}s")
                break
            time.sleep(0.3)

        if not booted:
            print("  ERROR: kernel did not boot in 60 seconds")
            # Print last serial lines for diagnosis
            for l in serial_lines[-10:]:
                print(f"    serial: {l}")
            return None

        # Collect FBF (frame counter) events for test_duration seconds
        fbf_times = []
        t_start = time.time()
        t_end   = t_start + test_duration

        while time.time() < t_end:
            # Check for new FBF lines
            for l in serial_lines:
                if l.startswith("FBF "):
                    ts = time.time() - t_start
                    if ts not in [x[0] for x in fbf_times]:
                        fbf_times.append((ts, l.strip()))
            time.sleep(1.0)

        # Now try mouse movement via monitor
        mon = connect_mon(timeout=10)
        cursor_before = None
        cursor_after  = None

        if mon:
            # Screendump before mouse move
            mon_cmd(mon, "screendump /tmp/before.png")
            time.sleep(0.5)

            # Move mouse significantly
            for _ in range(5):
                mon_cmd(mon, "mouse_move 150 150")
                time.sleep(0.05)

            time.sleep(1.0)  # wait for render

            # Screendump after
            mon_cmd(mon, "screendump /tmp/after.png")
            time.sleep(0.5)

            # Count changed pixels
            try:
                from PIL import Image
                img_a = Image.open("/tmp/before.png")
                img_b = Image.open("/tmp/after.png")
                pa = list(img_a.getdata())
                pb = list(img_b.getdata())
                diff = sum(1 for a, b in zip(pa, pb) if a != b)
                cursor_changed = diff
            except Exception as e:
                cursor_changed = -1
                print(f"  PIL error: {e}")

            mon.close()

        # Compute FPS
        if len(fbf_times) >= 1:
            # Each FBF line is "FBF N" where N is cumulative frame count
            # First FBF at time t0 means we got N frames in t0 seconds
            first_ts, first_line = fbf_times[0]
            first_n = int(first_line.split()[1])
            fps_avg = first_n / first_ts if first_ts > 0 else 0

            last_ts, last_line = fbf_times[-1]
            last_n = int(last_line.split()[1])

            if len(fbf_times) >= 2:
                # FPS between first and last
                dt = last_ts - first_ts
                dn = last_n - first_n
                fps_recent = dn / dt if dt > 0 else fps_avg
            else:
                fps_recent = fps_avg
        else:
            fps_avg = 0
            fps_recent = 0
            first_n = 0
            last_n = 0

        print(f"\n  ── Results ({accel_name}) ──")
        print(f"  FBF events seen: {len(fbf_times)}")
        if fbf_times:
            print(f"  First FBF: {fbf_times[0][1]} at t={fbf_times[0][0]:.1f}s")
            print(f"  Last  FBF: {fbf_times[-1][1]} at t={fbf_times[-1][0]:.1f}s")
            print(f"  Avg FPS (boot→first60): {fps_avg:.1f}")
            print(f"  Recent FPS:             {fps_recent:.1f}")
        if cursor_changed is not None and cursor_changed >= 0:
            print(f"  Pixels changed on mouse_move: {cursor_changed}")
        elif cursor_changed == -1:
            print(f"  (PIL not available for diff)")

        return {"fps_avg": fps_avg, "fps_recent": fps_recent,
                "fbf_count": len(fbf_times), "changed": cursor_changed if cursor_changed else 0}

    finally:
        if proc and proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except Exception:
                proc.kill()


if __name__ == "__main__":
    start = time.time()
    print("GOS Kernel FPS/Mouse Test — WHPX vs TCG")
    print(f"Binary: {BIN}")
    print(f"QEMU:   {QEMU}")

    result_whpx = run_test("WHPX+TCG", "whpx", test_duration=40)
    time.sleep(2)
    result_tcg  = run_test("TCG-only", "tcg",  test_duration=40)

    print("\n" + "="*60)
    print("  COMPARISON SUMMARY")
    print("="*60)
    if result_whpx:
        print(f"  WHPX avg FPS:    {result_whpx['fps_avg']:.1f}")
        print(f"  WHPX recent FPS: {result_whpx['fps_recent']:.1f}")
    if result_tcg:
        print(f"  TCG  avg FPS:    {result_tcg['fps_avg']:.1f}")
        print(f"  TCG  recent FPS: {result_tcg['fps_recent']:.1f}")
    if result_whpx and result_tcg and result_tcg.get('fps_avg', 0) > 0:
        ratio = result_whpx.get('fps_avg', 0) / result_tcg['fps_avg']
        print(f"  Speedup: {ratio:.1f}x")
    print("="*60)
