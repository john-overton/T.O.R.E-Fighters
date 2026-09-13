# Fighters Anthology main-menu extraction

This is the first native menu slice, researched against the user's local media on 2026-09-13. The TypeScript project's menu contained authored controls and is **not** the visual specification. Its format notes/decoders were reference material; the supplied `gameassets/reference-photos/Main-Screen.jpeg` is the visual target.

## Assets actually used

| Resource | Archive | Purpose |
| --- | --- | --- |
| `CHOOSEV.PIC` | `FA_1.LIB` | Exact aircraft/background variant in the user's photo; 640 × 480, embedded 256-color palette, title, logo, blank menu bar, panels, screws and Jane's plaque |
| `CHOOSEAC.PIC`, `CHOOSE3.PIC`, `CHOOSEU.PIC`, `CHOOSEM.PIC` | `FA_1.LIB` | Other original backgrounds; randomly selected on menu setup |
| `ACTION0L/M/R.PIC` | `FA_1.LIB` | Original enabled green button caps/middle/shadow |
| `ACTIOD0L/M/R.PIC` | `FA_1.LIB` | Original disabled gray button caps/middle/shadow |
| `FONTACT.PIC` / `FONTACD.PIC` | `FA_1.LIB` | Enabled/disabled proportional button labels, each a 1064 × 12 strip with 256 glyph records |
| `MENUFONT.PIC` | `FA_1.LIB` | Original blue menu font, 1487 × 16; top bar and dropdown labels |
| `BODYFONT.PIC` | `FA_1.LIB` | Original small font used for temporary placeholder messages |
| `CHOOSEAC.DLG` | `FA_2.LIB` | Original eight button labels and positions |
| `MAINMENU.MNU`, `FMENUD.MNU` | `FA_2.LIB` | Imported for menu research; runtime submenu structure is still authored |
| `&CLICK.11K`, `&BUTTON.11K`, `&TOGGLE1.5K` | `FA_2.LIB` | Recovered cue bank; activation uses BUTTON and toggles use TOGGLE1. CLICK is retained for research. Hover/focus is silent. |
| `AIR003.11K` | `FA_4B.LIB` (optional) | Recorded PCM music preview; 278,585 samples at the inferred 11,025 Hz rate (~25.27 seconds) |

There are 22 selected resources including optional music. All were decompressed by Rust and compared byte for byte against the reference Python decoders. Only these resources are imported into the app cache. The separate [general extraction tool](../EXTRACTION.md) can unpack every resource for research. No photo is used as the rendered background; the app reconstructs the scene from retail resources.

`CHOOSEAC.PIC` itself shows a stealth aircraft, not the supplied photo. Other alternatives found are `CHOOSE3` (Rafale), `CHOOSEM` (pilot/cockpit), and `CHOOSEU` (carrier deck). `CHOOSEV` matches the photo's aircraft pair. The embedded screen palette must also color the button/font sprites: using the flight `PALETTE.PAL` produces incorrect UI colors.

## Recovered layout

### Background selection in the executable

The menu setup routine at `FA.EXE` VA `0x4a08a0` requests a random value with upper bound five at `0x4a08f2..0x4a08f7` (`ECX=5`, call `0x4562f0`, which delegates to the generator at `0x4561d0`). The jump table at `0x4a0f24` selects these branches:

| Choice | Branch VA | Background | Native bar X |
| ---: | --- | --- | ---: |
| 0 | `0x4a091e` | CHOOSEAC | 70 |
| 1 | `0x4a0931` | CHOOSE3 | 185 |
| 2 | `0x4a0947` | CHOOSEU | 76 |
| 3 | `0x4a094e` | CHOOSEM | 76 |
| 4 | `0x4a0955` | CHOOSEV | 76 |

The app now chooses among all five on startup/menu construction, uses each background's embedded palette, and shifts top-bar rendering/hit regions to match. This is startup selection, not an invented timed slideshow; the original random sequence itself is not reproduced. `--background NAME` pins a variant for comparisons. Snapshots default to CHOOSEV unless explicitly overridden.

### Action layout

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
- **DCL:** raw literals (mode 0), dictionary bits 4–6, canonical length/distance codes, overlapping back-references, explicit terminator, and exact output size. The menu uses a 16 MiB output cap; the general extractor has a configurable cap. All 7,372 compressed entries advertise `00 06` and were successfully decompressed by the general extractor. The 22 menu resources also matched independent reference output byte for byte. Coded-literal mode 1 is rejected. See [third-party notices](../../THIRD_PARTY_NOTICES.md).
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

## Quick Mission Creator investigation shell (2026-09-13)

Choose Activity's Create Quick Mission action now opens the creator mock. Native setup at FA.EXE `0x42eb20–0x42eb33` chooses `QUIKMIS3.PIC` versus `QUIKMISS.PIC`; the former carries the Fighters Anthology logo matching the supplied reference photo. Bar placement around `0x42eb3c` uses an 84-pixel origin. The mock uses QUIKMIS3's own palette, original PIC font strips and button pieces, plus the original Ukraine briefing map.

The fields, Aircraft stub, theater dropdown, temporary Terrain Viewer button and help entries are authored for the requested investigation workflow. They do not establish recovered general DLG/MNU support. Click activation requires press/release on the same control; hover and focus stay silent. Escape dismisses the dropdown before returning to the previous screen. See [viewer baseline](../baselines/ukraine-viewer.md) and [parity backlog](../progress.md).

### Typography correction

The initial creator/notice BODYFONT face was unsuitable, and the tint path incorrectly replaced dark/edge pixels with solid white. The creator and placeholder notices now use the retail ARMFont sans-serif strip, with original shading multiplied by the requested tint; SMLFONT serves the compact HUD. FONTACT remains the button font. Synthetic tests verify shading and transparency; local font comparison and creator/notice captures are under `.local/font-audit/`. This is an authored face choice matching the supplied reference more closely, not a claim to have decoded the original font-selection call. All fonts remain raster artwork.
