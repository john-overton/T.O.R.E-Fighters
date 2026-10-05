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
populated, as are the CP437 letters the original fonts draw above 0x7F (imported
names such as the Kurile airport Berëzovka use them), and everything else is
blank with a zero advance. There is no colour
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

## Sharp UI text and icon atlas

`ui-text.bin` holds **Noto Sans Medium** (the same source font and SHA-256 as
`menu-font-large.bin`, copyright 2022 The Noto Project Authors, SIL Open Font
License 1.1 in `OFL-NotoSans.txt`) rendered at 48 px to the em, so the
multiplayer screens can draw their text at the window's resolution instead of
stretching the retail bitmap fonts. It also holds the screens' icons,
rasterized at 64 by 64 from the SVGs in `icons/` as coverage only (the game
tints them). It contains no retail game data. The file layout is documented in
`tools/build_ui_text_atlas.py`; the app lays the glyphs out in the retail fonts'
sizes and places (`docs/formats/menu.md`, "Sharp text"). Regeneration needs
Python, ImageMagick 7 and `rsvg-convert` (librsvg):

```sh
python3 tools/build_ui_text_atlas.py /path/to/NotoSans-Medium.ttf
```

## Icons

`icons/*.svg` are the marks a list row can carry: `lock` (a game with a
password), `crown` (the King), `ready` (a tick), `house` (the player whose
machine hosts), `you` (the player's own slot), `unable` (a cross) and the
platforms `windows`, `macos` and `linux`. They are hand drawn for this project
by an agent at John's request (2026-10-05): minimalist solid shapes on a 16 by
16 grid, all one colour, and covered by the repository `LICENSE` like the rest
of the source. They hold no retail art. The three platform marks are our own
simplified drawings of a window of four panes, an apple with a leaf and a bite,
and a penguin; they are not the vendors' logo artwork, and the Windows, Apple
and Linux names and marks belong to their owners.

## Connection screens' title lettering

`direct-network-connection-title.png` is the words DIRECT NETWORK CONNECTION, and
`internet-lobby-title.png` (slice I4, the Internet Lobby screen) the words
INTERNET LOBBY, each in
**Liberation Sans Regular** (version 2.1.5, copyright 2010 Google Corporation
and 2012 Red Hat, Inc., SIL Open Font License 1.1 in `OFL-LiberationSans.txt`,
copied from the font's `LICENSE`) over a dark copy of the same words offset two
pixels right and down as a shadow, 564 by 36 pixels on a transparent
background. It is drawn at the top right of the Direct Connection and lobby
screens' title bar, where the first letter starts at x 80 as retail's lettering
did. It holds no retail art: every clear pixel is `0, 0, 0, 0`, and the game
covers retail's own lettering with the bar's texture from the player's import,
in memory. Source `LiberationSans-Regular.ttf` SHA-256:
`baccc64becc3eb7d104b7c84d99f5314a0a1f896e2b3ea6c2f22fc08d2003bee`.

John first set the words in Helvetica, a commercial typeface, and asked on
2026-10-05 for this open-licensed redraw instead, sized to his (29 points,
narrowed by 4.9 percent). Regeneration needs Python and ImageMagick 7:

```sh
python3 tools/build_title_lettering.py /path/to/LiberationSans-Regular.ttf
python3 tools/build_title_lettering.py --title internet /path/to/LiberationSans-Regular.ttf
```

Both are lettered the same way and start at x 80 on the bar; a player's own
`DirectNetworkConnection.png` or `InternetLobby.png` in the data folder replaces
the one for its screen.

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
