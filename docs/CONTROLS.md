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
| Rudder left | Z | - | LT |
| Rudder right | X | - | RT |
| Throttle lever | - | - | - |
| Throttle rate (axis) | - | - | - |
| Throttle up | Page Up | - | RB |
| Throttle down | Page Down | - | LB |
| Throttle idle | - | - | - |
| Throttle 10% | 1 | - | - |
| Throttle 20% | 2 | - | - |
| Throttle 30% | 3 | - | - |
| Throttle 40% | 4 | - | - |
| Throttle 50% | 5 | - | - |
| Throttle 60% | 6 | - | - |
| Throttle 70% | 7 | - | - |
| Throttle 80% | 8 | - | - |
| Throttle 90% | 9 | - | - |
| Throttle 100% | 0 | - | - |
| Afterburner | Shift+B | - | Y |
| Autopilot (heading/altitude) | A | - | - |
| Waypoint autopilot | Ctrl+A | - | - |

### Systems

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Landing gear | G | - | A |
| Flaps | F | - | X |
| Airbrake / wheel brakes | B | - | B |
| Tailhook | H | - | - |
| Engine on/off | E | - | - |
| Weapon bays (F-22) | Shift+O | - | - |
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
| Clear designation | L | - | View + B |
| Seeker mode (bore/cued) | - | - | - |
| Jettison selected stores | K | - | View + Right stick press |
| Reset range target | Backslash | - | View + D-pad up |
| Incoming missile (range) | Shift+I | - | View + Xbox |
| Target jammer (range) | Shift+Y | - | View + Menu |
| Next damage class (test) | - | - | View + D-pad left |
| Fail station (test) | - | - | View + D-pad right |
| Damage player (test) | - | - | View + D-pad down |

### Sensors and instruments

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Radar power / radar channel | R | - | View + Left stick press |
| Jammer (ECM) | J | - | View + Y |
| Cycle sensor channel | M or O | - | - |
| Infrared channel | I | - | - |
| Contact history | Y | - | - |
| Scope range down | Comma | - | - |
| Scope range up | Period | - | - |
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
| Look left/right | - | Hold right button and drag | Right stick X |
| Look left | Shift+Left or Ctrl+Left | - | - |
| Look right | Shift+Right or Ctrl+Right | - | - |
| Look up/down | - | Hold right button and drag | Right stick Y |
| Look up | Shift+Up or Ctrl+Up | - | - |
| Look down | Shift+Down or Ctrl+Down | - | - |
| Head tracker yaw | - | - | - |
| Head tracker pitch | - | - | - |
| Center view | Shift+/ | - | Right stick press |
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
| Wing: break left | Alt+B | - | - |
| Wing: break right | Alt+R | - | - |
| Wing: break high | Alt+H | - | - |
| Wing: break low | Alt+V | - | - |
| Wing: fly straight | Alt+T | - | - |
| Wing: engage my target | Alt+E | - | - |
| Wing: protect me | Alt+P | - | - |
| Wing: attack on contact | Alt+W | - | - |
| Wing: engage from formation | Alt+F | - | - |
| Wing: disengage | Alt+D | - | - |
| Wing: echelon | Alt+1 | - | - |
| Wing: line abreast | Alt+2 | - | - |
| Wing: line astern | Alt+3 | - | - |
| Wing: spacing | Alt+8 | - | - |
| Wing: stacking | Alt+K | - | - |
| Wing: loose/medium control | Alt+C | - | - |
| Wing: approach target left | Alt+Shift+B | - | - |
| Wing: approach target right | Alt+Shift+R | - | - |
| Wing: approach target high | Alt+Shift+H | - | - |
| Wing: approach target low | Alt+Shift+V | - | - |
| Address whole flight | Alt+0 | - | - |
| Address wingman 1 | Alt+4 | - | - |
| Address wingman 2 | Alt+5 | - | - |
| Address wingman 3 | Alt+6 | - | - |
| Address wingman 4 | Alt+7 | - | - |
| Airport NAV / ILS | Shift+N | - | - |
| Next airport | Shift+A | - | - |
| Request landing | Shift+L | - | - |
| Repeat tower reply | Shift+R | - | - |
| Cancel approach | Shift+C | - | - |

### Game and menus

| Action | Keyboard | Mouse | Gamepad (Xbox) |
| --- | --- | --- | --- |
| Flight menu / back | Esc | - | Xbox |
| Pause | Ctrl+P | - | Menu |
| Time compression | C | - | - |
| End mission | Ctrl+Q | - | - |
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
| Flight | Left click | Operate instrument buttons, designate a scope contact, click the HUD seeker label or RELEASE LOCK |
| Flight | Hold right button and drag | Mouse look, when enabled on the Mouse tab |
| Flight | macOS Command+Q | Exit to desktop |
| Live map open | + / - | Zoom the map |
| Live map open | Arrow keys | Pan the map |
| Live map open | Home | Follow the player again |
| Live map open | Esc | Close the map |
| Any menu | Arrow keys, Tab, Enter, Esc | Move, select and back out |
| Controls screen | Tab / Shift+Tab | Next / previous device |
| Controls screen | Delete or Backspace | Clear the focused primary or secondary input |
| Head tracker | opentrack UDP on port 4242 | Turns the view; Center view makes the current head position straight ahead |
