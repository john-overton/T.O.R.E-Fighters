# AC-130 directed guns and linked firing

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-10-05. John requested player-selected targets,
tracking gun mounts and selectable linked combinations. This contract contains
source-derived installation and fitted control/aiming laws. It grants no
independent target choice, orbit control or AI pilot changes.

## Selection and controls

The AC-130 owns three gun slots in the order C_25, C_40, C_105. These correspond
to the reviewed [source stations](../formats/aircraft-variety.md), with each
station's own weapon, ammunition and burst record. Source count is preserved.
Initial firing membership is the first installed gun alone. Ordinary bracket
weapon selection keeps single-gun operation until the player links more than
one member. Once linked, selecting another gun preserves that group. Selecting
NAV or a non-gun station suspends group fire.

Ctrl+7 selects the next installed gun candidate and arms gun mode. Ctrl+8 adds
or removes that candidate from the group. Removing its last member leaves an
empty group and blocks fire rather than choosing another gun. On a standard
Linux gamepad, hold Select and push the right stick right past half travel to
choose the next candidate, or left past half travel to toggle it. Return the
stick before another activation. Group actions are meaningful only on AC-130;
these gestures retain their flight-control meanings on applicable VTOL and
rotorcraft. Existing combat buttons and left-stick cyclic remain available.
These keys and gestures are agent decisions dated 2026-10-05.

The ordinary Fire action releases every enabled gun that has ammunition, is not
failed or lost, and whose own airframe is not in the line of fire. It needs no
ballistic solution: with no target held, a pinned point, a point out of arc or
beyond maximum range, terrain in the way, or the barrels still slewing, the
rounds leave along the actual barrels wherever they point. Those states are
advisory labels on the status line, not blocks. Only SAFE (NAV or an
unarmed selection), LAUNCHER LOST, STATION FAILED, EMPTY, the projectile limit,
an empty group and NO LINE OF FIRE (the gun's own airframe) stop a gun. Eligible
guns begin on the same fixed tick of a new trigger press; thereafter each retains
its own source cadence, ammunition and physical-round scheduling. A blocked or
empty gun does not prevent another member from firing. Trigger release clears
pending rounds for all guns. Group changes also release pending rounds. Restart
recreates initial membership and neutral mount angles.

## Tracking and fitted limits

The guns train on the gunsight's aim point (below) every tick, in every sight
mode, with or without a target. A tracked object is aimed at with its true
position and velocity through the shared gun ballistics solver, including the
weapon's own speed, drop and available life; a pinned or free-slew ground point
is aimed at the same way with no velocity. When the solver finds no solution
(the point is beyond the rounds' reach) the guns still point along the plain
line to the aim point and report MAX RANGE. Firing does not need READY.
The launch direction follows the actual slewed mount. Projectile dispersion
retains the existing fitted 0.25-degree cone. Gun firing range is 0 through 13,000 feet for all three installed records,
using each weapon's source range rather than its longer projectile life.

The source headings establish left-facing neutral orientation at -90 degrees,
with neutral elevation 0. Source arc scalars are converted using the established
65520-angle-unit circle. Their meaning as half-widths rather than total widths
is an explicit fitted interpretation, because the original arc consumer has
not been established. Shipped limits are:

| Gun | Heading range | Elevation range |
| --- | --- | --- |
| C_25 | -150 to -30 degrees | -60 to +60 degrees |
| C_40 | -135 to -45 degrees | -45 to +45 degrees |
| C_105 | -115 to -65 degrees | -45 to +45 degrees |

Both heading and elevation slew at a fitted 30 degrees/second, independently,
at 120 Hz. A gun within a fitted 1-degree error of both desired angles reports ready;
otherwise it reads SLEWING, and it fires either way.
A demand beyond an arc is clamped for visible movement and marked CANNOT BEAR;
the gun fires along the arc limit. Every accepted direction points outward on
the left side; right-facing directions are rejected. Terrain between muzzle and
aim point reads TERRAIN MASK and does not block fire: the rounds meet the
terrain. There is no hidden path through the aircraft to keep
a group synchronized.

Projectile origins and animated barrels share fitted pivots and tips selected
from the original barrel mesh. Source X/right, Y/forward, Z/up units convert to
host feet at 2/3 scale, with host mount order right/up/forward. Raw PT offsets
are not used for this feature because the existing conversion does not align
with the visible barrels.

| Gun | Fitted source pivot | Source tip used to set barrel length |
| --- | --- | --- |
| C_25 | (-9.5, 29, -12) | (-16.5, 28, -14) |
| C_40 | (-11, -7, -11) | (-18, -7, -14) |
| C_105 | (-9, -25, -11.5) | (-21.5, -25, -14.5) |

The actual muzzle is the pivot plus barrel length along actual aim. The barrel
mesh follows the shortest rotation from its source tip-pivot vector to that
same direction. A conservative fitted left-skin plane at local right=-8 feet
blocks a release if its posed muzzle remains inward of that plane. Forward ray
checks also reject the conservative fitted source boxes: left wing
X=-99..-11, Y=-25..1, Z=5..7; inner nacelle X=-33..-18, Y=-25..19,
Z=-6..11; outer nacelle X=-59..-44 with the same Y/Z limits. The wing geometry
lies at source Z=6; the two-unit box thickness is fitted. These checks avoid
upward firing through the wing or nacelles. They can reject a shot that narrowly
clears the detailed original surface, because the boxes are conservative.
This also
blocks some combined extreme heading/elevation demands of C_25 despite their
individual arc limits. Feedback is NO LINE OF FIRE. Geometry hinges and
clearance are agent choices from the reviewed mesh, not original motion
consumers.

## The gunsight

Opinionated (John, 2026-10-09) unless marked. The AC-130's camera line of sight
is fixed-tick sim state, so single player, the multiplayer host and a restored
checkpoint all hold the same sight. It has three modes:

| Mode | Line of sight | Aim point |
| --- | --- | --- |
| Free slew | Body-relative heading and elevation the pilot slews | Where the line meets the ground; with no ground (sky), a point along it at the longest installed gun range (13,000 feet) |
| Pinned | Toward a fixed world ground point; slewing moves the point | The pin |
| Tracked | Toward an object, air or ground, at any range | The object, with lead |

- **Default view**: free slew at heading -90 degrees (abeam left) and elevation
  -25 degrees in the aircraft's own frame, zoom step 3. It is inside every
  gun's arc and clear of the airframe at any bank. Mission start, restart and
  respawn begin there; the guns start neutral and reach it in under a second.
- **Slew**: the seat sends a normalized deflection (-127 to 127, x right, y up)
  and its zoom step (1 to 6) with every tick's input; the host integrates the
  look at 120 Hz. Full deflection turns 0.75 of the camera's vertical field of
  view a second across the screen (the heading rate is divided by the cosine of
  elevation, floored at a quarter): 22.5 degrees a second at step 1 down to
  0.7 at step 6. Heading wraps through a full turn and elevation stops at
  89 degrees up or down (fitted). The zoom ladder is 30 degrees tall at step 1
  and halves each step (fitted, agent choice). Slewing while tracking does
  nothing and raises a one-off "L to drop" notice.
- **Backslash** (designate): the object nearest the line of sight within the
  pipper circle's angular radius (9/114 of the vertical field, 0.6 degrees at
  step 3), then the nearest, then the lowest id, is tracked; friendlies,
  destroyed and terrain-masked objects are skipped (agent choice). With no such
  object the ground under the crosshair is pinned. While tracking, Backslash
  keeps the track.
- **Shift+Backslash** (pin): the ground under the crosshair is pinned, also from
  a track (under a ground target, behind an air target). With no ground on the
  line of sight nothing changes and a one-off "no ground point" notice is
  raised.
- **L** (or `;`) drops a target or pin and slews freely from the current view.
  With nothing held, L travels back to the default view at 22.5 degrees a
  second, the fastest sight slew, whatever the zoom; it never snaps (John,
  2026-10-09; the rate is an agent choice). A slew on the way takes over.
- **Pod track**: on the AC-130, T, Shift+T, Enter and a scope click also start
  a sight track, and the radar selection follows the sight (it is set when the
  radar holds the object and cleared otherwise), so the HUD, the scope and the
  guns never disagree. A track ends only on L, a new designation, or the
  object's destruction or removal; the sight then pins the ground under its
  line of sight, so the pilot sees the hit.
- **Always-on Easy targeting**: the AC-130 has Easy targeting's effects
  whatever the session cheat says, including in multiplayer with cheats off,
  because the sight is the aircraft's sensor. Its target camera and HUD square
  follow the sight's track, ground objects included. Other aircraft are
  unchanged.
- The line of sight meets the ground by marching in steps of half the height
  above the ground (16 to 1,000 feet) out to 40 nmi, then halving the last
  step twenty times (fitted). Terrain within 25 feet of a point does not mask
  it, so a ground point is not masked by the ground it lies on (fitted).

## Feedback and shared state

The input page exposes the two group actions with AC-130 applicability. A group
label shows each included cannon and the current candidate. Weapon readiness
reports NO TARGET, CANNOT BEAR, SLEWING, MAX RANGE, MIN RANGE, NO LINE OF FIRE
(the gun's own airframe), TERRAIN MASK (terrain between muzzle and aim point),
EMPTY or GROUP EMPTY as applicable. The selected candidate's reason, blocking or
advisory, remains visible even when another group member is ready; every
member fires independently unless it is itself blocked.
The status line shows the advisory states only to tell the pilot a shot is not
solved; none of them holds the trigger.

Host, clients and replay receive actual heading/elevation and membership. The
six angle values use slot order C_25, C_40, C_105 and interleaved heading/elevation;
heading is divided by pi, elevation by pi/2 for bounded presentation transport.
Membership is a separate three-bit mask. Tracking state is fixed-tick owned;
local animation does not choose the target or decide whether a gun can fire.

## Acceptance and known limits

Use synthetic gun records and targets to check each single cannon, mixed groups,
first-tick linked releases and distinct subsequent cadence, tracking slew/limits,
target changes/loss, trigger release, empty/failed stations, empty groups and
restart. Check command tapes and replicated actual mount angles/membership.
The unavailable retail comparison does not establish retail parity. Original
mount slew, the arc interpretation, gun geometry hinges and muzzle-to-art scale
remain approximate. The next research step is the reviewed bounded hardpoint
arc consumer and explicit source muzzle/shape correspondence.
