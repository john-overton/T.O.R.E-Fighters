# Fighters Anthology main-menu extraction

This is the first native menu slice, researched against the user's local media on 2026-09-13. The TypeScript project's menu contained authored controls and is **not** the visual specification. Its format notes/decoders were reference material; the supplied `gameassets/reference-photos/Main-Screen.jpeg` is the visual target.

## Assets actually used

| Resource | Archive | Purpose |
| --- | --- | --- |
| `CHOOSEV.PIC` | `FA_1.LIB` | Exact aircraft/background variant in the user's photo; 640 × 480, embedded 256-color palette, title, logo, blank menu bar, panels, screws and Jane's plaque |
| `ACTION0L/M/R.PIC` | `FA_1.LIB` | Original enabled green button caps/middle/shadow |
| `ACTIOD0L/M/R.PIC` | `FA_1.LIB` | Original disabled gray button caps/middle/shadow |
| `FONTACT.PIC` / `FONTACD.PIC` | `FA_1.LIB` | Enabled/disabled proportional button labels, each a 1064 × 12 strip with 256 glyph records |
| `MENUFONT.PIC` | `FA_1.LIB` | Original blue menu font, 1487 × 16; top bar and dropdown labels |
| `BODYFONT.PIC` | `FA_1.LIB` | Original small font used for temporary placeholder messages |
| `CHOOSEAC.DLG` | `FA_2.LIB` | Original eight button labels and positions |
| `MAINMENU.MNU`, `FMENUD.MNU` | `FA_2.LIB` | Imported for menu research; runtime submenu structure is still authored |
| `&CLICK.11K`, `&BUTTON.11K`, `&TOGGLE1.5K` | `FA_2.LIB` | Hover, activation, and toggle sound cues; trigger mapping is reconstructed |
| `AIR003.11K` | `FA_4B.LIB` (optional) | Recorded PCM music preview; 278,585 samples at the inferred 11,025 Hz rate (~25.27 seconds) |

There are 18 selected resources including optional music. All were decompressed by Rust and compared byte for byte against the reference Python decoders. Only these resources are imported. No photo is used as the rendered background; the app reconstructs the scene from retail resources.

`CHOOSEAC.PIC` itself shows a stealth aircraft, not the supplied photo. Other alternatives found are `CHOOSE3` (Rafale), `CHOOSEM` (pilot/cockpit), and `CHOOSEU` (carrier deck). `CHOOSEV` matches the photo's aircraft pair. The embedded screen palette must also color the button/font sprites: using the flight `PALETTE.PAL` produces incorrect UI colors.

## Recovered layout

`CHOOSEAC.DLG` reports panel rectangle `(379, 80, 238, 361)`. Coordinates below are absolute, in the 640 × 480 canvas. Width includes the sprite's shadow area; the visible button face/hit area is ten pixels narrower.

| Button | X | Y | Width | Current behavior |
| --- | ---: | ---: | ---: | --- |
| Play Single Mission | 423 | 104 | 144 | Animated placeholder |
| Create Quick Mission | 423 | 136 | 144 | Animated placeholder |
| Create Pro Mission | 423 | 168 | 144 | Animated placeholder |
| Replay Last Mission | 423 | 200 | 144 | Disabled |
| Reference | 411 | 254 | 170 | Animated placeholder |
| Start New Campaign | 423 | 331 | 144 | Animated placeholder |
| Continue Old Campaign | 416 | 363 | 158 | Disabled |
| View Pilot Records | 423 | 395 | 144 | Animated placeholder |

These differ from the USNF/ATF records described in the older menu port. Runtime positions and labels come from the local DLG, not copied React/CSS coordinates. Action sprites are composed from a 24-pixel left cap, repeating 8-pixel middle, and 29-pixel right cap, all 30 pixels tall. Their masks preserve original shadows. Font glyph widths determine centered label positions.

`ACTION0` visually matches the photograph. `ACTION1..3` have different visible row counts and are not established hover/press frames. The current 120 ms brightness transition, one-pixel pressed displacement, keyboard outline, transient placeholder messages, and hit-area dimensions are authored. They must not be described as recovered native animation semantics.

## Formats and limits

- **EALIB:** validates magic, directory bounds, monotonic offsets, flags, and terminal sentinel. Flag 0 reads stored content; flag 4 reads a size prefix and DCL stream. Lookup is case-insensitive, last duplicate wins. No resource paths are used for extraction.
- **DCL:** raw literals (mode 0), dictionary bits 4–6, canonical length/distance codes, overlapping back-references, explicit terminator, and exact output size. A 16 MiB output cap prevents oversized allocations. All 7,372 compressed entries in the inspected archive directories advertise `00 06`; only the selected menu resources have been decompressed/validated by the runtime in this pass. Coded-literal mode 1 is rejected. See [third-party notices](../../THIRD_PARTY_NOTICES.md).
- **PIC:** bounded raw rasters and span sprites, 6-bit palette expansion, row-offset checks, span terminator/coverage checks, and 256 glyph records. Menu images are capped at 4,194,304 pixels. Font-strip index 255 is transparent; ordinary sprite transparency comes from span coverage, not a universal palette key.
- **DLG:** narrow CHOOSEAC reader. Parses PE/PL sections and relocation records as data, identifies plausible relocated label fields, and validates eight in-bounds button records. This is not a general widget/thunk-class decoder. It never executes x86 resource code.
- **MNU:** reference decoder recovers some labels, including `Exit to Windows` with `Alt-F4` in `MAINMENU.MNU` and `Pref`, `Graphics...`, `Sound...`, `Multi` in `FMENUD.MNU`. Tree flags, exact activity-menu composition, and native dropdown drawing are unresolved.
- **PCM:** unsigned 8-bit mono; `.11K` is treated as 11,025 Hz and `.5K` as 5,512 Hz following the reference convention. The application uses original sample bytes with linear resampling and authored gains, loop, and cue scheduling. No XMI synthesizer or instrument bank has been implemented.

The menu fonts are **PIC glyph strips**, not the separate compiled `.FNT` resources. Those FNT files remain unimplemented in Rust. `LAY` is not assumed to be a menu layout format.

## Music evidence and uncertainty

`FA_4B.LIB` contains 77 PCM tracks named like the XMI tracks in `FA_2.LIB`. `AIR003.11K` also occurs in the supplied `FA.EXE`: string VA `0x50c838`, referenced at `0x4b27cd` in a small routine starting at `0x4b27c0`. That routine copies the filename and calls routines at `0x4a6cc0` and `0x4a6ce0`. This establishes executable interest in the file, **not** that it is the activity-menu loop; call semantics and callers remain untraced.

The app previews that original PCM recording at low volume and allows disabling it. Native title/menu music mapping, loop behavior, and mixer rates are not claimed. `TITLE95.SEQ` references `^MF.11K`, but that asset was not found in the supplied archives. No replacement title recording was invented or downloaded.

## Stub behavior

- The six available activity buttons stay on the menu and briefly announce “coming soon.” Replay/continue are inert.
- `?` contains stub Help/About and working Exit to Desktop (portable wording for the recovered Windows exit action).
- `Pref` contains stub Graphics/Sound/Controls and working session-only music/effects toggles.
- `Multi` contains stub Host Game/Join Game/Player Setup. These labels/groupings and dropdown chrome are authored scaffolding, not a claimed decoded retail tree.
- Tab/arrows and Enter navigate; Escape dismisses; M toggles music; Command-Q/Alt-F4 and window close exit.

## Provenance and reproduction

Local reference sources: `USNF-ATF/Docs/formats/{ealib,dcl,pic,pal,mnu,fnt}.md` and `USNF-ATF/tools/retail/retail/{ealib,dcl,pic,mnu,audio}.py`, checkout `2d818054ff51db9f3353d0548dbd0e469b275a1a`.

Run `python3 tools/explore_assets.py` for a fresh inventory with archive hashes/offsets. Run `cargo run --locked -p tore-app -- --import gameassets/fighters-anthology --import-only` for native selective extraction. See [development commands](../DEVELOPMENT.md) for cache locations and preview generation. Detailed inventories, decoder hashes, and images remain local under `.local/exploration/`.
