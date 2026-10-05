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
right, forward, up. These shapes use one-third foot per unit. Source archive
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
