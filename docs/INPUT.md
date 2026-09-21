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

On Linux a controller exposing the standard two-stick gamepad controls receives
the following default mapping. The 8BitDo Ultimate 2 wireless controller's input
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
| Select / left-stick click | Combat modifier / forward cockpit view |
| Start | Pause/resume |
| Guide | Escape flight menu, if not intercepted by the desktop |
| Right-stick click | Recenter look |
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

Open **Escape → Control** during flight. This authored replacement for the old
input-device submenu uses the existing raster font and paused menu canvas.

- Set **Rumble: On**, then select **Save & apply**. No text editor is required.
- Choose a binding with Left/Right on **Binding**, or use **Add binding**. Select
  its action, then **Capture key / button / axis** and operate the desired control.
  Escape cancels capture; it does not become a binding. Keyboard modifiers are
  retained. Controller menu actions are suppressed while the editor is open.
- Device/Input rows also let you select an exposed control without capture.
  Behavior, dead zone, curve, inversion, priority and normalized min/center/max
  are editable. Behavior choices are filtered to those the selected action supports.
- Use Up/Down to select a row, Left/Right to change values, Enter to activate;
  mouse clicks on the left arrow decrement and elsewhere increment/activate.
- **Save & apply** validates and saves before replacing live bindings. Escape or
  **Back** discards changes since the last save. Removing a custom keyboard binding
  restores the stock shortcut; it does not disable the stock keyboard table.
  Shared assignments remain explicit; assigning an input does not delete another
  action assigned to it. Actions remain subject to current aircraft capabilities.

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
public identity is session-only. The editor reports this sharing. Native generic
HID identities and Windows/Linux identities keep their normal matching rules.
The editor covers the current binding model, not a calibration wizard, persistent
Apple player assignment, device firmware remapping or unsupported aircraft systems.

Normal sessions also save `preferences-v1.conf`: large/small instrument page sets,
active layout/selection, scope settings, cockpit/HUD/ladder visibility, HUD
brightness, zoom and music/effects. The file format is now **version 3**. The
retired `radar-mode` key is gone, and `rcs-range`, `radar-channel` and
`radar-history` are added, so the exposure page's scale, the selected sensor
channel and the history toggle persist along with the radar scope setting.
Version 1 and version 2 files still load: their retired scope mode is validated
and dropped, and a saved scope range migrates by its old nautical-mile value to
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
Settings load at startup and can be edited through Escape → Control.

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

`DEVICE` is an exact device identity, an alias, `keyboard`, or `*` for every
native device. Prefer aliases/exact identities when different equipment shares
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

Equipment actions: `gear`, `flaps`, `airbrake`, `hook`, `engine`, `burner`, `radar`,
`jammer`, `autopilot`, `waypoint-autopilot`. A and Ctrl-A toggle the two
[autopilot modes](spec/autopilot.md); controller bindings and pilot recordings
use the same simulation commands. `press` toggles; `switch`/`follow` request a setting. These request the
existing system behavior and never force animation fractions or bypass aircraft
capabilities. Rafale's unavailable hook stays unavailable. `throttle=0.75`
requests a preset. Axes are `pitch`, `roll`, `yaw`, `throttle`, `throttle-rate`,
`look-x`, `look-y`.

UI actions: `pause`, `menu`, `end-flight`, `restart`, `view-front`, `view-back`,
`view-up`, `view-external`, `center-look`, `cockpit`, `hud`, `zoom-in`, `zoom-out`,
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
RWR and RCS buttons 1/2 change their range. Radar buttons 1/2 change the scope
setting, button 3 cycles the available sensor channels and button 4 toggles
contact history. Buttons with no action on a page, and unsupported pages, remain
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
| Left shoulder | Semicolon: next weapon |
| South (A) | T/Enter: designate |
| East (B) | L: clear designation |
| West (X) | U: master arm/safe |
| North (Y) | J: own jammer toggle |
| Left-stick click | R: radar toggle |
| Right-stick click | K: selected external group jettison |
| D-pad up | Backslash: replace range target |
| D-pad down | D: explicit player-hit fixture |
| D-pad left | `]`: next damage class fixture |
| D-pad right | `[`: selected station failure fixture |
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

Profile chord syntax is `MODIFIER+CONTROL`, for example:

```text
bind pad button:314+button:311 fire hold
bind pad button:314+button:304 designate press
bind pad button:314+axis:17 range-target position=-1
```

Two distinct nonempty controls are required. Keyboard shortcuts retain their
existing `Ctrl-`/`Shift-` syntax. `fire` requires **hold** behavior; press/release
or axis modes are rejected instead of silently doing nothing. The editor lists
combat actions and standard Select combinations in its Input row. Capture remains
single-control capture; choose the combo through Input or edit the profile. Exact
Windows/macOS device IDs/control names still need an explicit platform profile.

Combo actions suppress the base control, including throttle and menu actions.
Pause/focus changes, disconnect, layer transitions and reset cancel fire;
physical neutral/release is required before rearming. Keyboard and controller fire
are independent, so releasing one cannot cancel a trigger held by the other.

## Weapon haptic envelopes

Rumble remains opt-in in Control → Rumble → Save & apply. Cues are authored,
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

Radar power OFF disables radar-missile bore and tones; IR remains independent; master arm permits a permanently
unguided DUMB release. Selecting passive IR alone does not switch power off.
Surface weapons cannot use the A2A bore toggle; surface designation remains deferred.
Armed independent air-to-air missiles automatically enter BORESIGHT when radar power is on and no target is
selected. IR also supports bore with radar power off. A selected track takes
priority for IR and forces CUED acquisition against that identity, even when a
stronger bore return exists. Clear the track to return to BORESIGHT; airborne
missiles keep their own targets. Select a target to return to CUED. Press **L**, the existing
`clear-designation` action, or click **RELEASE LOCK** at the upper right to clear
both sensor and HUD display selection. A selected target outside the HUD has a
direction chevron even after sensor coverage is lost; this grants no weapon
lock. [HUD target rules](spec/gunsight-targeting.md). The manual's targeting list
does not establish a retail release key.
`weapon-seeker-mode` remains rebindable, and the upper-right mode label remains
clickable. Supported radar weapons still need aircraft lock.

BORE uses a five-degree circular half-angle. Its blinking diamond marks a
provisional contact, not a guaranteed lock; the blinking triangle on the range
scale refers to that same contact. Selection favours the centre while retaining
signal-strength weighting. IR can acquire on the rail; active radar acquires only
after release. The bare percentage is a fitted estimate, not a calibrated retail percentage.
Short retail weapon labels, bore circle, estimate and readiness all move with
the forward HUD when looking around. Range, closure and estimated flight time
and target aspect angle are in the upper-right debug window. The HUD layout
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

| Shortcut | Order |
| --- | --- |
| Alt-B / Alt-R | Break left / right |
| Alt-H / Alt-V / Alt-T | Break high / low / fly straight |
| Alt-E | Engage the designated target |
| Alt-P | Protect me, assign a currently observed attacker |
| Alt-W | Attack on contact |
| Alt-F | Engage designated target from formation, medium control |
| Alt-D | Disengage and stop selecting targets |
| Alt-1 / Alt-2 / Alt-3 | Echelon / line abreast / line astern |
| Alt-8 | Toggle 512 / 2048 ft horizontal spacing |
| Alt-K | Cycle level / 512 ft high / 512 ft low stacking |
| Alt-C | Toggle loose / medium control |
| Alt-Shift-B / R / H / V | Approach the designated target from left / right / high / low |
| Alt-0 / Alt-4 through Alt-7 | Address all wingmen / one wingman |

Input profiles can use these as `key:Alt-b`, `key:Alt-8`,
`key:Alt-Shift-b` and the corresponding keys above. Alt-S remains the unimplemented
original radio-silence shortcut; it is not repurposed for spacing.

The message gives applied, rejected and no-motion counts. A target must be alive,
hostile and present in each recipient's own radar or visual contacts. Synthetic
headless actors without sensors retain their explicit direct-awareness fallback.
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

Formation selection changes the slot setting; disengage stops the engagement
and lets the safe rejoin procedure return the aircraft. Approaches assign the
selected target and continue that engagement after reaching the fitted approach
point. Protect me currently assigns a detected attacker once, rather than
maintaining a persistent escort policy. [Behavior and limits](spec/ai.md#live-wing-command-and-radio-integration).

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

Normal ground starts enter NAV with master arm SAFE. Airborne starts select
the canonical gun with master arm SAFE. Weapon cycling leaves NAV; NAV keeps
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
throttle and flight controls to take off. Other selected aircraft start airborne
at the displayed wing altitude. Restart restores the accepted airport/start.
Airborne remains the default. Ground start requires the researched flight model;
legacy and restricted native modes remain available for airborne starts.
