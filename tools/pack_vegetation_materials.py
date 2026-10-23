"""Pack original text-generated side/top pairs into material/shade R8 pages.

Usage: python3 tools/pack_vegetation_materials.py GENERATED OUTPUT
Each source PNG has side on the left and overhead on the right. Alpha is the
only cutout key. Warm chroma identifies wood; neutral pixels identify the
named plant's foliage, straw or stone. Requires Pillow only when authoring.
"""
import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image

CELL = 128
EXTENT = 124
KINDS = ('broadleaf', 'dead', 'conifer', 'palm', 'acacia', 'birch',
         'jungle_tree', 'green_bush', 'dry_bush', 'fern', 'banana', 'yucca',
         'dry_scrub', 'stone', 'boulder', 'outcrop', 'desert_rock', 'tropical_shrub')


def material(kind, rgb):
    r, g, b = rgb
    if kind in ('stone', 'boulder', 'outcrop', 'desert_rock'):
        return 2
    if kind == 'dry_scrub':
        return 3
    if kind == 'dead' or (r > g * 1.2 and r - b > 16):
        return 1
    return 0


def edge_audit(rgba, kind):
    """Conservative white-matte audit at the final authored texel size.

    Bright orange wood and solid highlights are not background evidence. A
    suspect must touch a cutout, be near white, and exceed a neighbouring
    opaque texel of the same material by at least 64 RGB levels.
    """
    pixels = rgba.load()
    bright, matte = [], {}
    for y in range(rgba.height):
        for x in range(rgba.width):
            rgb = pixels[x, y][:3]
            if pixels[x, y][3] < 128:
                continue
            neighbours = [pixels[xx, yy]
                          for yy in range(max(0, y - 1), min(rgba.height, y + 2))
                          for xx in range(max(0, x - 1), min(rgba.width, x + 2))
                          if (xx, yy) != (x, y)]
            if not any(p[3] < 128 for p in neighbours):
                continue
            m = material(kind, rgb)
            light = max(rgb) if m == 1 else sum(rgb) / 3
            darker = [p[:3] for p in neighbours
                      if p[3] >= 128 and material(kind, p[:3]) == m
                      and max(p[:3]) <= light - 64]
            if light >= 224 and darker:
                bright.append((x, y))
                if min(rgb) >= 224 and max(rgb) - min(rgb) <= 24:
                    # Use an existing same-material shade, not a new palette.
                    darker.sort(key=lambda p: sum(p))
                    matte[x, y] = darker[len(darker) // 2]
    return bright, matte


def defringe(rgba, kind):
    """Darken isolated white matte without eroding branches or moving bounds."""
    out = rgba.copy()
    _, matte = edge_audit(rgba, kind)
    for (x, y), rgb in matte.items():
        out.putpixel((x, y), (*rgb, rgba.getpixel((x, y))[3]))
    return out


def encode(source, kind, audit=None):
    """Trim using generated alpha, then retain coarse nearest-neighbour art."""
    rgba = source.convert('RGBA')
    alpha = rgba.getchannel('A').point(lambda a: 255 if a >= 128 else 0)
    bounds = alpha.getbbox()
    if bounds is None:
        raise ValueError('empty sprite')
    rgba.putalpha(alpha)
    rgba = rgba.crop(bounds)
    aspect = rgba.width / rgba.height
    # Quantize at 80 texels before packing, preserving the requested coarse style.
    rgba = rgba.resize((80, 80), Image.Resampling.NEAREST)
    bounds = rgba.getchannel('A').getbbox()
    rgba = rgba.crop(bounds).resize((EXTENT, EXTENT), Image.Resampling.NEAREST)
    # Thin tips may vanish during sampling. The visible base must reach the
    # grounding plane and both views must use the documented opaque extents.
    for _ in range(3):
        bounds = rgba.getchannel('A').getbbox()
        if bounds == (0, 0, EXTENT, EXTENT):
            break
        rgba = rgba.crop(bounds).resize((EXTENT, EXTENT), Image.Resampling.NEAREST)
    before_bright, before_matte = edge_audit(rgba, kind)
    rgba = defringe(rgba, kind)
    if audit is not None:
        after_bright, after_matte = edge_audit(rgba, kind)
        audit.update(bright_edge_before=len(before_bright),
                     bright_edge_after=len(after_bright),
                     white_matte_before=len(before_matte),
                     white_matte_after=len(after_matte))
    out = Image.new('L', (CELL, CELL), 255)
    for y in range(EXTENT):
        for x in range(EXTENT):
            r, g, b, a = rgba.getpixel((x, y))
            if not a:
                continue
            m = material(kind, (r, g, b))
            # Wood's orange is an ID marker, not a palette to preserve.
            light = max(r, g, b) if m == 1 else (r + g + b) / 3
            shade = round(max(0, min(1, light / 255)) * 31)
            out.putpixel((x + 2, y + 2), m * 32 + shade)
    return out, aspect


def rust_float(value):
    text = f'{value:.6f}'.rstrip('0')
    return text + '0' if text.endswith('.') else text


def build(source, output, audit_path=None):
    output.mkdir(parents=True, exist_ok=True)
    pages, records, audits = [], [], []
    for kind in KINDS:
        for variant in range(1, 4):
            name = f'{kind}-{variant}'
            path = source / f'{name}.png'
            pair = Image.open(path).convert('RGBA')
            cells, aspects = [], []
            for half in range(2):
                audit = dict(name=name, view=('side', 'top')[half])
                cell, aspect = encode(pair.crop((half * pair.width // 2, 0,
                                                (half + 1) * pair.width // 2,
                                                pair.height)), kind, audit)
                audits.append(audit)
                slot = len(records) * 2 + half
                if slot % 4 == 0:
                    pages.append(Image.new('L', (256, 256), 255))
                pages[-1].paste(cell, (slot % 2 * CELL, slot % 4 // 2 * CELL))
                cells.append(slot)
                aspects.append(aspect)
            records.append(dict(name=name, kind=kind, variant=variant,
                                side=cells[0], top=cells[1], radius=aspects[0]/2,
                                top_aspect=aspects[1],
                                source_sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    (output / 'pack.materials').write_bytes(b''.join(p.tobytes() for p in pages))
    (output / 'sprites.json').write_text(json.dumps(records, indent=2) + '\n')
    rows = ['// Generated by tools/pack_vegetation_materials.py.',
            'const SPRITES: &[Sprite] = &[']
    for r in records:
        rows.append('    Sprite { name: "%s", kind: "%s", side: %d, top: %d, radius: %s, top_aspect: %s },' %
                    (r['name'], r['kind'], r['side'], r['top'], rust_float(r['radius']), rust_float(r['top_aspect'])))
    rows.append('];\n')
    (output / 'pack_layout.rs').write_text('\n'.join(rows))
    if audit_path is not None:
        audit_path.write_text(json.dumps(audits, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--audit', type=Path, help='write all side/top edge counts as JSON')
    args = parser.parse_args()
    build(args.source, args.output, args.audit)
