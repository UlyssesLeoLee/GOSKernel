#!/usr/bin/env python3
"""Empirical verification of the B3b viewer<->kernel input round trip
(2026-06-11 session).

gos-vk-viewer connects to the kernel's @gos.vk visual bridge on COM3
(TCP:14445). That link is full-duplex: the kernel streams @gos.vk display-list
frames out, and the viewer forwards host keystrokes back over the SAME socket.
hypervisor::kernel_main drains those bytes via k_vk_host::vk_drain_input(),
echoes each one to COM1 (`vk-input: 0xNN`), pushes it into k_ps2's key queue,
AND posts a Signal::Data to k-shell's NODE_VEC so it drives the graph CLI like
a physical keyboard.

This script does not need the GUI viewer: it connects to TCP:14445 itself
(exactly what gos-vk-viewer's background thread does) and:

  1. Sends the ASCII bytes for "theme wabi\r" then "theme shoji\r" - the two
     k-shell commands that rebind the `theme.current -[Use]-> theme.*` edge
     (crates/k-shell/src/lib.rs: apply_theme_choice -> sync_theme_use_edges).
     Whatever the active theme is at boot, switching to wabi then shoji
     guarantees the SECOND switch is a real Use-edge rebind, which
     rebind_exclusive_use turns into a graph_epoch bump (ADR-004 SS2.2).
  2. Confirms every byte was echoed on COM1 as `vk-input: 0xNN` in order -
     proves the COM3-RX -> vk_drain_input -> k_ps2::push_key path.
  3. Confirms a fresh @gos.vk frame (VKEND:) arrives on COM3 noticeably
     faster after the "theme shoji\r" Enter than the steady-state keepalive
     cadence (~1s, VK_KEEPALIVE_TICKS) - proves the posted Signal::Data
     reached k-shell, dispatch_text_command ran the theme switch, and the
     resulting graph_epoch bump triggered vk_auto_refresh's fast path
     (~0.25s, VK_REFRESH_TICKS).

Usage: python tools/verify_b3b_input_roundtrip.py [com1_log_path]
Expects the kernel already running (`cargo run -p gos-kernel --release`)
with COM1 (stdio) redirected to com1_log_path (default vk_test_com1.log).
"""
import socket
import sys
import time

COM3_ADDR = ("127.0.0.1", 14445)
COM1_LOG = sys.argv[1] if len(sys.argv) > 1 else "vk_test_com1.log"

CMD1 = b"theme wabi\r"
CMD2 = b"theme shoji\r"


def read_frame_ends(sock, duration, leftover):
    """Read from sock for `duration` seconds. Return (timestamps of each
    VKEND:\\n seen, updated leftover bytes)."""
    deadline = time.monotonic() + duration
    ends = []
    buf = leftover
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        sock.settimeout(min(0.05, remaining))
        try:
            chunk = sock.recv(4096)
        except socket.timeout:
            continue
        except OSError:
            break
        if not chunk:
            break
        buf += chunk
        while b"VKEND:\n" in buf:
            idx = buf.index(b"VKEND:\n") + len(b"VKEND:\n")
            ends.append(time.monotonic())
            buf = buf[idx:]
    return ends, buf


def main():
    print(f"[verify] connecting to {COM3_ADDR[0]}:{COM3_ADDR[1]} ...")
    sock = socket.create_connection(COM3_ADDR, timeout=5)
    print("[verify] connected; draining startup backlog (0.3s)")
    _, leftover = read_frame_ends(sock, 0.3, b"")

    print("[verify] phase 1: baseline frame cadence (2.5s)")
    baseline_ends, leftover = read_frame_ends(sock, 2.5, leftover)
    if len(baseline_ends) >= 2:
        intervals = [b - a for a, b in zip(baseline_ends, baseline_ends[1:])]
        baseline_interval = sum(intervals) / len(intervals)
    else:
        baseline_interval = 1.0  # VK_KEEPALIVE_TICKS default assumption
    print(f"[verify]   frames seen={len(baseline_ends)} baseline_interval~={baseline_interval:.3f}s")

    WINDOW = 4.0
    FAST = 0.5

    print(f"[verify] phase 2: send {CMD1!r}")
    t1 = time.monotonic()
    sock.sendall(CMD1)
    ends1, leftover = read_frame_ends(sock, WINDOW, leftover)
    deltas1 = [e - t1 for e in ends1]
    fast1 = [d for d in deltas1 if d < FAST]
    print(f"[verify]   frames within {WINDOW}s={len(ends1)} fast(<{FAST}s)={len(fast1)} deltas={['%.3f'%d for d in deltas1]}")

    time.sleep(0.5)

    print(f"[verify] phase 3: send {CMD2!r} (guaranteed Use-edge rebind)")
    t2 = time.monotonic()
    sock.sendall(CMD2)
    ends2, leftover = read_frame_ends(sock, WINDOW, leftover)
    deltas2 = [e - t2 for e in ends2]
    fast2 = [d for d in deltas2 if d < FAST]
    print(f"[verify]   frames within {WINDOW}s={len(ends2)} fast(<{FAST}s)={len(fast2)} deltas={['%.3f'%d for d in deltas2]}")

    sock.close()

    # --- Check COM1 for the vk-input echo of every byte we sent, in order ---
    print(f"[verify] checking {COM1_LOG} for vk-input echoes")
    with open(COM1_LOG, "r", encoding="utf-8", errors="replace") as f:
        com1_text = f.read()

    expected = list(CMD1 + CMD2)
    echoed = [int(line.split("vk-input:")[1].strip(), 16)
              for line in com1_text.splitlines() if "vk-input:" in line]

    # The expected sequence must appear, in order, as a subsequence of what
    # was echoed (there may be a few stray bytes before/after from the host).
    pos = 0
    for b in echoed:
        if pos < len(expected) and b == expected[pos]:
            pos += 1
    echo_ok = pos == len(expected)

    print(f"[verify]   echoed {len(echoed)} bytes total; matched {pos}/{len(expected)} expected in order")
    if not echo_ok:
        print(f"[verify]   expected hex: {[hex(b) for b in expected]}")
        print(f"[verify]   echoed   hex: {[hex(b) for b in echoed]}")

    epoch_ok = len(fast2) >= 1
    epoch_line = f"theme-switch mutation triggered fast @gos.vk frame : {'PASS' if epoch_ok else 'FAIL'}"
    if epoch_ok:
        epoch_line += f"  (frame {fast2[0]:.3f}s after Enter, baseline~={baseline_interval:.3f}s)"
    print()
    print("=" * 60)
    print(f"COM3->kernel input echoed on COM1 (vk-input: 0xNN) : {'PASS' if echo_ok else 'FAIL'}")
    print(epoch_line)
    print("=" * 60)
    overall = echo_ok and epoch_ok
    print("OVERALL:", "PASS" if overall else "FAIL")
    sys.exit(0 if overall else 1)


if __name__ == "__main__":
    main()
