# Mirage 2000 acceptance record (M3, 2026-11-10)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The Mirage 2000 (`M2000.PT`) is a selectable retail identity in
`AircraftId::ALL` (the 40th entry) and `AircraftId::SELECTABLE` (the 41st),
appended at checkpoint code 40. It is not an alias for the Rafale, the Mirage
2000 desert record `M2000E.PT` or any other record: it has its own `M2000.PT`,
`M20.SH` and rig, and it names shared resources only where the record itself
names them (`VIS340.SEE`, `F18R.SEE`, `F18.ECM`, `DEFA.JT`, `F150.GAS`,
`AS15.JT`, `R530.JT`, `R550.JT` and `rafale.HUD`). The record is the Rafale
record with its own masses, thrust, fuel and envelope (27 of 219 statements
differ). John made five decisions about it between 2026-11-09 and 2026-11-10
(below). No retail comparison exists, so this record claims
source-reviewed, extraction-supported, headless-flight-supported and
rendered-flight-supported status, not retail parity.

## John's decisions

All five are opinionated. They are recorded as John's with their dates; every
other choice in this record is an agent decision and is labelled as one.

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 1 | 2026-11-09 | **HUD and cockpit follow the record**, as the game does: `rafale.HUD` resolves to `RAFALE.HUD` and the Rafale cockpit family (stem `RAF`, `~RAFH.PIC`, panel `~RAF_P.PIC`). Bound by name, never by aliasing the Rafale record. No Mirage cockpit art exists in any archive. | [HUD and cockpit](../formats/aircraft-variety.md#mirage-2000-sensors-hud-and-cockpit-m2k-7-2026-11-10) |
| 2 | 2026-11-09 | **One engine, one M53-P2's thrust.** The record says 2 engines and 17,687 / 32,000 lbf (the Rafale template's numbers, afterburner 49.6 percent over one real M53-P2). TORE runs 1 engine at 14,455 lbf dry and 21,380 lbf with afterburner (MILAVIA and aerospaceweb.org, accessed 2026-11-09). The override is one table entry in `aircraft_corrections.rs`; the catalog row and the decoder stay retail. | [Aircraft records](../spec/aircraft-records.md#mirage-2000-one-engine-one-m53-p2-2026-11-09) |
| 3 | 2026-11-09 | **Real size, scaled per axis** ("Per axis to real"), chosen over uniform scaling. Factors `[1.3616, 1.2290, 1.5053]` (right, forward, up) from the measured shape extents to 9.13 m span, 14.36 m length and 5.20 m height (fitted; cited sources disagree by a few percent). One per-aircraft scale table, default `[1, 1, 1]` for every other aircraft, pinned by a test. | [Aircraft shape scale](../formats/objects-and-shapes.md#aircraft-shape-scale-2026-11-09) |
| 4 | 2026-11-10 | **Belly pairs drop 0.1 ft into a rail** (the MiG-31's value), positions kept; **no big tanks on the belly row** (`F350.GAS` and `F500.GAS` are not offered on row 5 in normal loading); R530 and R550 on row 5 take the same drop. The wingtip part of this decision was superseded by decision 5. | [Mirage decisions](../spec/stores-on-pylons.md#the-mirage-2000-decisions-m2k-5-2026-11-10) |
| 5 | 2026-11-10 | **No wingtip station.** Row 8 is the Rafale template's wingtip rail; the real Mirage 2000 has nine store stations (five fuselage, two a wing) and no tip rail. TORE carries eight rows and no store on row 8. The decoded row stays documented as retail. | [No wingtip station](../spec/stores-on-pylons.md#no-mirage-2000-wingtip-station-m2k-5-2026-11-10-later-the-same-day) |

## Source identity and dependency closure

The source review is
[Mirage 2000 source review](../formats/aircraft-variety.md#mirage-2000-source-review-2026-11-09).
Retail media stays local; no extracted bytes are committed. The build pair is
the one every M3 review used:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| `M2000.PT` | `939c96b72012eeb090c73f4d371644b3649033095a5506f24a42689822c1bea9` |

The archive hashes were recomputed on 2026-11-10 (M2K-10) and match the earlier
reviews. The selected closure (the `FA_2.LIB` copies; the cockpit art is in
`FA_1.LIB`):

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `M20.SH` | Shape, 206 runtime faces, six state words | `78e2a93143c7bb5aeb80729a3c56613589e923aaf9080ec9012efdeb32b0f65f` |
| `M20_S.SH` | Shadow shape, 4 faces | `bab9ef07b37be6236168fbbac1b53a9b0643b25af3f5f0e04a8f3f406e97e656` |
| `M20_A.SH` | Damage body A | `b9b91f55c8bb2ca8b148c045b46013780a70d6d9437b50285b6f92ba23ca7829` |
| `M20_B.SH` | Damage piece B | `fcd58e3cb8188e9b799bc3fb113039c7c94f4ee784a86bdf6b61e28ba7a6b842` |
| `M20_C.SH` | Damage body C | `26f38ee4be850c22809ba89184e07b5cd82faad602138becad9ae1f5e2d758ca` |
| `M20_D.SH` | Damage piece D | `bb0740e2cc9f612777fd0dc1d4a9b78ea2b5b88d2b30e3cccca885731ba1e83e` |
| `F18R.SEE` | Radar, ADVANCED (shared with the F/A-18D) | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `F18.ECM` | Jammer, LATE, chaff 30 and flare 30 | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `VIS340.SEE` | Visual sensor | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `DEFA.JT` | Internal gun | `b4c787ef1322260d897393c0d0e7e1dfd93608723de06cd958553be18eb59764` |
| `F150.GAS` | Default centreline tank | `ca9054921aa93eb890801a5a1f64deb55c8bcc6d03854ff03f1b42833a9f4c95` |
| `AS15.JT` | Default belly store | `bbfdd179ee47b0c60f2676a0723d924e19b577f3f3a7520a53b8708ae06de7a5` |
| `R530.JT` | Default inner-wing store | `4b4b275d3f15e19d4aadf1d7c3aea3ea6f82ecd2af993cb870dd55bc86b8984d` |
| `R550.JT` | Default outer-wing store | `843ea3fa8c05e9ee8d86a01f7ea3a3d44cd8c315436ab83fb0bf4ca34a899ff8` |
| `RAFALE.HUD` | HUD and cockpit pointers (named by the record as `rafale.HUD`) | `b555f9febf6041297393c8d92e9e70343b1c5c9439316b1c0c1ec0721fe39228` |
| `~RAFH.PIC` | Forward cockpit (`FA_1.LIB`) | `e3dbd581e23afadfc669783693cdc6f0ddb30f3741344c5112ea5baa925b1444` |
| `~RAF_P.PIC` | Instrument panel (`FA_1.LIB`) | `be76aed99f87ba487c42e9a3424e7055e6f0a6922d485a65bce8e6e75088a678` |

The shape's `_M20.PIC` texture is in `FA_2.LIB`
(`0f298bf02366c417865a33a0dc1e857c7a56f910ee75efb2a52e1ec600b3c1b2`) and the
four damage looks name `_M20_A.PIC` to `_M20_D.PIC`
(`66b3ce62987524e0ad63d7adfcba6c36a276b5eee91cfc1d861c14e92f852a0d`,
`aa77e8772758f9ee479f363597a7e0093c0c64f1af9253c4ae0aba981a04b2a0`,
`86bc5b700c8cb493f9e225e563e3a9f7e9ca50425afc1196afc23f05825251dc` and
`8cf739c47028e8713786a7f46a83e26ebba52d2ebee646241f64e80b82b04b0c`), all present.
The PT names no `M2000.PTS` companion, so the absence is an explicit exception
and `has_pts()` returns false. The record's `typeSize` is 660 (219 statements);
the AI script `f.BI` and the radio callout `^ACM2000.5K` are present. The
`M20E.SH` family belongs to `M2000E.PT` and is not part of this identity. The
dependency report and extracted copies live only under
`.local/m3-imports/scratch/m2000/`. The app import command used for the profile
was:

```sh
TORE_DATA_DIR=.local/m3-aircraft/profile cargo run --locked -p tore-app -- \
  --import gameassets/fighters-anthology --import-only --no-audio
```

The selected CLI extraction was:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/m2000 --include 'M2000*' --include 'M20*' \
  --include 'RAFALE*' --include '*.HUD'
```

The CLI run found 241 resources and the usual five unrelated archive errors;
the app import reported no missing required reference. Extraction success alone
does not enable flight; the reviewed identity does. The parked path
(`("M2000.PT", Some(0x589c))` in `parked_aircraft.rs`) is unchanged and the
parked Mirage draws at the new size from the same shape (M2K-9).

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#mirage-2000-source-review-2026-11-09) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-m2000-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, HUD, cockpit and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `4b504564`.

### M2K-0 Source review (`df6814f8`)

Research mode. Decoded `M2000.PT` and every resource it names; found the record
to be the Rafale template (27 of 219 statements differ, `hudName` `rafale.HUD`);
mapped the six state words of `M20.SH` (`5890` plume, `5896` airbrake, `589c`
gear, `58a8` and `58ae` the flap words, `58b4` the rudder word); ran the rule 11
fold check over the neutral dump (every elevon, rudder, brake and gear face has
0 fixed vertices; no diagonal or partial surface); listed the HUD, engine-count
and delta-layout questions for John. Evidence:
[source review](../formats/aircraft-variety.md#mirage-2000-source-review-2026-11-09)
and `tools/check_docs.py` passing.

### M2K-1 Identity and record import (`81390037`)

`AircraftId::M2000` appended to the enum, `ALL` (40) and `SELECTABLE` (41), the
checkpoint code table (40), the parse keys `m2000` and `m2000.pt`, the help
texts, the live-fire list (DEFA, R530, R550, AS15), the strike class (Fighter,
agent decision), `stores_preview.rs`, the extraction key and the interim rig
spec. Decisions 1 and 2 applied: the Rafale HUD and cockpit by name, and the
engine override. Tests: `the_mirage_2000_runs_one_m53_p2_and_the_retail_values_still_decode_as_retail`
and `the_mirage_2000_reads_one_engine_and_one_m53_p2s_thrust_everyone_else_retail`
(a Rafale record with the same retail numbers stays 2 engines at 17,687 and
32,000 lbf). `--validate-ordnance` passed with the new pins. The `tore-app`
suite passed (2,425).

### M2K-2 Shape, real size and skin (`724f7840`)

Decision 3 applied: `tore_formats::aircraft::scale::shape_axis_scale(id)`, one
table, applied where shape units become feet (rendering, decals, damage looks,
rig pivots and hinge axes, station rows, gear contact, parked placement,
contrail and nozzle offsets). The measured `m20.SH` extents are 66, 115 and 34
units (right, forward, up) gear down. The skin has 206 runtime faces and 25 exact
twin groups with opposite stored normals (no MiG-21-style exception); the two
`E0` decal slots stay omitted. The `render-m2000-skin` scenario passes. Frames
show the Mirage about 0.9 of the Rafale's length and a top-view span to length
ratio of 0.65 (real 0.64).

### M2K-3 Elevons and rudder (`79654d12`)

Per side one angle for the inboard and outboard strip together, about the shared
mid axis (y -29, z -3): `-0.30 * elevator` plus a differential roll term of
`0.20 * aileron`, clamped to 0.50 rad (rule 12), plus 0.40 flap droop. The
rudder (quad and its two tip triangles as one panel) turns 0.35 rad. All fitted
agent decisions matching the fleet. Rule 11 fold table, vertices
total / turned / on hinge / fixed off axis: right inboard `3969`, `394a` 4/4/0/0;
right outboard `2176` 3/3/0/0, `2391`, `2190`, `2233` 3/2/1/0; left inboard
`3a31`, `3a50` 4/4/0/0; left outboard `1e87` 3/3/0/0, `23c3`, `1ea1`, `1f1b`
3/2/1/0; rudder quad `3bc6`, `3be5` 4/2/2/0; tip triangles `2b64`, `2b7c`
3/2/1/0. No face has a fixed vertex off its axis. Probe
`reviewed-controls-and-attachments`: 0 failures, largest inter-vertex change
1.3e-6 ft, `skin_gap_ft` 0 and anchor gap 0 on every sample, and 25 of 25
pitch-and-roll poses pass. Measured seam openings at the hinge (not gated):
0.150 ft at full pitch, 0.248 ft at full pitch plus roll, 0.184 ft for the
rudder.

### M2K-4 Gear, airbrake, burner (`113bfaf5`)

Word map: `5890` plume (8 faces), `5896` the one dorsal airbrake plate, `589c`
gear (mains `3876`, `3895`, `380b`, `382a`; nose `38e1`, `3900`); the flap and
rudder words are pinned and never set; no slat, canopy, nozzle, hook or door
word exists and none was invented. Gear: whole rigid plates, mains turning
inward about their top edge, the nose plate rearward and steering. Fold table:
right and left mains 4/2/2/0, nose 4/3/1/0, airbrake `3c73`, `3c92` 4/2/2/0,
plume 8 faces scaled from the exit plane, 0 fixed. Decision 2 drawn: one trail
and one nozzle glow (`engine_material` nozzle face `0x3794`, `outlet_count` 1).
Probe: 206 neutral faces, 0 failures, 203 dense gear lines passing (rigidity
7.9e-7 ft, ground contact at gear 1, lowest vertex the scaled 6.021 ft), 7 brake
sweep rows, 26 pitch-and-roll lines. `flight-animation-m2000`,
`flight-variety-m2000` and `render-m2000-skin` pass.

### M2K-5 Stations and pylons (`7970c45c`, `fd80c505`)

The first pass measured the decoded rows under the standard pylon rules and
stopped on clipping with frames, as John's standing rule requires. Decisions 4
and 5 then settled it. The station table below is the result.

| Row | Location | Mask | Default | Position (scaled ft right / up / fwd) | Result |
| ---: | --- | --- | --- | --- | --- |
| 3 | Internal gun | `0x000c` | `DEFA.JT` x250 | nose | |
| 4 | Centerline | `0x0605` | `F150.GAS` x1 | 0 / -7.53 / 0 | entry 0.09 ft |
| 5 | Fuselage (belly pairs) | `0x07e5` | `AS15.JT` x4 | 1.82 / -10.04 / 0 | tandem pairs at +4.05 and -4.05 on 2 inch rails, 0.1 ft rail drop; residual entry 0.46 to 0.65 ft (accepted by John); rear pair 0.52 ft from the main gear |
| 6 | Inner wing | `0x07e5` | `R530.JT` x2 | 4.99 / -10.04 / -4.10 | clean (0.42 ft clear) |
| 7 | Outer wing | `0x07e5` | `R550.JT` x2 | 9.08 / -10.04 / -8.19 | clean (0.18 ft clear) |
| 8 | Wingtip | | none | | retired (decision 5) |

Two parts of the rule set are Mirage specific and tested: `rail_drop`
(`M2000_BELLY_DROP` 0.1 ft in `placed_on`) and `aircraft_stations::retire`
(drops row 8 only while it is exactly the retail row and the last of nine, so no
index moves and every consumer, Cheat loading included, sees eight rows).
Audit pins moved on purpose with dated notes: 4,076 to 4,066 weapon
placements, 201 to 199 tank placements, 194 to 192 Cheat-only placements.
Stores-preview clip line for the default load: 0.57 ft (belly AS-15), 0.00 ft
on rows 6 and 7, 0.09 ft on the centreline tank. Nothing touches the gear or the
ground. Real-aircraft station facts (nine stations, Super 530 inboard, Magic
outboard) are from public pages for plausibility only.

### M2K-6 Ordnance, default load, countermeasures (`d697f0ce`)

No gameplay code changed: the record's rows, masks, capacities and defaults flow
through the shared loading rules. A retail-reading test pins the eight rows and
the `F18.ECM` counts. `--validate-ordnance` reports `M2000.PT: 5 source rows, 0
optional weapon rows, 174 compatible weapon placements, 7 tank placements;
defaults and restart passed; catalog 62 normal, 83 Cheat`. The default load
(27,007 lb against 37,480 lb): gun 250, one `F150.GAS` (990 lb), four `AS15.JT`,
two `R530.JT`, two `R550.JT`. Battery: `flight-livefire-m2000-slot1` to `slot4`
and `flight-jettison-m2000-slot2` to `slot4` (the AS-15 refuses the practice
aircraft, so slot 2 is a `REFUSED_SLOTS` entry as for the Rafale),
`a2g-release-origin-m2000`, `a2g-sarh-as15-m2000`, `a2g-jettison-m2000`,
`menus-ordnance-availability` and `a2g-catalog` all pass. Gun 250 shots, 62
hits, 1 kill. Each store leaves its own station, left first on a paired row.
Chaff and flares carry 30 and 30 (the `F18.ECM` record).

### M2K-7 Sensors, HUD, cockpit, instruments (`abd34d72`)

No game code changed; the bindings were wired in M2K-1. `--sensor-summary` reads
`M2000: radar F18R.SEE ... ADVANCED; infrared none; visual VIS340.SEE ...;
jammer F18.ECM LATE strength 0.30; signature radar 100 infrared 100`, equal to
the F/A-18D line. `--cockpit-sheet OUT M2000,RAFALE,F18` reports `m2000: stem
M20 cockpit ~RAFH.PIC art 1280x490 FITTED CLR` with the Rafale's mirror
rectangles. The ten instrument pages, the HUD in the glass and the front, side,
up, back (F2) and F3 views draw correctly. The Systems window is one fleet-wide
readout, so the single engine needs no change. Fuel reads `FUEL 6346 LBS (+ EXT
990 LBS)`. The cockpit eye point is the fleet's one fixed offset; measured
against the Mirage canopy it sits in the same place as against the Rafale's.
A new retail-reading test pins the HUD, stem and panel names.

### M2K-8 Flight model and validation (`8aaff294`)

The Mirage already flew the hybrid `--researched-flight` default on its own
configuration with decision 2's thrust; no flight code, record value or shared
constant changed. See the [flight envelope](#flight-envelope) and the
[open findings](#open-findings-for-john). New: the `performance_probe` example,
and the liftoff, liftoff-retail and approach scenarios. `variety_flight`:
`aircraft=M2000 conventional=pass deterministic=pass landing=pass ticks=1200`.
The flight battery family passed 16 of 16.

### M2K-9 AI, Quick Mission, multiplayer, replay (`6413bd19`)

No non-test game code changed. `--ai-roster-probe-ticks 3600`: the four
`M2000.PT` lines (Novice to Ace) `ticks=3600 models=2 radar=true ir=false
stores=4 gun=true projectiles=0 dropped=0 PASS`. `ai-m2000-quick-mission-2v2`
passes (two kills, nothing dropped); `menus-snap-quick-lobby-ordnance-laser-notes-m2000`
passes; `replay-rec-m2000-2v2` passes with two identical recordings and a clean
`--verify-render`; `net-content-missing` and `net-window-lobby` pass after
rebuilding `tore-server` (a stale server binary refused the bot with a
compatibility-code mismatch). The wire carries the identity as `SELECTABLE`
index 40 inside the 6-bit field. Parked check: `--surface-parked` lists
`M2000.PT` with the unchanged gear word and `M2000E.PT` with its own.

## Flight envelope

Measured on the default hybrid path (clean weight 23,446 lb with full internal
fuel; afterburner unless stated). Real figures are cited public pages for
comparison only and disagree among themselves; none is a flight manual.

| Quantity | Measured | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,197 kt (Mach 2.09) | 1,262 kt | 1,261 kn (Mach 2.2) |
| Top speed at sea level | 670 kt at 1,000 ft | 693 kt | 600 to 729 kn across pages |
| Level speed to edge ratio | 0.948 at every altitude | | |
| Ceiling | 64,422 ft zoom peak | 59,000 ft | 54,000 to 55,970 ft |
| Peak specific excess power | 418 to 435 ft/s (about 26,000 ft/min) | | climb 56,100 ft/min |
| Best sustained climb | 454 ft/s; 36,000 ft in 110 to 129 s | | |
| G limits | +7.1 / -3.5 clean; +6.6 / -3.3 default load | +8 / -4 | +9 / -3.2 |
| Instantaneous turn | 19.8 deg/s at 330 kt (6.2 G) | | not found |
| Fuel | 52.9 min dry, 6.6 min afterburner; 794 nm cruise | flows 2 and 16 lb/s | about 29 and 8.5 min; range 840 nm |
| Liftoff | 130.3 kt, 1,026 ft | | not found |
| Approach / touchdown | 145.2 / 132.6 kt, stopped | | approach 140 kt |

The hybrid default is the validated path; `--legacy-flight` (219.7 kt against
162.7 kt on the pull probe) and the restricted `--native-flight-tables` path stay
distinct and unchanged. Every air maneuver reported `non_finite=0`. The liftoff
(125 to 150 kt) and approach (140 kt plus or minus 5 percent) battery ranges are
fitted plausibility windows.

## Mirage 2000 positive G limit +9 (M2K-11, 2026-11-10)

Implementation and measurement mode. **Opinionated (John, 2026-11-10: "yeah go
ahead and make the correction")**: the Mirage's positive G limit is +9, the real
fly-by-wire limit, instead of the Rafale-template +8; the negative -4 stays.
The +9 polygon is **fitted** from the Mirage's own +8 row (slow-side speeds times
sqrt(9/8), fast side and altitudes from the +8 row, held to the 1 G top speed).
The override lives in `aircraft_corrections.rs` beside the engine override and is
applied where the record is read, only for `M2000.PT` while it still holds the
retail range and +8 row; the decoder and `fa-aircraft.csv` stay retail. Full
account, rows and measurements: [aircraft records](../spec/aircraft-records.md#mirage-2000-positive-g-limit-9-2026-11-10)
and [Mirage +9 G measurements](variety-flight.md#mirage-2000-positive-g-limit-9-2026-11-10).

| Fuel, lb | Pull at 461 kt, 1,000 ft, before | Now |
| ---: | ---: | ---: |
| 6,346 | 7.08 | 7.96 |
| 3,173 | 7.57 | 8.52 |
| 1,587 | 7.85 | 8.83 |
| 300 | 8.09 | 9.09 |

The M2K-8 flight table and the "Flight envelope" section above are the
pre-change measurement and are kept as dated.

## Open findings for John

Nothing here was changed; each is a decision or a fleet-level model question.

1. **Climb and acceleration are about 60 to 74 percent of the Rafale-thrust
   numbers and about 45 percent of the published 56,100 ft/min.** The cause is
   not thrust or weight but the shared drag law: drag equals the reference
   thrust at the top speed and grows with speed squared, and the model has no
   ram thrust. Matching the published climb would take about 2.2 times the
   M53-P2 (47,700 lbf), so decision 2 stands. Options: accept, or a
   per-aircraft drag shape or ram factor (a fleet-level model change).
2. **G limit +7.1 clean** is the record's +8 through the shared loaded-elevator
   rule (real +9; the negative side, 3.5, is close to the real 3.2).
   Update (M2K-11, 2026-11-10): John raised the positive limit to +9 (see
   [the +9 G section](#mirage-2000-positive-g-limit-9-m2k-11-2026-11-10)); the
   full-fuel pull is now 7.96 G.
3. **Fuel flows and tank retail.** Flows (2 and 16 lb/s) are the record's: dry
   45 percent under and afterburner 28 percent over the M53-P2's. Empty weight
   (17,100 lb) is 565 lb over the real 16,535 lb and the internal tank
   (6,346 lb) 624 lb under the real about 6,970 lb. All kept retail.
4. **Wingtip row retired** (decision 5). The row stays decoded and documented as
   retail; the Load Ordnance page shows five rows and no wingtip.
5. **Four AS-15s on the belly is the Rafale template's default**, kept from the
   record; it is not a published Mirage 2000 load. The AS-15 is a semi-active
   radar missile in TORE. The belly pairs still enter the belly skin by 0.46 to
   0.65 ft after the rail drop (accepted by John, 2026-11-10).
6. **Countermeasures are 30 chaff and 30 flares**, the retail `F18.ECM`. Public
   fit sources disagree (about 112 chaff cartridges for the Spirale, the flare
   count disputed). A Mirage-specific count would be the first per-aircraft ECM
   override.
7. **The cockpit eye point is shared.** `mirrors::pilot_eye` is one fixed offset
   (7 ft up, 10 ft ahead) for every aircraft, used by the rear mirrors, F2 Back
   and replay views. It sits in the same place against the Mirage canopy as
   against the Rafale's. A seated eye per aircraft would be a fleet change.
8. **The RWR threat frame is not made.** No scripted path plots an emitter or a
   missile on the player's RWR (the live-fire target emits no radar; the
   diagnostic incoming round plots at the centre). The frames show the window
   with JAM lit. The RWR code is aircraft-agnostic and unit-tested.
9. **Gun cadence.** The record's 250 rounds equal two real DEFA 554 (125 each),
   but the shared gun fields fire about 32 rounds a second against 40 to 60 for
   the real pair. Not Mirage specific.
10. **Signature 100 / 100** is the Mirage record's own (the Rafale's is 50 /
    70); it was read, not changed.
11. **No arrestor hook** (`hook_available=false`, from the record). Not compared
    with the real aircraft.
12. **A Mirage in live multiplayer flight was not played.** The net scenarios are
    aircraft-agnostic and exercise the identity list and content code only.

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extra on elevons).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 40; every named resource present (`M2000.PTS` an explicit exception); record audit done; no alias; import report clean; parked path unchanged |
| Look | Met | Own shape at the per-axis real-size scale (decision 3), damage looks A to D, `render-m2000-skin` passes, no show-through or clipped marking in the frames. The destroyed whole-body variant is unavailable, as for the MiG-25, F-16A and MiG-31 (no damage variant registered); the per-axis debris attachment has only the identity-scale path under a direct test |
| Whole-surface gate | Met | Every elevon strip, the rudder, the airbrake, the plume and the gear plates is a whole panel; the rule 11 fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity about 1e-6 ft in every pose |
| Animation | Met | Signed pitch, roll and rudder; flaps (a droop on the flap lever, since the shape has no flap surface); rigid gear with near-stow; airbrake and burner; neutral equals source; probe `reviewed-controls-and-attachments` 0 failures; the 25-pose pitch and roll grid passes with `skin_gap_ft=0`; `flight-animation-m2000` passes. No hook, canopy, nozzle or door word exists in the source |
| Stations | Met | Decoded rows measured under the standard pylon rules, clipping stopped and decided by John (decisions 4 and 5), preview audit clean apart from the accepted 0.46 to 0.65 ft belly entry |
| Ordnance | Met | Every default store permitted live and released from its own station; compatibility, even pairs, tanks, jettison and the AI default checked; 30 / 30 countermeasures from `F18.ECM`; `--validate-ordnance` passes for all 40 aircraft |
| Cockpit | Met | `RAFALE.HUD` and the Rafale cockpit by name (decision 1); aperture, overlays, mirrors and ten instrument pages checked; reviewed sensor presets only; absent systems absent. The RWR with a live threat was not demonstrated (open finding 8) |
| Flight | Met | Own configuration on the hybrid default; envelope and top speed reviewed; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally); the climb finding is open for John |
| AI and modes | Met | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby. A Mirage in live multiplayer flight was not played |
| Docs | Met | Source review, specs with provenance, this record, inventory, features and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: elevons | Met | Pitch, roll and the 25-pose grid pass with `skin_gap_ft=0` and rigid panels; every moving surface has a rule 11 face table above |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `M2K-2` to `M2K-9`
directories (skin and damage views, elevon and rudder poses, gear, airbrake and
burner stages, loaded stores and wingtip close-ups, ordnance and countermeasure
frames, cockpit, HUD and instrument pages, and the AI, Quick Mission, parked and
replay frames). Observed points: no underside show-through or clipped marking;
no half-moved panel; the gear tucks as rigid plates; the one dorsal airbrake
plate and the one afterburner plume draw; both wingtips are bare, the Magic hangs
under its pylon and the Super 530 under the inner pylon; the four AS-15s hang in
two tandem pairs; and the cockpit and Quick Mission pages name the exact Mirage
2000 identity. The frames are local evidence and are not committed.

## Full checks, 2026-11-10

Every command ran through `.local/m3-imports/light.sh` from the repository root,
and the windowed commands through `tools/agent-run.sh`. The logs are under
`.local/m3-aircraft/checks/M2K-10/`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passing |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passing |
| `cargo test --workspace --locked` | 7,513 passed, 0 failed, 108 ignored across 67 result lines (`net::standby` passed, no rule 13 rerun) |
| `cargo build --workspace --locked` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,241 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (376 tracked files; the new baseline carries the header and is checked when staged) |
| `tore-app --validate-ordnance --no-audio` (dev profile) | Passing: 40 aircraft, 4,066 weapon placements, 199 tank placements; 71 pod and 192 Cheat-only placements; `M2000.PT: 5 source rows, 174 compatible weapon placements, 7 tank placements` |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| Battery, render and animation: `render-m2000-skin`, `flight-animation-m2000` | 2 of 2 passed |
| Battery, flight and a2g: `*-m2000*` in the flight lane (live fire slots 1 to 4, jettison slots 2 to 4, liftoff, liftoff-retail, approach, variety, animation, `a2g-release-origin-m2000`, `a2g-sarh-as15-m2000`, `a2g-jettison-m2000`, `render-m2000-skin`) and `a2g-release-origin` | 17 of 17 passed |
| Battery, menus: `menus-snap-quick-lobby-ordnance-laser-notes-m2000`, `menus-ordnance-availability`, `a2g-catalog` | 3 of 3 passed |
| Battery, ai: `ai-m2000-quick-mission-2v2` | 1 of 1 passed |
| Battery, replay: `replay-rec-m2000-2v2` | 1 of 1 passed |
| Battery, net (after rebuilding `tore-server`): `net-content-missing`, `net-window-lobby` | each passed, run one at a time |

## Roster capacity finding

The wire codes an aircraft identity as its `AircraftId::SELECTABLE` index in 6
bits ([the wire format](../formats/net-protocol.md)), so the protocol can carry
64 identities at most. With the Mirage 2000 the list holds 41: 40 retail
aircraft and the F/A-XX concept. Only 23 more aircraft fit before a protocol
change. Porting the remaining decoded PT records needs a wider identity field
first; that is a protocol decision for John, not an aircraft-import decision.

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, Mirage in live multiplayer flight or performance
measurement was run. `--native-flight-tables` needs the extracted `sine-q15.bin`
and `atan-pa.bin`, which are not present locally, so it stays a separate
restricted path and was not run. The flight and rendered evidence above comes
from the Linux development host and the wrapped captures.
