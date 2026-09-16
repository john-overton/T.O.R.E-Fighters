"""Regenerate the open-licensed menu font atlas with ImageMagick, not retail data."""
import argparse
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('font', type=Path, help='NotoSans-Bold.ttf from Noto Sans')
    args = parser.parse_args()
    output = Path(__file__).resolve().parents[1] / 'crates/tore-app/assets/menu-font.bin'
    advances = bytearray(256)
    pixels = bytearray(256 * 16 * 11)
    advances[32] = 3
    for code in range(33, 127):
        char = chr(code)
        if char == chr(92):
            char *= 2
        cmd = ['magick', '-debug', 'annotate', '-size', '16x16', 'xc:black',
               '-font', str(args.font), '-pointsize', '10', '-fill', 'white',
               '-annotate', '+0+11', char, '-crop', '16x11+0+3', '+repage',
               '-depth', '8', 'gray:-']
        result = subprocess.run(cmd, check=True, capture_output=True)
        widths = re.findall(rb'Metrics:.*?width: ([0-9.]+)', result.stderr)
        if not widths or len(result.stdout) != 176:
            raise RuntimeError(f'Unexpected font output for {char!r}')
        advances[code] = min(16, round(float(widths[-1])))
        for y in range(11):
            start = y * 4096 + code * 16
            pixels[start:start + 16] = result.stdout[y * 16:(y + 1) * 16]
    output.write_bytes(bytes(advances) + pixels)


if __name__ == '__main__':
    main()
