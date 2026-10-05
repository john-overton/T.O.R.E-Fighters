# Aircraft control-surface geometry

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research evidence, 2026-10-05. Bounded SH readers inspected user-owned FA_2
resources; no imported instructions were executed. Coordinates are source
right, forward, up. The fighter shapes use one-third foot per unit; A310 uses two-thirds. Source archive
identity is recorded in the [variety inventory](aircraft-variety.md). Geometry
and branch membership establish these facts; continuous mechanical motion and
control assignments without an alias remain fitted in the
[presentation contract](../spec/variety-animation.md).

## Reviewed identities

| Aircraft | Shape | CODE bytes | Neutral faces | SHA-256 |
| --- | --- | ---: | ---: | --- |
| A-7E | A7.SH | 23270 | 261 | `6ca7e3d3aca0c192e2366566dadb9dcbf7b51d4295e0c4299759f4813efcee7c` |
| F-4B/J | F4J.SH | 25222 | 309 | `1564aab555b5e2ee243c9097b5323fd3bc2b91e1c29cc6bfdefdc253523b8712` |
| F-4E | F4E.SH | 20000 | 213 | `6ada13b67d0362eef93be98e323aef455aed103ce9528ea307cbe2478b033e45` |
| F-4G, F4.PT | F4.SH | 21456 | 233 | `bdca8778f8ff72e1bbe18b5312f210028a9a8cbd604bd1495ff3912e64664877` |

## Neutral rudders

The signed rudder branches replace the following neutral skins. The former
host rig installed the positive branch at control zero, then rotated it around
its bounding box. That displaced the shared source hinge. Keep the neutral
skins as the continuous animation's rest geometry.

| Shape | Neutral opposite skins | Shared fixed edge |
| --- | --- | --- |
| A7 | 0x4674, 0x4693 | [0,-65,38] to [0,-53,9] |
| F4J | 0x4a55, 0x4a7c | [0,-79,10] to [0,-88,29] |
| F4E | 0x41ef, 0x420e | [0,-57,8] to [0,-62,21] |
| F4 | 0x4118, 0x413f | [0,-66,9] to [0,-71,23] |

Signed source poses are quantized and are not perfectly symmetric. They do
not establish an exact live travel angle or input sign convention. A7 fixed fin
sections below Z=9 and above Z=38 stay static in the source branches. Their
non-hinge corners can separate from the moving rudder, a control clearance
rather than a detached leading hinge.

## Flap endpoints

Named left/right flap guards provide neutral and down skins. Raw state -1
selects the down endpoint. Raw +1 removes neutral skins without providing an
up endpoint; it must not be treated as an authored up pose.

| Shape | Left neutral to down | Right neutral to down |
| --- | --- | --- |
| A7 | 0x4a6a/0x4a91 to 0x49dc/0x4a03, closure 0x4a2a | 0x4963/0x498a to 0x48d5/0x48fc, closure 0x4923 |
| F4J | 0x52af/0x52d6 to 0x520c/0x522d/0x524e/0x526f | 0x519f/0x51c6 to 0x50fc/0x511d/0x513e/0x515f |
| F4E | 0x404a/0x4069 to 0x40bb/0x40e2/0x4101 | 0x3f53/0x3f72 to 0x3fc4/0x3feb/0x400a |
| F4 | 0x3f73/0x3f92 to 0x3fd4/0x3feb/0x400a | 0x3e9c/0x3ebb to 0x3efd/0x3f14/0x3f33 |

F4J deployed skins subdivide neutral quads into triangles. A7/F4E/F4 include
separate thick upper/lower hinge edges. Preserve endpoint correspondence and
shared trailing vertices instead of assuming a single rigid rotation. Adjacent
fixed wing and other-control skins remain static in the source flap branches;
shared non-hinge corners do not establish required flap/aileron coupling.

## Attachments and limits of the evidence

The A7 hook's source root edge is [0,-16,-14] to [0,-18,-11]. The F4J hook's
edge is [0,-55,-7] to [0,-56,-5]. These edges belong to opposite skins of one
blade. F4E/G do not offer a hook through their reviewed PT capability; no hook
geometry was located in this review.

Horizontal tail and outer trailing-wing polygons provide bounded candidates
for pitch/roll surfaces, but no independent pitch or aileron alias establishes
their live law. Source geometry and texture panels identify gear attachments
and deployed endpoints, not a full retraction mechanism. All fitted region
cuts, pivots, skin constraints, travel and timing must remain labeled as such.
The runtime guards verify the exact face sets and attachment vertices.

Detailed local evidence is retained in `.local/animation-audit/a7-geometry.json`,
`f4-geometry.json` and `f4-source-review.md`. These contain user-owned geometry
and stay ignored. Pose acceptance and remaining aircraft are recorded in the
[animation audit](../baselines/aircraft-animations.md).


## F-15C source additions

F15.SH SHA-256 is
`9f47b31a247bea282b5f2ab0656a39abe158e71dbe257a3ee617a334e710e8ad`;
_F15.PIC is
`7ea79e88a3e6bdd31d23bdee017d07f6cffbc40e43fd9b91dc9b97ec5ce48374`.
The bounded reader finds 30,762 CODE bytes, 346 neutral faces and five state
words: 0x8800 flame, 0x8806 brake, 0x880c gear, and 0x8818/0x881e flaps.
These branches add eight flame, four brake and 22 gear faces. Flap state -1
replaces neutral skins and supplies separate end closures.

Neutral flap pairs 0x5c3e/0x5c5d map to down 0x5cc6/0x5ca7, with closure
0x5ce5. The opposite pair 0x5b57/0x5b76 maps to 0x5bdf/0x5bc0, closure
0x5bfe. Source vertex order differs between skins and must be matched by
shared leading and trailing geometry.

Nose side panels 0x5a82/0x5aa1 use the same wheel/strut/diagonal-brace cutout,
with rectangle Y=50..64,Z=-27..-8. Texture UV [74,230] reaches the forward
upper corner [0,64,-8]. Front panels 0x5aee/0x5b0d show the same assembly
from its narrow axis. The untextured 0x5ac0/0x5ad7 panels are separate from
that wheel cutout and have an upper edge at Y=52,Z=-8. The texture rectangle
corners do not establish multiple fixed strut joints. The chosen continuous
fold remains [fitted](../spec/variety-animation.md#f-15c-control-and-device-fit).

Local source evidence is `single-geometry.json`, `f15-rig-plan.json` and the
F-15 atlas inspection under `.local/animation-audit/`. Retail retraction timing
and exact mechanisms remain unknown; a live retail comparison is unavailable.


## MiG-17F source additions

M17.SH SHA-256 is
`55552c8524853466cfa5cb482ce3be9b40cc4dbde99f25d6e1e488400b406939`;
_M17.PIC is
`07a629908ac897b3c2e9e03f132a0c58effc1bfe8d210d7d1e63fb053aedc9c5`.
The reviewed shape has 20,602 CODE bytes, 273 neutral faces and state words
0x6050, 0x6056, 0x605c, 0x6068 and 0x606e. Flame/brake/gear branches add
5/10/22 faces. No source rudder alias establishes the fitted yaw law.

Neutral rudder/cap skins have thick, distinct forward vertices but common
trailing points. The source flap down branch adds geometry without removing
neutral skins, and its front coordinates differ by 1..2 units. Overlaying both
branches or treating the extra quad as an end-cap triangle is not supported.
The deliberate anchored flap approximation is recorded in the
[MiG-17 fit](../spec/variety-animation.md#mig-17f-control-and-device-fit).

Atlas inspection distinguishes wheel/strut panels 0x4713/0x4732 and
0x483f/0x485e from camouflaged doors 0x4751/0x4770 and 0x487d/0x489c.
The nose pair 0x4a16/0x4a35 contains one wheel/strut cutout. Its painted top
band at V=143,U=225..233 maps to Y=32.7838..34.2973,Z=-6. The transparent
rectangle corners are not separate painted attachment points. These are
source-art observations; the chosen axis and travel remain fitted.

Evidence: `.local/animation-audit/mig17-geometry.json`, the original atlas
inspection sheet, and `mig17-painted-root-review.json`. Original mechanical
linkages, exact live deflections and timing remain unknown.


## F-16C and F-104N source additions

| Shape | CODE bytes | Neutral faces | SHA-256 |
| --- | ---: | ---: | --- |
| F16.SH | 31488 | 356 | `7d570f1f705cfaa6b1288653e47e4c502d7606109044f306972bd6c63161f315` |
| F104.SH | 28374 | 297 | `2ee08d6c1f0538e2c3060a7989567c002b7a0820614a7c124947f17981cdd352` |

F16 neutral rudder 0x6323/0x634a has the exact edge [0,-40,11] to [0,-54,37].
Its paired horizontal-tail root chords span X=+/-10,Y=-32..-59,Z=1. Source
flaps have four skins/closures on each side. Right lower inboard triangle
0x5fb4 matches deployed 0x607b through vertex order [1,2,0], rather than a
blanket order shared with neighboring skins. Nose brace 0x5e1f/0x5e36 has a
one-unit distal pin edge, which must stay intact under the fitted motion.

F104 neutral rudder 0x5646/0x566e has edge [0,-53,9] to [0,-55,21]. Its high
tail shares a centerline root chord at Y=-49..-73,Z=21. The flap down branch
provides closure 0x5920 on the left only. Main wheel skins are separate from
larger leg/bridge sheets and retain different longitudinal extents on each
side. Hook 0x6134/0x6154 has near edge [0,-32,-6] to [0,-35,-6]. Neither a
mirrored missing closure nor symmetric replacement wheel geometry is supported.

Local source inspection and original atlas crops are under
`.local/animation-audit/`. Continuous laws and limitations are in the
[F-16/F-104 fit](../spec/variety-animation.md#f-16c-and-f-104n-control-fits).

## A310 source additions

A310.SH SHA-256 is
`95602bbe6384c3a3ca0b20f7e8c5b35d9baba4283f86d490dd1da8fb7091fce4`,
with 20,868 CODE bytes, 248 neutral faces and exponent 9. Neutral rudder
0x484b/0x4872 shares edge [0,-105,17] to [0,-127,57]. Flap state -1 is down;
+1 omits skins. Neutral/down pairs are 0x4704/0x46ba and 0x472b/0x469b on
the left, 0x4622/0x45d8 and 0x4649/0x45b9 on the right. Separate trailing
triangles provide fitted roll candidates; the horizontal tail has no independent
pitch alias.

Original gear artwork contains crossed views of assemblies, rather than a
separate door for every panel. Main tires fit below source Z=-12. The nose
shaft/wheel/brace front-view attachment is X=-2..2,Y=63,Z=-8; the sloping
side-quad upper corners are image margins. Source body sections independently
bound the stow envelope used by the [A310 fit](../spec/variety-animation.md#a310-control-and-gear-fit).
Local source evidence is `transport-geometry.json`, `transport-source-review.md`
and the original gear atlas inspection under `.local/animation-audit/`.


## Harrier central landing-gear evidence

AV8.SH SHA-256 is
`00b8a8b4001cef06cf6b41b584481deaa4fd32e8d17c64045af075bd94f67df7`,
with 18,014 CODE bytes, 270 neutral faces and state words 0x5640, 0x564c,
0x5652. Its neutral geometry already includes central gear skins 0x43dd/0x43fc
(nose) and 0x42f6/0x4315/0x4334/0x4353 (main), plus brace 0x4372/0x4391.
The switched gear branch supplies eight additional outrigger faces. The nose
skins reach source Z=-21, below the outrigger minimum Z=-17. At one-third foot
per unit, the complete deployed mesh needs the
[7 ft contact fit](../spec/variety-flight.md). Counting only added faces misses
both central retraction ownership and the lowest ground-contact geometry.
Local evidence is `.local/animation-audit/av8-neutral-ground.json` and the
independent AV8 source/atlas review.


## Hercules family source additions

Shape hashes and original propeller groups are in
[rotor geometry](variety-rotors.md). C130.SH has 10,812 CODE bytes and 144
neutral faces, with gear state 0x3a30. AC130.SH has 19,666 CODE bytes and 339
neutral faces, with gear state 0x5cc0. Both use exponent 9. No signed primary
control branch was recovered; those assignments remain fitted.

C130 gear pairs are left 0x2909/0x292c, right 0x294f/0x2972, nose
0x2995/0x29b0. Original atlas samples show complete wheel/strut cards. Main
upper edges are X=-8/+9,Y=-7..5,Z=-12. The unchanged right corner [9,5,-12]
is 0.1042 unit beyond the triangulated body hull; UV [145,269] is source
palette index 255 on cutout subtype 0x6c, so this is transparent image margin.

AC130 mains 0x491c/0x4979 and 0x493b/0x495a contain two-wheel/fairing images.
Nose pairs 0x4998/0x49b7 and 0x49d6/0x49f5 contain isolated wheel images.
There is no separate strut or exact neutral-body vertex attachment in the
selected gear branch. A tail/underside review of all 339 decoded neutral faces,
signed named states and line records found no supported hook group. PT hook
command capability is a separate fact; visual mapping remains unknown.

Local evidence: `transport-geometry.json`, `transport-source-review.md`,
`c130-gear-independent-review.json` and `ac130-gear-independent-review.json`
under `.local/animation-audit/`. Continuous behavior and the hook research gap
are recorded in the [Hercules fit](../spec/variety-animation.md#c-130-and-ac-130-control-fits).


## A-10 source additions

A10.SH SHA-256 is
`982c980cb0af161eeb3d9a03ed65758ee06084e35d7a41dbed2cbcc05b7624c2`,
with 19,854 CODE bytes, 303 neutral faces and exponent 8. Reviewed aliases
are gearDown 0x5d70, gearPos 0x5d76, leftFlap 0x5d7c, rightFlap 0x5d82.
The PT has flags 0x11, airBrakesDrag=0 and aftThrust=0. No source hook bit or
nonzero afterburner capability is present; brake visual behavior remains unknown.

Separate aft tail pairs have exact span seams at Y=-66,Z=1. Solid fin skins
and cutout overlays have different offsets and thicknesses, so a shared fitted
partition must preserve art registration rather than rotate them about unrelated
bounds. Source flap down branches provide matched skins plus independent inboard
and outboard closures. Gear artwork separates complete crossed wheel/strut
assemblies from untextured doors. Local evidence is `a10-geometry.json`,
`a10-source-review.md` and original atlas/root inspection under
`.local/animation-audit/`. The deliberate exposed-main stow is an
[agent fit](../spec/variety-animation.md#a-10-control-and-exposed-gear-fit), not a
source fact about retail retraction.


## E-3 source additions

AWACS.SH SHA-256 is
`a82b3790e0582be838c13ab57e04abe8ab590170476b86a8c1676d73c3688699`,
with 26,794 CODE bytes, 296 neutral faces and exponent 9. Neutral rudder
0x51db/0x51f2 shares edge [0,-104,13] to [0,-109,38]. Flap pairs
0x5003/0x5022 and 0x4eb9/0x4ed8 map to exact down skins
0x50ba/0x50d9 and 0x4f70/0x4f8f, with two original closures per side.
Signed +1 omits those panels. Neighboring tip triangles alone do not establish
a complete independent aileron.

The original atlas shows four crossed strut/wheel faces per main assembly and
three nose faces, rather than a separate door per rectangle. The afterburner
alias removes static engine/fin geometry and adds no flame. Continuous motion
and selected pivots are in the [E-3 fit](../spec/variety-animation.md#e-3-control-and-gear-fit).
Source branches and atlas evidence are in the shared local transport review;
`awacs-painted-wheel-gap.json` distinguishes 12,044 opaque lower-region samples
from transparent card margins.
