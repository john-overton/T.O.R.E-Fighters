# Quick Mission placement policy validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-26. The reference is merged `0.2.2-rc1`
commit `963962da`. This pass implements John's 2026-10-25 decisions in the
[creator specification](../spec/quick-mission-menu.md#keeping-the-enemy-on-the-map)
and [surface placement policy](../spec/surface-defenses.md#jitter).
The same isolated FA 1.02F import as the [initial audit](objectives-audit.md)
was used. No retail executable was run and no retail gameplay comparison is
claimed. The numerical footprint limits and bounded search are fitted agent
choices, not recovered retail constants.

Local evidence is in the main checkout's
`.local/next-round/0.2.2-rc1/objectives-audit/`, including `report.md`, complete
before/after TSV tables, immutable binary hashes and all source captures.

## Method

The complete surface matrix covers 108 offered target templates and one no-target
selection in each of 16 theaters, 16 AAA/SAM strength pairs, all eleven distance
choices and seeds 1 through 3: 65,472 cases per run. Separate full-world checks
attempt 4,092 setups with six five-aircraft wings, distinguishing explicit launch
refusals from accepted worlds. Accepted worlds exercise start placement, ground
starts, mission objectives and clipped CAP regions. Refused worlds do not count
as thirty successfully placed aircraft.

Use the [reusable audit command](../DEVELOPMENT.md#quick-mission-objective-placement-audit).
The reference full audit completed before the pause. The interrupted follow-up
is preserved separately; the final follow-up reruns the entire matrix and its
supplemental commands. Builds use locked dependencies and an optional opt-level 2
for `tore-formats`, without changing the committed build profiles. The resumed
validation uses priority 15, four Cargo workers, four test threads, two battery
workers and one wrapped window. Memory is checked before each heavy run, waiting
below 8 GB available. Heavy runs are sequential.

Audit geometry reuse requires equal complete normalized raw inputs and an exact
repeat of the final front pass against its integer obstacles at the current
separation. Starts are recomputed from the repaired target; final canonical
airfield ranks are independent of separation and retained from the built scene.
Equal raw fronts alone are not sufficient after airport replacement. The 264-case battery compares every case,
finding and digest with independent scene builds. A synthetic regression compares
27 seed/distance combinations and rejects reuse when a final obstacle blocks the
cached front.

## Results

| Measure | Before | After |
| --- | ---: | ---: |
| Surface cases | 65,472 | 65,472 |
| All surface finding rows | 781,286 | 464,794 |
| Counted contract failures (different slope policies) | 25,036 | 44 |
| Unresolved placement rows | 90,596 | 154 |
| Generated-support footprint slope | Not measured separately | 44 |
| Generated-support runway clearance | Not measured separately | 0 |
| Generated-support object clearance | Not measured separately | 0 |
| All-vehicle footprint slope | Not measured separately | 154 |
| All-vehicle runway clearance | Not measured separately | 4,180 |
| All-vehicle object clearance | Not measured separately | 99,000 |
| Stale objective anchors | 0 | 0 |
| Nondeterministic layouts | 0 | 0 |
| Nondeterministic final scenes | 0 | 0 |
| Stale final scene placements | Not measured separately | 0 |
| Runway datum mismatch | Not measured separately | 0 |

The start audit attempted all 4,092 setups: 3,796 built worlds, 266 explicit distance refusals and 30 explicit placement refusals. Accepted worlds had no start or CAP findings. Before, all 4,092 worlds built, with 11,241 CAP edge crossings and 265 distance mismatches. The new guard refuses invalid setups before flight.

Unit counts changed in 265 cases; parked-aircraft counts changed in 0; scene digests changed in 29,780. The complete per-case comparison is retained locally, including all unchanged-count cases whose positions changed.

The final surface audit built 8,038 scenes and reused 57,434 after validating
the final front and recomputing starts.

Grounding, airport, parked-aircraft and merged-airport audits completed with zero command failures. Their diagnostic counts and retained exceptions are recorded in the report and full logs.

The former strict support counter used the coarse cell slope rule. Its 25,036
failures were 27.6% of 90,596 unresolved-support rows, not all unresolved cases.
The new counter uses per-type footprints and separately checks runway clearance,
object clearance, terrain class and bounds. Unmeasured before categories are not
zero. Finding rows repeat objects across distances, seeds and strengths. Assembly
intersections and coarse shoreline mismatches remain review evidence, not a count
of independently confirmed defects.

The battery permits at most eleven generated-support footprint slope failures
in its 264 cases and zero in every other strict category. Improvements pass.
Any additional contract failure is rejected independently, so unused slope
allowance cannot hide a determinism, final-scene or runway-datum regression.

## Visual checks and remaining limitations

The 44 counted failures are added-radar footprint slopes in Falklands
`QLFCARG`, retained after the bounded search exhausts. Including authored SA-6 and
ASA-5 launchers, there are 154 unresolved-unit rows across 110 surface cases and
ten seed/strength combinations. Normal launch refuses these placements; no
resolved vehicle is deleted. The search does not promise a valid point beyond
its fitted 8,192-foot extent.

Total unit counts changed in 265 Kuril cases because whole-front and whole-site
generation can still fail: 45 target/front cases and 220 area-defense cases,
including their support units. Of the 37 lower-count cases, 26 have an explicit
missing-front audit note. The other eleven repeat the no-target AAA-heavy,
SAM-moderate, seed-1 setup at each distance: it places 28 of 32 rolled area units,
compared with 30 before, and the creator reports the shortfall. Other setups gain
units. The final-front pass retains any already-created group if its recheck
fails. Parked-aircraft counts do not change. The complete case table identifies
every count and digest change.

The full table retains 4,180 vehicle runway-clearance and 99,000 vehicle
object-clearance review rows. These repeat placements across settings and include
authored assembly/shoreline cases; they are distinct from the zero generated-
support clearance failures. The Spain shelter-entrance radar and Vietnam boat
remain as described in the visual review. An entrance access corridor and the
boat's intended rendered water boundary need further evidence before relocation.
Authored route terrain-class findings remain exactly 1,584 before and after, with
no added route slope or bounds findings.

The supplemental surveys covered 75 imported layouts, 1,028 airfields and 19,114
routes; 108 templates with 149 parked aircraft; and 84 merged fields. All returned
zero contract problems. The same six pavement-on-water observations remain:
four also have the retail runway line on water cells, two concern only the host's
pavement. Grounding covered all sixteen base theaters with zero unread resources
and zero problems. Compiled campaign missions are outside this Quick Mission pass.


All source images stay in the local audit directory, never in the repository.
The final scene contact sheet is `followup-captures-v4/comparison.png`; original
before/after top and oblique PNGs are retained alongside it. UI captures are in
`followup-ui-v2/`. They were inspected after the resume. The resumed code change
only tightens diagnostic cache validation and shares the unchanged front pass;
it does not change the captured gameplay placement policy.

- Kuril `QKSCFT`, radar `0x5c000000`: moved 4,125 ft from `(1486847, 1342629)`
  to flat ground at `(1482722, 1342629)`.
- Kuril `QKSILO`, truck `0x58000007`: moved from `(1550576, 1410026)` to
  `(1549732, 1411127)`. The remaining visible slope is within the fitted vehicle
  limit. This does not establish suspension or retail parity.
- Spain `QSPSAM`, radar `0x5c000001`: still at `(1243383, 1051294)` beside the
  hardened shelter entrance. It is reported, not claimed fixed. Whole-object
  clearance does not model an entrance access corridor.
- Vietnam `QTSAM`, boat `0x50000034`: unchanged at `(169600, 509623)` on the
  paddy-like painted terrain. The rendered discrepancy is confirmed, but the
  intended retail water boundary remains unknown. No automatic rendered-water
  classifier was added, and authored ships were not moved from coarse-grid
  evidence alone.
- The creator visibly offers 200 nm and disables 300 nm in the sampled Ukraine
  setup, reports four unavailable Yak-141s for Baltic `QBFLT`, and reports 30 of
  40 area-defense units in the sampled heavy Kuril setup.

The separate `menus-validate-creator` validator stops at the same imported Su-35
ammunition-transfer failure in the untouched reference and this follow-up:
`station transfer did not conserve imported ammunition`. Its later creator
matrix is not reached. That inherited ordnance problem was not changed or hidden.

## Development checks

The workspace run passed 7,186 Rust tests with 107 ignored. After the final
audit-only cache optimization, all 2,343 application tests passed again, with
49 ignored; the other crates were unchanged. Formatting,
all-target workspace clippy with warnings denied, and the locked workspace build
passed. The Python suite ran 515 tests, with five skipped and no failures. Asset
guards passed for the source tree and both built executables. Documentation
checks passed for 364 tracked Markdown files. The wrapped smoke test passed.

All 39 targeted surface and Quick Mission battery scenarios passed. Six window
scenarios were repeated successfully with an explicit priority-15 executable
wrapper, because Hyprland launches from the compositor rather than inheriting the
parent process priority. The 264-case audit subset returned eleven contract
failures in both modes, with identical 857 finding rows and all case digests.
The final cached run built 25 scenes and reused 239; the direct run built all 264.

The broader creator validator's inherited failure is recorded above. No full
battery or cross-platform runtime suite is claimed.

The stricter cache check corrected fourteen Kuril silo cases missed by the
pre-pause heuristic: two gain a valid six-unit front, and twelve place an existing
front differently and correct its notes. The changes add 22 finding rows. Every
corrected full-scene digest was independently rebuilt and matched; logs and
comparison data are in `resume-cache-direct-cases/`. The dedicated
`surface-objectives-cache-kuril` regression passed all 4,224 Kuril matrix cases
with zero contract failures and matched its two independent probe scenes. Its
battery result is `followup-battery/20261026-112044-objectives-kuril-cache/summary.md`.
This brings the targeted placement battery to forty passing scenarios, in
addition to the repeated window and cached/direct checks.

## Integration

Surface positions, ranked airfield lists and digests change. CAP checkpoints
encode an optional clipping rectangle. The creator preserves its checked seed
through the map and lobby. Invalid airborne distances and unresolved placements
are refused rather than silently shortened or omitted. Ground-start overrides
remain supported. No flight adapter default changes.

No protocol or replay version was bumped. The release lead must coordinate those
bumps with the other release branches. Existing network timer/handover fixtures
and the landing checkpoint fixture now use 150 nm, because their former 200 or
300 nm setups depended on shortening. Existing no-bounds AI goldens are unchanged.
Linux CPU checks and wrapped NVIDIA/Vulkan captures were exercised; Windows and
macOS runtime checks were not run.
