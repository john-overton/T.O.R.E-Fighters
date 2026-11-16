# Gun pipper and three-dimensional target cues

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-21. John requested the manual's gun pipper and range
cue, a target square, and an edge-of-HUD chevron. This applies to every selectable
aircraft and its imported gun. Aircraft flight adapters and weapon guidance
rules remain separate.

## Manual-supported behavior

The local 1999 EA/Jane's Fighters Anthology manual, printed pages 83 and 86
(PDF pages 87 and 90), was read and visually inspected. Local PDF SHA-256:
`1a082378a8e8cd163ed6b398efcc1df80b67c2f104f6b90ac0733c88d58e26c3`.
It is available as `.local/missile-update/manual.pdf`; rendered reference pages
are local to `.local/gunsight-ground-review/`. No retail document is committed.

- Selecting guns displays ammunition and a pipper that accounts for bullet drop.
- Radar-off ranging uses a point 1,000 **feet** from the aircraft. It is not the
  1,000-meter reference found in older USNF remake notes.
- With radar on and an aircraft targeted, the pipper uses target range and
  automatically calculates lead. The pilot places it over the target to fire.
- A thick perimeter arc grows as range closes: absent outside maximum gun range,
  half a circle at half maximum range, and a full circle within 100 feet.
- The selected R/V target has a square designator. Target range uses nautical
  miles. The manual's offscreen symbol is `XX` at the screen perimeter.

The requested chevron and its position on the HUD boundary, in place of `XX`
on the screen boundary, are opinionated user choices. Exact original pixel
placement and the original ballistic computer's equations remain unknown.

## Fitted ballistic solution

The gun solution uses the imported gun's muzzle mount, scalar launch-speed rule,
axial acceleration/deceleration, gravity flag and fall-speed cap. A bounded
120 Hz predictor shares those projectile helpers. It does not invent vector
velocity inheritance that the live gun projectiles do not have. Ownship attitude
sets the barrel axis and its speed supplies the imported launch-speed rule.
Random gun dispersion is excluded: the pipper represents the center of the cone.

Without a usable radar observation, solve the dropped round's point at 1,000
feet. With a current radar observation of the selected aircraft, solve the first
constant-velocity intercept within the lesser of the gun lifetime and ten
seconds. The displayed pipper is the current barrel's predicted bullet point
minus target displacement over that time. Thus placing the pipper on the current
target gives the computed lead and drop correction. Future target maneuvers and
future ownship control changes cannot be predicted. No reachable solution means
no aimpoint, with `NO SOL` shown rather than a fabricated hit guarantee.

Maximum range comes from the imported gun launch zone. The range-arc fit
interpolates through (maximum, 0), (half maximum, 0.5), and (100 feet, 1), with
clamping outside those endpoints. The thin circle remains when the thick arc is
absent. Start the clockwise arc at twelve o'clock, following the manual diagram.
The fitted circle radius is nine HUD source pixels and the arc is three pixels
thick. A center dot marks the precise aimpoint. A numeric target range in nmi is
shown when a current radar or visual observation supplies it. Base mode is
explicitly labeled `1000 FT`. SAFE, empty or failed guns do not show a firing
pipper. The target designator itself is independent of arm and weapon selection.

## Target square and edge chevron

Project the selected target through the aircraft's complete yaw, pitch and roll
into the same angular HUD space as the existing flight and missile symbology.
A 14-pixel square follows an in-bounds target. John requested a centered X for
friendly forces on 2026-09-21, using his supplied retail screenshot as the visual
reference. Known friendly contacts use two diagonal strokes spanning six source
pixels (center minus/plus three), an agent-selected fitted size. Use the same HUD
color and transform as the square. Enemy, neutral and unknown contacts have no X.
Use explicit mission-side metadata, or known airport allegiance for runway
contacts; never infer allegiance from aircraft type or proximity. Off-HUD
chevrons remain unchanged. This is presentation only and changes no targeting,
guidance, firing permission or autonomous behavior.

Once its center leaves the inset
HUD rectangle x=184..456, y=106..380, replace the square with a chevron at that
rectangle's edge. The arrow points along the three-dimensional bearing and
elevation, including targets behind the aircraft. An exactly rearward target
uses the right edge as a deterministic tie-break. Return to a square as soon as
the target re-enters. No selected target produces no square or arrow.

The square and arrow follow the sensor selection only. When the scope loses
the target it drops, and so does its cue ([target selection keys](radar.md#target-selection-keys)).
With the [Easy targeting cheat](cheats.md) on, the selection stays set while
the target is off the scope, and outside the HUD the square floats over the
target at the HUD's own size and brightness instead of switching to the arrow;
the arrow remains for a target behind the view. It never grants radar observation, lock,
missile support or radar-gun lead. The gun solver consumes
current observations only. A provisional boresight seeker diamond never becomes
a selected target square by itself.

The expanded lower bound and startup modes follow the [HUD layout](hud-layout.md).
The cue uses the existing aircraft-forward HUD plane, including its fitted
head-look translation and fade. It is not a helmet-mounted display. Projection
and clipping must remain finite during full bank, steep pitch, rearward targets,
zoom and wide or tall windows. Off-HUD pippers are hidden, never clamped into a
false aimpoint; the edge chevron belongs to the target, not the computed pipper.

## Slewed gun mounts: the B-52's tail gun (B52-6, 2026-11-15)

Implementation mode, **fitted**, an agent decision (the coordinator's first pass of
2026-11-15 on the bomber batch: "a tail gun firing forward is a bug"). The record
parts are spec-derived from the retail aircraft records; the split between forward and
slewed mounts, the centre-line fire for a human and the HUD cue are fitted.

**What the record says.** An aircraft's gun row carries four angle words, 182 units a
degree: `slewH` and `slewP` (where the mount points at rest) and `slewLimitH` and
`slewLimitP` (the half-arc either side). The B-52's M61 reads heading 32,760 (180
degrees, astern), pitch 0 and 5,460 (30 degrees) each way. The Tu-26's `T23_1.JT`
turret reads heading 32,760 with 30 degree arcs, and the Tu-95's three turrets read heading 32,760, pitches 0, +3,640 (20 degrees up, the
dorsal turret) and -3,640 (20 degrees down, the ventral one), arcs 45 degrees in heading and 45, 5 and 5 degrees in pitch. Before this
change nothing read these words: every aircraft gun fired along the nose, so the B-52's
tail gun fired forward from the tail cone, 92 ft behind the reference point, through its own airframe.

**The rule (every reader follows it, `tore_sim::combat::slew`).**

- A mount whose heading word is zero is a **forward gun**. It fires along the nose,
  exactly as before, whatever its pitch word says. (The helicopters' chain guns read a 20 degree
  depression that has never been applied and still is not: honouring it would move every
  helicopter's sight and hits.)
- A mount with a heading is **slewed**. Its fire direction is the mount's rest
  direction in the aircraft's frame: the nose turned by the heading (clockwise seen from
  above, positive right) and then by the pitch (positive up). It turns, rolls and pitches
  with the aircraft. The muzzle is still the record's mount point.
- A gun station carries the slew only when it is an aircraft gun and the aircraft is not
  the AC-130 (whose trained mounts have their own rules, [AC-130 linked guns](ac130-linked-guns.md)).
  Every store, every pod and every forward gun carries the default, which is the nose line.

**Who reads the direction.**

| Reader | What it does with a slewed gun |
| --- | --- |
| The round (`State::release_round`: a human's trigger and an AI's alike) | leaves the muzzle along the rest direction; the shared dispersion cone is taken around it |
| The hit model, live fire, rewind | nothing to change: they read the round's own position and direction |
| The human's gunsight (HUD, the gunnery probe, the Load Ordnance gun check) | the 1,000 ft pipper and the radar lead are solved along the rest direction from the same muzzle (`Station::gun_view`); a forward gun gets its launcher back unchanged |
| The HUD (`weapon_hud::draw_gun`) | reads `<rounds> M61` and `TAIL` where a forward gun reads `LCOS` (`SAFE` while unarmed). No pipper: the line is behind the glass |
| A networked client's drawn rounds (`net::guns::gun_line`) | start at the mount and leave along the rest direction, as the host's do |
| A recorded mission (replay) | the projectiles are the simulation's own, so they replay as fired |
| The AI | see [AI gun employment](ai-gun-employment.md#slewed-guns-the-b-52s-tail-gun-b52-6-2026-11-15) |

**Aiming (fitted, simple).** A human fires the tail gun along the mount's centre line, astern
(the 30 degree arcs are not used by the human, no manual swing). There is no rear-view
reticle in the game's rear views (F2 Back has no HUD), so the cue is the `TAIL` text and the tracers
(every third round) going aft. The rounds do not carry the aircraft's own velocity (as for every gun), so the
rounds' paths are the same as a forward gun's.

**Tests.** `tore-sim`: `combat::slew::tests` (the record words, the forward mount's pitch ignored, the
aft mount, the Tu-95 dorsal tilt, the arcs, the muzzle point in the mount's frame) and
`combat::live::pair_tests` (`a_slewed_gun_fires_astern_and_a_forward_gun_does_not`,
`a_forward_gun_keeps_the_nose_line_and_its_sight_view`,
`a_slewed_guns_sight_view_points_astern_from_the_same_muzzle`). `tore-world`: `ai_wings::tail_gun_tests`.


## Update (T26-6, 2026-11-16): the Tu-26's tail gun

The Tu-26's `T23_1.JT` row carries the same slew words as the B-52's (heading 32,760, arcs 30 degrees), so it is a slewed mount with no new code: the round, the
human's sight, the networked client and the AI follow it (`ai_wings::tail_gun_tests::the_retail_tu26_and_tu95_gun_rows_are_slewed_tail_guns_that_fire_astern`). The
HUD reads `TAIL 2000 23MMX1`; the muzzle is the nozzle exit plane (T26-5). A human's rounds leave astern (frame `tail-gun-external` shows the tracers going
aft from the tail). `flight-livefire-tu26-slot1` fires it. See [the Tu-26 section](../formats/aircraft-variety.md#tu-26-backfire-c-ordnance-sensors-hud-and-cockpit-t26-67-2026-11-16).

## Update (T95-6, 2026-11-16): the Tu-95's tail turret

The Tu-95's `T23_1.JT` tail turret (row 3, moved to the cone's point by T95-5) reads the same slew words and fires
astern with the same `TAIL` cue and the same BMB-AI aim model as the B-52's: `flight-tu95-tail-gun` has an AI Tu-95
hit 5 of 180 rounds on an F-22 closing from astern. No code changed; the mount follows its record.
