# Desktop flight controls and Escape menu

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

On the [F/A-XX concept](spec/fa-xx.md), normal rudder input opens the split
flap on the commanded side. F-22 bay and device controls remain available.
**H** deploys or retracts its added hook over three seconds. The hook starts
stowed and is completely hidden when retracted.

The F/A-18D cockpit now covers the full flight canvas. The world renders behind transparent cockpit artwork and independently toggled instrument windows. There is no half-height viewport or opaque lower PANEL fill. Menus retain the proportional 640×480 canvas. This is still a development flight adapter; [behaviour provenance](behavior-provenance.md) records which flight and system components are spec-derived, native, fitted or opinionated.

Start with `cargo run --locked -p tore-app -- --free-flight`, or Choose Activity → Create Quick Mission → OK. Free flight skips loadout and starts with clean external stations. On a MacBook, use **Fn/Globe with the function keys** when macOS assigns those keys to system actions. Fn-Up/Down supplies PageUp/PageDown on compact keyboards. The physical US key positions are used in flight, including shifted numbers and Option combinations.

## Working flight commands

| Key | Action | Evidence/status |
| --- | --- | --- |
| Arrows | Pitch/bank; Down pulls up | Keyboard flight adapter |
| Shift-M | Toggle live map; Escape closes it | Right-side category toggles, buildings off; [map rules](spec/flight-map.md) |
| Z / X | Left/right rudder | Development mapping |
| PageUp / PageDown | Increase/decrease throttle while held | Development mapping |
| 1…9 / 0 | 10…90% / full throttle | Development mapping |
| Shift-B / E | Afterburner / engine toggle | Development mapping; afterburner requires engine and >95% throttle |
| Shift+O | F-22 main weapon bays | Fitted 1-second presentation; other aircraft ignore it |
| G / F / B / H | Gear / flaps / airbrake / hook | Adapter controls; full FA keyboard table still needs verification |
| R / J | Radar / jammer | R returns to the radar channel when infrared is selected, and otherwise toggles radar power. Radar power gates radar contacts and locks; powered ECM applies recovered contact-probability terms; decoy behavior remains open |
| M / O | Cycle the available sensor channels | Radar and the installed infrared sensor; with no infrared installed both report it unavailable and leave radar selected |
| I | Select the infrared channel | Passive: it stops radar transmission without moving the radar power switch |
| Y | Toggle scope contact history | Draws past observations as dimming dots; see the [sensor component](radar.md) |
| F1 | Forward cockpit view; reset pan/zoom | FA `FMENUD.MNU` |
| F2 / F3 | Look back / up | FA menu; authored angles, forward artwork projects out of view naturally |
| F10 | External chase view | FA menu; authored camera placement |
| Shift + arrows / Ctrl + arrows | Cockpit look-around; exterior orbit | Shift is a convenience alias; Ctrl has USNF manual evidence; FA-specific dispatch unverified |
| Shift + / | Recenter look/orbit without changing view or zoom; also recenters a head tracker | Development shortcut |
| Hold right mouse button and drag | Mouse look, with the same limits as keyboard look | Opinionated agent choice, 2026-09-22; see [input](INPUT.md#mouse-look) |
| Head tracker (opentrack UDP 4242) | Turns the view on top of keyboard, stick and mouse look, with the same limits | See [head tracking](INPUT.md#head-tracking-and-trackir) |
| + / - | Zoom view | USNF manual; authored 0.5–4× projection |
| Backspace | Toggle cockpit art, retain HUD/windows | FA menu (`BS`) |
| Shift-U | Toggle HUD | Development shortcut |
| Shift-[ / Shift-] | Dim / brighten HUD | FA menu |
| D | Report ownship and systems damage in the sim log | Manual p. 161; requested summary |
| Shift-0…9 | Toggle instrument windows (four large or six small) | FA menu; oldest open window is replaced |
| Comma / period | Decrease/increase scope range | USNF manual; applies to the RWR or the RCS page if either is the last opened window, and to the radar scope otherwise |
| C / Shift-C | Cycle 1×/2×/4×/8× time / select 0.5× | FA menu; fixed 120 Hz ticks, authored adapter time scaling |
| A / Ctrl-A | Toggle heading/altitude hold / waypoint autopilot | [Autopilot behavior](spec/autopilot.md), requested USNF-ATF modes |
| Ctrl-P | Pause/resume | FA menu |
| Escape | Open/close in-flight menu; return one level from submenus/help | FA menu/manual |
| Ctrl-Q | End mission and return to creator | FA menu; does not quit the application |
| Alt-F4 / Command-Q on macOS | Exit to desktop | FA menu / macOS app shortcut |
| F11 | Open keyboard help | Development shortcut |
| Alt-Enter | Switch between borderless fullscreen and the previous windowed size | Opinionated, requested by John on 2026-09-22; F11 is already keyboard help, so the window mode uses Alt-Enter alone |

These replace the earlier provisional **A/D rudder, +/- throttle, T afterburner, F2/F3 external views**. T is reserved for original target cycling. The headless simulation still uses the same deterministic state model; desktop key translation is separate.

Autopilot leaves throttle manual. Stick or rudder input above 15% disengages it.
Switching modes retains the captured altitude and heading. With no waypoint
system yet, Ctrl-A holds that heading and shows `AUTO` above `WP --`. Once supplied, a waypoint is labeled `WP <number>`.
Heading hold shows `AUTO` above `HDG ALT`, beside the heading tape.

## Instruments

**Escape → Pref → Large windows?** toggles the layout. Large is the default: four corner windows, with size and margins based on the 162×160 window and eight pixels at the 640×480 reference size. They anchor to the actual screen edges, including on wider displays. The initial arrangement is Systems top-left, RWR bottom-left, Radar bottom-right and Radar/Visual top-right, following the supplied large-style reference.

Small places six windows across the bottom in two groups of three. Their reference size is 96×95, with six-pixel gaps and eight-pixel outer margins. Each group anchors to its screen edge; the center gutter grows on wider displays. Its initial pages are Systems/RWR/Nav on the left and Radar/Visual/Radar/Weapons on the right. Each layout remembers its own selected pages for the session. Shift-0…9 toggles pages; opening beyond a layout's capacity replaces its oldest page. Switching layouts cancels any pending instrument click. Button hit testing uses the same scaling and rectangles as rendering.

The Envelope window (Shift-1) uses U for the current G curve, A for all positive-G curves, and C for locked-target comparison. Red marks the target's advantage. It shows clean-aircraft capability, with live altitude, G and speed readouts and a color-cycling square. Each aircraft sets its own chart scale. Missing comparison data is labeled explicitly. See the [envelope spec](spec/envelope.md) for fitted colors, scale and marker timing.

Each window is a 162×160 raster: the flying aircraft's own original frame at double size, with its title, number and button letters in the aircraft's HUD colours ([bezel spec](spec/instrument-bezel.md)). It is resampled directly to the flight overlay resolution. There is no intermediate 96×95 reduction, so small-window text retains source strokes on larger displays. These are fitted layouts, not the original placement, which uses 10- and 14-pixel margins and an 81×80 small window ([window placement](spec/instrument-bezel.md#window-placement)). Sizes and margins scale by the smaller of width/640 and height/480. Use `--instrument-layout large` or `--instrument-layout small` for startup or repeatable GPU captures.


Shift-1 Envelope; Shift-2 Forward View; Shift-3 Other View; Shift-4 Radar/Visual; Shift-5 RWR; Shift-6 Navigation; Shift-7 Systems; Shift-8 Weapons; Shift-9 Radar; Shift-0 Radar Cross Section. Forward View draws a short horizon bar centered on the nose, the flight path marker and plain TAS and MSL readouts over the picture in the HUD's primary color, with no pitch ladder; this is an opinionated addition requested by John on 2026-09-22. The marks use the same camera angles as the picture and refresh with it, about ten times a second. Page 0 now draws the exposure contour, received emitter symbols and its view scale; its buttons are `-` and `+`. Page 9 buttons are `-`, `+`, `M` for the channel cycle and `Y` for history. Hovering a contact on page 9 marks it with the selector corners, a click on it designates it, and an empty click leaves the current designation alone. Empty scopes and NO TARGET are intentional in target-free flight. Systems now shows live damage-driven TEMP/OIL/HYD, internal FUEL and external tank fuel. D reports damage in the bottom-center sim log. [System failures and fitted rates](spec/systems-damage.md).

The scope, the exposure page and the weapons all read one shared sensor
component. [What it models, what is authored tuning and what is deferred](radar.md).

## Recovered commands awaiting their systems

All shortcut labels present in the supplied `FMENUD.MNU` are recognized. This is **not a claim that every original desktop command or system is ported**. The complete FA non-menu key-dispatch table still needs recovery. Available source/manual commands without working systems display a short message:

| Key | Reserved original action / remaining work |
| --- | --- |
| F4 | Track target |
| F5 / F6 / F7 / F8 | Player-to-missile / wingman / target; target-to-player cameras |
| F9 / F12 | Fly-by / missile camera |
| Ctrl + view key / Alt + view key | Missile-relative / target-relative camera |
| W / Shift-W, N | Waypoint selection, navigation/weapons mode |
| M | HARM seeker was the reserved action on this key. M now cycles sensor channels, so HARM has no binding until air-to-ground exists |
| V | Set Other View camera |
| Shift-J / Shift-K | Jettison fuel / air-to-ground stores |
| Ctrl-T / Alt-S | Target information / radio silence |
| Alt-1…9 | Wingman straight/level, break and approach directions |
| Alt-B/C/T/H/V/E/W/R/P/D | Wingman return, scope/formation/spacing, engagement, protection and disengagement |

Space now holds the selected player trigger. Bracket keys select the previous/next weapon or NAV; T cycles radar targets, Shift-T cycles back and Enter selects a visible one. `--live-fire` enables the
explicit PT-default test range; backslash resets its target at a suitable range
for the selected weapon. Ordinary free flight loads the aircraft's supported default weapons. Airborne
starts select and arm the canonical gun; ground starts enter NAV with weapons disarmed. [Startup rules](spec/hud-layout.md). The restricted native research adapter stays clean.
The complete native weapon/countermeasure dispatch remains unverified.
[Live-fire scope and approximations](baselines/live-fire.md). USNF manual bindings are reference evidence pending FA-specific verification; FA menu labels take precedence.

## In-flight menu

The runtime reads **? / Control / Pref / View / Window / Cheat / Multi / Pos**, including nested options, from the imported FA menu module. Arrows/Tab and Enter/Space navigate; Right opens a child menu, Left backs out or changes the top menu, and Escape backs out before closing. Mouse activation requires a matching press and release. Hover/focus remains silent.

The menu pauses flight and engine loops. Focus loss pauses and clears held controls; resume explicitly with Ctrl-P or the menu. Closing the menu preserves a pre-existing explicit/focus pause. Opening menus never advances a hidden backlog of simulation time.

**Escape → Pref → Weapon diagnostics?** shows or hides the upper-right weapon diagnostic panel: launch mode, seeker status, RELEASE LOCK, range, closure, estimated flight time, target aspect and the three most recent guided shots. It is off by default, reads On or Off beside the row, shows a short "Weapon diagnostics: on/off" message and is saved with the other flight preferences. With it off, the large layout's top-right instrument sits in its normal corner and the panel's click areas do nothing. The row is an authored addition after the retail Pref rows, not part of `FMENUD.MNU`; opinionated, requested by John on 2026-09-23 (the label and On/Off readout are agent choices). `--weapon-diagnostics` starts a launch with it shown, including captures.

Working menu actions include views, instrument windows, time/pause, cockpit, pitch ladder, weapon diagnostics, HUD brightness, ending flight and exiting. Sound currently toggles effects; the original volume mixer is not implemented. Working cheats are listed under [cheats](#cheats). Other preferences, cheats, multiplayer and position commands are navigable placeholders with feedback. The bottom Resume / Restart / Keyboard Shortcuts actions are documented development additions. Menus do not silently enable unsupported cheats or alter the aircraft when an unrelated modifier shortcut is pressed.

## HUD and presentation limits

The HUD uses imported `HUD11.FNT`; instrument/menu text uses `WIN11.FNT`. It shows wrapped heading, true airspeed in knots, MSL altitude, G, throttle, afterburner and actual gear/flap/brake/hook state. AGL and vertical speed are reserved for ILS approaches. The enlarged pitch ladder uses five-degree steps, dashed below zero, and a 25% tighter attitude scale whose spacing and motion follow actual pitch and bank, including readable +/-90-degree marks. The zero bar separately projects the true horizon and aligns with level-flight velocity. The flight-path marker comes from current kinematic vertical speed and airspeed. No target, weapon solution or navigation waypoint is invented.

Layout, line symbology, frame scaling, pan, zoom and camera placement are authored. `~F18H.PIC` is uniformly scaled to cover the actual flight aspect ratio, showing more side artwork on wider screens and cropping only what is required to avoid stretching. Mirrors render live rear views every visible frame. ILS appears only with the runway threshold inside the aircraft's full 90-degree
forward cone and existing 5-NM/4,000-foot airport-relative band. The airport
service publishes tested localizer and glide deviations, but dedicated ILS HUD art remains open. Native F18 HUD callers, HUDSYM glyph meanings, full cockpit composition, corner-speed/weapon modes and native pixel parity remain open. The HUD remains aligned with the aircraft-forward datum and pans opposite head-look with the cockpit. External views omit it.

See [recovery details](formats/aircraft.md), [progress](research/progress.md), and [validation](baselines/cockpit-controls.md).


The flight overlay is independent of the fixed menu canvas and tracks the window aspect. It is composed at the physical drawable size, proportionally capped at 1920×1080 for bounded CPU/GPU work. At centered forward view, cockpit art covers that entire overlay; menus remain centered at their original proportions. The HUD uses a 0.7225 layout scale, an additional 15% reduction from the previous 0.85 scale, with projection compensation for angular cues; the pitch ladder adds its requested 25% spacing compression. TAS and MSL primary numbers have transparent backgrounds; surrounding tape numbers and hash marks are omitted. With the cockpit visible, HUD symbols are clipped to its reviewed glass aperture
and drawn behind the cockpit frame. Cockpit-off and below-1x wide views retain
the independent HUD. Static cockpit artwork, glass masks and unchanged
instrument rasters are cached.

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

+/- scales artwork and HUD text/symbology with world zoom. Zoom crops around the HUD
center at the screen center in forward view, including below 1x. Below 1x,
cockpit artwork and mirrors disappear; HUD and instrument windows remain. At
1x or above, artwork returns if the cockpit toggle is enabled. Head-look still
translates the aircraft-forward datum with the camera. The finite source art provides no rear/overhead interior.
Mirrors use their original source silhouettes with live rear views. See [sliding cockpit validation](baselines/cockpit-slide.md).

The local USNF manual's “View Panning & Zooming” section specifies Ctrl+arrows when keyboard flight control is used, and Right Shift plus joystick for joystick panning. The reference app chose Shift+arrows. The supplied FA readme did not resolve the Anthology-specific binding, so Shift remains an explicitly documented convenience alias rather than claimed recovered FA behavior. [Validation](baselines/look-around.md).

## Flight response and vertical flight

The development adapter now carries an independent world-space velocity vector. Thrust, drag, lift and gravity accelerate that vector rather than setting it to the nose direction each tick. Pitch/roll controls have finite response, and the HUD flight-path marker uses both lateral and vertical velocity. The nose and actual travel direction can differ. These response constants are authored, not recovered native FA control laws.

Attitude rotates as an orthonormal basis and is interpolated in that basis. The old ±1.5-radian flight pitch clamp is removed; flight can pass through vertical/inverted attitudes and complete loops with sufficient energy. Cockpit head-look still cannot look below its forward eye line, this separate viewing restriction does not limit aircraft pitch. [Evidence and limitations](baselines/flight-response-sky.md).

## Exterior animations

The seven added aircraft now have fitted moving flaps, pitch/roll/yaw surfaces,
rigid gear and continuous airbrakes. MiG-23 wings sweep visually with speed.
F-22 main bays open with Shift+O or an armed guided-weapon designation; this does
not delay firing. Its exterior canopy is amber and 75% opaque; cockpit rendering stays clear.
See the [animation contract](spec/aircraft-animation.md) for fits and limits.


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

Enable rumble through **Escape → Control**, the controller tab's **Rumble** setting, then **Apply**. Actual
afterburner engagement produces a subtle impulse followed by a quiet continuous
low-frequency rumble while it remains active. The switch alone below the model's activation
threshold does not. Event feedback designs and future weapon/turbulence hooks
are listed in [INPUT.md](INPUT.md); successful gun/missile events now drive the corresponding rumble cues in the live-fire adapter.

The flight **Control** tab opens the same input configuration screen as
**Pref → Controls...** on the main menu, rather than the retail device-selection
stub. Every key, mouse input and gamepad button is listed in the
[controls master list](CONTROLS.md), and every stock key can be remapped. Instrument page sets/layout, scope controls, zoom,
cockpit/HUD options and sound preferences save automatically between normal
sessions and survive aircraft changes/restarts. [Full settings contract](INPUT.md).

## Manual weapons follow-up

`[` and `]` cycle NAV and weapons. Weapon selection arms; NAV disarms.
The HUD status reads NAV, LCOS for the armed gun, or ARM for missiles.
There is no separate master-arm control. **L** clears designation and **K**
jettisons the selected external group in the live range. Class and station-fault
fixtures remain available through the controls editor and command-line setup.
Restart repairs/reloads. Selection releases the trigger before another press.
T cycles current radar contacts nearest first and Shift-T backwards, skipping
friendly aircraft and wrecks. Enter selects the visible radar or infrared
contact nearest the nose. A mouse click on the scope designates a contact
directly. A target the scope loses drops completely
([rules](spec/radar.md#target-selection-keys)). SAFE/EMPTY/STATION FAILED and the sensor and range inhibits are shown
separately from lock; a terrain-masked target now reports NO TARGET, because
masking clears the contact rather than inhibiting the launch. The systems continuation below adds automatic source-weighted failures
for supported equipment. Quick Mission AI integration is described in the [AI spec](spec/ai.md).

`--record-combat NEW_PATH` records explicit combat-service inputs and commands;
`--replay-combat PATH` replays headlessly with the same aircraft/theater/assets.
This is separate from pilot-input recording and does not re-simulate flight.
[Controls, evidence and remaining gaps](baselines/manual-weapons.md).

## Weapons and systems continuation

**D** reports your aircraft damage percentage, temperature/oil/hydraulic readings,
remaining engine power and failed systems through the bottom-center sim log.
It never damages the aircraft. The `damage-player` developer command and controller
fixture remain available explicitly. In the `--live-fire` range, **Shift-I** spawns one incoming selected source weapon, and **Shift-Y** toggles
target ECM. Those two fixtures moved off I and Y when those keys took over
infrared selection and contact history.
**J** controls own ECM and **R** radar. The incoming fixture does
not command AI or spend player ammunition. K selected-group jettison, L clear designation, bracket NAV/weapon cycling,
T/Shift-T/Enter targeting, Space hold fire and backslash target replacement remain available.

Standard Linux pads use **held Select** as the combat layer: RB fire, LB weapon,
A designate, B clear, X previous NAV/weapon, Y own ECM, L3 radar, R3 jettison. D-pad up replaces
target, down requests a player hit, left cycles class, right fails the station.
Select+Start toggles target ECM; Select+Guide spawns the incoming fixture when
the desktop exposes Guide. These suppress the corresponding base flight/menu
bindings. Release before switching layers. Unmodified Start pauses. F10/custom
`view-external` replaces Select's old default exterior action. Exact mappings,
profile migration/editor behavior, unsupported controllers and haptic limits are
in [INPUT.md](INPUT.md#manual-combat-layer-2026-09-14).

Weapons page V/R/E reports visual availability, radar and ECM: `+` available/on,
`-` off, `!` failed. A failed visual sensor now also removes visual contacts, so
the V indicator and the scope agree. Automatic source-index faults are separate from the manual
developer injection. Engine, fluid, control, structural and pilot failures now
affect ownship and use the sim log. The panel retains only its retail gauges
and fuel rows. See [systems damage](spec/systems-damage.md) for exact fitted rates.
[Recovered contracts, runtime evidence and remaining gates](baselines/weapons-systems.md).


HUD brightness now uses the recovered signed range -256..256, steps of 16 and
neutral zero, applied to the original palette entry before sunlight whitening.
The primary ink comes from each aircraft's HUD module; layout remains authored.
Version-1 preferences migrate the old setting relative to its neutral value 7;
version 2 persists the new signed amount. Other saved display choices are retained.

The main HUD shows outlined current TAS/MSL values without surrounding tape
numbers or hash marks, plus a curved bank scale immediately beneath the
pitch ladder (crash/engine-off alerts take priority). Active ILS places AGL below
the altitude box and V/S below the airspeed box. The scale rotates past a
fixed triangular index, with 10-degree ticks and numbers every 30 degrees.
It follows aircraft attitude, including full rolls, independently of head-look.
The readout outlines are transparent. Combat debug status and range hints are no longer overlaid on
flight; normal HUD and instrument windows remain available.

## Target camera (view 4)

View 4 automatically magnifies the target to fill the image along the player's
sight line, including elevation. The camera sits no farther than one nautical
mile from the target, staying at the player position for nearer targets. It shows
a grayscale live target image, aircraft/object type, damage bar,
clock bearing with Hi/Lo, and range/speed alternating every three simulation
seconds. HI/LO appears only beyond 10 degrees above/below the horizontal
plane through your aircraft, independent of pitch and bank. A black bar is undamaged; white grows with damage. Existing pilot skill
and activity appear for mission aircraft. An underlined A means attacking you;
plain A means another target. Objective assignments are not yet available in
launch data, so the panel says `OBJECTIVE ?`. This does not mark every enemy as
an objective. See [behavior and remaining limits](spec/target-window.md).

## Cheats

**Escape → Cheat** toggles the session cheats below. Each row shows **On** or
**Off**; in the Damage submenu the selected choice reads On. Cheats apply on the
next simulation step, including when chosen while paused, survive Restart and
are not saved to disk. Behaviour: [cheats specification](spec/cheats.md).

| Entry | Effect |
| --- | --- |
| Damage → Invulnerable / Normal | Invulnerable: weapon hits do no damage, no system faults and no pilot kill, and a midair collision does not kill you. Crashes into the ground still kill. Normal restores the damage model. |
| Unlimited ammo? | The player's rounds and stores never run out. |
| Unlimited fuel? | The player's fuel never drops, including from damage leaks. |
| No spins? | No spin entry; a spin in progress damps out as a stall. |
| Pull extra G? | The player can pull 9 G whatever the aircraft's limit and load. Near stall the low-speed ceiling still ramps up to 9 G. |
| Ignore weapon weights? | Stores add no weight and no loading drag; fuel left in external tanks still counts. |
| No redout or blackout? | Turns off the G effects. Above 5 G, after a delay of 5 s just over 5 G that shrinks by a second per extra G (2 s at 8 G), the view greys out from the edges inward in proportion to G, fully black at 7.5 G and above. Below -2 G, after 3 s, it reds out, fully at -3 G. Thresholds follow published human G tolerance for a pilot in a G-suit. Vision clears in about 3 s and the controls keep working. |
| No screen-shaking? | Turns off the view shake that starts at 6 G and reaches about 4 pixels at 9 G in the cockpit views. |
| No crashes? | Ground, water and unsafe landings bounce the aircraft back into the air instead of crashing it; a building turns it around. Safe runway landings still land. |
| Easy aiming? | The player's rounds and missiles see targets 50% larger; the player's missiles turn 50% faster and their in-flight seeker cone is 25% wider. |
| Easy targeting? | The radar keeps the target selected while it is off the scope, including behind you in a merge, and outside the HUD its square floats over the target at the HUD's own brightness. Awareness only: no lock, radar lead or missile support is kept while it is off the scope. |
| Enemy AI? → Novice / Average / Unchanged | Every enemy aircraft flies at that skill from now on; Unchanged restores each one's mission skill. Friendly aircraft are unaffected. |
| Air combat guns only? | Every aircraft may fire only its gun. The player's weapon keys skip the other stations and a selected missile switches to the gun; AI aircraft stop choosing their other stores. Turning it off restores them. |
| Ignore midair collisions? | Aircraft pass through each other. With it off, two aircraft whose 28 ft contact spheres touch are both destroyed and nobody is credited with a kill; an Invulnerable player survives. |

Every missile or bomb burst on an aircraft, the player's or an AI's, now jolts
it: a fading roll, pitch and push away from the burst, even with Invulnerable
on. Details: [missile hit jolt](spec/cheats.md#missile-hit-jolt).
| No turbulence?, No sun whiteout? | Described below. |

Flight cheats apply to the default and legacy flight models; the native research
path ignores them. Damage Realistic still reports that it is not implemented
yet.

## Sun glare cheat

**Escape → Cheat → No sun whiteout?** toggles glare suppression. **On** removes
sun whitening from world/cockpit/HUD palettes and disables lens-flare circles in
all rendered views; the original sun remains visible. It applies while paused,
lasts across mission restarts in the current session and is not saved to disk.
The developer override `TORE_SUN_GLARE=0` also disables these effects.

Main, mirror and instrument camera scenes now resolve their own altitude, fog,
palette and glare from the same weather clock. Mirror rendering stays GPU-only;
camera instruments use asynchronous feeds and can show an older completed image.
The target camera requests 24 frames per second; other camera panels retain
their roughly 10 Hz feed. Target-camera scenery is 10% darker for contrast,
with aircraft, static objects and text brightness preserved. [Evidence and remaining work](baselines/weather-cameras.md).

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
commands; X-31 and the seven [roster additions](spec/roster-aircraft.md) do not.
A-4E and Su-25 ignore burner commands and rejects a nonzero
burner capture fraction. F-14 visual sweep is automatic and fitted. X-31 thrust
vectoring controls remain unavailable. See [aircraft behavior](spec/additional-aircraft.md).

F-14D, A-4E, X-31 and the seven roster additions use their own FA roll response values. Below 220 ft/s,
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

## Gun pipper and target direction

Selected guns show ammunition and a ballistic pipper. With no usable radar
observation, `1000 FT` marks the manual's fixed reference distance. With radar
on and a current targeted aircraft observation, `RADAR` marks automatic range,
lead and drop correction. Place the pipper over the target. Its thick range arc
grows as the target closes; numeric range is in nautical miles. SAFE, empty and
failed guns hide the firing pipper.

The selected target has a square inside the HUD and a directional chevron at the
HUD edge when outside, including behind the aircraft. The cue drops with the
selection; the Easy targeting cheat keeps it and draws the square anywhere on
screen, without radar lead or weapon support. **L**, or
**RELEASE LOCK** in the weapon diagnostic panel when it is shown, clears both selections. Destroying/removing the target or
resetting the mission also removes its cue. The marker works with guns,
missiles and NAV selected. The [specification](spec/gunsight-targeting.md)
separates manual behavior from fitted projection and targeting rules.

For a controlled capture, `--hud-target-preview bearing,elevation,feet` first
selects a visible range aircraft, then repositions it relative to ownship. Use
with `--capture-flight`; the fixture cannot record or replay a mission.

## Missile seeker control

The upper range-scale value is now an estimated maximum for the observed
engagement, refreshed twice per simulation second. Launcher speed and attitude,
target direction/speed, altitude-dependent motor performance and turning losses
change it. Active-radar shots can be cued beyond the imported nominal launch
maximum and acquire with their own seeker later. Other guidance types still
require prelaunch acquisition within seeker range. The lower scale value remains
the imported minimum range. Onboard seeker limits are unchanged. The percentage and IN RNG use the predicted
intercept. Target evasion and future loss of guidance are not predicted.
Two short horizontal bars inside the vertical scale mark the favorable firing
window. They move with the estimated engagement and disappear when no favorable
window exists. This is a fitted recommendation, not a guaranteed hit. The target
triangle remains visible outside range, clamped to the appropriate scale end.
IN RNG reflects predicted reach and release readiness, independent of percentage
rounding. [Band rules](spec/missiles.md#favorable-firing-range-bars).

The TAS/MSL boxes sit slightly lower, with the surrounding tape numbers removed.
Their horizontal positions remain on either side of the ladder. TARGET DESTROYED remains
in diagnostics and release logic but is omitted from the HUD.

Armed independent air-to-air missiles automatically enter BORESIGHT when radar power is on and no target is
selected. IR also supports bore with radar power off. A selected track takes
priority for IR and forces CUED acquisition against that identity, even when a
stronger bore return exists. Clear the track to return to BORESIGHT; airborne
missiles keep their own targets. Select a target to return to CUED. Press **L**, the existing
`clear-designation` action, or click **RELEASE LOCK** in the upper-right weapon
diagnostic panel when it is shown, to clear
selection. The manual's targeting list does not establish a retail release key.
`weapon-seeker-mode` remains rebindable but has no default key; the panel's mode
label is clickable only while the panel is shown (**Pref → Weapon diagnostics?**). With radar power off, radar-missile bore, tones and target cues
turn off; armed A2A IR missiles retain bore search and guidance; weapon selection still permits an unguided release. That missile stays unguided
after radar power returns. Selecting the passive IR channel alone does not turn
off the power switch. Supported radar weapons need aircraft lock for guided shots.
A detected bore target inside minimum range displays MIN RANGE and blocks release.
AGM-65 and other surface profiles cannot engage the practice aircraft or use A2A
BORESIGHT. Surface designation is still deferred. See the
[minimum range and target-role rules](spec/missiles.md#minimum-engagement-and-target-role).

BORE uses a five-degree circular half-angle. Its blinking diamond marks a
provisional contact, not a guaranteed lock; the blinking triangle on the range
scale refers to that same contact. Selection favours the centre while retaining
signal-strength weighting. IR can acquire on the rail; active radar acquires only
after release. The bare percentage is a fitted estimate, not a calibrated retail percentage.
Short retail weapon labels, bore circle, estimate and readiness all move with
the forward HUD when looking around. Range, closure and estimated flight time
and target aspect angle are in the upper-right weapon diagnostic panel, hidden by default. The HUD layout
is 15 percent smaller; ARM, count/weapon and percentage with blinking IN RNG
align below speed. The range scale sits inside altitude; radar R/C/A sits below it. BORE READY is omitted. Neither clearing selection nor changing mode redirects an airborne shot.
An internal bay opens for BORESIGHT and release waits until 95 percent open.
Weapon readouts sit just below the airspeed and altitude boxes; NAV retains the bank scale. AGL and
vertical speed appear only with non-weapon ILS guidance. [Layout and startup modes](spec/hud-layout.md). CUED radar
lock diamonds blink when ready to fire. The radar instrument replaces the mouse
arrow with a crosshair across the entire black screen, up to the bezel.

Imported IR and radar search/lock samples provide the cues, with a louder lock
cue. Their assignment is fitted. Effects mute, pause, safe, empty and failed
stations silence them. `TORE_SEEKER_VOLUME=0..1` sets maximum amplitude, default
0.30. Re-import media to add the four samples to an older cache.
[Rules and constants](spec/missiles.md), [validation](baselines/hud-cleanup.md).

Each wing's skill selector also offers **Dummy (400 KTS)**. These training targets
hold their launch heading and altitude at 400 knots ground speed. They do not
fight, evade or follow wing commands, but remain damageable. Select a normal
skill to restore combat behavior. The player's aircraft remains under your
control. [Dummy mode specification](spec/dummy-aircraft.md).

Quick Mission wing counts launch independent AI aircraft by default, up to 29
plus the player. They use the selected aircraft and skill, with separate wing
formations. `--fixture-wings` retains straight-flight practice. Restart restores
the initial actors, stores, formations and damage state. Player wing shortcuts
are listed in [the input guide](INPUT.md#player-wing-orders).

Gun rounds have a 0.5-degree full spread cone (up to 0.25 degrees from aim),
with luminous warm cores and soft amber halos. Spread affects hits and repeats
deterministically on replay. See the [gun rules](spec/damage-smoke.md#gun-dispersion-and-luminous-tracers).

Aircraft accumulate damage in nose, cockpit, core, left wing, right wing and
tail regions. Light hits add persistent local marks. Concentrated damage grows
into local wing or fin tears, and a reviewed original damaged body appears only
when its missing region matches the hit. Global half-health still starts dark
smoke. Destroyed targets remain visible during their existing fall.
Powered missiles leave white smoke that disperses after burnout or impact.
Contrails, aircraft damage smoke and missile smoke share the clouds' live weather
palette, sunset lighting and haze, so they darken and tint at dusk and night.
Contrails last two simulation minutes: steady opacity for one minute, then
fade smoothly to invisible during the last minute. Pausing freezes their age;
stopping emission leaves existing puffs to finish their normal lifetime.
Reset restores intact aircraft and clears smoke. Regional thresholds, visual changes and smoke timing are fitted. Detached reviewed pieces inherit aircraft motion, fall, then disappear
with a brief ground-hit animation. AI damage uses the fitted health-to-authority rule in the [AI spec](spec/ai.md#live-integration-and-authored-boundaries); the player retains the existing flight adapters. See [damage and smoke behavior](spec/damage-smoke.md).

Damage appearance can be inspected with `--damage-preview 0..1` and
`--damage-preview-section nose|cockpit|core|left-wing|right-wing|tail`, together
with `--capture-flight PATH`. The fraction populates the selected region in
this presentation fixture; it does not simulate a shot. See the
[damage specification](spec/damage-smoke.md) for fitted thresholds.

## Starting on a runway

Quick Mission's **Start: Ground** choice exposes an airport selector for the
current theater. The player starts stationary, engine idling, gear and flaps down,
with brakes applied. Press **B** to release brakes, then increase throttle and
use the normal pitch controls for takeoff. Ground start uses the researched model;
no flight adapter is switched automatically. Wing aircraft retain their airborne
start. [Start behavior and fitted settings](spec/quick-mission-menu.md#player-ground-start).

While on a runway or using ILS, `XW` shows signed crosswind and the aircraft's
MTOW-class limit in knots. `NOTICE`, `ROUGH` and `LIMIT` identify increasing
difficulty. `TW` appears for tailwind, with a universal 10-knot limit. Positive
XW pushes toward runway right. These cues help choose a runway and manage rollout;
normal controls and tower clearance remain available. Parked tire grip remains.
[Wind thresholds and ground response](spec/runway-wind.md).

For takeoff, leave flaps extended, release **B**, apply full throttle (afterburner
only where available), and use gentle back pressure as the aircraft accelerates.
Ease the pull after liftoff and retract gear when clear. Flaps now add low-speed
lift instead of acting only as drag. Low-speed trim no longer pulls the nose
toward the former steep airborne target while accelerating on the runway. Ease
back pressure after liftoff to control the climb. There is no new takeoff-flap detent.
Headwind changes the ground speed needed for liftoff; the runway wind warnings
remain difficulty guidance. Land with controlled airspeed and settle the nose
after touchdown; overspeed full-flap approaches can float.
[Implementation and limits](spec/takeoff-ground-contact.md).

Developer takeoff captures can combine `--ground-start N`,
`--flight-probe-ticks TICKS`, `--maneuver takeoff` and `--capture-flight PATH`.
The level maneuver also supports an idle ground capture. Other ground pose
overrides remain rejected. [Acceptance and human test notes](baselines/takeoff-acceptance.md).

The original Map filter menu is omitted by request. Shift-M shows known runways
and current aircraft detections. Right-side buttons toggle Aircraft, Airfields,
Buildings, Surface and Emitters. Buildings start off; the other categories start
on. Unknown contacts use placeholders. Plus/minus zoom, arrows pan and Home follows the player. Flight
continues; map pointer input cannot operate the covered instruments.
`--flight-map --capture-flight PATH` captures this view. Map projection,
identification and surface detection are fitted rules in the [map spec](spec/flight-map.md).

NAV INFO and WEAPONS now use minus/plus selection and a third mode/page button.
See [selection and instrument rules](spec/weapon-navigation-selection.md).

Friendly selected contacts have a centered X inside their HUD target box.
[Identification and presentation rules](spec/gunsight-targeting.md#target-square-and-edge-chevron).

Partial wing or tail damage reduces control effectiveness and can cause an
uncommanded roll or yaw. Autopilot is unavailable after such structural damage.
The D report never rounds a surviving aircraft up to 100%; catastrophic whole-part
loss requires actual destruction. All damage marks and partial tears are also
currently hidden until 100%, while flight and system failures remain active.
See [damage behavior](spec/systems-damage.md).

Destroyed airborne aircraft now tumble and fall without accepting pilot commands.
Surviving engines may continue to push the wreck, including asymmetric thrust.
A 5% airburst check runs each second for five seconds, then every five seconds
until impact. A successful roll explodes and removes the aircraft. Restart clears
the wreck. See [destroyed-aircraft behavior](spec/destroyed-aircraft.md).

Pilot death immediately selects the F10 exterior view, recenters look/zoom and
closes the navigation map without pausing. Player nose loss is fatal to the pilot.
The player wreck explodes on ground impact; a safe landing does not.

A destroyed player aircraft continues trailing damage smoke while airborne,
even after pilot death. Impact or explosion stops emission; existing smoke fades.
