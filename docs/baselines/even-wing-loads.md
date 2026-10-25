# Even wing loads validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation validation, 2026-10-25, issue #6. This tests the
[even-wing-load contract](../spec/stores-on-pylons.md#even-wing-loads), following
John's final instruction to require complete pairs rather than alternate odd
extras between rows. [Source-row research](../formats/aircraft-ordnance.md#paired-point-counts-and-physics)
owns the retail evidence and build identity.

## Roster result

The imported roster audit found **551** changed aircraft/row/store maxima on
**24** retail aircraft: **468** change from 1 to 2 and **83** from 3 to 2.
**505** are supported weapons or tanks; **46** are imported setup-only weapon
definitions. Eligibility is unchanged: 3,411 supported weapon placements,
180 tank placements, 65 designator-pod placements and 170 Cheat-only novelty
placements remain. No default weapon or tank quantity changes on any of the
36 retail identities, for humans or AI wings. No Cheat maximum changes.

The Hornet wing rows 6 and 7 each have source maxItems 4 and a 4,000 lb
weight limit. AGM84A.JT weighs 1,165 lb, so the old weight-derived maximum
was 3 and is now 2. AS16.JT weighs 2,640 lb, giving an old maximum of 1,
now the requested minimum pair of 2. The latter is 5,280 lb, an explicit
1,280 lb exception to that old row limit. Aircraft takeoff limits still apply.
F16C.PT row 6 (2,700 lb class) is unchanged for AGM84A.JT, already 2
(2,700 / 1,165); its AGM84E.JT (1,380 lb) changes from 1 to 2.

Aircraft with no changed maxima: F22.PT, F22N.PT, C130.PT, AC130.PT,
E3.PT, IL76.PT, E2.PT, V22.PT, MI24.PT, CH47.PT, B747.PT and A310.PT.
The selectable F/A-XX was also audited with its own runtime identity and uses
its F22N donor's unchanged pair capacities. All 37 selectable identities pass.

## Complete changed maximum inventory

Rows are zero-based source ordinals, independent of the screen's display
order. Each store named in a cell changes by the indicated amount on that
row. `Setup only` entries remain unavailable for flight; they are included
so the audit does not mistake import presence for live support. All other
normal capacities, all single-point counts and every source record stay
unchanged. The local report also includes a tab-separated row per combination.

| Aircraft | Row | Old to new | Availability | Stores |
| --- | ---: | --- | --- | --- |
| A10.PT | 5 | 1 to 2 | Live | FAB1000.JT, MK82AIR.JT, MK82P.JT |
| A10.PT | 5 | 3 to 2 | Live | AS30.JT, B13.JT, FAB500.JT |
| A10.PT | 6 | 1 to 2 | Live | FAB1000.JT, MK82AIR.JT, MK82P.JT |
| A10.PT | 6 | 3 to 2 | Live | AS30.JT, B13.JT, FAB500.JT |
| A10.PT | 7 | 1 to 2 | Live | AGM84E.JT, AS14.JT, AS30.JT, B13.JT, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| A10.PT | 8 | 1 to 2 | Live | AGM84E.JT, AS14.JT, AS30.JT, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| A4E.PT | 5 | 1 to 2 | Live | F250.GAS, F350.GAS, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| A4E.PT | 5 | 3 to 2 | Live | GBU16.JT, GBU30.JT |
| A4E.PT | 6 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, B8.JT, CBU89.JT, FAB250.JT, GBU16.JT, GBU30.JT, RBK500.JT |
| A7.PT | 5 | 1 to 2 | Live | F350.GAS, F500.GAS, MK82AIR.JT, MK82P.JT |
| A7.PT | 6 | 3 to 2 | Live | F500.GAS, GBU28.JT |
| AH64.PT | 6 | 1 to 2 | Live | AA11.JT, AA11B.JT |
| AV8.PT | 5 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| AV8.PT | 5 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| AV8.PT | 5 | 3 to 2 | Live | AGM84A.JT, AS30.JT, B13.JT, F150.GAS, FAB500.JT |
| AV8.PT | 7 | 1 to 2 | Live | GBU10.JT, GBU10A.JT, PAVEWA3.JT, PAVEWAY.JT |
| F104.PT | 5 | 1 to 2 | Live | F150.GAS |
| F104.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT, PL10.JT |
| F104.PT | 7 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, AS7.JT, B8.JT, CBU89.JT, F150.GAS, FAB500.JT, GBU16.JT, GBU30.JT, R530.JT, RBK500.JT |
| F104.PT | 8 | 1 to 2 | Live | AGM84E.JT, AS14.JT, AS30.JT, F150.GAS, F250.GAS, GBU10.JT, GBU10A.JT, PAVEWA3.JT, PAVEWAY.JT |
| F14.PT | 6 | 1 to 2 | Live | AAML.JT, AS16.JT, F250.GAS, F350.GAS, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| F14.PT | 7 | 1 to 2 | Live | GBU12.JT |
| F15.PT | 5 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| F15.PT | 5 | 1 to 2 | Live | AGM45.JT, AGM84A.JT, AGM84E.JT, AGM88.JT, AS30.JT, B13.JT, B8.JT, F150.GAS, FAB500.JT, GBU16.JT, GBU30.JT |
| F15.PT | 5 | 3 to 2 | Live | AA12.JT, AIM7E.JT, MK20.JT |
| F16C.PT | 6 | 1 to 2 | Live | AGM84E.JT, AM39.JT, AS14.JT, AS16.JT, F250.GAS, F350.GAS, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| F16C.PT | 6 | 3 to 2 | Live | AGM45.JT, AGM88.JT, B8.JT, CBU89.JT, RBK500.JT |
| F16C.PT | 7 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, AS30.JT, CBU89.JT, FAB500.JT, GBU16.JT, GBU30.JT, RBK500.JT |
| F16C.PT | 8 | 1 to 2 | Live | AA11.JT, AA11B.JT |
| F16C.PT | 9 | 1 to 2 | Live | F500.GAS, GBU28.JT, MK82AIR.JT |
| F18.PT | 6 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| F18.PT | 6 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| F18.PT | 6 | 3 to 2 | Live | AGM84A.JT, AS30.JT, B13.JT, F150.GAS, FAB500.JT |
| F18.PT | 7 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| F18.PT | 7 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| F18.PT | 7 | 3 to 2 | Live | AGM84A.JT, AS30.JT, B13.JT, F150.GAS, FAB500.JT |
| F18.PT | 8 | 1 to 2 | Live | AA11.JT, AA11B.JT |
| F31.PT | 6 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| F31.PT | 6 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| F31.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| F31.PT | 7 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| F31.PT | 8 | 1 to 2 | Live | AA11.JT, AA11B.JT |
| F4.PT | 6 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| F4.PT | 6 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| F4.PT | 6 | 3 to 2 | Live | AGM84A.JT, B13.JT, F150.GAS, FAB500.JT |
| F4.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| F4.PT | 7 | 1 to 2 | Live | AGM84A.JT, AM39.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT |
| F4B.PT | 5 | 1 to 2 | Live | GBU28.JT |
| F4B.PT | 5 | 3 to 2 | Live | F350.GAS, FAB1000.JT |
| F4B.PT | 6 | 1 to 2 | Live | F500.GAS |
| F4E.PT | 6 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| F4E.PT | 6 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| F4E.PT | 6 | 3 to 2 | Live | AGM84A.JT, B13.JT, F150.GAS, FAB500.JT |
| F4E.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| F4E.PT | 7 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| F4J.PT | 6 | 1 to 2 | Live | GBU28.JT |
| F4J.PT | 6 | 3 to 2 | Live | F350.GAS, FAB1000.JT |
| F4J.PT | 7 | 1 to 2 | Live | F500.GAS |
| MIG17F.PT | 3 | 1 to 2 | Live | F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| MIG21.PT | 5 | 1 to 2 | Setup only | PL10.JT |
| MIG21.PT | 5 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, AIM7.JT, AS7.JT, B8.JT, CBU89.JT, FAB250.JT, GBU16.JT, GBU30.JT, R530.JT, RBK500.JT |
| MIG21.PT | 6 | 1 to 2 | Setup only | PL10.JT |
| MIG21.PT | 6 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, AIM7.JT, AS7.JT, B8.JT, CBU89.JT, FAB250.JT, GBU16.JT, GBU30.JT, R530.JT, RBK500.JT |
| MIG23.PT | 8 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| MIG23.PT | 8 | 1 to 2 | Live | AGM45.JT, AGM84A.JT, AGM84E.JT, AGM88.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, B8.JT, F150.GAS, FAB500.JT, GBU16.JT, GBU30.JT, MK82P3.JT |
| MIG29.PT | 5 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| MIG29.PT | 5 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| MIG29.PT | 6 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| MIG29.PT | 6 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| MIG29.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| MIG29.PT | 7 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| RAFALE.PT | 6 | 1 to 2 | Live | AS16.JT, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| RAFALE.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| RAFALE.PT | 7 | 1 to 2 | Live | AGM45.JT, AGM84A.JT, AGM84E.JT, AGM88.JT, AS30.JT, B13.JT, B8.JT, FAB500.JT, GBU16.JT, GBU30.JT |
| RAFALE.PT | 8 | 1 to 2 | Live | AA11.JT, AA11B.JT |
| SU25.PT | 7 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| SU25.PT | 7 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| SU25.PT | 7 | 3 to 2 | Live | AGM84A.JT, AS30.JT, B13.JT, F150.GAS, FAB500.JT |
| SU25.PT | 8 | 1 to 2 | Live | AS16.JT, F350.GAS, F500.GAS, FAB1000.JT, MK82AIR.JT, MK82P.JT |
| SU25.PT | 8 | 3 to 2 | Setup only | AA6.JT, AA9.JT |
| SU25.PT | 8 | 3 to 2 | Live | AGM84A.JT, AS30.JT, B13.JT, F150.GAS, FAB500.JT |
| SU27.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| SU27.PT | 7 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| SU27.PT | 7 | 3 to 2 | Setup only | PL10.JT |
| SU27.PT | 7 | 3 to 2 | Live | AA10.JT, AIM7.JT, AS7.JT, FAB250.JT, R27T.JT, R530.JT |
| SU27.PT | 8 | 1 to 2 | Setup only | PL10.JT |
| SU27.PT | 8 | 1 to 2 | Live | AGM45.JT, AGM65A.JT, AGM65G.JT, AGM88.JT, AIM7.JT, AS7.JT, B8.JT, CBU89.JT, FAB250.JT, GBU16.JT, GBU30.JT, R530.JT, RBK500.JT |
| SU35.PT | 7 | 1 to 2 | Setup only | AA6.JT, AA9.JT |
| SU35.PT | 7 | 1 to 2 | Live | AGM84A.JT, AGM84E.JT, AM39.JT, AS14.JT, AS30.JT, B13.JT, F150.GAS, F250.GAS, FAB500.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| SU35.PT | 7 | 3 to 2 | Setup only | PL10.JT |
| SU35.PT | 7 | 3 to 2 | Live | AA10.JT, AIM7.JT, AS7.JT, FAB250.JT, R27T.JT, R530.JT |
| YAK141.PT | 4 | 1 to 2 | Live | AGM84E.JT, AM39.JT, AS14.JT, F250.GAS, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |
| YAK141.PT | 5 | 1 to 2 | Live | AGM84E.JT, AM39.JT, AS14.JT, F250.GAS, FAB1000.JT, GBU10.JT, GBU10A.JT, GBU29.JT, GBU29P.JT, MK82P3.JT, MK84.JT, PAVEWA3.JT, PAVEWAY.JT |

## Validation scope

The roster audit checks maxima, pair-sized edits, saved odd-quantity migration
and strict multiplayer admission. Synthetic tests check singleton exclusions,
even maximum conversion, equal side masses, zero lateral mass moment, release
from the heavier remaining side of the selected point, total payload accounting,
transfers and the aircraft weight limit. Existing human/AI release, jettison,
checkpoint and replay tests retain the same per-point side rule.

Before/after GPU captures use the existing offscreen stores preview, viewed
from below, for F18.PT with two Harpoon points, F18.PT with Harpoon and Kickback,
and F16C.PT with Harpoons. Images and full logs remain in the local task scratch
folder. No retail artwork or generated images are committed.

Validation on Linux, Rust 1.91.1, `--locked`, eight build/test workers:

- Workspace tests: 7,156 passed, 106 existing ignored tests. Includes tore-sim
  (2,088 passed, 8 ignored) and tore-world (1,022 passed, 3 ignored).
- Workspace clippy with warnings denied, build and format check passed.
- Python tools: 500 passed and 5 skipped (505 run); battery family mapping passed.
- Documentation and source/app/extractor asset checks passed.
- Focused battery: 21/21 passed, with two jobs and one window. Includes
  `menus-ordnance-wing-pairs`, availability and derived-carrier audits,
  `qmc-wing-loadout`, all Hornet live-fire and jettison slots, empty/gun-only
  takeoff, F-22 wing release, unfired replay stores and F-15/F-16C/F-22A/F-22N
  replay release checks.
- Display smoke passed on the hidden agent workspace.
- The unchanged live/replay wire and checkpoint round-trip tests passed. No
  checksum golden or protocol/format version was changed.

Local evidence root: `.local/next-round/0.2.2-rc1/stores-balance/` in the main
checkout. `changed-maxima.tsv` lists one row per change. The battery report is
`battery/20261025-093442-stores-balance/summary.md`. GPU captures used the NVIDIA
RTX 4070 through `tools/agent-run.sh`; the task report lists the six below views.

Retail execution and retail visual comparison were unavailable. Per-wing CG
and drag torque are not implemented in T.O.R.E and are not claimed by this
validation.
