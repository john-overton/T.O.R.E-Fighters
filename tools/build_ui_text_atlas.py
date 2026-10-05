"""Regenerate the sharp UI text atlas with ImageMagick, not retail data.

`crates/tore-app/assets/ui-text.bin` holds Noto Sans Medium rendered large
(48 px to the em), so the multiplayer screens can draw their text at the
window's own resolution on top of the 640 by 480 menu canvas instead of
stretching the retail bitmap fonts with it. The game lays each glyph out in the
retail font's own cell, at the retail font's cap height, so text keeps its size
and place; only the glyph shapes are sharper. See `docs/formats/menu.md`
("Sharp text") and `crates/tore-app/assets/README.md`.

File layout, all little endian:

    b"TUA1"
    u16 cell_w, cell_h, columns, em, cap_height, origin_x, baseline, count
    count * 12 bytes: u8 code, u8 0, u16 advance (in 1/16 px), i16 ink_x
        (from the pen origin), i16 ink_y (from the baseline, up is negative),
        u16 ink_w, u16 ink_h
    the 8-bit alpha plane, `columns * cell_w` across and
        `ceil(count / columns) * cell_h` down; the glyph at index `i` in the
        records is in column `i % columns`, row `i // columns`

Everything is in atlas pixels. A glyph's origin is `origin_x` pixels into its
cell and its baseline `baseline` pixels down, so a cell holds the ink with at
least `PADDING` pixels clear on every side, enough for three levels of mipmaps.
ASCII 33 through 126 and the CP437 letters above 0x7F that
`tore_formats::text` maps are included; space has no ink and uses the retail
advance.

Usage:

    python3 tools/build_ui_text_atlas.py /path/to/NotoSans-Medium.ttf
"""
import argparse
from pathlib import Path
import re
import struct
import subprocess

ASSETS = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets'
EM = 48
CELL_W, CELL_H = 72, 76
ORIGIN_X, BASELINE = 10, 58
COLUMNS = 12
PADDING = 4

GLYPH_CODES = list(range(33, 127)) + [c for c in range(0x80, 0xA9) if c != 0x9E] + [0xAD, 0xE1, 0xE6, 0xF8]


def render(font, char):
    """The glyph on a CELL_W by CELL_H plane, and its advance in pixels."""
    if char == chr(92):
        char *= 2
    cmd = ['magick', '-debug', 'annotate', '-size', f'{CELL_W}x{CELL_H}', 'xc:black',
           '-font', str(font), '-pointsize', str(EM), '-fill', 'white',
           '-annotate', f'+{ORIGIN_X}+{BASELINE}', char, '-depth', '8', 'gray:-']
    result = subprocess.run(cmd, check=True, capture_output=True)
    widths = re.findall(rb'Metrics:.*?width: ([0-9.]+)', result.stderr)
    if not widths or len(result.stdout) != CELL_W * CELL_H:
        raise RuntimeError(f'Unexpected font output for {char!r}')
    return result.stdout, float(widths[-1])


def ink_box(plane):
    """(left, top, right, bottom) of the nonzero pixels, bottom and right exclusive."""
    left, top, right, bottom = CELL_W, CELL_H, 0, 0
    for y in range(CELL_H):
        row = plane[y * CELL_W:(y + 1) * CELL_W]
        if any(row):
            top = min(top, y)
            bottom = max(bottom, y + 1)
            left = min(left, next(x for x, v in enumerate(row) if v))
            right = max(right, CELL_W - next(x for x, v in enumerate(reversed(row)) if v))
    return left, top, right, bottom


def build(font):
    records = []
    planes = []
    cap_height = 0
    for code in GLYPH_CODES:
        char = bytes([code]).decode('cp437')
        plane, advance = render(font, char)
        left, top, right, bottom = ink_box(plane)
        if right <= left:
            continue
        for name, margin in (('left', left), ('top', top),
                             ('right', CELL_W - right), ('bottom', CELL_H - bottom)):
            if margin < PADDING:
                raise RuntimeError(f'{char!r} is {margin} px from the cell edge ({name})')
        if char == 'H':
            cap_height = BASELINE - top
        records.append((code, round(advance * 16), left - ORIGIN_X, top - BASELINE,
                        right - left, bottom - top))
        planes.append(plane)
    if not cap_height:
        raise RuntimeError('no H in the glyph set')
    rows = -(-len(records) // COLUMNS)
    width, height = COLUMNS * CELL_W, rows * CELL_H
    atlas = bytearray(width * height)
    for index, plane in enumerate(planes):
        x0, y0 = (index % COLUMNS) * CELL_W, (index // COLUMNS) * CELL_H
        for y in range(CELL_H):
            start = (y0 + y) * width + x0
            atlas[start:start + CELL_W] = plane[y * CELL_W:(y + 1) * CELL_W]
    out = bytearray(b'TUA1')
    out += struct.pack('<8H', CELL_W, CELL_H, COLUMNS, EM, cap_height, ORIGIN_X, BASELINE,
                       len(records))
    for code, advance, ink_x, ink_y, ink_w, ink_h in records:
        out += struct.pack('<BBHhhHH', code, 0, advance, ink_x, ink_y, ink_w, ink_h)
    out += atlas
    (ASSETS / 'ui-text.bin').write_bytes(bytes(out))
    print(f'ui-text.bin: {len(records)} glyphs, {width}x{height} plane, cap height {cap_height}, '
          f'{len(out)} bytes')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path, help='NotoSans-Medium.ttf')
    build(parser.parse_args().font)


if __name__ == '__main__':
    main()
