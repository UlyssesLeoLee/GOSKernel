#!/usr/bin/env python3
"""
GOS FPS/mouse test. Reads FBF events from QEMU stdout (COM1 = -serial stdio).
Tests WHPX vs TCG and checks mouse pixel changes.
"""
import subprocess, time, threading, sys, socket

QEMU = r"C:\Program Files\qemu\qemu-system-x86_64.exe"
BIN  = r"E:\DevCache\cargo\target\x86_64-gos-kernel\debug\bootimage-gos-kernel.bin"
MON  = ("127.0.0.1", 45555)

def run(accel, duration=35):
    cmd = [QEMU,
           "-drive", f"format=raw,file={BIN}",
           "-serial", "stdio",
           "-no-reboot",
           "-display", "none",
           "-monitor", f"telnet:{MON[0]}:{MON[1]},server,nowait",
           "-accel", accel, "-accel", "tcg"]
    print(f"\n=== {accel} test ===")
    lines = []
    proc = subprocess.Popen(cmd,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL)

    def reader():
        for raw in proc.stdout:
            lines.append(raw.decode(errors="replace").rstrip())

    th = threading.Thread(target=reader, daemon=True)
    th.start()

    # Wait for kernel to finish booting (desktop graph line)
    boot_time = None
    t0 = time.time()
    while time.time() - t0 < 90:
        for l in lines:
            if "desktop: graph" in l and boot_time is None:
                boot_time = time.time() - t0
                print(f"  Boot done at +{boot_time:.1f}s")
        if boot_time:
            break
        time.sleep(0.3)
    if not boot_time:
        print("  ERROR: did not boot in 90s")
        for l in lines[-15:]:
            print(f"    {l}")
        proc.terminate(); proc.wait()
        return None

    # Collect FBF lines for `duration` seconds after boot
    t_measure_start = time.time()
    t_measure_end   = t_measure_start + duration
    seen_fbf = set()
    fbf_records = []

    while time.time() < t_measure_end:
        for l in lines:
            if l.startswith("FBF ") and l not in seen_fbf:
                seen_fbf.add(l)
                rel = time.time() - t_measure_start
                fbf_records.append((rel, l))
        time.sleep(0.5)

    # Quick mouse test via monitor
    changed = None
    mon_sock = None
    for _ in range(20):
        try:
            s = socket.socket()
            s.settimeout(2)
            s.connect(MON)
            s.recv(4096)
            mon_sock = s
            break
        except Exception:
            time.sleep(0.5)

    if mon_sock:
        def mcmd(c):
            mon_sock.sendall((c+"\n").encode())
            time.sleep(0.08)
            try:
                mon_sock.settimeout(0.2)
                return mon_sock.recv(4096).decode(errors="replace")
            except Exception:
                return ""

        mcmd("screendump c:/tmp/gos_before.png")
        time.sleep(0.8)
        # Move mouse to different positions
        for dx, dy in [(200,200),(300,150),(400,300),(200,400),(350,250)]:
            mcmd(f"mouse_move {dx} {dy}")
            time.sleep(0.05)
        time.sleep(1.5)
        mcmd("screendump c:/tmp/gos_after.png")
        time.sleep(0.5)
        mon_sock.close()

        # Diff
        try:
            from PIL import Image
            a = list(Image.open("c:/tmp/gos_before.png").getdata())
            b = list(Image.open("c:/tmp/gos_after.png").getdata())
            changed = sum(1 for x,y in zip(a,b) if x != y)
        except Exception as e:
            print(f"  PIL diff error: {e}")

    proc.terminate()
    try: proc.wait(timeout=5)
    except Exception: proc.kill()

    # Compute FPS
    if fbf_records:
        # "FBF N" where N is cumulative frames
        first_t, first_l = fbf_records[0]
        first_n = int(first_l.split()[1])
        fps_boot = first_n / first_t if first_t > 0 else 0

        if len(fbf_records) >= 2:
            last_t, last_l = fbf_records[-1]
            last_n = int(last_l.split()[1])
            fps_steady = (last_n - first_n) / max(last_t - first_t, 0.001)
        else:
            fps_steady = fps_boot

        print(f"  FBF events: {len(fbf_records)}")
        print(f"  First: {first_l} at t={first_t:.1f}s → {fps_boot:.1f} FPS avg")
        if len(fbf_records) >= 2:
            print(f"  Last:  {last_l} at t={last_t:.1f}s → {fps_steady:.1f} FPS steady")
        if changed is not None:
            print(f"  Mouse pixel diff: {changed}")
        return {"fps": fps_steady, "fps_boot": fps_boot, "changed": changed}
    else:
        print(f"  No FBF events in {duration}s")
        print(f"  Last serial lines:")
        for l in lines[-10:]:
            print(f"    {l}")
        return None

if __name__ == "__main__":
    r_whpx = run("whpx", duration=35)
    time.sleep(2)
    r_tcg  = run("tcg",  duration=35)

    print("\n" + "="*50)
    print("SUMMARY")
    if r_whpx: print(f"  WHPX: {r_whpx['fps_boot']:.1f} FPS avg, {r_whpx.get('fps',0):.1f} FPS steady")
    if r_tcg:  print(f"  TCG:  {r_tcg['fps_boot']:.1f} FPS avg,  {r_tcg.get('fps',0):.1f} FPS steady")
    if r_whpx and r_tcg and r_tcg.get('fps_boot',0) > 0:
        ratio = r_whpx.get('fps_boot',0) / r_tcg['fps_boot']
        print(f"  Speedup: {ratio:.1f}x")
    print("="*50)
