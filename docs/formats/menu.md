# Fighters Anthology main-menu extraction

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).


This is the first native menu slice, researched against the user's local media on 2026-09-13. The TypeScript project's menu contained authored controls and is **not** the visual specification. Its format notes/decoders were reference material; the supplied `gameassets/reference-photos/Main-Screen.jpeg` is the visual target.

## Assets actually used

| Resource | Archive | Purpose |
| --- | --- | --- |
| `CHOOSEV.PIC` | `FA_1.LIB` | Exact aircraft/background variant in the user's photo; 640 × 480, embedded 256-color palette, title, logo, blank menu bar, panels, screws and Jane's plaque |
| `CHOOSEAC.PIC`, `CHOOSE3.PIC`, `CHOOSEU.PIC`, `CHOOSEM.PIC` | `FA_1.LIB` | Other original backgrounds; randomly selected on menu setup |
| `ACTDFLT.PIC` | `FA_1.LIB` | 20 by 27 default-button cap: striped marker, blue outer edge and left face rim |
| `ACTION0L/M/R.PIC` | `FA_1.LIB` | Original enabled green button caps/middle/shadow |
| `ACTIOD0L/M/R.PIC` | `FA_1.LIB` | Original disabled gray button caps/middle/shadow |
| `FONTACT.PIC` / `FONTACD.PIC` | `FA_1.LIB` | Enabled/disabled proportional button labels, each a 1064 × 12 strip with 256 glyph records |
| `MENUFONT.PIC` | `FA_1.LIB` | Original blue menu font, 1487 × 16; top bar and dropdown labels |
| `BODYFONT.PIC` | `FA_1.LIB` | Original small font used for temporary placeholder messages |
| `CHOOSEAC.DLG` | `FA_2.LIB` | Original eight button labels and positions |
| `MAINMENU.MNU`, `FMENUD.MNU` | `FA_2.LIB` | Imported for menu research; runtime submenu structure is still authored |
| `&CLICK.11K`, `&BUTTON.11K`, `&TOGGLE1.5K` | `FA_2.LIB` | Recovered cue bank; activation uses BUTTON. TOGGLE1 is imported but no current control plays it: the Sound/Music Prefs lever uses `&SWITCH.11K`. CLICK is retained for research. Hover/focus is silent. |
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
- **DLG:** `tore_formats::ui::dialog` parses PE/PL sections and relocation records as data and identifies draw records by their relocated imported draw thunks. It reads actions, list boxes, rockers, text, edit boxes and check boxes, and a size-zero dialog's picture name (see [Multiplayer connection screens](#multiplayer-connection-screens-dlg-records-panels-and-widget-pieces-ef0-2026-10-01)). It does not decode sliders, dials or the setup thunk's work. It never executes x86 resource code.
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
- `Pref` contains Graphics..., Sound..., Controls... and Re-import media.... Sound... opens the retail [Sound/Music Prefs](../spec/sound-prefs.md) dialog.
- `Multi` contains two authored rows (EF7, 2026-10-01): *Direct Connection...*, which opens the [Direct Connection screen](#the-direct-connection-screen-as-built-ef7), and *Internet Lobby...*, a stub that announces "coming soon" (the public lobby of stage I). They replace the earlier stub Host Game/Join Game/Player Setup. These labels/groupings and dropdown chrome are authored scaffolding, not a claimed decoded retail tree (retail's six rows are listed below).
- Tab/arrows and Enter navigate; Escape dismisses; Command-Q/Alt-F4 and window close exit.

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

`pause_menu.rs` provides keyboard/mouse traversal, hit geometry and drawing, with matching press/release and silent hover; `flight_ui.rs` dispatches the flight actions, and the [replay viewer](../REPLAYS.md#pause-menu) builds its own smaller tree from the imported `?` and `Pref` rows on the same widget. Native actions without a port show explicit feedback. The paused overlay's placement, submenu presentation, and bottom Resume/Restart/Keyboard Shortcuts actions are authored. Portable Exit to Desktop wording replaces the source Exit to Windows label. Source labels/accelerators remain external imported data. See [controls](../FLIGHT-CONTROLS.md).

## Briefing text selectors, 2026-09-14

The creator now follows `gameassets/reference-photos/quick-mission-creator-screen.jpg`:
Friendly Situation at left, Enemy Situation at right, selectable aircraft in
Wing 1, and selectable theater inside “You are flying over…”. The separate map
inset and Theater/Flight Setup box are removed. Aircraft on the top bar opens
the same aircraft selector. OK starts clean free flight; Cancel returns.

Original QUIKMIS3, ARMFONT/MENUFONT/FONTACT and button pieces are retained.
Current selector presentation, compact font metrics, bevels and filtering are
specified in the [Quick Mission menu spec](../spec/quick-mission-menu.md).
All briefing fields remain editable, with unsupported mission settings rejected
at launch. Selectors commit with OK/Enter; Cancel/Escape preserves the draft.
Matching press/release and click-only audio remain enforced.

### Authored controls editor and saved preferences

The in-flight `Control` root now opens the T.O.R.E binding/rumble editor. Its rows,
capture workflow, calibration fields and Save/Back actions are authored, not
recovered FMENUD callbacks. The bounded reader and imported source tree are
unchanged. Other source roots and the keyboard-help reference remain available.
Normal-session sound settings and flight display/instrument preferences persist;
the earlier session-only behavior is superseded. See [input settings](../INPUT.md).

## Creator and ordnance menu-tree recovery, 2026-09-14

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

## Multiplayer connection screens: DLG records, panels and widget pieces (EF0, 2026-10-01)

Research mode. This section holds the facts behind the retail connection
screens a player sees in multiplayer; the player-visible behaviour is in the
[multiplayer spec](../spec/multiplayer.md#retail-connection-screens). The
stage E and F slices that import and draw these screens read both.

Build identity: installed game `FA.EXE` 1.02F, SHA-256
`e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`;
`FA_2.LIB` SHA-256 begins `fb8b30216e739292` and `FA_1.LIB` `657254c5bb3bcf36`.
The 1.0 disc (`disc1/SETUP.ESA`) holds the same pictures byte for byte and
dialogs that differ by four bytes (the module link timestamp, file offsets 137
to 140); the decoded geometry is identical.

Evidence labels: **R** read from a retail resource with a repository reader,
**S** static reading of `FA.EXE` (addresses are virtual addresses), **M**
measured on John's retail screenshot of 2026-10-01 (`NETWORK CONNECTION`, taken
at 2403 by 1801 pixels), **I** inference, **U** unknown.

### Screenshot registration (M)

The screenshot is the 640 by 480 screen enlarged by 3.770 horizontally and
3.768 vertically and cropped, not 2403 divided by 640. Registering it on the
`NETIPX3.PIC` background (its top 80 rows are untouched by the dialog; the fit
reaches a correlation of 0.998) gives x = 3.770 (px + 0.5) - 4.25 and
y = 3.768 (py + 0.5) - 2.0. After that every rectangle below matches a whole
pixel, and the dialog rectangle from the header (10, 80, 619 by 395) is exact.
Anyone measuring a screenshot of the game must register it first; with the
nominal scale every edge is off by up to two pixels.

### DLG records (R, S)

All DLGs are PE-style modules whose CODE section holds the data; the reader
returns, per draw record, its static local position. Positions are local to the
dialog origin.

Header (code offsets):

| Offset | Meaning |
| --- | --- |
| 0 | Pointer to a setup thunk, or zero. `_MultiPreload` on `NEWNET`, `NETNEW`, `NETJOIN`, `NETTCP`, `NETIPX2`, `NETDIR`, `FORTAIRB`; `_ChoosePreload` on `CHOOSEAC` and `MC_DLG`. The loader calls it when the dialog opens (S, `0x487c35`). What it does is not decoded (U) |
| 4, 6 | Origin x, y (signed 16 bit) |
| 8, 10 | Width, height. **Zero means the dialog is a picture** (see below) |
| 12 | Zero |
| 14 | For a size-zero dialog, the name of the picture, a NUL-terminated string (`MODEM`, `MODEMCOM`, `SERIAL`, `COM`, `NETIPX`, `NETDIR`, `MC`). For a sized dialog the bytes are `ff 00 00 00` (meaning unknown) |

A sized dialog gets the **generic panel** of that size drawn at its origin
(recipe below). A size-zero dialog draws the named `.PIC` at its origin and
nothing else. `MODEMSTS` is the one exception to the rule: it has a size
(360 by 190) and a `MODEMSTS.PIC` (361 by 192) exists (U which is drawn).

Draw records, found through relocated pointers to import thunks. Offsets are
within the record:

| Draw | Fields |
| --- | --- |
| `_DrawAction` (24 bytes) | +4 x, +6 y, +17 action id, +18 width, +20 label pointer. Id 1 is the default (blue) button, 2 is Cancel, 0 any other. A label pointing at an import thunk is `_okString` (OK) or `_cancelString` (Cancel); otherwise it is a string. Width 0 leaves the width to the toolkit (`CALLSIGN`, `EDITSIGN`, the `MC_*` dialogs; the value is not established, U) |
| `_DrawListBox` | +4 x, +6 y, +8 width in pixels, +16 **row count** (visible rows). `NEWNET` 4 (confirmed by four rows on the screenshot, M), `NETNEW` and `NETJOIN` 8, `MODLIST` and `COMLIST` 10, `CALLSIGN` 14 |
| `_DrawRocker` | +4 x, +6 y. The vertical PREV/NEXT rocker that pages the list. Position (0, 0) on `MODLIST` and `COMLIST` means the toolkit places it |
| `_DrawText` (18 to 22 bytes) | +4 label pointer (zero when the game fills the words in), +12 x, +14 y, +16 a 16 bit value of unknown meaning: 9 on a text that another record follows, 10 on the last text of a dialog, 3 on the text beside a check box (I: it looks like the type of the record that follows) |
| `_DrawEditBox` | +4 x, +6 y, +8 a value, 40 on every network prompt and 25 on the pilot name (`MC_NAME`). The edit routine sizes the field as ten pixels a character plus 16 and 24 high (S, `0x48c710`), so the value is probably the longest entry (I; but 40 characters would not fit `NETEDT`'s 295 pixel panel, so this is U) |
| `_DrawCheck` | +4 x, +6 y (repeated at +8, +10), +12, +14 the size, 19 by 19 |
| `_DrawSliderHoriz`, `_DrawSliderVert`, `_DrawDial` | Not decoded (`MC_DLG` holds both sliders, `GRAFPREF` and `LOADORD` the dial) |

Texts and inline words that are not in the DLG are drawn by the screen's own
code (the labels of `NEWNET`, the list rows, the status words).

Decoded dialogs (origin, size or picture, controls in file order; all rows R):

| Dialog | Origin | Size or picture | Controls |
| --- | --- | --- | --- |
| `NEWNET` | 10, 80 | 619 by 395 | New (96, 339, default), Join (219, 339), Options (342, 339), Cancel (465, 339), all 85 wide; list (38, 105) 200 wide, 4 rows; rocker (270, 120) |
| `NETNEW` (host's players) | 10, 100 | 619 by 375 | Start (138, 319, default), Reject (282, 319), Cancel (426, 319), 85 wide; list (35, 46) 549 wide, 8 rows |
| `NETJOIN` (joiner's players) | 10, 100 | 619 by 375 | Cancel (267, 319); list (35, 46) 549 wide, 8 rows |
| `NETTCP` | 89, 80 | 465 by 395 | OK (113, 339), Default (208, 339), Cancel (303, 339), 85 wide |
| `NETIPX2` | 89, 80 | 465 by 275 | OK (113, 219), Default (208, 219), Cancel (303, 219) |
| `NETCEDT` (message prompt) | 10, 200 | 620 by 124 | OK (60, 75), Cancel (155, 75); edit box (56, 20) |
| `NETEDT`, `NETBEDT` | 172, 200 and 60, 200 | 295 and 514 by 124 | OK (60, 75), Cancel (150, 75) and (155, 75); edit box (20, 40) and (42, 40) |
| `EDITSIGN` (callsign prompt) | 168, 148 | 303 by 125 | OK (66, 90), Cancel (200, 90); edit box (25, 20); text "Enter your callsign:" (38, 20) |
| `CALLSIGN` (callsign list) | 230, 80 | 220 by 370 | OK (52, 337), Cancel (120, 337); list (30, 35) 160 wide, 14 rows; rocker (160, 295); text "Choose your callsign." (43, 10) |
| `MODEM` | 80, 84 | picture `MODEM` | Call (335, 322, default), Answer (335, 287), Cancel (335, 252), 118 wide; texts "Player name" (106, 32) and "Phone number" (286, 32) |
| `MODEMCOM` | 86, 96 | picture `MODEMCOM` | OK (253, 300), Cancel (348, 300); eight texts: "AT command strings:" (39, 90), "Initialization:" (66, 110), "Dial prefix:" (66, 176), "Listen for caller prefix:" (66, 210), "Hangup prefix:" (66, 244), "Dial suffix:" (243, 176), "Listen for caller suffix:" (243, 210), "Hangup suffix:" (243, 244) |
| `MODEMSTS` | 140, 174 | 360 by 190 | text "Connection status" (135, 35) |
| `MODLIST`, `COMLIST` | 138, 125 and 170, 125 | 364 and 300 by 270 | OK (32, 237), Cancel (127, 237); list (20, 20) 324 and 260 wide, 10 rows; rocker |
| `SERIAL` | 139, 98 | picture `SERIAL` | Call (53, 293, default), Answer (152, 293), Cancel (247, 293), 85 wide; text "Connection status" (135, 184) |
| `COM` | 146, 150 | picture `COM` | OK (225, 189), Cancel (225, 154) |
| `NETIPX`, `NETDIR` | 140, 92 and 89, 80 | pictures `NETIPX`, `NETDIR` | Left over from the earlier game; the 1.02F executable never names them (S) |

The mission-setting dialogs `MC_*` (the host's, listed in the
[spec](../spec/multiplayer.md#the-hosts-mission-setting-dialogs)) are all sized
panels of 220 by 220 (the single-list ones), 220 by 380 (`MC_NAT2`), 250 by 190
(`MC_SCR`), 380 by 95 (`MC_NAME`), 520 by 235 and 620 by 404 (`MC_NAT`,
`MC_NATF`, with check boxes), and `MC_DLG`, a picture (`MC`) at (0, 77) with OK
(482, 373) and Cancel (563, 373), 75 wide, two sliders and a rocker. All decode
with the reader.

### The generic panel (S, M)

The routine at `0x487e90` takes (x, y, w, h) and builds the panel from six
pieces plus a fill, all drawn in the screen's own embedded palette (never the
flight palette). On `NEWNET` the rectangle is the DLG header's (10, 80, 619 by
395) (M, whole-pixel scan against the registered screenshot).

1. **Fill.** `PANEL.PIC` (640 by 480, grey scratched texture, almost flat: palette
   entries 73 and 81) blitted into the rectangle. It fits whole; whether it tiles
   for a larger panel is not established (U).
2. **Corners.** `EDGETL` (31 by 35) at (x, y), `EDGETR` (30 by 35) at
   (x + w - 30, y), `EDGEBL` (31 by 35) at (x, y + h - 35), `EDGEBR` (30 by 35)
   at (x + w - 30, y + h - 35). Only `EDGETL` and `EDGEBR` carry a bolt head,
   which matches the screenshot (bolts at the top left and bottom right).
3. **Edges.** `EDGETB` (47 by 4): its top two rows tile along the top edge from
   x + 31 to x + w - 31 and its bottom two rows along the bottom edge (at
   y + h - 2). `EDGELR` (4 by 26): its left two columns tile down the left edge
   from y + 35 to y + h - 34 and its right two columns down the right edge, in
   25 pixel steps. The frame is two pixels thick.

The screenshot matches this recipe by eye and by a per-region difference of 2.9
grey levels over the panel (the remainder is resampling blur).

### What `NEWNET` adds on top of the panel (S, M)

Everything else on the NETWORK CONNECTION screen is drawn by the screen function
at `0x492740`. Coordinates are absolute on the 640 by 480 canvas; `(x, y)` of a
text is where its glyph strip starts.

| Item | Rectangle or position | Notes |
| --- | --- | --- |
| Background | `NETIPX3.PIC`, no panel | Its own 256 colour palette |
| Menu bar | "?" at (84, 40) | `MULTI.MNU`: `?` and `Exit to Windows [Alt-F4]` (R) |
| Title | `PANELFNT`, centred in the dialog, top at y 87 (x 255 for "TCP/IP Network connection", 128 wide) | The text is `%s Network connection` with `TCP/IP` or `IPX/SPX`; y is the dialog's y + 7 (S, `0x493db0`) |
| Inner frame | outline (30, 100) to (608, 454), one pixel | The dialog inset by 20 on every side (S, `0x493d60`); colour is palette entry 94 (value 198) (S) |
| "Callsign:" | `PANELFNT` at (45, 110) | x is the dialog's x + 35, y its y + 30 (S) |
| Callsign field | (88, 108), 139 by 13, flat fill (grey 97) | The game's inline field markup `Callsign:  .button` + 69 spaces + `..button`: 69 spaces of `PANELFNT` are 138 pixels. Empty, it is a flat lighter bar without the `EDIT*` pieces. Its text font while typing is not established (U; next step in "Unknowns") |
| "Games" | `PANELFNT` at (45, 165) | |
| Games box | outline (45, 180) to (313, 284), 269 by 105 | One pixel, colour as the inner frame; no fill |
| List rows | 4 rows at (48, 185 + 18 i), 200 wide | Row bar `LISTLFT` (30 by 17), `LISTMID` (20 by 17) tiled, `LISTRT` (30 by 17); row pitch 18 pixels (S, add `0x12` per row at `0x48afc7`, and M). Row text is drawn in `SMLFONT` (S, `0x48b233`); the selected row is painted with `LISTHI` (24 by 12 striped) 12 pixels high, starting 4 pixels in (S) |
| Rocker | `ROCKER02` at (280, 200) | The idle frame; the DLG position (270, 120) plus the origin |
| "PREV", "NEXT" | `PANELFNT` at (252, 201) and (252, 224) | `PANELFND` (the dim face) when disabled (S) |
| "PAGE" | `PANELFNT` at (91, 265) | Drawn by the list widget (S, `0x48b02e`) |
| Page box | `PAGEBOX` (50 by 17) at (120, 261) | Drawn at (x - 1, y - 4) of the counter's origin (S, `0x48b294`) |
| Page counter | `SMLFONT`, at y 264 | `"%d "` (the current page; no trailing space from 10 up) then `" of  %d"` (two spaces before the count; one from 10 up) joined into one string; the string starts at the counter's x + 15 minus the width of the first part, so the current page number is right aligned and "1  of  0" starts at x 129 (S, `0x48a7d0`, and M). The dark fill is palette entry 118 (value 12) |
| "Players" | `PANELFNT` at (340, 165) | |
| Players box | outline (340, 180) to (593, 284), 254 by 105 | Filled flat grey 81 (nearest palette entry 109) (M) |
| "Messages" | `PANELFNT` at (45, 304) | The retail heading line also holds a Send Message button (the string `Messages` + 25 spaces + `.button Send Message ..button`, S); it is not on the screenshot, taken before any connection (U when it shows) |
| Messages area | outline (45, 319) to (593, 406), 549 by 88, flat grey 81 | Eight lines of `PANELFNT` at its line height of 10 pixels plus 8 (S, `0x47f100`: height = 8 lines + 8); x = dialog x + 35, width = dialog width - 70 |
| Buttons | New face (106, 416) with the default cap `ACTDFLT` (20 by 27) at (86, 416); Join (229, 419), Options (352, 419), Cancel (475, 419); 85 wide | The DLG positions plus the origin. Retail and `Menu::action_button` draw the default button three pixels higher with its cap to the left; the multiplayer screens draw its face and the outline rows on top of its pieces where retail does, but of the cap only its first four columns (the outline's left side) 6 pixels left of the face and its last two (the face's own left rim) 2 pixels left of it, so the outline closes round the button about four pixels off its edge and the striped box is gone (*opinionated*, John, 2026-10-05) |
| Button labels | New in `FONTDFT` (bluish glyphs, brightest pixel about 223), the others in `FONTACT` (greenish, about 215) | Centred in the face: x = button x + (73 - text width) / 2 rounded down (the default button's label 1 pixel further left); glyph strip top at y + 6 (default: face y + 10) (M, fit; the two fonts score 24.2 and 24.4 against the next best 27 to 30, and the glyph colours settle it) |

A render of the whole screen made only from these numbers, beside the
screenshot, is kept with the lead's notes (`.local/mp-notes/stage-ef/ef0/`).

### Fonts (S, M)

| Font (all `FA_1.LIB` PIC glyph strips, 256 records) | Height | Draws |
| --- | --- | --- |
| `PANELFNT` (`PANELFND` the dim copy) | 10, line height 10 | Panel title, labels, headings, "PAGE", "PREV", "NEXT", the Messages area; each connection function loads `PANELFNT.PIC` (S: `0x49284e`, `0x4931a3`, `0x4937ec` and others). Widths: "TCP/IP Network connection" 128, "Callsign:" 40, "Games" 28, "Players" 34, "Messages" 43, all matching the screenshot to a pixel |
| `SMLFONT` | 12 | List row text and the page counter |
| `FONTACT` (`FONTACD` dim), `FONTDFT` (`FONTDFD` dim) | 12 | Button labels, normal and default |
| `WHEELFNT` | 14, ten pixels a glyph (monospaced) | Text in the `EDIT*` fields (S: `0x48c78a` loads it in the edit routine; the caret moves 10 pixels a character, `0x48bec0`). Kept by the import since EF1 (it was not in the survey's list) |
| `MPFONT` | 9 | The connected-state status window (below) |
| `PANLFNT2`, `FONT4X6`, `MFONT320` | | Present in `FA_1.LIB`, not used by these screens (U) |

### Widget pieces (R, S)

- **Edit field** `EDITL` (8 by 24), `EDITM` (20 by 24, tiled), `EDITR` (8 by 24):
  a recessed dark bar with a lighter red half, 24 high; the field is the
  entry count times 10 plus 16 wide (S). The caret is a white (palette 10)
  vertical line 12 high at the field's x + 8 + 10 times the cursor index, 5
  below its top; it is erased by blitting a one pixel slice of `EDITM`.
  *The game's screens do not draw it* (John, 2026-10-01: no red and grey text
  areas): every field the player types in on a multiplayer screen is the
  flat grey box NEWNET's Callsign field is (grey 97, `PANELFNT` text, a one
  pixel white caret 12 high, 13 pixels high for the callsign and 18 for the
  other fields), and the kit does not carry `EDITL/M/R` or `WHEELFNT`.
- **List** `LISTLFT` (30 by 17), `LISTMID` (20 by 17), `LISTRT` (30 by 17),
  `LISTHI` (24 by 12): see the table above. The `LISTMID` rows are: row 0 a
  light edge (73), rows 1 and 2 dark, rows 3 to 13 near black (12), rows 14 to 16
  a light bevel.
- **Check box** `CHECK00` to `CHECK06` (28 to 33 square). Frame order (S, `0x48b320`):
  `CHECK00` is the box off at rest, `CHECK06` the box on at rest. When the
  animation is on, switching on plays `CHECK01` to `CHECK06` and switching off plays
  `CHECK05` back to `CHECK00`, one frame per screen update; with the animation off
  only `CHECK00` or `CHECK06` is drawn. So 00 to 03 are unlit and 04 to 06 lit amber
  because the lamp warms up and cools down through the middle frames. `CHECK320`
  (48 by 12) is the 320 by 200 mode's version.
- **Rockers** `ROCKER00` to `ROCKER04` vertical (27 by 40 with the shadow), the
  idle frame `ROCKER02`; `ROCKERH0` to `ROCKERH4` horizontal (39 by 29), unused by the
  screens here (S: the code builds `ROCKERH%d`, U where it is used).
- **Buttons** `ACTION0L/M/R` green, `ACTDFT0L/M/R` blue default,
  `ACTIOD0L/M/R` and `ACTDFD0L/M/R` the disabled copies, `ACTDFLT`/`ACTDFLD` the
  20 by 27 cap. The disabled default pieces are needed wherever Start or Call
  can be unavailable. The default pieces are 33 rows tall: the top three are a
  navy outline and a grey gap above the face. The widget kit draws them, and of
  the cap (a frame round a striped box) only its first four columns (a navy
  line, a grey border and two clear columns that carry the top and bottom lines
  across) 6 pixels left of the face, and its last two (a grey border and a dark
  navy line, the face's left rim) 2 pixels left of it. The outline stands about
  four pixels off the button's edge, as the top line's end does beyond the
  shadow on the right. The striped box in between is not drawn (*opinionated*,
  John, 2026-10-05; he had the outline taken out, put back, then asked for the
  rim and the spacing).
- **Ghosted text** is drawn 1.275 times as bright as the first widgets drew it
  (*opinionated*, John, 2026-10-05: first 50 percent brighter, then 15 percent
  darker than that): an empty field's hint (tint 150 to 191), a disabled
  field's text (120 to 153), a dimmed list row (118 to 150) and the disabled
  button labels (`FONTACD`, `FONTDFD`, lifted by the same factor but no
  brighter than the live labels' body, 206). `PANELFND` (dim PREV and NEXT,
  disabled check box labels, notes) is left as retail has it: its body is
  already about 70 percent of the live font's, so more would put dimmed text
  above live text.

### What the import keeps for these screens (EF1, 2026-10-01)

Implementation mode. The lists are `MULTIPLAYER_ART` (`FA_1.LIB`, 35 pictures)
and `MULTIPLAYER_DATA` (`FA_2.LIB`, 26 dialogs and menus) in
`crates/tore-import/src/selection.rs`, and the game's pack check
(`crates/tore-app/src/assets.rs`) requires every name and reads each picture,
dialog and menu with the existing readers. The checked sizes below are the
installed game's; the 1.0 disc gives the same sizes, and its dialogs and menus
differ by the four timestamp bytes described above. The whole addition is
about 1.44 MB (1,438,167 bytes of resources) on a 178 MB pack, 63 resources including the two below.

| Group | Names and sizes in bytes |
| --- | --- |
| Backgrounds | `MODEM3`, `NETIPX3` (each 309,952) |
| Panel kit | `PANEL` 309,184; `EDGETL` 810; `EDGETR` 562; `EDGEBL` 552; `EDGEBR` 810; `EDGELR` 280; `EDGETB` 272 |
| Lists, fields | `LISTLFT` 740; `LISTMID` 484; `LISTRT` 756; `LISTHI` 400; `EDITL` 506; `EDITM` 640; `EDITR` 506; `PAGEBOX` 1,092 |
| Check boxes | `CHECK00` 1,234; `CHECK01` 1,138; `CHECK02` 1,022; `CHECK03` 944; `CHECK04` 1,022; `CHECK05` 1,138; `CHECK06` 1,234 |
| Fonts | `PANELFND` 10,944; `FONTDFT` 14,432; `FONTDFD` 14,432; `MPFONT` 9,360; `WHEELFNT` 23,536 |
| Disabled default button | `ACTDFD0L` 1,146; `ACTDFD0M` 650; `ACTDFD0R` 1,300; `ACTDFLD` 1,154 |
| Status window | `MPSTATUS` 27,024 (333 by 80) |
| `MC_DLG`'s picture | `MC` 260,364 (640 by 403) |
| Network dialogs | `NEWNET`, `NETNEW`, `NETJOIN`, `NETTCP`, `NETCEDT`, `NETEDT`, `NETBEDT`, `CALLSIGN`, `EDITSIGN`, `MODEM` (`.DLG`) (each 4,608) |
| Mission-setting dialogs | `MC_DELAY`, `MC_DIST`, `MC_DLG`, `MC_KILLS`, `MC_KILLT`, `MC_LIVES`, `MC_NAME`, `MC_NAT2`, `MC_NAT`, `MC_SCR`, `MC_TIME`, `MC_WETH` (`.DLG`, each 4,608); `MC_NATF.DLG` 8,704 |
| Menus | `CHOOSEM.MNU` and `MULTI.MNU` (each 4,608); `MC_MENU.MNU` 8,704 |
| Quick messages | `TORE_CHAT_V1` (the retail `CHAT.TXT`, see below) |
| Marker | `TORE_MULTIPLAYER_V1` (see [the pack contract](../spec/import-cache.md#markers)) |

Agent decisions, 2026-10-01:

- `MC.PIC` is kept because `MC_DLG` is a size-zero dialog that draws it.
- `MC_MENU.MNU` is kept: it is the mission creator's menu bar, and its
  Multiplayer menu (Time limit, Number of kills, End scenario conditions,
  Number of revives, Revive time delay, Revive distance) is what opens the
  `MC_*` dialogs. The brief named `MC_*` without saying whether the menu counts;
  it costs 8,704 bytes.
- Left out because no screen in the design shows them: `SERIAL3` and the modem
  and serial dialog panels (`MODEM`, `MODEMCOM`, `MODEMSTS`, `SERIAL`, `COM`,
  `MODEM2`, `SERIAL2` pictures), the dialogs `SERIAL`, `MODEMCOM`, `MODEMSTS`,
  `MODLIST` and `COMLIST`, the IPX dialogs and pictures (`NETIPX2` and
  `NETIPX`, the latter left over from the earlier game), `NETDIR`, `FORTAIRB`
  (Airbase Assault, phase 2 at the earliest), the horizontal rocker `ROCKERH0`
  to `4` (no screen of ours uses it, EF0) and `CHECK320` (the 320 by 200
  mode's check box). `MODEM3` is kept because the widget kit decodes its pieces in `MODEM3`'s
  palette, though the screens no longer draw its red photograph (2026-10-05).

The game keeps these resources in `Assets::multiplayer_resources`, apart from
`theater_resources` (*agent decision*). The combat tapes fingerprint every
resource in `theater_resources`, so folding the new art into it would have made
every recorded tape of the single-player baseline refuse to replay (a changed
fingerprint, not a changed flight); kept apart, the tapes and the baseline are
unchanged and the screens read the pieces from their own map.

**`CHAT.TXT`** is a loose file in no archive. The importer reads it from the
installed folder's root (any letter case), and from the disc's installer
container (`SETUP.ESA`, the `FA_MISC` entry, 336 stored and 591 decoded bytes)
by the container reader's DCL decoder, which is cheap: it decodes that one
entry in memory (`MediaSource::loose_file`). On the retail media both give the
same 591 bytes. The pack holds the bytes as read under `TORE_CHAT_V1`
(*agent decision*: a name of its own in the `TORE_*` family, so it can never
collide with an archive resource), and `tore_formats::chat::parse` reads them.
Absent or unreadable media files are not an import failure: the import report
and the summary say "quick chat messages unavailable" and chat has no quick
messages. The reader's rules (spec: [CHAT.TXT](../spec/multiplayer.md#chattxt)):
the first twelve lines before the Ctrl-Z byte, CRLF or LF, a line cut at 159
characters, the five receiver keywords matched without regard to case (and
trimmed, *agent decision*), a last field taken as the sound when it is at most
12 characters and ends in `.5K` or `.11K` and upper-cased, text cut at 50
characters. A blank line in the middle keeps its slot so F keys stay aligned
with lines (*agent decision*; retail's handling is unknown).

### Corrections to the notes above (EF0)

- The Choose Activity menu bar is `CHOOSEM.MNU`, not `MAINMENU.MNU`: the setup
  code at `0x4a0966` pushes `CHOOSEM` (`0x502834`) for all five backgrounds (S).
  Its Multi menu holds `Serial... [e]`, `Modem... [m]`, `IPX/SPX Network... [x]`,
  `TCP/IP  Network... [t]` (two spaces), `Disconnect... [d]` and `Airbase Assault
  [a]` (R). `MAINMENU.MNU` (`?`, `Campaign` with Replay This Mission and Exit
  Campaign) has five other references and is a campaign-mode menu (I).
  "Disconnect is grayed out until connected" is the manual's; the tree has no
  state bytes (U).
- The authored Multi rows (now *Direct Connection...* and *Internet Lobby...*,
  see "Stub behavior") are scaffolding; the retail tree is the one listed here.

### Settled while building the widget kit (EF2, 2026-10-01)

- **PREV and NEXT on an empty list are bright (M).** John's screenshot draws
  both labels in `PANELFNT`, not the dim `PANELFND`, with "1 of 0" and no rows.
  The note above that `PANELFND` is the disabled face stays (S), but "no page to
  turn to" is not what disables it; when retail dims them is U (next step: the
  enable calls around `0x48b02e`). The kit dims PREV on the first page and NEXT
  on the last page of a list that has rows, and leaves both bright when it is
  empty.
- **`LISTHI` is two 12 by 12 stripes side by side (R):** blue and white on the
  left half, gold and black on the right. Which one retail paints on the selected
  row, and whether it is a marker or a bar, is U. The kit puts the gold half as a
  marker 4 pixels in from the row's left edge, 1 below its top, with the row text
  starting at 20 (agent decision; the creator's selector list marks its current
  row gold too).
- **The kit's render of NETWORK CONNECTION** (every widget placed at the
  rectangles in the table above, nothing else) is the same picture as EF0's
  render of those numbers: none of the 307,200 pixels differs. It therefore scores
  as EF0's does against the screenshot: a mean grey difference of 2.9 over the
  panel and 4.5 over the whole screen below the title bar, after a one pixel blur
  of the render (the rest is the screenshot's resampling).

### The Direct Connection screen as built (EF7, 2026-10-01)

Implementation mode, from the rectangles above. The screen is
`crates/tore-app/src/direct_screen/`; its behaviour is in
[the architecture notes](../ARCHITECTURE.md#the-direct-connection-screen-as-built-ef7).
Everything on it is made of the widget kit at NEWNET's rectangles on `NETIPX3`: its grey photograph under its own title bar, its top 77 rows (John, 2026-10-05; from 2026-10-01 to then the screens drew `MODEM3`'s red photograph under `NETIPX3`'s bar, which the pictures' identical rows 0 to 76 allowed). The bar's retail lettering NETWORK CONNECTION and its shadow (lettering x 80 to 350, y 7 to 28, shadow x 83 to 354, y 9 to 32 on the imported picture; the help bar starts at row 36) are covered by a copy of the bar's own texture from the clean strip to their right (columns 364 to 639, the same rows, repeated; the texture's mean is 79 to 82 in every row, so no seam shows), and the lettering DIRECT NETWORK CONNECTION is drawn over that: `assets/direct-network-connection-title.png`, 564 by 36, the words in Liberation Sans 2.1.5 (SIL OFL 1.1) over a sharp dark copy offset 2 pixels right and down, on a transparent background with nothing under the clear pixels (made by `tools/build_title_lettering.py`: 29 points drawn 4.9 percent narrower, fitted to the Helvetica lettering John first supplied, 439 by 21 pixels), fixed to the bar's top right so its ink starts at x 80 where the retail lettering did. A player can use other words: the top 36 or 77 rows of `DirectNetworkConnection.png` in the data folder, a PNG at most 640 wide and fixed to the same corner, replace the shipped lettering with their transparency honoured (a whole 640 by 480 picture is fine), and a missing or unusable file leaves the shipped one (the log says why). That file is the player's own and never part of the repository or a package when it carries retail's badge or texture. The Internet Lobby screen (slice I4) has the same bar lettered INTERNET LOBBY (`assets/internet-lobby-title.png`, made by the same script with `--title internet`) and its own player file, `InternetLobby.png`, read the same way. The widgets' pieces keep `MODEM3`'s palette. With these placements
(agent decisions where the table above has no retail counterpart):

| Item | Rectangle or position | Notes |
| --- | --- | --- |
| Background, panel, title, frame, headings | as the NEWNET table | The panel title still reads "TCP/IP Network connection" |
| Callsign | field (88, 108), 139 by 13 | Flat `PANELFNT` bar; hint "your callsign" when empty |
| Connect to | label (45, 139); grey field at (110, 136), 216 by 18 | An address or a name with an optional port; hint "host or address" |
| Show full games | `CHECK0n` lamp at (330, 130) with its label | Kept in the settings; off hides full games |
| Games | list at (48, 185), 200 wide, four rows; rocker, PREV/NEXT, PAGE and the page box as NEWNET | Columns from the row's text origin: lock (centred, 11 wide), name (x 14, 84 wide), players over capacity (x 100, 22 wide, right aligned), state (x 126, 46 wide: Lobby, Flying, Closed, or `vVERSION` for another build) |
| Mission line | `PANELFNT` at (45, 290), up to 549 wide | The selected game's mission summary, or why the game cannot be joined; between the Games box (ends y 285) and the Messages heading (y 304) |
| Players | list at (346, 185), 242 wide, five rows, no pager | Crown, then the callsign |
| Messages | NEWNET's box, (45, 319), 549 by 88 | Grey system lines in the kit's colours |
| Buttons | New, Join, Options, Cancel at NEWNET's positions | The blue default face (its outline kept all round, the striped box removed, John 2026-10-05) is on Join once a game is selected or an address typed, on New otherwise; Cancel reads *Leave* while a session runs |
| Options panel | panel (80, 100), 480 by 360; grey fields at (240, 139) 60 wide (port), (240, 169) and (240, 199) 216 wide, all 18 high; quick messages box (115, 262), 410 by 128; OK (190, 408) and Cancel (330, 408) | Port (five digits), password (asterisks), game name; the box shows `F1  text  (to all)` lines |

Headless renders, `--snapshot-state` `direct` (empty, searching), `direct-games`
(six games on two pages, the last selected, with its players), `direct-trying`
(an address being tried), `direct-refused` (refusals in Messages) and
`direct-options`, are in the lead's notes (`.local/mp-notes/stage-ef/ef7/`),
beside EF2's mock.

### Sharp text (2026-10-05)

The menu is a 640 by 480 canvas the renderer stretches to the window with
nearest pixels, so the retail bitmap fonts (capitals 8 to 9 pixels tall) break
into uneven blocks at full screen. While the game draws a multiplayer screen
for its window, the screen's text calls (`ui_text::text`) are recorded, not
drawn into the canvas, and the renderer draws the glyphs at the window's own
resolution over the stretched canvas, in the same render pass
(`ui_text_renderer.rs`, `ui_text.wgsl`). Headless snapshots, the previews and
the tests do not record, so they still show the retail fonts. John asked for
this on 2026-10-05: easier to read, the same size, not fuzzy (*opinionated*).

- **Shapes.** Noto Sans Medium, open licensed and not retail, rendered at 48
  pixels to the em into `crates/tore-app/assets/ui-text.bin` by
  `tools/build_ui_text_atlas.py` (138 glyphs, ASCII and the CP437 letters
  above 0x7F). The GPU keeps three mipmap levels.
- **Size and place.** Each retail font the screens use (`PANELFNT`,
  `PANELFND`, `SMLFONT`, the four button fonts) is measured on its capital H:
  the sharp capitals are that tall on that baseline, narrowed to the retail
  letters' narrowness (about 0.76 for `PANELFNT`), in the font's brightest
  colour times 0.94, times the call's tint. A run starts where the retail text
  would and ends where it would: the face's own letter spacing is spread (at
  most 1 pixel tighter or 1.5 looser a letter) so the retail widths, which the
  screens centre, align and cut off with, still hold. The retail letters sit in
  near fixed width cells that look gappy in another face. Typed text
  (`text_cells`) keeps every glyph in its retail cell so the caret, which a
  field places from the retail widths, stays between the letters.
- **Icons.** The marks in list rows (padlock, crown, ready tick, house, the
  player's own arrow, the unable cross, the Windows, macOS and Linux
  platform marks and the relay mark of slice J6, two dots joined through a hub) are hand drawn minimalist SVGs in one colour
  (`assets/icons/*.svg`, John, 2026-10-05, *opinionated*), baked at 64 by 64 into
  the same atlas and drawn the same way: recorded and drawn sharp over the
  canvas, 12 pixels square, in one light grey (a dimmed row's tint darkens
  them, a row's own colour does not). Without the sharp layer the atlas's
  picture is averaged down to 12 by 12 into the canvas. They replace the first
  set of pixel pictures in six colours.
- **Pop-ups.** The text is on top of the canvas, so a panel drawn over earlier
  text (Options, Kick, Leave) records its rectangle (`draw_panel`) and the
  earlier glyphs are cut to what shows round it. A screen's backdrop is drawn
  once and kept, so its text is kept and replayed with it.
- **Checks.** `ui_text_renderer::tests` (ignored, needs a GPU adapter) draws
  recorded text through the real pipeline into an offscreen target and compares
  it with `ui_text::composite`, the CPU stand-in that the review pictures use:
  `cargo test -p tore-app --locked ui_text -- --ignored --nocapture`.
  `TORE_DATA_DIR=... TORE_MOCK_OUT=... cargo test -p tore-app --locked
  ui_text::tests::render_review_pictures -- --ignored` writes every screen at
  3 times size, sharp and as before.

### The lobby screen as built (EF8, 2026-10-01)

Implementation mode. The screen is `crates/tore-app/src/lobby_screen/`; its
behaviour, who may press which button and what was measured are in
[the architecture notes](../ARCHITECTURE.md#the-lobby-screen-as-built-ef8). It
reuses Direct Connection's background (`NETIPX3` and the player's title bar picture, if any),
panel (10, 80, 619 by 395), frame lines and grey text fields, with these
placements (all agent decisions; retail's `NETNEW` is a reference, not a
template):

| Item | Rectangle or position | Notes |
| --- | --- | --- |
| Title | "Lobby" centred on the panel at y 87 | |
| Game, Mission, start rule, rules | `PANELFNT` lines at (45, 102), (45, 115), (45, 128) and (45, 141), up to 549 wide; the start rule in `PANELFND` | The game's name, the mission's summary, the start rule in words and, since F2-L, "Rules: " and the King's settings in words (they were at 106, 120, 134 before) |
| Slots | heading (45, 152); list (45, 168), 286 wide, five rows; frame (40, 164, 355, 97) | Columns from the text origin: own mark (centred, 11 wide, x 0), "Wing 1 #3" (x 14, 54), aircraft (x 70, 84), holder, AI, "Closed (AI)" or "Reserved: Hawk" (x 156, 100), ready tick (x 258, 12) (F2-L widened the holder column; EF8 had the aircraft at x 70, 104, the holder at x 176, 62 and the tick at x 242). Pager: PREV (336, 184), NEXT (336, 207), rocker (364, 183), PAGE (336, 226), page box (336, 240) |
| Players | heading (400, 152); list (404, 168), 186 wide, five rows, no pager; grey box (400, 165, 194, 95) | Columns (J6): crown (x 0, 12), house (x 13, 12), ready tick or red cross (x 26, 12), platform (x 39, 12), relay mark (x 52, 12, a relayed player only), callsign (x 66, 60), state word (x 128, 40). The hint line under the lists says how the selected player connected, or its reason when it cannot play the mission |
| Hint line | `PANELFNT` at (45, 266), up to 549 wide | |
| Messages | heading (45, 282); EF6's box (45, 294), 549 by 78 (seven lines) | |
| Chat line | grey box (45, 377), 549 by 18 | Hint "type a message, Enter sends to all"; 80 characters |
| Buttons | y 419, 75 wide, x 45, 124, 203, 282, 361, 440, 519 (a 79 pitch; EF8 had six 85 wide at 45 to 510) | King: Mission..., Settings..., Players..., Loadout, Ready, Fly, Leave. Others: Settings... (282), Loadout (361), Ready (440), Leave (519). While the mission flies Loadout reads Watch (Stop Watch while watching). Kick moved into the Players panel |
| Settings panel | panel (45, 100), 550 by 362, over the screen | Title; four page buttons at y 132 (x 67, 159, 251, 343, 88 wide); rows from y 166 on a 21 pitch, the label at x 69 and the value box (x 295, 270 wide, 18 high; the Game page's name and password are grey text lines there); Realism is two columns of eight (labels x 67 and 329, 160 wide, value boxes 88 wide); the notice or the selected row's reason at y 400; Close (278, 422) |
| Players panel | panel (110, 150), 420 by 180 (Kick's place) | Two lines of words; Give crown (150, 282, 105 wide), Kick... (270, 282) and Close (370, 282, blue, the keyboard) |
| Kick and Leave panels | panel (110, 150), 420 by 180, over the screen | Kick: a grey line (132, 234), 376 by 18, for the reason (60 characters); Kick (210, 282) and Cancel (345, 282). Leave: Leave (210, 270) and Cancel (345, 270), Cancel has the blue face and the keyboard |

New pixel icons (authored, no retail art): the house (9 by 8), the player's own
arrow (7 by 7) and the unable cross (7 by 7), beside the crown, lock and ready
tick of EF2.

**The creator in Accept mode** (the King's Mission...) is the creator page of
this file's Quick Mission notes with one change of label and one of lock:
OK (387, 419) reads **Accept**, Start (field 33) and its airport (34) cannot be
changed (a click, left or right, says "Multiplayer: everyone starts airborne,
so Start is locked to Airborne. Accept sends this mission to the lobby."), and
the notice box sits at the lower left (30, 438 less its height), 340 wide and
up to three lines, so it does not cover the Start line. **Load Ordnance in
lobby mode** reads **Accept** on Fly (493, 414) and **Cancel** on Select Plane
(363, 414), and its menu's Cheat row reads "Cheat  Off (not allowed)". Single
player's two pages draw and behave as before.

Headless renders, `--snapshot-state` `lobby-king`, `lobby-joiner`,
`lobby-unable`, `lobby-flying`, `lobby-server`, `lobby-kick`, `lobby-leave` and
`lobby-ready`, and (F2-L) `lobby-settings`, `lobby-settings-revival`,
`lobby-settings-scoring`, `lobby-settings-realism`, `lobby-settings-joiner`,
`lobby-settings-pvp`, `lobby-settings-flying`, `lobby-players`,
`lobby-players-house`, `lobby-locks`, `lobby-watch` and `lobby-pvp`, and (J6)
`lobby-relay` (a relayed player selected: the relay mark and the hint line's
path) and `lobby-away` (a slot reading "AI (Goose away)"), and (L4, renamed `lobby-gaps` in L5) `lobby-gaps` (a
player selected, its system in the hint line, and the Messages line about how a
second player's game differs from the host's), and with
`--quick-mission` `lobby-creator`, `lobby-creator-refused`, `lobby-ordnance`,
`lobby-ordnance-refused` and `lobby-ordnance-cheat` (in the lead's notes) and, from L4,
`lobby-creator-gaps` (the aircraft list with items not everyone has dimmed),
`lobby-creator-gap-notice` (a choice of one refused, with the host's words in the
notice), `lobby-creator-gap-theaters` and `lobby-ordnance-gaps` (weapon cards darkened, the
first one refused)
(`.local/mp-notes/stage-ef/ef8/`).
