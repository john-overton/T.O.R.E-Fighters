# Quick Mission font atlas

`menu-font.bin` is rasterized from **Noto Sans Bold**, copyright 2022 The Noto
Project Authors. Its SIL Open Font License 1.1 is in `OFL-NotoSans.txt`, copied
from https://github.com/notofonts/latin-greek-cyrillic/blob/main/OFL.txt.
This asset contains no retail game data.

Source `NotoSans-Bold.ttf` SHA-256:
`1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913`.
The source font's name table identifies that copyright and OFL 1.1 license.

Regeneration uses Python and ImageMagick 7:

```sh
python3 tools/build_menu_font.py /path/to/NotoSans-Bold.ttf
```

The atlas contains 256 one-byte advance widths followed by a 4096 by 11
row-major alpha plane. ASCII 32 through 126 is populated. Each glyph has a
16-pixel slot; text advances are proportional. It is rendered at 10 px, with
an 8-pixel baseline in the cropped cell. The app supplies the flat text colour.
Only regeneration needs ImageMagick and the source font. Runtime uses the
committed atlas consistently on Linux, Windows and macOS.
