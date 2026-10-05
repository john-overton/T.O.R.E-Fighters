# Aircraft animation audit

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation audit, 2026-10-05. John requested aircraft-by-aircraft headless
animation checks after observing a detached A-7 rudder and missing control
surfaces. This audit uses the actual clean drawing transforms through
`Airframe::animation_faces`; it does not infer animation correctness from flight
stability, imported branch counts, or creator launch tests. Full validation is
on hold while this per-aircraft review is in progress.

## Method

`--aircraft ID --animation-probe OUTPUT_DIRECTORY --no-audio` writes geometry
metrics, actual posed OBJ data and top/side/rear CPU wireframe contact sheets
without a window. Outputs contain user-owned geometry and stay under ignored
`.local/`. Control samples are -1, -0.5, 0, 0.5, 1; device samples are 0, 0.25,
0.5, 0.75, 1. The probe distinguishes missing required motion, unreviewed
capabilities, and source-supported non-applicability. Existing
`--headless-flight` does not exercise the later rendered-pose override path.

Measure signed paired-surface movement, attachment invariants, coincident skin
coherence, finite/reversible poses and intermediate travel. Shared moving/fixed
vertices can legitimately separate between distinct controls, so generic gap
candidates require source review; reviewed attachment edges are acceptance
gates. Each aircraft also needs visual inspection of its source-based pose
sheets. A moving-face count alone is not an acceptance result.

## A-7 before repair

Evidence: `.local/animation-probes/a7-before/` and
`.local/animation-audit/a7-geometry.json`. The shape identity and motion contract
are recorded in the [A-7 specification](../spec/variety-animation.md#a-7-surface-repair-and-acceptance).

- Elevator, aileron and flaps changed zero faces at all five inputs.
- The rudder used positive-deflection skins instead of neutral skins. At full
  input a reviewed attachment separated about 1.423 ft from the fixed fin.
- The hook root separated about 0.870, 0.591 and 0.299 ft at deployment 0.25,
  0.5 and 0.75 respectively.
- Gear, brakes and hook produced finite changing geometry, but the aggregate
  bounds-based motion did not establish correct hinges or coherent assemblies.

The corrected A-7 geometry passed in `.local/animation-probes/a7-verified/`.
Eight required source-geometry controls move; reviewed neutral geometry, signed
pitch/roll, fixed rudder/hook roots and twin skins pass. Gear was separately
repaired after the first visual pass exposed crossing wheels. Across 21 samples
the minimum wheel gap is 0.711565 ft, upper attachments remain fixed and wheel
geometry remains rigid. Top/side/rear pose sheets were inspected, including
five-position gear overviews. This accepts the bounded fitted geometry slice,
not recovered mechanical timing or a texture/GPU comparison.

## F-4 family results

Exact F4B.PT, F4J.PT, F4E.PT and F4.PT were each probed before and after repair.
Before: all four lacked pitch, roll and flap motion and used deflected rudder
skins at neutral. After: each passed required motion, reviewed attachment,
neutral, direction and twin-skin gates. Source-hook capability is enabled for
B/J and disabled for E/G. Minimum main-gear gaps over 21 samples were 11.2902 ft
for B/J, 5.17382 ft for E and 4.85290 ft for G. Source asymmetries were retained.

All three shape families' pitch, rudder, roll, flap, brake and gear sheets were
inspected, plus B/J hook closing. No detached roots, reversed closing or wheel
crossing was seen. B/J's seven rendered diagnostic PNGs are byte-identical,
consistent with their shared shape; each report retains its exact PT identity.
Evidence is under `.local/animation-probes/f4*-before` and `f4*-after`; the
independent review is `.local/animation-audit/f4-visual-review.md`.

Focused checks for this checkpoint: five A-7 tests, seven F-4 tests and eight
probe tests passed, followed by an app-only build. No broad workspace, creator,
GPU or retail-runtime validation was run for this animation pass.

## Remaining aircraft

The code inventory covers all 37 playable profiles in the ignored local report
`.local/animation-audit/mapping-inventory.md`. At the start of this pass all 23
variety imports lacked pitch, roll and flap mappings. A-7 and the four F-4
variants now have focused geometry acceptance. The remaining 18 variety
aircraft and 14 older profiles still need individual measured acceptance. Existing transforms are not marked accepted merely because they are
present.

Review order: A-7; F-4B/J/E/G; F-15/F-16/F-104/MiG-17/A-10; transports, AWACS and
airliners; VTOL/tiltrotor/helicopters; then the earlier fighter rigs. Shared shape
families may share source analysis, while each exact aircraft keeps its own
capability and result. F-4 neutral rudders, flap endpoints and fitted control/device repairs now have
individual geometry acceptance. Other aircraft remain in the queue below.

## Per-aircraft progress

This table is an audit queue, not a blanket animation pass. A motion-only probe
does not accept unreviewed hinge geometry.

| Aircraft | Measured baseline | Current status |
| --- | --- | --- |
| `a7` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4b` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4j` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4e` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4g` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f15` | Missing elevator, rudder, aileron, flaps | Source geometry and fitted regions under review |
| `f16c` | Missing elevator, aileron, flaps | Source geometry reviewed; fitted rules being prepared |
| `f104` | Missing elevator, aileron, flaps | Source geometry reviewed; fitted rules being prepared |
| `mig17f` | Missing elevator, rudder, aileron, flaps | Queued |
| `a10` | Missing elevator, rudder, aileron, flaps | Queued |
| `c130` | Missing elevator, rudder, aileron, flaps | Queued |
| `ac130` | Missing elevator, rudder, aileron, flaps, hook | Queued |
| `e3` | Missing elevator, aileron, flaps | Queued |
| `il76` | Missing elevator, rudder, aileron, flaps | Queued |
| `e2` | Missing elevator, rudder, aileron, flaps | Queued |
| `b747` | Missing elevator, aileron, flaps | Queued |
| `a310` | Missing elevator, aileron, flaps | Queued |
| `av8` | Not yet probed | Queued |
| `yak141` | Not yet probed | Queued |
| `v22` | Not yet probed | Queued |
| `ah64` | Not yet probed | Queued |
| `mi24` | Not yet probed | Queued |
| `ch47` | Not yet probed | Queued |
| `f18` | Not yet probed | Queued |
| `rafale` | Not yet probed | Queued |
| `f14` | Not yet probed | Queued |
| `a4e` | Not yet probed | Queued |
| `x31` | Not yet probed | Queued |
| `mig29` | Not yet probed | Queued |
| `su27` | Not yet probed | Queued |
| `mig21` | Not yet probed | Queued |
| `su25` | Not yet probed | Queued |
| `mig23` | Not yet probed | Queued |
| `su35` | Not yet probed | Queued |
| `f22` | Not yet probed | Queued |
| `f22n` | Not yet probed | Queued |
| `faxx` | Not yet probed | Queued |
