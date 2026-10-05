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
0.5, 0.75, 1. Reviewed gear uses at least 21 evenly spaced samples, with
additional near-stow and known failure positions. The probe distinguishes missing required motion, unreviewed
capabilities, and source-supported non-applicability. Existing
`--headless-flight` does not exercise the later rendered-pose override path.

Measure signed paired-surface movement, attachment invariants, coincident skin
coherence, finite/reversible poses and intermediate travel. Shared moving/fixed
vertices can legitimately separate between distinct controls, so generic gap
candidates require source review; reviewed attachment edges are acceptance
gates. New planar self-intersections fail independently of movement and hinge
metrics; hook closing must raise the blade. Reports distinguish a generic motion
survey from reviewed attachment checks. Each aircraft also needs visual inspection of its source-based pose
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

The corrected A-7 passes `.local/animation-probes/a7-checked/`. Eight
required source-geometry controls move. Reviewed neutral geometry, signed
pitch/roll, rudder/hook roots and twin skins pass. Stronger checks exposed a
previous downward-closing hook and a main connector crossing near stow; the
hook now closes upward, and the final main-wheel lift ends slightly lower and
farther apart. All 16 source wheel corners fit sampled neutral-body sections.

Across 22 gear samples, including a near-zero pose before hiding, minimum wheel
gap is 0.711565 ft. Upper attachments remain fixed, wheels stay rigid and no new
planar crossings occur. A synthetic connector regression covers 401 fractions.
Top/side/rear sheets were reviewed, including intermediate gear and hook travel.
This accepts bounded fitted geometry, not recovered mechanical timing or a
texture/GPU comparison.

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

The four F-4 identities also pass the expanded topology/hook checks and a
near-stow gear sample in `.local/animation-probes/f4*-checked/`. Their geometry
rules did not change in this second probe pass.


## F-15 and MiG-17 results

F15.PT and MIG17F.PT pass the actual drawing-path probes in
`.local/animation-probes/f15-repaired` and `mig17-repaired`. Their source-neutral
geometry, signed controls, reviewed roots and same-surface joins pass. Gear
minimum side-to-side clearance is 6 ft on each aircraft, and maximum measured
rigid-wheel distance error is below 0.000002 ft. No new planar crossings remain
in the sampled poses. All required control/device sheets were inspected.

The F-15 probe includes 22 gear positions. Its former split nose connector
crossed while retracting; the complete assembly now folds rigidly at the
source texture's forward brace root. All 24 complete nose/door panel corners
fit sampled neutral-body cross sections at stow. The MiG-17 uses 23 gear
positions, including its previous connector-crossover fraction. Its new whole
rigid fold preserves an atlas-supported painted root. All 621 inspected opaque
nose texel centers fit sampled forebody bounds at stow. Transparent panel
rectangle bounds are not treated as solid wheel or strut volume.

These are bounded fitted geometry results. GPU texture/culling, damage-state
motion, recovered timing and retail runtime comparison were not validated.

## F-16C, F-104N and A310 results

F16C.PT and F104.PT pass their exact source checks and all 25 combined flap/roll
poses. Evidence is `.local/animation-probes/f16c-checked` and `f104-repaired`.
The F-16 painted nose brace had crossed at gear 0.60; its distal pin now stays
rigid and attached to the wheel cut. The F-104 had crossed a main connector at
0.35; its revised corner path preserves clearance below the fixed root chord.
Neither repair changes the required independent topology gate.

A310.PT passes `.local/animation-probes/a310-repaired`. Its previous weighted
nose connector crossed because transparent image corners were treated as extra
roots. Its complete rigid nose assembly now folds around the front-view
attachment. A separate 201-position source review finds no new planar or
own-skin triangulation intersections. Complete rigid regions fit upper, lower
and side body sections; minimum clearances are 10, 0.1227 and 0.5 source units.

| Aircraft | Gear samples | Minimum wheel gap, ft | Maximum rigidity error, ft |
| --- | ---: | ---: | ---: |
| F-16C | 22 | 0.933337 | 0.000000636 |
| F-104N | 22 | 5.666669 | 0.000000318 |
| A310 | 22 | 1.333333 | 0.000003179 |

All required groups pass, with zero reviewed root/skin gaps and no new sampled
planar crossings. Every moving control/device sheet and the static channel
sheets were inspected. Gear intermediates, combined flap/roll extremes and the
F-104 upward-closing hook were reviewed. This remains bounded CPU geometry
acceptance; no broad workspace, creator, GPU or retail-runtime validation has
been run for this animation pass.

## Focused checkpoint checks

The repaired ten-profile set passed 94 Rust tests selected by `animation` and
an app-only build. App-only Clippy, formatting and documentation consistency
also passed. This includes the shared dispatcher retaining propeller,
rotor and manually aimed gun overlays after aircraft-specific control rigs.
These checks do not accept profiles still
listed as queued, or replace the deferred full workspace/GPU validation.

## Remaining aircraft

The code inventory covers all 37 playable profiles in the ignored local report
`.local/animation-audit/mapping-inventory.md`. At the start of this pass all 23
variety imports lacked pitch, roll and flap mappings. Ten profiles have passed focused geometry review: A-7, the four F-4 variants,
F-15, F-16C, F-104N, MiG-17F and A310. Every profile has a baseline
motion survey; the table below records which still needs individual attachment
and pose acceptance. Existing transforms are not accepted merely because they
are present.

Review order: A-7; F-4B/J/E/G; F-15/F-16/F-104/MiG-17/A-10; transports, AWACS and
airliners; VTOL/tiltrotor/helicopters; then the earlier fighter rigs. Shared shape
families may share source analysis, while each exact aircraft keeps its own
capability and result. Other aircraft remain in the queue below.

## Per-aircraft progress

This table is an audit queue, not a blanket animation pass. A motion-only probe
does not accept unreviewed hinge geometry.

| Aircraft | Measured baseline | Current status |
| --- | --- | --- |
| `a7` | Missing elevator, aileron, flaps | Focused geometry, hook direction and topology accepted; mechanics fitted |
| `f4b` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4j` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4e` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f4g` | Missing elevator, aileron, flaps | Focused geometry accepted; mechanics remain fitted |
| `f15` | Missing elevator, rudder, aileron, flaps | Focused geometry and topology checks passed; mechanics fitted |
| `f16c` | Missing elevator, aileron, flaps | Focused geometry, topology and mixed flap/roll poses accepted; mechanics fitted |
| `f104` | Missing elevator, aileron, flaps | Focused geometry, hook and mixed flap/roll poses accepted; mechanics fitted |
| `mig17f` | Missing elevator, rudder, aileron, flaps | Focused geometry and topology accepted; mechanics fitted |
| `a10` | Missing elevator, rudder, aileron, flaps | Queued |
| `c130` | Missing elevator, rudder, aileron, flaps | Queued |
| `ac130` | Missing elevator, rudder, aileron, flaps, hook | Queued |
| `e3` | Missing elevator, aileron, flaps | Queued |
| `il76` | Missing elevator, rudder, aileron, flaps | Queued |
| `e2` | Missing elevator, rudder, aileron, flaps | Queued |
| `b747` | Missing elevator, aileron, flaps | Queued |
| `a310` | Missing elevator, aileron, flaps | Focused geometry and topology accepted; mechanics fitted |
| `av8` | Missing elevator, rudder, aileron, flaps, vector-pitch | Queued |
| `yak141` | Missing elevator, rudder, aileron, flaps, vector-pitch | Queued |
| `v22` | Missing elevator, rudder, aileron, flaps | Queued |
| `ah64` | Motion survey captured; hinges unreviewed | Queued |
| `mi24` | Motion survey captured; hinges unreviewed | Queued |
| `ch47` | Motion survey captured; hinges unreviewed | Queued |
| `f18` | Motion survey captured | Visual review found inward-folding main gear crossing; repair pending |
| `rafale` | Motion survey captured; hinges unreviewed | Queued |
| `f14` | Motion survey captured; hinges unreviewed | Queued |
| `a4e` | Motion survey captured; hinges unreviewed | Queued |
| `x31` | Motion survey captured; hinges unreviewed | Queued |
| `mig29` | Motion survey captured; hinges unreviewed | Queued |
| `su27` | Motion survey captured; hinges unreviewed | Queued |
| `mig21` | Motion survey captured; hinges unreviewed | Queued |
| `su25` | Motion survey captured; hinges unreviewed | Queued |
| `mig23` | Motion survey captured; hinges unreviewed | Queued |
| `su35` | Motion survey captured; hinges unreviewed | Queued |
| `f22` | Motion survey captured; hinges unreviewed | Queued |
| `f22n` | Motion survey captured; hinges unreviewed | Queued |
| `faxx` | Motion survey captured; hinges unreviewed | Queued |
