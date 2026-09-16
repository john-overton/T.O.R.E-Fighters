# Fighters Anthology main-menu extraction

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Research notes — research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature — see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


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

**Superseded for current playback (2026-09-14):** the [FA audio pass](music.md)
recovers native shell playlists and uses original recorded PCM. AIR003 is no
longer the menu preview loop. The following paragraph records the earlier
investigation, not current selection.

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

The fields, Aircraft stub, theater dropdown, temporary Terrain Viewer button and help entries are authored for the requested investigation workflow. They do not establish recovered general DLG/MNU support. Click activation requires press/release on the same control; hover and focus stay silent. Escape dismisses the dropdown before returning to the previous screen. See [viewer baseline](../baselines/ukraine-viewer.md) and [parity backlog](../research/progress.md).

### Typography correction

The initial creator/notice BODYFONT face was unsuitable, and the tint path incorrectly replaced dark/edge pixels with solid white. The creator and placeholder notices now use the retail ARMFont sans-serif strip, with original shading multiplied by the requested tint; SMLFONT serves the compact HUD. FONTACT remains the button font. Synthetic tests verify shading and transparency; local font comparison and creator/notice captures are under `.local/font-audit/`. This is an authored face choice matching the supplied reference more closely, not a claim to have decoded the original font-selection call. All fonts remain raster artwork.

## F/A-18D launch follow-up

The temporary Terrain Viewer button is replaced by Free Flight. The creator starts the selected theater with the imported Hornet, clean external fit and no loadout page. Other mission fields use dotted placeholders. The original menu and click-only sounds remain. See [aircraft/instrument coverage](aircraft.md).


## FA in-flight menu tree recovery

`FMENUD.MNU` is now parsed as bounded inert data in `tore-formats::ui::flight_menu`. The reviewed CODE tree begins at offset zero. Each node starts with sibling and child RVAs at +0/+4. Top-level/anonymous container labels begin at +24; selectable row labels follow the +18 `0x1e` marker. NUL terminates labels, `0x01` separates accelerators, `0x7f` marks submenu text, and inline `0x1d` is rendered as an arrow. Anonymous containers are flattened while preserving their child order. The reader checks RVA bounds, label lengths/encoding, cycles, depth (8), and total nodes (256); it never executes handlers or uses native pointers.

This recovers the supplied FA tree including `?`, Control, Pref, View, Window, Cheat, Multi, Map and Pos. It supersedes treating the flight menu tree as entirely authored. Runtime validates that reviewed root structure before presenting it. Generic menu flags, check-state callbacks, visibility of Map/Pos in different native modes, and other editions remain unverified. Native tree recovery does not implement its underlying cheats, multiplayer or flight systems.

`flight_ui.rs` provides keyboard/mouse traversal and action dispatch, with matching press/release and silent hover. Native actions without a port show explicit feedback. The paused overlay's placement, submenu presentation, and bottom Resume/Restart/Keyboard Shortcuts actions are authored. Portable Exit to Desktop wording replaces the source Exit to Windows label. Source labels/accelerators remain external imported data. See [controls](../FLIGHT-CONTROLS.md).

## Briefing text selectors — 2026-09-14

The creator now follows `gameassets/reference-photos/quick-mission-creator-screen.jpg`:
Friendly Situation at left, Enemy Situation at right, selectable aircraft in
Wing 1, and selectable theater inside “You are flying over…”. The separate map
inset and Theater/Flight Setup box are removed. Aircraft on the top bar opens
the same aircraft selector. OK starts clean free flight; Cancel returns.

Original QUIKMIS3, ARMFONT/MENUFONT/FONTACT and button pieces are retained. The
blue OK button uses imported ACTDFT0L/M/R; Cancel uses ACTION0L/M/R. Briefing field
rectangles derive from the original font's glyph advances. Unavailable wing,
enemy, loadout, weather and mission options have dim text/boxes without hit
regions. The exact fills, popup layouts, keyboard traversal and hover/press
presentation are fitted/authored, not decoded native DLG behavior. No opponents
are spawned and no editable setting is represented as implemented merely to fill
the screenshot. Matching press/release and click-only audio remain enforced.

Both selectors support mouse and keyboard, commit only on selection, and close
on Escape before leaving the creator. [Acceptance evidence](../baselines/rafale-quick-mission.md).

### Authored controls editor and saved preferences

The in-flight `Control` root now opens the T.O.R.E binding/rumble editor. Its rows,
capture workflow, calibration fields and Save/Back actions are authored, not
recovered FMENUD callbacks. The bounded reader and imported source tree are
unchanged. Other source roots and the keyboard-help reference remain available.
Normal-session music/effects and flight display/instrument preferences persist;
the earlier session-only behavior is superseded. See [input settings](../INPUT.md).

## Creator and ordnance menu-tree recovery — 2026-09-14

The existing bounded FMENUD grammar also reads QM_MENU.MNU and ARMPLANE.MNU.
It is exposed as `ui::menu_tree`; `flight_menu` remains a compatible wrapper.
The `menu_tree` example inspects extracted files without loading any native code.

QM_MENU recovers Aircraft → Fly all and Era → four source year ranges. The
current app's aircraft-selector shortcut is still authored. ARMPLANE recovers
Weapons → Unload All / Cheat (load anything anywhere), Airbase → Next Aircraft
(`]`) / Previous Aircraft (`[`), and Campaign → Replay This Mission / Exit Campaign.
Both include source exit actions. The screenshot's missing Campaign root means
mode-dependent visibility must be recovered before presenting the full resource
tree as the quick-mission screen. Native callbacks remain unimplemented.

`--native-menus` now makes creator-filter and ordnance-loading research repeatable,
with both executable/symbol hashes gating fixed addresses. String references
remain research candidates rather than imported active options. Full DLG record
recovery and screen implementation remain open. [Evidence](../baselines/menu-contract-pass.md).

### Active option tables and static DLG geometry

The next pass recovers the live selector dispatch, including theater-dependent
lists, and establishes QUICK14 as the shared runtime-populated selector. A bounded
import/relocation reader now inspects creator/ordnance DLG controls, while the
native text compositor supplies separate briefing rectangles. This supersedes
using stale QUICKB strings as active choices. See [the source contract](quick-mission.md)
and [validation](../baselines/menu-options-geometry.md). Screen rendering and
runtime inline hit regions remain unimplemented by this research step.
