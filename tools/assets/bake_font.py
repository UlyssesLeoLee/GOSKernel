"""bake_font.py — TTF → kernel-embeddable anti-aliased glyph atlas.

The kernel cannot afford a runtime TTF rasterizer (fontdue + alloc +
hinting tables); instead we pre-render each glyph offline at the
target pixel size and ship the resulting 8-bit-alpha bitmap as part
of the asset bundle.  The kernel side reads the atlas, indexes by
codepoint, and alpha-blends the glyph into the back-buffer at draw
time — a single look-up + lerp per pixel, no parsing.

Binary format ('FA' = Font Atlas):
    bytes  0..2     magic 'FA' (0x46 0x41)
    byte   2        cell_w        (u8)        cell width  in pixels
    byte   3        cell_h        (u8)        cell height in pixels
    byte   4        char_first    (u8)        first codepoint (e.g. 32)
    byte   5        char_count    (u8)        glyph count    (e.g. 95)
    bytes  6..8     atlas_w       (u16 LE)   cells_per_row × cell_w
    bytes  8..      alpha[atlas_w × atlas_h] each u8 in 0..=255

Row-major linear bitmap.  cells_per_row is derived as
`atlas_w / cell_w`; row of glyph `c` is `(c - char_first) / cells_per_row`
and column is `(c - char_first) % cells_per_row`.

Usage:
    python tools/assets/bake_font.py \\
        --font  C:/Windows/Fonts/NotoSansSC-VF.ttf \\
        --size  14 \\
        --out   assets/font_ui_14.fnt
"""

from __future__ import annotations
import argparse
import struct
import sys
from pathlib import Path


def bake(font_path: str, size_px: int, out_path: str,
         char_first: int = 32, char_count: int = 95,
         cells_per_row: int = 16) -> None:
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError:
        sys.exit("ERROR: pip install pillow")

    font = ImageFont.truetype(font_path, size_px)

    # Measure widest + tallest box to pick a uniform cell.  We render
    # variable-width glyphs into a fixed cell padded with transparent
    # space; the kernel reads cell-by-cell so it doesn't need width tables.
    max_w, max_h = 0, 0
    metrics_ascent = 0
    try:
        metrics_ascent, _descent = font.getmetrics()
    except AttributeError:
        metrics_ascent = size_px
    for cp in range(char_first, char_first + char_count):
        bbox = font.getbbox(chr(cp))
        # bbox = (left, top, right, bottom) — bottom can be > size_px
        # for glyphs with descender (g, p, y).
        w = max(bbox[2] - bbox[0], int(size_px * 0.45))
        h = bbox[3]
        max_w = max(max_w, w)
        max_h = max(max_h, h)

    # Uniform cell with 1-pixel side padding so anti-aliased halos
    # don't bleed into neighbouring glyphs in the atlas.
    cell_w = max(max_w + 2, int(size_px * 0.6))
    cell_h = max(max_h + 2, int(size_px * 1.25))
    if cell_w > 255 or cell_h > 255:
        sys.exit(f"ERROR: cell {cell_w}×{cell_h} > 255 px — pick smaller size")

    rows = (char_count + cells_per_row - 1) // cells_per_row
    atlas_w = cell_w * cells_per_row
    atlas_h = cell_h * rows
    if atlas_w > 0xFFFF:
        sys.exit(f"ERROR: atlas width {atlas_w} > 65535 — reduce cells_per_row")

    img = Image.new('L', (atlas_w, atlas_h), 0)
    draw = ImageDraw.Draw(img)

    for i, cp in enumerate(range(char_first, char_first + char_count)):
        col = i % cells_per_row
        row = i // cells_per_row
        # Centre the glyph horizontally; align baseline to the bottom
        # of the cell with a 2-px margin so descenders fit.
        bbox = font.getbbox(chr(cp))
        glyph_w = bbox[2] - bbox[0]
        cell_origin_x = col * cell_w
        cell_origin_y = row * cell_h
        # Anchor at baseline (PIL's `anchor="ls"` = left/baseline)
        draw_x = cell_origin_x + max(0, (cell_w - glyph_w) // 2 - bbox[0])
        draw_y = cell_origin_y + metrics_ascent
        draw.text((draw_x, draw_y - bbox[3] + max_h),
                  chr(cp), font=font, fill=255, anchor="ls")

    with open(out_path, 'wb') as f:
        f.write(b'FA')
        f.write(struct.pack('<BBBBH', cell_w, cell_h, char_first, char_count, atlas_w))
        f.write(img.tobytes())

    sz = Path(out_path).stat().st_size
    print(f'{out_path}: cell {cell_w}×{cell_h}, atlas {atlas_w}×{atlas_h}, '
          f'{char_count} glyphs, {sz} bytes')


def main():
    ap = argparse.ArgumentParser(description=__doc__.split('\n', 1)[0])
    ap.add_argument('--font', required=True,
                    help='TTF/OTF path (e.g. C:/Windows/Fonts/NotoSansSC-VF.ttf)')
    ap.add_argument('--size', type=int, default=14, help='pixel size')
    ap.add_argument('--out', required=True, help='output .fnt path')
    ap.add_argument('--first', type=int, default=32,
                    help='first codepoint (default 32 = space)')
    ap.add_argument('--count', type=int, default=95,
                    help='glyph count (default 95 = ASCII printable)')
    args = ap.parse_args()
    bake(args.font, args.size, args.out, args.first, args.count)


if __name__ == '__main__':
    main()
