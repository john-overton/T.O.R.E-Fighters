"""Generate the application icon. Uses only the Python standard library.

The mark is a plain geometric delta on a dark ground: an original drawing, not
retail art, so it is safe to commit. Run this script to regenerate
`tools/package/tore.png` after changing the geometry below.

    python3 tools/package/make_icon.py
    python3 tools/package/make_icon.py --size 512 out.png

The geometry is scale free, so the macOS script asks for each `.iconset` size
directly and hands the folder to `iconutil`; nothing is resampled. The MSI uses
the default Windows Installer icon, because producing a multi-resolution `.ico`
without an image library is more machinery than the result is worth.
"""

import argparse
from pathlib import Path
import struct
import sys
import zlib


# Dark ground, mid panel, light mark. Opaque everywhere so the icon reads the
# same on a light or a dark desktop.
GROUND = (14, 20, 30)
RING = (52, 74, 104)
MARK = (226, 232, 240)


def inside_triangle(x, y, a, b, c):
    """True when the point lies inside the triangle a, b, c."""
    def side(p, q):
        return (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0])

    signs = (side(a, b), side(b, c), side(c, a))
    return all(s >= 0 for s in signs) or all(s <= 0 for s in signs)


def pixel(x, y, size):
    """Colour one pixel of the mark: a notched delta inside a ring."""
    centre = size / 2.0
    dx, dy = x + 0.5 - centre, y + 0.5 - centre
    radius = (dx * dx + dy * dy) ** 0.5
    if radius > size * 0.47:
        return GROUND
    nose = (centre, size * 0.14)
    left = (size * 0.16, size * 0.84)
    right = (size * 0.84, size * 0.84)
    notch_apex = (centre, size * 0.56)
    if inside_triangle(x + 0.5, y + 0.5, nose, left, right):
        if inside_triangle(x + 0.5, y + 0.5, notch_apex, left, right):
            return RING
        return MARK
    if radius > size * 0.40:
        return RING
    return GROUND


def render(size):
    """Build the raw scanlines, each prefixed by the PNG filter byte 0."""
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            rows.extend(pixel(x, y, size))
    return bytes(rows)


def chunk(kind, payload):
    body = kind + payload
    return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))


def encode(size, raw):
    header = struct.pack(">2I5B", size, size, 8, 2, 0, 0, 0)  # 8-bit truecolour
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def main():
    default = Path(__file__).resolve().parent / "tore.png"
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", nargs="?", type=Path, default=default)
    parser.add_argument("--size", type=int, default=256, help="Square edge in pixels.")
    args = parser.parse_args()
    if not 16 <= args.size <= 1024:
        parser.error("Size must be between 16 and 1024.")
    data = encode(args.size, render(args.size))
    # The committed 256 px icon has a budget; larger ones are build products.
    if args.size <= 256 and len(data) > 20 * 1024:
        print(f"Icon is {len(data)} bytes, over the 20 KB budget.", file=sys.stderr)
        return 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    print(f"Wrote {args.output} ({len(data)} bytes).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
