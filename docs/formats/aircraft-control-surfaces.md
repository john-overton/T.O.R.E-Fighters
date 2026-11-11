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

Whole-panel stabilators, 2026-11-09 (slice TR-10). The F-4 family rig pinned both
root chord points of each all-moving stabilator (F-4B/J `346e`/`3489` and
`3613`/`362e`, F-4E `2b15`/`2b30` and `2405`/`2420`, F-4G `281c`/`282f` and
`233f`/`2352`), so every quad straddled the pin and folded (rule 10's fold check:
two moved and two pinned vertices per face; a geometry check changed an
inter-vertex distance by 1.08 to 1.45 source units on pitch). Each whole quad,
both windings, now turns rigidly about the lateral spindle through its root
chord midpoint, tilted by that panel's own anhedral (`0.48` J, `0.46` E, `0.45`
G) so the spindle lies in the surface. The panels are pitch only, so no aileron
term is added, and no root corner is pinned, so the root chord midpoint is the
invariant and the small root gap is accepted. See the
[F-4 family fit](../spec/variety-animation.md#f-4-family-surface-repair).

The A-7 tail needed no code change. Its `2be6`/`2c59` and `1f3a`/`3506`
triangles already turn whole about the root chord line (3 of 3 vertices move, 0
pinned), and a geometry check found 0 source-unit change on pitch, so rule 10
passes. See the
[A-7 fit](../spec/variety-animation.md#a-7-surface-repair-and-acceptance).


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

Update (B5-3, 2026-11-10, opinionated, John's decision round 2, item E3): the
nose gear no longer folds aft about the forward brace root. The real F-15's
nosewheel "retracts forward" (aircraftinformation.info, McDonnell F-15 Eagle,
airframe and flight control systems,
<https://www.aircraftinformation.info/JB_AIF/usaf_fighters/f15_2.html>,
accessed 2026-11-10; the F-15E rig of F15E-4 cites the same page). The six nose
faces (`5a82`/`5aa1` wheel sheets, `5ac0`/`5ad7` strut plates, `5aee`/`5b0d` cross
plates; source x -2 to 2, y 48 to 64, z -27 to -8) now fold forward through 90
degrees as one rigid body about the lateral line Y=48, Z=-8, behind and level
with every vertex. Gear down (1) is the source pose with the foot at z -27; the
stow is Y 48 to 67 and Z -8 to 8 inside the nose. The main legs, doors and the
F-15C text above are unchanged.

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

Each stabilator is one five-corner double-sided panel: `0x5606`/`0x564b` on the
left and `0x52cd`/`0x52e6` on the right, root chord X=+/-10 from Y=-32 to -59 at
Z=1, tip corners at X=26..29 and Z=-3 (anhedral), and no fixed root corner. Its
fitted motion became whole-panel and roll active on 2026-11-08 (slice F16A-9):
the whole panel turns about a lateral pivot spindle through the root chord midpoint and
takes a differential roll, so no part stays pinned while the rest turns. See the
[F-16/F-104 fit](../spec/variety-animation.md#f-16c-and-f-104n-control-fits).

F104 neutral rudder 0x5646/0x566e has edge [0,-53,9] to [0,-55,21]. Its high
tail shares a centerline root chord at Y=-49..-73,Z=21. The flap down branch
provides closure 0x5920 on the left only. Main wheel skins are separate from
larger leg/bridge sheets and retain different longitudinal extents on each
side. Hook 0x6134/0x6154 has near edge [0,-32,-6] to [0,-35,-6]. Neither a
mirrored missing closure nor symmetric replacement wheel geometry is supported.

Whole-panel stabilator, 2026-11-09 (slice TR-10). The high-tail rig pinned the
X=0 center chord and turned only the outboard vertices, so each stabilator half
(`2f9e`/`2fbb` left, `1f4d`/`1f6b` right) straddled that pin and folded (rule
10's fold check: two moved and two pinned vertices per face; a geometry check
changed an inter-vertex distance by 0.09 source units on pitch). Both halves now
turn rigidly about the lateral spindle through the root chord midpoint
`[0,-61,21]`, the same `-0.30*elevator` law and sign. The panel lies at a
constant Z=21, so it has no anhedral to tilt the spindle by, and no center chord
is pinned. The horizontal tail is pitch only, so no aileron term is added.

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


## Rafale source attachment review

Exact RAFALE.PT uses RAF.SH, 19,334 CODE bytes and 206 neutral faces. Existing
source identities remain in the [Rafale import baseline](../baselines/rafale-quick-mission.md).
Gear pairs 0x3c50/0x3c6f and 0x3be5/0x3c04 are opposite cutout views at
X=-4/+4,Y=5..15,Z=-18..-7. Their original atlas top row V=128 contains a
shaft band at U=214..218 and a forward brace band at U=235..239. UV [239,128]
maps to Y=14.545455,Z=-7. This painted point supports the chosen fitted pivot;
the fold direction and angle remain authored.

The nose door's source upper edge is [-1,52,-6] to [-1,68,-5], which is
sloped rather than horizontal. Upper and lower flap fronts both lie at Y=-23,
but have different Z coordinates. Preserve both thick edges when applying the
existing fitted mix. Canards remain separate rigid panels around fitted shaft
points [+/-10,35,1], not control skins that should pin every root-chord corner.
The original rudder shares edge [0,-36,5] to [0,-44,30].

Local evidence: `rafale-neutral-source.json`, `rafale-devices-source.json`,
`rafale-main-gear-atlas.png` and `rafale-painted-main-stow.json` under
`.local/animation-audit/`. Nonzero source control branches include arithmetic
outside the reviewed static grammar, so this pass retains explicitly fitted
flap/canard travel rather than inventing recovered endpoints.

## Additional individually reviewed sources

These exact FA_2 shapes were inspected through bounded data readers and their
own indexed atlases. No original runtime comparison was available. CODE lengths
and hashes identify the evidence, not an alternate aircraft identity.

| Shape | CODE bytes | Neutral faces | SHA-256 |
| --- | ---: | ---: | --- |
| Y141.SH | 21134 | 308 | `9b2fb3601090c85ab6a81cf17d6478941f01688310cbd4baf526bd6a890a330b` |
| V22.SH | 17204 | 160 | `bfd5e34dd536d1c35f04237c8da120082fa55fd46edbb188077db1a4c35cc725` |
| IL76.SH | 28644 | 309 | `c47f8738e5621c210009b3a42521a23451a3f68a9287686512ad13867edd4fd9` |
| E2C.SH | 29258 | 345 | `b00bba4377071c6077f7c1f7d4397da7f19e9c4d248ded85c9fb2231863e39ca` |
| B747.SH | 31108 | 373 | `422b894147068524d57cf3511b7c2847976142f4520e2e922714520f88ee930f` |
| F18.SH | 26934 | 282 | `d6c876d63d10a05072c8afd8a53cedffdd9cdfbcff4c4576a90c1b6c064b8bb9` |
| MIG29.SH | 29290 | 328 | `16435a8cb4ab6c5a1b9f3e6b5ee3ee5b3578ffffb42ee95fde78ca883c0c07d3` |
| SU27.SH | 12838 | 146 | `eae9ac13f399b64e632fcacbca8207be6f0d654a6fddc4027087e389c0fa79f1` |
| SU35.SH | 27114 | 329 | `431589b29f4531236a5ccb11242eda779bdc9fe76ccec8ee2c345d45a64c039b` |
| MIG21.SH | 14952 | 159 | `1c38812a2b12f17fb886e630c6eecf393807166b5664017d25e50925251d6d36` |

IL76, E2C and B747 use two-thirds foot per source unit; the others use one-third.

Y141's nose gear is already present in neutral geometry; only its main gear is
switched. Its left tail consists of one upper quad and two lower triangles with
asymmetric roots. The nozzle contains eight shell faces between Y=-47 and -53
and one eight-vertex outlet. V22 has thick flap fronts at different heights,
source down trailing endpoints and original tip closures. Its twelve gear
faces contain complete shaft/wheel views, not guessed independent doors. The
vtAngle alias produces no changed bounded draw branch; original tilt timing
remains unknown.

Whole-panel stabilators, 2026-11-09 (slice TR-7). The Yak-141 tail rig turned
only the vertices past `|x|>13` (left) or `|x|>14` (right) and held the root
edge, so every tail face straddled that threshold and folded (rule 10's fold
check: left `4b07` two moved, one seam and one pinned; `4b4f` one moved and two
held; `4b64` two moved and one pinned; right `484c`/`4894` two moved and two
held seam vertices). Each side's whole panel now turns rigidly about the lateral
spindle through its root chord midpoint (`[14,-69.5,-1]` right,
`[-12.5,-69.5,-1]` left) with the unchanged `-0.30*elevator` law. The panels lie
at Z=-1, so they have no anhedral to tilt the spindle by, and no root corner is
pinned. These tails are pitch-only: no available source states that the real
Yak-141 deflects them differentially for roll, so no aileron term is added and
the roll-mixing question stays open for John. See the
[Yak-141 controls note](../spec/variety-animation.md#yak-141-controls-and-nozzle).

IL76 has twenty cards forming five crossed strut/wheel assemblies. Its inner
flap down endpoint separates from the neighboring outer trailing triangle by
1.4907 feet, an original control clearance. No rudder alias was found. E2C has
four geometric trailing-fin candidates, asymmetric flap endpoints and a
source-degenerate left cap. Its main leg and wheel planes are offset by one
unit. The original left contact matches its nacelle; the right side cannot be
inferred by mirroring. Its selected propeller phase has four cards, with eight
alternate-phase cards excluded.

B747's right lower flap changes triangle diagonal between neutral and down.
Both endpoints are nonplanar. Neutral material is subtype 0xed, palette 148;
down material is subtype 0x64, palette 0, on a different atlas region. Checking
outer corners alone misses both the topology and material difference. Its
36 gear faces contain four bogies and a nose assembly; complete deployment
reaches Z=-30, while neutral geometry reaches only Z=-18.

F18 main gear atlases contain painted shaft/brace roots at left [-8,-7,-6]
and right [9,-7,-6]. Its nose brace and untextured doors are separate from
wheel cards. Thick flap fronts have distinct upper/lower positions. Refer to
the [F/A-18D correction](../spec/aircraft-animation.md#fa-18d-attachment-corrections)
for authored travel, including intentionally retained stow joint artwork.

MIG29 and SU27 have canted, asymmetric fins; a generic vertical hinge does not
retain their cut intersections. Both expose signed flap geometry. SU27 has
original trailing heights -5 and +3 around neutral -1, plus asymmetric slat
skins. Its hook branch is degenerate and PT capability is disabled. SU35's
right down-flap branch adds skins without removing the broad neutral upper
panel, unlike the left. Isolate the right inboard portion at X=49 rather than
copying the left X=-50 boundary. MIG21 has no signed flap/rudder aliases;
those control roles remain geometric fits. Its actual main shaft coordinates
are X=+21/-20 and its nose upper edge is sloped.

Whole-panel differential roll, 2026-11-09 (slice TR-3). The MiG-29 stabilators
were previously split at Y=-41, so the inboard leading-edge triangles stayed
fixed while the outboard quads turned. Two left faces, `4cf6` and `4dbc`,
straddle that line, so rule 10's fold check fails and the split folded them.
The whole flat plate per side, the four outboard quads plus the inboard
leading-edge triangles (`474f`/`4776`/`479e`/`47c5`/`296b`/`2989`/`4179`/`41a1`
right, `4cf6`/`4dbc`/`4d1d`/`4de3`/`4d44`/`4e0a` left), now turns rigidly about
a lateral spindle through the root chord midpoint `[±16.5,-35,0]` and takes a
fitted `0.10*aileron` differential roll, raising the right trailing edge and
lowering the left, the outboard-aileron sense, with the pitch and roll terms
summed and clamped to 0.40 rad. The plate lies at a constant Z=0, so it has no
anhedral to tilt the spindle by, and no root corner is pinned. See the
[MiG-29 attachment corrections](../spec/aircraft-animation.md#mig-29-attachment-corrections).

Whole-panel differential roll, 2026-11-09 (slice TR-4). The Su-27 tail rig turned
only the vertices past `|x|=21` and held the two vertices on the root chord line
`X=±21`, so part of each flat tail quad stayed put while the outboard end turned
(rule 10's fold check). The whole tail quad per side, both windings
(`16d5`/`170f` left, `1928`/`19be` right), now turns rigidly about a lateral
spindle through the root chord midpoint `[±21,-41,-6]` (root chord `Y=-26..-56`
at `X=±21`) and takes a fitted `0.10*aileron` differential roll, raising the
right trailing edge and lowering the left, the flaperon sense, with the pitch
and roll terms summed and clamped to 0.40 rad. The plate lies at a constant
`Z=-6`, so it has no anhedral to tilt the spindle by, and no root corner is
pinned. See the
[Su-27 attachment corrections](../spec/aircraft-animation.md#su-27-attachment-corrections).

Whole-panel differential roll, 2026-11-09 (slice TR-5). The Su-35 tail rig turned
only the vertices past `|x|=24` and held the root edge, so part of each tail face
stayed put while the outboard end turned (rule 10's fold check: every tail face
straddled the threshold). The whole tail pentagon per side, both windings
(`5312`/`535e` left, `25f2`/`2639` right), now turns rigidly about a lateral
spindle through the root chord midpoint and takes a fitted `0.10*aileron`
differential roll, raising the right trailing edge and lowering the left, the
outboard-aileron sense, with the pitch and roll terms summed and clamped to
0.40 rad. The right root chord runs `[20,-25,-5]..[17,-46,-5]` (midpoint
`[18.5,-35.5,-5]`), the left `[-20,-25,-5]..[-20,-46,-5]` (midpoint
`[-20,-35.5,-5]`). The plate lies at a constant `Z=-5`, so it has no anhedral to
tilt the spindle by, and no root corner is pinned. See the
[Su-35 attachment corrections](../spec/aircraft-animation.md#su-35-attachment-corrections).

Whole-panel canards, 2026-11-09 (slice TR-5b). The TR-5 line "The canards are
unchanged" is superseded. The Su-35 canard rig turned only the vertices past
`|x|=20`, so the right pair's root vertex at `x=19` and the left pair's root
vertices at `x=-20` stayed put while the outboard ends turned (rule 10's fold
check; the right pair straddles, and the left pair's held seam vertices make it
non-rigid too). The whole canard quad per side, both windings (`52b0`/`52d7`
left, `256d`/`2580` right), now turns rigidly about its own lateral spindle
through its root chord midpoint (`[19.5,45.5,0]` right, `[-20,45.5,0]` left),
keeping the existing `0.25*elevator` pitch law and sign and adding no roll. The
plate lies at a constant `Z=0`, so the spindle stays lateral. No root corner is
pinned, so the small root gap is accepted. See the
[Su-35 attachment corrections](../spec/aircraft-animation.md#su-35-attachment-corrections).

Whole-panel differential roll, 2026-11-09 (slice TR-6). The F-15 tail rig turned
only the vertices forward of a swept span axis and held the root edge, so part of
each five-corner stabilator stayed put while the rest turned (rule 10's fold
check: every tail face has one moved, two seam and two pinned vertices). The
whole flat panel per side, both windings (`55df`/`560c` left, `51a4`/`51d1`
right), now turns rigidly about a lateral spindle through the root chord midpoint
(`[-22,-53.5,-1]` left, `[21,-53.5,-1]` right) and takes a fitted `0.10*aileron`
differential roll, raising the right trailing edge and lowering the left, the
aileron sense, with the pitch and roll terms summed and clamped to 0.40 rad. The
panel lies at a constant `Z=-1`, so it has no anhedral to tilt the spindle by,
and no root corner is pinned. See the
[F-15C control and device fit](../spec/variety-animation.md#f-15c-control-and-device-fit).

Whole-panel stabilators, 2026-11-09 (slice TR-8). The F-22 family tail rig turned
only the vertices past `|x|=19` and held every source root point, so every tail
face (right `32ba`/`32cc`/`35ad`/`35c1`, left `3987`/`3ba9` on F22.SH; F22N and
F/A-XX use the F22N addresses) straddled that threshold and folded (rule 10's
fold check). Each side's whole flat panel now turns rigidly about the lateral
spindle through its root chord midpoint (`[18,-52.5,1]` right,
`[-18.5,-52.5,1]` left), keeping the existing `-0.30*elevator` pitch and opposed
`0.10*aileron` roll, so a positive aileron raises the right trailing edge and
lowers the left. The panels lie at a constant Z=1, so they have no anhedral to
tilt the spindle by, and no root corner is pinned. See the
[F-22 family attachment corrections](../spec/aircraft-animation.md#f-22-family-attachment-corrections).

Whole-panel stabilator, 2026-11-09 (slice TR-9). The MiG-23 tail rig turned only
the vertices past `|x|=4` and held the inboard points, so every tail face (right
`3543`/`356b`, left `3730`/`3758`) straddled that threshold and folded (rule 10's
fold check: each face has two moved, one seam and two pinned vertices). Each
side's whole flat five-corner panel now turns rigidly about the lateral spindle
through its root chord midpoint (`[3.5,-23,1]` right, `[-3.5,-23,1]` left),
keeping the existing `-0.30*elevator` pitch and opposed `0.10*aileron` roll, so a
positive aileron raises the right trailing edge and lowers the left. The panel
lies at a constant Z=1, so it has no anhedral to tilt the spindle by, and no root
corner is pinned. See the
[MiG-23 attachment and material corrections](../spec/aircraft-animation.md#mig-23-attachment-and-material-corrections).

Branch snapshots, source atlases and source-section reviews remain local under
`.local/animation-audit/`. Continuous control roles, angles and gear mechanisms
are separately documented [variety fits](../spec/variety-animation.md) and
[older-aircraft fits](../spec/aircraft-animation.md). Unlocated devices and
original timing remain unknown; inspect additional source control consumers
before asserting those behaviors.

## Su-25 source materials and attachments

SU25.SH SHA-256
`196ed566d0b891241dbfb3f66cebee9fc0829204075f589338c6c18be986e137`
has 29,626 CODE bytes and 334 neutral faces, at one-third foot per unit.
Rudder skins 0x5e68/0x5e8f share a diagonal fixed edge; signed branches replace
only those skins, leaving fin 0x572f/0x58ed static. Neutral UVs occupy atlas
X=62; signed poses widen to X=64/65 or 60/58. Constant neutral UVs on deflected
geometry therefore lose the original material correspondence. Flap down
endpoints also change upper-tip UV by two pixels. All use subtype 0xed and
_SU25.PIC; lower flap shade is 151, upper flap/rudder shade 153. Preserve
facing-skin material order separately from shared geometry.

Gear includes complete wheel cards, a separate narrow nose brace and untextured
inner panels of unresolved original role. Source top edges differ from older
fitted pivots; the nose front shaft is at Y=41,Z=-9. No afterburner alias or
switched flame group was found. Evidence is `su25-materials.txt` and bounded
`mig21-su25-mig23-geometry.json` under `.local/animation-audit/`. Continuous
interpolation and retraction use the [documented fit](../spec/aircraft-animation.md#su-25-attachment-and-material-corrections).

## F-22 donor attachment review

F22.SH SHA-256 is
`eba06b716e45431fa7401d99579eb0d4c8f1882eb80928c726186907c170f8b6`,
with 20,012 CODE bytes and 245 neutral faces. F22N.SH SHA-256 is
`736649d76b7e4aea059586f00d7c7474df777ab9ddedde14baa075a34a90d380`,
with 20,146 CODE bytes and 248 neutral faces. Both use one-third foot per unit.
F/A-XX remains a separately identified opinionated F22N donor variant.

Original inner wing fronts run from Y=-28 to -22 and have separate thick skin
heights. A single lateral hinge at Y=-22 moves their inner roots. Original
neutral/down lower panels have different subdivisions; the left neutral lower
panel is a pentagon. Both flap aliases replace inner and outer panels together
at -1, while positive values omit them. These branches do not recover continuous
control mixing. Tail roots include an intermediate right [18,-64,1] point;
whole-tail rotation had moved the full source attachment chord.

Each donor has twelve gear faces: three wheel/shaft pairs, two solid main-door
pairs and one textured nose-door pair. Main cards contain painted V braces,
with asymmetric upper planes X=-13,Z=-5 and X=12,Z=-6. Their atlas regions are
identical despite different UV locations. Solid doors are separate, at X=-16
and +18, with sloped upper edges. Original door roots already lie outside parts
of the neutral fuselage, with reviewed side gap up to 3.15 source units; the
nose door also extends beyond the tapering nose at its forward upper margin.
That source discrepancy is distinct from a newly detached animated hinge.
Complete deployment reaches Z=-23. F22N hook skins are 0x40a1/0x40c0.

Evidence: bounded `f22-family-geometry.json`, original atlas close-ups and source
section reviews under `.local/animation-audit/`. Continuous corrections are
[agent fits](../spec/aircraft-animation.md#f-22-family-attachment-corrections),
not recovered original mechanisms. Original timing remains unknown.

## X-31 and MiG-23 source endpoints

Exact F31.PT uses F31.SH, SHA-256
`96f838de26b7e867bd46f9795f48f15d3399684254af7ed1fa00bf5b1ecd9201`,
21,974 CODE bytes and 225 neutral faces. MIG23.SH SHA-256 is
`849a0e48809fd2d8141b6e1e641a6b275b2dd5a5e64f3b0c9042bc1ec628abac`,
with 23,312 CODE bytes and 219 neutral faces. Both use one-third foot per unit.
Neither source is replaced by another variant.

F31 flap aliases replace only the inner trailing skins and add separate outer
closures. Both front thickness levels remain fixed; trailing points move from
Y=-21,Z=-6 to Y=-20,Z=-8. Its outer panels stay unchanged in these branches.
Left upper panels 0x30f2/0x31aa belong with lower 0x3af8 and were omitted by
the older fitted roll mapping. Signed rudder branches retain their diagonal
front and provide both original deflection endpoints. Original wheel atlases
support separate complete main assemblies, nose wheel/strut, narrow brace and
untextured panels. Prototype vector behavior and the lack of a VTOL lever
capability are documented in the [existing flight contract](../spec/additional-aircraft.md).

MiG23 signed rudder branches replace only 0x42f9/0x4318, leaving the fixed fin.
Neutral front UV U=55/181 reverses to 181/55 in the positive branch; the negative
branch retains its order. Halfway UV interpolation would collapse both to 118.
The source skins retain subtype 0xed, shade 89 and _MIG23.PIC. Flaps have exact
position/UV down endpoints with upper shade 89 and lower 151. The swing-wing
alias produces no changed bounded draw branch, so the continuous sweep law
remains fitted. Its complete crossed main cards have three collinear front-top
vertices at absolute X=2/5/9; nose brace body and distal edges have length sqrt(5).

Local evidence: `x31-geometry.json`, original atlas/root reviews,
`mig21-su25-mig23-geometry.json` and `mig23-materials.txt` in the shared ignored
audit directory. Endpoint facts do not establish original continuous timings.

## F-14D source attachment review

Exact F14.PT uses F14.SH, SHA-256
`28be633d94fefdc286df4bc2cbe602749ebc91eb2cdbdf7c93cf50707f4a5441`,
with 29,462 CODE bytes and 313 neutral faces, at four-thirds foot per unit.
Neither F-14B nor SWPATCH geometry is substituted. Gear adds sixteen faces,
brakes four, flame eight and source hook two collapsed untextured triangles.
The existing imported F22N hook-art substitution is a separate documented
opinionated component.

Negative flap branches replace 0x540d/0x5434 and 0x4fe9/0x5010. Only inboard
trailing Z=1 becomes 0; both fronts and outer trailing points stay unchanged.
Positive branches omit the neutral skins without replacement. Original
continuous response cannot be inferred from these discrete endpoints.

Original gear art separates complete four-face assemblies per main and nose,
a narrow diagonal nose brace and an untextured panel. Painted shaft roots
support [±6,1,0] for mains and [0,17,-1] for nose. The brace and panel have
their own upper attachments; assigning them the wheel pivot moves those body
attachments. Preserve known static geometry/alignment repairs when constructing
the reference body, while testing new movement independently. Source evidence
is in `f14-geometry.json`, original atlas inspections and the shared ignored
audit directory. Corrected motion is an [agent fit](../spec/aircraft-animation.md#f-14d-attachment-corrections).

## A-4E source mechanisms and remaining gap

Exact A4E.PT uses A4.SH, SHA-256
`ee98333ab0fc419e0fb2abdd31e57ae24e672d6e4d919790c53b4bfb62d4053b`,
with 24,506 CODE bytes and 237 neutral faces, at one-third foot per unit.
Its original _A4.PIC atlas is 256x449. Pitch rear seams are diagonal; source
rudder lower roots have distinct X=±1 positions while their upper root and
trailing points coincide. Whole-panel rotation about a central line had moved
those thick lower attachments.

Each negative flap branch replaces two trailing skins while adding six
forward-wing strips and six trailing/cap faces. The forward strips overlap
unchanged broad wing skins with little exact vertex correspondence. Their
continuous role remains unknown. Trailing down endpoints reach Y=-23,Z=-12;
the lower front also shifts from Y=-18 to -17. Source UV changes are local
one/two-pixel adjustments, not a jump across unrelated atlas regions. Moving
caps and static sidewalls are distinct original groups. Their reconciliation
and the explicit lower-front discrepancy belong in the
[A-4E fit](../spec/aircraft-animation.md#a-4e-attachment-corrections).

Gear art distinguishes complete main cards, nose wheel/shaft, crossed brace,
camouflaged doors and plain doors. The original camouflaged door's front upper
corner is 1.18254 units above the sampled neutral wing upper section; do not
call it a newly detached animated hinge. Complete deployment reaches Z=-26 at
the nose, mains -24, while neutral body reaches -10. Brake groups distinguish
primary leaves, fixed cavity faces and actuator strips with asymmetric body
ends. Source stowed hooks 0x573f/0x575f have deployed counterparts
0x56eb/0x56ca with corresponding art; their front edges intersect but the
source endpoint widths/lengths differ.

Evidence is `a4e-geometry.json`, original atlas inspections and independent
source-section sweeps in the ignored audit directory. Original continuous
mechanisms/timing and the forward-strip consumers remain unknown. No original
module is executed; additional drawing/state-consumer research is the next
step for those strips.

## MiG-25 source additions

MIG25.SH SHA-256 is
`d76f8f7384305426b58543334b629aff757bedddfe2a78e6be6ccdd2393814bb`,
with 26,154 CODE bytes, 320 runtime neutral faces and five state words
`0x7600`, `0x7606`, `0x760c`, `0x7618`, `0x761e`. It names only one gear branch
(`0x760c`, twelve cards). No signed pitch, roll, yaw or flap branch exists, so
the control roles below are fitted geometric assignments; the panels and hinge
lines are exact source geometry. Coordinates use one-third foot per unit.

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right stabilator | `3daa`, `3e4a` | Flat double-sided plate at Z=-1, root chord X=18 from Y=-45 to -69, tip X=48 at Y=-83..-91 |
| Left stabilator | `434c`, `43a2` | Mirror of the right at X=-18..-48 |
| Right rudder | `320f`, `3237` | Rear fin panel behind the split line (16,-40,7) to (23,-63,38) |
| Right fixed fin | `325f`, `327d` | Forward fin panel ahead of that split, root X=16 from Y=-19 to -40 |
| Left rudder | `2c1f`, `2cb2` | Mirror behind the split line (-16,-40,8) to (-23,-63,38) |
| Left fixed fin | `2c01`, `2c48` | Forward fin panel ahead of the split |
| Right flap | `5020`, `5047` | Inboard trailing pair, hinge edge (18,-25,8) to (39,-29,5), trailing edge (18,-36,6) to (38,-39,4) |
| Left flap | `5127`, `514e` | Mirror at X=-18..-39 |
| Right aileron | `3c96`, `3ea3`, `3cb9`, `3ce2` | Outboard trailing panel, hinge edge (39,-29,5) to (70,-22,2); a forward triangle and an aft quad per skin |
| Left aileron | `41d1`, `43fb`, `421d`, `42b1` | Mirror at X=-39..-70 |

The all-moving stabilator is a zero-thickness plate, so no root corner is a
fixed hinge; it turns about its root chord midpoint. Every other surface turns
about the exact edge the fixed panel shares, so those corners do not move. The
aileron is split on a triangle diagonal: the forward triangle and the aft quad
of each skin must stay in one group, or one half would be left behind the
hinge. The two flap faces per side are exactly the pair the source words
`0x7618` (left) and `0x761e` (right) remove; M25-3 turns them.

Whole-panel differential roll, 2026-11-08 (slice TR-1). Each stabilator plate
was already one group turned about the lateral spindle through its root chord
midpoint; it now also takes a fitted `0.10*aileron` differential roll, raising
the right trailing edge and lowering the left, the outboard-aileron sense, with
the pitch and roll terms summed and clamped to 0.40 rad. The plate lies at a
constant Z, so it has no anhedral to tilt the spindle by, and no root corner is
pinned. See the [MiG-25 fit](../spec/variety-animation.md#mig-25-control-fit).

`0x760c` adds twelve gear cards (four per main, four nose, low point Z=-28):
one strut plate and one painted wheel card per main side, and a centre strut
with its cross plate at the nose. M25-4 turns each main pair as one rigid
assembly about the aft-top lateral hinge (Y=-19, Z=-10) and the nose about
(Y=45, Z=-9), tucking it to the fuselage belly plane at stow; the nose also
steers about its vertical strut axis. `0x7600` adds eight aft quads at X=±10
and Z=±8 from Y=-129 to -70: two crossed double-sided plates, one per engine,
rooted at the Y=-70 nozzle exit. M25-4 maps it to the afterburner exhaust,
scaling the plume forward from that root as the demand falls. `0x7606` adds two
raised quads near Y=3..-13, Z=9..23: one double-sided dorsal panel hinged on the
spine front edge (0,3,9). The shared base order places it at the brake slot, so
M25-4 maps it to the airbrake fraction, closing the panel flat along the spine.
The moving roles, the fitted angles and the gear stow are recorded in the
[MiG-25 fit](../spec/variety-animation.md#mig-25-control-fit).

## MiG-31 source additions

MIG31.SH SHA-256 is
`6dc4ed9cb6c99675cf7076dd38c90146e2e6300acd3c11bc565a5269f8a46a89`, with
20,816 CODE bytes, 214 runtime neutral faces and six state words `0x6120`,
`0x6126`, `0x612c`, `0x6138`, `0x613e`, `0x6144`. It names one gear branch
(`0x612c`, twelve cards), two flap-removal branches (`0x6138` the left pair,
`0x613e` the right) and one rudder branch (`0x6144`). No signed pitch or roll
branch exists, so those roles are fitted geometric assignments; the panels and
hinge lines below are exact source geometry. Coordinates use one-third foot per
unit.

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right stabilator | `3d69`, `3db1` | Flat double-sided plate at Z=-3, root chord X=17 from Y=-24 to -56, tip X=35..43 at Y=-59..-68 |
| Left stabilator | `39f6`, `3a3e` | Mirror of the right at X=-17..-43 |
| Right rudder | `448e`, `44ad` | Rear-lower fin quad behind the hinge edge (13,-42,8) to (16,-51,31); the pair the rudder word `0x6144` replaces |
| Left rudder | `44eb`, `450a` | Mirror behind (-13,-42,8) to (-16,-51,31) |
| Right flap | `41a6`, `41c5` | Inboard trailing pair, hinge edge (17,-5,4) to (45,-16,2), trailing edge (17,-16,4) to (45,-23,2); removed by `0x613e` |
| Left flap | `4260`, `427f` | Mirror at X=-17..-45; removed by `0x6138` |
| Right aileron | `3ae3`, `3c64` | Outboard trailing quad, hinge edge (45,-16,2) to (81,-9,-1) |
| Left aileron | `3893`, `3925` | Mirror at X=-45..-81 |

Every control surface is one whole source panel: a flat zero-thickness quad whose
two faces are its opposite windings, so no surface is split on a diagonal and
there is no half-panel to leave behind. The wing skin outboard of the flap and
inboard of the aileron is a separate fixed quad with a leading triangle, and the
fin's inboard root plate and tip quad are separate fixed surfaces; none of them
moves. The all-moving stabilator is a zero-thickness plate, so no root corner is
a fixed hinge; it turns about its root chord midpoint. The flap pairs are exactly
the pairs the source words `0x6138` (left) and `0x613e` (right) remove. The
rudder word `0x6144` swaps the two rear-lower fin quads `448e/44ad` and
`44eb/450a` for deflected copies `431a/4339` and `4377/4396` whose lower trailing
corner moves 4 units in +X on both sides and whose hinge corners do not move: a
yaw, the same sign on both fins. The fin's tip quad and hidden root plate stay
fixed, as in the source.

Whole-panel differential roll, 2026-11-08 (slice TR-2). Each stabilator plate was
already one group turned about the lateral spindle through its root chord
midpoint; it now also takes a fitted `0.10*aileron` differential roll, raising
the right trailing edge and lowering the left, the outboard-aileron sense, with
the pitch and roll terms summed and clamped to 0.40 rad. The plate lies at a
constant Z=-3, so it has no anhedral to tilt the spindle by, and no root corner
is pinned. See the [MiG-31 fit](../spec/variety-animation.md#mig-31-control-fit).

`0x612c` adds twelve gear cards (four per main, four nose, low point Z=-30): one
strut plate and one painted wheel card per main side, and a centre strut with its
cross plate at the nose. M31-4 turns each assembly as one rigid body about its
lateral hinge. `0x6120` adds twelve aft plates behind the tail at X -14 to 14, Y=-129
to -68, Z=-9 to 5 (the afterburner plume) and `0x6126` adds two raised dorsal
quads at X=±5, Y=32 to 50, Z=8 to 24 (the airbrake); M31-4 maps both. The moving
roles and fitted angles are recorded in the
[MiG-31 fit](../spec/variety-animation.md#mig-31-control-fit).

## Mirage 2000 control surfaces (M2K-3, 2026-11-09)

`M20.SH` (18,624 CODE bytes, 206 runtime faces, six state words `0x5890`,
`0x5896`, `0x589c`, `0x58a8`, `0x58ae`, `0x58b4`; review in
[aircraft variety](aircraft-variety.md#mirage-2000-source-review-2026-11-09))
names no signed pitch, roll or flap-travel branch, so the elevons and the rudder
are **fitted** whole-panel assignments over exact reviewed source faces. Source
coordinates below are shape units (x right, y forward, z up, one third foot
before the per-axis scale of [M2K-2](objects-and-shapes.md#aircraft-shape-scale-2026-11-09));
the rig scales every hinge point with the same factors as the shape, so a turned
panel stays rigid.

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right elevon, inboard strip | `3969` (top), `394a` (bottom) | Quads, x 3 to 17; hinge edge y -29 at z -2 (top) and z -4 (bottom), trailing edge y -32, z -3 |
| Right elevon, outboard strip | `2176`, `2391` (top), `2190`, `2233` (bottom) | Four triangles, x 17 to 33; the tip hinge vertex (33,-29,-3) is on the mid axis |
| Left elevon, inboard strip | `3a31`, `3a50` | Mirror, x -18 to -3 (the seam is at x -18, so the left inboard strip is 15 units against the right's 14) |
| Left elevon, outboard strip | `1e87`, `23c3`, `1ea1`, `1f1b` | Mirror, x -33 to -18 |
| Rudder | `3bc6`, `3be5` (quad, both windings), `2b64`, `2b7c` (tip triangles, one per winding) | Hinge edge (0,-35,4) to (0,-40,19); trailing edge (0,-39,5) to (0,-42,19); triangles (0,-40,19), (0,-42,19), (0,-42,21) |

**Hinge axes.** Elevons: the straight line y = -29, z = -3 along X, through the
whole span of both strips; the wedge turns about this mid axis (it is not a
vertex line: the hinge-edge vertices sit 1 unit above and below it, and the
outboard tip vertex is on it). Rudder: the fin trailing edge from (0,-35,4)
toward (0,-40,19), direction (0,-5,15), so a positive angle swings the trailing
edge toward +X (right), the sense of the fleet's rudders.

**Rule 11 fold table (rig definitions).** Every elevon face turns whole about the
mid axis; the only vertex that stays put is the outboard tip vertex, which is on
the axis. The rudder's two hinge vertices are on the hinge edge and stay put.

| Surface | Face | Vertices | Turn about the axis | On the hinge axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right inboard | `3969` | 4 | 4 | 0 | 0 |
| Right inboard | `394a` | 4 | 4 | 0 | 0 |
| Right outboard | `2176` | 3 | 3 | 0 | 0 |
| Right outboard | `2391` | 3 | 2 | 1 (33,-29,-3) | 0 |
| Right outboard | `2190` | 3 | 2 | 1 (33,-29,-3) | 0 |
| Right outboard | `2233` | 3 | 2 | 1 (33,-29,-3) | 0 |
| Left inboard | `3a31`, `3a50` | 4 each | 4 each | 0 | 0 |
| Left outboard | `1e87` | 3 | 3 | 0 | 0 |
| Left outboard | `23c3`, `1ea1`, `1f1b` | 3 each | 2 each | 1 (-33,-29,-3) | 0 |
| Rudder quad | `3bc6`, `3be5` | 4 each | 2 each | 2 each: (0,-35,4), (0,-40,19) | 0 |
| Rudder tip triangle | `2b64`, `2b7c` | 3 each | 2 each | 1 each: (0,-40,19) | 0 |

No face has a fixed vertex off its hinge axis and no vertex was chosen by a
coordinate threshold: each side's six faces and the rudder's four faces are
listed by address, and the rig refuses the shape if any of them is missing,
duplicated, or holds a vertex off the reviewed wedge or fin points. Verdict:
**no face folds**; the probe reports the largest inter-vertex distance change in
each group as about 1e-6 ft.

**Both strips take one angle.** The inboard and outboard strip of a side share
the seam vertices (17,-29,-2), (17,-29,-4) and (17,-32,-3) (left: x -18), so
both turn by the same angle about the same axis; a different angle would show a
seam. The probe's per-side skin gap between the strips is 0 on every sample.

**Flap words `0x58a8` and `0x58ae` (agent decision, 2026-11-09).** Each word
deletes one inboard strip (`0x58a8` the left `3a31`/`3a50`, `0x58ae` the right
`394a`/`3969`) and adds nothing, so a rig that set them would make the inboard
elevon vanish in flight. The rig pins both words at load (a mismatch refuses the
shape) but never sets them: the inboard strips stay drawn at every flap
setting. The flap lever instead **droops** both elevons (below), as the Rafale
and MiG-31 flaps lower their trailing surfaces.

**Rudder word `0x58b4`** swaps the quad for a copy offset 1 to 2 units in +X (a
fixed, positive-only pose, roughly 0.55 to 0.6 rad before its rounding to whole
units). The rig pins it at load and never sets it; the quad turns about its hinge
edge instead, in both directions.

**The rudder's shared trailing vertex (agent decision).** The quad's top
trailing vertex (0,-42,19) is shared with the two small triangles `2b64` and
`2b7c`, whose third vertex (0,-42,21) is in turn shared with the fixed fin cap
`2b16`/`2b31` (the cap's aft edge runs from (0,-40,19) to (0,-42,21)). The source's
own rudder word leaves the triangles fixed and cracks that corner. Neither
alternative is exact: leaving the triangles fixed would pin the quad's trailing
vertex off the hinge axis (a fixed vertex off the axis folds or shears the
face); turning them with the quad keeps all four faces one rigid panel and moves
only the triangles' apex (0,-42,21) away from the cap corner. The rig does the
latter. The apex is 1.26 units from the hinge axis, so at the 0.35 rad travel
the corner opens by about 0.18 ft (measured, `seam-opening.csv`), on the top
edge of the rudder only, where a real rudder also opens a wedge to the fin cap.

**Elevon seam opening (measured).** The hinge-edge vertices are 1 unit (1.5
scaled units vertically) off the mid axis and are shared with the fixed wing
skin, so a turn opens a small wedge: 0.15 ft at 0.30 rad (full pitch), 0.20 ft
at 0.40 rad (full flap droop), 0.25 ft at 0.50 rad (full pitch plus full roll).
This is the same wedge issue as every thin trailing-edge strip and is reported,
not a failure; the probe's skin gap (0) measures the seam between the two strips
of one side, and the strip pair midpoints on the mid axis stay put (anchor gap 0).

**Fitted magnitudes (agent decisions, 2026-11-09, matching the fleet).** Angle
positive lowers the trailing edge. Per side: `-0.30*elevator` (pull raises both
trailing edges), `-/+0.20*aileron` (a positive aileron raises the right
trailing edge and lowers the left, the sense of every fleet aileron); the sum of
pitch and roll is clamped to 0.50 rad (rule 12: the sum of the two maxima), then
`+0.40*flaps` droops both. Rudder `0.35*rudder`. The Rafale's elevons use the
same four numbers (`rafale_animation.rs`: `flaps*0.40 - elevator*0.30 +/-
aileron*0.20`, rudder 0.35), as does the MiG-31's rudder and flap; nothing here is
a source value. The shape has no flap surface of its own (the flap words only
delete the inboard strips), so the droop is a TORE choice made so the flap lever
shows on the elevons.

The gear keeps the shared interim rise until M2K-4: the six `0x589c` plates
(`380b`, `382a`, `3876`, `3895`, `38e1`, `3900`) lift by their own scaled
vertical extent as the gear stows and are hidden at zero. The plume (`0x5890`)
and the airbrake (`0x5896`) stay unmapped (M2K-4). See the
[Mirage 2000 fit](../spec/variety-animation.md#mirage-2000-control-fit).

## Mirage 2000 gear, airbrake and plume (M2K-4, 2026-11-09)

Slice M2K-4, Claude Sonnet (sonnet-high), implementation mode. This maps the
three device words of `M20.SH` that [M2K-3](#mirage-2000-control-surfaces-m2k-3-2026-11-09)
left unmapped. Source coordinates are shape units (x right, y forward, z up, a
third of a foot before the per-axis scale of
[M2K-2](objects-and-shapes.md#aircraft-shape-scale-2026-11-09)); every hinge
point is scaled with `scale_point` so a turned piece stays rigid. Motion and
angles are **fitted** (agent decisions, 2026-11-09); the faces and the word
roles are spec-derived from the retail shape.

**Word roles.** `0x5890` is the afterburner plume (eight faces), `0x5896` the
airbrake (two faces), `0x589c` the gear (six faces, the parked gear word of
`parked_aircraft.rs`, unchanged). The three trailing words are as M2K-3: the
flap words `0x58a8`, `0x58ae` and the rudder word `0x58b4` are pinned and never
set. The shape has **no slat, canopy, nozzle, hook or gear-door word** and none
is invented. Each of the three device words adds its faces and removes none
(checked against the branch at load; a mismatch refuses the shape).

| Piece | Faces | Source geometry |
| --- | --- | --- |
| Right main plate | `3876`, `3895` (two windings) | Flat plate in the plane x = 7, y -18 to -12, z -12 to -3 |
| Left main plate | `380b`, `382a` | The mirror, x = -7 |
| Nose plate | `38e1`, `3900` | Flat plate in the plane x = 0, y 32 to 37, z -12 to -5 |
| Airbrake | `3c73`, `3c92` | One quad, x -1 to 1, from (y 24, z 5) up to (y 17, z 10) |
| Plume, inner pair | `3d74`, `3d93` (plane x = 0), `3db2`, `3dd1` (plane z = 0) | y -57 to -44 |
| Plume, outer pair | `3cf8`, `3d17` (plane x = 0), `3d36`, `3d55` (plane z = 0) | y -80 to -57 |

**Gear motion (fitted).** Each plate is one whole rigid piece, no vertex held
back. The three assemblies are the right main, the left main and the nose; the
source has no door faces, so the wing root and belly skin close the bay and no
door is animated (a recorded mapping gap, as the MiG-31).

- Each main turns about its own top edge, the line along Y at z = -3 on its own
  x, by `90 degrees * (1 - gear)`, swinging **inward**: the foot comes up and
  across under the wing root and at near-stow the plate lies flat at z = -3,
  1 unit below the wing's upper skin (z -2) and 1 above its lower skin (z -4),
  hidden inside the wing and fuselage. The two stowed plates overlap in x
  (right x 7 to -2, left x -7 to 2) at the same height, inside the body where
  neither is visible. Hidden (not drawn) at gear 0, as the shared devices.
- The nose plate turns about a lateral axis (along X) through its forward top
  corner (0,37,-5) by `90 degrees * (1 - gear)`, swinging **rearward**: the foot
  (0,37,-12) swings aft and up and the plate stows in the nose-bay volume at
  y 30 to 37, z -5 to 0, inside the fuselage. It also steers about a vertical
  axis through its centre (0,34.5): `-nosewheel_angle()`, as the MiG-31 nose.
- Gear down (gear 1) is the source pose exactly, so the parked path
  `("M2000.PT", Some(0x589c))`, the ground contact and the stand-on-the-shape
  grounding are unchanged: the lowest gear vertex is the scaled 12 unit low
  point (12 units at a third of a foot times the z factor 1.5053, 6.021 ft) the
  flight model stands the aircraft on, which the probe measures.

Provenance of the direction: the real Mirage 2000's single-wheel main legs
retract **inward** and the twin-wheel nose leg **rearward** (Wikipedia, *Dassault
Mirage 2000*, landing gear paragraph; aatlse.org Mirage 2000 descriptions,
accessed 2026-11-09). Public text only; the hinge lines, the 90 degree stow and
the law are fitted. The source plates are one flat card per wheel, so the real
forward-and-inward rotation of the main strut is not reproduced.

**Airbrake (fitted).** The source is **one** double-sided centreline plate 2
units wide, hinged on its **forward** edge (y 24, z 5, on the spine) and raised
aft to (y 17, z 10). (The M2K-0 review text called y 24 the aft edge; y is
forward, so the hinge is the forward edge. The vertices and the face table are
unchanged.) The whole plate turns about that edge along X: brake 1 is the
source raised pose, brake 0 hides it, and in between it turns by
`closed_angle * (1 - brake)`. The closed angle (0.766 rad, 43.9 degrees,
computed on the scaled shape at run time) lays the plate on the spine line from
(0,25,5) to (0,-1,4), which the probe measures at 3e-6 ft. The real Mirage 2000
has small airbrakes above and below each wing root; the source has one dorsal
plate, so the rig turns what the source has and invents no surface.

**Plume (fitted).** The plume plates scale along Y from the nozzle exit plane
(y -44, the one nozzle disc `3794`) by the afterburner fraction, so the exit
corners stay put and the plume grows to the source length (y -80) at 1. Every
plate scales by the same function of y, so the seam at y -57 and the crossed
planes stay joined. Hidden at 0.

**Nozzle glow and contrail (engine count 1).** The nozzle disc is the single
hexagon `3794` (subtype `0x64`, six vertices at y -44, x -2 to 2, z -4 to 2).
`engine_material::nozzle` names it and `outlet_count` is 1 for the Mirage (the
M2K-1 override of the record's 2 engines), so there is one nozzle glow and
`contrail_offsets` makes **one** trail at the disc centre: x 0, 0.50 ft below
the shape origin, 20.03 ft behind it (the exit plane at y -44 scaled to 18.03
ft, 2 ft behind). The plume reaches 32.8 ft behind the origin, so the trail
starts inside the burner. A unit test pins the offset on a synthetic hexagon.

**Rule 11 fold table (rig definitions).** Every face turns whole; the vertices
that stay are on the hinge axis. No vertex was chosen by a coordinate threshold:
every face is named by address and the rig refuses a shape whose corners differ
from the reviewed values.

| Surface | Face | Vertices | Turn | On the hinge axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right main | `3876`, `3895` | 4 each | 2 each | 2 each: (7,-12,-3), (7,-18,-3) | 0 |
| Left main | `380b`, `382a` | 4 each | 2 each | 2 each: (-7,-12,-3), (-7,-18,-3) | 0 |
| Nose | `38e1`, `3900` | 4 each | 3 each | 1 each: (0,37,-5) | 0 |
| Airbrake | `3c73`, `3c92` | 4 each | 2 each | 2 each: (-1,24,5), (1,24,5) | 0 |
| Plume (eight faces) | `3cf8` ... `3dd1` | 4 each | scaled from y -44 | the exit-plane vertices | 0 |

Verdict: **no face folds** and no face keeps a fixed vertex off its axis. The
probe reports the largest inter-vertex distance change as 1e-6 ft for each gear
assembly and the airbrake, and the skin gap between the two windings of every
plate and between plume plates as 0.

The probe adds a 202-value dense gear sweep (`dense-gear.csv`, 203 lines, every
row passing: rigid, hinges fixed, feet inward or aft, nothing below the hinge
height near stow, ground contact at 1), a seven-value airbrake sweep
(`brake-sweep.csv`) and the gear, brake and exhaust witnesses. See the
[Mirage 2000 gear, airbrake and plume fit](../spec/variety-animation.md#mirage-2000-gear-airbrake-and-plume-fit).

## F-15E gear, speed brake and plume (F15E-4, 2026-11-10)

Implementation, slice F15E-4, Claude Sonnet (sonnet-high). `AircraftId::F15E`
flies `F15E.SH` through its own rig, `f15e_animation.rs` (no code is shared
with the F-15C's `f15_animation.rs`, whose shape is a different model, and the
identity is never aliased). The rig owns the gear, the dorsal speed brake and
the two plumes. The stabilators, ailerons, flaps and fins are F15E-3's: the
rig's `animate` has one arm per device and the control surfaces join it as
further arms, so nothing here is restructured for them. The shape is read at
`aircraft_scale` [1, 1, 1] (size is John's, still open); every hinge point is
named in shape units and scaled with `scale_point`, so a later scale entry keeps
every part rigid.

**Word map.** Five words, all accounted for.

| Word | Reading | Rig |
| --- | --- | --- |
| `0x80c0` | Two afterburner flames, 8 plates | Mapped: plumes scale from the nozzle exit plane |
| `0x80c6` | Dorsal speed brake, 4 faces | Mapped: one piece about its forward hinge |
| `0x80cc` | Gear down, 18 faces | Mapped: whole rigid pieces (below) |
| `0x80d8`, `0x80de` | Left and right flap words | Pinned at load, never set (below) |

**Flap words (new finding).** They are signed, and the F15E-0 review only read
value 1. Value **1** deletes the inboard flap wedge (left `5557`, `5576`; right
`5470`, `548f`) and adds nothing, so a rig that set it would make a surface
vanish. Value **-1** swaps the wedge for a lowered copy (left `55c0`, `55df`
plus the end triangle `55fe`; right `54d9`, `54f8`, `5517`), whose trailing edge
is five units lower (z -2 against 3), which is the shape's own flap-down branch
(the F-15C's flap words work the same way). The rig refuses a shape whose four
branches differ from these address sets, never sets either word, and keeps the
neutral wedge drawn in every pose, as the Mirage 2000 did. The droop itself
(from the -1 branch or a whole-panel turn) is F15E-3's decision.

**Gear faces (18, 3 pieces per leg).** Per main leg: an inboard door plate
(plane x +-7, y -6 to 9, z -18 to -12, untextured), a wheel sheet (plane x +-13,
y -4 to 6, z -27 to -11) and a cross plate (plane y 1, x +-9 to +-15, z -27 to
-11), each in both windings (`50e3`/`50fa`, `5156`/`5175`, `5194`/`51b3` left;
`524f`/`5266`, `52c2`/`52e1`, `5300`/`531f` right). Nose: a wheel sheet (x 0,
y 48 to 62, z -27 to -8), an untextured strut plate (x +-2, y 46 to 50, z -21 to
-8) and a cross plate (x -2 to 1, y 51 to 53, z -27 to -8), `539b` to `5426`.
The rig checks every corner of every plate against these values at load.

**Retraction direction (cited).** The real F-15's main gear legs "retract into
the fuselage, the legs turning 90 degrees as they retract forward to lie flat
in wells underneath the fuselage", and its "nosewheel retracts forward"
([aircraftinformation.info, McDonnell F-15 Eagle, airframe and flight control
systems](https://www.aircraftinformation.info/JB_AIF/usaf_fighters/f15_2.html),
accessed 2026-11-10). The F-15C rig's fit swings its nose aft about the forward
top corner (a fit read from the painted brace, no source law); the F-15E follows
the cited direction instead, an agent decision, so the two disagree. John may
choose either.

**Gear law (fitted, agent decision 2026-11-10; hinge lines and angles are not
recovered).** `quarter = 90 degrees * (1 - gear)`.

- Each main leg (wheel sheet and cross plate, 4 faces) is **one rigid body**: it
  turns `quarter` about a vertical line through the wheel centre (x +-12, y 1),
  which lays the wheel flat, then `quarter` about a lateral line (along X at y
  -4, z -9) which folds it forward. The fold line lies behind every leg vertex,
  so the lowest point only rises while closing and never dips below the gear-down
  height. Stowed, the leg lies flat at x 7 to 17 (mirrored), y -2 to 14, z -7 to
  -1: in the bay between the keel (x 6) and the shelf wall (x 21), above the
  belly skin (z -12 to -13) and below the wing skin.
- Each inboard door turns `quarter` about its own top edge (the line along Y at
  x +-7, z -12) and closes outward, ending flat at z -12 over x 7 to 13 (against
  the belly skin line, 0.5 to 0.9 units above it).
- The six nose plates are **one rigid body**: they turn `quarter` forward about
  a lateral line at y 46, z -8 (behind and above every nose vertex) and steer
  about a vertical line through the strut (x 0, y 52) by `-nosewheel_angle`.
  Stowed, they stand at y 46 to 65, z -8 to 8, inside the nose between the belly
  (z -9) and the fuselage top.
- Gear 1 is the source pose with the lowest point at z -27 (9 ft below the
  origin, the flight model's own gear low point); 0 hides the pieces.

**Speed brake.** The four faces (a main plate rising aft from z 16 at y 20 to z
30 at y -1, and a front closing plate falling to z 11 at y 25, both windings) are
one piece. It turns about the lateral line y 25, z 11, where the front plate
meets the dorsal skin, by `0.774 rad * (1 - brake)`: brake 1 is the source raised
pose, 0 hides it, and the closed angle (44.3 degrees) is solved on the loaded
shape so the free edge's mid point lies on the dorsal ridge (the line through the
source spine vertices (0, 20, 10) and (0, -10, 6), faces `24b3` and `24c5`, which
the rig checks at load); it comes to rest at y -6.9, z 6.4. Fitted. The real
speed brake is one large dorsal plate ([same source](https://www.aircraftinformation.info/JB_AIF/usaf_fighters/f15_2.html)),
which the shape matches.

**Burners and trails.** Each plume is two crossed plates (a vertical plate at x
+-8, z -6 to 5, and a flat plate at z -1, x +-4 to +-13), both windings, from y
-62 back to y -115 (38.3 ft behind the origin). They scale along Y from the
nozzle exit plane (y -61) by the afterburner fraction. `engine_material` names the
four nozzle faces at y -61 (`3d2e` and its cap `3d94` on the right, `3d55` and
`3d79` on the left) and `outlet_count` is 2 (the record's engine count, which
agrees with the shape), so there are two nozzle glows and `contrail_offsets`
makes two trails: x +-2.83 ft, 0.50 ft below the origin, 22.33 ft behind it, each
inside its plume. A unit test (`aircraft::tests`) pins the two offsets and the
nozzle bounds on synthetic faces.

**Rule 11 fold table.**

| Piece | Faces | Vertices | Moved | On a hinge | Fixed |
| --- | --- | --- | ---: | ---: | ---: |
| Right door | `524f`, `5266` | 4 each | 2 each | 2 each: (7,9,-12), (7,-6,-12) | 0 |
| Left door | `50e3`, `50fa` | 4 each | 2 each | 2 each: (-7,9,-12), (-7,-6,-12) | 0 |
| Right leg (one body) | `52c2`, `52e1`, `5300`, `531f` | 4 each | all | 0 (both lines are free) | 0 |
| Left leg (one body) | `5156`, `5175`, `5194`, `51b3` | 4 each | all | 0 | 0 |
| Nose (one body) | `539b`, `53ba`, `53d9`, `53f0`, `5407`, `5426` | 4 each | all | 0 | 0 |
| Speed brake (one piece) | `568c`, `56a3`, `56ba`, `56d1` | 4 each | 2 each in the front plate, all in the main plate | 2 in each front-plate face: (-3,25,11), (3,25,11) | 0 |
| Plume (eight faces) | `574f` to `5860` | 4 each | all (scaled from y -61) | 0 | 0 |
| Flap wedges | `5557`, `5576`, `5470`, `548f` | 4 each | none | none (F15E-3) | all |

Verdict: **no face folds.** The legs and the nose have no vertex on their fold
lines (the lines run through free space behind and above them), so each is a
rigid body that meets no fixed face; the doors and the brake keep their hinge
edges. The load-time check refuses a shape whose corners differ.

**Probe (`--animation-probe`, `reviewed-controls-and-attachments`).** 350
neutral faces, 0 required or check failures. Gear, brake and exhaust each read
`required moved`; the elevator, aileron, rudder and flap controls read
`unreviewed` (F15E-3 restores `Required` for them in
`aircraft_animation_probe.rs`). The 202-value dense gear sweep
(`dense-gear.csv`, 203 lines) passes every row: rigidity at most 2.2e-6 ft,
anchor gap 0, skin gap 0, no new planar crossings, ground contact at gear 1
(lowest point 27 units, 9 ft), legs and nose forward, doors outward, nothing
below the gear-down height, and every piece inside its bay near stow. The
seven-value brake sweep (`brake-sweep.csv`) is rigid to 1.3e-6 ft with the
hinge gap 0 and the closed free edge 8e-6 ft off the ridge.

Frames are in `.local/m3-aircraft/frames/F15E-4/` (local only): gear down, half,
near stow and up from the side and below with 3x crops of the mains and nose, the
brake open, half, nearly closed and off, the afterburner on and off from behind,
and the aircraft parked on a runway.

Provenance: faces, word roles, flap branches and corners spec-derived (retail
shape, re-verified at load); retraction directions from the cited public text;
every hinge line, the twist and fold angles, the stow envelopes, the 0.774 rad
closed brake angle and the plume scaling fitted (agent decisions 2026-11-10);
engine count 2 from the record; nothing here is attributed to John.

## F-15E control surfaces (F15E-3, 2026-11-10)

Implementation, slice F15E-3, Claude Sonnet (sonnet-high). `f15e_animation.rs`
now also owns the two all-moving stabilators, the two ailerons and the two
inboard flaps, as further arms of `Rig::animate` and load-time checks in
`Rig::load`. Rule 11 fold table (corners from the `state_neutral` dump; a
script also confirmed that no other face lies wholly inside any moving panel):

| Surface | Faces | Vertices | Moved | On the turn line | Fixed |
| --- | --- | ---: | ---: | ---: | ---: |
| Right stabilator | `4b5f`, `4b86`, `4ba7`, `4c5d`, `4c8a` | 4, 3, 4, 5, 4 | all | 0 (the spindle runs through the root chord midpoint, not a vertex) | 0 |
| Left stabilator | `42f1`, `4318`, `4339`, `43ef`, `441c` | 4, 3, 4, 5, 4 | all | 0 | 0 |
| Right aileron | `4b33`, `4ac4` | 4 each | all | 1 (the tip vertex 59, -27, 3) | 0 |
| Left aileron | `42c5`, `41d9` | 4 each | all | 1 | 0 |
| Right flap wedge | `5470`, `548f` (+ end triangle `5517`) | 4 each (3) | all | 0 (the line runs through the front edge's mid height) | 0 |
| Left flap wedge | `5557`, `5576` (+ end triangle `55fe`) | 4 each (3) | all | 0 | 0 |
| Fins | `4ae3`, `4b0a`; `4242`, `4269` | 4 each | none | n/a | all |

**No face folds**: every moving face turns with its whole panel. Each stabilator
is flat at z -1 (both windings, zero anhedral, the real jet has about 15
degrees), root chord y -41 to -70, tip x 47 and -47. Only its aft root corner
(21, -70, -1) and (-22, -70, -1) is shared with fixed faces (eight each); the
forward root corner is shared with nothing fixed.

**Stabilators (fitted, labelled as the F-15C's, TR-6).** Each panel turns about
a lateral spindle (axis +X, no tilt) through its root chord midpoint, (21, -55.5,
-1) right and (-22, -55.5, -1) left, so the tip moves. Angle about +X, radians:
`(-0.30 * elevator - side * 0.10 * aileron).clamp(-0.40, 0.40)`, `side` +1 right,
-1 left. A pull raises both trailing edges; a positive aileron raises the right
and lowers the left (the aileron sense). Real F-15 stabilators take roll as
well as pitch (the taileron standard, TR-6 entry above, and the F15E-0 review's taileron note); John's decision of
2026-11-10 made them whole panels with roll for the F-15E. Magnitudes are the
fleet's, not recovered. The aft root corner opens from the fuselage by
`2 * 14.5 * sin(angle / 2)` units: 1.44 ft at the 0.30 rad pitch limit, 1.92 ft
at the 0.40 rad pitch-plus-roll corner, the same as the F-15C's tail.

**Ailerons (fitted).** Each aileron is a two-face wedge (top `4b33`/`42c5`,
bottom `4ac4`/`41d9`) turned as one panel about its slanted front edge. The
front (hinge) edge runs from (+-39, -24) to (+-59, -27); its root pair is
three units thick (z 1 and 4) and its tip is one vertex (z 3), so the vertices
are not collinear. Axis choice: the line from the root pair's mid point
(+-39, -24, 2.5) to the tip vertex (+-59, -27, 3), turning outward. It keeps the
tip vertex and the root mid point exactly fixed and moves each root vertex
(1.5 units off the axis) by `2 * 1.5 * sin(angle / 2)`, which is the least any
rigid turn that keeps the tip vertex can leave at the root (an axis through
either root vertex leaves 3 units on the other). Travel `-aileron * 0.20` rad
about the outward axis on both sides (the F-15C's 0.20, fitted): a positive
aileron raises the right trailing edge and lowers the left. **Measured root
gap: 0.0998 ft (0.30 units) at full travel** against the two fixed wing faces
that share the hinge edge. The outboard trailing corner (59, -34, 3) is shared
with two fixed wingtip faces (`49db`, `49f4`) and opens 0.46 ft there (7 units
aft of the axis), and the inboard trailing corner (39, -30, 3) meets the flap's
corner; neither is a skin hole in the neutral pose, both are the usual notch of
a rigid surface and are visible in the frames. No partial or diagonal surface.

**Flaps (fitted).** The two flap words are never set. The inboard wedge stays
drawn and turns whole, trailing edge down, about the lateral line through its
front edge's mid height (y -24, z 2.5), `0.80 * flaps` rad (46 degrees at full
travel). The `-1` branch is the source's lowered flap, but a rigid wedge cannot
equal it (its chord shrinks from 6 to 5 units while the trailing edge drops
five); 0.80 rad is the turn that brings the wedge's trailing edge nearest the
branch's, 0.74 units (0.25 ft) off, and the rig refuses a shape where that
error exceeds one unit. The branch's end triangle (`55fe` left, `5517` right,
outboard end only) is drawn while the flap is down, re-seated on the neutral
wedge's trailing corner so it stays rigid with the flap, and hidden at flaps 0
(the neutral wedge has no end triangle). Hinge-edge opening against the wing: 0.39
ft at full flap (1.5 units off the line); the trailing corners, which touch
fuselage and aileron faces, open 1.56 ft.

**Rudders: none (John, 2026-11-10).** Each fin is one flat plate with no rudder
face or word. The rig turns nothing for a rudder command, the probe's rudder
control is `not-applicable` for the F-15E, and the probe requires every surface
to stay on its neutral faces under it.

Provenance: faces, corners, hinge edges and the `-1` branch spec-derived (retail
shape, re-verified at load); the stabilator law is the F-15C's fitted law
(agent decision on John's 2026-11-08 and 2026-11-10 requests), the aileron and
flap travels, the axes and the closure re-seating fitted (agent decisions
2026-11-10); nothing is attributed to John beyond the three decisions he made
(whole-panel tailerons with roll, no rudder motion, size 1:1).

**Update (F15E-5, 2026-11-10): flaps now deflect 30 degrees.** John decided on
2026-11-10 (round 2, opinionated) that the F-15E's flaps deflect the real
aircraft's roughly 30 degrees, not the 0.80 rad (46 degrees) fitted above. The
NASA Dryden flight test report "Dynamic Ground Effects Flight Test of an F-15
Aircraft" ([NTRS 19950005778](https://ntrs.nasa.gov/citations/19950005778), read
2026-11-10) gives the F-15's trailing-edge flaps 30 degrees of downward travel.
`FLAP_DOWN` in `crates/tore-app/src/f15e_animation.rs` is now pi/6 (0.5236 rad);
the hinge line, the whole-panel turn and the end triangle re-seated on the wedge's
trailing corner are unchanged. At 30 degrees the trailing corner lands 1.98 units
(0.66 ft) from the source's lowered copy instead of 0.74 units, so the load check
that refused a shape more than one unit from the copy now accepts up to three
units (a different shape is still refused). Measured openings at full flaps drop
with the angle: hinge edge against the wing 0.26 ft (was 0.39 ft), trailing
corners 1.04 ft (was 1.56 ft). The probe (`--animation-probe`, `profile-f15e`)
still reports `350 neutral faces, 0 required/check failures`; the 21-value flap
sweep is rigid (at most 3e-7 ft) with skin gap 0. Frames:
`.local/m3-aircraft/frames/F15E-5/final-crop3x-flaps0-vs-flaps30-*.png`. The
text above is the F15E-3 slice's and is kept as written.

## Gripen gear, airbrakes and plume (GRI-4, 2026-11-10)

Slice GRI-4 (batch 5), Claude Sonnet (sonnet-high), implementation mode. This is
the first part of the `GRI.SH` rig, in its own module
(`crates/tore-app/src/gripen_animation.rs`). It runs before GRI-3, which adds the
canards, elevons and rudder to the same module (the `animate` method has the
marked place for their arms, and the probe module `gripen.rs` has the marked
place for their witnesses). Source coordinates are shape units (x right, y
forward, z up, a third of a foot; the Gripen has the identity scale, so these
are also the drawn units). Motion and angles are **fitted** (agent decisions,
2026-11-10); the faces and the word roles are spec-derived from the retail
shape, re-checked at load: the rig refuses a shape whose layout, branches or
plate corners differ from the review of
[GRI-0](aircraft-variety.md#gripen-source-review-2026-11-10).

**Every word accounted for.** `0x8080` the plume (4 faces), `0x8086` the two
airbrake plates (4 faces), `0x808c` the gear (22 faces). `0x8098` and `0x809e`
are the left and right flap words (each deletes that wing's six elevon strip
faces and adds none) and `0x80a4` the rudder word (swaps the rudder quad for a
turned copy). The three trailing words are pinned at load (each branch must add
and remove exactly the reviewed faces) and **never set**: the elevon strips and
the neutral rudder stay drawn so the elevons never vanish in flight. Turning
them is GRI-3's. The rudder naturally belongs with the other control surfaces (it
shares the fin and the neutral/word swap with the flap words), so it is left to
GRI-3. The slot at `0x8092` is skipped in the source. The shape has no hook
(hook flag clear), slat, canopy, nozzle or canard word, and none is invented.

| Piece | Faces | Source geometry |
| --- | --- | --- |
| Right main leg | `5bca`, `5be9` (slanted strut plate), `5c08`, `5c2f` (wheel plate) | Strut x 4 to 6, y -14 to -8, z -5 to -16; wheel plate in the plane y -11, x 2 to 7, z -5 to -16 |
| Left main leg | `59ec`, `5a0b`, `5a2a`, `5a51` | Strut x -6 to -4; wheel plate x -6 to -1 (one unit nearer the centreline than the right mirrored: a source asymmetry, the rig mirrors the law, not the plate) |
| Right door pair | wall `5b29`, `5b40`; root plate `5b57`, `5b6e` | Root plate x 9 to 13, z -3 to -4, y -14 to 1; wall x 13 to 15, z -4 to -9, sharing the edge x 13, z -4 with the root plate |
| Left door pair | wall `594b`, `5962`; root plate `5979`, `5990` | The mirror |
| Nose group | `5c9b`, `5cba` (side plate, plane x 0, y 38 to 44, z -15 to -5), `5cd9`, `5cf8` (cross plate, x +-1, y 40 to 41), `5d44`, `5d5b` (wheel plate, x +-2, y 42 to 43, z -11 to -5) | |
| Airbrakes | right `62ad`, `62c4`; left `634a`, `6361` | One quad per side, forward edge (+-9, -38, z -4 to 1), free edge (+-13, -43, z -3 to 0) |
| Plume | `63af`, `63d6` (plane x 0), `63fd`, `6424` (plane z -2) | y -85 to -61, two crossed double-sided plates |
| Nozzle exit disc | `49a6` (octagon, z -3 to 1), `49d5` (hexagon, z -4 to -3) | Both at y -61, subtype `0x64`, x -2 to 3 |

**Gear motion (fitted).** Whole pieces, none cut. Gear 1 is the source pose
exactly (the lowest gear vertex is the shape's 16 unit low point, 5.333 ft, which
the probe measures at gear 1); gear 0 hides every gear face.

- *Main leg* (strut plus wheel plate, one rigid piece): turns about a horizontal
  hinge line through (4, -16, 0) whose direction is 35 degrees from the X axis
  toward Y, by `71.5 degrees * (1 - gear)`, so the leg folds **forward and
  inboard**. The 71.5 degrees is worked out in code (`leg_stow`) as the turn that
  lays the foot centre (5, -11, -10.5) level with the hinge. The hinge point sits
  above the leg and behind its rear edge on purpose: with it no corner ever
  dips below the gear-down low point while the leg folds (the probe's lowest
  point never falls below -16 units), and the stowed leg lies in x -4.6 to 5.2,
  y -11.0 to 1.4, z -4.6 to 4.6, inside the fuselage and above its belly (z -6).
  The left leg is the exact mirror of the right law.
- *Main door pair* (two rigid plates): the root plate turns a half turn about the
  line along Y on its inboard edge (x 9, z -3), up and inboard, while the outer
  wall folds back onto the root plate about their shared edge (x 13, z -4) by
  125.8 degrees (`door_fold`, the turn from the wall's direction to the way back
  along the root plate), so the closed pair lies flat in x 5.0 to 10.2, z -3.3 to
  -2.0, inside the fuselage side apart from the wall's 1.3 unit overshoot past
  the hinge (the wall is 5.4 units long, the root plate 4.1). The shared edge
  stays together (probe: seam 0). The wall passes through the wing root as the
  pair swings up: a one to two second transient at the end of the retraction,
  recorded as a known cosmetic (the source has no wing-root skin in that
  region to clip against, the rig keeps rigid plates).
- *Nose group* (six faces, one rigid piece): turns about a lateral axis (along X)
  through its forward top corner (0, 44, -5) by `100 degrees * (1 - gear)`,
  swinging **rearward**, a little past level so the stowed plates lie above the
  belly (stowed y 34.2 to 45.0, z -5.0 to 2.6). It steers about a vertical axis
  through the strut (0, 41) by `-nosewheel_angle()`, as the MiG-31 nose.

Provenance of the direction (public text only): the real Gripen's single-wheel
main legs retract at a forward angle and its two-wheel nose leg retracts
rearward (airvectors.net, "The SAAB JAS 39 Gripen", read as a search-result
extract on 2026-11-10: the page itself returned 404 to a direct fetch, so this
is secondary and not independently confirmed), and the mains stow in the
fuselage (aircraftrecognitionguide.com, Saab 39 Gripen, accessed 2026-11-10: "Single
wheel main gears retract in the fuselage or wing roots"). Wikipedia (JAS 39
Gripen, accessed 2026-11-10) says only "retractable tricycle landing gear" and
does not give a direction. **The inboard part, every hinge line, the hinge point
placement, the stow angles and the door folding are fitted.** Real nose-gear
retraction direction should be treated as unconfirmed (the same search results
also named a twin-wheel nose leg, where the shape has one wheel plate).

**Airbrakes (fitted).** The two plates (real: "a pair of air brakes ... on the
sides of the rear fuselage", Wikipedia, accessed 2026-11-10; "below, and in front
of the exhaust, ... on both sides of the fuselage", aircraftrecognitionguide.com)
each turn whole about their **forward** vertical edge (x +-9, y -38, a line along
Z), the hinge GRI-0 recorded. Brake 1 is the source open pose, brake 0 hides the
plates, and between them each plate turns by `52.7 degrees * (1 - brake)`, the
turn (computed in `brake_closed`) that lays the plate parallel to the rear
fuselage side line from (9, -36) to (4, -56), half a unit outside it (the hinge
vertex is half a unit outside that line, so the closed plate is flush to within
that half unit). The probe's sweep measures the closed free edge on that
direction line to 2e-6 ft. Both plates take the same brake value; the real
canards also angle down as air brakes on landing, which is GRI-3's to decide
with John's canard answer.

**Plume (fitted).** The four plume plates scale along Y from the nozzle exit
plane (y -61) by the afterburner fraction: the exit corners stay, the plume
grows to the source length (y -85, 24 units, 8.0 ft) at 1, hidden at 0, every
plate by the same function of y so the crossed planes stay joined.

**Nozzle glow and contrail (engine count 1).** `engine_material::nozzle` names
the two exit-disc faces `49a6` and `49d5` (one nozzle in two pieces: the
octagon above z -3 and the hexagon below it) and `outlet_count` is 1 for the
Gripen, so there is **one** nozzle glow and `contrail_offsets` makes **one**
trail, centred on the discs' bounding box: x 0.17 ft right of the shape's
centreline, 0.50 ft below the origin, 22.33 ft behind it (the exit plane at
y -61 is 20.33 ft, plus 2 ft). The plume reaches 28.3 ft behind the origin, so
the trail starts inside the burner. The record's engine count is 1 and real
(GRI-0), so unlike the Mirage 2000 no override is involved. A unit test pins the
offsets and the bounds on synthetic discs; a windowed contrail frame was not
captured (trails form only above the onset altitude and the capture starts low).

**Rule 11 fold table.** Every piece turns whole, named by address. "On axis" are
vertices that stay put by lying on the hinge line; "turn" vertices move;
"fixed off axis" would be a fold.

| Piece | Face | Vertices | Turn | On the hinge axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right and left main leg | `5bca`, `5be9`, `5c08`, `5c2f` (and the left four) | 4 each | 4 each | 0 (hinge point (+-4, -16, 0) is not on a face) | 0 |
| Door wall | `5b29`, `5b40` (left `594b`, `5962`) | 4 each | 4 each (2 of them only by the root plate's turn, on the fold axis) | 2 each: (13, 1, -4), (13, -14, -4) for the fold | 0 |
| Door root plate | `5b57`, `5b6e` (left `5979`, `5990`) | 4 each | 2 each | 2 each: (9, -14, -3), (9, 1, -3) | 0 |
| Nose | `5c9b`, `5cba` | 4 each | 3 each | 1 each: (0, 44, -5) for the retraction | 0 |
| Nose | `5cd9`, `5cf8`, `5d44`, `5d5b` | 4 each | 4 each | 0 | 0 |
| Airbrake | `62ad`, `62c4`, `634a`, `6361` | 4 each | 2 each | 2 each: (+-9, -38, 1), (+-9, -38, -4) | 0 |
| Plume | `63af`, `63d6`, `63fd`, `6424` | 4 each | scaled from y -61 | the exit-plane vertices | 0 |

Verdict: **no face folds and none keeps a fixed vertex off its axis.** The
probe measures the largest inter-vertex distance change as 1.4e-6 ft over the
202 gear values and 6.4e-7 ft over the airbrake sweep, the skin gap inside
every piece as 0 and the door seam as 0.

The probe adds a 202-value dense gear sweep (`dense-gear.csv`, 203 lines, every
row passing: rigid, nothing below the gear-down low point, feet inboard and
forward, nose aft, ground contact at 1, stowed pieces inside the hull near 0,
no new planar crossing), a seven-value airbrake sweep (`brake-sweep.csv`) and
the gear, brake and exhaust witnesses. See the
[Gripen gear, airbrake and plume fit](../spec/variety-animation.md#gripen-gear-airbrake-and-plume-fit).

## Gripen control surfaces (GRI-3, 2026-11-10)

`GRI.SH` (28,848 CODE bytes, 383 runtime faces; review in [aircraft
variety](aircraft-variety.md#gripen-source-review-2026-11-10)) draws the Gripen at
its own size (identity axis scale), so the source coordinates below are plain
shape units (x right, y forward, z up, a third of a foot each). The shape has no
canard word, two flap words (`0x8098` left, `0x809e` right, each deleting that
wing's six elevon-strip faces) and a positive-only rudder word (`0x80a4`). The
motion is **fitted** whole-panel assignments over exact reviewed faces; the rig
(`gripen_animation.rs`) refuses a shape whose canard, elevon or rudder faces are
missing, duplicated or hold a vertex off the reviewed points.

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right canard, plate | `212d`, `2284` | One pentagon, both windings: (10,19,1), (22,15,2), (22,19,2), (8,37,1), (8,30,1) |
| Right canard, root filler | `22f3`, `2311` | Triangle (10,19,1), (9,19,1), (8,30,1), against the plate's root edge |
| Left canard, plate | `24ae`, `2553` | Mirror, with the forward root corner at (-7,37,1) (a unit off the mirror) |
| Left canard, root filler | `25c2`, `25e0` | Mirror triangle (-10,19,1), (-9,19,1), (-8,30,1) |
| Right elevon, inboard | `5fd2`, `5ff1` | Quad, x 12 to 26; hinge edge y -27 (z -2 flat face, z -1 sloped face), trailing edge from (12,-33,-2) to (26,-32,-2) |
| Right elevon, outboard | `6010`, `602b`, `6046`, `6061` | Four triangles, x 26 to 40; trailing corner (40,-31,-2) |
| Left elevon, inboard | `6194`, `61b3` | Mirror quad, x -12 to -25 |
| Left elevon, outboard | `61d2`, `61ed`, `6208`, `6223` | Mirror triangles, x -25 to -40 (the seam is at x -25 against the right's 26) |
| Rudder | `5d92`, `5db9` | One quad, both windings: (0,-43,6), (0,-50,6), (0,-49,23), (0,-43,23) |

**The filler belongs to the canard.** The root filler touches only the plate (its
edge (10,19,1) to (8,30,1)) and the fuselage side at one vertex; every fixed
face that shares a vertex with the canard group shares only (8,30,1) (right) or
(-8,30,1) (left). Left fixed it would stay as a stray sliver in the root notch as
the plate turned, so each canard is plate plus filler, four faces, one rigid
panel.

**Hinge axes.** Canards: a lateral spindle through each root chord midpoint,
right (9,28,1) (midpoint of (8,37,1) and (10,19,1)), left (-8.5,28,1) (midpoint of
(-7,37,1) and (-10,19,1)), direction (12,0,1) on the right (the plate's own
dihedral, z rising 1 over the 12 units from the root edge to the tip) and its
mirror on the left; never a swept span axis. The left canard is turned by
reflecting, turning about the mirrored right spindle and reflecting back, so the
two sides are exact mirrors. Elevons: the straight line y -27, z -1.5 along X
through the whole span of both strips (the hinge vertices sit at z -2 and z -1,
half a unit either side). Rudder: the vertical leading edge x 0, y -43.

**Rule 11 fold table (rig definitions).** Every face of a moving surface turns
whole; a vertex stays put only if it is on the hinge line.

| Surface | Face | Vertices | Turn | On the axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Canard plate, each side | 2 faces | 5 | 5 | 0 | 0 |
| Canard root filler, each side | 2 faces | 3 | 3 | 0 | 0 |
| Elevon inboard, each side | 2 faces | 4 | 4 | 0 | 0 |
| Elevon outboard, each side | 4 faces | 3 | 3 | 0 | 0 |
| Rudder quad | `5d92`, `5db9` | 4 | 2 | 2 each: (0,-43,6), (0,-43,23) | 0 |

Verdict: **no face folds.** Elevon hinge vertices are half a unit off the mid
axis, so they move rather than stay, but a hinge pair (z -2 and z -1 at the same
x) has its midpoint on the axis, and the probe holds those midpoints (anchor gap
0). The canards have no source hinge, so no vertex is pinned; the probe holds each
root chord midpoint. The probe's largest inter-vertex distance change is about
1e-6 ft in every group.

**Both strips take one angle.** The inboard and outboard strip of a side share
the seam vertices (26,-32,-2), (26,-27,-2) and (26,-27,-1) (left x -25), so both
turn by the same angle about the same line; the skin gap between the strips is 0
on every sample. The flap words (`0x8098`, `0x809e`) delete both strips of a
side together, so the rig pins them at load and never sets them: the strips stay
drawn and turn. The rudder word `0x80a4` (a fixed, positive-only copy swung about
31 to 34 degrees) is pinned and never set; the quad turns about its hinge in
both directions.

**Fitted magnitudes (agent decisions, 2026-11-10).** All in radians.

| Surface | Law |
| --- | --- |
| Elevons, angle positive lowers the trailing edge | `-0.30*elevator -/+ 0.20*aileron` (a positive aileron raises the right trailing edge and lowers the left, as every fleet aileron), the pitch and roll sum clamped to 0.50, then `+0.40*flaps` droops both strips of both sides. The Mirage 2000's four numbers. |
| Canards, angle positive raises the leading edge | `+0.25*elevator -/+ 0.15*aileron` (right `-`, left `+`), the sum clamped to 0.35. A nose-up command raises both leading edges; roll right lowers the right canard's leading edge and raises the left's, so the canard on the dropping wing loses lift. The 0.25 is the Su-35's canard pitch; the 0.15 roll and 0.35 bound are new. |
| Rudder, positive swings the trailing edge to +X | `0.35*rudder` about the vertical hinge, the fleet's travel. |

The canards take roll as well as pitch by John's decision of 2026-11-10 ("Canards
definitely roll", the GRI-0 option 3); the magnitudes and the bound are an agent
choice. The sum bound limits how far a canard root can open against the intake
side. No landing air-brake bias (leading edge down with the brakes) is applied;
that GRI-0 option was not chosen. Flaps do not move the canards.

**Measured seam openings** (`seam-opening.csv`, reported, not gated; feet at full
travel). Inboard elevon root slit, the strip's trailing corner (12,-33,-2) (left
(-12,-33,-2)) against the fixed centre-body strip `5716`/`57a8` (left `51d4`/`52dc`),
6.0 units from the hinge line so it opens `6.0 * angle` units: 0.40 ft at roll
(0.20 rad), 0.60 ft at full pitch (0.30), 0.80 ft at full flaps (0.40), 0.99 ft at
full pitch plus roll (0.50), 1.75 ft at the extreme corner, full flap droop with
pitch and roll bringing one side to 0.90 rad. This is the largest opening on the
aircraft: the Gripen's strips are 6 units (2 ft) deep at the root against the Mirage
2000's 3 units. Canard root corner (8,30,1) against the intake side: 0.10 ft at roll,
0.17 ft at full pitch (0.25 rad), 0.23 ft at the 0.35 bound. Rudder trailing corners
against the spine and fin cap: 0.81 ft at 0.35 rad (the one unit drop of the spine to
the quad's bottom corner shows as a notch in the rear frames).

**Rudder shared vertices.** The trailing corners (0,-50,6) and (0,-49,23) are shared
with fixed spine and fin-cap faces (`3bba`, `3bcd`, `4ba9`, `4bd7`, `4c36`, `4c9f`,
`4d1c`, `4d85`); the quad's top edge (y -43 to -49 at z 23) is the lower edge of
the cap strip. Turning the quad opens those corners as above. Neither fixed
alternative is exact (holding a trailing vertex pins a vertex off the hinge), so
the rig turns the whole quad and accepts the opening, as every reviewed fin does.

Frames and probe sheets: `.local/m3-aircraft/frames/GRI-3/`; probe CSVs:
`.local/m3-aircraft/checks/GRI-3/probe/`. See the [Gripen control
fit](../spec/variety-animation.md#gripen-control-fit).

## F-5E control surfaces (F5E-3, 2026-11-10)

`F5E.SH` (21,488 CODE bytes, 253 runtime faces, six state words `0x63c0`,
`0x63c6`, `0x63cc`, `0x63d8`, `0x63de`, `0x63e4`; review in [aircraft
variety](aircraft-variety.md#f-5e-control-surfaces-for-john)) names no signed
pitch, roll or flap-travel branch, and its stabilators and ailerons have no
state word at all, so every control role below is a **fitted** whole-panel
assignment over exact reviewed source faces. John's decisions of 2026-11-10
(stabilators pitch only, ailerons roll, size 1:1) fix the roles. Coordinates are
shape units (x right, y forward, z up, one third foot; the F-5E has the identity
scale, so shape units are the drawn units).

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right stabilator | `3a9d` (top), `3b8c` (back winding) | One flat plate at z -4: root x 5 (y -23 to -37), tip x 20 (y -30 to -35); shares no vertex with any other face |
| Left stabilator | `3f2c`, `40eb` | Mirror, x -5 to -20 |
| Right aileron | `3b45` (top layer), `3b9f` (bottom layer) | Strip x 31 to 43: hinge y -5 at z -3 (top) and z -4 (bottom), trailing edge y -8 (x 31) and y -7 (x 43) at z -4 |
| Left aileron | `4067`, `41ce` | Mirror, x -31 to -43 |
| Right flap | `44d9`, `44f8` | Strip x 7 to 31: hinge (7,-4) to (31,-5), trailing edge y -11 (x 7) to y -8 (x 31); word `0x63de` deletes both |
| Left flap | `45c0`, `45df` | Mirror; word `0x63d8` deletes both |
| Rudder | `4755`, `4774` (quad, both windings) | Hinge edge (0,-31,4) to (0,-29,22); trailing edge (0,-37,4) to (0,-32,22); word `0x63e4` swaps it for `469b`/`46ba` (about 40 degrees to starboard, rounded) |

**Selection.** The rig selects every panel by face address, never by a
coordinate threshold: the ailerons, which have no state word, are the four
faces above (each wholly on one side of the wing, x 31 to 43), and the load
refuses the shape if a face is missing, duplicated, or holds a vertex off the
reviewed corners. The flap words, the rudder word and the three device words
(gear `0x63cc` adds `49cc 49eb 4a37 4a56 4aa2 4ac1`, speed brake `0x63c6` adds
`4802 4819`, plume `0x63c0` adds the eight `48a7` to `4980` plates) are
checked against their exact added and removed face sets at load.

**Hinge axes.** Stabilators: a lateral line (along X) through the root chord
midpoint, (+/-5, -30, -4), the taileron standard. The plate is flat (every
vertex z -4), so its own anhedral is zero and the spindle is not tilted; it is
never a swept span axis. Ailerons: the straight line y -5, z -3.5 along X
through the strip (the mid axis of the hinge pair, as the Mirage's elevons).
Flaps: the line from (7,-4,-3.5) to (31,-5,-3.5), so the flap axis is 2.4
degrees off the aileron's and each side turns about its own axis. Rudder: the
fin trailing edge from (0,-31,4) toward (0,-29,22), direction (0,2,18); a
positive angle swings the trailing edge toward +X (right), the sense of the
fleet's rudders.

**Rule 11 fold table (rig definitions).** Every moving face turns whole about
its axis; no face has a fixed vertex off its hinge line.

| Surface | Face | Vertices | Turn about the axis | On the axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right stabilator | `3a9d`, `3b8c` | 4 each | 4 each (the root midpoint is not a vertex) | 0 | 0 |
| Left stabilator | `3f2c`, `40eb` | 4 each | 4 each | 0 | 0 |
| Right aileron | `3b45`, `3b9f` | 4 each | 4 each (hinge pair 0.5 unit off the mid axis) | 0 | 0 |
| Left aileron | `4067`, `41ce` | 4 each | 4 each | 0 | 0 |
| Right flap | `44d9`, `44f8` | 4 each | 4 each (hinge pair 0.5 unit off the mid axis) | 0 | 0 |
| Left flap | `45c0`, `45df` | 4 each | 4 each | 0 | 0 |
| Rudder quad | `4755`, `4774` | 4 each | 2 each | 2 each: (0,-31,4), (0,-29,22) | 0 |

Verdict: **no face folds**; the probe reports the largest inter-vertex distance
change in each group as about 6e-7 ft (floating point). The fin cap
(`3cf2`, `3da1`) touches the rudder's top trailing vertex (0,-32,22) only at
that corner and stays fixed (it is not part of the rudder quad); that corner
opens by about 0.35 ft at full rudder (`seam-opening.csv`, the same corner
opening the Mirage's rudder reports), at the top edge of the rudder only.

**Flap words (agent decision, 2026-11-10; Mirage precedent).** Each flap word
deletes one wing's strip and adds nothing, so a rig that set it would make the
flap vanish in flight. The rig pins both words at load but never sets them: the
strips stay drawn at every flap setting and the flap lever **droops** them. The
flap and the aileron strips share the seam vertices at x +/-31; they are
separate surfaces with separate jobs, so a step opens there when they take
different angles. From the hinge geometry the trailing corner (31,-8,-4) drops
about 0.38 ft at full flap with a neutral aileron, and the step against a
fully raised right aileron is about 0.58 ft. That seam, the rudder's top corner
and the flap root edge are reported, not gated.

**Flap root edge (measured).** The root edge (x 7) is shared with the 1-unit
closure strips `3161`, `317d` (left `23c0`, `38b2`), which stay fixed (they hold
the fuselage side). Their free corner (7,-11,-4) is 7 units from the hinge, so
at the full 0.40 rad droop it opens about 0.93 ft (measured). John may want a
smaller droop; it is one constant (`FLAP_DROOP`).

**Fitted magnitudes (agent decisions, 2026-11-10, matching the fleet).** Angle
positive lowers the trailing edge.

| Control | Law |
| --- | --- |
| Pitch | Both stabilators, `-0.30*elevator` radians (pull raises both trailing edges), **no roll term** (John, 2026-11-10: stabilators pitch only). The F-4 and F-104 stabilators use the same law (TR-10). |
| Roll | The ailerons only, `-0.20*aileron` on the right and `+0.20*aileron` on the left (a positive aileron raises the right trailing edge and lowers the left, as every fleet aileron). The ailerons take no pitch and no flap droop. |
| Flaps | `+0.40*flaps` lowers both flap strips (the Mirage and MiG-31 droop). |
| Yaw | The rudder quad `0.35*rudder` radians about its hinge edge. |

Nothing here is a source value. The shape draws no leading-edge flap (the real
aircraft has full-span automatic ones) and none is invented.

**Gear, speed brake and plume (F5E-4).** Not owned by this rig yet: the six
gear plates keep the shared interim rise and the brake and plume words stay
unmapped. The rig lists the reviewed faces and pins the three words so F5E-4
takes them over in the same module. See the [F-5E
fit](../spec/variety-animation.md#f-5e-control-fit). Update (F5E-4,
2026-11-10): the same rig now owns them; see
[F-5E gear, speed brake and plume (F5E-4, 2026-11-10)](#f-5e-gear-speed-brake-and-plume-f5e-4-2026-11-10).

## F-5E gear, speed brake and plume (F5E-4, 2026-11-10)

Slice F5E-4, Claude Sonnet (sonnet-high), implementation mode. This maps the
three device words of `F5E.SH` that [F5E-3](#f-5e-control-surfaces-f5e-3-2026-11-10)
left to the shared groups, in the same `f5e_animation` module. Source
coordinates are shape units (x right, y forward, z up, a third of a foot; the
F-5E has the identity scale). Motion and angles are **fitted** (agent
decisions, 2026-11-10); the faces and the word roles are spec-derived from the
retail shape. Each of the three words adds its faces and removes none; the rig
checks that at load, with every plate's exact corners.

| Word | Faces | Source geometry | Rig |
| --- | --- | --- | --- |
| `0x63cc` gear | 6: right main `49cc`, `49eb`; left main `4a37`, `4a56`; nose `4aa2`, `4ac1` | Three flat plates, both windings each. Mains in the planes x +14 and x -15 (y -2 to 6, z -18 to -4); nose in the plane x 0 (y 53 to 60, z -17 to -5). No doors, struts or wheels beyond them | One rigid piece per plate |
| `0x63c6` speed brake | 2: `4802` (untextured), `4819` (textured) | One plate, x +/-2: front edge (y 29, z 10), free edge (y 22, z 16), rising aft | One rigid piece about its front edge |
| `0x63c0` plume | 8: `48a7`, `48e5`, `4961`, `4980` (left), `48c6`, `4904`, `4923`, `4942` (right) | Per engine a vertical plate (x -3 or +3, z -3 to 3) and a horizontal plate (z 0, x -6 to 0 or 0 to 5), both windings, y -49 to -79 | Scaled along Y from the exit plane |

The two nozzle exit rings are the octagons `42a6` (left, x -5 to -1) and `4454`
(right, x 1 to 5) at y -48, z -3 to 1 (subtype `0x6c`). The three flap and
rudder words stay as F5E-3 left them (pinned, never set).

**Gear motion and source.** Each main plate turns `90 degrees * (1 - gear)` about
the line along Y at its own top edge (z -4, on its own x), swinging inward and
up: the foot (x 14, z -18) arrives at (x 0, z -4), so the plate ends flat at the
wing root under the fuselage, hidden at gear 0. The nose plate turns about a
lateral line through its aft top corner (y 53, z -5) and swings forward and up
into the nose (the foot (y 53, z -17) arrives at (y 65, z -5)), and steers about
a vertical line through its centre (y 56.5) before the fold. Grounded at gear
down: the pose at gear 1 is the source pose, whose lowest point (the mains, z
-18) is the 18 unit low point the flight model stands the aircraft on
(`variety.rs`, B5-1). The nose plate's foot is z -17, one unit (0.33 ft) above
the mains' ground line, as the retail shape draws it (the parked frames
show all three wheels at the runway).

**Public source (cited honestly).** I could not reach a primary source (the
T.O. 1F-5E-1 flight manual, `usaf-sig.org` lists it, was not fetched). A
web search surfaced an aircraft summary page (`ccxy.tripod.com/f-5e.html`,
Jane's-style data, host not reachable from here) whose excerpts say the main
units retract inward into the fuselage; the same page's two excerpts disagree
about the nose, one saying the nosewheel retracts forward and the other aft.
**Mains inward is the task's stated direction and the one consistent excerpt.
The nose forward is an agent decision from one of two conflicting secondary
excerpts: open item, one sign (`NOSE_HINGE` and the `angle` sign in
`gear_positions`) to flip if the flight manual says aft.** The Wikipedia
"Northrop F-5" article and the DCS and Virtavia manuals I read give no
retraction direction (Wikipedia notes only the Canadair CF-5's two-position nose
gear, "incorporated" on the F-5E to reduce take-off distance).

**Speed brake.** The one dorsal plate is all the source has (John, 2026-11-10:
rig the source's plate). The real F-5E's brakes are a pair under the fuselage
(general knowledge, not cited this pass); not drawn. The plate turns whole
(both windings) about its front edge, the line y 29, z 10 along X. Open (brake
1) is the source pose; closed is the turn of **0.866 rad (49.6 degrees)** that
lays the free edge on the dorsal ridge line from (0, 37, 10) to the mid point
(0, -34, 4) of the rear pair (+/-1, -34, 4) (all in face `3420`): the free edge
lands at (0, 19.9, 8.55), the plate lies along the ridge, and the front edge sits
0.68 unit above the ridge at y 29, the source's own step (there is no closing
front plate in this shape). Hidden at brake 0.

**Plume and nozzles.** The plume plates scale along Y from the nozzle exit plane
(y -48): at full afterburner they are the source pose (first edge y -49, tip y
-79), at partial they shorten toward the exit plane, hidden with no exhaust. The
engine material gives the F-5E two outlets (`engine_material::outlet_count` 2,
nozzle faces `42a6` and `4454`, left and right by the sign of x), so two nozzle
glows draw and, from the engine count of 2, two contrails: one behind each
nozzle's centre at x -3 and +3, z -1, 2 ft behind the exit plane (unit test
`the_f5e_draws_two_contrails_one_behind_each_nozzle`). The renderer draws the
glow and trail from the pilot's heat and the plume from `state.exhaust`.

**Rule 11 fold table (rig definitions).** Every moving piece turns whole; no
face has a vertex held off its hinge line.

| Piece | Faces | Vertices | Turn about the axis | On the axis (stay) | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right main | `49cc`, `49eb` | 4 each | 2 each (feet) | 2 each: (14, -2, -4), (14, 6, -4) | 0 |
| Left main | `4a37`, `4a56` | 4 each | 2 each | 2 each: (-15, -2, -4), (-15, 6, -4) | 0 |
| Nose plate | `4aa2`, `4ac1` | 4 each | 3 each | 1 each: (0, 53, -5) (the other top corner swings) | 0 |
| Speed brake | `4802`, `4819` | 4 each | 2 each (free edge) | 2 each: (+/-2, 29, 10) | 0 |
| Plume | eight plates | 4 each | the tip vertices | root edge stays on the exit plane's side | 0 |

Verdict: **no piece folds**. The probe reports rigidity of at most 1.6e-6 ft for
every gear pose and at most 6.4e-7 ft for every brake pose (floating point).
Fixed seam openings: none (the gear and brake hang from or rest on faces they do
not share vertices with; the closed-brake front edge step above is the source's).

**Findings and open items.** (1) Nose retraction direction above. (2) The brake
is dorsal and single against the real paired ventral brakes (John's decision).
(3) The gear is plates without doors, so the closed bay is the skin; near stow
the main plate lies flat in the wing's lower skin plane (z -4) and at gear 0 is
hidden. (4) Nose foot 0.33 ft above the main ground line, retail geometry.

**Provenance.** Faces, word roles and plate corners spec-derived (retail shape
read as data); the retraction directions, angles, the closed brake angle and the
plume scaling are fitted agent decisions (2026-11-10); the dorsal brake rig is
John's decision (2026-11-10); the twin glow and trail from the engine count of 2
(record, real aircraft: two J85-GE-21). Not a claim of original retraction
timing.

## Mirage F1 control surfaces (MF1-3, 2026-11-10)

`MF1.SH` (19,200 CODE bytes, 236 runtime faces, six state words `0x5ad0`,
`0x5ad6`, `0x5adc`, `0x5ae8`, `0x5aee`, `0x5af4`; review in
[aircraft variety](aircraft-variety.md#mirage-f1-source-review-2026-11-10), shape
scale in [MF1-2](objects-and-shapes.md#mirage-f1-shape-scale-mf1-2-2026-11-10))
names no signed pitch or roll branch and no flap travel, so the tailplane, the
ailerons and the rudder are **fitted** whole-panel assignments over exact
reviewed source faces. Source coordinates below are shape units (x right, y
forward, z up, before the per-axis scale `[1.0335, 1.1804, 1.0546]`); the rig
scales every hinge point and axis with the same factors as the shape, so a turned
panel stays rigid.

| Panel | Faces | Source geometry |
| --- | --- | --- |
| Right tailplane | `2d0e`, `2d36` (two windings) | One flat five-vertex panel at z -1: (6,-22), (5,-39), (6,-41), (23,-45), (23,-39); no state word |
| Left tailplane | `2c60`, `2c88` | Mirror with the tip at x -22 (one unit shorter than the right) |
| Right aileron (outboard trailing-edge panel) | `3ce8` (top quad), `3cc9` (bottom quad), `3d0f` (top wedge), `3d30` (bottom wedge) | x 23 to 35; hinge line (23,-6) to (35,-11) at mid thickness z 1 to 1.5; trailing edge (23,-12) to (35,-15), z 1; the panel is 4 ft wide at 59% to 90% of the semi-span |
| Left aileron | `3e93`, `3e74`, `3eba`, `3edb` | Exact mirror in x |
| Rudder | `40e1`, `4108` (quad, both windings) | Hinge edge (0,-30,3) to (0,-39,22); trailing edge (0,-37,3) to (0,-44,22) |

**Hinge axes (rig definitions).** Tailplane: a **lateral** spindle (along X)
through the root chord midpoint, (+-5.5, -31.5, -1) (the root chord runs y -22
to y -41); the panel is flat, so its own anhedral is zero and the spindle is not
tilted (the all-moving tail standard: whole panel, lateral spindle through the
root chord midpoint, tilted only by the panel's own anhedral; never a swept span
axis). Ailerons: the straight leading edge of the
panel's wedge, through the mid-thickness points (23,-6,1) and (35,-11,1.5),
direction (12,-5,0.5) on the right and the same line mirrored on the left. The
wedge's hinge vertices sit 1 unit (root) and 0.5 unit (tip) above and below it,
and the whole panel turns about the mid line. Rudder: the fin line from (0,-30,3)
to (0,-39,22), pointing up and aft, so a positive angle swings the trailing edge
toward +X (right), the sense of the fleet's rudders.

**Rule 11 fold table (rig definitions).** Every face of every moving surface turns
whole about its axis; no vertex is held fixed off an axis.

| Surface | Face | Vertices | Turn about the axis | Held fixed | Fixed off the axis |
| --- | --- | ---: | ---: | ---: | ---: |
| Right tailplane | `2d0e`, `2d36` | 5 each | 5 each | 0 | 0 |
| Left tailplane | `2c60`, `2c88` | 5 each | 5 each | 0 | 0 |
| Right aileron | `3ce8` (top quad) | 4 | 4 | 0 | 0 |
| Right aileron | `3cc9` (bottom quad) | 4 | 4 | 0 | 0 |
| Right aileron | `3d0f`, `3d30` (wedges) | 3 each | 3 each | 0 | 0 |
| Left aileron | `3e93`, `3e74` (quads), `3eba`, `3edb` (wedges) | 4, 4, 3, 3 | all | 0 | 0 |
| Rudder | `40e1`, `4108` | 4 each | 2 each | 2 each: (0,-30,3), (0,-39,22) on the hinge | 0 |

The review's own table counted the wedge hinge vertices as "on the hinge" because
they are within one unit of the hinge line. The rig turns them with the rest of
the panel about the mid line (they move by at most their offset times the angle,
0.2 unit at the full 0.20 rad), exactly as the Mirage 2000's elevon wedge does.
No face of any moving surface is split across a diagonal or a coordinate
threshold: each panel is selected by face address, and the rig refuses the shape
if a listed face is missing, duplicated, holds a vertex outside its reviewed
points, or if a state word adds or removes a different face set than reviewed.
**Verdict: no face folds**; the probe reports the largest inter-vertex distance
change in each group as about 1e-6 ft.

**Pitch only (John, 2026-11-10).** The tailplane panels take the same angle for
pitch and **none** for roll: a positive elevator raises both trailing edges
(`-0.30 * elevator` radians, positive lowers). Roll belongs to the ailerons. The
probe proves each surface stays put under every other control (tailplane under
roll, rudder and flaps; ailerons under pitch, rudder and flaps).

**Ailerons and the flap-slot words (agent decision, 2026-11-10).** The source's
one trailing-edge panel per wing is outboard, at the aileron position. Words
`0x5ae8` (left) and `0x5aee` (right) delete exactly that panel and add nothing,
so a rig that set them would make an aileron vanish in flight. The rig pins both
words at load (a mismatch refuses the shape) and never sets them. The panels
take roll only: `-/+0.20 * aileron` (a positive aileron raises the right trailing
edge and lowers the left, the sense of every fleet aileron). They do **not** act
as the flap: the sources found describe the real F1's high-lift kit as separate
double-slotted trailing-edge flaps and leading-edge slats with ailerons
(globalsecurity.org variants page: "differentially operated two-section
double-slotted flaps, ailerons, two-section spoilers"), and no source places
the flaps at the aileron position, so the flap lever moves nothing on this
shape (there is no inboard flap panel in the source; none is invented). The probe
records the flap control as not applicable for this aircraft and fails if the
lever moves any geometry. If John wants the flap lever visible, the one-line
alternative is a droop of the ailerons (the Mirage 2000 droops its elevons 0.40
with the flap lever); it is not applied.

**Rudder word `0x5af4`** swaps the quad for a copy deflected about 45 degrees to
+X (a fixed, one-direction pose, roughly 0.8 rad). The rig pins it at load and
never sets it; the quad turns about its hinge edge instead, in both directions,
with the fleet's travel `0.35 * rudder` (fitted).

**Seam openings (measured, `seam-opening.csv`, reported not gated).** Every
surface shares some vertices with fixed faces, so a turn opens a small crack:
tailplane root vertex (5,-39,-1) shared with the rear fuselage, 0.88 ft at the
full 0.30 rad pitch; aileron inboard seam (23,-8) and (23,-12) shared with the
fixed inboard wing, 0.43 ft at the full 0.20 rad roll; rudder trailing top
vertex (0,-44,22) shared with the fin cap, 0.61 ft at the full 0.35 rad. This is
the whole-panel rule's accepted cost (the MiG-31 tail and the Mirage 2000
elevons do the same); the probe's skin gap (0) measures the seam inside each
panel, and the hinge or spindle midpoints stay put (anchor gap about 2e-7 ft).

**Fitted magnitudes (agent decisions, 2026-11-10, matching the fleet).** Angle
positive lowers the trailing edge. Tailplane `-0.30 * elevator` (the F-4 and
F-104 all-moving tails); ailerons `-/+0.20 * aileron` (the F-4, F-16 and F-104
roll surfaces); rudder `0.35 * rudder` (every fleet rudder). Nothing here is a
source value.

The gear, airbrake and plume are **not** owned by this rig: the gear keeps the
shared interim rise (word `0x5adc`, plates `3b7e`, `3b9d`, `3be9`, `3c08`,
`3c54`, `3c73`) and the airbrake (`0x5ad6`: `4182`, `4199`) and plume (`0x5ad0`:
`4207`, `422e`, `4255`, `427c`, `42a3`, `42ca`, `42f1`, `4318`) stay unmapped
until MF1-4; `mf1_animation.rs` pins those words and face lists at load as the
places MF1-4 takes over. See the
[Mirage F1 fit](../spec/variety-animation.md#mirage-f1-control-fit).

Update (MF1-4, 2026-11-10): the gear, airbrake and plume are now owned by the
rig; see [Mirage F1 gear, airbrake and plume](#mirage-f1-gear-airbrake-and-plume-mf1-4-2026-11-10).

## Mirage F1 gear, airbrake and plume (MF1-4, 2026-11-10)

Slice MF1-4 (implementation, Claude Sonnet, sonnet-high). `mf1_animation.rs` now
loads the three device words with the control surfaces of
[MF1-3](#mirage-f1-control-surfaces-mf1-3-2026-11-10). Each word adds faces and
removes none (pinned at load: a different face set refuses the shape), the added
faces are scaled per axis `[1.0335, 1.1804, 1.0546]` like the rest of the shape,
and every hinge point and axis below is in shape units run through
`scale_point(Mf1, ..)`. Nothing is split: each device is whole rigid pieces
selected by face address. All six words are accounted for:

| Word | Effect in the source | Rig |
| --- | --- | --- |
| `0x5adc` | Adds the three gear plates (6 faces, both windings) | Set by the gear lever, animated (this section) |
| `0x5ad6` | Adds the one dorsal airbrake plate (2 faces) | Set by the brake lever, animated |
| `0x5ad0` | Adds the plume (8 faces) | Set by the afterburner, animated |
| `0x5ae8`, `0x5aee` | Delete one wing's outboard trailing-edge panel, add nothing | Pinned, never set (MF1-3: the panels are the ailerons) |
| `0x5af4` | Swaps the rudder quad for a copy 45 degrees to starboard | Pinned, never set (MF1-3: the quad turns) |

Beyond the six words the shape has no door, canopy, hook, slat, spoiler, nozzle
petal or inboard flap geometry, and none is invented.

**Gear (fitted; direction from a public source).** The F1's nose gear retracts
rearward and its main gear retracts forward and rotates to tuck into the
fuselage (Greg Goebel, "The Dassault Mirage F1", Air Vectors,
`airvectors.net/avmirf1.html`: "The nose landing gear assembly retracted
backwards while the main gear assemblies retracted forward, rotating to tuck
neatly into the fuselage"; a general history, not a maintenance manual). The
source plates, in shape units:

| Plate | Faces | Geometry |
| --- | --- | --- |
| Left main | `3b7e`, `3b9d` | Top edge x -3, y -5 to 4, z -6; foot edge x -7, z -17 (a leg and wheel seen from the side, 9 wide, 11 tall, leaning outboard) |
| Right main | `3be9`, `3c08` | Mirror, x +3 and +7 |
| Nose | `3c54`, `3c73` | Plane x 0, y 38 to 47, z -7 to -17 |

| Piece | Hinge (rig definition) | Angle |
| --- | --- | --- |
| Each main | The point of its own top edge 1 unit forward of the rear corner, (+-3, -4, -6), axis 15 degrees off lateral toward forward (right (0.966, 0.259, 0), left (0.966, -0.259, 0), the mirror) | `95 degrees * (1 - gear)`, the foot swings forward and inward |
| Nose | Lateral axis (along X) through its own top forward corner (0, 47, -7) | `-110 degrees * (1 - gear)`, the foot swings rearward and the whole plate rises above the hinge line |
| Nose steering | Vertical axis through the plate centre (0, 42.5, -12) | `-nosewheel_angle`, applied first |

Gear 1 is the source pose (grounded: the lowest vertex is z -17 units, the MF1-2
scaled gear low point 17/3 * 1.0546 = 5.98 ft the flight model and the parked
path stand on); gear 0 draws nothing. A purely lateral axis would not stow the
mains: the plate is 9 deep and 11 tall, so a quarter turn stands it on edge with
its outboard foot corner about 1 ft above the wing shoulder, outside the hull.
The 15 degree lean and the 95 degree angle were found by searching the hinge
point, lean and angle against the fuselage section read off the skin faces
(7 units half-wide from z -4 to 1 at y 7, 6 at z -7 and z 3, 2 at the ridge z 5,
narrowing aft to 0.72 of that at y -39): stowed, each main lies at x 2.7 to 6.1,
y -4.0 to 6.5, z -7.3 to 2.5 (shape units), inside the hull to within 0.1 unit.
The cost of the lean: the main foot's rear corner dips 0.22 unit (0.08 ft)
below its gear-down height in the first few degrees of travel. The probe allows
0.1 ft. The nose plate stows in the plane x 0 between y 38.6 and 50.1, z -7 to 5.9 (the nose bay roof is at z 6).

**Airbrake (fitted).** The one source plate (`4182`, `4199`, both windings):
forward edge (-2, 29, 5) to (3, 29, 5), free edge (+-2, 19, 14) raised aft. The
hinge is the forward edge, a line along X at (y 29, z 5), and the whole plate
turns about it (the plate is not split: its four vertices are two on the hinge
and two on the free edge). Brake 1 is the source raised pose; the angle
`0.707 rad (40.5 degrees) * (1 - brake)` lays the plate along the dorsal ridge
line (0, 37, 6) to (0, 7, 5) at a small brake (measured on the scaled shape: the
free edge within 4e-6 ft of the ridge line at 1e-6); brake 0 hides it. The
closed plate lies 0.7 unit under the ridge skin, so a thin sliver of its
outboard corner shows beside the spine at small values (frame
`crop3x-airbrake-closed-side`). The real F1's brakes are paired panels under the
intakes (MF1-0 review); the source has only this dorsal plate, so only it moves.

**Burner (fitted).** The plume is two crossed pairs of double-sided plates: x = 0
plane (`4207`, `422e` outer, y -56 to -81; `42a3`, `42ca` inner, y -42 to -56,
z -5 to 3) and z = -1 plane (`4255`, `427c` outer; `42f1`, `4318` inner, x -3 to
4). The nozzle exit plane y -42 is the root: every plume vertex scales along Y
about it by the afterburner fraction, so each plate stays in its plane (nothing
folds; a plate that only stretches along its own plane stays flat), shared edges
stay together (the y -56 edge is the same in the inner and outer plate), and the
exit corners never move. Exhaust 0 draws nothing. `engine_material` gains the
Mirage F1: `outlet_count(Mf1) = 1` (the record's one Atar 9K-50) and the nozzle
faces `2e92` and `2eb3`, the two half discs either side of x = 0 at y -42 (x -4
to 4, z -6 to 3, subtype 0x64; the two thin triangles at x +-4 in that plane have
no area and carry no material). That gives one glow on the whole disc and one
contrail at the disc centre (x 0, z -1.5) 2 ft behind the exit plane; MF1-2's
shape-end fallback is no longer used (unit test
`the_mirage_f1_draws_one_contrail_on_the_centreline_behind_its_one_nozzle`).

**Rule 11 fold table (rig definitions).** Every added face turns or stretches
whole; no vertex is held fixed off its hinge, and no face is selected by a
coordinate threshold.

| Piece | Faces | Vertices | Moved as one rigid body | Held fixed | Fixed off the hinge |
| --- | --- | ---: | ---: | ---: | ---: |
| Left main | `3b7e`, `3b9d` | 4 each | 4 each | 0 (the hinge point is on the top edge, not a vertex) | 0 |
| Right main | `3be9`, `3c08` | 4 each | 4 each | 0 | 0 |
| Nose | `3c54`, `3c73` | 4 each | 3 each | 1 each: (0, 47, -7), on the hinge | 0 |
| Airbrake | `4182`, `4199` | 4 each | 2 each | 2 each: (-2, 29, 5), (3, 29, 5), on the hinge | 0 |
| Plume | the eight plates | 4 each | stretched about y -42 | the exit-plane vertices (y -42) | 0 |

**Verdict: no face folds.** The probe's largest inter-vertex distance change in
each gear plate and in the airbrake is about 6e-7 ft; the gear skin gap is 0.

**Probe numbers** (`.local/m3-aircraft/checks/MF1-4/poses`, 0 failures): gear,
brake and exhaust `required moved`; 202 dense gear values (0 to 1 in 200 steps
plus 1e-6), every row passing: rigidity about 1e-6 ft, anchor gap about 2e-7 ft,
skin gap 0, no direction failure, no new planar crossing, ground contact at gear
1 (lowest point the scaled low point within 1e-3 ft), stow gates at 1e-6; the
seven-value brake sweep (hinge gap 0, rigidity about 6e-7 ft, closed within
4e-6 ft of the ridge, never raising the free edge); exhaust: exit corners fixed
(gap 0), tips forward as the demand falls, the MF1-3 25-pose pitch/roll grid
still passing.

**Provenance.** Gear, airbrake and plume motion and every hinge and angle:
fitted, agent decisions 2026-11-10. Retraction directions: public secondary
source (Air Vectors, above), not original behaviour. Face lists, word effects,
plate corners and the nozzle disc: spec-derived from the shape. Engine count 1:
John's decision 23, 2026-11-10. Nothing here is a claim of the original's
retraction timing or travel.
