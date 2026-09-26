# Keyboard

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research specification, 2026-09-26, research mode. Build: reviewed FA.EXE 1.02F,
SHA-256 `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`.
Evidence is static disassembly of the in-flight key handling, cross-checked
against the in-flight menu accelerators in `FMENUD.MNU` and the Jane's US Navy
Fighters manual. Nothing was run and no retail session was observed.

This file records what the original's keys do. Where T.O.R.E places each
command, and why some differ, is in [input](../INPUT.md#key-placement); every
current default is in the [controls list](../CONTROLS.md).

## How keys are read

The game reads the physical key, not the character, and ignores whether it is
a keypad or navigation-cluster key. The keypad therefore works as the
navigation cluster whatever NumLock says: keypad 0 is Insert, keypad period is
Delete, keypad 1 is End, keypad 3 is Page Down, keypad 8, 2, 4 and 6 are the
arrows, and keypad plus and minus zoom. Keypad 5 has its own meaning.
Shift with a letter is a separate command from the letter. Ctrl and Alt
combinations are always separate commands.

**Confidence.** *Confirmed* means the handler shows a message, sound or state
name that identifies it. *Inferred* means the handler's effect is clear but not
named.

## Flight

| Key | Action | Confidence |
| --- | --- | --- |
| Arrows | Stick, when the stick device is Keyboard | Confirmed |
| End / Page Down | Rudder left / right, when the rudder device is Keyboard | Confirmed; direction inferred |
| 1 / 2 / 3 / 4 / 5 | Throttle 0 / 25 / 50 / 75 / 100 percent | Confirmed |
| 6 | Throttle 101 percent: afterburner | Confirmed |
| 7 / 8 | Throttle down / up 5 percent | Confirmed |

Key 8 is not capped: from 96 percent or more it passes 100, which lights the
afterburner while the throttle shows 100. Any setting at or below 100 turns the
afterburner off, so 7 from afterburner gives 96 percent.
| A | Autopilot on or off (`Autopilot enabled` / `disabled`) | Confirmed |
| B | Speed brake; on the ground it also brakes the wheels | Confirmed |
| F / G / H | Flaps / gear / tail hook | Confirmed |
| O | Bomb-bay doors | Confirmed |
| Shift+E, twice | Eject; the first press only warns | Confirmed |
| Ctrl + arrows | Thrust vectoring, on aircraft that have it (`VCTR`) | Confirmed |
| 0 | Thrust vectoring back to neutral | Confirmed |
| Z / X | Wing sweep forward / aft, limited per aircraft | Inferred |
| Shift+Z / Shift+X | Wing sweep to the stops | Inferred |

The keyboard throttle and rudder keys work only while Keyboard is the chosen
throttle or rudder device in the Control menu.

## Weapons and countermeasures

| Key | Action | Confidence |
| --- | --- | --- |
| Space | Fire the selected weapon | Inferred |
| Tab | Fire the gun, whatever weapon is selected | Confirmed |
| [ / ] | Previous / next weapon | Confirmed |
| Insert / Delete | Release one chaff / one flare ([countermeasures](countermeasures.md)) | Confirmed |
| Enter | Target the next visible object, left to right | Confirmed |
| ' (apostrophe) | Target the visible object nearest the screen center | Confirmed |
| ; (semicolon) | Clear the current target; radar and seekers stay on | Confirmed |
| / and \\ | Designate the next visible object, or the one nearest the center, as the IR/laser target; only with **IR/Laser advanced targeting?** on | Confirmed |
| T / Shift+T | Next / previous radar target | Inferred |
| Shift+K | Jettison air-to-ground ordnance | Confirmed |
| Shift+J | Jettison external fuel (`External fuel jettisoned`) | Confirmed |
| Ctrl+A / Ctrl+Z / Ctrl+X | Nearest bandit / friendly / ground target | Confirmed |

Ctrl+A, Ctrl+Z and Ctrl+X are the find-nearest-objects cheat that the Multi
menu can allow.

## Sensors and navigation

| Key | Action | Confidence |
| --- | --- | --- |
| R | Air-to-air radar | Confirmed |
| Shift+R / Ctrl+R | Air-to-ground radar | Confirmed |
| I / M | Infrared / HARM seeker on or off | Confirmed |
| J | Jammer on or off | Confirmed |
| Y | Radar history | Confirmed |
| , / . | Radar range down / up | Inferred; direction from the manual |
| U | IFF interrogation of the target | Confirmed |
| N | Force the HUD into navigation mode (`NAV`, or `ILS` near a landing aid), hiding weapon cues; off, the HUD follows the selected weapon | Confirmed |
| W / Shift+W | Next / previous waypoint, wrapping at the ends; nothing without a waypoint list | Confirmed |
| Shift+A | AWACS air-to-air radar link on or off (`Supplemental air-air radar link on.`) | Confirmed |
| Shift+G | Air-to-ground radar link on or off | Confirmed |
| D | Damage report (`% OF MAX DAMAGE`) | Confirmed |
| Shift+D | Show the recent message history again | Confirmed |
| Shift+F | Target damage (`%s is %d%% destroyed`) | Confirmed |
| Shift+I | Airbase aircraft inventory | Confirmed |

## Cockpit, views and game

| Key | Action | Confidence |
| --- | --- | --- |
| F1–F10, F12 | Views, as the View menu lists them | Confirmed |
| Ctrl / Alt + view key | The same view from the last missile / the target | Confirmed |
| Shift + arrows | Pan the view (plain arrows when a joystick flies) | Confirmed |
| Keypad 5 | Recenter the view while held (Shift+keypad 5 when the keyboard flies) | Confirmed |
| = / - | Zoom in / out (`View zoom: %d%%`) | Confirmed |
| Shift+1 … Shift+0 | Instrument windows 1 to 10 | Inferred |
| Shift+' / Shift+; | Zoom the high-altitude bombing window in / out, six steps | Confirmed |
| Shift+[ / Shift+] | Dim / brighten the HUD | Confirmed by the menu |
| Backspace | Cockpit on or off | Confirmed by the menu |
| V | Move the current view into the Other View window (Shift+3) and return to the front view | Confirmed |
| Shift+M | Map | Inferred |
| C / Shift+C | Time compression 1×, 2×, 4×, 8× in turn / slow motion | Confirmed |
| Ctrl+P / Ctrl+Q | Pause / end mission | Confirmed |
| Ctrl+V | Ride of the Valkyries on or off | Confirmed |
| Ctrl+T | Target information | Confirmed by the menu |
| Ctrl+F | Frame rate | Confirmed |
| Alt+S | Radio silence | Confirmed |
| Esc / Alt+F4 | Menu / exit to Windows | Confirmed |

## Wingman orders

| Key | Order | Confidence |
| --- | --- | --- |
| Alt+1 | Steady, straight and level | Confirmed |
| Alt+2 / Alt+3 | Break left / right | Confirmed |
| Alt+4 / Alt+5 | Break low / high | Confirmed |
| Alt+6 / Alt+7 | Approach target left / right | Confirmed |
| Alt+8 / Alt+9 | Approach target low / high | Confirmed |
| Alt+E | Engage my target; also sets loose formation | Confirmed |
| Alt+R | Engage my target from formation; also sets at least medium formation | Confirmed |
| Alt+F | Engage the IR/laser-designated target | Confirmed |
| Alt+W | Engage every target of my target's class (`Attack bandits`, `Attack SAMs`, ...) | Confirmed |
| Alt+P | Clear my six | Confirmed |
| Alt+D | Disengage | Confirmed |
| Alt+B | Bug out | Confirmed |
| Alt+T | Cycle formation: echelon, line abreast, line astern | Confirmed |
| Alt+C | Loose / medium formation | Confirmed |
| Alt+H | Tighten up / combat spread | Confirmed |
| Alt+V | Formation high / level / low | Confirmed |

Every order goes to the whole flight; there is no key to address one wingman.
The approach orders (Alt+6 to Alt+9) first send the engage order and add the
approach only when a hostile target exists.

## Keys the original leaves free

In flight these do nothing: plain E, K, L, P, Q and S; Shift with B, H, L, N,
O, P, Q, S, U, V or Y; the shifted characters `<`, `>`, `?` and `|`; F11;
Alt+0; Home, Page Up and keypad `*`. Home and Page Up (keypad 7 and 9) work
only in the mission editor.

## Unknown

- Which keyboard keys, if any, change the Control menu devices. The menu
  itself does.
- The in-flight effect of Alt+Enter, Ctrl+C and the Windows keys.

## Source notes

Key messages arrive in the window procedure at `0x411600`, which builds a code
from the scan code (lParam bits 16–22, extended bit dropped) and a modifier
byte (Shift 1, Ctrl 2, Alt 4). Unmodified and Shift-only codes pass through the
translation table at `0x4ecc00`, so printable keys become characters and
Shift+letter becomes the capital letter. The in-flight dispatcher is
`0x414690`. The keyboard throttle and rudder are device callbacks
(`0x417c20`, `0x417d10`): keys 1 to 6 store 0, 25, 50, 75, 100 and 101, and
7 and 8 subtract or add 5 from the current value without a cap; `0x451b00`
caps the throttle at 100 each frame and lights the afterburner above it. Wingman
keys send radio messages through `0x4180a0` (type 0xB, addressee 0x8001, the
whole flight); their text was matched against the phrase table at `0x4ff170`
and the attack builder at `0x48d894`. Radar links print from `0x48ea10`; V
calls `0x4385e0`; the Valkyries score is entry 8 of the music table at
`0x4f47a0`. Chaff and flare are in [countermeasures](countermeasures.md).
