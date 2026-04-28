"""QEMU monitor probe — sends commands and captures frames in one TCP session."""
import socket, sys, time, os

HOST, PORT = "127.0.0.1", 55556

def open_mon():
    s = socket.create_connection((HOST, PORT), timeout=4)
    s.settimeout(0.5)
    # Drain banner.
    time.sleep(0.4)
    try:
        while s.recv(8192):
            pass
    except socket.timeout:
        pass
    return s

def cmd(s, line, settle=0.4):
    s.sendall((line + "\n").encode())
    time.sleep(settle)
    out = b""
    try:
        while True:
            buf = s.recv(8192)
            if not buf:
                break
            out += buf
    except socket.timeout:
        pass
    return out.decode("utf-8", errors="replace")

def main():
    s = open_mon()

    # 1. Snapshot of IRQ counters before key input.
    pre_irq = cmd(s, "info irq", 0.8)
    print("--- pre-input IRQ ---")
    for line in pre_irq.splitlines():
        if line.strip() and "(qemu)" not in line and "info" not in line.lower():
            print("  " + line.strip())

    # 2. Take baseline frame.
    cmd(s, "screendump E:/GOSKernel/test-frames/frame-pre.ppm", 1.5)

    # 3. Send keys: w h e r e Enter
    for k in ("w", "h", "e", "r", "e", "ret"):
        cmd(s, f"sendkey {k}", 0.20)
    time.sleep(0.7)

    # 4. Capture frame after.
    cmd(s, "screendump E:/GOSKernel/test-frames/frame-post.ppm", 1.5)

    # 5. Final IRQ snapshot.
    post_irq = cmd(s, "info irq", 0.8)
    print("\n--- post-input IRQ ---")
    for line in post_irq.splitlines():
        if line.strip() and "(qemu)" not in line and "info" not in line.lower():
            print("  " + line.strip())

    s.close()

    # Diff frames.
    pre = "E:/GOSKernel/test-frames/frame-pre.ppm"
    post = "E:/GOSKernel/test-frames/frame-post.ppm"
    if os.path.exists(pre) and os.path.exists(post):
        with open(pre, "rb") as fp1, open(post, "rb") as fp2:
            a, b = fp1.read(), fp2.read()
        identical = a == b
        print(f"\n--- frame diff ---")
        print(f"  pre:  {len(a)} bytes")
        print(f"  post: {len(b)} bytes")
        print(f"  identical: {identical}")

if __name__ == "__main__":
    main()
