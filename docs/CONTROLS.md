# Controls master list

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Every default keyboard key, mouse input and gamepad button, in one place. The
gamepad column uses Xbox names: View is the old Back/Select button and Menu is
Start. Players change any of these in **Pref → Controls...** on the main menu
or **Escape → Control** in flight; see [input](INPUT.md) for how bindings,
modifiers and profiles work.

The [keyboard map](tore-keyboard-map.html) is the printable flight, comms,
view and replay reference. Its [update conventions](tore-keyboard-map-rules.md)
keep test commands out of that reference.

The tables below are generated from `crates/tore-app/src/input_catalog.rs` and
the standard gamepad defaults in `crates/tore-app/src/input.rs`. A test fails
when they drift. After changing a default, regenerate with:

```sh
TORE_UPDATE_CONTROLS_DOC=1 cargo test --locked -p tore-app controls_doc
```

Gamepad defaults apply automatically to standard Linux gamepads. On Windows and
macOS, and for sticks, throttles and pedals, bind controls in the controls
screen. "View + RB" means hold View, then press RB. A dash means no default.

Added gamepad mappings depend on aircraft capability. Powered-lift aircraft use
View+Start for the engine; vectoring jets and the V-22 use View+D-pad Up for
forward nozzle/nacelle neutral. On the AC-130 View+right stick slews the gunsight,
View+D-pad Up and Down pick and link its guns (the range reset and damage test
they carry elsewhere are off there), View+A designates under the crosshair and
View+A held half a second pins the ground. Other aircraft keep their original
modifier look/rudder/fixture mappings.
The full profile remains visible in Controls. See [input contexts](INPUT.md).

**Keys that depend on the aircraft.** On the AV-8 and Yak-141, Z and X step the
nozzles and Shift+Z and Shift+X are the nozzle presets, as in the retail manual;
on every other aircraft they stay a rudder pair. On the helicopters and the V-22
Ctrl with the arrows is the cyclic trim and the throttle keys set the
collective, and on the helicopters 0 recentres the trim. Ctrl+Shift+A cycles
the powered-lift stability level and Ctrl+Alt+A is hover hold. A cell such as
"Z (not on AV-8 / Yak-141)" names where a key acts; the full table is in
[input](INPUT.md#vtol-tiltrotor-and-helicopter-controls). On some European
layouts Windows reports AltGr as Ctrl+Alt, so AltGr+A also reaches hover hold.

**macOS and laptop keyboards.** The powered-lift keyboard defaults use Ctrl with
the arrow keys (nozzle pitch on the vectoring jets, cyclic trim on the
helicopters and the V-22) and Ctrl with Home, End, Page Up and Page
Down (collective and nacelle conversion). On a Mac, macOS claims Ctrl+Up for
Mission Control, Ctrl+Down for App Exposé and Ctrl+Left and Ctrl+Right to switch
Spaces, so those presses may never reach the game unless you turn the shortcuts
off in System Settings, Keyboard, Keyboard Shortcuts, Mission Control. Laptop
keyboards also have no dedicated Home, End, Page Up or Page Down keys; they are
Fn with the arrow keys, which makes the Ctrl combinations awkward. Every one of
these can be remapped in the controls screen (Controls, then the action's row),
and a gamepad's View+stick defaults avoid the problem.


<!-- controls-table:start -->

### Flight controls

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Pitch (nose up/down) | - | - | Left stick Y |
| Pitch: nose down | Up | - | - |
| Pitch: nose up | Down | - | - |
| Roll (bank left/right) | - | - | Left stick X |
| Roll left | Left | - | - |
| Roll right | Right | - | - |
| Rudder (yaw) | - | - | - |
| Rudder left | End or Z (not on AV-8 / Yak-141) | - | LT |
| Rudder right | Page Down or X (not on AV-8 / Yak-141) | - | RT |
| Nozzle pitch (VTOL) lever | - | - | - |
| Nozzle pitch (VTOL) rate axis | - | - | View + Right stick Y |
| Nozzle pitch (VTOL): decrease | Ctrl+Up (fixed-wing and AV-8 / Yak-141 only) | - | - |
| Nozzle pitch (VTOL): increase | Ctrl+Down (fixed-wing and AV-8 / Yak-141 only) | - | - |
| Nozzle yaw (VTOL) lever | - | - | - |
| Nozzle yaw (VTOL) rate axis | - | - | View + Right stick X |
| Nozzle yaw (VTOL): decrease | Ctrl+Left (fixed-wing only) | - | - |
| Nozzle yaw (VTOL): increase | Ctrl+Right (fixed-wing only) | - | - |
| Nacelle conversion (V-22) lever | - | - | - |
| Nacelle conversion (V-22) rate axis | - | - | View + Right stick X |
| Nacelle conversion (V-22): decrease | Ctrl+Page Up | - | - |
| Nacelle conversion (V-22): increase | Ctrl+Page Down | - | - |
| Collective (helicopters / V-22) lever | - | - | - |
| Collective (helicopters / V-22) rate axis | - | - | View + Right stick Y |
| Collective (helicopters / V-22): decrease | Ctrl+End | - | - |
| Collective (helicopters / V-22): increase | Ctrl+Home | - | - |
| Nozzles/nacelles: forward neutral | 0 (not on helicopters) | - | View + D-pad up |
| Nozzles up (aft) 10 degrees (AV-8, Yak-141) | Z (AV-8 / Yak-141 only) | - | - |
| Nozzles down 10 degrees (AV-8, Yak-141) | X (AV-8 / Yak-141 only) | - | - |
| Nozzles to 0, or braking stop to vertical (AV-8, Yak-141) | Shift+Z (AV-8 / Yak-141 only) | - | - |
| Nozzles vertical, again to the braking stop (AV-8, Yak-141) | Shift+X (AV-8 / Yak-141 only) | - | - |
| Cyclic trim fore/aft (helicopters / V-22) rate axis | - | - | - |
| Cyclic trim forward (helicopters / V-22) | Ctrl+Up (V-22 and helicopters only) | - | - |
| Cyclic trim aft (helicopters / V-22) | Ctrl+Down (V-22 and helicopters only) | - | - |
| Cyclic trim left/right (helicopters / V-22) rate axis | - | - | - |
| Cyclic trim left (helicopters / V-22) | Ctrl+Left (V-22 and helicopters only) | - | - |
| Cyclic trim right (helicopters / V-22) | Ctrl+Right (V-22 and helicopters only) | - | - |
| Pedal trim (helicopters / V-22) rate axis | - | - | - |
| Pedal trim left (helicopters / V-22) | - | - | - |
| Pedal trim right (helicopters / V-22) | - | - | - |
| Trim set / force trim release (helicopters / V-22) | - | - | - |
| Trim to centre (helicopters) | 0 (helicopters only) | - | - |
| Stability level: Off, Damper, Attitude (VTOL) | Ctrl+Shift+A (not on fixed-wing) | - | - |
| Stability level Off (VTOL) | - | - | - |
| Stability level Damper (VTOL) | - | - | - |
| Stability level Attitude (VTOL) | - | - | - |
| Throttle / engine power lever | - | - | - |
| Throttle rate (axis) | - | - | - |
| Throttle up | - | - | RB |
| Throttle down | - | - | LB |
| Throttle idle | 1 | - | - |
| Throttle 25% | 2 | - | - |
| Throttle 50% | 3 | - | - |
| Throttle 75% | 4 | - | - |
| Throttle 100% | 5 | - | - |
| Throttle afterburner | 6 | - | - |
| Throttle down 5% | 7 | - | - |
| Throttle up 5% | 8 | - | - |
| Afterburner | Shift+B | - | Y |
| Autopilot (heading/altitude) | A | - | - |
| Waypoint autopilot | Ctrl+A | - | - |
| Hover hold autopilot (helicopters / V-22) | Ctrl+Alt+A | - | - |

### Systems

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Eject (press twice) | Shift+E | - | - |
| Landing gear | G | - | A |
| Flaps | F | - | X |
| Airbrake / wheel brakes | B | - | B |
| Tailhook | H | - | - |
| Engine on/off | E | - | View + Menu |
| Weapon bays (F-22) | O | - | - |
| Damage report | D | - | - |

### Weapons

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Fire / release weapon | Space | - | View + RB |
| Next weapon / NAV | ] | - | View + LB |
| Previous weapon / NAV | [ | - | View + X |
| Next gun candidate (AC-130) | Ctrl+7 | - | View + D-pad up |
| Link/unlink candidate gun (AC-130) | Ctrl+8 | - | View + D-pad down |
| Gunsight: designate under crosshair (AC-130) | Backslash | - | View + A (tap) |
| Gunsight: pin ground point (AC-130) | Shift+Backslash | - | View + A (hold half a second) |
| Gunsight: zoom in (AC-130) | Shift+Apostrophe | - | - |
| Gunsight: zoom out (AC-130) | Shift+Semicolon | - | - |
| Gunsight: slew left/right (AC-130) | - | - | View + Right stick X |
| Gunsight: slew up/down (AC-130) | - | - | View + Right stick Y |
| Gunsight: slew left (AC-130) | Alt+Left | - | - |
| Gunsight: slew right (AC-130) | Alt+Right | - | - |
| Gunsight: slew up (AC-130) | Alt+Up | - | - |
| Gunsight: slew down (AC-130) | Alt+Down | - | - |
| Next radar target | T | - | View + A |
| Previous radar target | Shift+T | - | - |
| Select visual target | Enter or Apostrophe | - | - |
| Clear designation | Semicolon or L | - | View + B |
| Seeker mode (bore/cued) | - | - | - |
| Release chaff | Insert | - | View + D-pad left |
| Release flare | Delete | - | View + D-pad right |
| Jettison selected stores | Shift+K | - | View + Right stick press |
| Reset range target | Ctrl+Shift+Backslash | - | View + D-pad up |
| IFF squawk on the target | U | - | - |
| Incoming missile (range) | Ctrl+Shift+I | - | View + Xbox |
| Target jammer (range) | Shift+Y | - | View + Menu |
| Next damage class (test) | - | - | - |
| Fail station (test) | - | - | - |
| Damage player (test) | - | - | View + D-pad down |

### Sensors and instruments

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Radar power / radar channel | R | - | View + Left stick press |
| Jammer (ECM) | J | - | View + Y |
| Cycle sensor channel | M | - | - |
| Infrared channel | I | - | - |
| Contact history | Y | - | - |
| Scope range down | Period | - | - |
| Scope range up | Comma | - | - |
| NAV / ILS mode | N | - | - |
| Next waypoint | W | - | - |
| Previous waypoint | Shift+W | - | - |
| Next instrument | Ctrl+Tab | - | D-pad right |
| Previous instrument | Ctrl+Shift+Tab | - | D-pad left |
| Select instrument 1 | Ctrl+1 | - | - |
| Select instrument 2 | Ctrl+2 | - | - |
| Select instrument 3 | Ctrl+3 | - | - |
| Select instrument 4 | Ctrl+4 | - | - |
| Select instrument 5 | Ctrl+5 | - | - |
| Select instrument 6 | Ctrl+6 | - | - |
| Instrument button 1 | Ctrl+Shift+1 | - | D-pad up |
| Instrument button 2 | Ctrl+Shift+2 | - | D-pad down |
| Instrument button 3 | Ctrl+Shift+3 | - | - |
| Instrument button 4 | Ctrl+Shift+4 | - | - |
| Window: Envelope | Shift+1 | - | - |
| Window: Forward view | Shift+2 | - | - |
| Window: Other view | Shift+3 | - | - |
| Window: Radar/Visual | Shift+4 | - | - |
| Window: RWR | Shift+5 | - | - |
| Window: Navigation | Shift+6 | - | - |
| Window: Systems | Shift+7 | - | - |
| Window: Weapons | Shift+8 | - | - |
| Window: Radar | Shift+9 | - | - |
| Window: Radar cross section | Shift+0 | - | - |

### View

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Front cockpit view | F1 | - | Left stick press |
| Look back | F2 | - | - |
| Look up (view) | F3 | - | - |
| External view | F10 | - | - |
| Track current target | F4 | - | - |
| Player to inbound missile | F5 | - | - |
| Player to wingman | F6 | - | - |
| Player to target | F7 | - | - |
| Target to player | F8 | - | - |
| Fixed fly-by view | F9 | - | - |
| Missile to its target | F12 | - | - |
| Save and open Other View | V | - | - |
| Target-relative tracking (Alt-F4 exits) | - | - | - |
| Last missile: forward view | Ctrl+F1 | - | - |
| Last missile: back view | Ctrl+F2 | - | - |
| Last missile: up view | Ctrl+F3 | - | - |
| Last missile: tracking view | Ctrl+F4 | - | - |
| Last missile: threat view | Ctrl+F5 | - | - |
| Last missile: wingman view | Ctrl+F6 | - | - |
| Last missile: to target view | Ctrl+F7 | - | - |
| Last missile: target to reference view | Ctrl+F8 | - | - |
| Last missile: fly-by view | Ctrl+F9 | - | - |
| Last missile: external view | Ctrl+F10 | - | - |
| Last missile: missile to target view | Ctrl+F12 | - | - |
| Target: forward view | Alt+F1 | - | - |
| Target: back view | Alt+F2 | - | - |
| Target: up view | Alt+F3 | - | - |
| Target: threat view | Alt+F5 | - | - |
| Target: wingman view | Alt+F6 | - | - |
| Target: to target view | Alt+F7 | - | - |
| Target: target to reference view | Alt+F8 | - | - |
| Target: fly-by view | Alt+F9 | - | - |
| Target: external view | Alt+F10 | - | - |
| Target: missile to target view | Alt+F12 | - | - |
| Look left/right | - | Hold right button and drag | Right stick X |
| Look left | Shift+Left | - | - |
| Look right | Shift+Right | - | - |
| Look up/down | - | Hold right button and drag | Right stick Y |
| Look up | Shift+Up | - | - |
| Look down | Shift+Down | - | - |
| Head tracker yaw | - | - | - |
| Head tracker pitch | - | - | - |
| Center view | Keypad 5 or Shift+/ | - | Right stick press |
| Zoom in | Equals | Wheel up | - |
| Zoom out | Minus | Wheel down | - |
| Cockpit art | Backspace | - | - |
| HUD | Shift+U | - | - |
| Dim HUD | Shift+[ | - | - |
| Brighten HUD | Shift+] | - | - |
| Live map | Shift+M | - | - |
| Show target info | Ctrl+T | - | - |

### Replay drone

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Drone: cycle flight/follow/free | Backquote (~) | - | - |
| Drone: follow aircraft | - | - | - |
| Drone: free camera | - | - | - |
| Drone: move forward | W | - | - |
| Drone: move backward | S | - | - |
| Drone: move left | A | - | - |
| Drone: move right | D | - | - |
| Drone: move up | E | - | - |
| Drone: move down | Q | - | - |
| Drone: four times faster | Shift | - | - |
| Drone: hold to look with mouse | - | Right button | - |
| Drone: increase movement speed | - | Wheel up | - |
| Drone: decrease movement speed | - | Wheel down | - |

### Communication

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Wing: fly straight | Alt+1 | - | - |
| Wing: break left | Alt+2 | - | - |
| Wing: break right | Alt+3 | - | - |
| Wing: break low | Alt+4 | - | - |
| Wing: break high | Alt+5 | - | - |
| Wing: approach target left | Alt+6 | - | - |
| Wing: approach target right | Alt+7 | - | - |
| Wing: approach target low | Alt+8 | - | - |
| Wing: approach target high | Alt+9 | - | - |
| Wing: engage my target | Alt+E | - | - |
| Wing: engage from formation | Alt+R | - | - |
| Wing: sort (a different bandit for each wingman) | Alt+A | - | - |
| Wing: attack on contact | Alt+W | - | - |
| Wing: protect me | Alt+P | - | - |
| Wing: disengage | Alt+D | - | - |
| Wing: bug out | Alt+B | - | - |
| Wing: next formation | Alt+T | - | - |
| Wing: loose/medium control | Alt+C | - | - |
| Wing: spacing | Alt+H | - | - |
| Wing: stacking | Alt+V | - | - |
| Wing: land at selected airport | Alt+L | - | - |
| Radio silence | Alt+S | - | - |
| Monitor the battle net (the other flights' contact reports and attack calls) | Alt+N | - | - |
| Address whole flight | Alt+0 | - | - |
| Address wingman 1 | Alt+Shift+1 | - | - |
| Address wingman 2 | Alt+Shift+2 | - | - |
| Address wingman 3 | Alt+Shift+3 | - | - |
| Address wingman 4 | Alt+Shift+4 | - | - |
| Reply to the flight: Engaging | Alt+Shift+E | - | - |
| Reply to the flight: Winchester | Alt+Shift+W | - | - |
| Reply to the flight: Bingo fuel | Alt+Shift+B | - | - |
| Request help from the flight | Alt+Shift+H | - | - |
| Chat line (network games only) | Backquote (~) | - | - |
| Next airport | Shift+N | - | - |
| Request landing | Shift+L | - | - |
| Repeat tower reply | Ctrl+Shift+R | - | - |
| Cancel approach | Ctrl+Shift+C | - | - |

### Game and menus

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Flight menu / back | Esc | - | Xbox |
| Pause | Ctrl+P | - | Menu |
| Time compression | C | - | - |
| Slow motion | Shift+C | - | - |
| End mission | Ctrl+Q | - | - |
| Valkyries music | Ctrl+V | - | - |
| Score board (network games) | K | - | - |
| Mark replay moment | Ctrl+B | - | - |
| Restart flight | - | - | - |
| Keyboard help | F11 | - | - |
| Menu up | Up | - | D-pad up |
| Menu down | Down | - | D-pad down |
| Menu left | Left | - | D-pad left |
| Menu right | Right | - | D-pad right |
| Menu select | Enter | - | A |
| Menu back | Esc | - | B |
| Fullscreen / window | Alt+Enter | - | - |
| Exit to desktop | Alt+F4 | - | - |

<!-- controls-table:end -->

## Built-in controls outside the tables

| Situation | Input | Action |
| --- | --- | --- |
| Flight | Left click | Operate instrument buttons, designate a scope contact, click the seeker label or RELEASE LOCK in the weapon diagnostic panel when Pref → Weapon diagnostics? shows it |
| Flight | Hold right button and drag | Mouse look, when enabled on the Mouse tab |
| Flight, Pref → Debug panels? on | Right-click without dragging | The debug menu on the aircraft or missile under the pointer, or a list of aircraft; not while mouse look is off and the right button is bound |
| Flight, debug menu open | Up / Down / Tab, Home / End, PageUp / PageDown, Enter / Space / Right, Esc / Left | Move, choose, close; other keys still fly |
| Flight, debug panels shown | Left click, mouse wheel | Pin, close and filter buttons; scroll the panel under the pointer |
| Flight | macOS Command+Q | Exit to desktop |
| Networked flight (not single player) | Backquote (~), the key above Tab | Open the chat line. It is not rebindable and only a flight joined to a host has it; single player never opens it, and its Controls screen row says so |
| Chat line open | Typed characters, Backspace | Type the line: printable ASCII, up to 80 characters. Every flight key you were holding is let go and the keyboard types instead of flying; the joystick and the mouse still fly |
| Chat line open | Tab / Shift+Tab | Choose the receiver: All, Friendlies, Enemies, Wing, Target, round (Shift+Tab goes back) |
| Chat line open | Enter | Send the line to the receiver and close the line (an empty line just closes). Closed, Enter designates the nearest visible aircraft as before |
| Chat line open | Esc | Close the line without sending |
| Chat line open | F1 to F12 | Send the matching line of `CHAT.TXT`, to the line's own receiver or the one chosen, and close the line. Closed, the F keys are the views |
| Live map open | + / - | Zoom the map |
| Live map open | Arrow keys | Pan the map |
| Live map open | Home | Follow the player again |
| Live map open | Esc | Close the map |
| Any menu | Arrow keys, Tab, Enter, Esc | Move, select and back out |
| Controls screen | Tab / Shift+Tab | Next / previous device |
| Controls screen | Delete or Backspace | Clear the focused primary or secondary input |
| Replays screen | Up / Down, PageUp / PageDown, Home / End, mouse wheel | Move through the recordings |
| Replays screen | Enter or double-click | Watch the selected recording |
| Replays screen | Delete or Backspace | Delete the selected recording, after a confirmation |
| Replays screen | Tab / Shift+Tab | Move through the list and the buttons |
| Replays screen | Esc | Close the auto-delete settings or the confirmation, otherwise back to the main menu |
| Head tracker | opentrack UDP on port 4242 | Turns the view; Center view makes the current head position straight ahead |
| Replay viewer | Space | Play or pause |
| Replay viewer | J / K / L | Play backwards / pause / play forwards; J or L again doubles the speed, up to 16x |
| Replay viewer | Up / Down | Next faster or slower speed in the same direction: 1/8x, 0.25x, 0.5x, 0.75x, 1x, 2x, 4x, 8x, 16x |
| Replay viewer | Left / Right | Back or forward 5 seconds; while paused, one tick; with Shift, 30 seconds |
| Replay viewer | Home / End | Start or end of the recording |
| Replay viewer | PageUp / PageDown | Previous or next timeline marker |
| Replay viewer | Tab / Shift+Tab | Next or previous aircraft |
| Replay viewer | F1 to F10, F12 | The flight views, on the selected aircraft; F6 again for the next wingman |
| Replay viewer | Alt / Ctrl + F1 to F10, F12 | The same views from the selected aircraft's target / its newest missile in flight; Alt+F4 still quits |
| Replay viewer | O / Shift+O | Object view: look at the next or previous aircraft, weapon or ground object present now |
| Replay viewer | Keypad 5 or Shift+/ | Recenter the look, keeping the view and zoom |
| Replay viewer | + / - (keypad too) | Zoom in or out, 0.5x to 4x |
| Replay viewer | Mouse wheel | Scroll a debug panel or menu under the pointer; otherwise configured drone speed controls (default wheel) from 20 to 5,000 feet per second, or zoom in a flight view |
| Replay viewer | Hold right button and drag | Look around in a flight view; the drone uses its configured look hold (default right button) |
| Replay viewer | Left click, drag on the timeline | Transport bar buttons; jump to or scrub through a moment |
| Replay viewer | Right-click without dragging | On the camera button, choose a specific view. In the scene, the debug menu on the aircraft, weapon or ground object under the pointer, with View from here and Look at this for the object view, or a list of aircraft to jump to |
| Replay viewer, debug menu open | Up / Down / Tab, Home / End, PageUp / PageDown, Enter / Space / Right, Esc / Left | Move, choose, close |
| Replay viewer | N / T / C | Name labels / mission timer / Comms panel on or off |
| Replay viewer | I / F / G | AI thinking / telemetry panel of the selected aircraft / guidance panel of its newest missile in flight, on or off |
| Replay viewer | M / X | Debug menu on the selected aircraft / close every debug panel |
| Replay viewer | R / Shift+R | Flight path trails on or off / next trail length (10, 30, 60, 120 or 300 seconds) |
| Replay viewer | H | Hide or show the whole interface and the pointer; playback and camera keys keep working |
| Replay viewer | P | Save the view, without the interface, as a PNG in `screenshots/` |
| Replay viewer | Esc | The pause menu, over the view even with the interface hidden: pauses playback; Esc again or Resume replay plays on as before, ? > End Replay goes back to the Replays screen, and the Control tab opens the controls screen |
| Replay viewer, pause menu open | Arrow keys, Tab, Enter / Space, Esc, left click | Move, choose and back out, as in the flight menu; every other key and click waits until it closes |
| Watching a networked mission (slice F2-O2) | The replay viewer's keys | As in the viewer; the bar reads LIVE at the live edge, Space, J and the scrub keys leave it, End returns to it, and nothing goes past live. Esc opens the pause menu, whose first row, Stop Watching, returns to the lobby |
| Watching your own aircraft while the AI flies it (slice F2-O3) | The stick, rudder, throttle and trigger | The controllers, the arrow keys (Shift makes them look), Z, X, End, Page Down, Space, 1 to 8 and any key your controls bind take the aircraft back and are not the viewer's keys on this screen (the arrows do not scrub, Space does not pause); every other key and the mouse are the viewer's. Stop Watching in the pause menu leaves the aircraft to the AI and goes to the lobby, or takes the aircraft back in a game with no lobby screen or one you host |

## Multiplayer phase 2

[Design](ARCHITECTURE.md#phase-2-the-rest-of-stage-f). John answered the key
questions on 2026-10-05 (all as recommended). **Built** (slice F2-C, listed in
the tables above and remappable like any other key):

| Situation | Input | Action |
| --- | --- | --- |
| Flight | U | IFF squawk on the displayed target: "IFF: Friendly" for one of the player's side, "IFF: no reply" for any other, "IFF: no target" with none (retail's key) |
| Flight | Ctrl+T | Show Target Info on or off, as the Pref menu's row: identities under visible aircraft and objects, a human's callsign beneath in a network game (retail's key) |
| Flight | K | Score board in a network flight, open or closed (built, F2-S): the players ranked by the game's tally with their side, kills, losses, damage and ratio, the sides' totals, the kill limit, the time left and the winner. Single player says "Score board: network games only" |
| Flight, as a wingman | Alt+Shift+E / W / B / H | Reply to the flight: Engaging, Winchester, Bingo fuel, or request help (built, F2-R). A wingman's call goes to every human of its flight as a radio line and recording ("Red two: Winchester"); you hear your own as YOU, other flights hear nothing, and radio silence drops it for a seat that has it on. A seat may call once in two seconds. A plane that leads its wing says "You lead this flight." (single player always does) |

**Proposed, not built:**

| Situation | Input | Action |
| --- | --- | --- |
| Networked flight, aircraft lost | Enter | Fly again, when the respawn rule, lives and delay allow (retail's key; slice F2-V) |
| Networked flight, the AI flying for the player | Any flight control | Take the aircraft back (slice F2-A) |

Stage G's keys are built: Alt+A sorts the flight's targets (slice G3c) and Alt+N monitors the battle net (slice G8; the [guide](DATALINK.md#frequencies)).
