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

Agent choices, explicitly fitted. The A-7 and F-4 family use the aircraft-specific rules
below; other variety rigs retain these initial rules pending individual review.

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
own reviewed groups and fitted motion. Outside the corrected A-7 and F-4 family, remaining flap/elevator/aileron groups,
VTOL jet nozzle geometry, unlocated hook geometry and live mirror masks for new
cockpit families are still incomplete. Static parts remain original geometry;
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
| Hook | Preserve both reviewed source root-edge points. Rotate distal vertices toward stow around fitted mean root `[0,-17,-12.5]`, lateral axis, by `1.2 * (1-hook)` radians. Hide at zero deployment. |
| Main gear | Fitted two-stage linkage: as gear decreases 1 to 0.5, wheel centers move from X=+/-11 to +/-3 at Y=-2,Z=-22 while tilting 0.70 radians. From 0.5 to 0, centers rise to Z=-10. Upper vertices with Z>=-13 remain fixed; lower vertices use weight `clamp((-13-Z)/6,0,1)` toward the common linkage transform. Wheels remain rigid; connecting skins deform. This replaces an inward hinge fit that crossed the wheels. |
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
stowed wheel bounds inside abs(X)<=5,Z=-13..-7 source units. Inspect top, side
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
