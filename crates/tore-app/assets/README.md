# Bundled assets

Everything in this directory is either openly licensed or our own work.
None of it contains retail game data.

## Menu font atlases

There are two atlases, both rasterized from **Noto Sans**, copyright 2022 The
Noto Project Authors. The SIL Open Font License 1.1 is in `OFL-NotoSans.txt`,
copied from
https://github.com/notofonts/latin-greek-cyrillic/blob/main/OFL.txt. Neither
asset contains any retail game data.

- `menu-font.bin`, 16 by 11 cells rendered at 10 px from **Noto Sans Bold**.
  Used by the Quick Mission and ordnance screens, which are laid out around
  it. Source `NotoSans-Bold.ttf` SHA-256:
  `1df075a380fc7cb898acf64c1f7b3b4dd780de3caa860178bf929de35817a913`.
- `menu-font-large.bin`, 24 by 17 cells rendered at 14 px from **Noto Sans
  Medium**. Used by the locate screen and the main-menu version label, where
  the 11 px atlas broke up into blocks once the 640x480 canvas was scaled to a
  1080p or fullscreen window. Source `NotoSans-Medium.ttf` SHA-256:
  `635d93d1131d791f2576de90b3bb0f7cdf61929906e8420a61b5f7f8e76420bb`.

Each source font's name table identifies that copyright and OFL 1.1 license.

Both files use the same format: 256 one-byte advance widths, then a row-major
8-bit alpha plane `256 * cell width` pixels across and one cell tall. Glyph
`code` occupies the columns `code * cell width` onwards on every row, and its
advance byte says how far the pen moves after it. ASCII 32 through 126 is
populated and everything else is blank with a zero advance. There is no colour
in the file: the app supplies a flat text colour and the stored alpha is
coverage, so glyph edges blend with whatever is behind them.

Regeneration uses Python and ImageMagick 7:

```sh
python3 tools/build_menu_font.py /path/to/NotoSans-Bold.ttf \
    --large /path/to/NotoSans-Medium.ttf
```

Either source may be left out; only the named atlas is rewritten. Only
regeneration needs ImageMagick and the source fonts. Runtime uses the
committed atlases consistently on Linux, Windows and macOS.

## Application icon

`icon/tore-*.png`, `icon/tore.ico` and `icon/tore-64.rgba` are downscales of
`docs/images/tore-fighters-logo.png`, the project logo: a render of a round
embroidered patch. It is our own artwork, made for this project, and is covered
by the repository `LICENSE` like the rest of the source. It is not retail art
and contains no retail game data.

The committed set is 16, 32, 48, 64, 128, 256 and 512 px plus the Windows
`tore.ico` and a 16 KiB straight-alpha RGBA copy of the 64 px icon. The set
stays under a 600 KB total budget that the generator enforces.
1024 px is deliberately absent and 512 px is quantized to 255 colours; both
choices are explained in [packaging](../../../docs/DEVELOPMENT.md#application-icon).

Regeneration needs Python and ImageMagick 7:

```sh
python3 tools/package/build_icons.py
```

`crates/tore-app/build.rs` reads `icon/tore.ico` and embeds it in
`tore-app.exe` on windows-msvc. The packaging scripts read the PNGs for the
Linux desktop entry and the macOS `.icns`. The app embeds `tore-64.rgba` for
the main-menu badge; no external image or runtime image decoder is needed.
