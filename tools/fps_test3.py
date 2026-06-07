#!/usr/bin/env python3
"""
GOS FPS + per-phase timing test.
Reads FBF and PERF lines from QEMU stdout (COM1 = -serial stdio).
PERF lines look like:
  PERF fill=Xµs rope=Yµs sphere=Zµs ring=Wµs ui=Vµs
"""
import subprocess, time, threading, socket, re

QEMU = r"C:\Program Files\qemu\qemu-system-x86_64.exe"
BIN  = r"E:\DevCache\cargo\target\x86_64-gos-kernel\debug\bootimage-gos-kernel.bin"
MON  = ("127.0.0.1", 45555)

def run(accel="whpx", duration=40):
    cmd = [QEMU,
           "-drive", f"format=raw,file={BIN}",
           "-serial", "stdio",
           "-no-reboot",
           "-display", "none",
           "-monitor", f"telnet:{MON[0]}:{MON[1]},server,nowait",
           "-accel", accel, "-accel", "tcg"]
    print(f"\n=== {accel} profiling test ===")
    lines = []
    proc = subprocess.Popen(cmd,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        stdin=subprocess.DEVNULL)

    def reader():
        for raw in proc.stdout:
            l = raw.decode(errors="replace").rstrip()
            lines.append(l)

    th = threading.Thread(target=reader, daemon=True)
    th.start()

    # Wait for boot
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
        proc.terminate(); proc.wait()
        return None

    # Collect for `duration` seconds
    t_end = time.time() + duration
    seen = set()
    fbf_records  = []
    perf_records = []

    while time.time() < t_end:
        for l in lines:
            if l not in seen:
                seen.add(l)
                if l.startswith("FBF "):
                    fbf_records.append((time.time() - boot_time - t0 + boot_time, l))
                if l.startswith("PERF "):
                    perf_records.append(l)
        time.sleep(0.2)

    proc.terminate()
    try: proc.wait(timeout=5)
    except Exception: proc.kill()

    # FPS
    fps = None
    if fbf_records:
        first_t, first_l = fbf_records[0]
        first_n = int(first_l.split()[1])
        fps = first_n / first_t if first_t > 0 else 0
        print(f"  FBF: {len(fbf_records)} events, first={first_l.strip()} @ {first_t:.1f}s → {fps:.2f} FPS avg")
    else:
        print(f"  No FBF events in {duration}s")

    # PERF
    if perf_records:
        # Parse all PERF lines and average each field
        pattern = re.compile(r'(\w+)=(\d+)µs')
        totals = {}
        counts = {}
        for line in perf_records:
            for key, val in pattern.findall(line):
                totals[key] = totals.get(key, 0) + int(val)
                counts[key] = counts.get(key, 0) + 1
        print(f"  PERF samples: {len(perf_records)}")
        print(f"  Per-phase avg (µs) over {len(perf_records)} samples:")
        fields = ["fill", "rope", "sphere", "ring", "ui"]
        total_frame = 0
        for f in fields:
            if f in totals:
                avg = totals[f] // counts[f]
                total_frame += avg
                print(f"    {f:8s}: {avg:6d} µs")
        print(f"    {'TOTAL':8s}: {total_frame:6d} µs  ({1e6/total_frame:.1f} FPS ceiling)")
        # Print raw lines too so we can see variability
        print(f"  Last 5 PERF lines:")
        for l in perf_records[-5:]:
            print(f"    {l}")
    else:
        print("  No PERF lines received (need debug build with rdtsc patch)")

    return {"fps": fps, "perf": perf_records}

if __name__ == "__main__":
    r = run("whpx", duration=45)
    print("\n" + "="*50)
    if r and r["fps"]:
        print(f"  WHPX FPS: {r['fps']:.2f}")
    print("="*50)
