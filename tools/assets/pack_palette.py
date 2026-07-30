#!/usr/bin/env python3
"""
GOS asset palette packer.  PPM (binary RGB) → 1-byte-per-pixel palette
index .pal binary using the same 256-color palette the kernel's
k-fb crate programs into the VGA DAC.

Usage:
    python pack_palette.py <input.ppm> [<input.ppm> ...]
    python pack_palette.py --all          # all PPMs under assets/

Output (next to input):
    <stem>.pal   raw bytes, width*height entries

The .pal binary's leading 4 bytes are a magic + width + height header
so `crates/k-assets/build.rs` can verify dimensions at compile time:

    bytes 0..2   magic 'GA' (0x47 0x41)
    bytes 2..3   width  (u8 — kept tiny since textures ≤ 128)
    bytes 3..4   height (u8)
    bytes 4..    palette indices, row-major

For textures larger than 255 in either dimension, switch to a u16
header (extend magic to 'GA2' etc.) — not needed for the current set.
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path
from typing import List, Tuple

# Mirror crates/k-fb/src/lib.rs PALETTE + HUE_PEAKS exactly.
# Channel values are 6-bit (0..63), converted to 8-bit (0..255) via
# `v*4 + v/16` matching k-fb's `conv6` function.

NAMED_PALETTE = [
    (2, 4, 10),    # 0  Background
    (4, 18, 28),   # 1  HeaderBar
    (55, 58, 60),  # 2  Foreground
    (8, 40, 56),   # 3  NodeKernel
    (16, 50, 32),  # 4  NodeService
    (52, 44, 8),   # 5  NodeDriver
    (48, 12, 40),  # 6  NodeApp
    (48, 24, 32),  # 7  NodeOther
    (8, 12, 18),   # 8  BarEmpty
    (58, 58, 16),  # 9  Highlight
    (60, 8, 12),   # 10 Error
    (30, 40, 46),  # 11 DimWhite
]

# Hue ramp peaks (Lambertian ramps).  Each ramp spans 8 shade slots,
# starting at slot 16 + hue_idx * 8.
HUE_PEAKS = [
    (16, 48, 60),  # cyan   — hardware / kernel
    (60, 16, 50),  # magenta — driver
    (60, 50, 12),  # yellow — service
    (20, 58, 42),  # mint   — app / plugin entry
    (58, 28, 38),  # rose   — other / generic
]


def conv6(v: int) -> int:
    """6-bit → 8-bit channel expansion, matches k-fb conv6."""
    return min(255, v * 4 + v // 16)


def build_palette_rgb() -> List[Tuple[int, int, int]]:
    """Build the full 256-entry 8-bit RGB palette matching the kernel."""
    out = [(0, 0, 0)] * 256
    for i, (r, g, b) in enumerate(NAMED_PALETTE):
        out[i] = (conv6(r), conv6(g), conv6(b))
    for hue_idx, peak in enumerate(HUE_PEAKS):
        base = 16 + hue_idx * 8
        for shade in range(8):
            scale = min(8, 1 + shade)
            r = min(63, (peak[0] * scale) // 8)
            g = min(63, (peak[1] * scale) // 8)
            b = min(63, (peak[2] * scale) // 8)
            out[base + shade] = (conv6(r), conv6(g), conv6(b))
    return out


PALETTE = build_palette_rgb()


def closest_palette_index(r: int, g: int, b: int) -> int:
    """Find palette slot with minimum Euclidean distance in 8-bit RGB."""
    best_d = 1 << 30
    best_i = 0
    for i, (pr, pg, pb) in enumerate(PALETTE):
        dr, dg, db = r - pr, g - pg, b - pb
        d = dr * dr + dg * dg + db * db
        if d < best_d:
            best_d = d
            best_i = i
    return best_i


def parse_ppm(path: Path) -> Tuple[int, int, bytes]:
    """Parse a binary P6 PPM into (width, height, raw_rgb_bytes)."""
    with path.open('rb') as f:
        magic = f.readline().strip()
        if magic != b'P6':
            raise ValueError(f"{path}: expected P6, got {magic!r}")
        # Skip comment lines, then read width height, then maxval
        def next_token() -> bytes:
            buf = b''
            while True:
                c = f.read(1)
                if not c:
                    raise ValueError(f"{path}: unexpected EOF in header")
                if c == b'#':
                    f.readline()
                    continue
                if c.isspace():
                    if buf:
                        return buf
                    continue
                buf += c
        w = int(next_token())
        h = int(next_token())
        m = int(next_token())
        if m != 255:
            raise ValueError(f"{path}: expected maxval 255, got {m}")
        return w, h, f.read(w * h * 3)


def pack_ppm(src: Path) -> Path:
    w, h, raw = parse_ppm(src)
    if w > 255 or h > 255:
        raise ValueError(f"{src}: dims {w}×{h} too big for u8 header")
    out_path = src.with_suffix('.pal')
    out = bytearray()
    out += b'GA'
    out += bytes([w, h])
    for i in range(0, len(raw), 3):
        r, g, b = raw[i], raw[i + 1], raw[i + 2]
        out.append(closest_palette_index(r, g, b))
    out_path.write_bytes(out)
    print(f"  {src.name} ({w}×{h}) → {out_path.name} ({len(out)} bytes)")
    return out_path


def workspace_root() -> Path:
    cur = Path(__file__).resolve()
    for _ in range(5):
        cur = cur.parent
        if (cur / 'Cargo.lock').is_file() and (cur / 'crates').is_dir():
            return cur
    raise SystemExit("could not locate workspace root")


def main(argv):
    if not argv:
        print(__doc__, file=sys.stderr)
        return 1
    root = workspace_root()
    if argv[0] == '--all':
        targets = sorted((root / 'assets').glob('*.ppm'))
    else:
        targets = [Path(p) for p in argv]
    if not targets:
        print("no PPM files found", file=sys.stderr)
        return 1
    print(f"packing {len(targets)} file(s) against k-fb palette:")
    for t in targets:
        pack_ppm(t)
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
