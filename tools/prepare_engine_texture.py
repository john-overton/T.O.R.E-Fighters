"""Prepare the user-supplied engine image for dependency-free runtime loading.
Requires ImageMagick only when changing the source PNG, never while playing.
"""
from pathlib import Path
import struct
import subprocess


def main():
    root = Path(__file__).resolve().parents[1] / 'assets' / 'aircraft'
    source = root / 'engine-texture-full.png'
    reduced = root / 'engine-texture.png'
    subprocess.run(['magick', str(source), '-filter', 'Box', '-resize', '25%', str(reduced)], check=True)
    source = reduced
    size = subprocess.check_output(['magick', 'identify', '-format', '%w %h', str(source)], text=True)
    width, height = map(int, size.split())
    if not (1 <= width <= 2048 and 1 <= height <= 2048):
        raise ValueError('Engine image must be at most 2048 x 2048')
    pixels = subprocess.check_output(['magick', str(source), '-depth', '8', 'rgba:-'])
    if len(pixels) != width * height * 4:
        raise ValueError('Unexpected RGBA byte count')
    (root / 'engine-texture.rgba').write_bytes(b'TORErgba' + struct.pack('<II', width, height) + pixels)


if __name__ == '__main__':
    main()
