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
stability, imported branch counts, or creator launch tests. The individual CPU pose review is complete for all 37 profiles. Broader
repository validation follows this review; results are recorded below.

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

## Older F/A-18D and Rafale repairs

Exact F18.PT passes `.local/animation-probes/f18-checked`. Its old inward
main-wheel fold crossed the centerline; the new rooted oblique folds keep
source tire assemblies rigid and separated. Thick flap fronts, separate
nose-brace attachments and door boundaries were corrected. All eight moving
sheets, static sheets, neutral overlay and all 22 gear details were reviewed.
Maximum source-root error is 0.000005185 ft, skin/brace connection error
0.000002716 ft, rigid-wheel error 0.000001907 ft and minimum sampled wheel gap
0.939457 ft. All original tire samples fit source sections at stow. Twenty-eight
upper-right strut samples remain outside the belly and are intentionally kept
visible rather than disappearing; this is an explicit fitted retained joint.

Exact RAFALE.PT passes `.local/animation-probes/rafale-checked`, including 125
mixed flap/pitch/roll poses. Main wheel cards now fold aft around the painted
forward brace point instead of crossing each other. Both thick flap fronts and
the sloped nose-door hinge stay attached. All seven moving sheets, the neutral
overlay and mixed-control extremes were reviewed. Main-card separation stays
2.666667 ft; maximum root error is 0.000003331 ft and rigid-panel error
0.000001590 ft, with zero reviewed skin gaps and new planar crossings. All
2,908 inspected opaque main samples fit sampled body sections at stow; five
of 578 raw-card grid points outside the hull are transparent margins.

These checks include retained existing controls, not just repaired gear. The
older initial presentation baselines did not establish these attachment and
separation properties. Retail mechanical timing and GPU texture/culling remain
outside this bounded CPU acceptance.

## Further transport and powered-lift results

Actual source probes `yak141-first`, `v22-first`, `il76-first`, `e2-first` and
`b747-first` under `.local/animation-probes/` all pass required controls,
attachment/skin checks, signed movement, 22 gear samples and new planar-crossing
gates. All control/device sheets, neutral overlays and gear intermediates were
reviewed. Combined sweeps cover 25 Yak nozzle poses, 25 V-22 flaperon plus
25 conversion/rotor poses, 25 Il-76 flap/roll poses, 25 E-2 flap/roll plus
25 pitch/yaw poses, and 25 Boeing flap/roll poses.

| Aircraft | Maximum gear root error, feet | Maximum rigid-assembly error, feet | Minimum main-card gap, feet |
| --- | ---: | ---: | ---: |
| Il-76 | 0.000001311 | 0.000002543 | 4.6667004 |
| E-2 | 0.000000318 | 0.000001113 | 20 |
| Boeing 747 | 0.000005087 | 0.000005087 | 2.6666667 |

All three have zero reviewed surface-root and skin gaps. Independent 201-pose
source reviews preserve whole shaft/wheel assemblies and enclosed full-stow
samples. E-2 body/nacelle sections are treated as separate intervals, not a
single solid box. Boeing coverage includes 11,016 full-stow triangle samples
and painted upper-shaft attachments throughout travel, plus exact source flap
triangle interiors and materials. Its complete deployed ground plane confirms
the existing 20-foot clearance. Yak nozzle ring/seams remain exact with outlet
rigidity error below 0.000001 feet. V-22 nacelle/prop attachment and rigidity
stay below the 0.0001-foot limit; shared rotor postprocessing remains active.

These are fitted geometry acceptances. The Yak flexible nozzle neck, V-22
blade feathering, E-2 radome movement and original mechanisms/timing retain
the limitations in their contracts. Textured GPU and retail runtime comparisons
were not performed.

## Further older-fighter results

Exact `mig29-first`, `su27-first`, `su35-checked` and `mig21-first` source probes
pass required controls, 22 gear samples and 25 flap/roll combinations each.
Every moving/static sheet, selected gear intermediate and combined-control
extreme was inspected. The MiG-21 ventral brake also received an isolated
actual-OBJ inspection. Reviewed cut points, paired skins and combined-control
roots stay coherent, with no signed-motion failures or new planar crossings.

The old MiG-29 right main gear crossed the centerline by 3.252184 source units
at gear 0.05. Its corrected aft fold retains signed X>=13. Su-27 main cards
retain X=±23. Su-35's existing inward fold was already separate, with measured
minimum signed X=2.385374, and is retained while correcting the nose brace.
MiG-21 corrects asymmetric source shafts instead of mirroring the left pivot.
Su-35 and MiG-21 retain their distinct source root asymmetries. These checks
accept specific fitted presentation, not original mechanics or flight dynamics.

Su-25 passes `su25-checked`, seven focused tests and the app build. All moving
and static sheets, intermediate/near-stow gear and 25 mixed poses were reviewed.
Its old right main crossed the centerline by 3.599803 source units at gear 0.20;
complete cards now retain source X positions. Independent witnesses preserve
fixed fin/forward wing, source UV and shade order, sixteen rigid cards and a
separately anchored nose brace. All five signed rudder samples have zero UV
correspondence error. Combined cases have zero root, skin and material errors,
new planar crossings and direction failures. No exhaust geometry is asserted.

## F-22 family results

Exact `f22-checked`, `f22n-checked` and `faxx-checked` pass required source
checks after the first runtime probes rejected a motionless rudder fit and a
separated brake center seam. The revised fin cut includes actual free trailing
vertices; joined brake deformation retains every front root and the common
rear edge. A separate applicability correction skips the nonexistent F-22 hook.
No synthetic-test pass was counted as actual-source acceptance.

All actual control/device sheets, 66 gear rows, 75 combined pitch/roll poses and
25 F/A-XX flap/yaw poses were inspected. The latter check measured leaf angles,
not just leaf counts. Maximum attachment error is 0.000001272 feet, paired-skin
gap zero and rigid-assembly error 0.000002226 feet. Minimum raw main-card gap
is 1.225935 feet. The 175-degree nose fold is confirmed from the actual near-zero
OBJ; dense source review encloses all 4,225 nose-card samples in both donors'
vertical body sections.

All fifteen before/after bay comparisons match exactly at 0, 0.25, 0.5, 0.75
and 1. Closed bays retain exactly five original belly faces and no lining;
open bay groups retain 39 faces on F-22 and 41 on F-22N/F/A-XX. The new dispatch
also has a synthetic regression for split leaves and lining preservation.
Source evidence and comparisons remain in the shared ignored audit directory.

Limitations remain explicit: original door roots have source clearances, the
F-22 nose has an incomplete side strip, and stock flap interiors follow fitted
travel rather than exact original down-branch triangulations. Vertical-section
containment is not a full watertight collision proof. Canopy/recolor and textured
GPU presentation are not newly accepted by these CPU checks. Reports now include
a separate selection key so F/A-XX cannot be mistaken for its F22N donor.

## X-31 results

Exact `x31-checked` passes required source controls and attachments, including
explicit brake/exhaust requirements. All 17 control/device sheets, 22 gear
poses and 25 full-exhaust combined prototype-vector poses were inspected.
The combined inputs use signed achieved auxiliary rates, not powered-lift
lever fields. Plume, paddle and source-nozzle witnesses agree within the fitted
envelope. Maximum combined attachment error is 0.000001274 feet, skin gap
0.000000040 feet and rigidity error 0.000003815 feet. Minimum sampled main
tire-side gap is 0.7838332 feet at gear 0.10. There are no new planar crossings.

Whole wheel cards retain their dimensions, the nose brace stays attached
without inversion, and opaque source-art stow witnesses pass before hiding.
Inner flaps reproduce source endpoints; outer flap-only panels deliberately
stay fixed. The source-based prototype sheets show both signed directions
without a visible break at the nozzle in these CPU views. Original mechanisms,
textured GPU presentation and retail handling remain outside this acceptance.

## MiG-23 results

Exact `mig23-checked` passes required source checks, including explicit brake
and exhaust requirements. Every moving/static sheet, all 22 gear poses and all
125 sweep/flap/roll combinations were reviewed, including enlarged intermediate
and extreme views. The independent sweep witness infers the observed rigid
wing transform; it does not call the production animation formula.

Maximum observed attachment error is 0.000020907 feet, below the 0.0001-foot
limit. Skin/material errors, new planar crossings and signed-direction failures
are zero. All six signed rudder/material samples, including tiny positive
input, have zero UV error and correct original correspondence. Whole main cards
retain source X positions, the separate nose brace stays attached and exact
full deployment returns to source placement. These checks accept fitted
presentation, not original gear bay volumes, timing or textured GPU appearance.

## F-14D results

Exact `f14-checked` passes 25 focused tests and the source probe. All 17
control/device sheets, 22 gear samples, six hook samples including positive
near-stow, 125 sweep/flap/roll poses and 25 pitch/roll poses were reviewed.
Initial flap/sweep failures were diagnostic-reference errors: ordinary
initialization starts at 450 knots, already swept, whereas draft witnesses
assumed fully forward wings. Isolated F-14 controls now explicitly use the
existing unswept 400-knot threshold. Combined tests cover the real coupled
behavior using observed wing frames, not production sweep formulas.

Maximum single-control attachment gap is 0.000000080 feet, paired-skin gap
0.000005087 feet and rigid-panel error 0.000002544 feet. Minimum sampled main
wheel gap is 3.095322 feet. Across coupled wing/flap/roll poses maximum root gap
is 0.000001422 feet and rigid error 0.000003815 feet. Across pitch/roll poses
maximum root gap is 0.000001273 feet. No introduced planar crossings, signed
motion failures or endpoint/material failures occur.

Direct original donor-atlas sampling against actual near-stow hook OBJ finds
282 opaque points per skin, all 564 inside reviewed body-section hulls. A raw
transparent card corner lies outside, so full-card containment is not claimed.
The body/flame reference retains the existing static F-14 geometry repair;
independent guards separately check alignment, tail correction and new motion.
The new runtime preserves source scale, existing sweep/vapor mapping and donated
hook art. Body-section hulls are not full closed-volume collision proof, and
GPU appearance and retail comparison remain unvalidated.

## A-4E results

Exact `a4e-checked` passes all seven required controls, 18 focused tests and
all 202 gear, 202 hook and 202 brake samples, plus 25 flap/roll and 25 pitch/roll
combinations. All 17 control/device sheets, 22 selected rows from each dense
device sweep and both combined sheets were reviewed by the runtime and witness
owners. All 734 OBJ/PPM outputs are byte-identical to the first reviewed
geometry. The first pitch failure was a witness role-ordering mistake: it
incorrectly treated a moving right-tail panel as fixed. Explicit role lists
and a synthetic regression correct that mistake without changing valid geometry.

Maximum single-control attachment error is 0.000000779 feet and paired-skin
gap 0.000001006 feet. Minimum full main-card separation is 1.0010751 feet.
There are no newly introduced planar crossings. Independent source review
checks 642 opaque main texel centers per side and 4,488 complete nose assembly
triangle samples. The existing 26/3-foot clearance includes the lowest nose
wheel. Actual rigid wheel/brace assemblies, separate doors, brake linkages and
original stowed/deployed hook endpoints remain coherent through the sweeps.

Known limits remain explicit: forward flap-branch strips are unlocated as
continuous devices, lower flap-front position differs from the source down
endpoint by one source unit, and camouflaged door roots retain original
clearance up to 1.18254 source units. Mechanisms/timing are fitted. This pass
does not claim textured GPU, original-runtime or combined steering acceptance.
Static unreviewed device rows do not establish original absence.

## Fixed rotorcraft gear state

The actual loaded Apache and Chinook had visible fixed wheels but started with
simulation gear retracted. The correction keeps their gear down on creation,
commands and authoritative ticks. Focused tests passed for ignored toggle/set
retraction, repairing an inconsistent old state before gentle contact, and
continued Hind retraction/extension. The six-aircraft gentle vertical landing
and departure test passes without forcibly deploying fixed rotorcraft gear.
The Apache/Chinook animation battery now checks loaded state and commands
before selecting artificial mesh poses. The rule is explicitly fitted in the
[variety flight contract](../spec/variety-flight.md).

## Final validation

All 37 `flight-animation-*` scenarios pass in
`.local/animation-battery/20261005-204805-all-thirty-seven/summary.md`.
The focused suite reached 234 animation tests at the 36-profile checkpoint;
A-4E then added eighteen focused runtime/witness tests. The completed workspace
run passed 3,181 Rust tests, with 42 ignored. All 239 Python tests passed.
Workspace formatting, Clippy with warnings denied and build passed. Source and
app/extractor binary asset checks and documentation consistency passed.

The full run initially rejected two old aircraft-vertex hashes in the synthetic
combat scene. Reviewed Hornet/Rafale gear and surface changes explain those
outputs, including retained Hornet gear art at stow. Their two reference hashes
were refreshed; combat geometry, camera and ejection hashes remain unchanged.
The targeted rendering regression and subsequent full workspace run pass.

Combined-pose gates now explicitly reject nonfinite coordinates and normals;
synthetic NaN/infinity regressions protect them. Scenario stdout requires exact
identity and literal zero failures, so ten cannot match zero. Required combined
artifacts and their complete sample counts are checked separately. Fixed Apache
and Chinook gear state is verified before the diagnostic overrides mesh poses.

A wrapped A-4E GPU smoke run with half-deployed gear/flaps/brake/hook and mixed
controls presented and captured successfully. The capture
`.local/animation-audit/a4e-gpu-smoke.ppm` was inspected. It confirms presentation
and readback for that scene, not full textured acceptance of every aircraft or
pose. The optional Vulkan validation layer was unavailable on this host.
All windowed execution used `tools/agent-run.sh` on an isolated workspace.

These results are from Linux. Physical controller, human handling, LAN sessions,
other-platform runs, full mission/creator batteries and retail-runtime comparison
were not performed in this animation pass. Body-section containment is bounded
source evidence, not a full volumetric collision test. Explicit device/source
gaps in the per-aircraft contracts remain outside this acceptance.

## Fleet coverage

The code inventory covers all 37 playable profiles in the ignored local report
`.local/animation-audit/mapping-inventory.md`. At the start of this pass all 23
variety imports lacked pitch, roll and flap mappings. All 37 profiles have bounded geometry acceptance in the table below, with
explicit device gaps such as AC-130 hook geometry and helicopter blade feathering. Every profile has a baseline
motion survey followed by individual attachment and pose acceptance. Existing
transforms were not accepted merely because they were present.

Review order: A-7; F-4B/J/E/G; F-15/F-16/F-104/MiG-17/A-10; transports, AWACS and
airliners; VTOL/tiltrotor/helicopters; then the earlier fighter rigs. Shared shape
families may share source analysis, while each exact aircraft keeps its own
capability and result. The completed per-aircraft coverage is recorded below.

## Per-aircraft progress

This table records bounded geometry acceptance and its limits. It is not a
claim of complete original animation behavior or textured GPU acceptance.

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
| `il76` | Missing elevator, rudder, aileron, flaps | Independent flaps/roll and five rigid gear assemblies reviewed; mechanics fitted |
| `e2` | Missing elevator, rudder, aileron, flaps | Controls, rigid gear, hook, props and 50 combined poses reviewed; mechanics fitted |
| `b747` | Missing elevator, aileron, flaps | Controls, source flap topology/materials and five rigid gear assemblies reviewed; mechanics fitted |
| `a310` | Missing elevator, aileron, flaps | Focused geometry and topology accepted; mechanics fitted |
| `av8` | Missing elevator, rudder, aileron, flaps, vector-pitch | Controls, all central/outrigger gear and 25 nozzle combinations reviewed |
| `yak141` | Missing elevator, rudder, aileron, flaps, vector-pitch | Controls, asymmetric gear and 25 nozzle combinations reviewed; mechanics fitted |
| `v22` | Missing elevator, rudder, aileron, flaps | Controls, rigid gear and 50 flaperon/conversion combinations reviewed; mechanics fitted |
| `ah64` | Motion survey captured | Cyclic and rotor geometry reviewed; blade feathering unresolved |
| `mi24` | Motion survey captured | Cyclic, rotor and rigid gear reviewed; blade feathering unresolved |
| `ch47` | Motion survey captured | Cyclic, differential yaw and tandem separation reviewed; blade feathering unresolved |
| `f18` | Motion survey captured | Controls, own attachments and gear separation reviewed; mechanics fitted |
| `rafale` | Motion survey captured; hinges unreviewed | Controls, own attachments and gear separation reviewed; mechanics fitted |
| `f14` | Wrong roll sign and detached nose brace/panel | Controls, rigid gear, opaque hook stow and 150 coupled poses reviewed |
| `a4e` | Reversed roll, detached roots and shared device pivots | Controls, dense gear/hook/brake and coupled poses reviewed; forward-strip mapping unresolved |
| `x31` | Missing upper skins, wrong roll and detached gear attachments | Controls, complete gear and 25 prototype-vector poses reviewed |
| `mig29` | Motion survey captured; hinges unreviewed | Canted rudders, independent controls and separated aft-fold gear reviewed; mechanics fitted |
| `su27` | Motion survey captured; hinges unreviewed | Flaperons/slats, canted rudders and anchored gear brace reviewed; mechanics fitted |
| `mig21` | Motion survey captured; hinges unreviewed | Asymmetric hinges, thick brake and rigid gear reviewed; mechanics fitted |
| `su25` | Crossed gear, fixed fin moved, wrong rudder materials | Controls, exact signed materials and anchored rigid gear reviewed; mechanics fitted |
| `mig23` | Crossed gear and detached fitted surface roots | Controls, materials, rigid gear and 125 sweep/flap/roll poses reviewed |
| `su35` | Motion survey captured; hinges unreviewed | Own asymmetric controls, canards and anchored gear brace reviewed; mechanics fitted |
| `f22` | Detached source roots and gear doors | Controls, own rigid gear and coupled poses reviewed; bay geometry preserved |
| `f22n` | Detached source roots and gear doors | Controls, own rigid gear and coupled poses reviewed; bay geometry preserved |
| `faxx` | Detached source roots and gear doors | Controls, own rigid gear and coupled poses reviewed; bay geometry preserved |
