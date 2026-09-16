# Desktop flight controls and Escape menu

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

The F/A-18D cockpit now covers the full flight canvas. The world renders behind transparent cockpit artwork and independently toggled instrument windows. There is no half-height viewport or opaque lower PANEL fill. Menus retain the proportional 640×480 canvas. This is still a development flight adapter; [behaviour provenance](behavior-provenance.md) records which flight and system components are spec-derived, native, fitted or opinionated.

Start with `cargo run --locked -p tore-app -- --free-flight`, or Choose Activity → Create Quick Mission → OK. Free flight skips loadout and starts with clean external stations. On a MacBook, use **Fn/Globe with the function keys** when macOS assigns those keys to system actions. Fn-Up/Down supplies PageUp/PageDown on compact keyboards. The physical US key positions are used in flight, including shifted numbers and Option combinations.

## Working flight commands

| Key | Action | Evidence/status |
| --- | --- | --- |
| Arrows | Pitch/bank; Down pulls up | Keyboard flight adapter |
| Z / X | Left/right rudder | Development mapping |
| PageUp / PageDown | Increase/decrease throttle while held | Development mapping |
| 1…9 / 0 | 10…90% / full throttle | Development mapping |
| Shift-B / E | Afterburner / engine toggle | Development mapping; afterburner requires engine and >95% throttle |
| G / F / B / H | Gear / flaps / airbrake / hook | Adapter controls; full FA keyboard table still needs verification |
| R / J | Radar / jammer | Radar gates live-range contacts/locks; powered ECM applies recovered contact-probability terms; decoy behavior remains open |
| F1 | Forward cockpit view; reset pan/zoom | FA `FMENUD.MNU` |
| F2 / F3 | Look back / up | FA menu; authored angles, forward artwork projects out of view naturally |
| F10 | External chase view | FA menu; authored camera placement |
| Shift + arrows / Ctrl + arrows | Cockpit look-around; exterior orbit | Shift is a convenience alias; Ctrl has USNF manual evidence; FA-specific dispatch unverified |
| Shift + / | Recenter look/orbit without changing view or zoom | Development shortcut |
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
| Shift-T | Reverse target cycling (T / Enter now designate in live range) |
| W / Shift-W, N, A | Waypoint selection, navigation/weapons mode, autopilot |
| M | HARM seeker |
| I / Y | Original IR sensor / radar history remain unavailable; these keys invoke incoming weapon / target ECM development fixtures in `--live-fire` |
| V | Set Other View camera |
| Shift-J / Shift-K | Jettison fuel / air-to-ground stores |
| Ctrl-T / Alt-S | Target information / radio silence |
| Alt-1…9 | Wingman straight/level, break and approach directions |
| Alt-B/C/T/H/V/E/W/R/P/D | Wingman return, scope/formation/spacing, engagement, protection and disengagement |

Space now holds the selected player trigger. Semicolon selects the next PT weapon
slot; T or Enter designates an actual range target. `--live-fire` enables the
explicit PT-default test range; backslash resets its target at a suitable range
for the selected weapon. Ordinary free flight loads only the internal gun.
The complete native weapon/countermeasure dispatch remains unverified.
[Live-fire scope and approximations](baselines/live-fire.md). USNF manual bindings are reference evidence pending FA-specific verification; FA menu labels take precedence.

## In-flight menu

The runtime reads **? / Control / Pref / View / Window / Cheat / Multi / Map / Pos**, including nested options, from the imported FA menu module. Arrows/Tab and Enter/Space navigate; Right opens a child menu, Left backs out or changes the top menu, and Escape backs out before closing. Mouse activation requires a matching press and release. Hover/focus remains silent.

The menu pauses flight and engine loops. Focus loss pauses and clears held controls; resume explicitly with Ctrl-P or the menu. Closing the menu preserves a pre-existing explicit/focus pause. Opening menus never advances a hidden backlog of simulation time.

Working menu actions include views, instrument windows, time/pause, cockpit, pitch ladder, HUD brightness, ending flight and exiting. Sound currently toggles effects; the original volume mixer is not implemented. Other preferences, cheats, multiplayer, map and position commands are navigable placeholders with feedback. The bottom Resume / Restart / Keyboard Shortcuts actions are documented development additions. Menus do not silently enable unsupported cheats or alter the aircraft when an unrelated modifier shortcut is pressed.

## HUD and presentation limits

The HUD uses imported `HUD11.FNT`; instrument/menu text uses `WIN11.FNT`. It shows wrapped heading, true airspeed in knots, MSL altitude, terrain-relative AGL, vertical speed in ft/min, G, throttle, afterburner and actual gear/flap/brake/hook state. The pitch ladder uses five-degree steps, dashed below zero, and the renderer's perspective/bank convention. The flight-path marker comes from current kinematic vertical speed and airspeed. No target, weapon solution or navigation waypoint is invented.

Layout, line symbology, frame scaling, pan, zoom and camera placement are authored. `~F18H.PIC` is uniformly scaled to cover the actual flight aspect ratio, showing more side artwork on wider screens and cropping only what is required to avoid stretching. Mirrors render live rear views every visible frame. Native F18 HUD callers, HUDSYM glyph meanings, full cockpit composition, corner-speed/ILS/weapon modes and native pixel parity remain open. The HUD remains aligned with the aircraft-forward datum and pans opposite head-look with the cockpit. External views omit it.

See [recovery details](formats/aircraft.md), [progress](research/progress.md), and [validation](baselines/cockpit-controls.md).


The flight overlay is independent of the fixed menu canvas and tracks the window aspect. It is composed at the physical drawable size, proportionally capped at 1920×1080 for bounded CPU/GPU work. At centered forward view, cockpit art covers that entire overlay; menus remain centered at their original proportions. The HUD is 15% smaller, with projection compensation keeping the pitch ladder aligned with the camera. TAS and MSL primary numbers have transparent backgrounds; nearby tape labels are suppressed instead of drawing dark backing rectangles. Static cockpit artwork and unchanged instrument rasters are cached.

`--window-size 1280x720` selects an initial logical window size for inspection (minimum 640×480). Flight captures now preserve that window's aspect and the capped overlay resolution; terrain-only captures remain 960×720. See [responsive validation](baselines/responsive-flight-ui.md).

## View and smoothness clarification

F2/F3 replaced the early prototype exterior bindings when the native menu controls were recovered. They look back/up from ownship and omit the exterior mesh. The forward cockpit/HUD overlay translates opposite head-look and fades at its viewing limits; back/up views do not duplicate the forward frame behind or above the pilot. **F10 shows the aircraft from outside**; F1 restores the cockpit. The oblique developer camera remains available through `--flight-view 2` and instrument 3.

The performance pass removes the extra post-render wait, interpolates camera/aircraft/HUD poses between fixed 120 Hz ticks, and keeps live instrument GPU readbacks asynchronous. Controls still drive the same authored flight adapter; this is not a new native flight-model claim. [Measurements and diagnostics](baselines/flight-performance.md).

## Look-around and exterior orbit

Hold **Shift + arrows** (or **Ctrl + arrows**) to turn the camera at one radian/second. In the cockpit, Left/Right turn around horizontally; Up looks upward as far as overhead. Down returns toward the forward eye line and **cannot look below it**. This limit is relative to the aircraft's forward pitch, not an altitude or world-horizon constraint. In F10 exterior view, arrows orbit around the aircraft in both axes, including below it and over the poles; the aircraft stays centered at a constant distance. Vertical orbit can make the view inverted as it crosses overhead. No ground-collision constraint is added to this inspection orbit.

Release the arrow to stop moving the view; its orientation stays where you left it. **Shift + /** recenters the current camera without changing view or zoom. **F1** returns to the forward cockpit and resets look/zoom. A look arrow remains claimed until physical release even if Shift/Ctrl is released first, so a repeated key cannot unexpectedly pitch or roll the aircraft. Pause/focus loss clears held input. Shift-/ uses the physical slash key, so US keyboards may label the resulting character `?`.

Head-look rotates about the aircraft’s axes, including during banked flight.
Cockpit artwork and HUD remain pointed straight ahead relative to the aircraft.
Looking right moves both left on screen; looking left moves both right. Their
shared translation follows the projected aircraft-forward datum and never
clamps to available artwork width. The image remains flat, so horizontal look
keeps its bottom level rather than tilting it like a nearby plane. Looking up
moves it downward. Both fade from 45–65 degrees horizontal look and 35–55 degrees
upward. These are fitted presentation rules, not recovered native FA projection.
Instruments remain screen-anchored.

+/- scales artwork and HUD text/symbology with world zoom. Zoom-in crops around
the eye line; zoom-out keeps the bottom anchored and constrains artwork width to
cover the screen. The finite source art provides no rear/overhead interior.
Mirrors use their original source silhouettes with live rear views. See [sliding cockpit validation](baselines/cockpit-slide.md).

The local USNF manual's “View Panning & Zooming” section specifies Ctrl+arrows when keyboard flight control is used, and Right Shift plus joystick for joystick panning. The reference app chose Shift+arrows. The supplied FA readme did not resolve the Anthology-specific binding, so Shift remains an explicitly documented convenience alias rather than claimed recovered FA behavior. [Validation](baselines/look-around.md).

## Flight response and vertical flight

The development adapter now carries an independent world-space velocity vector. Thrust, drag, lift and gravity accelerate that vector rather than setting it to the nose direction each tick. Pitch/roll controls have finite response, and the HUD flight-path marker uses both lateral and vertical velocity. The nose and actual travel direction can differ. These response constants are authored, not recovered native FA control laws.

Attitude rotates as an orthonormal basis and is interpolated in that basis. The old ±1.5-radian flight pitch clamp is removed; flight can pass through vertical/inverted attitudes and complete loops with sufficient energy. Cockpit head-look still cannot look below its forward eye line, this separate viewing restriction does not limit aircraft pitch. [Evidence and limitations](baselines/flight-response-sky.md).

## Exterior animations

Use **0 then Shift+B** for full throttle and afterburner, and **F10** to inspect the model. G/F/B/H animate gear/flaps/airbrake/hook continuously. Pitch/roll inputs move fitted stabilators; Z/X move fitted trailing rudders. Engine/fuel/throttle gate afterburner consistently across HUD and audio; flame length has a short visual transition. These reuse original polygons with authored hinges and schedules. [Coverage, captures and remaining work](baselines/f18-animations.md).

During a banked pull, the adapter now retains load-related nose/flight-path separation and accounts for body-yaw turn response. The velocity marker remains projected from actual velocity: coordinated AoA is below the nose, while transient sideslip appears laterally. Native AoA/control-law parity remains open. [Details and probes](baselines/banked-pull-aoa.md).

## Rafale C selection

Select Rafale C by clicking the Wing 1 aircraft name in Quick Mission, or use
`--aircraft rafale --free-flight`. The same input/release, pause and camera
bindings apply to the selected aircraft. Its PT values, cockpit artwork, DEFA
250-round inventory and engine clips come from its own retail profile. This is
the existing fixed-tick adapter using Rafale data, not a complete native flight
model. The Rafale and Hornet retain separate fitted animation rigs, as described
below. See [initial scope and evidence](baselines/rafale-quick-mission.md).

Rafale C: H reports unavailable because the recovered RAF.SH model has no hook
animation import. G/F/B and pitch/roll/rudder animate its own gear, elevons,
airbrakes, canards and rudder; motion schedules are fitted. Aircraft changes now
replace the GPU cockpit texture as well as exterior resources.

## Mirrors and uncapped presentation

Both aircraft have live center/left/right mirrors. They render the world and
ownship into one rear texture each visible frame, with reflected,
aspect-preserving crops inside the original outlines. They follow cockpit pan,
zoom and fade; hiding the cockpit hides the mirrors. Viewpoint/crops are fitted,
not recovered native optics. The render loop requests uncapped presentation;
physics stays at 120 Hz. [Implementation, checks and measurements](baselines/mirrors.md).

## Shared controller input and instrument focus

The authored [controller layer](INPUT.md) accepts gamepads, sticks, throttles,
pedals and button boxes through typed pilot commands. Standard Linux gamepads
have documented default bindings; custom profiles cover other descriptors and
platforms. Keyboard and controller inputs may be assigned together, with explicit
axis priority, throttle pickup and independent release/disconnect handling.
`--list-inputs` and `--monitor-inputs 30` run without a window or retail media.

Ctrl-Tab / Ctrl-Shift-Tab selects the next/previous existing instrument;
Ctrl-1..6 selects a slot and Ctrl-Shift-1..4 operates its stock button positions.
Selection does not move windows or alter their raster content. Unimplemented
controls remain unavailable. These shortcuts are T.O.R.E additions, not native
FA dispatch evidence. Optional rumble and input-only tick tapes are documented
in [INPUT.md](INPUT.md); [acceptance](baselines/input.md) distinguishes physical
hardware checks from synthetic and cross-compilation evidence.

Enable rumble through **Escape → Control → Rumble: On → Save & apply**. Actual
afterburner engagement produces a subtle impulse followed by a quiet continuous
low-frequency rumble while it remains active. The switch alone below the model's activation
threshold does not. Event feedback designs and future weapon/turbulence hooks
are listed in [INPUT.md](INPUT.md); successful gun/missile events now drive the corresponding rumble cues in the live-fire adapter.

The flight **Control** tab now opens the authored binding editor rather than the
retail device-selection stub. Instrument page sets/layout, scope controls, zoom,
cockpit/HUD options and sound preferences save automatically between normal
sessions and survive aircraft changes/restarts. [Full settings contract](INPUT.md).

## Manual weapons follow-up

The explicit live range adds **U** arm/safe, **L** clear designation, **K** jettison
selected external weapon group, **]** cycle damage-class fixture and **[** fail
selected station. Restart repairs/reloads. These are development bindings;
Shift/Ctrl/Alt combinations retain their prior meanings. Firing stops on weapon,
arm, jettison and fixture transitions and requires release before another press.
T/Enter cycles actual living contacts within the imported visual/radar coverage.
SAFE/EMPTY/STATION FAILED and sensor/range/terrain inhibits are shown separately
from lock. The systems continuation below adds automatic source-weighted failures
for supported equipment; combat AI remains deferred.

`--record-combat NEW_PATH` records explicit combat-service inputs and commands;
`--replay-combat PATH` replays headlessly with the same aircraft/theater/assets.
This is separate from pilot-input recording and does not re-simulate flight.
[Controls, evidence and remaining gaps](baselines/manual-weapons.md).

## Weapons and systems continuation

In the explicit `--live-fire` range, **D** requests a gun-strength player hit,
**I** spawns one incoming selected source weapon, and **Y** toggles target ECM.
These development fixtures replace the unavailable I/IR and Y/history actions;
D is also a development binding. **J** controls own ECM and **R** radar. I does
not command AI or spend player ammunition. U arm/safe, K selected-group jettison,
L clear designation, semicolon next weapon, T/Enter designate, Space hold fire,
backslash target replacement and bracket fault/class controls remain available.

Standard Linux pads use **held Select** as the combat layer: RB fire, LB weapon,
A designate, B clear, X arm, Y own ECM, L3 radar, R3 jettison. D-pad up replaces
target, down requests a player hit, left cycles class, right fails the station.
Select+Start toggles target ECM; Select+Guide spawns the incoming fixture when
the desktop exposes Guide. These suppress the corresponding base flight/menu
bindings. Release before switching layers. Unmodified Start pauses. F10/custom
`view-external` replaces Select's old default exterior action. Exact mappings,
profile migration/editor behavior, unsupported controllers and haptic limits are
in [INPUT.md](INPUT.md#manual-combat-layer-2026-09-14).

Weapons page V/R/E reports visual availability, radar and ECM: `+` available/on,
`-` off, `!` failed. Automatic source-index faults are separate from the manual
bracket injection. Unknown engine/hydraulic effects remain unimplemented.
[Recovered contracts, runtime evidence and remaining gates](baselines/weapons-systems.md).


HUD brightness now uses the recovered signed range -256..256, steps of 16 and
neutral zero, applied to the original palette entry before sunlight whitening.
The primary ink comes from each aircraft's HUD module; layout remains authored.
Version-1 preferences migrate the old setting relative to its neutral value 7;
version 2 persists the new signed amount. Other saved display choices are retained.

The main HUD shows outlined current TAS/MSL values and a curved bank scale at
the bottom (crash/engine-off alerts take priority). The scale rotates past a
fixed triangular index, with 10-degree ticks and numbers every 30 degrees.
It follows aircraft attitude, including full rolls, independently of head-look.
The readout outlines are transparent. Combat debug status and range hints are no longer overlaid on
flight; normal HUD and instrument windows remain available.

## Sun glare cheat

**Escape → Cheat → No sun whiteout?** toggles glare suppression. **On** removes
sun whitening from world/cockpit/HUD palettes and disables lens-flare circles in
all rendered views; the original sun remains visible. It applies while paused,
lasts across mission restarts in the current session and is not saved to disk.
The developer override `TORE_SUN_GLARE=0` also disables these effects.

Main, mirror and instrument camera scenes now resolve their own altitude, fog,
palette and glare from the same weather clock. Mirror rendering stays GPU-only;
camera instruments retain their asynchronous, roughly 10 Hz feed and can show
an older completed image. [Evidence and remaining work](baselines/weather-cameras.md).

### Wind/turbulence continuation

**Cheat → No turbulence?** toggles physical turbulence for this session and
survives flight restart. HUD TAS, AGL and vertical speed now use the shared
AirData sample with explicit wind, terrain and standard atmosphere when in its
supported altitude range; labels remain TAS and geometric altitude. Missing
IAS/CAS or indicated/pressure altitude are not synthesized.


## Native research flight restriction

With `--native-flight-tables DIR`, the same pilot input feeds the joined native
airborne service. Existing device animation/threshold and fuel timing are host
adaptations. The **No turbulence?** setting stays on in this mode; enabling it
reports that environmental turbulence is unavailable. Reaching terrain contact
pauses with an explicit unsupported-contact message; restart resets the native
state. That limit belongs to this research option alone: ordinary free flight has
working authored ground contact. Legacy/hybrid retain their existing controls and
behavior.
[Scope and validation](baselines/native-live-flight.md).

## Additional aircraft

The additional FA aircraft use the same controls. F-14D and A-4E accept hook
commands; X-31 does not. A-4E ignores burner commands and rejects a nonzero
burner capture fraction. F-14 visual sweep is automatic and fitted. X-31 thrust
vectoring controls remain unavailable. See [aircraft behavior](spec/additional-aircraft.md).

F-14D, A-4E and X-31 use their own FA roll response values. Below 220 ft/s,
ordinary stick and rudder also command the source low-speed auxiliary rotation;
authority depends on throttle and is removed on the ground or without power.
There is no separate X-31 nozzle key. See the
[control contract](spec/additional-aircraft.md) for numbers and fitted components.

Researched flight is now the default; `--legacy-flight` preserves the previous
model. HUD and audio share the [stall warning signal](spec/stall-warnings.md),
including the original imported warning samples.

Hybrid [spin dynamics](spec/spin-transitions.md) use continuous axis values.
Opposite rudder decelerates rotation; wrong rudder can build it. Forward stick
moves the nose proportionally, with effectiveness reduced by fast rotation and
poor airflow. There is no fixed recovery ramp. Early intervention can stop a
spin while the aircraft remains stalled. Recovery uses the 25-degree airflow
cone and normal-rudder-rate threshold; sufficiently fast, aligned flight clears
the warning. Idle throttle is supported. TAS includes sideways/downward motion.

Spin entry torque begins gently at the clean-stall boundary, increasing with
speed deficit and back-stick. No fixed rudder percentage suddenly enables full
spin torque in hybrid flight. A-4 researched flight also uses a faster fitted
roll response; see [A-4 roll tuning](spec/additional-aircraft.md#a-4-roll-tuning).
