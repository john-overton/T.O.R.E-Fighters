# Shared bomb flight validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation on Linux, 2026-10-15, branch nr-bombphys based on
3653df60. The flight constants are fitted agent choices in
[bomb motion](../spec/air-to-ground.md#bomb-motion), not measured retail parity.
The GBU-12/16/38 rows were measured on nr-bombs2, based on f805054a, using
`cargo test --locked -p tore-sim per_store_bomb_flight_measurements -- --nocapture`.
Their fitted definitions are in [smaller guided stores](../spec/missiles.md#smaller-guided-bomb-stores).
No GPU run or retail executable comparison was performed.

## Numerical store probe

Synthetic records use the reviewed weight, pack count and class metadata.
Every release is level at 20,000 ft and 450 kt. Guided numerical probes steer
to a point 24,000 ft ahead, bypassing launch-envelope gating; the valid live
release checks are separate below. Arrival is the first integrated tick below
the ground. Pack rows describe one body, with record weight divided by count.

| Store | Release kt | Arrival kt | Fall seconds | Travel ft |
| --- | ---: | ---: | ---: | ---: |
| CBU87.JT | 450 | 703.313 | 36.65000 | 25438.747 |
| CBU89.JT | 450 | 713.012 | 36.51667 | 25583.603 |
| FAB1000.JT | 450 | 741.075 | 36.15000 | 25987.873 |
| FAB250.JT | 450 | 706.122 | 36.60833 | 25479.818 |
| FAB500.JT | 450 | 724.637 | 36.35833 | 25752.278 |
| GBU10.JT | 450 | 710.658 | 33.82500 | 24002.733 |
| GBU10A.JT | 450 | 710.658 | 33.82500 | 24002.733 |
| GBU12.JT | 450 | 671.688 | 34.46667 | 24000.243 |
| GBU16.JT | 450 | 693.894 | 34.05833 | 24000.753 |
| GBU38.JT | 450 | 676.414 | 35.18333 | 24003.600 |
| GBU28.JT | 450 | 738.400 | 33.84167 | 24005.446 |
| GBU29.JT | 450 | 720.079 | 34.20000 | 24002.474 |
| GBU29P.JT | 450 | 720.079 | 34.20000 | 24002.474 |
| GBU30.JT | 450 | 700.292 | 34.62500 | 24000.905 |
| MK20.JT | 450 | 696.455 | 36.75000 | 25336.190 |
| MK82.JT | 450 | 703.313 | 36.65000 | 25438.747 |
| MK82AIR.JT | 450 | 165.313 | 71.45000 | 7899.712 |
| MK82HD.JT | 450 | 252.459 | 54.43333 | 12784.267 |
| MK82P.JT | 450 | 252.459 | 54.43333 | 12784.267 |
| MK82P3.JT | 450 | 703.313 | 36.65000 | 25438.747 |
| MK84.JT | 450 | 738.444 | 36.18333 | 25950.905 |
| PAVEWA3.JT | 450 | 720.079 | 34.20000 | 24002.474 |
| PAVEWAY.JT | 450 | 710.658 | 33.82500 | 24002.733 |
| RBK250.JT | 450 | 703.313 | 36.65000 | 25438.747 |
| RBK500.JT | 450 | 713.012 | 36.51667 | 25583.603 |
| ~BLDR.JT | 450 | 687.019 | 36.88333 | 25189.564 |
| ~MOOSE.JT | 450 | 687.019 | 36.88333 | 25189.564 |

A separate Mk 82 AIR body released at 500 ft and 450 kt arrives at 166.772 kt
in 6.69167 s, traveling 2,948.172 ft. The aircraft continuing level is about
2,134 ft ahead when the bomb lands.

## Sight and timer agreement

The live no-scatter test covers 45 combinations: 250, 450 and 650 kt;
800, 5,000 and 15,000 ft; level, 10/30/45 degree dives and a 10 degree climb.
Every impact is within one foot of the shared predictor. The level T's
higher/lower-ground checks retain their 30 ft target-contact bound and the
three required held-trigger tests retain their release-window and hit checks.
A separate integration matrix checks level and 30 degree dives at
500/5,000/20,000 ft, 250/450/600 kt and all three retarder areas. Predicted
terrain crossing is within one tick of the first below-ground simulation tick.

The recorded stationary-target checks use actual imported records and preserve
normal release scatter. Their guided release TTI measurements are:

| Case | Predicted seconds | Actual seconds | Error in ticks | Arrival kt | Last pre-impact horizontal miss |
| --- | ---: | ---: | ---: | ---: | ---: |
| GBU-10, level 20,000 ft, 450 kt, held T-bar | 31.754879 | 31.750000 | -0.585 | 713.801 | 8.9 ft |
| GBU-30, 12,000 ft, 450 kt, 98% of reach interval | 35.878796 | 35.866667 | -1.455 | 581.309 | 17.2 ft |

Both targets were destroyed. The endpoint is the designated object's height
when reached, or terrain for a miss. Changing a laser spot after release can
change the predicted time, which is refreshed in flight.

The recorded Mk 82 30 degree dive releases with its pipper 1.5 ft from the
selected point. Its scattered impact is 152.4 ft from that point. The low
Mk 82 AIR pack and both guided cases also complete their recorded passes.
CPU replay verification reports zero missing or differing transforms in all
four cases. These recordings do not suppress scatter; the no-scatter checks
above isolate sight accuracy.

## Cost and reproduction

One cold 20,000 ft guided reach query measured 3,538 microseconds in the
optimized test build; 120 horizontally translated queries with identical
flight state took 23 microseconds through the exact memo. This is a local
measurement, not a cross-machine performance guarantee. Changing launch state
can require a new fly-out. The search first narrows the release geometry, then
uses eight flight samples and up to eight refinements at each energy edge.

Unit tests live with combat ballistics, guided flight and live bomb tests.
`a2g-bomb-flight-*` records the four headless cases and verifies replay
transforms. Logs, recordings, the full run report and the GPU frame plan are
local-only under `.local/tmp-nr-bombphys/` and
`.local/next-round/reports/nr-bombphys.md`.

## Smaller-store recorded releases

On nr-bombs2 based on f805054a, the headless `a2g-bomb-flight-gbu12-level`
and `a2g-bomb-flight-gbu38-level` cases use an F-16C at 450 kt, a level
release at the shared T bar, and a truck-park target. Both destroy the target
and verify their recorded render transforms on the CPU with zero missing or
differing frames. These are game-model measurements, not retail comparisons.

| Store | Release altitude ft | Flight s | Arrival kt | Aim-point miss ft | Prediction error ticks |
| --- | ---: | ---: | ---: | ---: | ---: |
| GBU-12 | 20000 | 32.116667 | 676.736 | 8.2 | -0.519 |
| GBU-38 | 12000 | 29.533333 | 579.615 | 14.9 | -1.360 |

The battery also retains the four existing shared-flight cases and
`replay-a2g` for older-recording compatibility. The actual imported-store
carrier, catalog and persistence audit is documented in
[ordnance availability](ordnance-availability.md).
