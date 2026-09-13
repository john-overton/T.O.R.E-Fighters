# Desktop flight controls and Escape menu

The F/A-18D cockpit now covers the full flight canvas. The world renders behind transparent cockpit artwork and independently toggled instrument windows. There is no half-height viewport or opaque lower PANEL fill. Menus retain the proportional 640×480 canvas. This is still a development flight adapter, not accepted native flight/system parity.

Start with `cargo run --locked -p tore-app -- --free-flight`, or Choose Activity → Create Quick Mission → Free Flight. Free flight skips loadout and starts with clean external stations. On a MacBook, use **Fn/Globe with the function keys** when macOS assigns those keys to system actions. Fn-Up/Down supplies PageUp/PageDown on compact keyboards. The physical US key positions are used in flight, including shifted numbers and Option combinations.

## Working flight commands

| Key | Action | Evidence/status |
| --- | --- | --- |
| Arrows | Pitch/bank; Down pulls up | Keyboard flight adapter |
| Z / X | Left/right rudder | Development mapping |
| PageUp / PageDown | Increase/decrease throttle while held | Development mapping |
| 1…9 / 0 | 10…90% / full throttle | Development mapping |
| Shift-B / E | Afterburner / engine toggle | Development mapping; afterburner requires engine and >95% throttle |
| G / F / B / H | Gear / flaps / airbrake / hook | Adapter controls; full FA keyboard table still needs verification |
| R / J | Radar / jammer | Existing system toggles; no radar detection or ECM threat simulation |
| F1 | Forward cockpit view; reset pan/zoom | FA `FMENUD.MNU` |
| F2 / F3 | Look back / up | FA menu; authored camera angles, no rear/up cockpit artwork |
| F10 | External chase view | FA menu; authored camera placement |
| Ctrl + arrows | Pan view | USNF manual; authored motion, cockpit/HUD hidden while panned |
| + / - | Zoom view | USNF manual; authored 0.5–4× projection |
| Backspace | Toggle cockpit art, retain HUD/windows | FA menu (`BS`) |
| Shift-U | Toggle HUD | Development shortcut |
| Shift-[ / Shift-] | Dim / brighten HUD | FA menu |
| Shift-0…9 | Toggle instrument windows (four large or six small) | FA menu; oldest open window is replaced |
| Comma / period | Decrease/increase scope range | USNF manual; applies to RWR if it is the last opened window, radar otherwise |
| O | Cycle the radar display mode | Development shortcut; sensor physics remain unported |
| C / Shift-C | Cycle 1×/2×/4×/8× time / select 0.5× | FA menu; fixed 120 Hz ticks, authored adapter time scaling |
| Ctrl-P | Pause/resume | FA menu |
| Escape | Open/close in-flight menu; return one level from submenus/help | FA menu/manual |
| Ctrl-Q | End mission and return to creator | FA menu; does not quit the application |
| Alt-F4 / Command-Q on macOS | Exit to desktop | FA menu / macOS app shortcut |
| F11 | Open keyboard help | Development shortcut |

These replace the earlier provisional **A/D rudder, +/- throttle, T afterburner, F2/F3 external views**. T is reserved for original target cycling. The headless simulation still uses the same deterministic state model; desktop key translation is separate.

## Instruments

**Escape → Pref → Large windows?** toggles the layout. Large is the default: four corner windows, with size and margins based on 160×156 and eight pixels at the 640×480 reference size. They anchor to the actual screen edges, including on wider displays. The initial arrangement is Systems top-left, RWR bottom-left, Radar bottom-right and Radar/Visual top-right, following the supplied large-style reference.

Small places six windows across the bottom in two groups of three. Their reference size is 96×94, with six-pixel gaps and eight-pixel outer margins. Each group anchors to its screen edge; the center gutter grows on wider displays. Its initial pages are Systems/RWR/Nav on the left and Radar/Visual/Radar/Weapons on the right. Each layout remembers its own selected pages for the session. Shift-0…9 toggles pages; opening beyond a layout's capacity replaces its oldest page. Switching layouts cancels any pending instrument click. Button hit testing uses the same scaling and rectangles as rendering.

The instrument contents retain their original 160×156 raster and are resampled directly to the flight overlay resolution. There is no intermediate 96×94 reduction, so small-window text retains source strokes on larger displays. These are fitted layouts, not recovered native placement rules. Sizes and margins scale by the smaller of width/640 and height/480. Use `--instrument-layout large` or `--instrument-layout small` for startup or repeatable GPU captures.


Shift-1 Envelope; Shift-2 Forward View; Shift-3 Other View; Shift-4 Radar/Visual; Shift-5 RWR; Shift-6 Navigation; Shift-7 Systems; Shift-8 Weapons; Shift-9 Radar; Shift-0 Radar Cross Section. Page 0 explicitly reports its unimplemented status. Empty scopes and NO TARGET are intentional in target-free flight. Temperature, oil and hydraulics remain unavailable instead of displaying fabricated healthy values.

## Recovered commands awaiting their systems

All shortcut labels present in the supplied `FMENUD.MNU` are recognized. This is **not a claim that every original desktop command or system is ported**. The complete FA non-menu key-dispatch table still needs recovery. Available source/manual commands without working systems display a short message:

| Key | Reserved original action / remaining work |
| --- | --- |
| F4 | Track target |
| F5 / F6 / F7 / F8 | Player-to-missile / wingman / target; target-to-player cameras |
| F9 / F12 | Fly-by / missile camera |
| Ctrl + view key / Alt + view key | Missile-relative / target-relative camera |
| T / Shift-T, Enter / apostrophe | Target cycling and designation |
| W / Shift-W, N, A | Waypoint selection, navigation/weapons mode, autopilot |
| I / M / Y | IR sensor, HARM seeker, radar history |
| V | Set Other View camera |
| Shift-J / Shift-K | Jettison fuel / air-to-ground stores |
| Ctrl-T / Alt-S | Target information / radio silence |
| Alt-1…9 | Wingman straight/level, break and approach directions |
| Alt-B/C/T/H/V/E/W/R/P/D | Wingman return, scope/formation/spacing, engagement, protection and disengagement |

Space is the development fire binding and currently reports unavailable; actual weapon release and the complete FA weapon/countermeasure key table remain future work. Imported weapons/ECM inventory is not an implemented combat simulation. USNF manual bindings are reference evidence pending FA-specific verification; FA menu labels take precedence.

## In-flight menu

The runtime reads **? / Control / Pref / View / Window / Cheat / Multi / Map / Pos**, including nested options, from the imported FA menu module. Arrows/Tab and Enter/Space navigate; Right opens a child menu, Left backs out or changes the top menu, and Escape backs out before closing. Mouse activation requires a matching press and release. Hover/focus remains silent.

The menu pauses flight and engine loops. Focus loss pauses and clears held controls; resume explicitly with Ctrl-P or the menu. Closing the menu preserves a pre-existing explicit/focus pause. Opening menus never advances a hidden backlog of simulation time.

Working menu actions include views, instrument windows, time/pause, cockpit, pitch ladder, HUD brightness, ending flight and exiting. Sound currently toggles effects; the original volume mixer is not implemented. Other preferences, cheats, multiplayer, map and position commands are navigable placeholders with feedback. The bottom Resume / Restart / Keyboard Shortcuts actions are documented development additions. Menus do not silently enable unsupported cheats or alter the aircraft when an unrelated modifier shortcut is pressed.

## HUD and presentation limits

The HUD uses imported `HUD11.FNT`; instrument/menu text uses `WIN11.FNT`. It shows wrapped heading, true airspeed in knots, MSL altitude, terrain-relative AGL, vertical speed in ft/min, G, throttle, afterburner and actual gear/flap/brake/hook state. The pitch ladder uses five-degree steps, dashed below zero, and the renderer's perspective/bank convention. The flight-path marker comes from current kinematic vertical speed and airspeed. No target, weapon solution or navigation waypoint is invented.

Layout, line symbology, frame scaling, pan, zoom and camera placement are authored. `~F18H.PIC` is uniformly scaled to cover the actual flight aspect ratio, showing more side artwork on wider screens and cropping only what is required to avoid stretching. Mirrors remain source flat fills. Native F18 HUD callers, HUDSYM glyph meanings, full cockpit composition, corner-speed/ILS/weapon modes and native pixel parity remain open. The HUD is forward-view only; it is hidden for external or panned views to avoid presenting aircraft attitude as camera-conformal symbology.

See [recovery details](formats/aircraft.md), [progress](progress.md), and [validation](baselines/cockpit-controls.md).


The flight overlay is independent of the fixed menu canvas and tracks the window aspect. It is composed at the physical drawable size, proportionally capped at 1920×1080 for bounded CPU/GPU work. Cockpit art fills that entire overlay; menus remain centered at their original proportions. The HUD is 15% smaller, with projection compensation keeping the pitch ladder aligned with the camera. TAS and MSL primary numbers have transparent backgrounds; nearby tape labels are suppressed instead of drawing dark backing rectangles. Static cockpit artwork and unchanged instrument rasters are cached.

`--window-size 1280x720` selects an initial logical window size for inspection (minimum 640×480). Flight captures now preserve that window's aspect and the capped overlay resolution; terrain-only captures remain 960×720. See [responsive validation](baselines/responsive-flight-ui.md).
