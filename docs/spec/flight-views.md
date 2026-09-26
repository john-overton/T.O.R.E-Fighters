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
| F2 | Back, looking over the tail |
| F3 | Up |
| F4 | Track the current target within head-rotation limits |
| F5 | External view of the player, facing the closest inbound missile |
| F6 | External view of the player, facing a wingman |
| F7 | External view of the player, facing the current target |
| F8 | External view of the current target, facing the player |
| F9 | Watch the player fly past a fixed world position |
| F10 | External player view, with orbit controls |
| F12 | External view of the last player missile, facing that missile's target |

Alt plus a view key references the selected target instead of the player. Ctrl
references the last player-launched missile. An unmodified view key restores the
normal reference. Alt+F4 remains the protected desktop exit shortcut; target-relative
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

- Existing F1/F2/F3/F10 placement stays intact. Back adds 180 degrees of yaw;
  Up adds 0.8 radians. External sits 180 feet behind and 60 feet above the subject.
- Tracking uses the subject's attitude, full horizontal rotation and elevation
  clamped to 0..90 degrees relative to its eye line, matching existing head look.
  The cockpit/HUD remain tied to the player's nose. Remote forward/back/up/track
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
- F6 chooses the first living airborne member by wing/member order in the same
  friendly wing as the player (wing 1). Target-relative F6 uses that target's
  wing; missile-relative F6 uses its owner's wing. No eligible member is invented.
- F5 chooses the nearest live non-gun projectile aimed at the reference subject,
  measured in three-dimensional feet. The player's explicit incoming fixture
  also qualifies. This view does not grant sensor locks or weapon support.
- The last player missile is the highest launched projectile identity observed
  during the flight, excluding guns and incoming fixtures. Its expiration does
  not switch the view back to an older missile. F12 uses its retained target,
  independent of the player's later designation. Without a retained target,
  it looks along the missile's flight direction.
- Missing subjects at selection leave the current view untouched and explain
  what is missing. If an active subject disappears, return to F1 once with a
  message. Missing subjects in Other View clear its image. Coincident positions
  use the subject's forward direction. Paused simulation keeps camera subjects
  and fly-by positions still. Restart clears references and saved views.

Unknown: retail distances, fly-by placement/reposition policy, exact head limits,
wingman cycling, missing-subject behavior, and how compound reference modes resolve
relations. Here modifiers replace the player endpoint; target-relative F12 follows
the target's last live missile and missile-relative F12 follows the player missile.
Next research: bounded review of the original camera selection and placement
handlers, only if closer tuning is wanted. These gaps do not block fitted views.

The existing View transitions preference and Other View numeric instrument
overlays are outside this camera-selection pass.
