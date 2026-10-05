"""Regenerate the connection screens' title lettering, not retail data.

`crates/tore-app/assets/direct-network-connection-title.png` is the words
DIRECT NETWORK CONNECTION in Liberation Sans Regular (SIL Open Font License
1.1, a metric compatible open face) over a dark copy of the same words offset
two pixels right and down, on a transparent background, 564 by 36 pixels. The
game covers retail's own title lettering with the bar's texture and draws this
over it, fixed to the title bar's top right so the first letter starts at x 80
where the retail lettering did. See `docs/formats/menu.md` ("The Direct
Connection screen as built") and `crates/tore-app/assets/README.md`.

The size was fitted to the lettering John first set in Helvetica and
shadowed the same way, with the words twice (a dark copy under a white one,
offset slightly): its white was 439 pixels wide and 21 tall. Here the face is
set at 29 points and drawn 4.9 percent narrower than its natural width, so the
letters keep their even spacing. The edges are antialiased from a picture drawn
four times as large. The shadow is 31 grey and sharp. Every clear pixel is
written `0, 0, 0, 0`, so no colour is hidden under it. Only ImageMagick 7 and
Python are needed.

Usage:

    python3 tools/build_title_lettering.py \
        /usr/share/fonts/liberation/LiberationSans-Regular.ttf
"""
import argparse
from pathlib import Path
import subprocess

ASSET = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets/direct-network-connection-title.png'
TEXT = 'DIRECT NETWORK CONNECTION'
WIDTH, HEIGHT = 564, 36
SUPER = 4  # the picture is drawn this many times as large, then averaged down
POINTS = 29
SQUEEZE = 0.951  # across, so the words are 439 pixels wide
LEFT, TOP = 4.6, 8.0  # where the white's top left corner goes
SHADOW = (31, 2, 2)  # grey, then the offset right and down in pixels


def big_plane(font):
    """The words on a plane SUPER times as large, 8-bit coverage."""
    cmd = ['magick', '-size', f'{WIDTH * SUPER}x{HEIGHT * SUPER}', 'xc:black', '-font', str(font),
           '-pointsize', str(POINTS * SUPER), '-fill', 'white',
           '-annotate', f'+{6 * SUPER}+{30 * SUPER}', TEXT, '-depth', '8', 'gray:-']
    plane = subprocess.run(cmd, check=True, capture_output=True).stdout
    if len(plane) != WIDTH * HEIGHT * SUPER * SUPER:
        raise RuntimeError('unexpected ImageMagick output')
    return plane


def coverage(plane, corner, dx=0, dy=0):
    """The plane averaged down to WIDTH by HEIGHT, squeezed across, with its
    white corner at LEFT, TOP shifted by (dx, dy) pixels."""
    wide = WIDTH * SUPER
    out = []
    for y in range(HEIGHT):
        for x in range(WIDTH):
            x0 = corner[0] + ((x - dx) - LEFT) * SUPER / SQUEEZE
            y0 = corner[1] + ((y - dy) - TOP) * SUPER
            xa, xb = max(int(x0), 0), min(int(x0 + SUPER / SQUEEZE) + 1, wide)
            ya, yb = max(int(y0), 0), min(int(y0 + SUPER) + 1, HEIGHT * SUPER)
            if xb <= xa or yb <= ya:
                out.append(0)
                continue
            total = sum(sum(plane[row * wide + xa:row * wide + xb]) for row in range(ya, yb))
            out.append(total // ((xb - xa) * (yb - ya)))
    return out


def build(font):
    plane = big_plane(font)
    wide = WIDTH * SUPER
    lit = [i for i, v in enumerate(plane) if v > 128]
    corner = (min(i % wide for i in lit), min(i // wide for i in lit))
    text = coverage(plane, corner)
    shadow = coverage(plane, corner, SHADOW[1], SHADOW[2])
    grey = SHADOW[0]
    rgba = bytearray()
    for t, s in zip(text, shadow):
        ta, sa = t / 255, s / 255
        alpha = ta + sa * (1 - ta)
        if alpha <= 0:
            rgba += bytes((0, 0, 0, 0))
            continue
        colour = (255 * ta + grey * sa * (1 - ta)) / alpha
        rgba += bytes((round(colour),) * 3 + (round(alpha * 255),))
    subprocess.run(['magick', '-size', f'{WIDTH}x{HEIGHT}', '-depth', '8', 'rgba:-',
                    '-strip', f'PNG32:{ASSET}'], check=True, input=bytes(rgba))
    print(f'{ASSET.name}: {WIDTH}x{HEIGHT}, {ASSET.stat().st_size} bytes')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path, help='LiberationSans-Regular.ttf')
    build(parser.parse_args().font)


if __name__ == '__main__':
    main()
