# Target camera window

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. View 4 presents the selected target using the
original instrument pieces and font, with a grayscale scene behind dark text.

## Evidence and behavior

The local 1999 EA/Jane's Fighters Anthology manual, printed page 101 (PDF page
105), specifies type/callsign, activity, clock bearing with Hi/Lo, damage,
tactical goal and pilot skill. PDF SHA-256:
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
The page was rendered and inspected from `.local/missile-update/manual.pdf`.
John's two supplied screenshots corroborate layout, the mission-objective label,
and NM/KTS fields. Their executable build identity is unknown. They do not
establish timing or hidden behavior. John requested a three-second alternation
on 2026-09-21, an opinionated timing choice.

- Type appears at the top; the camera shows the selected object's live geometry.
- John requested on 2026-09-21 that the target fill the view along the player's
  sight line, with the camera between player and target and no farther than
  one nautical mile from the target. The camera moves along the complete
  player-to-target line, including elevation, and faces the target. Automatic
  magnification fits the target regardless of range or aspect.
- John requested on 2026-09-21 a background about 10% darker. After grayscale
  conversion, scenery (sky, terrain and clouds) uses 90% of its former brightness.
  Aircraft and static objects retain their brightness, as do text and the frame.
- John requested on 2026-09-21 a 24-frame-per-second target-camera refresh.
  Requests follow a wall-clock 24 Hz phase, independent of the simulation clock
  and other camera panels. A slow host skips missed frames rather than queuing
  bursts. GPU readbacks remain asynchronous, so delivered rate depends on the host.
- A black damage bar means undamaged; fully white means destroyed.
- Bearing uses 12 at the nose, 3 right, 6 aft, 9 left, relative to player heading.
- John requested on 2026-09-21 that HI/LO use elevation relative to the level
  horizontal plane through the player, independent of aircraft pitch and bank.
  HI means strictly above +10 degrees; LO means strictly below -10 degrees.
  Exactly +/-10 degrees and all angles between them show neither. This requested
  threshold is opinionated; the manual does not give the original number.
- Bottom right alternates slant distance in NM and ground speed in KTS every
  360 simulation ticks at 120 Hz, starting with distance. Pausing freezes it.
- The objective row shows only `Obj: Survive` or `Obj: Destroy`, relative to the
  player's assignment. Survive identifies an escorted friendly or a friendly
  marked must-survive in mission requirements. Destroy identifies an enemy
  explicitly required by the player's destroy assignment. Other contacts have
  no objective label. Allegiance alone does not establish a requirement.
- Activity, tactical goal and skill keep their existing independent meanings.
- Goal A means attack, E evade, N neutral, T takeoff, C crash, L land.
  Underline A/E only when directed at the player. Skill has 0..3 dots.
  T covers waiting on the runway, taxiing and the takeoff itself. L covers
  holding at marshal, the landing and the landed aircraft. Keeping L after the
  aircraft stops is an agent decision (2026-09-23): rollout and parking end the
  same landing sequence, and the manual lists no separate code for it.

## Fitted presentation and unresolved evidence

Agent choices: round clock bearing to the nearest hour and speed to whole
knots; range has one decimal.
Damage is one minus current/initial hit points, clamped to 0..1, filling upward.
The camera uses the player's position and interpolated target position to
establish the viewing line. It sits one nautical mile (6,076 feet using the
existing simulation convention) behind the target along that line. Agent choice:
when player-to-target range is at or below one nautical mile, it stays at the
player position rather than moving behind the player. Coincident positions
remain finite and use that same position. Displayed range, bearing and Hi/Lo
still measure from the player, never from this presentation camera.
Agent-chosen framing keeps image roll level and fits aircraft mesh vertices into
90% of image width and 52% of image height, reserving the top/bottom text rows.
The single objective row is at y=110 in the 162 by 160 window and uses the
original instrument font.
One dimension fills that area without cropping the other. Zoom follows projected
geometry each camera refresh, so range, target size, and aspect cannot leave a
small target surrounded by empty space. Ground objects use their oriented bounds.
This framing rule expresses John's requested behavior; the exact margins and
bounds fit are fitted implementation choices. Empty/degenerate geometry keeps
unit zoom; points inside the one-foot near plane cannot supply a usable fit.
To address surface flicker during magnification, the target camera's near clip
is half the nearest fitted subject depth, with a one-foot minimum. This fitted
choice improves depth precision while retaining every fitted subject vertex.
Foreground scenery closer than that plane can be clipped in this camera only.
Ordinary views keep their existing one-foot near clip. True coplanar geometry
and source-model defects are not repaired by this precision change.
Text is shortened to its pixel budget so it cannot overlap the goal/damage strip.

Existing aircraft activity and skill are read only. Pursuing/attacking maps to
A; defending/evading/breaking maps to E; destroyed maps to C. Other current
activities map to N. SEARCHING, ACQUIRING and REJOINING are explicit neutral
goal mappings from the [AI awareness service](ai-awareness.md); searching and
rejoining never imply an active weapon target. An attack goal's player underline
uses the existing selected target identity. The evaded threat's identity is not exposed, so E is not
underlined. Ground objects and fixtures have unknown activity/skill/goal.
There is no invented firing prediction: activity describes current state, not
whether a launch will occur. Return-to-base is not labeled L before landing.

Unknown: original Hi/Lo threshold, exact camera pose, speed definition, bar
fill direction, complete activity wording, campaign objective loading, and
player-specific evade provenance. Next research should inspect mission objective
records and activity display contracts. No AI decisions or flight adapters change.
