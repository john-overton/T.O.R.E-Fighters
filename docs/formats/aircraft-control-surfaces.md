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
