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

## Fitted motion

Agent choices, explicitly fitted:

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
own reviewed groups and fitted motion. Remaining flap/elevator/aileron groups,
VTOL jet nozzle geometry, unlocated hook geometry and live mirror masks for new
cockpit families are still incomplete. Static parts remain original geometry;
the import does not claim every surface is animated.

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
