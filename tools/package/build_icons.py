"""Generate the committed application icon set from the project logo.

The source is `docs/images/tore-fighters-logo.png`, our own artwork: a round
embroidered-patch render on a transparent ground. Nothing here touches retail
media.

    python3 tools/package/build_icons.py            # regenerate everything
    python3 tools/package/build_icons.py --check    # report only, write nothing
    python3 tools/package/build_icons.py --verify-ico PATH
    python3 tools/package/build_icons.py --verify-res PATH

Outputs land in `crates/tore-app/assets/icon/`:

- `tore-16.png` through `tore-512.png`, used by the Linux packages, the macOS
  `.iconset` and the Windows `.ico`.
- `tore.ico`, the multi-resolution Windows icon.

Downscaling uses ImageMagick 7 (`magick`) with a Lanczos filter, plus a light
unsharp pass at 64 px and below so the small sizes stay legible. Only
regeneration needs ImageMagick; every build and every packaging script reads
the committed files.

Two size decisions, both to keep the committed set under the 600 KB budget in
`crates/tore-app/assets/README.md`:

- 1024 px is not committed. The source render is photographic, so a lossless
  1024 px PNG is about 2 MB on its own.
- 512 px is quantized to 255 colours. It is only ever shown as a macOS Retina
  512 pt icon, where the banding is not visible; every smaller size is full
  colour.

`tore.ico` carries 16, 32, 48 and 64 px as uncompressed 32-bit DIBs and 256 px
as a PNG, which is the layout Windows documents. The 256 px entry reuses the
bytes of the committed `tore-256.png`, so the two cannot drift apart.

`--verify-ico` and `--verify-res` parse a finished file and print its entries.
`--verify-res` exists so the Win32 resource writer in `crates/tore-app/build.rs`
can be checked against the documented `.res` layout without a Windows host.
"""

import argparse
import struct
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "docs" / "images" / "tore-fighters-logo.png"
OUTPUT = ROOT / "crates" / "tore-app" / "assets" / "icon"

# Committed PNG sizes. macOS needs 16 through 512; Linux uses 256.
PNG_SIZES = (16, 32, 48, 64, 128, 256, 512)
# Sizes small enough that a plain Lanczos downscale reads as mushy.
UNSHARP_MAX = 64
# The one size quantized to a 255-colour palette; see the module docstring.
QUANTIZED = 512
# Windows icon entries. Everything but the largest is stored as a raw DIB.
ICO_SIZES = (16, 32, 48, 64, 256)
ICO_PNG_SIZE = 256
# Total committed bytes the icon directory must stay under.
BUDGET = 600 * 1024

RT_ICON = 3
RT_GROUP_ICON = 14
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def run(args):
    """Run a command, returning its stdout as bytes."""
    result = subprocess.run(args, check=True, stdout=subprocess.PIPE)
    return result.stdout


def base_convert(size):
    """The shared ImageMagick pipeline: trim the transparent margin, downscale."""
    args = [
        "magick",
        str(SOURCE),
        "-background", "none",
        "-alpha", "on",
        "-trim", "+repage",
        "-filter", "Lanczos",
        "-resize", f"{size}x{size}",
    ]
    if size <= UNSHARP_MAX:
        args += ["-unsharp", "0x0.75+0.55+0.008"]
    return args + ["-strip", "-define", "png:compression-level=9"]


def write_png(size, destination):
    """Render one committed PNG."""
    args = base_convert(size)
    if size == QUANTIZED:
        args += ["-dither", "None", "-colors", "255", f"PNG8:{destination}"]
    else:
        args += [f"PNG32:{destination}"]
    run(args)


def raw_bgra(png):
    """Decode a PNG to top-down straight-alpha BGRA rows via ImageMagick."""
    return run(["magick", str(png), "-depth", "8", "-alpha", "on", "BGRA:-"])


def dib_entry(png, size):
    """Encode one icon image as the uncompressed DIB an .ico expects.

    The bitmap header claims twice the real height because an icon DIB holds
    the colour (XOR) bitmap followed by a 1-bit transparency (AND) mask, both
    stored bottom-up. The mask is redundant for a 32-bit icon but Windows still
    expects the bytes to be there.
    """
    pixels = raw_bgra(png)
    stride = size * 4
    if len(pixels) != stride * size:
        raise SystemExit(f"{png}: expected {stride * size} BGRA bytes, got {len(pixels)}")
    rows = [pixels[y * stride:(y + 1) * stride] for y in range(size)]

    mask_stride = ((size + 31) // 32) * 4
    mask_rows = []
    for row in rows:
        bits = bytearray(mask_stride)
        for x in range(size):
            if row[x * 4 + 3] == 0:  # Fully transparent pixels are masked out.
                bits[x // 8] |= 0x80 >> (x % 8)
        mask_rows.append(bytes(bits))

    xor = b"".join(reversed(rows))
    and_mask = b"".join(reversed(mask_rows))
    header = struct.pack(
        "<IiiHHIIiiII",
        40,          # biSize
        size,        # biWidth
        size * 2,    # biHeight, colour bitmap plus mask
        1,           # biPlanes
        32,          # biBitCount
        0,           # biCompression, BI_RGB
        len(xor) + len(and_mask),  # biSizeImage
        0, 0, 0, 0,  # pixels per metre and palette counts, unused at 32-bit
    )
    return header + xor + and_mask


def build_ico(images):
    """Assemble the .ico from {size: image bytes}."""
    count = len(images)
    directory = bytearray(struct.pack("<HHH", 0, 1, count))
    offset = 6 + 16 * count
    body = bytearray()
    for size in sorted(images):
        data = images[size]
        directory += struct.pack(
            "<BBBBHHII",
            size % 256,  # 256 is stored as 0
            size % 256,
            0,   # no palette
            0,   # reserved
            1,   # colour planes
            32,  # bits per pixel
            len(data),
            offset,
        )
        body += data
        offset += len(data)
    return bytes(directory) + bytes(body)


def describe_ico(path):
    """Print one line per .ico entry. Returns 0 when the file parses."""
    data = path.read_bytes()
    reserved, kind, count = struct.unpack_from("<HHH", data, 0)
    if reserved != 0 or kind != 1:
        print(f"{path}: not an icon file", file=sys.stderr)
        return 1
    print(f"{path}: {count} entries, {len(data)} bytes")
    for index in range(count):
        width, height, colours, _, planes, bits, size, offset = struct.unpack_from(
            "<BBBBHHII", data, 6 + 16 * index
        )
        encoding = "PNG" if data[offset:offset + 8] == PNG_MAGIC else "DIB"
        print(
            f"  {width or 256:4} x {height or 256:<4} {encoding} "
            f"{bits} bpp, {planes} plane, {colours} palette entries, {size} bytes"
        )
    return 0


def describe_res(path):
    """Print one line per resource in a Win32 .res file. Returns 0 on success."""
    data = path.read_bytes()
    names = {RT_ICON: "RT_ICON", RT_GROUP_ICON: "RT_GROUP_ICON"}
    offset = 0
    index = 0
    while offset < len(data):
        if offset + 8 > len(data):
            print(f"{path}: truncated header at {offset}", file=sys.stderr)
            return 1
        data_size, header_size = struct.unpack_from("<II", data, offset)
        if header_size < 32 or offset + header_size > len(data):
            print(f"{path}: bad header size {header_size} at {offset}", file=sys.stderr)
            return 1
        type_flag, type_id, name_flag, name_id = struct.unpack_from("<HHHH", data, offset + 8)
        version, memory_flags, language, _, _ = struct.unpack_from("<IHHII", data, offset + 16)
        if type_flag != 0xFFFF or name_flag != 0xFFFF:
            print(f"{path}: entry {index} does not use ordinal type and name", file=sys.stderr)
            return 1
        body = offset + header_size
        end = body + data_size
        if end > len(data):
            print(f"{path}: entry {index} data runs past the end", file=sys.stderr)
            return 1
        payload = data[body:end]
        if index == 0:
            if data_size or type_id or name_id:
                print(f"{path}: missing the null resource header", file=sys.stderr)
                return 1
            print(f"{path}: {len(data)} bytes, null header {header_size} bytes")
        else:
            label = names.get(type_id, f"type {type_id}")
            note = ""
            if type_id == RT_ICON:
                note = ", PNG" if payload[:8] == PNG_MAGIC else ", DIB"
            elif type_id == RT_GROUP_ICON and len(payload) >= 6:
                entries = struct.unpack_from("<H", payload, 4)[0]
                note = f", {entries} icons"
                expected = 6 + 14 * entries
                if len(payload) != expected:
                    print(
                        f"{path}: group icon is {len(payload)} bytes, expected {expected}",
                        file=sys.stderr,
                    )
                    return 1
            print(
                f"  {label} name {name_id}: {data_size} bytes, header {header_size}, "
                f"language 0x{language:04X}, memory flags 0x{memory_flags:04X}, "
                f"version {version}{note}"
            )
        padded = (data_size + 3) & ~3
        if end + (padded - data_size) > len(data):
            print(f"{path}: entry {index} padding runs past the end", file=sys.stderr)
            return 1
        offset = end + (padded - data_size)
        index += 1
    if index < 2:
        print(f"{path}: no resources after the null header", file=sys.stderr)
        return 1
    return 0


def report(paths):
    """Print the committed sizes and enforce the budget."""
    total = 0
    for path in paths:
        size = path.stat().st_size
        total += size
        print(f"  {path.relative_to(ROOT)}  {size:>7} bytes")
    print(f"  total {total} bytes of a {BUDGET} byte budget")
    if total > BUDGET:
        print("Icon set is over budget. Drop a size or quantize another.", file=sys.stderr)
        return 1
    return 0


def build(check_only):
    if not SOURCE.is_file():
        print(f"Missing the source artwork: {SOURCE}", file=sys.stderr)
        return 1
    if check_only:
        existing = sorted(OUTPUT.glob("tore-*.png")) + [OUTPUT / "tore.ico"]
        missing = [p for p in existing if not p.is_file()]
        if missing:
            print("Missing: " + ", ".join(str(p) for p in missing), file=sys.stderr)
            return 1
        return report(existing) or describe_ico(OUTPUT / "tore.ico")

    OUTPUT.mkdir(parents=True, exist_ok=True)
    written = []
    for size in PNG_SIZES:
        destination = OUTPUT / f"tore-{size}.png"
        write_png(size, destination)
        written.append(destination)

    images = {}
    for size in ICO_SIZES:
        png = OUTPUT / f"tore-{size}.png"
        if size == ICO_PNG_SIZE:
            images[size] = png.read_bytes()
        else:
            images[size] = dib_entry(png, size)
    ico = OUTPUT / "tore.ico"
    ico.write_bytes(build_ico(images))
    written.append(ico)

    print("Wrote:")
    status = report(written)
    return status or describe_ico(ico)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true",
                        help="Report the committed sizes without regenerating.")
    parser.add_argument("--verify-ico", type=Path, metavar="PATH",
                        help="Parse an .ico and print its entries.")
    parser.add_argument("--verify-res", type=Path, metavar="PATH",
                        help="Parse a Win32 .res file and print its entries.")
    args = parser.parse_args()
    if args.verify_ico:
        return describe_ico(args.verify_ico)
    if args.verify_res:
        return describe_res(args.verify_res)
    return build(args.check)


if __name__ == "__main__":
    sys.exit(main())
