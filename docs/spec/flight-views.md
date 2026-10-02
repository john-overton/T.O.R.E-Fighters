# Flight views

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-23. John requested the remaining retail views,
function-key bindings and keyboard documentation. Camera behavior is spec-derived;
placement, subject fallback and selection details below are fitted agent choices.
No simulation, sensor permission or autonomous decisions change.

Second pass, 2026-09-28, from the player reports in GitHub issue #1, requested
by John: F2 and F3 turn with the aircraft, F2 shows the airframe, F6 cycles
wingmen, the target views keep a target within visual range, and F7 gains a
bearing compass. Sensor selection and weapon permission are unchanged.

Third pass, 2026-09-28, requested by John: the fly-by moves on once the
subject is 3 nmi away, Back from another aircraft shows that aircraft's
airframe, and the [mission replay viewer](../REPLAYS.md#cameras) gets the
same views (see [Mission replay](#mission-replay)).

## Evidence

The local 1999 EA/Jane's Fighters Anthology manual, printed pages 89 and 103-104
(PDF pages 93 and 107-108), describes Other View, all eleven views and reference
modifiers. Pages were rendered and read, because text extraction loses the F-key
and modifier glyphs. PDF SHA-256:
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
Source: `.local/missile-update/manual.pdf` in the original development checkout.
The existing reviewed `FMENUD.MNU` importer supplies the View menu. This is manual
evidence, not an observed executable run; no executable build establishes camera
placement or timing. Retail comparison is unavailable.

## Player-visible behavior

| Default key | View |
| --- | --- |
| F1 | Forward cockpit, resets look and zoom |
| F2 | Back, looking over the spine and tails from the pilot's seat |
| F3 | Up, toward the canopy roof |
| F4 | Track the current target within head-rotation limits |
| F5 | External view of the player, facing the closest inbound missile |
| F6 | External view of the player, facing a wingman; press again for the next |
| F7 | External view of the player, facing the current target, with a bearing compass |
| F8 | External view of the current target, facing the player |
| F9 | Watch the player fly past a fixed world position; a new one once the player is 3 nmi from it |
| F10 | External player view, with orbit controls |
| F12 | External view of the last player missile, facing that missile's target |

Alt plus a view key references the selected target instead of the player. Ctrl
references the last player-launched missile. An unmodified view key restores the
normal reference. In the mission replay the same modifiers work from the
selected aircraft: Alt its target, Ctrl its newest missile. Alt+F4 remains the protected desktop exit shortcut; target-relative
tracking can be rebound in Controls. F11 remains TORE keyboard help. These two
host compatibility decisions are opinionated agent choices.

Shift+arrows pan forward/back/up views and orbit external view, as page 104
specifies. Ctrl+arrows is FA's thrust vectoring and does not look. Keypad 5
recenters ([keyboard](keyboard.md)). Mouse, controller and head
look retain the same limits. Automatic tracking, relation and fly-by cameras
control their own direction. Zoom remains 0.5x to 4x. Selecting a view snaps immediately and resets
look and zoom; Shift+/ centers without changing either the mode or zoom.

V saves the current view, reference, pan and zoom into Other View and opens
Shift+3's window. It keeps following the chosen relation as objects move. Its
initial view is Back, as page 89 specifies. A stored fly-by keeps its fixed
position. Saving Forward is supported as a convenience. The window remains a
scene camera without a second cockpit overlay.

## Fitted camera rules

- Back and Up turn about the aircraft's own axes, never the world's: Back
  faces the tail around the aircraft's vertical axis, and Up tilts 0.8 radians
  (about 46 degrees) above the nose around the wing axis. Climbing lowers the
  rear view, banking tilts its horizon the opposite way, and the Up view stays
  with the aircraft through a loop or barrel roll without flipping. Before
  2026-09-28 both adjusted world angles, which reversed the rear view and
  flipped the Up view past vertical.
- Back looks from the pilot's eye, 7 feet above and 10 feet ahead of the model
  origin (the rear-view mirrors' eye), and draws the player's airframe, so the
  spine and tails are in view. The default Other View is Back and shows it too.
  F1, F3 and F4 still omit the airframe. External sits 180 feet behind and 60
  feet above the subject. Back from another aircraft (Alt's target, or any
  aircraft in the mission replay) does the same from that aircraft's pilot's
  eye and draws its airframe; a missile reference's Back stays at the missile
  and hides it.
- Tracking uses the subject's attitude, full horizontal rotation and elevation
  clamped to 0..90 degrees relative to its eye line, matching existing head look.
  The cockpit/HUD remain tied to the player's nose. Remote forward/up/track
  views omit the player's cockpit and hide the reference aircraft or missile.
  Ground-reference eye positions use the object's origin; ground interiors are
  not modeled.
- Relation cameras sit 180 feet behind the first subject along the line toward
  the second subject and 60 feet above it, facing along that line. Missile
  relation cameras use 30 feet behind and 10 feet above. They show both subjects
  when field of view and terrain permit; there is no automatic zoom.
- Fly-by sets its position once per selection: three seconds of current velocity
  ahead, 300 feet to the subject's right and 100 feet above. It keeps that world
  position and turns toward the moving subject. Press F9 again for another pass.
  Once the subject is more than 3 nmi (18,228 feet, straight line in three
  dimensions) from that position, the view picks a new one by the same rule,
  so a long fly-by keeps passing instead of watching a dot. John asked on
  2026-09-28 for a reset after "like 3-4 miles"; the 3 nmi figure is an
  opinionated agent choice. A saved fly-by in Other View moves on by the same
  rule, independently of the main view's.
- F6 chooses the first living airborne member by wing/member order in the same
  friendly wing as the player (wing 1). Pressing F6 again, with the same
  reference, moves to the next member and wraps back to the first; a message
  names the wingman by radio callsign ("Wingman view: Red two"). If the
  followed wingman is lost, the view moves to the next member in order, and
  returns to F1 only when none remain. Target-relative F6 uses that target's
  wing; missile-relative F6 uses its owner's wing. No eligible member is invented.
- F5 chooses the nearest live non-gun projectile aimed at the reference subject,
  measured in three-dimensional feet. The player's explicit incoming fixture
  also qualifies. This view does not grant sensor locks or weapon support.
- The last player missile is the highest launched projectile identity observed
  during the flight, excluding guns and incoming fixtures. Its expiration does
  not switch the view back to an older missile. Alt/Ctrl references in mission replay follow the
  selected aircraft's missiles by the same rule; unmodified replay F12 cycles
  all active missiles by stable id, advancing when the selected one expires; another aircraft's shots at
  the player count. F12 uses its retained target,
  independent of the player's later designation. Without a retained target,
  it looks along the missile's flight direction.
- Missing subjects at selection leave the current view untouched and explain
  what is missing. If an active subject disappears, return to F1 once with a
  message. Missing subjects in Other View clear its image. Coincident positions
  use the subject's forward direction. Paused simulation keeps camera subjects
  and fly-by positions still. Restart clears references and saved views.

Unknown: retail distances, fly-by placement/reposition policy, exact head limits,
the retail wingman cycling order, missing-subject behavior, and how compound reference modes resolve
relations. Here modifiers replace the player endpoint; target-relative F12 follows
the target's last live missile and missile-relative F12 follows the player missile.
Next research: bounded review of the original camera selection and placement
handlers, only if closer tuning is wanted. These gaps do not block fitted views.

## Target views and visual range

Without the Easy targeting cheat, a target drops when the radar or infrared
scope loses it ([selection rules](radar.md#target-selection-keys)). The target
views are the exception, requested by John on 2026-09-28: F4, F7 and F8, and
their Alt references, keep following a dropped target while it is within visual
range, in any direction. Visual range is the search range of the aircraft's
visual sensor, 10 nmi for every imported aircraft; the sensor's viewing cone,
its blind spot behind the tail and terrain masking do not apply, so a target
behind or below the player stays (John, 2026-09-28, after the first version cut
out too soon). A pilot who is dead or whose visual sensor has failed has no
visual range. Once the target is beyond it the views lose it for good, and
coming back inside does not restore it; a new selection is needed. The
selection itself, radar support, missile guidance, the HUD square and the
Shift-4 target window still drop at once. Easy targeting keeps the target for
the views as before. Mission recordings keep this view target, and a replay's
target views on the player follow it ([cameras](../REPLAYS.md#cameras)).

## F7 bearing compass

Opinionated addition, requested by John on 2026-09-28. F7 draws a compass strip
at the top of the screen, in the HUD's color and font, styled after the HUD
heading strip: a tick every 10 degrees with two-digit labels, a caret at the
center and a three-digit readout below it. The strip stays centered on the
compass bearing from the player to the target, so the caret and readout always
show the target's direction; there is no separate target marker. It spans the
middle half of the screen, about 80 degrees either side, and narrows to stop
short of the top instrument windows. The compass shows only in F7 with the
player as the reference, only while the view has a target, and never over the
Shift-M map. It leaves with the target. Layout and scale are agent choices.

The existing View transitions preference and Other View numeric instrument
overlays are outside this camera-selection pass.

## Mission replay

The [replay viewer](../REPLAYS.md#cameras) uses these views with any recorded
aircraft as the reference, and matches the game: F6 again moves to the next
wingman with the same message, F2 shows the aircraft's airframe, Alt and Ctrl
pick the target and missile references, F7 carries the compass, and keypad 5,
Shift+/ and the zoom keys work as in flight. It differs in one way John
chose on 2026-09-28: a view whose subject is missing says why once and shows
the aircraft from outside, then recovers when the subject returns, instead of
refusing the key or returning to F1. Playing backwards, the fly-by point is
placed ahead of the reversed motion (an agent choice). The replay also has an
object view from any object to any other, described there.
