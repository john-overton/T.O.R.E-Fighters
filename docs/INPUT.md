# Controllers and pilot input

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

This is an authored T.O.R.E input layer, not recovered retail controller dispatch.
Keyboard, gamepad, stick, throttle, pedals and button-box controls share typed
pilot actions. The original instruments and their scope operations are driven
through those same actions. This layer adds no external screen export, no window
rearrangement, no sensor behaviour of its own and no fabricated readings; the
shared [sensor component](radar.md) decides what the scopes show.

## Quick start

Every default key, mouse input and gamepad button is in the
[controls master list](CONTROLS.md). On Linux a controller exposing the standard
two-stick gamepad controls receives the following default mapping. The 8BitDo Ultimate 2 wireless controller's input
capabilities have been inspected on the development host, and the user confirmed
the Linux rumble test pulse works. Flight handling and disconnect checks remain open.

```sh
cargo run --locked -p tore-app -- --free-flight
cargo run --locked -p tore-app -- --list-inputs
cargo run --locked -p tore-app -- --monitor-inputs 30
```

| Standard Linux control | Default flight action |
| --- | --- |
| Left stick | Roll / pitch; pulling the stick back pulls the aircraft up |
| Right stick | Head-look / exterior orbit |
| Left / right trigger | Left / right rudder; equal pulls cancel |
| Left / right shoulder | Decrease / increase throttle while held |
| South / east face button (A/B on Xbox layout) | Gear / airbrake toggle |
| West / north face button (X/Y on Xbox layout) | Flaps / afterburner toggle |
| View (Select) / left-stick click | Combat modifier / forward cockpit view |
| Start | Pause/resume |
| Guide | Escape flight menu, if not intercepted by the desktop |
| Right-stick click | Recenter look, including a head tracker |
| D-pad left/right | Previous/next instrument selection |
| D-pad up/down | Selected instrument's first/second stock button |
| D-pad and south/east buttons in menus | Navigate and accept/back |

Shift+O toggles the F-22 main bays. The `bay` action is available in the controls
editor, text profiles and recorded input. Other aircraft ignore it.

Keyboard assignments continue working. Standard keyboard axes have priority over
controller axes while pressed. These gamepad defaults are convenience mappings,
not a claim about FA's original joystick layout. Back/paddle/extra buttons are
available only if firmware/driver exposes them as independent inputs. Devices
with different descriptors, including Windows/macOS raw controller numbering,
need a custom profile; no universal button-index layout is assumed.

`--no-controllers` disables native device discovery and feedback while retaining
keyboard controls and any custom keyboard bindings. It also allows deterministic
bridge tests without touching hardware.

## In-game controls and saved preferences

**Alt-Enter** switches between native borderless fullscreen and the previous
windowed size, on every screen: the menus, the Quick Mission creator, the locate
screen and flight. It is not rebindable and no controller button is assigned to
it. F11 is not used, because it already opens the in-flight keyboard help. The
game starts in borderless fullscreen; the choice is saved as `fullscreen` in
`preferences-v1.conf` and `--windowed` starts one run in a window without
changing it.

### The controls screen

Open **Pref → Controls...** on the main menu or **Escape → Control** in flight.
Both open the same input configuration screen; in flight the game stays paused.
The layout follows John's 2026-09-22 mockup; its details are an opinionated agent
design drawn with the imported raster font. Every default is listed in the
[controls master list](CONTROLS.md).

- **Devices on the left.** Keyboard, Mouse, each detected controller, stick,
  throttle, pedal set or button box, any disconnected device the profile still
  names, and Head tracker. The device type is guessed from its name and controls
  (a fitted rule): it only picks the tab title and which settings appear.
- **Settings at the top right**, per device. Controllers: rumble, modifier
  buttons, deadzone, stick sensitivity, trigger sensitivity and defaults for new
  gamepads. Sticks, throttles and pedals: modifiers, deadzone and sensitivity.
  Mouse: mouse look, sensitivity and invert. Head tracker: see
  [head tracking](#head-tracking-and-trackir). Deadzone and sensitivity apply to
  every axis binding on that device; sensitivity scales the axis output from 0.10
  to 1.00. Trigger sensitivity scales trigger (`trigger-*`) bindings the same way.
- **Mappings below**, grouped as Flight controls, Systems, Weapons, Sensors and
  instruments, View, Communication and Game and menus. Click a group header to
  fold it. Each action has a primary and secondary input; click one to capture
  a replacement and operate the control. Axis rows also have an invert box and a
  response curve button (1.0, 1.5, 2.0, 3.0). **Clear** removes the action's
  inputs on this device; Delete or Backspace clears only the focused input.
  Profile bindings for actions the screen does not list appear under
  **Other bindings**, where they can be cleared.
- **Keyboard remapping is complete.** Stock keys show as the primary input and can
  be changed or cleared. A key given to one action is taken from its previous
  action, and the message names it. Giving an action back its own stock key simply
  re-enables it. Esc, Alt+Enter and Alt+F4 cannot be taken.
- **Capture** picks the behaviour from the row: a stick or axis on an axis row,
  a button or D-pad direction on a direction row (for example Rudder left), a
  trigger moved on a direction row becomes a trigger contribution, and a trigger
  pushed on a button action (such as Fire) acts as a button once past halfway. A
  stick moved on a direction row is assigned to the whole axis row instead. Esc
  cancels capture.
- **Apply** validates and saves before replacing live bindings. **Reset device**
  restores the selected device's defaults in the draft. **Back** or Esc discards
  anything not applied. Arrow keys, Tab, Page Up/Down and Enter navigate; a
  controller's D-pad, A and B do too, except while capturing.

The editor saves the explicitly loaded `--input-profile` file, or `input-v1.conf`
in the [application data directory](DEVELOPMENT.md). It writes a canonical profile
(comments/formatting are not retained), using a temporary file and rename so a
failed write does not truncate the current file. Save errors appear in the editor;
invalid drafts do not replace either the file or live bindings.

**Default mappings for new gamepads** preserves automatic standard Linux mappings
when enabling rumble before connecting a pad. Turn it off for entirely explicit
profiles. The version-1 directive is `gamepad-defaults on|off`; absent means off
for existing custom profiles. Known device mappings already in the profile are
retained instead of appending another default set. With defaults on, removing all
bindings for a device lets it receive defaults again when reconnected.

macOS GameController capture explicitly uses shared `*` bindings because its
public identity is session-only. Native generic HID identities and Windows/Linux
identities keep their normal matching rules. The screen covers the current
binding model, not a calibration wizard, persistent Apple player assignment,
device firmware remapping or unsupported aircraft systems. Per-binding priority
and min/center/max calibration remain profile-file settings; the screen keeps
whatever the file holds.

### Modifiers

A modifier is a held control that switches its device to another layer of
bindings, like Ctrl or Shift on a keyboard. A device can have up to four. Select
the **Modifier buttons** row with a click, Enter or the controller's A button,
then press a button or D-pad direction: a new one is added, and one that is
already a modifier is removed. Left and right on the D-pad do nothing on that
row, so moving through the settings never starts a capture. Esc cancels. Any button
or a single D-pad direction can be a modifier, so D-pad left and D-pad right can
be two separate modifiers. While capturing, hold one or two modifiers and press
the control: `View + D-pad left + A` is a valid binding. When several held
modifiers match, the binding naming the most of them wins.

A declared modifier is dedicated: its own unmodified bindings stop acting, so
declaring D-pad left removes its Previous instrument and menu-left actions.
Standard gamepad defaults declare View (Select) as the combat modifier. Keyboard
modifiers are simply Ctrl, Alt and Shift in any combination, such as Ctrl+G,
Ctrl+Shift+G or Ctrl+Alt+G.

### Mouse look

Hold the right mouse button and drag to look around in flight. Sensitivity 1.0
turns the view 2 radians per 1,000 pixels of mouse travel (an opinionated agent
value); the range is 0.1 to 5.0. Mouse look obeys the same limits as the
keyboard and stick: in the cockpit you cannot look below the forward eye line.
Turn mouse look off to bind the right button to an action instead. The middle,
back and forward buttons and the wheel are bindable. The wheel zooms by default.
The left button always operates instruments and the HUD.

### Head tracking and TrackIR

The game listens for head poses on loopback UDP port 4242 in opentrack's "UDP
over network" format: 48-byte datagrams of six little-endian doubles, x/y/z in
centimetres then yaw/pitch/roll in degrees. The listener binds `127.0.0.1` only,
so it needs no firewall permission and accepts no remote sender. The head angle
is added to the player's look angle, the way the right stick and mouse look turn
the view, and the result obeys the same cockpit limit: you cannot look below the
forward eye line. **Center view** (Shift+/, or right-stick press) makes the
current head position straight ahead. Head position and roll are not used; zoom
stays on its own controls.

TrackIR's own software only sends data to games registered with NaturalPoint,
so T.O.R.E does not talk to it directly. Run [opentrack](https://github.com/opentrack/opentrack)
with its TrackIR input (or a webcam or phone tracker) and the "UDP over network"
output set to `127.0.0.1` port 4242. Trackers that appear as a joystick instead can
bind the **Head tracker yaw** and **Head tracker pitch** axis rows on that
device's tab; full axis travel maps to plus or minus 180 degrees of yaw and 90
degrees of pitch before the head sensitivity setting.

Head tracker settings: receiver on or off, yaw and pitch sensitivity (0.1 to
3.0) and inversion for each. The sign convention is fitted: opentrack's positive
yaw is taken as a turn to the left. It has not been checked against real
TrackIR hardware; use the invert settings if a direction is reversed. A port
already in use is reported in the Head tracker status line and on the console.

### Profile directives for these settings

```text
modifier DEVICE CONTROL         # up to 64 in total; CONTROL may be a D-pad direction
disable keyboard KEY            # a stock key the player removed
disable mouse wheel:up          # a stock mouse binding the player removed
mouse-look on|off               # default on
mouse-sensitivity 0.1..5        # default 1
mouse-invert on|off             # default off
head-tracker off|udp:PORT       # default udp:4242, ports 1024..65535
head-scale YAW PITCH            # default 1 1; 0.1..3, negative inverts
```

Existing profiles without these lines keep the defaults. Older builds cannot read
a profile saved with them.

Normal sessions also save `preferences-v1.conf`: large/small instrument page sets,
active layout/selection, scope settings, cockpit/HUD/ladder visibility, the
weapon diagnostic panel (`weapon-diagnostics`, off by default), HUD
brightness, zoom and music/effects. The file format is now **version 5**. The
retired `radar-mode` and `rwr-range` keys are gone (the RWR follows the shared
radar range), `rcs-range`, `radar-channel` and `radar-history` are present, and
`fullscreen` stores the window mode, so the exposure page's scale, the selected
sensor channel and the history toggle persist along with the shared scope range.
Version 1 to 4 files still load and drop a saved `rwr-range` after validating
it; they, and a version 5 file written before `weapon-diagnostics` existed, load
with the diagnostic panel hidden. Version 1 and 2 files also have their retired scope mode validated and
dropped, and their saved scope range migrates by its old nautical-mile value to
the nearest current setting, with equal distances choosing the lower one. That
maps 10 to 10, 20 to 25, 40 to 50, 80 to 100 and 160 to 150 nautical miles. An
old index is never reinterpreted as a different range. These persist across
launches, aircraft changes and flight restarts. Pause, head-look and aircraft state are not restored as user
preferences. Preference loading is silent; a malformed file is reported and
preserved. Smoke/capture/performance diagnostics ignore these display preferences
and do not write them, keeping existing visual probes reproducible. Explicit
instrument layout/page and zoom flags override saved values for normal launches.

Hybrid spin handling uses continuous pitch and rudder axis values for torque
and nose response, including fine deflections. John requested analog controls
as the baseline on 2026-09-16. Keyboard values feed the same flight model; they
do not define a separate all-or-nothing recovery law. Existing calibration,
deadzones and response curves remain user-configurable. See
[spin dynamics](spec/spin-transitions.md) for the fitted flight response.

## Profiles and calibration

Generate an editable profile without importing media or opening a window:

```sh
cargo run --locked -p tore-app -- --write-input-profile my-input.conf
cargo run --locked -p tore-app -- --input-profile my-input.conf --free-flight
```

The generator refuses to overwrite an existing file. It writes active standard
Linux gamepad bindings when the required capabilities are present and commented
suggestions for other controls. It does not infer unknown equipment functions.
Copy an edited profile to `input-v1.conf` in the application's data directory for
automatic loading; see [platform paths](DEVELOPMENT.md). `--input-profile` chooses
an explicit file. A custom profile replaces automatic controller defaults unless it enables
`gamepad-defaults on`;
existing keyboard flight/navigation shortcuts remain unless explicitly rebound.
Settings load at startup and can be edited in the [controls screen](#the-controls-screen).

Profiles use UTF-8 text, `#` comments and whitespace-separated tokens. The first
non-comment line must be `tore-input 1`. Limits: 256 KiB, 1,024 bindings and 64
aliases. Unknown actions, incompatible modes, invalid calibration and duplicate
aliases fail with a line number. Identity/control tokens contain no whitespace.

Example (replace the identity with the monitor's exact device ID):

```text
tore-input 1
rumble off
alias stick DEVICE_ID_FROM_MONITOR
bind stick axis:0 roll axis -1 0 1 0.08 1.5 1 10
bind stick axis:1 pitch axis -1 0 1 0.08 1.5 1 10
bind stick button:304 gear press
bind keyboard Ctrl-g gear press
```

A binding is:

```text
bind DEVICE CONTROL ACTION MODE [MIN CENTER MAX DEADZONE CURVE SCALE PRIORITY]
```

`DEVICE` is an exact device identity, an alias, `keyboard`, `mouse`, or `*` for
every native device (never the keyboard or mouse). Prefer aliases/exact identities when different equipment shares
axis numbers. Never use enumeration order for persistent assignments. USB serials
are preferred; serial-less devices fall back to physical connection identity and
may require rebinding when moved. Devices with no serial or physical identity use
a session node identity; those bindings may need updating after reconnection. Some
composite peripherals expose generic button-only interfaces, which remain
unassigned until explicitly bound. Windows IDs are local to that machine. The
macOS raw HID fallback uses location identity. Apple GameController endpoints use
`macos-gc-session-…` identities: the public API does not expose the HID serial used
by our raw backend. These IDs deliberately change between processes/reconnections;
never use device names or enumeration order as persistent physical identities.
Generated Apple-gamepad suggestions use `bind * …`, commented by default, to share
named controls across gamepads. Such bindings can persist across sessions but cannot
distinguish two identical gamepads; per-device persistent Apple-gamepad assignment
needs a later explicit player-assignment flow. Profiles are platform-local.

The monitor shows **raw native values**. Absolute axes are normalized using the
reported range to `-1..1` before profile calibration. Thus the default calibration
is `-1 0 1 0.08 1 1 10`, even when the monitor reports `0..65535`. Independent
negative/positive spans, inversion (`SCALE=-1`), dead zone and exponent curves are
supported. Unit throttle axes map the calibrated endpoints to `0..1`; their
center/deadzone/curve are not applied. Trigger modes map to a signed unipolar
contribution and do apply dead zone/curve. No extra temporal input filter is added
on top of the aircraft's existing model-owned response. Radial stick dead zones
and automatic calibration wizards are not implemented.

| Mode | Meaning / compatible actions |
| --- | --- |
| `axis` | Centered pitch, roll, yaw, throttle-rate, look-x or look-y |
| `unit` | Absolute throttle position with pickup |
| `positive` / `negative` | Button-held signed contribution to a centered action |
| `trigger-positive` / `trigger-negative` | Signed unipolar analog contribution; pairs add within a device/priority |
| `press` / `release` | One command on the selected edge; repeated reports do not retrigger |
| `hold` | Hold an equipment switch on while any assigned, armed source holds it |
| `switch` | Explicit equipment on/off when the physical contact changes; initial state does not actuate |
| `follow` | Continuously request the physical equipment setting, including at initialization/resume |
| `position=N` | One command on entering a discrete position; initial position does not actuate |
| `delta` | Encoder steps; throttle-rate means 1% per step; UI actions repeat for matching direction |

For a counterclockwise encoder UI binding use `SCALE=-1`; clockwise uses `1`.
Deltas are limited to 32 steps per event. Native discrete hats/selectors can use
`position=N`; multiple contacts are separate physical controls and are not
silently guessed to be one three-position selector. Device-specific neutral
combinations need explicit bindings or future reviewed composition support.

`eject` is a one-shot pilot action, normally bound with `press`. Press twice
within the [confirmation interval](spec/ejection.md#host-rules), releasing
between presses; holding or key repeat cannot confirm. It is not an on/off
switch and has no default gamepad button. The stock shortcut is Shift+E,
also available through the `key:Shift-e` shortcut alias.

Equipment actions: `gear`, `flaps`, `airbrake`, `hook`, `engine`, `burner`, `radar`,
`jammer`, `autopilot`, `waypoint-autopilot`. A and Ctrl-A toggle the two
[autopilot modes](spec/autopilot.md); controller bindings and pilot recordings
use the same simulation commands. `press` toggles; `switch`/`follow` request a setting. These request the
existing system behavior and never force animation fractions or bypass aircraft
capabilities. Rafale's unavailable hook stays unavailable. `throttle=0.75`
requests a preset. Axes are `pitch`, `roll`, `yaw`, `throttle`, `throttle-rate`,
`look-x`, `look-y`, and the absolute `head-yaw` and `head-pitch`. Mouse controls
are `button:right`, `button:middle`, `button:back`, `button:forward`, `wheel:up`
and `wheel:down`; each wheel notch is one press.

UI actions: `pause`, `menu`, `end-flight`, `restart`, `view-front`, `view-back`,
`view-up`, `view-external`, `view-track`, `view-threat`, `view-wing`,
`view-target`, `view-target-player`, `view-fly-by`, `view-missile`, `store-view`,
`view-target-track`, `center-look`, `cockpit`, `hud`, `zoom-in`, `zoom-out`,
`range-down`, `range-up`, `radar-mode`, `sensor-channel`, `sensor-infrared`,
`sensor-history`, `page-0` through `page-9`, instrument
commands below, and `menu-up/down/left/right/accept/back`. `sensor-channel` is an
alias of `radar-mode`; both cycle the available radar and infrared channels
rather than the retired cosmetic display mode. `sensor-infrared` requests the
passive channel and `sensor-history` toggles the scope contact trail. Existing
profiles that bind `radar-mode` keep working and now cycle channels. `key:Shift-0` or
`key:Ctrl-t` dispatches a stock flight shortcut through the same menu/availability
handler; it is a one-shot command, not a synthetic held keyboard key. Unsupported
systems still report unavailable. Bind continuous flight actions directly.
Custom keyboard controls use physical names such as `g`, `ArrowDown`, or
`Ctrl-Shift-g`; modifier order is Ctrl, Alt, Shift, Super. OS exit shortcuts should
remain on the keyboard.

## Shared assignments and interruption

Every binding tracks its own physical baseline and contribution. Releasing or
disconnecting one source never releases another source's held contribution.
Equipment `hold` combines sources with OR; `switch` emits on/off only on a real
change. A `follow` binding keeps physical authority over that equipment, including
a keyboard toggle; disconnecting it relinquishes authority and retains the last
setting. Use `switch` when keyboard/cockpit overrides should persist.

Centered analog actions choose the highest-priority active source. Equal-priority
ownership stays with the current source until it becomes neutral; ties on initial
acquisition use stable binding/device order. Signed button/trigger contributions
combine within a device and priority, then participate in the same arbitration.
This avoids jitter-driven last-event ownership. Independent axes can have different
owners. Opposing contributions cancel. Throttle uses pickup: after a preset,
rate adjustment, reconnect or resume, the physical lever must reach/cross the
current setting (4% tolerance) before taking over. It never snaps to zero on
unplug. When multiple absolute throttles are assigned, priorities determine the
eligible owner rather than averaging positions.

Menus and explicit/focus pause clear pending gameplay commands and held output.
Physical state remains tracked. Neutral buttons/axes rearm on resume; controls
still held must return to neutral/release first. Custom keyboard chords retain
the original release owner even after modifiers change. Menu buttons cannot leak
into flight as a new press. An actively contributing primary controller's
removal pauses flight; reconnecting does not resume it. A switch-only box removal
does not change latched equipment settings. Nonfinite samples and native/core queue overflow drop stale
commands and pauses rather than silently leaving controls stuck.

## Instrument selection without screen changes

`Ctrl-Tab` / `Ctrl-Shift-Tab` selects the next/previous existing instrument slot.
`Ctrl-1..6` selects a slot directly. `Ctrl-Shift-1..4` operates its four existing
button positions. These are authored shortcuts, not decoded retail bindings.

Controller actions are `instrument-next`, `instrument-previous`, `instrument-1`
through `instrument-6`, `control-1` through `control-4`, and direct actions such as
`instrument-2-control-1`. `page-N` toggles the existing instrument page. A brief
existing-style notice identifies focus; there is no raster alteration, window
movement or new screen content. Layout/page changes reset focus to the first
slot. Absent slots and unimplemented controls report unavailable. Scope
buttons are instantaneous commands, so there is no invented held sensor action.
RCS buttons 1/2 change its scale. RWR and radar buttons 1/2 change the shared
scope range (the RWR shows it capped at 50 miles, see the
[RWR specification](spec/rwr.md#range)). Radar button 3 cycles the available
sensor channels and button 4 toggles contact history. Buttons with no action on a page, and unsupported pages, remain
unavailable.

## Fixed ticks and input tapes

`tore-sim` receives a typed `PilotInput` for each 120 Hz tick: continuous demand
plus ordered one-shot commands. Device/key strings and calibration stay outside
simulation. Equipment commands and throttle presets now apply at tick entry;
actuator audio follows actual state changes. Existing aircraft response and
configuration remain separate. Camera look resolution cannot acquire flight-axis
or throttle ownership.

Native discovery/polling runs on a dedicated worker (4 ms service interval), with
bounded messages to the app (8 ms wake deadline when idle). Linux delivers ordered
evdev transitions at report boundaries; macOS generic devices use HID queues.
Windows raw controllers and Apple GameController profiles sample native current
state, so a complete pulse between samples can be missed. Once captured, short command press/release pairs survive until their
next simulation tick. Live input is admitted to the next available tick; this is
not a claim of identical wall-clock input sampling under arbitrary renderer
stalls. Paused gameplay edges are discarded, not played back at resume.

```sh
cargo run --locked -p tore-app -- --free-flight --record-input flight-input.txt
cargo run --locked -p tore-app -- --replay-input flight-input.txt
```

Pilot-only recording retains its clean-aircraft start, with no external stores,
to preserve existing replay initial conditions. Normal unrecorded free flight
loads supported default weapons. Use `--record-combat` for weapon-service tapes.

Recording requires a direct free-flight start without a capture, probe or initial control/device-pose override and stops when that flight ends/restarts. Files are create-new and flushed
on exit. Tapes contain **pilot inputs only**, not mission saves, UI/camera commands,
assets or initial state. Replay uses a fresh flight in the chosen theater; supply
the same aircraft, theater, model flag, assets and configuration as the recording.
The format is bounded to one hour/64 MiB with ordered ticks and finite values.
Fractional axes and ordered commands round-trip and reproduce identical states
under 30/60/144 Hz render schedules in synthetic tests. Cross-CPU bitwise flight
parity and whole-mission replay remain separate acceptance work.

## Feedback and native backends

`rumble off` is the default. `rumble on` enables authored event feedback on
connected rumble-capable devices with actual pilot/equipment bindings. Devices at
rest still receive feedback (including when the keyboard engages afterburner);
unassigned and UI-only devices do not. Targets are captured when an event is
accepted, so connection/reconnection alone never replays an effect.

| Confirmed event | Strong / weak | Nominal duration | Live producer |
| --- | --- | --- | --- |
| Gun fired | 8% / 16% | 67 ms | Hook only; gun simulation unavailable |
| Missile launched | 18% / 10% | 150 ms | Hook only; weapon release unavailable |
| Bomb released | 12% / 7% | 100 ms | Hook only; weapon release unavailable |
| Rocket launched | 10% / 16% | 83 ms | Hook only; weapon release unavailable |
| Turbulence | Up to 10% / 6%, scaled by severity | 125 ms | Hook only; no turbulence producer yet |
| Afterburner engaged | 6% / 10% | 150 ms | Actual inactive-to-active transition |
| Afterburner running | 3.5% / 1% | Continuous quiet bed | Actual active state; bounded renewable native leases |
| Damage | 25% / 18% | 167 ms | Hook only; combat damage unavailable |
| Crash | 35% / 20% | 183 ms | Actual transition into crashed state |

These are opinionated tactile designs, chosen by the implementation rather than
recovered from the original. They are not directional flight-stick forces. The
dependency-free `tore_input::feedback` mixer has eight
fixed slots and combines motor strengths by maximum, capped by these designs,
rather than adding overlapping effects. Gun/rocket repeats are admitted at most
20 Hz, turbulence/missile/bomb/damage at 10 Hz, afterburner at 2 Hz and crash at 1 Hz.
Mixer updates are limited to 20 per simulated second and catch-up ticks are
coalesced before native submission; small cues may be delayed up to 50 ms
behind another update. Each native pulse has a finite duration; stopping event
production lets the effect expire. No rumble alters authoritative simulation or RNG.

The app queues events after the 120 Hz flight tick, then advances the mixer once.
Afterburner feedback requires actual activation: merely setting its switch below
the model's throttle threshold, with no fuel, or with the engine off does not
produce a pulse. Raising throttle through that threshold with the switch armed
can activate it. While active, a quiet low-frequency bed continues beneath other
impulses, using 750 ms native leases renewed every 500 ms of simulated flight.
The engagement impulse is not retriggered by renewal. Disengaging afterburner
cancels its bed and remaining engagement pulse; unrelated effects may finish.
A stalled/exited producer cannot leave an infinite native effect. Pause/focus
loss and flight teardown clear pending impulses/cooldowns as well as native effects.

Future weapon systems must call `Input::feedback` with the typed event after a
successful shot/release, never for dry fire, an unavailable control or a held fire
key alone. A future turbulence producer must supply finite normalized severity;
steady wind, turns, G-load and stall are not substituted for turbulence. These
hooks do not implement weapons, loadouts, damage or turbulent flight dynamics.

```sh
cargo run --locked -p tore-app -- --test-rumble only
# Or select an exact persistent ID from --list-inputs (Linux/Windows):
cargo run --locked -p tore-app -- --test-rumble DEVICE_ID_FROM_LIST_INPUTS
```

`only` completes discovery, then requires exactly one rumble-capable controller;
it fails instead of picking the first when several are present. Use it for Apple
gamepads whose session ID changes between CLI runs. The test does not require
`rumble on`, retail media, or a display. It requests a 200 ms pulse at 20% strength.
Native API acceptance and
physical response are distinct; the latter needs a human check. Feedback requests
are bounded to 1–2,000 ms and per-device deadlines; stop requests cannot be blocked
by a full effect queue. Pause, focus loss, overflow, shutdown and worker drop
stop effects and discard pending stale requests. Reconnect never replays them.

| Platform | Implementation / limits |
| --- | --- |
| Linux | evdev capabilities and report boundaries, nonblocking reads, SYN_DROPPED state recovery, FF_RUMBLE. USB serial plus interface identity when available. No exclusive grabs or system permission changes. |
| Windows | Built-in Windows.Gaming.Input RawGameController and Gamepad vibration through `windows` 0.58 bindings. Dedicated WinRT worker initialization/teardown; device removal and read failure stop vibration before releasing the endpoint. Current-reading polling; custom mappings required. |
| macOS | On macOS 11+, supported gamepads use GameController profiles and controller-created CoreHaptics engines. Apple's `supportsHIDDevice:` suppresses duplicate HID input. Other joystick/multi-axis/button-box devices retain IOKit HID queues, with no generic HID rumble. Custom mappings required. |

Apple gamepad controls are `gc-axis:HEX_NAME`, `gc-button:HEX_NAME` (native
pressed state) and `gc-pressure:HEX_NAME` (0..1 raw pressure, normalized as an axis).
Names are UTF-8 bytes encoded as hex so spaces cannot break a profile. Pressure
controls support analog triggers independently of digital button edges. Copy the
control token from the monitor/generated profile; do not bind both a pressure
control and its digital counterpart to the same one-shot action.

macOS uses separate left/right handle localities when both are advertised, with
strong/weak mapped to intensity and low/high sharpness respectively. Otherwise
the default locality receives `max(strong, weak)` at medium sharpness. This is an
authored haptic approximation, not identical motor frequencies across hardware.
Effects have finite native duration as well as the worker deadline. Replacement,
interruption and teardown stop retained players/engines; errors discard the engine
and never automatically replay a pulse. Engines are created only when feedback is
requested. Native cold starts can delay the input worker; no haptic call blocks
the renderer/simulation thread. macOS 11+ is the supported haptics configuration;
older macOS linking/runtime is not validated.

Only `tore-input-native` permits audited unsafe FFI. Linux adds the already-used
`libc` platform bindings; Windows adds features to the already-locked `windows`
0.58 dependency. macOS adds narrowly enabled `objc2-game-controller` and
`objc2-core-haptics` 0.3.2 bindings and `block2` 0.6.2, reusing already-locked
Objective-C/Foundation/dispatch bindings. Input policy remains hand-rolled;
`tore-input` is dependency-free and safe Rust. All other workspace crates retain
`unsafe_code = forbid`. No SDL, third-party controller runtime, USB driver,
retail executable, or device-specific output protocol is embedded or executed.

Native API references: [Linux event protocol](https://docs.kernel.org/input/event-codes.html),
[Windows raw controller reading](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.input.rawgamecontroller.getcurrentreading),
[Apple HID elements](https://developer.apple.com/documentation/iokit/1588671-iohiddevicecopymatchingelements),
[Apple controller haptics](https://developer.apple.com/documentation/gamecontroller/gccontroller/haptics),
[haptic localities](https://developer.apple.com/documentation/gamecontroller/gcdevicehaptics/createengine(withlocality:)),
[HID support query](https://developer.apple.com/documentation/gamecontroller/gccontroller/supportshiddevice(_:)),
[Windows vibration](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.input.gamepad.vibration).
See [acceptance and open hardware checks](baselines/input.md).

## Manual combat layer, 2026-09-14

New standard Linux gamepad profiles reserve **Select as a held combat modifier**.
Press Select first, then the action control. Release the action before switching
layers; a held control cannot become a new action when Select is pressed/released.
Old saved explicit profiles retain their assignments; use the editor to add the
new bindings or regenerate a profile deliberately. No saved file is overwritten.

| While holding Select | Keyboard equivalent / action |
| --- | --- |
| Right shoulder | Space: hold fire/release |
| Left shoulder | ] next NAV/weapon |
| South (A) | T: next radar target |
| East (B) | L: clear designation |
| West (X) | [ previous NAV/weapon |
| North (Y) | J: own jammer toggle |
| Left-stick click | R: radar toggle |
| Right-stick click | K: selected external group jettison |
| D-pad up | Backslash: replace range target |
| D-pad down | Explicit `damage-player` developer fixture; keyboard D reports damage |
| D-pad left | Next damage class fixture |
| D-pad right | Selected station failure fixture |
| Start | Shift-Y: target jammer fixture |
| Guide | Shift-I: one incoming selected source weapon fixture |

The `radar` equipment action behaves exactly like the keyboard R: while the
infrared channel is selected it returns the scope to radar, and otherwise it
toggles radar power. Bind `sensor-channel` or `sensor-infrared` to select a
channel without touching the power switch.

Guide may be intercepted by the desktop; bind `incoming` to another exposed
button or combo in **Escape → Control** when necessary. No desktop shortcuts are
changed. Unmodified Start still pauses; unmodified flight buttons, shoulders,
rudder triggers and instrument navigation retain their functions. External view
uses F10 or a custom `view-external` binding; Select is no longer its default.
No AI makes the incoming launch decision. Fixtures/jettison require `--live-fire`.

Profile chord syntax is `MODIFIER+CONTROL` or `MODIFIER+MODIFIER+CONTROL`, for
example:

```text
modifier pad button:314
modifier pad axis:16=-1
bind pad button:314+button:311 fire hold
bind pad button:314+button:304 designate press
bind pad button:314+axis:17 range-target position=-1
bind pad button:314+axis:16=-1+button:304 flaps press
```

Two or three distinct nonempty tokens are required. A token may be a virtual
button over a physical control: `axis:16=-1` is pressed while that hat or D-pad
axis reads exactly -1, and `axis:5>0` or `axis:1<-0.5` is pressed while the
normalized axis is past that threshold. Virtual buttons work as modifiers and as
bound controls. Keyboard and mouse bindings take no chords; keyboard shortcuts
keep their `Ctrl-`/`Alt-`/`Shift-` syntax. `fire` requires **hold** behavior;
press/release or axis modes are rejected instead of silently doing nothing.
Capture in the controls screen records held modifiers automatically. Exact
Windows/macOS device IDs/control names still need an explicit platform profile.

Combo actions suppress the base control, including throttle and menu actions.
Pause/focus changes, disconnect, layer transitions and reset cancel fire;
physical neutral/release is required before rearming. Keyboard and controller fire
are independent, so releasing one cannot cancel a trigger held by the other.

## Weapon haptic envelopes

Rumble remains opt-in: the controller tab's Rumble setting, then Apply. Cues are authored,
not native force-feedback recovery. Strong/weak motor amplitudes are normalized:

| Confirmed event | Duration | Strong / weak |
| --- | ---: | ---: |
| Gun representative shot (sustained fire refreshes) | 67 ms | 0.08 / 0.16 |
| Own missile launch | 150 ms | 0.18 / 0.10 |
| Bomb release | 100 ms | 0.12 / 0.07 |
| Player-owned bomb impact confirmation | 200 ms | 0.08 / 0.04 |
| Rocket launch | 83 ms | 0.10 / 0.16 |
| Actual player damage | 167 ms | 0.25 / 0.18 |
| Player destruction/crash | 183 ms | 0.35 / 0.20 |

Gun/missile/player-damage/destruction producers are connected. Bomb/rocket cues
are tested mixer contracts; the two PT defaults contain no bombs/rockets and
there is no runtime producer or enabled alternative loadout for those cues yet.
Remote target hits and incoming fixture launches do not masquerade as ownship
haptic events. A defeated incoming contact causes no damage impulse.

Nine fixed slots mix by maximum amplitude, never additive escalation; native
requests are capped at 20 Hz and use finite leases. Repeats have cooldowns.
Pause, focus loss, restart, profile replacement, disconnect and shutdown stop
feedback. Unsupported devices continue silently. A native API failure disables
feedback on that device until reconnect and reports one explanation. Native
worker deadlines and bounded requests guard against stuck effects. Actual motor
strength and comfort still require physical controller acceptance.

## Additional aircraft capabilities

F-14D, A-4E and X-31 EFM use the existing input bindings. Device commands respect
capabilities: A-4E has no burner, and X-31 has no hook. No thrust-vectoring input
is implemented. See [aircraft behavior](spec/additional-aircraft.md) and
[validation](baselines/aircraft-fa-expansion.md).

`--flight-throttle 0..1` sets the initial throttle for engine-material captures;
see the [engine material contract](spec/engine-material.md).

Researched flight is now the default; `--legacy-flight` preserves the previous
model. HUD and audio share the [stall warning signal](spec/stall-warnings.md),
including the original imported warning samples.

## Missile seeker control

Radar power OFF disables radar-missile bore and tones; IR remains independent; weapon selection permits a permanently
unguided DUMB release. Selecting passive IR alone does not switch power off.
Surface weapons cannot use the A2A bore toggle; surface designation remains deferred.
Armed independent air-to-air missiles automatically enter BORESIGHT when radar power is on and no target is
selected. IR also supports bore with radar power off. A selected track takes
priority for IR and forces CUED acquisition against that identity, even when a
stronger bore return exists. Clear the track to return to BORESIGHT; airborne
missiles keep their own targets. Select a target to return to CUED. Press **L**, the existing
`clear-designation` action, or click **RELEASE LOCK** in the upper-right weapon
diagnostic panel when it is shown, to clear
both sensor and HUD display selection. A selected target outside the HUD has a
direction chevron; the Easy targeting cheat keeps it after sensor coverage is
lost, which grants no weapon lock. [HUD target rules](spec/gunsight-targeting.md). The manual's targeting list
does not establish a retail release key.
`weapon-seeker-mode` remains rebindable but has no default key; the diagnostic
panel's mode label is clickable only while the panel is shown (**Escape → Pref →
Weapon diagnostics?**). Supported radar weapons still need aircraft lock.

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
arrow with a crosshair while the pointer is over its plotting area.

Imported IR and radar search/lock samples provide the cues, with a louder lock
cue. Their assignment is fitted. Effects mute, pause, safe, empty and failed
stations silence them. `TORE_SEEKER_VOLUME=0..1` sets maximum amplitude, default
0.30. Re-import media to add the four samples to an older cache.
[Rules and constants](spec/missiles.md), [validation](baselines/hud-cleanup.md).

## Player wing orders

These host shortcuts are opinionated agent choices. Orders address friendly
wing 1 in a live AI Quick Mission. Alt-0 selects the whole flight; Alt-4 through
Alt-7 select wingmen 1 through 4. Restart restores whole-flight addressing.
Unavailable recipients or targets produce explicit messages. Paused flight does
not issue orders. The imported flight menu has no wing-order submenu.

Both sides start in neutral formation, even with free-fire objectives. Your
wingmen wait for your engagement commands while continuing radar scans and
missile defense. AI flight leaders issue engagement orders in response to
perceived attacks on their flight or protected aircraft.

| Shortcut | Order |
| --- | --- |
| Alt-B / Alt-R | Break left / right |
| Alt-H / Alt-V / Alt-T | Break high / low / fly straight |
| Alt-E | Engage the designated target |
| Alt-P | Protect me, maintain an escort duty |
| Alt-W | Attack on contact |
| Alt-F | Engage designated target from formation, medium control |
| Alt-D | Disengage and return to neutral formation |
| Alt-1 / Alt-2 / Alt-3 | Return to formation: echelon / line abreast / line astern |
| Alt-8 | Toggle 512 / 2048 ft horizontal spacing |
| Alt-K | Cycle level / 512 ft high / 512 ft low stacking |
| Alt-C | Toggle loose / medium control |
| Alt-U | Bug out: return to base and stop answering orders |
| Alt-L | Land at the airport selected with Shift-A |
| Alt-Shift-B / R / H / V | Approach the designated target from left / right / high / low |
| Alt-0 / Alt-4 through Alt-7 | Address all wingmen / one wingman |

Input profiles can use these as `key:Alt-b`, `key:Alt-8`,
`key:Alt-Shift-b` and the corresponding keys above. Alt-S remains the unimplemented
original radio-silence shortcut; it is not repurposed for spacing.

The manual's wingman table puts bug out on Alt-B. T.O.R.E keeps Alt-B as break
left, and John chose Alt-U for bug out and Alt-L for land at selected airport
on 2026-09-23. Land at selected airport has no retail equivalent; it is an
opinionated addition John requested the same day. Bug out sends each addressed
wingman to its own home runway, the departure runway after a ground start or
else the nearest friendly or neutral airport. A wingman with no known base
stays and is counted in the reply. Land at selected airport sends the addressed
wingmen to the airport the tower has selected (Shift-A): the runway you are
cleared for there, or else its longest usable runway. Hostile, unknown or
unpermitted neutral airports and airports with no usable runway are refused
with a message. A bugged-out wingman no longer answers any order; later orders
skip it and say so. Both orders print the call ("Bug out", "Land at Field")
without voice, because the reviewed radio catalog has no recording for them.
Bug out is ignored on the ground, during takeoff, or from landing marshal
onward. It is accepted during the route home, as in retail; the reply counts it. Landing wingmen report holding
at marshal, landing and landed. While you approach a friendly airport with the
gear down, below 4,000 ft and within 25,000 ft, AI aircraft for that airport
hold at marshal until you are down and clear
([player priority](spec/airports.md#wing-landing-orders-and-player-priority)).

The message gives applied, rejected and no-motion counts. An explicit attack target must be alive,
hostile and present in each recipient's own radar or visual contacts. Synthetic
headless actors without sensors retain their explicit direct-awareness fallback.
Protect me needs no selected or currently detected attacker.
The first living wingman alone replies to an accepted engage/protect assignment.
Commands take effect immediately, independently of their radio recordings.
A new command interrupts queued old command audio. Sound off mutes radio along
with effects; an independent radio-traffic preference remains unimplemented.
Missing recordings or an old cache leaves commands and text operational.
Reimport user-owned media to load [verified radio mappings](formats/radio.md).

Line abreast alternates right/left at one spacing, then right/left at two
spacings, preserving the echelon sides. Routine formation, spacing and stacking
changes now use controlled repositioning from each aircraft's current location.
Aircraft coordinate conflicting paths and can establish aft clearance before
moving inward. A new order replaces the pending path; real collision danger
still permits breakout. Harder turns can therefore interrupt a transition.
Normal vertical wandering now requests at most five feet, with smooth changes.

Formation selection and disengage cancel the engagement and let the safe rejoin
procedure return the aircraft. They remain neutral until a new engagement order
or a newly perceived attack; an already-known missile warning cannot restart
the pursuit. Evasion continues when needed. Spacing and stacking alone do not
cancel or authorize combat. Approaches assign the
selected target and continue that engagement after reaching the fitted approach
point. Protect me assigns a persistent escort duty: wingmen assess detected
hostiles near or approaching your aircraft and respond to shared attack reports.
They return when the escort pursuit limit is reached.
[Behavior and limits](spec/ai-awareness.md#mission-roles-and-rules-of-engagement).

For live testing, use a Quick Mission with at least three friendly aircraft:

1. Issue Alt-D, then change formation, spacing and stacking. Confirm smooth
   physical repositioning and one player call, without generic wingman replies.
2. Select wingman 2 with Alt-5 and issue a break. Confirm wingman 1 keeps its
   assignment. Alt-0 restores whole-flight orders.
3. Designate a detected enemy and issue Alt-E, then immediately Alt-D. Confirm
   accepted recipients disengage and stale “Engaging” audio does not follow it.
4. Try an absent or unobserved target, pause, mute sound, and restart. Confirm
   no false acknowledgment, paused order or replayed old radio.
5. Repeat a diving reversal. Confirm separate safe rejoins, restrained status
   reports and no aircraft pose jumps. A request for steady flight is advisory.
## Airport commands

Normal ground starts enter NAV with weapons disarmed. Airborne starts select
and arm the canonical gun. Bracket keys cycle NAV and weapons; NAV keeps
target cues while suppressing weapon readouts. [Layout and defaults](spec/hud-layout.md).

The controls editor exposes `airport-nav`, `airport-next`,
`airport-request-landing`, `airport-repeat`, and `airport-cancel`. The default
profile binds them to Shift-N, Shift-A, Shift-L, Shift-R, and Shift-C in that
order. NAV mode, gear down, range, and airport-relative altitude govern automatic
ILS guidance; the threshold must also be within the aircraft's 90-degree
forward cone. Outside the 5-NM/4,000-foot-above-airport band, even ILS ARM is
hidden. A clearance is not required to display eligible guidance. Airport selection remains
explicit when supplied, otherwise the nearest usable runway is selected. A
successful landing request and its repeat play the reviewed retail clear-to-land
recording. Landing completion plays the reviewed welcome-home recording, and
repeat then replays that welcome. Other
tower replies remain text only.

## Quick Mission ground start

In the creator, set **Start** to **Ground**, then choose **Airport**. Continue
through the normal loadout screen. The player starts on that runway with engine
idling, gear/flaps down and brakes applied. **B** releases brakes; use the normal
throttle and flight controls to take off. The player's AI wingmen queue on the taxiway
and wait until the player is airborne before entering the runway. Startup
takeoff clearance and wing departure/landing reports are automatic; Alt-S
suppresses routine wing reports but keeps player clearance. Other
wings start airborne at the displayed wing altitude. Restart restores the accepted airport/start.
Airborne remains the default. Ground start requires the researched flight model;
legacy and restricted native modes remain available for airborne starts.

Shift-M toggles the live map. Escape closes it, plus/minus zoom, arrows pan and
Home resumes following the player. Map navigation takes priority over keyboard
flight bindings for those keys while open. Other flight controls remain live.
Right-side map buttons toggle categories, with Buildings off by default.
Selections last until flight restart; no category settings appear in Escape. See the
[map specification](spec/flight-map.md).

Quick Mission inline options support right-click to cycle backward, wrapping to
the last available value. Left-click retains forward cycling and the existing
list pickers. The player's wing never cycles below one. Right-click requires a
matching press/release and cannot launch, cancel or select through a modal list.
The ordnance view retains right-click quantity decrement.

In Load Ordnance, drag a catalog weapon onto a compatible station to load it.
Its picture follows the pointer. Drag a loaded station to another station to
transfer ammunition, or back into the catalog area to empty it. Escape cancels
a drag. Left-click a loaded station to add one, up to its capacity. Click an
empty station to load the selected catalog weapon. Empty stations retain a red
outline. See the
[drag and quantity rules](spec/ordnance-presentation.md#dragging-and-empty-stations).
Quick Mission's Guns only restriction applies to every friendly and enemy wing
and remains active after restart.

View 4 target-camera fields and status meanings are described in the
[flight controls guide](FLIGHT-CONTROLS.md#target-camera-view-4).

Quick Mission wing skill menus include **Dummy (400 KTS)** for straight-flying
training targets. See [behavior](spec/dummy-aircraft.md).

## NAV and weapon selection

`[` and `]` cycle backward/forward through NAV and weapons. Selection controls
arming; U and semicolon have no action. Old `master-arm` profile entries are
accepted but do nothing. The editor offers `weapon-next` and `weapon-previous`.
NAV INFO minus/plus select destinations; button 3 switches mission/airport mode.
WEAPONS minus/plus select NAV/weapons; button 3 pages the store list.
IR boresight growl follows the actively tracked bore target and its displayed
percentage without designation. Empty bore and lost tracks are silent. A radar
missile in boresight sounds its lock tone on the bore return without
designation, cued seekers sound only while tracking, and radar tones fall
silent inside minimum range.
[Behavior and current route limitations](spec/weapon-navigation-selection.md).

## Ownship damage report

D reports current aircraft damage and system readings through the bottom-center
sim log. The rebindable `damage-report` action does the same. Damage notifications
share that log; the Systems instrument retains its four gauges and two fuel rows.
The separate `damage-player` fixture remains a development action, including
existing controller profiles. See [systems behavior](spec/systems-damage.md).

## Quick Mission group objectives

Click the highlighted objective in a friendly or enemy group's briefing sentence
to choose its primary group, free fire or other duty. Right-click cycles
backward. Click its survival field to toggle required/optional. Tab/Shift-Tab
and arrow keys reach all objective and survival fields; Enter activates them.
Shift-4 shows only `Obj: Survive` for a protected/required friendly or `Obj:
Destroy` for a designated enemy objective. Other contacts have no objective
label. [Assignment rules](spec/ai-awareness.md#quick-mission-objective-stamps).

## Ejection

Press **Shift+E twice** to eject. The first press asks for confirmation; holding
the key does not confirm. The eject action can be rebound in Controls. A living
pilot can escape an already destroyed airborne aircraft. The camera follows
the pilot and parachute while the abandoned aircraft falls independently.
Low or inverted escapes can be fatal. See [ejection behaviour](spec/ejection.md)
for timing, survival and the AI recovery assessment. Undamaged AI aircraft above
200 feet AGL never eject automatically, as requested by John on 2026-09-23.

## Flight view shortcuts

F1/F2/F3 remain Forward/Back/Up and F10 remains External. F4 tracks the target;
F5 faces the nearest inbound missile; F6 faces a wingman; F7 faces the target
from the player; F8 faces the player from the target; F9 is a fixed fly-by;
F12 follows the last player missile toward its own target. V saves the current
camera into Other View and opens Shift+3. F11 still opens keyboard help.

Alt+view selects a target reference and Ctrl+view a last-missile reference.
These are separately rebindable catalog commands (`key:Alt-F7`, for example).
Alt+F4 remains protected Exit; `view-target-track` is available without a default
binding. Unmodified view keys restore the normal reference. Missing subjects
produce feedback without changing the requested selection's predecessor.
Automatic views control direction; pan/orbit remains available in F1/F2/F3/F10.
See [view behavior and fitted rules](spec/flight-views.md).
