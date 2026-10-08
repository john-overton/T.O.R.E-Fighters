# Variety aircraft device presentation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation contract, 2026-10-05. Geometry comes from each reviewed stock
shape in the [variety source inventory](../formats/aircraft-variety.md).
The measured neutral projection and individual guard branches below are
validated at load before using their face groups. Recovered import aliases
identify the gear, flame, brake, hook and rudder branches; source code is
never executed. Offsets describe data provenance, not a gameplay requirement.

## Reviewed geometry

| Shape | CODE bytes | Neutral faces | Added device faces |
| --- | ---: | ---: | --- |
| C130.SH | 10812 | 144 | gear 6 |
| AC130.SH | 19666 | 339 | gear 8 |
| AWACS.SH | 26794 | 296 | gear 11, rudder 2 |
| IL76.SH | 28644 | 309 | gear 20 |
| E2C.SH | 29258 | 345 | gear 10, hook 2 |
| AV8.SH | 18014 | 270 | gear 8 |
| Y141.SH | 21134 | 308 | gear 4 |
| V22.SH | 17204 | 160 | gear 12 |
| APA.SH | 20460 | 370 |  |
| HIND.SH | 24994 | 387 | gear 6 |
| CH47.SH | 18329 | 231 |  |
| M17.SH | 20602 | 273 | flame 5, brake 10, gear 22 |
| F4J.SH | 25222 | 309 | flame 16, brake 6, gear 22, hook 2, rudder 2 |
| F4E.SH | 20000 | 213 | flame 8, brake 2, gear 6, rudder 2 |
| F4.SH | 21456 | 233 | flame 8, brake 2, gear 6, rudder 2 |
| A7.SH | 23270 | 261 | flame 4, brake 10, gear 26, hook 2, rudder 2 |
| F15.SH | 30762 | 346 | flame 8, brake 4, gear 22 |
| F16.SH | 31488 | 356 | flame 6, brake 8, gear 25, rudder 2 |
| F104.SH | 28374 | 297 | flame 4, brake 8, gear 24, hook 2, rudder 2 |
| A10.SH | 19854 | 303 | gear 20 |
| B747.SH | 31108 | 373 | gear 36, rudder 2 |
| A310.SH | 20868 | 248 | gear 14, rudder 2 |

F-4B and F-4J share F4J.SH. The same shape exponent supplies the existing host
scale rule: one-third foot per coordinate at exponent 8 and two-thirds at
exponent 9. The body, attachments and debris must use the same scale.

Y141.SH requires a bounded signed greater-than-or-equal-to-zero guard for its
flaps at CODE+0x386d and +0x3b28. The reader selects drawing records for that
reviewed guard; it does not interpret arbitrary machine instructions.

## Initial generic motion

Agent choices, explicitly fitted. All 23 variety aircraft now use individually reviewed rules below. The shared
initial device rules still apply only to explicitly retained device mappings.

- Gear uses its original deployed branch. During retraction, the group moves
  upward by its source vertical extent times the remaining travel fraction;
  fully retracted faces disappear. This is rigid travel, not recovered hinges.
- Airbrakes retain their source deployed pose at full extension and rotate
  toward their fitted forward hinge by up to 0.7 radians while closing.
  The added brake faces disappear at zero extension. Where the source has
  separate closed skins, as on the F-16, they return at zero extension and
  disappear while the open branch is active.
- Hooks retain the source deployed pose at full extension and fold toward
  their forward upper bound by up to 1.2 radians while stowing. They disappear
  at zero extension. This does not establish carrier arrestment physics.
- Identified rudder groups rotate by 0.35 radians times the live rudder signal
  about the source group's forward vertical hinge. Replaced neutral branch
  faces are removed, so both copies do not draw together.
- Flame length follows the live exhaust fraction from its forward root;
  zero exhaust hides the flame. Non-afterburning aircraft do not gain thrust
  or afterburner effects from the existence of a source flame branch.

[Propellers, rotors and tiltrotor nacelles](rotor-presentation.md) have their
own reviewed groups and fitted motion. Unlocated hook geometry, individual
rotor blade feathering and live mirror masks for new cockpit families remain
incomplete. Static parts remain original geometry;
the import does not claim every surface is animated.

## A-7 surface repair and acceptance

John reported the one-sided, detached rudder and missing pitch/roll surfaces on
2026-10-05 and requested individual headless animation checks before further
full validation. The old generic rig used an already-deflected rudder as its
neutral shape and rotated it about a bounds-derived vertical axis. It had no
pitch, roll or flap mappings. Its finite geometry was insufficient evidence of
correct articulation.

The corrected rig reads A7.SH from the user's media. The
[source review](../formats/aircraft-control-surfaces.md) records its identity,
neutral skins, attachment edges and deployed flap endpoints. The following assignments and travel laws are agent-authored fits,
2026-10-05; they are not claims of recovered retail control deflections. Source
coordinates are right, forward, up, at one-third foot per unit.

| Control | Source geometry and fitted motion |
| --- | --- |
| Yaw | Keep the reviewed neutral rudder skins. Rotate both around their shared source edge, by `0.35 * rudder` radians. Both endpoints remain fixed and zero input reproduces the source neutral geometry. |
| Pitch | Rotate the two trailing tail skins on each side around their existing diagonal seams, with axes oriented rightward, by `-0.30 * elevator` radians. Positive pitch raises both trailing controls. The assignment of those source triangles as elevators is fitted. |
| Roll | Use the separate outer-wing trailing triangles. Hold their upper/lower hinge edges and outer tip fixed, and move their shared trailing vertex through a fitted 0.20-radian centerline rotation. Positive roll lowers the left trailing point and raises the right. This constrained skin deformation keeps the two skins joined; it is not a recovered independent aileron branch. |
| Flaps | Source `_PLleftFlap`/`_PLrightFlap` state -1 supplies down geometry; +1 omits skins and is not an up endpoint. Interpolate each neutral skin vertex to its matched down vertex by the live 0..1 fraction. Preserve all four hinge-skin endpoints on each side and shared trailing vertices. Show original outboard closure triangles during deployment. |
| Brakes | Keep cavities static. Rotate the left/right panel skins about their own forward vertical source seams, closing through `atan(7/8)` radians with opposite signs. Brace roots remain anchored while outer points follow the panel. Source full-open geometry is exact; continuous travel and brace deformation are fitted. |
| Hook | Preserve both reviewed source root-edge points. Rotate distal vertices toward stow around fitted mean root `[0,-17,-12.5]`, lateral axis, by `-1.2 * (1-hook)` radians. Closing must raise the blade, not swing it farther down. Hide at zero deployment. |
| Main gear | Fitted two-stage linkage: as gear decreases 1 to 0.5, wheel centers move from X=+/-11 to +/-3 at Y=-2,Z=-22 while tilting 0.70 radians. From 0.5 to 0, centers move to X=+/-3.5,Z=-11.2, keeping the connecting panel below its hinge edge until it stows inside the body. Upper vertices with Z>=-13 remain fixed; lower vertices use weight `clamp((-13-Z)/6,0,1)` toward the common linkage transform. Wheels remain rigid; connecting skins deform. This replaces an inward hinge fit that crossed the wheels. |
| Nose gear and doors | Nose assembly folds around `[0,46,-12]`, lateral axis, through fitted -pi/2 while closing. Each door uses its own source attachment edge and fitted pi/2 travel. Hide deployed branches at zero gear. Mechanical sequencing remains fitted. |

Preserve source texture coordinates, colors and materials. Rotate normals with
rigid surfaces and recompute them for constrained skin interpolation. Never use
an already-deflected source branch as the neutral rudder.

For this aircraft, sweep negative, zero and positive control inputs and at least
three intermediate device positions, with 21 samples for gear through the same transformed faces used by
the renderer. Require pitch, roll, yaw, flaps, gear, brake and hook to move.
Require exact zero-state neutral rudder skins, fixed reviewed hinge endpoints to
within 0.0001 ft, coherent twin skins, opposite roll signs and matching pitch
signs. Require main wheels to stay separated by at least 0.70 ft with each side at
least 0.35 ft from center, preserve wheel dimensions and upper roots, and fit
stowed wheel bounds inside abs(X)<=5.5,Z=-13.6..-8.8 source units. Inspect top, side
and rear contact sheets, including intermediate gear poses. Finite/reversible geometry alone does not pass an aircraft. The
[animation audit](../baselines/aircraft-animations.md) records per-aircraft
results and remaining gaps.

## F-4 family surface repair

F-4B/J share F4J.SH; F-4E and F-4G keep their own shapes, source asymmetries and
capabilities. The [source review](../formats/aircraft-control-surfaces.md)
records identities, neutral skins and branch endpoints. These continuous laws
are agent-authored fits, 2026-10-05. They do not establish retail animation
mechanics or change the flight model's existing device timing.

| Surface | Fitted contract |
| --- | --- |
| Rudder | Start from each shape's neutral opposite skins. Rotate around its reviewed shared diagonal edge by `0.35 * rudder` radians. Zero input reproduces source neutral geometry. |
| Pitch | Keep both source tail-root boundary vertices fixed. Rotate distal vertices around a rightward lateral axis through their mean by `-0.30 * elevator` radians. J left/right pivots are [-2.5,-78,7]/[2.5,-77.5,7]; E [-2.5,-55,6]/[2.5,-55,6]; G [-2.5,-64,7]/[2.5,-64,7]. This is constrained skin motion, not a recovered stabilator shaft. |
| Flaps | Morph matched vertices from source neutral to down by the live 0..1 fraction. J retains the neutral quad topology while matching the triangulated down outline. E/G retain separate upper/lower fixed edges and original deployed end closures. |
| Roll | J splits only the reviewed crossing outer-wing skins at `y+32+0.35*(abs(x)-43)=0`. E/G use their separate trailing triangles, preserving thick hinge edges and shared trailing points. Outward axes use the same `-0.20 * aileron` angle on both sides, raising right and lowering left for positive input. Source root/tip asymmetries remain. Flaps and ailerons are independently actuated. |
| Brakes | Preserve source full-open geometry at 1 and forward root edges. J closes around [0,23,11] by `atan(10/12)*(1-brake)` radians; E around [0,27,10] by `pi/4*(1-brake)`; G around [0,21,11] by the same angle. Hide deployed branches at zero. |
| Hook, B/J | Preserve the source root-edge points; close distal points upward around fitted mean [0,-55.5,-6], lateral axis, by `-1.20*(1-hook)` radians. Hide at zero. E/G retain their source-disabled hook capability; no source hook geometry was located. |
| Main gear | Separate each side and preserve its own upper edge. Forward-direction axes pass through J [+/-29,-14.5,-8], E [22,-3,-6]/[-21,-3,-6], and G [+/-22,-8,-5]. Inward travel is +/-`1.45*(1-gear)` radians. Preserve textured wheel/leg panels as source units, without invented wheel steering groups. |
| Nose gear | Fit lateral pivots J [0,65,-8], E [0,53.5,-7], G [0,48.5,-6]. Distal points fold aft by `-pi/2*(1-gear)`, with source upper-edge vertices fixed. J wheel skins follow the same transform. |
| J doors | Inboard door pivots [+/-13,-7,-8], forward axes, opposite pi/2 closing. Nose forward door [0,68,-8], lateral axis, +pi/2; side door [2,52.5,-8], forward axis, +pi/2. Preserve source attachment edges. No added timing sequence is claimed. |

Maintain UVs, colors and materials, and recompute constrained-skin normals.
Acceptance uses exact zero/source and full-deployment endpoints, signed pitch
and roll, fixed roots, same-surface twin coherence, 21 intermediate gear samples,
and per-aircraft contact sheets. Ordinary clearance between distinct controls
is separate from a broken hinge. The audit records geometry acceptance only;
texture/GPU and original-runtime comparison are not implied.

## AC-130 mounts

The three barrel groups rotate about the shared fitted source pivots in
`combat::gunship`. Rodrigues rotation aligns each original tip-to-pivot axis
with the actual combat-owned heading and elevation, using the same barrel
length as projectile emission. A synthetic numerical test compares rendered
tips and emitted muzzle positions for all three mounts at multiple angles.
Membership and mount angles are carried by local snapshots, multiplayer and
mission recordings; old recordings without those poses retain static geometry.
The [gunship contract](ac130-linked-guns.md) owns firing arcs and rates.

## Cockpits

Each aircraft uses its reviewed source HUD and cockpit choice. Shared F4, AV8
and SU33 art uses the existing glass mask for that family. New A7, F104, F16,
Y141 and M17 glass polygons are fitted from visual inspection of their imported
1280 by 490 cockpit art. The AC130 cockpit has an open windshield over a low
panel; its transparent area receives the HUD. Opaque art still covers symbols.
These aperture shapes are presentation fits, not recovered retail draw limits.
The original overlays remain imported. Live rear-camera masks are enabled
only where the family already has reviewed fill seeds, otherwise the artwork
remains static.

## Validation

Synthetic transform tests cover rotor pivots, nacelle endpoints and neutral
engine behavior. Per-aircraft load validation checks the measured shape layout
and every implemented branch count. Device captures and source-art inspection
must be recorded separately from flight and multiplayer validation. No claim
of retail animation timing or complete moving-surface parity is made.


## F-15C control and device fit

Implementation fit, agent decision, 2026-10-05. F15.SH source skins and named
flap endpoints are retained. No source animation law for pitch, roll or gear
retraction was recovered. Source coordinates use one-third foot per unit.

| Control | Fitted contract |
| --- | --- |
| Yaw | Split upper fin skins at `y=-57+(z-5)*5/38`. Rear regions rotate `0.35*rudder` radians around their diagonal seam; lower fins stay fixed. |
| Pitch | Split the two tail skins at their existing diagonal rear seams. Rotate rear regions by `-0.30*elevator` radians around rightward axes. Both trailing controls rise for positive input. |
| Roll | Split the outer trailing wing skins at abs(X)=42 and 56. Only the central strip moves through `-0.20*aileron` radians about its outward leading axis. Pin each thick leading-skin edge; retain original inner and tip strips. Positive input raises right and lowers left. |
| Flaps | Interpolate exact matched source neutral/down vertices. Upper/lower leading edges stay fixed; original end closures appear only above zero deployment. No automatic flap/roll coupling. |
| Brake | Preserve full-open source geometry and forward edge at Y=28,Z=11. Close about the lateral axis by `atan(19/27)*(1-brake)` radians; hide at zero. |
| Main gear | Keep upper attachments at Z=-11. Split rigid lower wheel sheets at Z=-18. While closing, turn the lower assembly aft through pi/2 about [+/-13,3,-18] and translate Y by -9 and Z by +12 times closing fraction. The upper connector uses a linear height weight. Doors retain their source hinges; connecting braces follow the adjoining pieces. |
| Nose gear | Fold complete wheel/strut panels aft through pi/2 around [0,64,-8], the forward painted brace attachment. Rotate the separate solid door around its own source edge at Y=52,Z=-8. Do not pin transparent rectangle corners as separate mechanical roots. All source panel dimensions remain rigid. |
| Gear visibility | Full deployment reproduces source geometry; zero deployment hides the added branch. Just before hiding, main wheel sheets fit abs(X)=9..15,Y=-15..-6,Z=-11..-1. Complete nose and door panels fit abs(X)<=2,Y=39..64,Z=-8..6. These are fitted stow envelopes, not recovered gear bays. |

Acceptance checks signed controls, fixed source attachment points, exact neutral
and deployed endpoints, coherent skins, rigid wheels and new polygon crossings.
Gear needs intermediate poses and a near-zero pose before hiding. Stowed F-15
nose panel vertices were checked against neutral body cross sections. These
checks establish bounded geometry, not original mechanical timing or GPU
texture appearance.


## MiG-17F control and device fit

Agent-authored fit, 2026-10-05, using exact MIG17F.PT/M17.SH. Source skins,
materials and deployed device geometry are retained. Surface roles, continuous
angles and gear mechanisms are fitted, not recovered original laws.

| Control | Fitted contract |
| --- | --- |
| Yaw | Keep each thick rudder skin's distinct forward/root vertices fixed. Rotate common trailing/cap points together by `0.35*rudder` about [0,-41,4] to [0,-57,23]. Forward fin and underside regions stay static. |
| Pitch | Pin tail roots [0,-43,15] and [0,-55,15]. Rotate distal vertices `-0.30*elevator` about the rightward axis through [0,-49,15]. |
| Roll | Keep each thick outer trailing skin's forward edges fixed. Move shared trailing points through `-0.20*aileron` about outward axes, left [-28,-12.5,-1] to [-42,-23,-2.5], right [28,-13,-1] to [42,-23,-2.5]. Positive input raises right and lowers left. Tip and flap controls stay separate. |
| Flaps | Preserve neutral front seams. Interpolate trailing vertices [+/-13,-7,-1] to [+/-13,-7,-4] and [+/-28,-21,-1] to [+/-28,-20,-5]. The additive source down branch moves front coordinates by 1..2 units; this fit deliberately keeps the neutral attachments instead of matching those moved front coordinates. |
| Brakes | Keep cavities fixed. Close panels about their own diagonal source root edges by opposite `atan(6.5/6)*(1-brake)` angles, right negative, left positive. Brace roots remain fixed and distal points follow the panel. Full-open geometry is exact; hide at zero. |
| Main gear | Rotate complete wheel/strut panels inward by pi/2 about forward axes through [+/-20,3.5,-2]. Stowed panels fit abs(X)=9..20,Y=0..7,Z=-2. Preserve dimensions and original upper edges. |
| Main doors | Inner pairs pivot through [+/-7,6,-2], closing outward by pi/2; outer pairs pivot through [+/-21,5,-2], closing inward by pi/2. Well faces stay static while the branch is active. |
| Nose doors | Use each source upper edge [+/-2,32,-6] to [+/-2,40,-5]. Close inward through 0.70 radians, stopping short of crossing the center. Residual stow clearance and disappearance at zero are fitted. |
| Nose gear | Rotate the complete wheel/strut rigidly aft by `-100 degrees*(1-gear)` about [0,34.2973,-6], lateral axis. This is the front endpoint of the painted attachment band. A collinear UV marker makes it directly testable without changing the source outline. Transparent rectangle corners move with the assembly. |
| Exhaust | Scale the reviewed source flame branch from its own forward root with live exhaust; hide at zero. |

At full stow, all 621 sampled opaque nose texel centers fit inspected neutral
forebody slices, with Y=27.452..34.560 and Z=-6.726..0.028. The wider rectangle
inspection bounds Y=26..36,Z=-8..1 include transparent margins and do not claim
solid-volume coverage. Require fixed painted root, rigid complete panels,
neutral/deployed endpoints, coherent skins, signed controls and no new polygon
crossings throughout the sampled poses. A prior weighted connector was rejected
because it collapsed or crossed during retraction. Mechanical timing and
texture/GPU comparison remain unvalidated.


## F-16C and F-104N control fits

Agent-authored continuous motion, 2026-10-05. Each exact aircraft retains its
own neutral skins, deployed branches and asymmetries from the
[source review](../formats/aircraft-control-surfaces.md#f-16c-and-f-104n-source-additions).
Coordinates use one-third foot per unit.

| Component | F-16C | F-104N |
| --- | --- | --- |
| Rudder | Neutral skins around the diagonal source hinge, `0.35*rudder` radians | Same angle about its own source hinge |
| Pitch | Pin tail root chords; rotate distal vertices `-0.30*elevator` around lateral pivots [+/-10,-45.5,1] | Pin the centerline root chord; rotate distal high-tail vertices by the same angle around [0,-61,21] |
| Flaps | Morph all eight original skins to exact source down poses | Morph exact source down poses; retain the original left-only closure |
| Fitted flaperons | After flap morph, rotate shared trailing points `-0.20*aileron` around outward axes [+/-26,-3,0] through [+/-14,-12,1]; pin forward edges | Same angle, outward axes [+/-21,0,-2] through [+/-9,-15,-1.5]; pin forward edges |
| Brakes | Original eight closed faces at zero, eight open faces at one; interpolate the source footprint. Retain the source's one-unit root quantization | Keep Y=24 roots fixed; close distal open skins about [0,24,7] through `atan(2/3)` radians; hide at zero |
| Hook | Source capability/branch not present; none added | Pin both source near-edge points; close distal blade upward by `-1.05*(1-hook)` around [0,-33.5,-6], lateral axis |
| Exhaust geometry | Source flame scales from Y=-59 with live exhaust | Source flame scales from Y=-61 |

Flaperon mixing is an explicit fit. Full flap with zero roll remains the source
down endpoint. Check all 25 combinations of five flap and five roll positions;
require coherent shared skins, fixed forward edges and signed differential
movement. Flame geometry does not grant an afterburner capability.

F-16 main wheels split at Z=-14. Rigid lower pieces rise 10 units during the
first half of closing and another 8 during the second half; then X shifts +2.5
on the left and -2.7 on the right. Upper roots stay fixed. Main stow bounds are
left X=-9.5..-1.5, right X=1.3..10.3, Y=-9..0,Z=-3..4. The nose base track
rises 8 units in each half and moves aft 10 units in the second half. Its painted
brace's distal one-unit pin edge rotates as a rigid edge through -pi/2 in the
first 0.30 closing fraction, around [0,32,-13.5]. The actual distal Z=-14 pin
sets the wheel-cut translation, adding Y=-0.5*sin(angle), Z=0.5*(1-cos(angle)).
The upper brace pins remain fixed. Nose stow bounds are X=-2..2,Y=19.5..25.5,
Z=-4.5..2.5. Doors wait until halfway closed, then rotate left main -3*pi/4,
right main pi-atan(4/5), and nose pi/2 around their source upper edges.

F-104 main wheel skins rotate around their own centers, right [13,-21,-10],
left [-13,-21.5,-10]. They rise 2 units in the first half of closing and 6 in
the second half. During the second half they move inward 1.5 units and turn
pi/2 around forward axes, with opposite signs. Right stow bounds are
X=8.5..14.5,Y=-24..-18,Z=-2; left X=-14.5..-8.5,Y=-24..-19,Z=-2. Connecting
skins keep source upper roots and axle/brace junctions attached. Their inner
source corner at abs(X)=3,Z=-13 stays at or below Z=-5.25, leaving 0.25 unit
clearance below its root chord and preventing a crossing. Nose sheets split at
Z=-9, rise 4 units in each half and move aft 7 in the second half. Upper roots
stay fixed; nose stow bounds are X=-2..2,Y=28..33,Z=-5..-1.

These gear paths and interior clearances are fitted. Require rigid wheels,
exact deployment, near-stow inspection, intact roots and no introduced polygon
crossings. A moving, finite panel alone does not pass.

## A310 control and gear fit

Agent-authored continuous motion, 2026-10-05, over A310.SH at two-thirds foot per
unit. Keep the neutral rudder and rotate it `0.35*rudder` about its diagonal
source edge. Split a trailing elevator strip at
`Y+112+0.5*(abs(X)-7)=0`, Z=11, and rotate it `-0.30*elevator`, retaining the
cut and tail-root endpoints. Outer trailing roll triangles are independent of
flaps: keep their thick forward edges fixed and move the shared trailing point
through `-0.20*aileron` around the mean outward hinge. Morph flaps to their own
exact source down poses.

Main rigid wheel regions begin below Z=-12. They move 16 units aft before
rising 19 units. Source roots at Z>=-9 stay fixed; the connecting region deforms
between those roots and the rigid cut. The painted upper strut becomes an
attached diagonal under this approximation. Main wheels stow at abs(X)<=11,
Y=-45..-33,Z=-2..7. The entire nose wheel/shaft/brace folds rigidly aft through
186 degrees around its front-view source attachment line at Y=63,Z=-8. Its
transparent side-quad margins move with it, rather than acting as extra hinges.

The complete rigid nose fits abs(X)<=2,Y=59..71,Z=-10..6 and stays above the
source belly `Z=-10+0.25*(Y-59)`. Both upper and lower body-section containment
were checked separately. Hide added gear at zero. All travel, sequencing and
control-role assignments remain fitted; unlocated device articulation and
original mechanical timing remain unknown.


## Mi-24 gear fit

Agent-authored retraction, 2026-10-05. HIND.SH supplies three opposite pairs
of complete wheel/strut texture panels. Rotate each main assembly rigidly
outward and up by 160 degrees during closing, around its source upper edge at
X=+/-7,Z=-17, parallel to Y. The nose assembly folds rigidly aft by 90 degrees
around its forward upper source corner [0,43,-17], parallel to X. Hide added
gear at zero. These folds preserve the source deployed endpoint and panel
sizes; actual mechanical axes and timing remain unknown.

Complete stowed main panels fit abs(X)=6.7..7.1,Y=-23..-13,Z=-17..-3. The
nose fits X=0,Y=30..43,Z=-17..-9. All 24 original panel corners fit sampled
neutral-body cross sections at stow. Check independent source roots, wheel
rigidity, side separation and a near-zero pose before hiding. Rotor cyclic
motion remains in the [rotor contract](rotor-presentation.md).


## C-130 and AC-130 control fits

Agent-authored partitions and motion, 2026-10-05. Both exact source shapes only
name a gear branch; the control assignments below are fitted. Preserve each
shape's own asymmetries, original artwork, nacelle corridors and common
propeller overlay. AC-130 barrel aiming remains a separate shared overlay.
Coordinates use two-thirds foot per unit.

| Component | C-130 | AC-130 |
| --- | --- | --- |
| Rudder | Split aft fin at the diagonal through [0,-67,11] and [0,-61,52]; rotate rear strip `0.35*rudder` | Split aft fin behind Y=-80; rotate rear strip `0.35*rudder` about its fixed vertical cut |
| Pitch | Tail cut `Y+64+0.1*(abs(X)-8)=0`, axis Z=9, angle `-0.30*elevator`; preserve aft root points | Tail cut `Y+78+0.05*(abs(X)-10)=0`, axis Z=4.5, same angle; preserve roots [+/-4,-90,5] and quantized hinge points |
| Wing hinge | `Y=-8+(abs(X)-28)*9/70` | `Y=-16+(abs(X)-26)*10/71`, Z=6 |
| Flaps | Independent trailing strips at abs(X)=14..21 and 33..46, down 0.45 radians | Independent strips at abs(X)=13..20 and 34..45, down 0.45 radians |
| Roll | Separate strips at abs(X)=60..92, opposed 0.20-radian travel | Separate strips at abs(X)=62..90, opposed 0.20-radian travel |

C-130 gear artwork contains three complete wheel/strut assemblies. Rotate the
main panels rigidly outward through 150 degrees about their own upper edges;
rotate the nose aft through 90 degrees about [0,49,-13]. Preserve roots and
hide at zero. One unchanged right upper image-margin corner lies 0.1042 unit
outside the source body hull; its original pixel is transparent index 255.
Do not distort the painted assembly to force that margin into the hull.

AC-130 mains are two-wheel/fairing cards, and the nose has isolated wheel cards
without separate strut meshes or reviewed body attachment vertices. Recess
complete cards rigidly: left displacement [2,0,10], right [-3,0,10], nose
[0,0,8], each multiplied by closing fraction. Mains stow at X=+/-9,
Y=-9..8,Z=-10..-4; nose at X=-2/+1,Y=40..44,Z=-12..-8. Hide at zero. This
is fitted recession into the source body, not a claim of pinned mechanical
hinges. Do not fabricate a strut or use image margins as joints.

AC-130 hook command capability remains enabled by its PT data. Whole decoded
shape review has not located supported visual hook geometry. That mapping is
explicitly unknown, not absent or complete. Next research is review of bounded
unselected drawing records and the exact PT hook consumer. These fits do not
alter gun selection, tracking, firing, AI, or flight-force behavior.


## A-10 control and exposed-gear fit

Agent-authored fit, 2026-10-05. Keep exact A10.PT/A10.SH identities. The
separate aft tail skins rotate `-0.30*elevator` about their source span seams at
Y=-66,Z=1. Inner trailing roll skins retain their distinct Y=-6 forward edges;
shared trailing points follow `-0.20*aileron` around outward mean seams, right
[24,-6,-3.5] to [47,-6,-1.5], left [-25,-6,-3.5] to [-48,-6,-1.5]. Outer
flaps remain independent and morph to their exact source down endpoints,
including both original end closures. A neutral thick closure is a boundary
wedge, not necessarily a collapsed triangle.

Split all solid and cutout fin skins at Y=-63. Keep forward pieces fixed and
move aft points in X by `tan(0.35*rudder)*(-63-Y)`, retaining Y/Z. This fitted
shear preserves registration of thick skins and original cutout art; it is not
a recovered rigid rudder mechanism.

Complete crossed wheel/strut images fold forward around painted roots, mains
[-23,-1,-8] and [21,-1,-8] through 90 degrees, nose [0,38,-4] through 100
degrees. Collinear UV markers expose those roots without changing the outline.
**Retain original main wheel cutouts at zero gear**, an agent choice because
the tested rigid footprint does not fit completely inside the source fairings.
Do not shrink tires or hide remaining painted pixels. The fitted exposed stow
has at most 2.76 source units, 0.92 ft, of painted lower exposure; this is not
claimed as recovered retail behavior. Main bounds are Y=-1..13,Z=-12..-4,
left X=-26..-20, right X=19..24. The rigid nose hides after stowing; all 1,794
opaque source samples must fit inspected neutral-body sections at near-zero.

Independent main doors keep their source upper edges at Y=-5,Z=-9 and close
through pi-atan(5). The nose front panel uses Y=35,Z=-4 and pi-atan(11).
The nose side door uses X=3,Z=-4 and pi/2 about Y. Nose and doors hide at zero.
Visual brake articulation remains unknown. No hook or afterburner is added.

## AV-8B control, nozzle and gear fit

Agent-authored continuous motion, 2026-10-05. Source facts and the complete
central-gear ground-contact correction are in the
[Harrier evidence](../formats/aircraft-control-surfaces.md#harrier-central-landing-gear-evidence).
Tail roots stay fixed while distal skins rotate `-0.30*elevator`. Split the
fin at `Y=-73-0.20*(Z-5)` and rotate the trailing region `0.35*rudder` about
[0,-73,5], direction [0,-4,20]. Flaps morph to exact source down endpoints,
independently of outer roll. Outer roll uses its own diagonal source hinges,
left [-27,-33,0] to [-49,-30,-3], right [28,-33,1] to [49,-30,-3], through
`-0.20*aileron` on outward axes. Preserve inner outrigger strips.

Rotate each original nozzle card about its own center: rear [+/-8,-14.5,-3.5],
front [+/-11,3,-2.5], about X by the simulated nozzle angle: 0 aft, 90
vertical, 100 at the braking stop (the PT's `vtLimitDown`, VTOL overhaul
slice P4, 2026-10-08). The drawn angle is the flight model's actual nozzle
angle in degrees. There is no yaw term: neither jet vectors sideways. All
four centers and card sizes remain fixed, including shared vertices of the
split left rear card. This expresses the force direction without adding a
new nozzle mesh. Check 0, 25, 50, 75, 90 and 100 degrees, each with the old
vector yaw at 0 and at full travel (it must not move the cards).

Own all sixteen gear pieces, including the eight always-present central pieces.
Outrigger wheels rise 14 units and move inward 3 units in the second half of
closing; their upper legs keep source roots fixed. Central main wheel cards
rise 14 and move aft 4 in the second half, while the brace's lower edge follows
as one rigid two-unit edge. Nose cards split at Z=-13 above the painted tire;
rigid lower pieces rise 15 and move aft 8 in the second half, with original
upper roots fixed. Hide all sixteen at zero; deployment reproduces the complete
source geometry. The corresponding contact fit is 7 ft, including the central
nose wheel. Source mechanical timing and unlocated brake presentation remain
unknown; no compatibility adapter or flight-force law changes here.


## E-3 control and gear fit

Agent-authored motion, 2026-10-05, using exact E3.PT/AWACS.SH. Keep neutral
rudder skins and rotate them `0.35*rudder` around their diagonal source edge.
Pitch uses a trailing tail cut `Y+107+0.30*(abs(X)-7)=0`, at Z=9, with
`-0.30*elevator` around rightward axes and fixed actual roots. Preserve original
left/right asymmetries.

Morph flap skins to exact source down endpoints, then move shared trailing
vertices through `-0.20*aileron` around the outward mean hinge axes. Keep both
thick leading-skin edges fixed. Original closures follow the same endpoint
correspondence and appear when either flap or roll is nonzero. This differential
flaperon assignment is fitted; test all 25 flap/roll combinations.

Fold complete crossed gear assemblies rigidly around fitted pivots: right
[4.525,-26,-3.65] and left [-4.22,-26,-3.94], forward axes with outward
half-turns; nose [0,72,-7], lateral axis with an aft half-turn. The main pivots
are fitted near the upper strut region, not recovered shafts or source vertices.
Do not independently pin transparent rectangle margins. Hide gear at zero.

Full stow boxes are right X=0.05..6.05,Y=-34..-18,Z=-6.3..10.7; left
X=-6.44..-0.44,Y=-34..-18,Z=-5.88..10.12; nose X=-2..2,Y=67..75,Z=-7..5.
Source-body section tests include complete cards and their interior grids.
The closest raw card margins are transparent; report their conservative gap
separately from the indexed-art wheel/strut clearance. The source afterburner
alias adds no flame, so it is not connected to exhaust demand. Radome animation
and original timing remain unreviewed.

## Yak-141 controls and nozzle

Agent fits, 2026-10-05. Preserve the asymmetric source tail roots. Distal tail
points use -0.30 radians times elevator around the lateral mid-root line.
Twin rudders use rear cuts `Y=-74-(5/21)*(Z-6)` and rotate 0.35 radians times
rudder around each actual diagonal cut. Outer roll strips begin at X=-29/+30,
with outward axes [-25,-6,0]/[25,-6,0] at Y=-28.5,Z=3; rotate -0.20 radians
times aileron while retaining the cut. Inboard flaps morph to their own exact
down endpoints independently of roll.

The nozzle's front ring at Y=-47 remains fixed. Its eight-point outlet moves
rigidly around [0.5,-47,-3], pitching around X by the simulated nozzle angle
in degrees: 0 aft, 90 vertical, 100 at the braking stop (VTOL overhaul slice
P4, 2026-10-08). There is no yaw term: the Yak-141 does not vector sideways.
Original shell faces connect that moving outlet to the fixed ring. This is a
flexible-neck fit, not recovered mechanics. No lift-fan or door art is invented:
the reviewed shape has no separate lift-engine intake or exhaust door faces.

Keep painted wheel circles rigid. Split right main, left main and nose cards
at Z=-13,-12,-14 respectively. The main lower pieces lift 16 units, moving
inward 2 units during the second half of retraction; nose lower pieces lift
15 units and move aft 6 units during that half. Upper attachment edges remain
fixed while their lower boundaries follow the wheels. The always-present nose
cards retract too. Hide all gear only at zero; deployment preserves source
geometry. These deformable upper connectors and stow paths are authored fits.

## V-22 controls and gear

Agent fits, 2026-10-05. Pitch moves tail strips aft of Y=-123 by -0.30 radians
times elevator. Rudders use `Y=-128-(4/29)*(Z-1)` and rotate 0.35 radians times
rudder around their own diagonal cuts. Fixed fins and all cut points remain
unchanged. Flaperons first morph to their own source flap endpoints, then move
only their joined trailing vertices by -0.20 radians times aileron around
outward mean axes [±75,9,0], rooted at [-20,-4,10.5]/[19,-4,10.5]. Keep both
thick forward edges fixed. Original wingtip closures follow the same trailing
point; do not overlap them with duplicate source down-cap triangles.

All twelve shaft/wheel cards stay rigid. Mains close through opposite half-turns
around source Y at their painted shaft centers [±23,-4,-29]. Nose gear closes
aft 90 degrees around [0,82,-28]. Fully deployed positions are exact; gear zero
hides all twelve cards. Nacelles and selected propeller cards retain the
[shared rotor rules](rotor-presentation.md), including conversion around
[±102,0,12] and propeller hubs [±102,51,16]. Blade feathering remains unknown.

## Il-76 controls and gear

Agent fits, 2026-10-05. Rudder rotates 0.35 radians times yaw around the source
trailing-fin edge. Pitch moves aft of `Y=-113-0.30*(abs(X)-2)` through -0.30
radians times elevator around rightward axes at Z=58, retaining actual roots.
Separate outer trailing triangles carry -0.20 radians times aileron, with both
thick forward edges and wingtip fixed. Inner flaps and their caps morph to
exact source endpoints independently of roll; their source control clearance
must not be mistaken for a torn shared skin.

All five crossed strut/wheel assemblies stay rigid. Each main closes outward
180 degrees about the forward axis at X=±7.5,Z=-17, retaining fore/aft spacing.
Nose gear folds aft 180 degrees around [0,45,-15]. Hide only at zero deployment.
These are authored attachment and stow fits, not measured original mechanisms.

## E-2 controls, hook and gear

Agent fits, 2026-10-05. Four rudder strips aft of Y=-42 turn 0.35 radians times
yaw, retaining each canted cut edge. Pitch strips aft of
`Y=-40-0.05*abs(X)` move -0.30 radians times elevator only in span corridors
abs(X)=2..9 and 13..19, leaving body and fin attachments fixed. Flaperons morph
to asymmetric source endpoints and add -0.20 radians times aileron around
own outward mean hinges. Source closures appear when either demand is nonzero.
Do not thicken the source-degenerate left cap to resemble the right.

Main leg/wheel assemblies fold forward 115 degrees around left [-16,0,-7]
and right [16,0,-6], preserving their one-unit leg/wheel lateral offset.
Nose gear folds aft 90 degrees around [0,30,-7]. Hook folds aft 90 degrees
around [0,-13,-8], lifting into its stowed envelope. Gear and hook disappear
only at zero. Retain the shared phase-selected propeller overlay. Original
radome motion and mechanical timing remain unknown.

## Boeing 747 controls and gear

Agent fits, 2026-10-05. Rudder rotates 0.35 radians times yaw around its source
edge [0,-155,15] to [0,-186,58]; the adjacent upper fin remains fixed. Pitch
strips lie aft of `Y=-166-0.5*(abs(X)-8)`, moving -0.30 radians times elevator
about rising rightward axes [1,-side*0.5,side*4/43] through [side*8,-166,12].
Retain actual body roots and cut edges. Independent roll strips occupy
abs(X)=118..145 aft of `Y=-57-(abs(X)-113)*23/41`. Their outward mean axes
[side*41,-23,4.5] pass through [side*113,-57,4.5]; travel is -0.20 radians
times aileron. Engine attachment corridors and wingtips remain fixed.

Flaps preserve their exact neutral and down surfaces, including triangle
interiors. The right lower panel needs a four-triangle common subdivision to
preserve both different source diagonals. Interpolate its center and corners.
Use original neutral material at zero and original down material for positive
flap, avoiding an interpolation across unrelated atlas regions. This material
switch and continuous geometry are fitted timing choices.

All 36 gear faces form five rigid assemblies. Main bogies close outward
180 degrees around forward axes at X=±6.5,Z=-12.5, retaining fore/aft spacing.
Nose gear closes aft 165 degrees around [0,100.5,-16], on its painted upper
shaft. All source deployment positions remain exact; hide only at zero.
Complete deployed gear retains the existing 20-foot ground clearance.
