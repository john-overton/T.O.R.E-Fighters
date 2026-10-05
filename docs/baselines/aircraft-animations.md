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

## Hercules family and helicopter results

C130.PT and AC130.PT pass their specific control/gear witnesses and complete
pose-sheet review in `.local/animation-probes/c130-first` and `ac130-first`.
Their original propeller phase filtering remains active: C-130 retains eight
cards, AC-130 retains 32, without alternate phase overlays. All four propellers
move on each aircraft. AC-130's ten barrel faces covering three mounts retain
their combat pivot radii through both aiming sweeps, within 0.000002 source unit.

| Aircraft | Gear samples | Minimum wheel gap, ft | Maximum rigid error, ft |
| --- | ---: | ---: | ---: |
| C-130 | 22 | 11.333334 | 0.000001907 |
| AC-130 | 22 | 12.000004 | 0.000000636 |
| Mi-24 | 22 | 4.498559 | 0.000000954 |

Independent 201-position source reviews confirm rigid Hercules gear. C-130 has
one unchanged transparent upper-card margin outside its source hull; its
original pixel is index 255. AC-130's final complete stow cards pass a 35 by 13
point grid against source body sections. Its source gear does not establish
fixed strut roots, so the probe tests rigid recession and stow instead of
invented hinges. **AC-130 hook visual mapping remains unknown**, while its PT
command capability stays enabled. This is not full device acceptance.

AH64.PT, MI24.PT and CH47.PT pass independent rotor attachment, rigidity,
signed cyclic and spin-plane checks in `.local/animation-probes/*-combined`.
Pitch, roll and rotor phase sheets were inspected on each, along with CH-47
differential yaw and the worst combined tandem pose. Mi-24's repaired gear
has rigid source panels, fixed roots, exact deployment and complete stow;
its intermediate gear sheets were also reviewed.

The combined sweeps contain 1,200 poses each for AH-64 and Mi-24, and 6,000 for
CH-47. Largest inferred mast attachment error is 0.000007626 ft, and largest
panel dimension error is 0.000010173 ft. CH-47 minimum rear/front panel clearance
is 0.388398 ft across overlapping projected footprints. Original tilted Mi-24
and CH-47 panels now spin in their source planes instead of wobbling around
vertical. Collective feathering and AH-64/Mi-24 pedal blade-pitch geometry
remain unimplemented because the original art is a flat rotor image. Their
flight controls remain available; those visual limitations are not labeled
retail behavior or physical absence.

## A-10, Harrier and E-3 results

A10.PT passes `.local/animation-probes/a10-first` and eleven focused tests.
All five required groups, source flap/deployed endpoints, fixed roots and
paired skins pass. All sheets were reviewed, including retained main-wheel
stow and the neutral overlay. Gear uses 22 poses, with a minimum main gap of
13 ft and maximum whole-assembly dimension error of 0.000001272 ft. The
independent runtime atlas witness checks every opaque nose sample against
source body sections at near-zero gear. Main wheels deliberately remain visible
at zero in their documented exposed stow; this is an agent fit, not retail parity.

AV8.PT passes `.local/animation-probes/av8-reviewed`, four focused tests and all
25 nozzle pitch/yaw combinations. The four nozzle centers, exact source flap
endpoints and paired skins pass. Gear has 22 samples and includes the previously
unowned central gear, which retracts with the outriggers. All moving and static
sheets, gear intermediates and nozzle extremes were reviewed. A separate
synthetic ground-start plus 240-tick idle test confirms the complete nose wheel
rests on the runway with the corrected 7 ft contact fit.

E3.PT passes `.local/animation-probes/e3-first` and four focused tests. All
control/device sheets, 22 gear poses and 25 flap/roll combinations were reviewed.
Maximum fitted-pivot error is 0.000004779 ft and maximum rigidity error is
0.000003815 ft, with zero control hinge/skin gaps or new planar crossings.
The conservative raw-card gap is 0.326726 ft near stow; independent original
indexed-art sampling through 201 poses establishes at least 1.11098 ft between
painted lower wheel/strut regions. Complete stow cards and dense interior grids
fit source body sections. Radome motion and other unreviewed devices remain
outside this bounded acceptance.

## Focused checkpoint checks

The eighteen-profile set passed 127 Rust tests selected by `animation`, ten
rotor-focused tests, the Harrier ground-contact test and an app-only build.
App-only Clippy, formatting and documentation consistency passed. The shared
dispatcher retains propeller, rotor and manually aimed gun overlays after
specific control rigs.

All eighteen `flight-animation-*` scenarios pass in
`.local/animation-battery/20261005-173532-eighteen-reviewed/summary.md`.
Thirty-seven focused Python report/selection tests passed. The battery requires
reviewed attachment scope and passing per-control gates, plus complete combined
rotor, flaperon and nozzle artifacts where applicable. A motion-only report
cannot pass as reviewed geometry.

These checks do not accept queued profiles or replace the deferred full
workspace, full battery, creator, GPU and retail-runtime validation.

## Remaining aircraft

The code inventory covers all 37 playable profiles in the ignored local report
`.local/animation-audit/mapping-inventory.md`. At the start of this pass all 23
variety imports lacked pitch, roll and flap mappings. Eighteen profiles have bounded geometry acceptance in the table below, with
explicit device gaps such as AC-130 hook geometry and helicopter blade feathering. Every profile has a baseline
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
| `a10` | Missing elevator, rudder, aileron, flaps | Controls and gear reviewed; exposed main-wheel stow is fitted |
| `c130` | Missing elevator, rudder, aileron, flaps | Controls, rigid gear and propeller overlay reviewed |
| `ac130` | Missing elevator, rudder, aileron, flaps, hook | Controls, gear, props and gun overlays reviewed; visual hook unresolved |
| `e3` | Missing elevator, aileron, flaps | Controls, rigid gear and 25 flap/roll combinations reviewed |
| `il76` | Missing elevator, rudder, aileron, flaps | Queued |
| `e2` | Missing elevator, rudder, aileron, flaps | Queued |
| `b747` | Missing elevator, aileron, flaps | Queued |
| `a310` | Missing elevator, aileron, flaps | Focused geometry and topology accepted; mechanics fitted |
| `av8` | Missing elevator, rudder, aileron, flaps, vector-pitch | Controls, all central/outrigger gear and 25 nozzle combinations reviewed |
| `yak141` | Missing elevator, rudder, aileron, flaps, vector-pitch | Queued |
| `v22` | Missing elevator, rudder, aileron, flaps | Queued |
| `ah64` | Motion survey captured | Cyclic and rotor geometry reviewed; blade feathering unresolved |
| `mi24` | Motion survey captured | Cyclic, rotor and rigid gear reviewed; blade feathering unresolved |
| `ch47` | Motion survey captured | Cyclic, differential yaw and tandem separation reviewed; blade feathering unresolved |
| `f18` | Motion survey captured | Main-wheel crossing and attachment repairs in progress |
| `rafale` | Motion survey captured; hinges unreviewed | Visual review found crossed main gear and detached thick flap fronts; repair pending |
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
