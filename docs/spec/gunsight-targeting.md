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
A 14-pixel square follows an in-bounds target. Once its center leaves the inset
HUD rectangle x=184..456, y=106..380, replace the square with a chevron at that
rectangle's edge. The arrow points along the three-dimensional bearing and
elevation, including targets behind the aircraft. An exactly rearward target
uses the right edge as a deterministic tie-break. Return to a square as soon as
the target re-enters. No selected target produces no square or arrow.

A separate presentation selection remembers the explicitly designated identity
until the pilot clears/replaces it, the target dies/disappears, or the mission
resets. This fitted UI selection can follow the target's world position outside
sensor coverage so the off-HUD direction cue remains useful. It does not grant
radar observation, lock, missile support or radar-gun lead through lost coverage.
The existing sensor selection still expires normally. The gun solver consumes
current observations only. A provisional boresight seeker diamond never becomes
a selected target square by itself.

The expanded lower bound and startup modes follow the [HUD layout](hud-layout.md).
The cue uses the existing aircraft-forward HUD plane, including its fitted
head-look translation and fade. It is not a helmet-mounted display. Projection
and clipping must remain finite during full bank, steep pitch, rearward targets,
zoom and wide or tall windows. Off-HUD pippers are hidden, never clamped into a
false aimpoint; the edge chevron belongs to the target, not the computed pipper.
