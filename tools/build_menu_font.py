"""Regenerate the open-licensed menu font atlases with ImageMagick, not retail data.

Two atlases are produced, both in the same simple format: 256 one-byte glyph
advances, then a row-major 8-bit alpha plane of `256 * CELL_W` by `CELL_H`
pixels. Glyph `code` occupies the column range `code * CELL_W` for `CELL_W`
pixels on every row; its byte in the advance table says how far the pen moves
after drawing it. ASCII 32 through 126 is populated and everything else is
blank with a zero advance. There is no colour in the file: the application
supplies a flat text colour and uses the stored alpha for coverage.

- `menu-font.bin`, 16 by 11 cells rendered at 10 px from Noto Sans Bold. This
  is the original atlas, kept unchanged for the Quick Mission and ordnance
  labels that are laid out around it.
- `menu-font-large.bin`, 24 by 17 cells rendered at 14 px from Noto Sans
  Medium. Antialiased and light enough to stay readable when the 640x480
  canvas is scaled up to a 1080p or fullscreen window. The locate screen and
  the main-menu version label use it.

Usage:

    python3 tools/build_menu_font.py /path/to/NotoSans-Bold.ttf \
        --large /path/to/NotoSans-Medium.ttf

Either source may be omitted; only the named atlases are rewritten.
"""
import argparse
from pathlib import Path
import re
import subprocess

ASSETS = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets'
# (file name, cell width, cell height, point size, baseline, top crop). The
# glyph is drawn on a square canvas at `baseline` and the cell is cut out
# `crop` rows down from the top, which trims the empty space above the
# ascender without clipping any ASCII glyph.
SMALL = ('menu-font.bin', 16, 11, 10, 11, 3)
LARGE = ('menu-font-large.bin', 24, 17, 14, 18, 5)


def build(font, spec):
    name, cell_w, cell_h, points, baseline, crop = spec
    canvas = max(cell_w, baseline + cell_h)
    advances = bytearray(256)
    stride = 256 * cell_w
    pixels = bytearray(stride * cell_h)
    advances[32] = max(3, round(points * 0.28))
    for code in range(33, 127):
        char = chr(code)
        if char == chr(92):
            char *= 2
        cmd = ['magick', '-debug', 'annotate', '-size', f'{canvas}x{canvas}', 'xc:black',
               '-font', str(font), '-pointsize', str(points), '-fill', 'white',
               '-annotate', f'+0+{baseline}', char,
               '-crop', f'{cell_w}x{cell_h}+0+{crop}', '+repage',
               '-depth', '8', 'gray:-']
        result = subprocess.run(cmd, check=True, capture_output=True)
        widths = re.findall(rb'Metrics:.*?width: ([0-9.]+)', result.stderr)
        if not widths or len(result.stdout) != cell_w * cell_h:
            raise RuntimeError(f'Unexpected font output for {char!r}')
        advances[code] = min(cell_w, round(float(widths[-1])))
        for y in range(cell_h):
            start = y * stride + code * cell_w
            pixels[start:start + cell_w] = result.stdout[y * cell_w:(y + 1) * cell_w]
    (ASSETS / name).write_bytes(bytes(advances) + pixels)
    print(f'{name}: {cell_w}x{cell_h} cells, {256 + stride * cell_h} bytes')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path, nargs='?', help='NotoSans-Bold.ttf for menu-font.bin')
    parser.add_argument('--large', type=Path,
                        help='NotoSans-Medium.ttf for menu-font-large.bin')
    args = parser.parse_args()
    if not args.font and not args.large:
        parser.error('give a source font, --large, or both')
    if args.font:
        build(args.font, SMALL)
    if args.large:
        build(args.large, LARGE)


if __name__ == '__main__':
    main()
