# Mirage F1 acceptance record (M3 batch 5, 2026-11-11)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The Mirage F1 (`MF1.PT`) is a selectable retail identity in `AircraftId::ALL`
(the 45th entry) and `AircraftId::SELECTABLE` (the 46th), appended at checkpoint
code 45 and carried on the wire as `SELECTABLE` index 45 inside the 9-bit field
(NET-ID9, protocol 83). It is not an alias for the Rafale C (`RAFALE.PT`), the
Mirage 2000 (`M2000.PT`) or the Mirage III and 5 records (`MR3.PT`, `M5.PT`): it
has its own `MF1.PT`, `MF1.SH` and rig, and it names shared resources only where
the record itself names them (`VIS340.SEE`, `F18R.SEE`, `AV8.ECM`, `DEFA.JT`,
`F250.GAS`, `R550.JT` and `rafale.HUD`). `MF1.PT` stays a parked target as well:
the `parked_aircraft.rs` entry `("MF1.PT", Some(0x5adc))` did not change. The
record is the Mirage III's with new numbers, not one template: the thrust (15,000
and 30,000 lbf against one real Atar 9K-50), the weights and fuel (copied from
the Mirage III, 5 to 19 percent off), the four-tank belly row, the centreline
pod and the afterburner flow were copies or authored for another engine. John
decided the thrust, the cockpit, the tailplane, the centreline and the size on
2026-11-10, the belly pair and the tip on 2026-11-11, and the flight maxima and
fuel flows on 2026-11-11 (below). No retail comparison exists, so this record
claims source-reviewed, extraction-supported, headless-flight-supported and
rendered-flight-supported status, not retail parity.

## John's decisions

Every decision is opinionated and recorded as John's with its date; every other
choice in this record is an agent decision and is labelled as one. Numbers 23 to
27 are the batch decision round ("keep recs", `BATCH5-DECISIONS.md`); M1, M2, O4
and O5 are round 3 (`BATCH5-DECISIONS-3.md` "Answers", 2026-11-11).

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 23 | 2026-11-10 (round 1) | **Thrust: one real Atar 9K-50** (11,020 dry, 15,873 afterburner lbf), as the Mirage 2000. The record already says one engine. | [Aircraft records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10) |
| 24 | 2026-11-10 (round 1) | **Weights and fuel stay retail** (15,542 lb empty, 6,120 lb fuel, 30,203 lb maximum), as the Mirage 2000. | same |
| 25 | 2026-11-10 (round 1) | **HUD and cockpit follow the record**: `rafale.HUD`, stem `RAF`, `~RAFH.PIC`, the Rafale glass mask, as the Mirage 2000. No Mirage F1 cockpit art exists. | [Mirage F1 sensors, HUD and cockpit](../formats/aircraft-variety.md#mirage-f1-sensors-hud-and-cockpit-mf1-7-2026-11-10) |
| 26 | 2026-11-10 (round 1) | **Tailplane: pitch only** (no source for roll on the F1). | [Mirage F1 control surfaces](../formats/aircraft-control-surfaces.md#mirage-f1-control-surfaces-mf1-3-2026-11-10) |
| 27 | 2026-11-10 (round 1) | **Centreline: keep the station with no default pod** (no ALQ-167); **size: per-axis scale to the real span, length and height**, as the Mirage 2000. | [Stations](../spec/stores-on-pylons.md#the-mirage-f1-mf1-5-2026-11-10), [real size](../formats/objects-and-shapes.md#mirage-f1-shape-scale-mf1-2-2026-11-10) |
| R2 | 2026-11-10 (round 2) | **The per-axis span fit keeps the two wingtip rails included** (factors as MF1-2 committed: 1.0335, 1.1804, 1.0546). | same |
| M1 | 2026-11-11 (round 3, "follow recs") | **Belly tanks: the row is capped at two stores, one a side, and moved out to about 3.2 ft** (the measured clean slot, 3.24 ft, a fitted number). | [Stations finished](../spec/stores-on-pylons.md#the-mirage-f1-stations-finished-mf1-5b-2026-11-11) |
| M2 | 2026-11-11 (coordinator reading of "M1"; John may override) | **Wingtip Magic: a fitted tip offset** (1.1 ft outboard, the smallest that clears), never a wing pylon. | same |
| O4 | 2026-11-11 (round 3) | **The jammer stays as the record's** (`AV8.ECM`: chaff and flares, no radar jamming). | [Sensors](../formats/aircraft-variety.md#mirage-f1-sensors-hud-and-cockpit-mf1-7-2026-11-10) |
| B5-4 | 2026-11-11 (after round 3) | **"Correct all flight model maximums and fuel flows per realistic numbers that fit in TORE."** For the F1: afterburner flow 23 to 9 lb/s, a +8 row and a -4 row, the 66,125 ft ceiling kept (inside 65,600 to 66,000 real), top speeds kept. | [Flight maxima and fuel flows](../spec/aircraft-records.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11) |
| O5 | 2026-11-11 (round 3) | **AI: a separate pass after the batch** on why the F-5E never fires its AIM-9B and the related findings. No AI work here (AGENTS.md). | not here |

Not asked of John and decided by an agent (each labelled where it lives): the
belly and wing refusal lists and the 0.3 ft line (following his F-15E A2
answer), the `MK82.JT` retained-selection fallback, every fitted motion law and
hinge, the 1.1 ft tip offset, the 3.24 ft slot as a number, the battery windows,
the glass polygon and mirror seeds (none: the Rafale's are shared).

## Source identity and dependency closure

The source review is
[Mirage F1 source review](../formats/aircraft-variety.md#mirage-f1-source-review-2026-11-10).
Retail media stays local; no extracted bytes are committed. The archive hashes
were recomputed on 2026-11-11 (X-10) and match the earlier reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |

The selected closure (the `FA_2.LIB` copies, recomputed 2026-11-11 from the local
scratch extraction). Retail record values; B5-2 and B5-4 corrections and the
MF1-5b station defaults are applied where the record is read, never in the
decoder or the catalog:

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `MF1.PT` | Record, 12,659 bytes, `typeSize` 636, eight hardpoints | `ea10025dfe50ebeeb543e2173fb9a033404845183294c16485a3d38fd1537249` |
| `MF1.SH` | Shape, 236 runtime faces (gear word `0x5adc`), six state words | `a19501cfa37ad8b28315080ff97db2ec232cdb33a7cde9a109879e00367cbe41` |
| `MF1_S.SH` | Shadow shape | `77deb0f5d32e6e3f5f60d4ab9f212e5ec61c8c2f03f85b148d48261f33107d53` |
| `MF1_A.SH` | Damage body A (225 faces) | `21ec1c3ac526646a3c7f53d546044a5d1867c29a555c001f308b6f9c3970fa5d` |
| `MF1_B.SH` | Damage piece B (53 faces) | `fa4ab5d0d3ae5d3d2bc706dd875107dd64c8140b2783bcd3da44a8b94b140224` |
| `MF1_C.SH` | Damage body C (216 faces) | `dccf56cf4f8d22475cb2ccc50270d4e07f438c9a520e92e4dcf38eb488b50d54` |
| `MF1_D.SH` | Damage piece D (49 faces) | `02364ac9c89546e251fa2f294eca23140f4af9d2b64a0a18e6be0f6f87ae0c72` |
| `_MF1.PIC` | Shape texture | `85b82d8647d44965903cdbc6d3209ec983369eb39a16c0c8c316c055c21c6eee` |
| `VIS340.SEE` | Visual sensor | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `F18R.SEE` | Radar (the record's) | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `AV8.ECM` | Jammer suite row (kept, O4) | `90d9dff4344c2914a2b90f240de63f021a0e5b73cb6ea894a186986a916f6ce2` |
| `ALQ167.ECM` | Centreline pod named by the record, cleared by B5-2 | `cab8049aa941866fc906fcef385f19b78b721545f8b36ab7bdaa62f9563065a8` |
| `DEFA.JT` | Internal gun, 250 rounds | `b4c787ef1322260d897393c0d0e7e1dfd93608723de06cd958553be18eb59764` |
| `F250.GAS` | Default belly tank | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |
| `R550.JT` | Default wing and wingtip store | `843ea3fa8c05e9ee8d86a01f7ea3a3d44cd8c315436ab83fb0bf4ca34a899ff8` |
| `RAFALE.HUD` | HUD and cockpit pointers (named by the record as `rafale.HUD`) | `b555f9febf6041297393c8d92e9e70343b1c5c9439316b1c0c1ec0721fe39228` |

The AI script `f.BI` is present, and so is `^ACMF1.5K`. The PT names no
`MF1.PTS` companion (none exists), so the absence is an explicit exception, and
there is no Mirage F1 cockpit art (`~MF*`), so the Rafale family draws. The
encyclopedia (`MF1.INF`, `MF1_*.PIC` in `FA_3.LIB`) is reference media and was
read for numbers only. `MF1.PT` is also a parked target; its parked row (gear
word `0x5adc`, scale 0.3333, origin-up 5.98 ft, heading 89) is unchanged and
verified (MF1-2, MF1-4, MF1-9). The dependency report and extracted copies live
only under `.local/m3-imports/scratch/mf1/`. The app import reported no missing
required reference and the usual five unrelated archive errors.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#mirage-f1-source-review-2026-11-10) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-mf1-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, jammer (no RF mode), visual, HUD, cockpit and stores; no infrared sensor row; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `25a8c5b0`
(the aircraft branch `m3-mf1` was merged by `30418470`; its slice commits are
listed).

### MF1-0 Source review (`8ee4783b`)

Research mode. Decoded `MF1.PT` (238 fields) against all 145 PT records: it is
the Mirage III record with new numbers (differs from `MR3.PT` in 40 fields, from
`M2000.PT` in 50, from `RAFALE.PT` in 60). One engine already; thrust, weights,
fuel, hit points and signatures are copies; the G rows, ceiling and speeds are
the F1's own and agree with the encyclopedia. `typeSize` 636 needed a new arm.
Shape within 5 percent on span and height, 15 percent short in length (the nose).
Six state words `5ad0 5ad6 5adc 5ae8 5aee 5af4`; the fold check found every
moving surface a whole panel with 0 fixed vertices off the hinge. Row 4 is a real
centreline station that defaults to the US ALQ-167 pod; `AV8.ECM` is an internal
suite. No Mirage F1 cockpit art.

### B5-1 Identity (`cbd9ce1e`) and B5-2, B5-3 corrections (`179cac30`, `4ba24b77`)

B5-1 appended `AircraftId::Mf1` (checkpoint code 45, parse keys `mf1` and
`mf1.pt`, strike class Fighter by agent decision); `typeSize` 636 needed a new
arm in `aircraft.rs`. Interim rig spec 19,200 CODE bytes, 236 faces, gear word
`5adc` +6 faces. HUD, stem and picture bound by the record's name; the Rafale
glass mask applies. B5-2 applied decisions 23 and 27: thrust 11,020 and 15,873
lbf as `CORRECTIONS` rows (guarded on the retail 15,000 and 30,000) and the
centreline row's `ALQ167.ECM` default cleared (mask, count, class and position
kept). B5-3 had nothing for the F1. All guarded to the retail record and unit
tested; the audit pins moved once (a dated note each).

### MF1-2 Shape, real size and skin (`f7b92cde`)

Per-axis scale `[1.0335, 1.1804, 1.0546]` (decision 27, R2), fitted to the real
span 27.559 ft (8.40 m, rails included), length 49.967 ft (with the nose probe)
and height 14.764 ft: the scaled shape is within 0.05 percent of all three
(test). Every consumer (drawn aircraft, stores preview, damage looks, gear low
point 17 to 17.93 units, station rows, wingtip vapor, parked draw) reads the
scale through the existing generic path. 236 runtime faces (238 export), 23
exact twin groups, all with opposite stored normals (test); damage looks A (225
faces), B (53), C (216), D (49) with no registered damage variant (intact skin
with regional cuts). Two `E0` decal slots (`2587` left wing, `2d74` right wing)
stay omitted by the projector; the wing crop shows a whole skin with no hole.
`render-mf1-skin` passes.

### MF1-3 Control surfaces (`ac184a2a`)

Tailplane `-0.30 * elevator` both panels about a lateral spindle through the
root chord midpoint (+-5.5, -31.5, -1), no roll term (decision 26). Ailerons
`-/+0.20 * aileron` (outboard trailing-edge panels, 4 faces a side) about the
wedge leading-edge line. Rudder `0.35 * rudder` about (0,-30,3) to (0,-39,22).
All fitted agent decisions with fleet magnitudes. The flap-slot words `5ae8` and
`5aee` are pinned and never set: the sources list separate flaps and slats and
none puts a flap at the aileron position, so the flap lever moves nothing (the
probe lists flaps as not applicable and fails if the lever moves geometry).

| Panel | Vertices a face | Turned | On axis | Fixed off axis |
| --- | ---: | ---: | ---: | ---: |
| Tailplane (free flat plate a side) | 5 | all | 0 | 0 |
| Aileron quads | 4 | 4 | hinge line mid-thickness | 0 |
| Aileron wedges | 3 | 3 | hinge line mid-thickness | 0 |
| Rudder quad | 4 | 2 | 2 | 0 |

Probe: 236 neutral faces, 0 failures; the 25 pitch and roll poses pass; rigidity
about 1e-6 ft, skin gap 0, anchor gap about 2e-7 ft. Measured openings (not
gated): tailplane root 0.88 ft, aileron inboard seam 0.43 ft, rudder top 0.61 ft
at full travel.

### MF1-4 Gear, airbrake, burner (`6e9085ec`)

Every word is accounted for: `5ad0` plume, `5ad6` airbrake, `5adc` gear; `5ae8`,
`5aee`, `5af4` pinned (MF1-3). Mains turn 95 degrees forward and inward about a
point on their own top edge (axis 15 degrees off lateral, fitted so the plate
stows inside the hull within 0.1 unit); the nose plate turns 110 degrees
rearward and steers; gear 1 is the source pose, low point -17 units (5.98 ft).
The source's one dorsal airbrake plate (`4182`, `4199`) turns 0.707 rad about its
forward edge onto the ridge. Eight plume plates stretch along Y from the exit
plane y -42; one glow and one contrail (`engine_material` faces `2e92`, `2eb3`,
`outlet_count(Mf1)` 1).

| Part (faces) | Vertices | Turned | On hinge | Fixed off axis |
| --- | ---: | ---: | ---: | ---: |
| Left main `3b7e 3b9d` | 4 | 4 (one rigid body) | 0 | 0 |
| Right main `3be9 3c08` | 4 | 4 | 0 | 0 |
| Nose `3c54 3c73` | 4 | 3 | 1 | 0 |
| Airbrake `4182 4199` | 4 | 2 | 2 | 0 |
| Plume (8 plates) | stretched from y -42 | | exit plane held | 0 |

Probe: 202 dense gear rows pass (rigidity at most 1e-6 ft, ground contact at
gear 1 within 1e-3 ft, stow gate at 1e-6), 7 airbrake rows (closed free edge
3.5e-6 ft off the ridge). The main foot dips 0.08 ft in the first degrees of
travel (inclined axis; the gate allows 0.1 ft).

### MF1-5 and MF1-5b Stations and pylons (`0fa9479b`, `9b0a99bb`)

The first pass (`0fa9479b`) measured the rows under the standard pylon rules and
stopped on clipping, as the standing rule requires: four belly tanks crossing
each other and the main gear, the tip Magic 1.48 ft inside the tip. The finish
(`9b0a99bb`) applied M1 and M2. The final stations, scaled shape, ft:

| Row | Location (right / up / forward) | Default | Hang | Result |
| ---: | --- | --- | --- | --- |
| 0 to 3 | sensor, radar, `AV8.ECM`, gun | `VIS340`, `F18R`, `AV8.ECM`, `DEFA` x250 | internal | nothing drawn, no pod body (B7) |
| 4 | Centerline, 0 / -6.33 / 1.97 | empty (B5-2) | 2 inch rail | unchanged; one store; a centreline `F250` is 1.36 ft clear of the belly pair, 0.69 ft off the gear |
| 5 | Wing, two stores, **3.24 / record height / 0** (was four at 2.07) | `F250.GAS` x2 | 1 ft wing pylon, one a side | clean: no entry, 0 crossings, gear down 1.25 ft clear (was 0 and 24 / 20 crossings), ground 3.10 ft, no pair crossing (was 32) |
| 6 | Wing, paired, 7.23 / -3.52 / -1.18 | `R550.JT` x2 | 1 ft wing pylon | clean (left Magic 0.05 ft, 20 crossings, accepted) |
| 7 | Wingtip, paired, **tip edge + 1.1 ft out** / 0.70 / -4.33 | `R550.JT` x2 | tip rule on the shifted edge | clean: no entry, 0 crossings, 0.48 and 0.24 ft off the tip (was 1.48 and 1.45 ft, 66 and 56 crossings) |

The audit's `clip` line for hardpoint 5 reads 4.29 ft: a limit of its
straight-down metric (it finds the main-gear plate's low outer corner under the
tank's inner edge), not an entry; the triangle check finds none and 1.25 ft
clear. Hardpoint 7 reads 0.00 ft. The mechanism: `adjust_defaults` caps row 5 at
`maxItems` 2 under the whole-retail-table guard, `station_move` `(Mf1, 5)` Wing
3.24 ft, and a per-aircraft `tip_offset` on the tip edge the tip rule already
reads, so drawing, departing stores, replay and session records needed no change.
Row alternatives at the capped count: row 5 (65 records at 3.24 ft) 19 clean, 46
enter the airframe (13 by 0.3 ft or less, 33 by 0.37 to 3.39 ft) and **none
crosses the gear**; row 4 22 of 58 clean, 36 enter the belly, 15 cross the gear;
row 6 39 of 47 clean; row 7 9 of 10 clean (`PL7` has no crossing and reads 1.24
and 1.35 ft by the vertical metric, finding 4). Real stations (cited): Jane's
text in `MF1.INF` lists two 1,200 L inner-wing tanks, a 2,200 L belly tank, a
Magic or Sidewinder on the tip rails; the 3.24 ft slot has no source.

### MF1-6 Ordnance, default load, countermeasures (`084f0cf6`)

One rule added, no row, capacity or default authored: normal loading refuses 34
stores that enter the airframe by more than about 0.3 ft (agent decision
following John's F-15E A2 line; the F-5E kept its wing entries, his C2): 33 on
row 5 and `AS30.JT` on row 6 (a first version refused seven on row 6 and was
corrected in place the same day). Cheat loading skips the rule. `MF1.PT` 175
to 141 compatible weapon placements, catalog 62 to 60 normal (`AS16`, `GBU28`
only fit the belly row), Load Ordnance 181 to 147 choices. An empty belly row's
retained selection (it remembered the now refused `R550.JT`) falls back to
`MK82.JT`, as the F-15 and MiG-17 rows do. Default load: `DEFA` x250, empty
centreline, two `F250.GAS` (3,300 lb), two wing `R550`, two wingtip `R550`;
26,143 lb of 30,203 lb, the same for human, AI, saved loads, lobby and restart.
Release origins (`--a2g-run`): wing Magics 7.23 ft out, 1.12 ft down, depth
1.00 ft; tip Magics 14.88 ft out, 0.70 ft up, no pylon depth; a belly Mk 82 pair
3.24 ft out, 1.45 ft down, depth 1.00 ft. Countermeasures 30 chaff and 30 flares
from `AV8.ECM` (retail, no value authored). Live fire: gun 54 hits 1 kill,
wing and tip Magic one shot, hit, kill; jettison of the wing group (payload 4,481
to 3,892 lb) and the tip group; AGM-65G on the belly kills the lead truck. A new
retail-reading test pins the rows. The audit pin moved 4,652 to 4,618 here, then
to 4,610 at the merge (below).

### MF1-7 Sensors, HUD, cockpit, instruments (`f0d81df0`)

No behaviour changed; the bindings were wired in B5-1 and every sensor record
was already reviewed. Radar `F18R.SEE` Advanced (90 / 50 nmi), visual `VIS340.SEE`,
no infrared, jammer `AV8.ECM` Early **strength 0.00** (no RF mode, so no RF noise;
the one difference from the Mirage 2000's `F18.ECM`). The HUD draws inside the
Rafale glass mask (cockpit sheet centre x 461, y 24, w 360, h 79). One engine
readout, `FUEL 6120 LBS (+ EXT 3300 LBS)` with the current load. The eye point is
the fleet's one fixed offset, 4.9 ft above the F1's roof line (the Mirage 2000's
is 3.5, the Rafale's 4.0). Live fire `R-550 Magic ... LOCK`, `shots=1 hits=1
kills=1`. A new retail-reading test pins the sensor rows, the HUD equal to the
Rafale's and Mirage 2000's, and that no `~MF*` cockpit exists.

### MF1-8 Flight model and validation (`09fc6c43`)

Measurement only; no model, record or shared constant changed. Its figures were
taken on the four-tank load and before B5-4, so the
[flight envelope](#flight-envelope) below re-measures them. The flight family
passed 6 of 6; liftoff, liftoff-retail and approach windows were added (fitted
agent decisions, refitted by MF1-5b).

### MF1-9 AI, Quick Mission, multiplayer, replay (`395b2f93`)

No non-test game code changed. `--ai-roster-probe-ticks 3600`: four `MF1.PT` lines
`models=2 radar=true ir=false stores=5 gun=true PASS`. `ai-mf1-quick-mission-2v2`
(the AI fires its R550 twice on radar, one player kill, dropped=0, anomalies=0),
`replay-rec-mf1-2v2`, the menus lobby notes scenario, `net-content-missing` and
`net-window-lobby` pass; the Quick Mission test and
`the_mf1_round_trips_at_code_45_and_no_identity_shares_it` pin the list row and
the checkpoint code. The parked check (`--surface-parked`) shows two `MF1.PT` at
FRA `QFLAIR` and one at `QFSAIR`, unchanged.

### Batch corrections and the merge (`52c0178b`, `30418470`)

B5-4 (`52c0178b`) applied John's 2026-11-11 call to the F1: afterburner flow 23
to 9 lb/s, a fitted +8 row (from the +7 row by one rounding) and a fitted -4 row
(the +8 polygon), ceiling and top speeds kept. The merge (`30418470`, 37
conflicts, resolved append-only) took the pin to 4,610 weapon placements (4,644
less 34 refusals; `MF1.PT` 141) and ran the whole workspace: 7,644 passed, 0
failed.

## Final numbers

**Stations (MF1-5b).** Two belly tanks at 3.24 ft a side; the tip Magics 1.1 ft
off the tip; the centreline empty; the wing pair at the record's 7.23 ft. Table
above.

**Ordnance (MF1-6).** 34 refusals on normal loading (33 belly, `AS30` on the wing
pair); 141 compatible weapon placements and 6 tank placements for `MF1.PT`; the
audit total after the merge is 4,610 weapon placements.

**Corrections (B5-2, B5-4).**

| Field | Retail record | TORE | Real (cited) | Source of the change |
| --- | --- | --- | --- | --- |
| Thrust, lbf | 15,000 / 30,000 | 11,020 / 15,873 | one Atar 9K-50, about 11,000 / 15,900 | B5-2, John 23 |
| Afterburner flow, lb/s | 23 | 9 | 15,873 x 1.991 / 3,600 = 8.78 (Wikipedia TSFC table, Atar 09K-50) | B5-4 |
| Military flow, lb/s | 3 | 3 (kept) | 11,020 x 0.981 / 3,600 = 3.00 | already matched |
| G rows | -3 / +7 | **-4 / +8** (fitted rows) | +7.5 / -3.6 (MIGFlug "as commonly published") | B5-4 |
| Ceiling, ft | 66,125 | 66,125 (kept) | 65,600 to 66,000 | inside 1 percent |
| Top speed | 800 kt sea level, 1,345 kt at 36,000 ft | kept | 800 kt (Mach 1.2); 1,266 kt (Mach 2.2) | record is the F1's own |
| Weights, fuel | 15,542 / 6,120 / 30,203 lb | kept | Jane's text in `MF1.INF` gives 24,030 lb (the figure MF1-8 compared the 21,662 lb clean aircraft with) | decision 24 |
| Size | shape 15 percent short | real size, per axis | 27.6 by 50.0 by 14.8 ft | decision 27 |

## Flight envelope

Re-measured on **2026-11-11** (MF1-10), after the B5-4 flight changes and the
MF1-5b default load, on the default hybrid path with `performance_probe` and
`envelope_probe` (afterburner unless stated). The clean aircraft is 21,662 lb
with full internal fuel; the default load is 26,143 lb (two belly tanks, four
Magics, 250 rounds, carried 4,481 lb, 87 percent of the 30,203 lb maximum),
which the probe takes as `PROBE_PAYLOAD=4481` dead mass. The "MF1-8" column is
the 2026-11-11 morning measurement on the four-tank load (29,839 lb) and the
pre-B5-4 rows; values that did not move say so. Real figures are cited public
pages and the retail text; none is a flight manual.

| Quantity | Clean | Default load | MF1-8 clean / default load | Real |
| --- | --- | --- | --- | --- |
| Top speed 36,000 ft | 1,271 kt (Mach 2.22), unchanged | 1,225 kt (Mach 2.13) | 1,271 / 1,190 kt | Mach 2.2, 1,266 kt |
| Top speed 1,000 ft | 762 kt (Mach 1.16), unchanged | 734 kt (Mach 1.11) | 762 / 714 kt | 800 kt, Mach 1.2 |
| Level speed to edge ratio | 0.945 at every altitude | | same | |
| Ceiling | level to 66,000 ft (832 kt there, held); zoom climb 63,914 ft (MF1-8, not re-run) | not re-run | | 65,600 to 66,000 ft |
| Peak Ps / best sustained climb | 376 ft/s (22,600 ft/min) / 360 to 375 ft/s, unchanged | 298 ft/s | 376 / 252 ft/s | 41,930 (Jane's) or 47,800 ft/min (MIGFlug): **47 to 54 percent** |
| Acceleration 363 to 460 / 590 kt, 1,000 ft | 11 / 29 s, unchanged | 13 / 37 s | 11 / 29; 15 / 45 s | none |
| Pull at 461 kt, 1,000 ft by fuel 6,120 / 3,060 / 1,530 / 300 lb | **7.01 / 7.53 / 7.82 / 8.07 G** | **6.37 / 6.80 / 7.03 / 7.23 G** | 6.14 / 6.59 / 6.84 / 7.06; 5.18 / 5.50 / 5.68 / 5.83 | +7.5 |
| Push, same fuels | **-3.48 / -3.74 / -3.88 / -4.01 G** | **-3.16 / -3.38 / -3.49 / -3.59 G** | -2.61 to -3.00 | -3.6 |
| Instantaneous turn 461 kt | 7.01 G, 16.2 deg/s (was 6.14 G, 13.4) | 6.37 G, 14.4 deg/s | | none |
| Sustained turn 461 kt, 1,000 ft | 3.63 G, 8.1 deg/s, unchanged | 3.24 G, 7.4 deg/s | 3.63 / 2.93 G | none |
| Endurance on 6,120 lb | 34.0 min military, **11.3 min afterburner** (was 4.4), cruise 49.8 min | cruise 46.2 min | 34.0 / 4.4 / 49.8; cruise 43.5 | about 11.8 min afterburner |
| Liftoff | 103.9 kt, 211 m (MF1-8, not re-run) | **121.5 kt, 1,158 ft (353 m)** (battery run 20261111-122530) | 135.4 kt, 504 m on four tanks | ground run 600 m |
| Approach / touchdown | 115.9 kt (MF1-8, not re-run) | **127.6 / 118.7 kt** | 145.6 kt on four tanks | about 125 kt |

Logs: `.local/m3-aircraft/checks/MF1-10/` (`perf-mf1-clean.txt`,
`perf-mf1-default-load.txt`, `gby-mf1-*.txt` and `gby-mf1load-*.txt` for the
pulls, `envelope-mf1.txt`, `run_probes.sh`). What changed against MF1-8: the pull
and push rows (the +8 and -4 rows), the instantaneous turn, the afterburner
endurance, and every default-load figure (two tanks and a lighter load). The
clean top speeds, climb, acceleration, sustained turn, ceiling and cruise did not
move: B5-4 changed rows and a flow, not thrust, weight or the shared drag law.
The default-load afterburner climb schedule of the probe is an artefact of the
probe's dead-mass payload (at Mach 0.9 it cannot leave the ground, at Mach 0.8
it reports a best rate at 61,813 ft), as it was in MF1-8, so no loaded climb is
quoted. The clean ground runs (103.9 kt liftoff, 115.9 kt approach) are MF1-8's:
thrust and clean weight did not change since; the loaded values are today's.

The hybrid default is the validated path; `--legacy-flight` and the restricted
`--native-flight-tables` path stay distinct and unchanged. Every air maneuver in
MF1-8 reported `non_finite=0` (the scripted `dive` and `overspeed` end in an
overspeed loss); `--headless-flight 1200 --aircraft mf1` after MF1-5b and B5-4
reads `non_finite=0`, fuel 6,120 to 6,099 lb. The liftoff (125 to 150 kt, value
121.0 kt) and approach (125 to 150 kt, value 127.6 kt) battery windows are
fitted plausibility ranges (agent decisions); today's runs read 121.5 and 127.6
kt and pass.

## Open findings for John

Nothing here was changed by this slice; each is a decision or a fleet-level
model question. The first three are the ones John's answers so far leave open.

1. **The centreline row (row 4) is unmasked.** MF1-6 masked the belly pair and the
   wing pair but not the centreline: of its 58 alternatives, 22 are clean, **36
   enter the belly (0.05 to 0.71 ft) and 15 cross the main gear**. They load only
   through Load Ordnance, never by default. John has not been asked; the options
   are to mask them on the same 0.3 ft line (the F1 default load does not change),
   or to leave the centreline as the record has it.
2. **The F1's -3.6 G has a single secondary source.** The -4 row fits John's
   "real numbers" rule from one page (MIGFlug "Dassault Mirage F1", "as commonly
   published", read 2026-11-11); the +7.5 G comes from the same page. No second
   source was found. -4 pushes 3.74 G at half fuel (0.14 over); the retail -3
   pushed 2.80 G (0.8 under). If the source is rejected the fix is one guarded
   `CORRECTIONS` row.
3. **The shared drag law gives climb at about half of the published rate.** The
   clean peak sustained climb is 360 to 375 ft/s (21,600 to 22,500 ft/min)
   against 41,930 (Jane's text) or 47,800 ft/min (MIGFlug): 47 to 54 percent
   (the F-5E is 50 percent after its trim, the Mirage 2000 45). The cause is the
   shared drag rule (drag equals the reference thrust at the record's top speed
   and grows with speed squared, no ram thrust), not the F1's record. A
   fleet-level drag or ram fit is John's call; the F1's thrust is real.
4. **The loaded pull follows the shared loaded-elevator rule.** Full fuel and the
   default load pull 6.37 G (the +8 row's 8 G divided by the loaded-elevator
   factor); clean full fuel 7.01 G; the row is reached only nearly empty (8.07 G at
   300 lb, 0.57 G over the real +7.5, the half-fuel rule's known cost).
5. **Top speed at sea level is 762 kt, 38 under the real 800 kt** (5 percent) and
   at altitude is on the real figure. No change (inside the B5-4 tolerance).
6. **The default load is lighter but still 87 percent of maximum weight.** It
   runs two tanks; liftoff is 121 kt (3 percent under the fitted 125 to 150 kt
   window, covered by the 10 percent slack).
7. **Weights and fuel stay retail** (decision 24): the clean aircraft (21,662
   lb with full internal fuel) is 10 percent under the 24,030 lb figure in
   Jane's text, as MF1-8 recorded, and the 6,120 lb tank is the Mirage III's.
8. **The F1's AI fires its R550 on radar although there is no infrared sensor
   row** (`ir=false`), where the F-5E's AI never fires its AIM-9B. Noted for the
   AI pass (O5); no behaviour changed.
9. **Rig and look gaps.** The flap lever moves nothing (no flap panel in the
   source; the alternative is drooping the ailerons 0.40 with the lever, one
   constant, not applied); the real airbrakes are paired panels under the
   intakes and the shape has one dorsal plate; the airbrake closed pose leaves a
   sliver of the plate's outboard corner beside the spine at small brake values;
   the nose stow puts the plate top at z 5.9 under a roof at z 6; the aileron
   0.20 rad is small on a 4 to 6 unit chord; seam openings at full travel
   (tailplane root 0.88 ft, rudder top 0.61 ft); both wing decal slots are
   omitted, so no national marking draws on the wings.
10. **Station numbers with no source.** The inner pylon's lateral position
    (3.24 ft is a measurement of this shape, not a source); 46 row-5 alternatives
    enter the airframe (Load Ordnance only; 33 of them are refused, 13 at 0.3 ft
    or less are not); `PL7` on the tip reads 1.24 and 1.35 ft by the vertical
    metric though it has no crossing at 1.1 ft (1.4 ft clears it, not applied);
    the left wing-pair Magic is 0.05 ft in (accepted).
11. **The jammer is chaff and flares only** (O4, John kept it), unlike the
    Mirage 2000. Real countermeasure counts are not sourced (30 and 30 are the
    shared record's).
12. **HUD glass and mirrors are the Rafale's**; the eye point is the fleet's
    fixed offset, 4.9 ft above the F1's roof line. A seated eye per aircraft would
    be a fleet change.
13. **The RWR threat frame is not made.** No scripted path plots an emitter or
    missile on the RWR; the code is aircraft-agnostic and unit-tested.
14. **Gun cadence** (shared, about 32 rounds a second) was not compared; the real
    DEFA 553 rate was not sourced against it.
15. **A Mirage F1 in live multiplayer flight was not played.** The net scenarios
    are aircraft-agnostic.
16. **Frames index.** The frame pack is indexed in the local `INDEX.md`.

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extras on elevons, applied here to the
tailplane and ailerons).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 45 (`the_mf1_round_trips_at_code_45_and_no_identity_shares_it`); `typeSize` 636 handled; every named resource present (`MF1.PTS` an explicit exception); record audit done (copies listed and corrected by John); no alias of the Rafale, Mirage 2000, `MR3.PT` or `M5.PT`; the parked row `("MF1.PT", Some(0x5adc))` unchanged; import report clean |
| Look | Met | Own shape at real size (decision 27), damage looks A to D, `render-mf1-skin` passes (run 20261111-122530), no show-through or clipped marking in the frames. The destroyed whole-body variant is not registered; both wing decal slots stay omitted, so the wings carry no marking |
| Whole-surface gate | Met | Every tailplane, aileron, rudder, airbrake, gear and plume piece is a whole panel; the fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity about 1e-6 ft in every pose. The seam openings are inherent to rigid panels |
| Animation | Met | Signed pitch (pitch-only tailplane), roll, rudder; rigid gear with near-stow; the dorsal airbrake and one burner; neutral equals source; probe 0 failures; the 25-pose grid passes with `skin_gap_ft=0`; `flight-animation-mf1` passes. The flap lever moves nothing by design (flaps not applicable; finding 9); no hook, canopy or door word exists in the source |
| Stations | **Not fully met** | Default load and every default store clean under the standard pylon rules after John's M1 and M2 (belly pair at 3.24 ft, tip Magics 1.1 ft off the tip; the audit's 4.29 ft hardpoint 5 line is a metric artefact, finding in the station section). **The centreline row's alternatives are unmasked: 36 records enter the belly and 15 cross the gear (finding 1)**, which John has not decided. The left wing-pair Magic's 0.05 ft is accepted |
| Ordnance | Met | Every default store permitted live and released from its own station; 34 refusals; compatibility, even pairs, tanks, jettison and the AI default checked; 30 / 30 countermeasures from `AV8.ECM`; `--validate-ordnance` passes for all 45 aircraft (4,610 placements, `MF1.PT` 141 and 6 tank placements) |
| Cockpit | Met | `rafale.HUD` and the Rafale panel by name (decision 25); the HUD draws inside the shared glass; overlays and ten instrument pages checked; reviewed sensor presets only; absent systems absent (no infrared row). The RWR with a live threat was not demonstrated (finding 13) |
| Flight | Met with open findings | Own configuration on the hybrid default with John's corrections (Atar thrust, +8 and -4 rows, 9 lb/s, ceiling kept); envelope re-measured 2026-11-11; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally). Findings 2 and 3 are open for John |
| AI and modes | Met | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby. The AI fires its R550 (finding 8). An F1 in live multiplayer flight was not played (finding 15) |
| Docs | Met | Source review, specs with provenance, this record, inventory, features, counts and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: tailplane and ailerons | Met | Pitch, roll and the 25-pose grid pass with `skin_gap_ft=0` and rigid panels; every moving surface has a rule 11 face table above |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `MF1-2` to `MF1-9`
directories (`MF1-5b` and `MF1-6` for the final stations and ordnance): skin and
damage views, tailplane, aileron and rudder poses, gear, airbrake and burner
stages, the default load from five angles with the gear down and up, belly pair
and tip Magic close-ups, ordnance and countermeasure frames, cockpit, HUD and
instrument pages, and the AI, Quick Mission, parked and replay frames. Observed
points: the F1 reads as a Mirage F1 at its real size with the probe spike on the
radome and no show-through; no half-moved panel; the gear tucks as rigid plates;
the airbrake plate and one afterburner plume draw; the two belly tanks hang on
their pylons beside the fuselage with the wheel standing clear in the side crop;
the tip Magic stands off the thin tip on both sides; the cockpit shows the
Rafale panel with the HUD inside the glass; and the Quick Mission list names the
exact "Mirage F1" between the Mirage 2000 and the Rafale C. The frames are local
evidence and are not committed. The final stations were not re-captured in
MF1-10 (no presentation change).

## Full checks, 2026-11-11

Run on the merged `m3-aircraft` head `30418470` plus this slice's documentation
changes (no Rust or tool change in this slice). Every command ran through
`.local/m3-imports/light3.sh` from the repository root (John at the PC, nice 19,
4 jobs), one heavy command at a time, and the windowed commands through
`tools/agent-run.sh`. The logs are under `.local/m3-aircraft/checks/MF1-10/`.

| Command | Result |
| --- | --- |
| `cargo build --locked -p tore-app -p tore-server`, `-p tore-session --bin tore-bot` | Passing (rebuilt before the battery) |
| `cargo test --locked --workspace` | Not rerun: no Rust changed. The merge's run on this head stands: 7,644 passed, 0 failed, 108 ignored |
| `performance_probe` and `envelope_probe` for `MF1.PT`, clean and default load (`PROBE_PAYLOAD=4481`) | Figures in the flight envelope above; `run_probes.sh` and the logs in the check folder |
| Battery, Mirage F1: `--scenario '*-mf1*'` (render, animation, flight, a2g, ai, replay and menus lanes) | 19 of 19 passed (run 20261111-122530): a2g-jettison, a2g-maverick, a2g-release-origin, ai-mf1-quick-mission-2v2, flight-animation, flight-livefire slots 1 to 5, flight-jettison slots 2 and 3, flight-liftoff, flight-liftoff-retail, flight-approach, flight-variety, menus-snap-quick-lobby-ordnance-laser-notes-mf1, render-mf1-skin, replay-rec-mf1-2v2 |
| Battery, shared pins and net (after rebuilding `tore-server` and `tore-bot`): `menus-ordnance-availability`, `a2g-catalog`, `a2g-release-origin`, `net-content-missing`, `net-window-lobby` | 5 of 5 passed (run 20261111-122925), each run one at a time |
| `tore-app --validate-ordnance --no-audio` (`.local/m3-aircraft/profile`) | Passing: `MF1.PT: 5 source rows, 2 optional weapon rows, 141 compatible weapon placements, 6 tank placements; defaults and restart passed; catalog 60 normal, 83 Cheat`; `45 aircraft, 18 recovered weapon stations, 4610 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed`; 80 pod and 220 Cheat-only placements passed |
| `--surface-parked FRA QFLAIR --seconds 0` | Two `MF1.PT` (`0x50000011`, `0x50000012`): class `0x8000`, hp 95, gear `0x5adc`, scale 0.3333, origin-up 5.98, heading 89; the Mirage 2000 (`0x589c`) and Rafale (`0x5b62`) entries beside them unchanged |
| `cargo fmt --all -- --check` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` (and on `target/debug/tore-app`, `target/debug/tore-extract`) | 2,265 files; 1 file; 1 file |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| `python3 tools/check_docs.py` | Passing (380 files; the new baseline carries the header and is checked when staged) |

Clippy was not rerun: no Rust changed in this slice and the merge's clean run
on this head stands.

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, Mirage F1 in live multiplayer flight, `ai-fuzz` or the
full AI lane was run. `--native-flight-tables` needs the extracted
`sine-q15.bin` and `atan-pa.bin`, which are not present locally, so it stays a
separate restricted path and was not run. A loaded climb measurement and the
clean ground runs were not repeated (see the flight envelope). The wingtip and
belly stores were measured on the scaled shape's triangles; the real inner-pylon
position has no source. The flight and rendered evidence above comes from the
Linux development host and the wrapped captures.
