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

The [keyboard map](tore-keyboard-map.html) is the printable flight, comms and
view reference. Its [update conventions](tore-keyboard-map-rules.md) keep test
commands out of that reference.

The tables below are generated from `crates/tore-app/src/input_catalog.rs` and
the standard gamepad defaults in `crates/tore-app/src/input.rs`. A test fails
when they drift. After changing a default, regenerate with:

```sh
TORE_UPDATE_CONTROLS_DOC=1 cargo test --locked -p tore-app controls_doc
```

Gamepad defaults apply automatically to standard Linux gamepads. On Windows and
macOS, and for sticks, throttles and pedals, bind controls in the controls
screen. "View + RB" means hold View, then press RB. A dash means no default.

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
| Rudder left | End or Z | - | LT |
| Rudder right | Page Down or X | - | RT |
| Throttle lever | - | - | - |
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

### Systems

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Eject (press twice) | Shift+E | - | - |
| Landing gear | G | - | A |
| Flaps | F | - | X |
| Airbrake / wheel brakes | B | - | B |
| Tailhook | H | - | - |
| Engine on/off | E | - | - |
| Weapon bays (F-22) | O | - | - |
| Damage report | D | - | - |

### Weapons

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Fire / release weapon | Space | - | View + RB |
| Next weapon / NAV | ] | - | View + LB |
| Previous weapon / NAV | [ | - | View + X |
| Next radar target | T | - | View + A |
| Previous radar target | Shift+T | - | - |
| Select visual target | Enter or Apostrophe | - | - |
| Clear designation | Semicolon or L | - | View + B |
| Seeker mode (bore/cued) | - | - | - |
| Release chaff | Insert | - | View + D-pad left |
| Release flare | Delete | - | View + D-pad right |
| Jettison selected stores | Shift+K | - | View + Right stick press |
| Reset range target | Backslash | - | View + D-pad up |
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
| Scope range down | Comma | - | - |
| Scope range up | Period | - | - |
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
| Address whole flight | Alt+0 | - | - |
| Address wingman 1 | Alt+Shift+1 | - | - |
| Address wingman 2 | Alt+Shift+2 | - | - |
| Address wingman 3 | Alt+Shift+3 | - | - |
| Address wingman 4 | Alt+Shift+4 | - | - |
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
| Replay viewer | F1 to F10, F12 | The flight views, on the selected aircraft |
| Replay viewer | Backquote (`) | Drone camera following the selected aircraft, then flying free, then back to the flight view |
| Replay viewer | W A S D, E / Q | Drone: move, climb / descend; hold Shift for four times the speed |
| Replay viewer | Mouse wheel | Scroll a debug panel or menu under the pointer; otherwise drone speed from 20 to 5,000 feet per second, or zoom in a flight view |
| Replay viewer | Hold right button and drag | Look around, or turn the drone |
| Replay viewer | Left click, drag on the timeline | Transport bar buttons; jump to or scrub through a moment |
| Replay viewer | Right-click without dragging | The debug menu on the aircraft or missile under the pointer, or a list of aircraft to jump to |
| Replay viewer, debug menu open | Up / Down / Tab, Home / End, PageUp / PageDown, Enter / Space / Right, Esc / Left | Move, choose, close |
| Replay viewer | N / T / C | Name labels / mission timer / Comms panel on or off |
| Replay viewer | I / F / G | AI thinking / telemetry panel of the selected aircraft / guidance panel of its newest missile in flight, on or off |
| Replay viewer | M / X | Debug menu on the selected aircraft / close every debug panel |
| Replay viewer | R / Shift+R | Flight path trails on or off / next trail length (10, 30, 60, 120 or 300 seconds) |
| Replay viewer | H | Hide or show the whole interface and the pointer; playback and camera keys keep working |
| Replay viewer | P | Save the view, without the interface, as a PNG in `screenshots/` |
| Replay viewer | Esc | Show the interface if it is hidden, otherwise back to the Replays screen |
