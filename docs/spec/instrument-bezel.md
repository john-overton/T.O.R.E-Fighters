# Instrument window bezels

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-23. Describes how the original game frames each
instrument window (Envelope, RWR, Radar, Target Cam and the rest): which art is
used, where it sits, how big it is, and why its colour changes with the
aircraft being flown.

## What the player sees

- Every instrument window is framed by a bezel painted for the cockpit the
  player is flying. There is no single shared frame. An A-4E has a light grey
  frame with round corner screws and pale buttons. A MiG-21 has a speckled grey
  frame with teal corner blocks and black buttons. A Su-35 has a flat grey
  frame with cut-off top corners and no screws. A Eurofighter and an F-22 have
  darker grey frames with small corner bolts.
- The bezel is one picture per cockpit family, `~XX_P.PIC` in `FA_1.LIB`
  (for example `~F4_P.PIC`, `~M21_P.PIC`). It is 81 by 80 pixels and already
  contains everything except text: the outer frame, the title bar, the small
  square number box, the recessed screen, and the four buttons with their
  separators.
- In the four-window layout at 640x480 the bezel is drawn at double size, so
  each window is **162 by 160** pixels. The screen inset is **138 by 114**
  pixels starting at **(12, 20)** inside the window.
- The bezel colours come from the cockpit art's own 64-colour palette (palette
  indices 0 to 63), the same colours as the cockpit frame around the canopy.
  The picture carries no palette of its own. The flyable aircraft's bezels use
  only indices 0 to 46 (other cockpits in the archive reach 59).
- Text on the bezel uses three per-aircraft colours chosen by the aircraft's
  HUD file: one for the title and window number, one for button letters, and
  one for the brief highlight square shown when a button is clicked. Light
  frames get dark text, dark frames get light text.
- The title (for example `ENVELOPE`) is centred on the window in the WIN11
  font, with one blank pixel between letters. The window number sits in or at
  the small square at the top left.

## Per-aircraft bezels

The panel name is read from the aircraft's HUD file (see provenance). Colour
indices refer to that cockpit's 64-entry palette; RGB is the 8-bit expansion of
the daytime palette stored in `~XXH.PIC`.

| TORE id | PT | HUD | Bezel art | Look | Screen inset colour | Title / number | Button letters | Click highlight |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| f18 | F18.PT | F18.HUD (null pointer, see unknowns) | `~F18_P.PIC` | grey-green frame, pale buttons | 46 (4,28,4) dark green | 5 (255,255,255) | 0 (0,0,0) | 0 (0,0,0) |
| rafale | RAFALE.PT | RAFALE.HUD (null pointer) | `~RAF_P.PIC` | lilac grey frame, round corner screws, pale buttons | 0 (4,4,12) | 1 (255,255,255) | 0 (4,4,12) | 0 (4,4,12) |
| f14 | F14.PT | F14CC.HUD in PT; TORE uses F14.HUD | `~F14_P.PIC` | blue-grey frame, pale blue buttons | 0 (0,0,0) | 39 (198,226,246) | 0 (0,0,0) | 0 (0,0,0) |
| a4e | A4E.PT | F4.HUD | `~F4_P.PIC` | light grey frame, round corner screws, pale buttons | 0 (0,0,0) | 11 (72,80,89) | 11 (72,80,89) | 11 (72,80,89) |
| x31 | F31.PT | F31.HUD (null pointer) | `~F31_P.PIC` | dark violet frame, bevelled lower corners, black buttons | 0 (8,4,12) | 1 (250,250,250) | 1 (250,250,250) | 1 (250,250,250) |
| mig29 | MIG29.PT | SU33CC.HUD | `~SU33_P.PIC` | teal frame, grey button strip, black buttons | 0 (0,0,0) | 1 (198,214,214) | 1 (198,214,214) | 1 (198,214,214) |
| su27 | SU27.PT | AV8.HUD | `~AV8_P.PIC` | slate blue-grey frame, pale buttons | 0 (0,0,0) | 7 (255,255,255) | 19 (109,109,109) | 4 (149,165,178) |
| mig21 | MIG21.PT | MIG21.HUD | `~M21_P.PIC` | speckled grey frame, teal corner blocks, black buttons | 0 (0,0,0) | 7 (16,48,48) | 19 (174,186,194) | 4 (48,121,129) |
| su25 | SU25.PT | SU33CC.HUD | `~SU33_P.PIC` | as mig29 | 0 (0,0,0) | 1 | 1 | 1 |
| mig23 | MIG23.PT | SU33CC.HUD | `~SU33_P.PIC` | as mig29 | 0 (0,0,0) | 1 | 1 | 1 |
| su35 | SU35.PT | SU35.HUD (null pointer) | `~SU35_P.PIC` | flat grey frame, cut top corners, no screws, pale lilac buttons | 0 (0,0,0) | 1 (170,170,170) | 0 (0,0,0) | 0 (0,0,0) |
| f22 | F22.PT | F22.HUD | `~F22_P.PIC` | dark olive grey frame, corner bolts, pale buttons | 0 (0,0,0) | 39 (234,234,234) | 0 (0,0,0) | 0 (0,0,0) |
| f22n | F22N.PT | F22N.HUD | `~F22_P.PIC` | as f22 | 0 (0,0,0) | 39 | 0 | 0 |
| faxx | F22N.PT | F22N.HUD | `~F22_P.PIC` | as f22 (follows its F22N presentation) | 0 | 39 | 0 | 0 |

F14.HUD and F14CC.HUD carry identical panel names and colours, so TORE's
existing F14.HUD choice gives the same bezel. The MiG-21 teal highlight colour
(index 4) is the only non-grey text colour among the flyable set.

## Geometry (640x480, four windows)

All numbers are window-relative pixels unless stated. The original computes
them as a base value shifted left by a per-axis scale; the four-window layout
at 640x480 with the large-windows preference uses a scale of two on both axes.

- **Window**: 162 x 160 (base 81 x 80, the bezel picture's size).
- **Screen inset** (drawn by the page): (12, 20), 138 x 114 (base 6, 10, 69 x 57).
  This matches the RWR base extents already recorded in
  [aircraft formats](../formats/aircraft.md).
- **Title**: text centred on x = 81 with its glyph cell top at y = 6. With WIN11
  the first ink row is y = 7. If the title is wider than 98 pixels it is cut
  and `...` appended until it fits.
- **Window number**: a single digit, the page number, centred on x = 28, cell
  top y = 6, in the title colour. Page 10 shows `0`; an internal page 11 shows
  `2` (its player meaning is unknown).
- **Number box click area**: (22, 4), 14 x 14.
- **Buttons**: four click areas 30 x 26 at x = 18, 48, 78, 108 and y = 134.
  The button art itself is in the bezel picture.
- **Button letter**: one character centred on the button's x + 16, cell top at
  y + 8, in the button-letter colour.
- **Click highlight**: when a button is pressed a filled 14 x 14 square in the
  highlight colour is drawn at the button's (x + 10, y + 6) with a click sound,
  until the window is redrawn.
- **Text spacing**: every glyph advances by its font advance plus one pixel.
  Measured widths are the sum of (advance + 1) minus one. `ENVELOPE` in WIN11 is
  51 pixels wide and starts at x = 56.

### Window placement

With W and H the screen size:

- **Four windows, large** (W >= 640, H >= 480, large-windows preference on,
  scale 2): top-left (10, 14), top-right (W - 172, 14), bottom-right
  (W - 172, H - 174), bottom-left (10, H - 174). At 640x480: (10,14), (468,14),
  (468,306), (10,306). Slots are numbered top-left, top-right, bottom-right,
  bottom-left.
- **Six windows, small** (W >= 640, H >= 480, preference off, scale 1): 81 x 80
  windows in a row at y = H - 106 (374 at 480), x = 5, 91, 177 on the left and
  W - 258, W - 172, W - 86 on the right (382, 468, 554 at 640), five-pixel gaps
  and margins. The bezel is drawn at its native size and all offsets above are
  halved (screen inset 69 x 57 at (6, 10), buttons at 9, 24, 39, 54 and y = 67).
- **Four windows, small screens** (below 640x480): the four-corner arrangement
  at scale 1. One further mode with scale 1 horizontally and 2 vertically
  exists for a specific display height (see unknowns).

## Colour rule in flight

The bezel is drawn into the same 256-colour frame as the cockpit art, so its
colours are whatever the live palette holds at those indices. That means it
changes exactly as the cockpit frame does under any cockpit palette effect
(sun whitening, weather, lighting). The flyable bezels use indices 0 to 46, so
the recorded fog tint of the shared grey ramp 47 to 60 does not reach them.
The HUD brightness control only moves index 40, which no bezel uses.
This is an inference from the indexed display, not a separate measurement: all
five retail screenshots are daytime and their fitted colour gain against the
stored palette was 0.96 to 1.02.

## Evidence and provenance

- Build: `FA.EXE` 1.02F, SHA-256
  `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
  `FA_1.LIB` SHA-256
  `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f`,
  `FA_2.LIB` SHA-256
  `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198`.
- **Panel name.** Each HUD's 0x2b2-byte root (copied to 0x521360 by 0x406193,
  see [weather formats](../formats/weather.md)) holds the panel name at root
  +0x275 (`~f4_p` in F4.HUD). The window setup routine 0x438b70 loads it
  through 0x4b7cd0 with the per-axis scale and keeps the result at 0x5215ef
  (root +0x28f). The title routine 0x43a400 blits that picture at the window
  origin (0x4b7fa0) before drawing text. Native.
- **Text colours.** Root +0x2a8 is set before the title and number (0x43a421),
  +0x2a9 before each button letter (0x43a138), +0x2aa before the click
  highlight rectangle (0x43a0f4). The click call comes from 0x43a036 after the
  click sound. Native.
- **Geometry.** Window sizes and positions are written by 0x438870 (layout
  chosen by window count: 4 gives corners, otherwise the six-window row).
  Scale and window count come from 0x438b70: scale 2 and four windows need
  W >= 640, H >= 480 and preference bit 0x800000 of 0x4eb6f8; six windows need
  the same size with that bit clear. Title position, truncation width, number
  position, button rectangles (0x539d88) and letter offsets are in 0x43a400,
  0x438a02..0x438b57 and 0x43a0b0. Text centring and the advance + 1 width are
  in 0x4986b0 and 0x4989f0. Native.
- **Screenshot validation.** John's six retail screenshots (2026-09-21, build
  identity unknown) were matched against every cockpit bezel with a scale and
  offset fit, masking the screen, title and button letters. Each window matched
  exactly one bezel, with mean absolute error 1.3 to 3.6 (0 to 255 scale),
  against 8 or more for the next best:
  - `16-41-54` (gun `MK 12`): `~F4_P`, the A-4E.
  - `16-41-26` (gun `GSH-23`): `~M21_P`, MiG-21 (MIG21.PT or MIG21F.PT).
  - `16-40-56` (gun `GSH-301`): `~SU35_P`, Su-35 (SU35.PT or SU35B.PT).
  - `16-40-24` (gun `BK27`): `~EF2_P`, E2000.HUD (Eurofighter or Tornado,
    not TORE-flyable).
  - `20-23-34` and `20-23-24` (gun `M61`): `~F22_P`, F-22 or F-22N.
  The fitted scale was 2.000 bezel pixels per screen pixel on both axes in
  every window. Fitted window origins across four windows per screenshot gave
  separations of 458 x 292 screen pixels, matching the placement rule above.
  Reconstructions (bezel at double size, WIN11 title/number/letters in the HUD
  colours, advance + 1 spacing) differ from the retail crops by 6 to 10 mean
  absolute outside the screen, the residual being blur in the captured video
  scaling around text and edges.
- **EDGE and PANEL art.** `EDGETL/TR/BL/BR/LR/TB.PIC`, `PANEL.PIC`,
  `PANELFNT.PIC` and `PANELFND.PIC` belong to the generic dialog frame and
  button code (loader around 0x487eb2 next to `ACTION`, `ROTARY`, `LIGHTON` and
  list-box art). The instrument window routines never reference them. They
  are not instrument bezels.

## Unknowns and next steps

- **Null HUD pointers.** F18, RAFALE, F31 and SU35 PTs have a null HUD pointer.
  Every null-pointer PT in the archive has a same-stem `.HUD` file, the
  executable holds `f18.HUD` and `.HUD` strings, and the Su-35 screenshot shows
  the SU35.HUD bezel, but SU35B.PT also names SU35.HUD, so the screenshot does
  not settle it. Next step: trace the PT load path that consumes the `.HUD`
  suffix string at 0x4ebe7c (file offset 0xea47c).
- **F/A-18 screen inset colour.** `~F18_P` fills the screen inset with index 46
  (dark green) rather than black. Whether pages without a full background (RWR,
  Radar) clear the inset first is not established. Next step: a retail F/A-18D
  RWR screenshot, or the clear calls in the page routines near 0x43ed2f.
- **Night and weather.** No retail screenshot at night, dusk or in cloud. The
  colour rule above follows from the indexed display.
- **Scale 1 by 2 mode.** 0x438b70 selects horizontal scale 1 and vertical scale
  2 when the word at 0x55c066 exceeds 350. The player-facing mode is unknown.
- **`~XX_w` names.** Each HUD also names `~XX_w` (root +0x282); no such
  resource exists in the reviewed archives and its use is unknown.
- **Other pages' button letters.** Only Envelope (`U A C`) and the `- +`,
  `M Y` sets were confirmed from call sites; the letters for every page are
  outside this spec.

## TORE screen tint (opinionated)

**Opinionated, requested by John on 2026-09-23.** TORE clears the screen of the
green-symbology pages to a faint green, RGB (4, 18, 6), so they read as small
CRTs: RCS (0), RWR (5), NAV INFO (6), SYSTEMS (7), WEAPONS (8) and RADAR (9),
including their failed, switched-off and no-data states (agent choice: an
unlit tube still shows its tinted glass). Envelope (1) and the picture pages
FRONT VIEW, OTHER VIEW and TARGET CAM (2 to 4) keep black. Symbology colours
are unchanged.

This is not the original's colour. Sampling John's retail captures inside the
RWR and Radar screens (16-40-24, 16-40-56, 16-41-26, 16-41-54 and 20-23-34,
dark pixels only) gives exact black for most pixels, with a mean of about
(0.2, 1.5 to 2.0, 0.2); the A-4E capture reads (0, 2 to 4, 2). That residue is
green bleed from the capture's scaling around the symbology, not a screen
fill. (4, 18, 6) is the value suggested with the request, checked by eye on
zoomed crops against those captures; it is visible but dark.

## Implementation notes

What shipped in TORE (2026-09-23, `crates/tore-app/src/instruments.rs`):

1. **Frame.** Each window draws only the aircraft's frame picture at double
   size, nearest neighbour, over the whole 162 x 160 window. The procedural
   grey chrome, the `EDGE*.PIC` corners, the drawn number box and the drawn
   buttons are gone. `EDGE*.PIC` and `PANEL*.PIC` are no longer loaded with
   an aircraft or part of its import closure; `PANELFNT.PIC` stays in the menu
   import.
2. **HUD fields.** `tore_formats::hud::Hud` reads the panel name at +0x275
   (NUL-terminated within its 13 bytes) and `title_color` (+0x2a8),
   `button_color` (+0x2a9) and `press_color` (+0x2aa). The aircraft import
   closure and the cache check require `~<cockpit stem>_P.PIC` for every
   flyable aircraft; loading rejects a HUD that names any other picture and a
   frame that is not 81 x 80.
3. **Palette.** The frame is kept as palette indices and coloured every frame
   through the same live cockpit palette as the cockpit art and HUD
   (`Airframe::cockpit_palette`). In daytime captures of all fourteen flyable
   identities the frame, title, number and letters match the research
   reconstructions within one RGB level (rounding of the 6-bit expansion). In
   dawn, sunset and night captures of the A-4E the frame did not change, and
   neither did the cockpit art: both follow the one palette.
4. **Geometry.** Window 162 x 160, screen (12, 20) 138 x 114. Page content is
   drawn in screen coordinates, so every page moved by one pixel right and one
   up relative to the old (11, 21) screen. Title centred on x = 81 and number
   (page id modulo 10) centred on x = 28, both with cell top y = 6. Titles
   wider than 98 pixels are shortened with `...` (no current title needs it).
   Button click areas are 30 x 26 at x = 18, 48, 78, 108, y = 134, with no gaps
   between them; letters are centred on x + 16 at y + 8. WIN11 with one blank
   pixel between glyphs, centred with the left edge at centre minus
   (width - 1) / 2.
5. **Press square.** Fitted: the 14 x 14 square at (x + 10, y + 6) in
   `press_color` covers the button's letter while the mouse button is held on
   it, and only on buttons that have a letter. The original keeps it until the
   window is next redrawn and plays a click sound; TORE plays no click.
   Hardware-button presses show no square.
6. **Screen background.** Pages clear their screen first, so the F/A-18
   frame's dark green inset (index 46) is never seen. Envelope and picture
   pages clear to black, the others to the tint above. Whether the original
   RWR and Radar pages clear the F/A-18 inset is still unknown.
7. **Placement.** TORE keeps its fitted placement, only adapted to the new
   size: large windows at (8, 8), (8, 312), (470, 312), (470, 8) with
   eight-pixel margins at 640 x 480, and a 96 x 95 small layout at y = 377.
   The original placement above was not adopted; see
   [flight controls](../FLIGHT-CONTROLS.md#instruments).
8. **Not implemented.** The number box click area (22, 4) has no action in
   TORE; its original action is not recorded here. The six-window small
   layout at scale 1 is not reproduced.
