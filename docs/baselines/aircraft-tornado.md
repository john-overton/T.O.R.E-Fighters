# Tornado IDS acceptance record (M3 batch 5, 2026-11-11)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The Panavia Tornado IDS (`TOR.PT`) is a selectable retail identity in
`AircraftId::ALL` (the 44th entry) and `AircraftId::SELECTABLE` (the 45th; Update (TOR-13, 2026-11-12): beside it `AircraftId::TornadoClean`, the Clean version, a variant on this record; this `tornado` identity is the Combat version),
appended at checkpoint code 44 and carried on the wire as `SELECTABLE` index 44
inside the 9-bit field (NET-ID9, protocol 83). It is not an alias for the Su-24
(`SU24.PT`, the record it was copied from, which is not a roster aircraft), the
Mirage 2000, the Eurofighter (`E2000.PT`) or any other record: it has its own
`TOR.PT`, `TOR.SH` and rig, and it names shared resources only where the record
itself names them (`VIS240.SEE`, `F18R.SEE`, `MIG31.ECM`, `PAVESPK.SEE`, `BK27.JT`,
`F250.GAS`, `PAVEWAY.JT`, `MK82P3.JT`, `AIM9M.JT` and `e2000.HUD`). The record is the
Su-24's with eight numbers changed (TOR-0), not the Tornado's: the thrust, the
weights, the hit points, the G rows and the fuel flows were copies for another
aircraft. John decided the thrust, weights, G limit, fuel flows, draw scale, swing
wing, tailerons, HUD, cockpit, sensors and outer stations on 2026-11-10, the
stations, the tank, the glove plate and the Paveway fin tips on 2026-11-11, the
Sidewinder side rail on 2026-11-11, and the flight maxima on 2026-11-11 (below).
No retail comparison exists, so this record claims source-reviewed,
extraction-supported, headless-flight-supported and rendered-flight-supported
status, not retail parity.

The Tornado is the roster's only swing wing whose stores ride the sweep (the
MiG-23's wing also sweeps with speed, with no swivelling stations). The wing sweeps
from 25 to 67 degrees with speed, and the two inner wing pylons swivel with it,
carrying their stores. That is the part of this record most likely to differ from what a player
expects, so the stations, the sweep and the finding on the AI's stores have
their own tables below.

## John's decisions

Every decision is opinionated and recorded as John's with its date; every other
choice in this record is an agent decision and is labelled as one. The numbers
17 to 23b are the batch decision round ("keep recs", `BATCH5-DECISIONS.md`,
2026-11-10); D3 is round 2 (`BATCH5-DECISIONS-2.md`); T1 to T5 are round 3
(`BATCH5-DECISIONS-3.md` "Answers", 2026-11-11).

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 17 | 2026-11-10 (round 1) | **Copied values: real thrust** (two RB199 Mk 103, 17,300 dry and 32,150 lbf afterburner in all), **real weights** (31,065 lb empty, 61,620 lb maximum) **and +7.5 G** (held as whole-G rows, so +7 and, after B5-4, +8). | [Aircraft records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10) |
| 18 | 2026-11-10 (round 1) | **Draw scale: the header scale (true size)**, 2/3 ft a unit. | [Shape, scale and skin](../formats/aircraft-variety.md#tornado-shape-scale-and-skin-tor-2-2026-11-10) |
| 19 | 2026-11-10 (round 1) | **Swing wing: a fitted sweep rig with rotating pylons, as on the real aircraft.** The wing pylons swivel to stay aligned with the airflow as the wing sweeps, and stores on them follow. | [Control rig](../formats/aircraft-control-surfaces.md#tornado-control-rig-tor-3-2026-11-10), [stations](../spec/stores-on-pylons.md#the-tornado-ids-swivelling-wing-pylons-and-a-stop-on-clipping-tor-5-2026-11-10) |
| 20 | 2026-11-10 (round 1) | **Tailerons: whole panels with roll and a smaller limit** (0.25 rad against the fleet's 0.40); frames shown. | [Control rig](../formats/aircraft-control-surfaces.md#tornado-control-rig-tor-3-2026-11-10) |
| 21 | 2026-11-10 (round 1) | **HUD and cockpit follow the record**: Eurofighter `E2000.HUD`, stem `EF2`, `~EF2*`. No Tornado cockpit art exists. | [Sensors, HUD and cockpit](../formats/aircraft-variety.md#tornado-sensors-hud-and-cockpit-tor-7-2026-11-11) |
| 22 | 2026-11-10 (round 1) | **Radar, jammer and Pave Spike row stay as the record's**, no pod body drawn. | same |
| 23b | 2026-11-10 (round 1) | **Remove the outer wing stations, which the real aircraft reserves for pods**: the outer-wing row is retired so no store hangs there. Update (TOR-13, 2026-11-12): the Combat version carries pods on those two stations again, fixed and no weapon row; the Clean version keeps them empty. | [Stations](../formats/aircraft-variety.md#batch-5-record-and-station-corrections-b5-2-2026-11-10) |
| D3 | 2026-11-10 (round 2, "follow recs") | **Fuel flows corrected to two real RB199 Mk 103** (3 and 22 lb/s), replacing the Su-24's 6 and 38. | [Fuel flows](../spec/aircraft-records.md#batch-5-follow-up-corrections-tornado-fuel-flows-b5-3-2026-11-10) |
| T1 | 2026-11-11 (round 3) | **Row 7 moves onto the swinging wing panel** at a fitted position, so the swivel carries it. | [Stations finished](../spec/stores-on-pylons.md#the-tornado-ids-inner-wing-pair-on-the-wing-one-centreline-tank-a-glove-plate-tor-5b-2026-11-11) |
| T2 | 2026-11-11 (round 3, accepted after the stop) | **Row 6 Paveways stay where they are and the fin-tip entry is accepted** (0.96 ft front pair, 0.55 ft rear pair; the bodies clear and no outboard move clears the tips). | same |
| T3 | 2026-11-11 (round 3) | **Default load: one centreline tank**, not the pair that crossed the Paveways. | same |
| T4 | 2026-11-11 (round 3) | **A small fitted glove plate** fills the wing-root wedge as the wing sweeps. | same, [glove plate](../formats/aircraft-control-surfaces.md#tornado-glove-plate-tor-5b-2026-11-11) |
| T5 | 2026-11-11 (round 3) | **Tailerons keep 0.25 rad.** | [Control rig](../formats/aircraft-control-surfaces.md#tornado-control-rig-tor-3-2026-11-10) |
| Rail | 2026-11-11 (after TOR-6) | **"Lets do a custom side rail kind of like the f-14 for this station and put it on the inner [wing] rail like the RL Tornado."** An AIM-9 side rail on each inboard swivelling pylon. | [Side rails](../spec/stores-on-pylons.md#the-tornado-aim-9-side-rails-on-the-inboard-swivelling-pylons-tor-6b-2026-11-11) |
| TOR-11 | 2026-11-12 | **"I don't think for the tornado the wings should start sweeping back until it's about 350 mph."** The sweep starts at 304 kt (350 mph), was a fitted 250; the full sweep stays 600 kt. | [TOR-11](#tor-11-sweep-from-350-mph-and-the-inboard-aim-9-rail-2026-11-12) |
| TOR-13 | 2026-11-12 | **"Lets go two versions to TORE, 1 is the (Combat) which has the ECM and countermeasure pod on the outer wing pylons we removed earlier (not configurable), and the second is (Clean) which doesn't have them on the outer wing pylons today but doesn't have the jammer or chaff/flares."** Then: model the pods, a yellow triangle on the ECM pod's sides, chaff and flares out of the back of the countermeasures pod; Sky Shadow left, BOZ-107 right (John's photo). Update to 23b: the outer wing stations return, fixed, on the Combat version only. | [TOR-13](#tor-13-two-versions-combat-and-clean-2026-11-12) |
| TOR-11 | 2026-11-12 | **"The AIM-9 rail on the wing pylon is too long and on the wrong side."** The rail moves to the pylon's inboard flank and its drawn arm shortens from 5.6 to 3.0 ft. | [TOR-11](#tor-11-sweep-from-350-mph-and-the-inboard-aim-9-rail-2026-11-12) |
| B5-4 | 2026-11-11 (after round 3) | **"Correct all flight model maximums and fuel flows per realistic numbers that fit in TORE."** For the Tornado: a +8 row beside the +7 row, a 50,000 ft ceiling (record 60,000), the flows confirmed at 3 and 22 lb/s, top speeds kept. | [Flight maxima and fuel flows](../spec/aircraft-records.md#batch-5-flight-maxima-and-fuel-flows-b5-4-2026-11-11) |
| B5-4a | 2026-11-11 (after B5-4) | **"Tornado T2 accepted as is"**; the shared drag law is left alone. | not here |
| O5 | 2026-11-11 (round 3) | **AI: a separate pass after the batch** (the F-5E's AIM-9B and related findings). No AI work here (AGENTS.md). | not here |

Not asked of John and decided by an agent (each labelled where it lives): the
sweep speed schedule (250 to 600 kt; Update (TOR-11, 2026-11-12): the start is John's, 304 kt) and the pivot position, the taileron pitch
and roll weights, every fitted hinge and stow pose, the choice of which rows
swivel (7 and 8), the row 7 position (9.5 ft out, 1.5 ft forward), the glove
plate geometry, the 0.3 ft refusal line and its lists (following his F-15E A2
answer), the rail position (1.25 ft outboard of the pack; Update (TOR-11, 2026-11-12): inboard, John's call) and its refusals, the
Mk 82 `rounds` and live scenarios, the HUD glass polygon, the mirror seeds, the
battery windows, and the `same_stores` change in session recordings.

## Source identity and dependency closure

The source review is
[Tornado source review](../formats/aircraft-variety.md#tornado-source-review-2026-11-10).
Retail media stays local; no extracted bytes are committed. The archive hashes
were recomputed on 2026-11-11 (TOR-10) and match the earlier reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |

The selected closure (the `FA_2.LIB` copies, recomputed 2026-11-11 from the local
scratch extraction; the cockpit pictures `~EF2*` are in `FA_1.LIB`). Retail
record values; B5-2, B5-3 and B5-4 corrections and the TOR-5, TOR-6 and TOR-6b
station rows and defaults are applied where the record is read, never in the
decoder or the catalog:

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `TOR.PT` | Record, 12,201 bytes, `typeSize` 660, nine hardpoint rows | `39f9c677de59267bfcb87634a944433f7d659d47c1a850e9e699c1eb2d3f99fe` |
| `TOR.SH` | Shape, 287 runtime faces (313 gear down), header scale 2, six state words | `1fc8b434696f88fa8bb28da0c7350aeb3ea9ce2e48c345f54081d1a03824afdb` |
| `TOR_S.SH` | Shadow shape | `03a831378eeeb2841eda0240255deb629b50f1995341f28309afcc235aefa40d` |
| `TOR_A.SH` | Damage body A (290 faces) | `98bb65ae64beb19414903ff8add7b8c8a223906d2e2f67ac4d9f97dd3c004769` |
| `TOR_B.SH` | Damage piece B (36 faces) | `e570a281aed5a322b7ad78f6376165ac80460bad8be7968f45e9eaef53d9ded1` |
| `TOR_C.SH` | Damage body C (314 faces) | `39b5a1f9a3360ecfc9e920aff6913dd157fefb1c640d15524e4ab3424ea287fa` |
| `TOR_D.SH` | Damage piece D (37 faces) | `35756f4049894393eaadf53f6aebdea18dace1e85995dfb667f035ef5776a2f6` |
| `_TOR.PIC` | Shape texture | `5d6f20e9debca91487c99d0d7d1eca5d5c0a94ac69f8543071a5bd8116bc7599` |
| `VIS240.SEE` | Visual sensor | `a3224581abef75e0fe4e6dbb37d2d47d5ab4704bb1043e09dd6aa2e5a4510ee1` |
| `F18R.SEE` | Radar (the record's) | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `MIG31.ECM` | Jammer suite row (kept, decision 22) | `874f7b364ce6068341d6a17e10c4b73b37efcf86ec0d528a62f93e9c0296e56e` |
| `PAVESPK.SEE` | Fixed Pave Spike row (kept, no pod body) | `7fd9c6d3e909015e8715056e5c0f13ce472cdb7fb70ac10bdcd4a54cb6e53b91` |
| `BK27.JT` | Internal gun, 360 rounds | `0458077843fcc3c6252acfdb706f735cd7b26f80c0d36a74adbe80ab008a106f` |
| `F250.GAS` | Default centreline tank | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |
| `PAVEWAY.JT` | Default shoulder store | `28b92b6c21d129aa89b593b4bac1294d72ce87c6107826029d1235a5a32904df` |
| `MK82P3.JT` | Default inner wing store | `e61d8bc6f9ff372119cdca503dc3850c34730f0bcccd717521375a2ad9c73f6c` |
| `AIM9M.JT` | Default side rail store (the retail outer row's content) | `fdac769d40e374ed691a2758c80f4deddf44ecb136d8973374ea053ca03723cf` |
| `E2000.HUD` | HUD and cockpit pointers (named by the record as `e2000.HUD`) | `4ac570f7df6e65c7a00d372ba95ca7ccc8ddedf9361a3ee5051910e5e6738b5d` |
| `F.BI` | AI script | `8719314060695f82a0dbf905fc8169576a33f9147ac27db8ca2904fca467b30b` |

The PT names no `TOR.PTS` companion (none exists), so its absence is an explicit
exception. There is no Tornado cockpit art (`~TOR*`), so the Eurofighter family
`~EF2*` draws; no other aircraft names `E2000.HUD` or the `EF2` stem. `FA_2.LIB`
also holds `swpatch.lib`'s different `TOR.SH` (CODE 28,198), which the importer
never reads (it reads only `FA_1.LIB` and `FA_2.LIB`). The encyclopedia
(`TOR.INF`, `TOR_*.PIC` in `FA_3.LIB`) is reference media and was read for
numbers only. `TOR.PT` is not a parked target and has no parked row. The
dependency report and extracted copies live only under
`.local/m3-imports/scratch/tornado/`. The app import reported no missing required
reference and the usual five unrelated archive errors.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#tornado-source-review-2026-11-10) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-tornado-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, jammer (late, strength 0.30), visual, HUD, cockpit and stores; no infrared sensor row; the Pave Spike row gives the designator with no pod body; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `30418470`
(the aircraft branch `m3-tornado` was merged by `12021337`; its slice commits are
listed).

### TOR-0 Source review (`6d47298f`)

Research mode. Decoded `TOR.PT` against the Su-24: eight numeric statements
differ (year, `maxAlt`, internal fuel, the two structure values and three debris
values); weights, thrust, hit points and the G rows -3 / +6 are the Su-24's
copies. The shape's header scale is 2 (the MiG-23's class), so true size needs
2/3 ft a unit where the default 1/3 draws it half size. The wing is one fixed
spread pose with no sweep word. Surfaces: tailerons (one five-vertex polygon a
side), trailing strips, a rudder block, four airbrake faces and gear; no slats,
spoilers or reversers. `e2000.HUD` is the Eurofighter's HUD and no Tornado
cockpit exists. `F18R.SEE`, `MIG31.ECM` and the fixed `PAVESPK.SEE` row are the
Su-24's and not real for a Tornado. The template-copy list went to John, and his
answers are decisions 17 to 23b.

### B5-1 Identity (`cbd9ce1e`), B5-2 and B5-3 corrections (`179cac30`, `4ba24b77`)

B5-1 appended `AircraftId::Tornado` (checkpoint code 44, parse keys `tornado`,
`tor` and `tor.pt`, strike class Fighter by agent decision); `typeSize` 660 is the
default arm made explicit. Nothing drew `TOR.SH` at its header scale (the debris
scale function listed only the F-14, the MiG-23 and the transports), so the
Tornado joined the MiG-23 arm and the interim rig spec took 2/3. The default gun
`BK27.JT` could not load (`validate_for_editing` refuses a store outside
`SUPPORTED_WEAPONS`), so it is admitted as the station's own internal gun and
stays out of the catalog. B5-2 applied decisions 17 and 23b: thrust 17,300 and
32,150 lbf, empty 31,065 lb, maximum 61,620 lb and a fitted +7 G row (the +6 row
by sqrt(7 / 6) on the slow side), all `CORRECTIONS` rows guarded on the retail
values and unit tested; the outer-wing row 8 retired (the last row, so no index
shifts). B5-3 applied D3: flows 6 to 3 lb/s military and 38 to 22 afterburner
(thrust times the cited RB.199 specific fuel consumption 0.598 and 2.5, rounded to
whole pounds a second). The audit pins moved once per slice with a dated note each.

### TOR-2 Shape, scale, damage looks, decals (`50e74bc3`)

No production code changed: B5-1's scale entry was already right. The shape
reads at 2/3 ft a unit through every consumer (drawn aircraft, stores preview,
damage looks and thresholds, damage bodies, breakup offsets, gear low point 7.33
ft, contrail offsets, wingtip vapor). Measured 44.7 ft span (spread), 54.0 ft
long and 20.0 ft high against 45.6, 54.8 and 19.5 (-2.0, -1.5 and +2.6 percent).
287 runtime faces, 22 exact twin groups (21 with opposite normals; the sliver
pair `255c` and `257a`, a centreline sliver 1 unit wide, draws both members and is
invisible). Damage looks A to D load at the same scale; no whole-body damage
variant is registered, so a destroyed Tornado draws the intact skin with regional
cuts. Two `E0` decal slots (`2ce3`, `2e3d`, a 5 by 6 unit plate on the fin
centreline) stay omitted, so the fin draws as plain camouflage with no hole. A
retail-reading test (`tornado_shape.rs`) pins the numbers; `render-tornado-skin`
passes.

### TOR-3 Rig: swing wing and control surfaces (`2820fc8f`)

Sweep is a fitted speed schedule: spread to 250 kt, linear to fully swept at 600
kt, flaps hold the wing out; it draws only (no flight law reads it). The shape's
spread pose is the minimum sweep (25.5 degrees measured), so each wing panel
(13 whole faces a side, the trailing strips included) turns 0 to 42 degrees as
one rigid piece about a vertical axis at (9.5, -1.5) units right and (-10.5, -1.5)
left (the left glove is a unit further out). The pivot is the centre of the
smallest circle round the glove corners the panel shares (radius 2.55 units,
fitted); the real pivot position is not in any source. Tailerons `-0.20 *
elevator - side * 0.08 * aileron`, limit 0.25 rad, whole panels about the lateral
spindle through the root chord midpoint (6, -22.5, -2). Flaps: the strips stay
drawn and droop 0.40 rad about their leading edge, riding with the sweep. Rudder:
one rigid panel about the block's leading edge, 0.35 rad. The wingtip vapor
follows the swept tip.

Update (TOR-11, 2026-11-12): the wing is spread to 304 kt (350 mph, John), not 250;
the full sweep stays 600 kt ([TOR-11](#tor-11-sweep-from-350-mph-and-the-inboard-aim-9-rail-2026-11-12)).

Update (TOR-12, 2026-11-12): the pivot, the wing's place and the seam above are John's
Blender review now ([TOR-12](#tor-12-johns-wing-review-ported-2026-11-12)); the seam table
and the pivot sentences describe the TOR-3 measurement only.

| Sweep, degrees | Speed on the schedule, kt | Wing turn, degrees | Worst seam gap, ft |
| ---: | ---: | ---: | ---: |
| 25 | 250 or less | 0 | 0 |
| 35 | 333 | 10 | 0.30 |
| 45 | 417 | 20 | 0.59 |
| 55 | 500 | 30 | 0.88 |
| 60 | 542 | 35 | 1.02 |
| 67 | 600 or more | 42 | 1.22 |

The speeds are the schedule inverted (computed, not separate measurements); the
seam gap is the worst shared-vertex separation measured in the probe (`sweep-seam.csv`,
equal to the circle bound). Tip travel at full sweep is 11.9 ft. The free leading
root corner (9, 7) opens 2.7 to 5 ft at 67 degrees because the shape has no glove
ahead of y 1; TOR-5b's plate narrows that (below).

| Panel | Faces | Vertices turned | On the axis | Fixed off the axis |
| --- | ---: | ---: | ---: | ---: |
| Wing panel (a side, strips included) | 13 | all | 0 | 0 |
| Trailing strips | 4 | 3 each | 1 each | 0 |
| Tailerons | 4 | 5 each | 0 | 0 |
| Rudder | 2 | 3 | 2 | 0 |

Probe: 287 neutral faces, 0 required and check failures; rigidity at most 1.3e-6
ft, skin gap 0, anchor gap 0; the 25-pose pitch and roll grid passes at 25 and at
67 degrees and the 35-pose sweep and flap grid measures the turn within 0.01
degree of the law. Measured openings (not gated): taileron forward corner swings
2.41 ft at the limit, the flap seam against the glove fillets 0.77 ft at full
droop, the rudder's top aft corner 0.98 ft. TOR-3 also added the `--flight-speed
KNOTS` developer switch (captures a swing wing at a chosen sweep).

### TOR-4 Gear, airbrakes, burners (`ab886283`)

Every word is accounted for: `7dc0` plumes, `7dc6` airbrakes and `7dcc` gear
mapped here; `7dd8` and `7dde` flaps and `7de4` rudder as TOR-3. No hook, slat,
spoiler, canopy or reverser word exists. Gear: three rigid pieces (right main 10
faces, left main 10, nose 6), each turning `90 degrees * (1 - gear)` forward about
a lateral line at the leg's rear edge (mains y -1, z -2; nose y 25, z -3.5), with
the nose steering first; gear 1 is the source pose, low point z -11 units (7.33
ft). Direction cited from a secondary public summary ("retracts forwards"); the
per-leg detail has no source. Airbrakes: the word's four faces are two plates (one
a side, both windings), each turning `0.588 rad * (1 - brake)` about its forward
edge (y -7, z 2). Burners: eight plume faces (two crossed pairs, one per nozzle)
scale along Y from the exit plane y -24; `engine_material` gained the two nozzle
octagons (`3e90` left, `3eb7` right), `outlet_count` 2, so two glows and two
contrails replace the 15-percent-of-span fallback.

| Part (faces) | Vertices a face | Turned | On hinge | Fixed off axis |
| --- | ---: | ---: | ---: | ---: |
| Right main (10) | 4 | all | 0 | 0 |
| Left main (10) | 4 | all | 0 | 0 |
| Nose (6) | 4 | all | 0 | 0 |
| Right brake plate `57ec 5803` | 4 | 2 | 2 | 0 |
| Left brake plate `5889 58a0` | 4 | 2 | 2 | 0 |
| Plumes (8) | stretched from y -24 | all | exit plane held | 0 |

Probe: 202 dense gear rows pass (rigidity at most 2.2e-6 ft, anchor gap 0, skin
gap 0, 0 new crossings, lowest point 7.3333 ft at gear 1 and never lower while
closing), 7 brake rows, 7 exhaust rows. Known source asymmetries: the left main
foot corner overshoots the lower left skin by 0.17 ft at full stow (gate 0.2 ft)
and the closed left brake plate hovers up to 0.35 ft over the skin (reported, not
gated).

### TOR-5 and TOR-5b Stations and pylons (`d0225435`, `7b7dc614`)

The first pass (`d0225435`) wired the swivel: `Rack::turn` carries the swing
wing's turn, `side_pylons` rotates the pylon base about the wing pivot, so
`store_point`, `release_point`, the drawing and jettison agree, and the store
keeps the airframe's heading. The turn is a pure function of speed and flaps, set
each tick by the host and by the drawing, never checkpointed or recorded
(`turn` is a `skip` field). It then stopped on clipping, as the standing rule
requires: the retail row 7 is on the fuselage side inside the pivot (its Mk 82s
entered the airframe 2.61 ft at 25 degrees and hung in air at 67). The finish
(`7b7dc614`) applied round 3: row 7 onto the panel by `station_move` (9.5 ft
out, 1.5 ft forward, picked from a grid of 8.5, 9.5 and 10.5 ft out and -1, 1.5
and 3 ft forward; no source gives the real offset), one default centreline tank
by `aircraft_stations::default_counts` (capacity stays two), and four whole glove
plates (a plate a side, two windings, flat at z 1.1 ft above the wing, cloned look
from the glove's top triangles; addresses `0x7e00` to `0x7e03`) that fill the
wedge as the wing sweeps (plan-view gap beside the glove, square units, before
and after: right 8.8 to 3.4 at 46 degrees and 16.5 to 3.2 at 67; left 16.9 to 5.0
and 23.1 to 4.7; the spread-pose baseline is 1.8 right and 5.0 left, gaps in the
source model). T2 stopped on clipping again and John accepted it.

**Station table** (final, default load, TOR-6b; Update (TOR-12, 2026-11-12): rows 7 and 8 are now 7.96 and 6.71 ft out, 1.7 ft forward, and 8.77 and 7.52 ft out at 67 degrees, on both sides alike, see [TOR-12](#tor-12-johns-wing-review-ported-2026-11-12); Update (TOR-14, 2026-11-13): rows 7 and 8 are 0.38 ft aft at 25 degrees and 2.94 ft aft at 67 (7.38 and 6.13 ft out), see [TOR-14](#tor-14-pylons-centred-on-the-wing-one-size-2026-11-13); feet right / up / forward on the
right side; the rows in `TOR.PT` order):

| Row | Station | Default | Position | Hang | Result |
| ---: | --- | --- | --- | --- | --- |
| 0 to 3 | sensor, radar, jammer, Pave Spike row | `VIS240`, `F18R`, `MIG31.ECM`, `PAVESPK` | internal | nothing drawn, no pod body (B7) | n/a |
| 4 | gun | `BK27` x360 | internal | one muzzle, 2.33 / -1.67 / 18.33 | live fire hits (below) |
| 5 | Centreline, capacity two | `F250.GAS` x1 (1,650 lb of fuel) | 0 / -5.94 / 5.67, both sweeps | fixed fuselage pylon | 0.00 ft into the airframe, 0.46 ft above the ground gear down |
| 6 | Shoulder, four stores | `PAVEWAY.JT` x4 | 2.00 / -4.55 / -2.93 and 12.27, both sweeps | fixed fuselage pylon | **0.96 ft front pair, 0.55 ft rear pair: fin tips (John accepted, T2)**; bodies clear; 0.50 ft (front) and 0.86 ft (rear) above the ground |
| 7 | Inner wing pair, swivels | `MK82P3.JT` x2 (one three-bomb pack a side) | 25 degrees: 9.50 / -1.45 / 1.50; 67 degrees: right 10.36 / -1.45 / -1.26, left -10.53 / -1.45 / -0.81 | wing pylon on the swinging panel (pivot 6.33 ft out), 1.0 ft deep | **0.00 ft** into the airframe at both sweeps; 5.4 ft above the ground gear down; gear legs 4.4 ft (25) and 5.3 to 5.5 ft (67) away |
| 8 | Side rail on row 7's pylon, swivels | `AIM9M.JT` x2 (one a side) | 25 degrees: 10.75 / -0.74 / 1.50; 67 degrees: right 11.61 / -0.74 / -1.26, left -11.78 / -0.74 / -0.81, always 1.25 ft beside the pack | shoulder rail (the F-14's treatment) | **0.00 ft** into the airframe; 0.06 ft clear of the default pack by bounding box (AIM-9B 0.15, AA-2 0.19, AA-11 0.21, AA-8 0.34, AA-11B 0.06); 5.86 ft above the ground |

The retail outer-wing row (the outer pair) is retired (decision
23b); TOR-6b re-uses its index and content (`AIM9M.JT` x2) for the side rail, so
no row index moved and no saved load in a release names it. Only rows 7 and 8
swivel; rows 5 and 6 are fuselage pylons. The left and right pivots sit a unit
apart in the source, so the pack pair is 0.45 ft out of mirror fore and aft at 67
degrees. Alternatives measured at the final positions (`checks/TOR-5/matrix-final`,
52 runs): row 7 Mk 82, Mk 84, GBU-38, AGM-45, AGM-88, CBU-87, FAB-500, LAU-10 and
B-8 0.00 at both sweeps, GBU-30 0.00 and 1.10 ft; row 6 Mk 82, Mk 84 and two tanks
0.00; an all-heavy load (a 500 gallon tank, four Mk 84, two Mk 84) 0.00 and 0.03
ft into the ground with the gear down.

### TOR-6 and TOR-6b Ordnance, default load, side rails (`a5d63f86`, `12c35c7f`)

**Refusals (TOR-6, an agent decision applying John's F-15E A2 line, 2026-11-11).**
Every Load Ordnance catalog store on rows 5, 6 and 7 was measured alone against
the airframe (117 CPU stores-preview runs, row 7 at 25 and 67 degrees). Normal
loading refuses what enters the airframe by more than about 0.3 ft; Cheat skips
the rule. **Row 6, 33 stores:** `AA11` 0.39, `AA12` 0.35, `AA2` 0.48, `AA6` 1.01,
`AA9` 0.58, `AGM45` 0.93, `AGM84A` 0.47, `AGM84E` 0.46, `AGM88` 0.93, `AIM120`
0.37, `AIM7` 0.75, `AIM7E` 0.75, `AIM9B` 0.52, `AIM9M` 0.60, `AIM9X` 0.36, `AM39`
1.17, `AS14` 0.48, `AS15` 0.62, `AS30` 2.12, `AT12` 0.33, `AT2` 0.44, `GBU12` 0.90,
`GBU16` 0.98, `GBU28` 0.33, `GBU29`, `GBU29P`, `GBU30` and `PAVEWA3` (all the
`GB27.SH` body) 1.93, `MICA` 0.48, `PL10` 0.93, `PL7` 0.93, `R530` 0.62, `R550` 0.90.
**Row 7, 3 stores:** `GBU29`, `GBU29P`, `GBU30` 1.10 ft at 67 degrees (0.00 at 25).
Kept: the default `PAVEWAY.JT` and the two stores that hang the same shape
(`GBU10`, `GBU10A`, 0.96 ft, an agent decision: John accepted that shape), `AS7`
at exactly 0.30 ft, everything at or under 0.3 ft and every row 7 store but the
three. All four row 5 tanks read 0.00 (the 500 gallon tank 0.03 ft into the
ground gear down, under the line). Neighbour crossings (down to -1.64 ft) and the
bounding-box gear gap were not used as a rule. Code: `aircraft_stations::{
weapon_allowed, TORNADO_SHOULDER_REFUSED, TORNADO_INNER_WING_REFUSED}`; the audit
requires a refused store to have capacity 0.

**Default load (human, AI, saved, lobby, restart).** `BK27.JT` x360; `F250.GAS`
x1; `PAVEWAY.JT` x4; `MK82P3.JT` x2 (each press releases a three-bomb pack, 1,500
lb); `AIM9M.JT` x2 on the side rails; jammer `MIG31.ECM` 60 chaff and 60 flares as
retail. 13,228 lb with the tank's fuel, 55,514 of 61,620 lb on the Load Ordnance
page (before TOR-6b 12,848 lb; before TOR-5b 14,696 lb with two tanks).

**BK 27.** No gun code changed. Ballistics are the shared gun fields: muzzle
velocity 2,933 ft/s against about 3,609 real (19 percent low); 2 rounds a shot, 32
rounds a second against 33 to 57 for two real guns at 1,000 to 1,700 rpm; damage
`25/2/7/7/25`; red tracer. 360 rounds equals the real two guns of 180. Sight `LCOS
354 BK27 RADAR`, solved from the station's own mount. Live fire slot 1: 320 shots,
51 hits, 1 kill. One muzzle point (2.33 ft right, 1.67 ft below the origin, 18.33
ft forward) for two real guns.

**Side rail (TOR-6b, John's request).** `aircraft_stations::side_rail` appends row 8
after `retire` (Wing, mask 0x85, class 5, `AIM9M.JT` x2, guarded to the exact retail
eight rows). `station_move(Tornado, 8)` hangs it on row 7's pylon as a shoulder
rail (the F-14's `Hang::Shoulder`, 1.25 ft out), and `side_pylons` carries a
shoulder rail on a swivelling pylon by sweeping the pylon's point and adding the
offset after (turning the offset would have slid the rail about 0.8 ft aft at 67
degrees). The flank is **outboard**, the F-14's; which flank the real rail uses is
in no source (public pages say only one Sidewinder or ASRAAM a wing on the inboard
pylon; one mod note puts it "just to the inside", second hand, not used); the
inboard flank was not measured. The mask admits 11 catalog stores; normal loading
refuses four (`PL7` 0.50 ft (0.83 at 67), `R550` 0.40, `AT12` 0.44, `GBU12` 1.02)
and keeps seven (`AIM9B`, `AIM9M`, `AIM9X`, `AA2`, `AA8`, `AA11`, `AA11B`, all 0.00 ft
at both sweeps). The air-to-air page lists seven missiles (before TOR-6b only
`AA-11B` and `R-60M`).

Update (TOR-11, 2026-11-12): the flank is now **inboard**, John's call (the paragraph
above's "outboard" and "not measured" are the 2026-11-11 state); see
[TOR-11](#tor-11-sweep-from-350-mph-and-the-inboard-aim-9-rail-2026-11-12).

**Releases and jettison (all pass).** `--live-fire` slot 1 gun, slot 2 GBU-10 (one
shot), slot 3 Mk 82 pack (3 shots, 3 hits, 1 kill), slot 4 AIM-9M (one shot and one
hit at 200 kt and at 650 kt). `--a2g-run` release origins at 200 kt (25
degrees) and 650 kt (67 degrees): Paveways 2.00 ft out, 12.27 then -2.93 ft
forward, the same at both sweeps; Mk 82 packs 9.50 ft out, 1.50 ft forward at 25
degrees and left -10.53 ft out, 0.81 ft aft, right 10.36 ft out, 1.26 ft aft at 67;
rail missiles left then right from (-10.75, -0.74, 1.50) and (10.75, -0.74, 1.50)
at 25 degrees and (-11.78, -0.74, -0.81) and (11.61, -0.74, -1.26) at 67. Jettison
at both sweeps releases the tank from x 0, 5.94 ft down, 5.67 ft ahead, and the
packs and missiles from the same swivelled points. Countermeasures: five chaff
leave 55, three flares leave 57 (capacity 60 and 60). The audit pin moved 4,652 to
4,616 (TOR-6) and 4,623 (TOR-6b), 220 to 222 Cheat-only, and 4,581 and 222 at the
merge (below).

### TOR-7 Sensors, HUD, cockpit, instruments (`9603eea5`)

B5-1 had wired stem, picture, HUD and overlays, so every cockpit view already
drew the Eurofighter art; the HUD glass (an empty mask, so no HUD drew) and the
mirror seeds (three dead panels) were missing. The glass is one closed
transparent region inside the combiner frame: a flood fill gives exactly 73,769
pixels (x 475 to 803, y 105 to 364); the polygon runs on the opaque line pixels
every 16 rows (36 vertices, 0 leaked pixels, 852 lost at the chords). Mirror
seeds: centre (479, 11, 319 by 78), left and right (125 and 999, 183, 153 by 262),
flat (105, 113, 134), equal to the `~EF2_LH` and `~EF2_RH` picture sizes. Sensors:
radar `F18R.SEE` search 90 nmi, track 50, Advanced; infrared none; visual
`VIS240.SEE` 120 by 90 degrees; jammer `MIG31.ECM` Late, strength 0.30. Live fire:
gun `LCOS 354 BK27 RADAR`, target camera `TORNADO IDS`; the GBU-10 reads `ARM 1`,
`IN RNG`, `LSR 4.6` and `TTI 0:32` with the Pave Spike row designating, `NO LASER`
without. The Weapons window shows `360 BK27`, `4 GBU-10`, `2 MK-82P3`, `60 CHAFF`,
`60 FLARE` (before TOR-6b; now also the Sidewinders); the Systems window one
engine readout and `FUEL 11221 LBS (+ EXT ... LBS)`. Eye point: the fleet's fixed
offset (7 ft up, 10 ft ahead), 3.7 ft above the shape's top faces; the Tornado
shape has no canopy faces. TOR-7's note that no Sidewinder is offered, so no
infrared lock, is superseded by TOR-6b (an AIM-9M lock was proved in live fire,
slot 4).

### TOR-8 Flight model and validation (`f8808c2c`)

Measurement only: no model, record or shared constant changed; the liftoff,
approach and fuel windows in `flight.py` were fitted (liftoff 135 to 165 kt,
approach 140 to 175 kt, agent decisions; no primary source for either speed was
found). The figures were taken on the two-tank load (56,982 lb) and before B5-4,
so the [flight envelope](#flight-envelope) below re-measures them. The flight
family passed 6 of 6. TOR-8's findings for John (top speeds need no fit, the
record ceiling was 20 percent over the real 50,000 ft, climb meets the published
figures, no thrust reverser, no sweep term, retail row shapes at high and low
speed) are carried into the open findings.

### TOR-9 AI, Quick Mission, multiplayer, replay (`45c23d2f`)

No game behaviour was added; the Tornado already carried the fighter strike class
and its record's default load. `--ai-roster-probe-ticks 3600`: four `TOR.PT` lines
`models=2 radar=true ir=false stores=3 gun=true PASS`. One replay bug from TOR-5
was fixed in shared code: `Rack::turn` is set by the host and not recorded, so the
live-versus-replayed stores comparison counted it as a difference
(`verify-render` failed, `differing=2400`); `same_stores` in
`tore-session/src/recording/convert.rs` now ignores it (the drawing recomputes it
from the recorded speed and flap devices, which are still compared every tick),
after which `verify-render: PASS ticks=2401 missing=0 differing=0`. The AI wing
takes the record's class word `0x4000` (bomber), so an AI Tornado bombs by default
when the mission has a ground target and it is a friendly wing past the first
(`ROLE ... role=strike`, `releases=3 impacts=3 kills=1 egress=yes`); in a fighter
fight it fights with the gun. `ai-tornado-quick-mission-2v2`,
`replay-rec-tornado-2v2`, the menus lobby notes scenario, `net-content-missing`
and `net-window-lobby` pass; the Quick Mission test and
`the_tornado_round_trips_at_code_44_and_no_identity_shares_it` pin the list row
(`SU24.PT` is refused in the player and wing lists) and the checkpoint code. The
replay draws the sweep from the recorded devices (a unit test at 700, 425 and 700
kt with half flaps, and a real recording at 545 to 580 kt, 93 percent swept).

### Batch corrections and the merge (`52c0178b`, `12021337`)

B5-4 (`52c0178b`) applied John's 2026-11-11 call to the Tornado: a fitted +8 row
beside the +7 row (each from the +6 row, one rounding), and the ceiling 60,000 to
50,000 ft by clipping every polygon at the cap (the 0 G polygon at the record's
own step above it), so each speed band up to the ceiling is the record's own to 1
ft/s; the flows (3 and 22) were confirmed unchanged and the -3 row kept. The merge
(`12021337`, 41 conflicts, resolved append-only) took the pin to 4,581 weapon
placements (4,610 less 29; `TOR.PT` 85 to 56) and ran the whole workspace: 7,690
passed, 0 failed.

## Final numbers

**Stations.** The table above: one centreline tank, Paveways on the shoulders
(fin tips 0.96 and 0.55 ft in, accepted), Mk 82 packs on the swinging panel at 9.5
ft out, AIM-9M side rails 1.25 ft inboard of the packs (outboard until TOR-11, 2026-11-12),
the outer row retired, four glove plates.

**Sweep.** 25 to 67 degrees from 304 kt (350 mph, John 2026-11-12, TOR-11; 250 kt until
then) to 600 kt, flaps hold the wing out, a schedule that moves drawing and swivelling
stations only (no flight law).

**Ordnance (TOR-6, TOR-6b).** 33 refusals on row 6 and 3 on row 7 and 4 on row 8
on normal loading (40 in all); `TOR.PT` 56 compatible weapon placements and 8 tank
placements; catalog 36 normal, 83 Cheat; the audit total after the merge is 4,581
weapon placements.

**Corrections (B5-2, B5-3, B5-4).**

| Field | Retail record | TORE | Real (cited) | Source of the change |
| --- | --- | --- | --- | --- |
| Thrust, lbf (both engines) | 35,270 / 50,700 | 17,300 / 32,150 | 2 x RB199-34R Mk 103, 8,650 / 16,075 each (MILAVIA) | B5-2, John 17 |
| Empty weight, lb | 41,885 | 31,065 | 14,091 kg (MILAVIA, aerospaceweb) | B5-2, John 17 |
| Maximum take-off, lb | 90,390 | 61,620 | 27,961 kg (MILAVIA) | B5-2, John 17 |
| Military flow, lb/s | 6 | 3 | 17,300 x 0.598 / 3,600 = 2.87 (RB.199 TSFC, Wikipedia, jet-engine.net) | B5-3, John D3 |
| Afterburner flow, lb/s | 38 | 22 | 32,150 x 2.5 / 3,600 = 22.3 | B5-3, John D3 |
| G rows | -3 / +6 | -3 / **+8** (fitted +7 and +8 rows) | +7.5 / -3 (`TOR.INF`) | B5-2 (+7), B5-4 (+8) |
| Ceiling, ft | 60,000 | **50,000** | 50,000 (Wikipedia GR4, TOR-8 read) | B5-4 |
| Top speed | 806 kt sea level, 1,369 kt at 36,000 ft | kept (793 / 1,321 kt after the drag law) | Mach 1.2 (about 800 kt) and Mach 2.2 | record is the Tornado's own |
| Fuel, hit points, engines | 11,221 lb, the Su-24's hit points, 2 | kept | 11,221 lb is the record's own | n/a |
| Size | 2/3 ft a unit | header scale, true size | 45.6 ft spread span, 54.8 long, 19.5 high | decision 18 |

## Flight envelope

Re-measured on **2026-11-11** (TOR-10), after the B5-4 flight changes, TOR-5b's
single tank and TOR-6b's side-rail missiles, on the default hybrid path with
`performance_probe` and `envelope_probe` (afterburner unless stated). The clean
aircraft is 42,286 lb (31,065 empty plus 11,221 fuel); the default load is 55,514
lb (13,228 lb carried: one centreline tank with 1,650 lb of fuel, four Paveways,
two Mk 82 packs, two AIM-9Ms and 360 gun rounds, 90 percent of the 61,620 lb
maximum), which the probe takes as `PROBE_PAYLOAD=13228` dead mass. The "TOR-8"
column is the 2026-11-11 morning measurement on the two-tank load (56,982 lb,
`PROBE_PAYLOAD=14696`) and the pre-B5-4 rows; values that did not move say so.
Real figures are cited public pages and the retail text; none is a flight manual.

| Quantity | Clean | Default load | TOR-8 clean / default load | Real |
| --- | --- | --- | --- | --- |
| Top speed 36,000 ft | 1,321 kt (Mach 2.30), unchanged | 1,272 kt (Mach 2.22) | 1,321 / 1,266 kt | Mach 2.2 at 36,090 ft, 1,266 kt |
| Top speed 1,000 ft | 793 kt (Mach 1.20), unchanged | 763 kt (Mach 1.16) | 793 / 760 kt | about Mach 1.2 (Bundeswehr 1.3) |
| Level speed to edge ratio | 0.965 from 1,000 to 45,000 ft; **0.948 at the 50,000 ft ceiling (1,135 kt, was 1,156)** | | 0.965; 0.952 at 60,000 ft | |
| Ceiling | level flight holds to 50,000 ft (1,135 kt); afterburner zoom climb peaks at 57,910 ft (Mach 0.8 schedule) or 55,514 ft (Mach 0.9); **a level start at 60,000 ft cannot hold altitude (largest altitude change 20,084 ft, speed settles at 562 kt, Mach 0.98)** | zoom 55,414 / 55,170 ft; a level start at 60,000 ft changes 23,556 ft | level to 60,000 ft (837 kt, held); zoom 62,108 / 62,032 ft | 50,000 ft service ceiling |
| Peak Ps at 1,000 ft / best sustained climb | 406 ft/s / 423 to 433 ft/s (25,400 to 26,000 ft/min), unchanged | 294 ft/s / not quoted (probe artefact) | 406 / 285 ft/s; 423 to 433 / not quoted | 15,100 ft/min (GR4 quote); 30,000 ft in under 2 min from brake release |
| 30,000 ft in | 91 to 101 s, unchanged | 117 to 131 s | 91 to 101 / 120 to 134 s | under 2 min |
| Acceleration 363 to 590 kt, 1,000 ft | 26 s, unchanged | 36 s | 26 / 38 s | none |
| Pull at 461 kt, 1,000 ft by fuel 11,221 / 5,610 / 2,805 / 300 lb | **7.05 / 7.52 / 7.79 / 8.04 G** | **6.13 / 6.49 / 6.68 / 6.87 G** | 6.17 / 6.59 / 6.82 / 7.04; 5.29 / 5.59 / 5.76 / 5.91 | +7.5 |
| Push, same fuels | -2.63 / -2.81 / -2.91 / -3.01 G, unchanged | -2.29 / -2.42 / -2.50 / -2.57 G | -2.63 to -3.01; same | -3 |
| Instantaneous turn 461 kt | 7.05 G, 15.7 deg/s (was 6.17 G, 13.1) | 6.13 G, 13.2 deg/s (was 5.29 G, 10.8) | | none |
| Sustained turn 461 kt, 1,000 ft | 4.81 G, 10.2 deg/s, unchanged | 4.14 G, 9.3 deg/s | 4.82 G, 10.2 / 4.14 G, 9.4 | none |
| Endurance on 11,221 lb | 62.3 min military, 8.5 min afterburner, cruise 73.7 min, unchanged | cruise 68.4 min | same | none |
| Liftoff | not run (a ground start always carries the default load) | **147.2 kt after 1,805 ft (550 m), rotation 129.0 kt** (`flight-liftoff-tornado`, run 20261111-140235); 112.3 kt after 1,034 ft with `--retail-stall-speeds` | 150.6 kt after 1,945 ft (593 m) | take-off run 900 m |
| Approach / touchdown / rollout | not run | **156.2 kt approach, 155.3 kt touchdown, rollout 2,380 ft (725 m)** (`flight-approach-tornado`) | 160.1 kt; rollout 757 m | landing run 370 m with reversers |

Logs: `.local/m3-aircraft/checks/TOR-10/` (`perf-tornado-clean.txt`,
`perf-tornado-default-load.txt`, `gby-tornado-*.txt` and `gby-tornadoload-*.txt`
for the pulls, `envelope-tornado.txt`, `envelope-points.txt`, `fuel-tornado.txt`,
`run_probes.sh`). What changed against TOR-8: the pull and instantaneous-turn
rows (the +8 row), the loaded figures (a lighter load: 55,514 against 56,982 lb),
and everything above the 50,000 ft ceiling (B5-4). The clean top speeds, climb,
acceleration, sustained turn and endurance did not move: B5-4 changed rows, a
ceiling and flows, not thrust, weight or the shared drag law. **The 60,000 ft
ceiling behaviour**: before B5-4 the record's 60,000 ft was held in level flight
at 837 kt; the polygons now stop at 50,000 ft, so level flight is held up to it
and a start at 55,000 or 60,000 ft cannot hold altitude (the afterburner settles
at Mach 0.98 and sinks 20,000 ft; `envelope_probe` no longer lists the level rows
above 50,000 ft). The zoom climb still crosses the cap (peak 55,500 to 57,900
ft), which is momentum, not thrust, and was not cut. The probe's loaded
afterburner climb schedule is read as in MF1-10 (dead-mass payload), so the
loaded climb rate is quoted only as a ratio to the clean climb.

The hybrid default is the validated path; `--legacy-flight` and the restricted
`--native-flight-tables` path stay distinct and unchanged. Every air maneuver in
TOR-8 reported `non_finite=0` (the scripted `dive` and `overspeed` end in an
overspeed loss); `--headless-flight 1200 --aircraft tornado` after TOR-6b and
B5-4 reads `non_finite=0`, fuel 11,221 lb. The liftoff and approach battery
windows (135 to 165 kt and 140 to 175 kt) are fitted plausibility ranges (agent
decisions); the TOR-10 battery readings are in the checks below.

## AI-flown swivelling stations

Checked 2026-11-11 (TOR-10), as TOR-9 left it open (its item 3, repeated in
TOR-5b and TOR-6b): do an AI-flown Tornado's swivelling stations follow the
sweep?

**Half yes, half no.** The drawing follows: for every aircraft but the
player's, the renderer puts the swing wing's turn on each swivelling row from
that aircraft's speed and flaps (`pylons::swung`, called from
`render_snapshot.rs`), so AI and replayed Tornados draw their packs and
rail missiles on the swept wing (the replay test and the TOR-9 frames show the
wing swept at 545 to 580 kt; no frame from below shows the stores themselves).
**The AI's physics does not follow: its stores leave from the spread positions.**
The host sets `Rack::turn` only for human-flown aircraft (`step_all_rewound`
calls `Ownship::set_wing_turn`); an AI aircraft's stores live in the AI wings'
own configuration (`ai_wings`), which nothing gives a turn, so its racks stay at
turn 0.

Evidence. A temporary line (reverted; `git status` shows no Rust change) in
`release_strike` printed the rack's turn at the AI's first release, from
`--a2g-run UKR QUFACT --pilot ai --aircraft tornado --load MK82P3 --quick-wing
--seconds 120` (log `.local/m3-aircraft/checks/TOR-10/ai-swivel-strike.log`):

| Quantity | Value |
| --- | --- |
| AI Tornado speed at the release | 457.1 kt |
| Wing turn the schedule gives at that speed | 0.4337 rad (24.9 degrees, a 50 degree sweep) |
| `Rack::turn` on the AI's row 7 | **0.0000** |
| Release point used (right, up, forward, ft) | (9.50, -1.45, 1.50), the spread position |
| Where the swept pylon hangs at that turn | (10.26, -1.45, -0.06): 0.76 ft further out, 1.56 ft further aft |
| At the AI's usual 545 to 580 kt attack speed (full sweep, 0.70 rad) | 0.87 ft further out, 2.61 ft further aft |

The row 8 rail missiles are off by the same amount (the rail rides the pylon's
swept point). The AI's bomb sight predicts from the same spread point
(`next_release_point`), so its aim is consistent with where the bombs start.
Effect: a pack or missile starts up to about 2.6 ft from where its pylon is
drawn; nothing breaks (`ai-tornado-quick-mission-2v2` passes, the AI fires its
AIM-9M at 20,157 ft, and the strike run drops three bombs with three impacts).
Hits were not measured against the offset. **Not fixed** (this slice is
documentation). The fix would be one call each tick that gives every AI actor's
configuration the turn its flight gives (`set_wing_turn` in `ai_wings`, with a
test); it changes only where a store starts, not what the AI decides, so it
stays on the right side of the no-AI-work rule, but it is John's call.

**Update (B5-AI, 2026-11-11): fixed.** John asked for it (decision 19, the
pylons swivel like the real aircraft). `AiWings::carry_swing_wings`, called first
in `AiWings::step`, sets each AI configuration's turn from its own aircraft's
speed and flaps through `swing::wing_turn`, every tick, on every peer; nothing is
checkpointed or recorded (the checkpoint skips `turn`). The same temporary print
at the same first release (tick 5,592, 457.1 kt) now reads `Rack::turn` 0.4338
and a release point of (-10.32, -1.45, 0.22) against (-9.50, -1.45, 1.50) before
(left pylon: 0.82 ft further out, 1.28 ft further aft; the right-hand figures
above are 0.76 and 1.56 ft). The run still drops three bombs, three impacts, one
kill. Log `.local/m3-aircraft/checks/B5-AI/tor-fix.txt` against `tor-nofix.txt`.

## TOR-11 Sweep from 350 mph and the inboard AIM-9 rail (2026-11-12)

Implementation mode, in `m3-tornado`, after the batch merge (`b7d7abeb`); not yet committed
when written. John (2026-11-12): "I don't think for the tornado the wings should start
sweeping back until it's about 350 mph." and "The AIM-9 rail on the wing pylon is too long
and on the wrong side."

- **Sweep.** `swing::SWEEP_FROM_KNOTS` 250 to 304 (350 mph; opinionated, John), the full sweep
  stays 600 kt (nothing forced a change); doc comment, `speed_for_sweep`, the unit tests, the
  animation probe's expected sweep (it now reads the constants) and the quoted examples follow.
  At the old numbers' sample speeds the drawn sweep is now 28.7 degrees at 330 kt, 47.3 at 461 kt,
  66.0 at 593 kt; 45 degrees is 445 kt. The scenarios' 200 kt (spread) and 650 kt (fully swept)
  are unchanged and still pose 25 and 67 degrees. Human and AI both use `wing_turn` (the AI
  through `carry_swing_wings`), so both follow; the AI-flown release at 457.1 kt now sits at a 46.7
  degree sweep, not 50 (the TOR-10 measurement is a dated record).
- **Rail flank and length.** New `Location::InboardRail` (byte 0x84) with `Hang::InboardRail`:
  the missile is 1.25 ft inboard of row 7's pylon (8.25 ft out at 25 degrees; 8.91 / -8.95 at
  45; 9.11 / -9.28 at 67), the offset added after the sweep as before, drawn on a 3.0 ft arm
  (was 5.61 ft on the 9.35 ft AIM-9M, from 0.6 of its length). The F-14's and F-15's shoulder
  rails are untouched. The 3.0 ft is fitted: the real LAU-7 launcher is 110 in overall (Marvin
  Engineering data sheet) but no source gives the shoe-span rail, see the
  [spec](../spec/stores-on-pylons.md#the-tornado-aim-9-rail-on-the-inboard-flank-shorter-and-the-sweep-from-350-mph-tor-11-2026-11-12).
- **Clearance, inboard, AIM-9M, default load (25 / 45 / 67 degrees).** Into the airframe 0.00 /
  0.00 / 0.00 ft; pack 0.06 ft clear at all three; row 6 Paveways 3.63 / 4.30 / 4.49 ft; centreline tank
  6.58 / 7.24 / 7.44 ft; gear legs 3.09 / 3.71 / 3.90 ft (was 6.3 outboard at 67); lowest point 5.86 ft above the
  ground, gear down. Every other kept missile 0.00 ft into the airframe; the four refused
  stores read the same entries as outboard, so the refusals, 4,581 placements and 222
  Cheat-only stand. Release origin and jettison follow (scenarios updated).
- **Checks (2026-11-12).** `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`
  clean; `cargo test --locked --workspace --no-fail-fast` 7,692 passed, 0 failed (two new: the
  inboard rail's hanging and arm in `pylons`, and the schedule's 304 kt knee; the sweep tests in
  `swing`, `tornado_animation`, `replay::convert` and the rail position test in `hardpoints`
  moved with the numbers); `--validate-ordnance --no-audio` 45 aircraft, 4,581 weapon placements,
  233 tank placements, 222 Cheat-only (no pin moved); Tornado battery
  (`--scenario '*tornado*'`) 21 of 21; tools unittests OK; `tools/check_docs.py` passed.
- **Frames.** `.local/m3-aircraft/frames/TOR-11/` (local evidence): before and after at 25, 45 and 67 degrees from below, front-low and above, with 3x crops and side-by-side pairs.

## TOR-13 Two versions, Combat and Clean (2026-11-12)

Update (TOR-13, 2026-11-12), decision 23b revisited: John asked for two selectable Tornado IDS
versions. Full behaviour, numbers and the clearance table are in
[stores on pylons](../spec/stores-on-pylons.md#two-tornado-ids-versions-and-the-combat-version-outer-wing-pods-tor-13-2026-11-12);
this is the acceptance summary.

- **Identities.** `tornado` is now "Tornado IDS (Combat)" and a new `tornadoc` is "Tornado IDS
  (Clean)" (`AircraftId::TornadoClean`, wire index 45, checkpoint code 46; the Mirage F1's wire index
  moves 45 to 46), a runtime variant on the same `TOR.PT`, shape, rig, cockpit, flight model and
  weapon stations, never an alias. The selectable roster is 47 (45 retail identities, the F/A-XX
  and this variant); `AircraftId::ALL` stays 45.
- **Combat** carries the BOZ-107 (right, 717 lb, 13.1 by 1.58 ft) and the Sky Shadow (left, 441 lb,
  9.8 by 1.38 ft) on fixed outer wing pylons, 16.0 ft out and 1.5 ft forward on the swinging
  panel (fitted), swivelling with it (Update (TOR-12, 2026-11-12): 14.46 ft out and 1.7 ft forward after the wing moved). They are no hardpoint row: Load Ordnance (Normal and Cheat), saved
  loads, the lobby and the AI never see them. The record's `MIG31.ECM` stays (jammer, 60 chaff, 60
  flares); chaff and flares leave the BOZ-107's aft face, following the sweep. Carried mass is
  1,158 lb more than the previous Tornado, through the existing fixed-equipment mass and loaded
  drag law (no per-store drag exists). **Flight figures that depend on the load (TOR-8's default-load
  rows, liftoff, approach, rollout) move for Combat; the Clean version is exactly the earlier
  Tornado.** Measured (`flight-liftoff-*` and `flight-approach-*`): default gross 56,672 lb for
  Combat, 55,514 for Clean; liftoff 149.9 kt after 1,916 ft (Clean 147.2 kt, 1,805 ft); touchdown
  158.0 kt (Clean 155.3). The other TOR-8 default-load figures were not re-measured.
- **Clean** is the previous Tornado with no jammer, chaff or flares (`MIG31.ECM` is one record, so
  they go together) and no outer pods. The RWR is not tied to the record and stays. The weapons
  window lists no `CHAFF` and `FLARE` counts and the RWR window no `JAM` for an aircraft with no
  dispenser or jammer.
- **Whole-surface gate.** At 25, 45 and 67 degrees both pods clear the wing panel (1.03 to 1.06 ft,
  the pylon), the flap strips (0.63 ft with the flaps down, 1.17 to 1.97 up), the tailerons (3.98 ft at
  67), the glove plates, the fuselage, the row 7 store (3.61 ft) and the AIM-9 rail (4.15 ft), the
  gear (10.6 ft) and the ground (4.76 and 4.96 ft above the gear contact). Nothing crosses; no move
  was needed.
- **Pins.** `--validate-ordnance`: 45 aircraft, 18 recovered stations, 4,581 weapon placements, 233 tank
  placements, 222 Cheat-only, 80 pods: **none moved** (the pods are no row); a new step checks the
  versions.
- **Frames** (`.local/m3-aircraft/frames/TOR-13/`): the Combat version from below and the front
  quarter at 25 and 67 degrees, close-ups of the Sky Shadow (yellow triangle; palette entry 187,
  (247, 194, 40)) and the BOZ-107's aft end, a flare and chaff release leaving the BOZ-107, the Clean
  version from below, and the Quick Mission aircraft list with both names.
- **Checks (2026-11-12).** `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` clean;
  `cargo test --locked --workspace --no-fail-fast` 7,701 passed, 0 failed; `--validate-ordnance` and
  `--validate-creator` passed (0 problems, 177,000 and 76,000 fuzz events); the Tornado battery
  (`*tornado*`, with the new Clean, countermeasure-origin and two-version net scenarios) 41 of 41 after
  two scenario-table fixes; `net-window-lobby` and `net-content-missing` 2 of 2; tools unittests 559
  OK; `tools/check_docs.py` passed. Local: `.local/m3-aircraft/checks/TOR-13/`.
- **Open for John.** The outer pylon position and the pods' diameters are fitted; the Sky Shadow
  pair of bulges and the BOZ-107's nine outlets are the TOR-13a models; in a Guns only start (no
  external stores) the shared rule carries none of the fixed external equipment's mass, the pods' included,
  although the pods stay drawn (a fixed pod's existing behaviour, not examined further);
  `by_label("Tornado IDS")` no longer names an identity (a recording made before this slice, none was
  released).

## TOR-12 John's wing review ported (2026-11-12)

Update (TOR-12, 2026-11-12), implementation mode, opinionated (John: nine decisions from his Blender
review of the wing and fuselage, ported to both versions). Behaviour and numbers:
[the wing placement](../formats/aircraft-control-surfaces.md#tornado-wing-placement-tor-12-2026-11-12) and
[stores on pylons](../spec/stores-on-pylons.md#the-tornados-wing-placed-pylons-moved-with-it-tor-12-2026-11-12);
this is the acceptance summary.

- **What changed.** The left wing is the right one mirrored; the wing sits 1.54 ft in, 0.1167 ft up and 0.20 ft forward;
  the root points are 2.3483 ft further inboard; the pivot is (4.3333, -0.9007, 0.31) ft, left mirrored; the glove
  plates are gone; twelve rear fillet faces are not drawn; four closing walls are added; the fuselage points at
  (+-6.6667, 4.0, 0) ft moved to y 4.83625 ft; the pylons (rows 7, 8, the pods 9 and 10) stay at the same place on the
  panel. The shape is 279 faces (287, minus 12, plus 4), the probe line (it was 291).
- **Match with the Blender poses.** The placed and swept vertices match the exported scene within 2e-5 ft at sweep 25 and
  67 (a unit test on five vertices); the left and right wings are an exact mirror (0 ft, `wing_mirror_error_ft`, at all ten
  sweep samples); the five root vertices stay inside the fuselage's skin at every sample.
- **Probe.** `--aircraft tornado --animation-probe`: 279 neutral faces, 0 required and check failures; rigidity, skin gap
  and fold unchanged; root travel at 67 degrees 2.03 ft (outboard 1.75, inboard 0.65); the flap strip seam opens 0.51 ft
  at full droop (0.77 before); the stow check passes (left foot overshoot 0.17 ft as before).
- **Clearances.** Pods against the wing panel 1.04 to 1.09 ft, strips 0.63 to 2.18 ft, tailerons 2.62 to 8.74 ft, fuselage
  7.05 to 7.73 ft, the Mk 82 pack 3.61 to 5.46 ft; stores 4.88 to 5.98 ft above the gear contact for the wing rows; the Mk 82
  pack 2.97 ft from the gear legs at 25 degrees (was 5.29). **The AIM-9M rail enters the fuselage by 0.05 to 0.11 ft at 35
  to 45 degrees** (under the 0.3 ft line; not moved, a finding for John). Full table in stores on pylons.
- **Retired rig numbers.** The seam table, the pivot circle and the glove plate in TOR-3 and TOR-5b above are history; their
  tests are replaced by the placement tests.
- **Pins moved.** The battery's release and jettison origins for rows 7 and 8 (a2g-release-origin, a2g-jettison, both
  versions): 7.96 / 1.70 / 8.77 and 6.71 / 7.52 ft out. The audit pins did not move.
- **Frames** (`.local/m3-aircraft/frames/TOR-12/`): Combat from below and above at 25 and 67 degrees, front quarters at 25,
  the wing root and wall at 25 and 67, the Clean version from below. Observed: both sides now read alike; at 67 degrees a
  sliver of sky shows beside the wing's trailing root and the taileron where the rear fillet was.

- **In flight.** `.local/m3-aircraft/frames/TOR-12/inflight-*.png`: the Combat Tornado at 452 kt (46 degrees) in the
  chase and oblique views, wings swept evenly.
- **Checks (2026-11-12).** `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` clean;
  `cargo test --locked --workspace --no-fail-fast` 7,705 passed, 0 failed, 108 ignored; `--validate-ordnance` and
  `--validate-creator` passed (0 problems, 177,000 and 76,000 fuzz events; the pins did not move); the Tornado battery
  (`*tornado*`, windows 1, jobs 1) 40 of 40; tools unittests 559 OK; `tools/check_docs.py` passed. Not rerun: the net
  battery scenarios (no change in that code). Local: `.local/m3-aircraft/checks/TOR-12/`.
- **Open for John.** The AIM-9M rail's 0.05 to 0.11 ft entry into the fuselage at 35 to 45 degrees (options in stores on
  pylons); the sliver of sky beside the swept wing's trailing root; the vapor point now sits on the drawn tip on both
  sides (was 0.67 ft outboard of the right tip), an agent decision.

## TOR-14 Pylons centred on the wing, one size (2026-11-13)

Update (TOR-14, 2026-11-13), implementation mode, opinionated (John: "all pylons need to be centered on the wing
always point forward but have center position locked on the wing as it sweeps back"; "when wings are swept forward
the out pylons with the pods grow in size"; the mid-chord definition, the fixed 3.3 ft pylon length and the numbers are
agent decisions, fitted). Behaviour and numbers:
[stores on pylons](../spec/stores-on-pylons.md#the-tornados-pylons-centred-on-the-wing-one-size-at-every-sweep-tor-14-2026-11-13);
this is the acceptance summary. Both versions (the Clean one has rows 7 and 8).

- **Cause of the growth.** The drawn pylon was 0.45 of the store's length (5.9 ft under the BOZ-107), centred on the
  store's middle and trimmed to the surface under it; the outer pylon stood 1.2 ft ahead of the spread wing's leading
  edge, so at 25 degrees nothing was over its middle, the slab kept its full length in the open, and as the wing swept
  under it the trim shortened it. Not the 0.117 ft the wing was raised by.
- **What changed.** Pylon centres are the placed panel's mid-chord at their spans, which did not move (7.96 and 14.46 ft
  out): 0.38 ft aft (rows 7, 8) and 2.22 ft aft (rows 9, 10) at 25 degrees; 7.38 / -2.94 (6.13 for the rail) and
  10.98 / -8.66 at 67. The drawn pylon is 3.3 ft long, 0.28 ft wide and 1.0 ft deep (over the lugs) at every sweep and
  for every store, centred on the carried point. Locking and the swivel were already right (the carried point, the
  wing's turn undone) and are pinned by new tests.
- **Pins moved.** The battery's release and jettison origins for rows 7 and 8 (a2g-release-origin, a2g-jettison, both
  versions): 7.96 / -0.38 / 7.38 / -2.94 and 6.71 / 6.13 ft out. The chaff and flare outlet moved with the pod (14.46,
  -1.67, -8.78 at 25 degrees; 10.98, -1.67, -15.22 at 67); the battery's bounds held. The audit pins did not move.
- **Open for John.** **The BOZ-107's tail enters the right taileron by 0.57 ft at 67 degrees** (over the 0.3 ft line; the
  Sky Shadow 0.07, the AIM-9M rail 0.03; nothing at 25 and 45): options in stores on pylons. The AIM-9M rail's entry into
  the glove wall is 0.11 ft at every sweep from 25 to 65 degrees now (TOR-12: 0.05 to 0.11 at 35 to 45); the wall is
  unchanged. The Mk 82 pack and the rail are nearer the tailerons at full sweep (0.72 and 0.00 ft). The stores' origins
  are centred on the pylon, so the Mk 82's body is 0.87 ft aft and the AIM-9M's 1.21 ft ahead of the pylon's centre.
- **Frames** (`.local/m3-aircraft/frames/TOR-14/`): from above at 25, 45 and 67 degrees, from below at 25, 45 and 67,
  close-ups of the outer pod pylon and the inner pylon with the rail at 25 and 67 degrees at the same camera offset
  (low and from above).
- **Checks (2026-11-13).** `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings` clean;
  `cargo test --locked --workspace --no-fail-fast` 7,709 passed, 0 failed, 108 ignored (four new tests); `--validate-ordnance`
  passed (`TOR.PT: 5 source rows, 56 compatible weapon placements, 8 tank placements`; the pins did not move);
  `--validate-creator` passed (177,000 and 76,000 fuzz events, 0 problems); the Tornado battery (`*tornado*`, windows 1,
  jobs 1) 40 of 40; tools unittests 559 OK (5 skipped); `tools/check_docs.py` passed. Local:
  `.local/m3-aircraft/checks/TOR-14/` (`clear3.sh`, `clearance.sh`, `frames.py`, `final.sh`, the logs).

## TOR-14b Pods on rear lugs (2026-11-13)

Update (TOR-14b, 2026-11-13), implementation mode, opinionated (John: "Both pods are technically mounted towards the
rear in RL have the agent do the same for these and avoid the taileromms"; the lug station is a fitted agent decision).
Behaviour and numbers:
[stores on pylons](../spec/stores-on-pylons.md#the-tornados-pods-hang-from-rear-lugs-tor-14b-2026-11-13).

- **What changed.** The BOZ-107 and the Sky Shadow hang from lugs a third of their length aft of their middle
  (4.37 and 3.28 ft; `AuthoredPod::lug_aft`, `StoreSize::lug_aft`), so the pod hangs mostly ahead of its pylon. The
  pylon (14.46 ft out, on the mid-chord, 3.3 ft) did not move. The chaff and flare outlet is 2.19 ft aft of the pylon
  (10.98, -1.67, -10.84 at 67 degrees; was -15.22). The Mk 82 pack and the AIM-9M are as they were.
- **Result.** At 67 degrees both pods clear both tailerons by 1.37 / 1.75 ft or more, neutral and at full pitch and roll
  (the BOZ-107's 0.57 ft entry in TOR-14 is gone); nothing else is nearer than 1.37 ft (the Sky Shadow's nose under the
  panel's leading edge); at 25 degrees with the flaps down the pods are 0.69 / 0.88 ft from the drooped strips.
- **Pins moved.** The battery's `flight-countermeasures-tornado-boz-sweep67` now pins the outlet at (10.98, -1.67,
  -10.85) within 0.3 ft. The release and jettison origins did not move (the pods are never released).
- **Frames** (`.local/m3-aircraft/frames/TOR-14b/`): from below at 25 and 67 degrees and above at 67; pod close-ups
  at 25 and 67 at the TOR-14 camera offset; a side view of one pod at 25 with the pylon over its rear.
- **Checks (2026-11-13).** `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` clean; `cargo test
  --locked --workspace --no-fail-fast` 7,710 passed, 0 failed, 108 ignored (the new pod test); `--validate-ordnance`
  passed (pins unchanged); the Tornado battery 40 of 40; tools unittests 559 OK; `tools/check_docs.py` passed. Not rerun:
  `--validate-creator` (no creator data changed). Local: `.local/m3-aircraft/checks/TOR-14b/`.

## Open findings for John

Nothing here was changed by this slice; each is a decision or a model question.
The first eight are the ones John's answers so far leave open (items 4 and 7 were
listed by TOR-6 and TOR-6b, item 8 is new).

1. **The shared drag law** (fleet level, John's call). The Tornado is inside the
   published figures (sea level Mach 1.20 against about 1.2; 36,000 ft Mach 2.30
   against 2.2, 4.5 percent over, inside B5-4's 5 percent; 30,000 ft in 91 to 101
   s against under 2 min), so nothing was fitted. The law's cost shows on other
   aircraft: the Gripen 11 percent under at sea level, the F-15E 8 percent under
   at altitude, the F-5E trimmed at a cost of climb and sustained turn, the
   Mirage F1 climbing at half the published rate.
2. **No thrust-reverser term.** The source has no reverser word and the flight
   model has no reverser force, so the landing rollout is 2,380 ft (725 m, was
   757 m on the two-tank load) against the real 370 m with reversers; the liftoff
   run, 1,805 ft (550 m), is shorter than the real 900 m. Adding a reverser is a
   flight-law change for John.
3. **No sweep term in the flight law.** The sweep draws the wing and moves the
   swivelling stations and nothing else. The real sweep changes lift and drag a
   great deal, but because the drawn sweep is a pure function of speed, the
   Tornado's own envelope rows already carry the speed-dependent result. A sweep
   multiplier on the slow edge and the drag would be a fitted per-aircraft change;
   not made. The pilot cannot select the sweep (the real aircraft has a lever);
   the 250 to 600 kt schedule is TORE's.
4. **BK 27 muzzle velocity is 19 percent low** (2,933 ft/s against about 3,609
   real) and cadence is the shared 32 rounds a second against 33 to 57 for two
   real guns, because the gun fields are shared; and **one muzzle** (starboard,
   18.33 ft forward) stands for the two real guns, one each side. Listed, not
   changed (the Gripen's single row is the same).
5. **The glove plate colour.** The four plates are flat colours cloned from the
   glove's top triangles, not the wing's camouflage pattern, and the right side
   keeps about 0.8 square ft more gap than the spread wing at 67 degrees (a small
   sky notch at the rear root; closing it would cut through the glove's top face).
6. **Unsourced numbers.** The rail's flank (outboard chosen, the F-14's; the
   inboard flank was not measured and no source names the real one), the inner
   pylon position (9.5 ft out and 1.5 ft forward is a measurement on this shape; Update (TOR-12, 2026-11-12): 7.96 ft and 1.7 ft after John's wing review),
   the sweep pivot (a fitted circle; the real pivot has no source; Update (TOR-12, 2026-11-12): John's measured (4.3333, -0.9007) ft), the 250 to 600
   kt schedule, the taileron pitch and roll weights and the 0.25 rad limit, the
   per-leg gear retraction (only "retracts forwards" is cited).
7. **Wide row-7 alternatives cross the rail missile's box.** Load Ordnance only:
   FAB-250 0.04 ft clear, HARM family -0.79, CBU-87 -0.13, B-13 and RBK-250 -0.16,
   Mk 84 -0.23, RBK-500 -0.38 ft against the AIM-9M, by bounding box (not a
   refusal under the neighbour rule, not measured at mesh level; the F-14 text
   accepts a wide store touching the shoulder AIM-9's lower fins, John,
   2026-10-14).
8. **The AI's swivelling stations release from the spread positions** (the section
   above). New in this slice; a one-call fix, John's call.
   **Update (B5-AI, 2026-11-11): fixed.** `AiWings::step` now gives every AI
   configuration its own aircraft's `swing::wing_turn(speed, flaps)` each tick
   (`carry_swing_wings`), as a human's ownship has it, so AI bombs and rail
   missiles leave from the swept pylon. Same TOR-10 run: at the first release
   (457.1 kt, turn 0.4337 rad) `Rack::turn` is 0.4338 (was 0.0000) and the
   release point moves from (-9.50, -1.45, 1.50) to (-10.32, -1.45, 0.22), 0.82
   ft out and 1.28 ft aft; the run still drops three bombs for three impacts and
   one kill. Test `an_ai_tornados_swivelling_stations_follow_its_speed_and_flaps_each_step`.
   Detail in [lane AI, B5-AI](../testing/lane-ai.md#b5-ai-the-f-5es-aim-9b-the-f-15es-role-and-the-ai-tornados-racks-2026-11-11).
9. **The AI Tornado bombs by default** (record class word `0x4000`, as the A-4E and
   F-111F; the F-15E's AI flies CAP). It now also fires its rail AIM-9M although
   the record gives it no infrared sensor row (`ir=false`), as the Mirage F1's AI
   does and the F-5E's never does: noted for the AI pass (O5), nothing changed.
   **Update (B5-AI, 2026-11-11):** no infrared missile needs a sensor row, and the
   F-5E's AI does fire; its AIM-9B is rear-aspect, the AIM-9M all-aspect (first
   launch 13 s head-on against 135 s). The Tornado bombs by default through the
   class word, which the F-15E (`0x8000`) lacks: see [lane AI,
   B5-AI](../testing/lane-ai.md#b5-ai-the-f-5es-aim-9b-the-f-15es-role-and-the-ai-tornados-racks-2026-11-11).
10. **The missile choice on the Tornado is narrow and Eastern in part.** Row 6
    refuses every air-to-air missile and the HARM, Harpoon and Exocet families (they enter
    the fuselage); row 8's mask keeps `AA2`, `AA8`, `AA11` and `AA11B` beside the
    three AIM-9s (the F-14's mask rule). A Western-only list is one constant.
11. **Row 6's Paveways enter the fuselage by their fin tips** (0.96 ft front pair,
    0.55 ft rear pair), accepted by John (T2); `GBU10` and `GBU10A` keep the same
    shape; the GBU-30 family stays refused on row 7 (1.10 ft at 67 degrees).
12. **Rig gaps.** No slat, spoiler or reverser exists in the source; the glove seam
    opens up to 1.22 ft at 67 degrees and the free leading root corner opens 2.7 to
    5 ft before the plates; the taileron forward corner swings 2.41 ft; the left
    main foot pokes 0.17 ft and the closed left brake plate hovers 0.35 ft (source
    asymmetry); the flap lever droops the trailing strips 0.40 rad (fitted); the left and right pack pair sits 0.45 ft out of mirror at 67 degrees
    because the source's pivots are a unit apart.
13. **Cockpit.** Single-seat Eurofighter art on a two-seater: no rear seat or view;
    the fixed eye is 3.7 ft over the shape's top faces and a seated eye would be a
    fleet change; the HUD glass polygon and the mirror seeds are fitted (the
    alternative to the seeds is dead panels).
14. **The half-fuel rule lets the Tornado pass the real limit near empty:** 8.04 G
    at 300 lb against +7.5 (the clean aircraft at half fuel pulls 7.52 G).
15. **The ceiling above the cap.** Level flight stops at 50,000 ft (and the level top
    speed there is 1,135 kt, 1.8 percent under the polygon edge, ratio 0.948 against
    0.965 below); a zoom climb still reaches 55,500 to 57,900 ft on momentum.
16. **Shared code touched** (guarded to the Tornado): `Rack::turn` and the swivelling
    shoulder carry in `side_pylons` (TOR-5, TOR-6b), `same_stores` in the session
    recording (TOR-9), `--flight-speed` in `main.rs`, a developer switch added by
    TOR-3 and kept (documented in INPUT.md); drop it if unwanted.
17. **Not demonstrated.** An RWR with a live threat; a Tornado in live multiplayer
    flight (the net scenarios take no aircraft); an AI roster probe of the refused
    catalog; no Tornado cockpit comparison with retail (none exists).
18. **No whole-body damage variant** is registered and the two `E0` decal slots on
    the fin stay omitted (the fin draws plain camouflage).
19. **Saved loads** from before TOR-6b (an eight-row Tornado) were never in a
    release; none were migrated.
20. **Frames index.** The frame pack is indexed in the local `INDEX.md`.

   Update (TOR-11, 2026-11-12): findings 6 and 7 move with the rail. The flank is now
   inboard (John's call, no source), so the unsourced flank is John's choice, not an agent's;
   the sweep start is John's 304 kt, so only the 600 kt end of the schedule is unsourced. The
   wide row-7 stores cross the inboard missile's box by the same amounts as before (FAB-250
   0.04 ft clear to HARM -0.79 ft).

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extras on elevons, applied here to the
tailerons, the swing wing and the swivelling pylons).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 44 (`the_tornado_round_trips_at_code_44_and_no_identity_shares_it`); `typeSize` 660 handled; every named resource present (`TOR.PTS` an explicit exception); record audit done (copies listed and corrected by John); no alias of the Su-24 (`SU24.PT` is refused in the Quick Mission lists), the Mirage 2000 or the Eurofighter; not a parked target; import report clean |
| Look | Met | Own shape at the header scale (true size, decision 18), damage looks A to D, `render-tornado-skin` passes (run 20261111-140235), no show-through or clipped marking in the frames. The destroyed whole-body variant is not registered, the fin's two decal slots stay omitted, and the glove plates are flat colours (finding 5) |
| Whole-surface gate | Met | Every wing panel, taileron, flap strip, rudder, airbrake, gear and plume piece is a whole panel and the glove plates are four whole faces that never move; the fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity at most 2.2e-6 ft in every pose. The seam openings are inherent to rigid panels (finding 12) |
| Animation | Met | Sweep 25 to 67 degrees as whole panels, signed pitch and roll on whole tailerons, flap droop, rudder, rigid gear with near-stow, two airbrake plates, two burners; neutral equals source; probe 291 neutral faces, 0 required and check failures (re-run 2026-11-11); the 25-pose grids at both ends of the sweep pass; `flight-animation-tornado` passes. The sweep is a fitted schedule with a fitted pivot (finding 6); no slat, spoiler, reverser, hook or canopy word exists in the source |
| Stations | Met, with one John-accepted exception | Default load and every default store clean under the standard pylon rules after John's T1, T3 and T4 and the side rail: tank 0.00 ft, Mk 82 packs 0.00 ft at both sweeps, rail missiles 0.00 ft. **The Paveways' fin tips enter the fuselage 0.96 ft (front) and 0.55 ft (rear): John accepted it (T2).** Open findings 6 (unsourced flank and position) and 7 (wide row-7 alternatives against the rail missile, bounding box only) are not decisions yet |
| Ordnance | Met | Every default store permitted live and released from its own station at both sweeps (gun, GBU-10, Mk 82 pack, AIM-9M); 40 refusals; compatibility, even pairs, tanks, jettison at both sweeps and the AI default checked; 60 / 60 countermeasures from `MIG31.ECM`; `--validate-ordnance` passes for all 45 aircraft (4,581 placements, `TOR.PT` 56 and 8 tank placements) |
| Cockpit | Met | `e2000.HUD` and the Eurofighter panel by name (decision 21); the HUD draws inside a measured glass polygon; mirrors live; overlays and ten instrument pages checked; reviewed sensor presets only; absent systems absent (no infrared row). The RWR with a live threat was not demonstrated and the rear seat has no view (findings 13, 17) |
| Flight | Met with open findings | Own configuration on the hybrid default with John's corrections (RB199 thrust, weights, +8 row, 3 and 22 lb/s, 50,000 ft); envelope re-measured 2026-11-11; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally). **No thrust reverser and no sweep term are modelled (findings 2, 3)**: the rollout is twice the real landing run |
| AI and modes | **Not fully met** | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby, and the AI fights and bombs, all pass. **An AI-flown Tornado's stores leave from the spread positions, not the swept ones** (finding 8): drawn right, released up to 2.6 ft off. (Update, B5-AI, 2026-11-11: fixed, the AI's racks now carry the sweep turn; this row is met.) A Tornado in live multiplayer flight was not played |
| Docs | Met | Source review, specs with provenance, this record, inventory, features, counts and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: swing wing, tailerons, swivelling pylons | Met | Wing turns whole at every sweep with the flaps holding it out; pitch, roll and the 25-pose grids pass with `skin_gap_ft=0`; every moving surface has a rule 11 face table above; the pylons carry their stores through the sweep in the simulation (human), the drawing (every aircraft) and the replay. The AI's release origin is the exception (finding 8) |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `TOR-2` to `TOR-9`
directories (`TOR-5b` and `TOR-6b` for the final stations): skin and damage views,
the wing at 25, 46 and 67 degrees with the glove plates, taileron, flap and rudder
poses, gear, airbrake and burner stages, the default load from four angles at
both sweeps with 3x crops of each station row, the AIM-9 rail launch, ordnance
and countermeasure frames, cockpit, HUD and instrument pages, and the AI, Quick
Mission and replay frames. Observed points: the Tornado reads as a Tornado at
its real size with no show-through; at 25 degrees each Mk 82 pack hangs under its
wing 9.5 ft out with the Sidewinder beside it, and at 67 degrees both hang under
the swept panel streamwise, never in empty air; the tank sits between the
Paveway pairs; each glove plate reads as a short extension at 25 degrees and
fills the wedge at 46 and 67, leaving a small sky notch at the right rear root;
the gear tucks as rigid plates; the two airbrake plates and both burner plumes
draw; the cockpit shows the Eurofighter panel with the HUD inside the glass; and
the Quick Mission list names the exact "Tornado IDS" on page 3 of 4 with no
Su-24. The frames are local evidence and are not committed. The final stations
were not re-captured in TOR-10 (no presentation change). Update (TOR-12, 2026-11-12): the glove plates in the
description above are gone; `TOR-12` frames (below and above at 25 and 67 degrees, front quarters, the wing root
and wall, the Clean version) show the placed wing.

## Full checks, 2026-11-11

Run on the merged `m3-aircraft` head `12021337` plus this slice's documentation
changes (no Rust or tool change in this slice; a temporary print used to check
the AI's stores was reverted before the builds recorded here). Every command ran
through `.local/m3-imports/light3.sh` from the repository root (John at the PC,
nice 19, 4 jobs), one heavy command at a time, and the windowed commands through
`tools/agent-run.sh`. The logs are under `.local/m3-aircraft/checks/TOR-10/`.

| Command | Result |
| --- | --- |
| `cargo build --locked -p tore-app -p tore-server`, `-p tore-session --bin tore-bot` | Passing (rebuilt before the battery) |
| `cargo test --locked --workspace`, `cargo clippy --workspace --all-targets --locked -- -D warnings` | Not rerun: no Rust changed. The merge's runs on this head stand: 7,690 passed, 0 failed, 108 ignored, clippy clean |
| `performance_probe` and `envelope_probe` for `TOR.PT`, clean and default load (`PROBE_PAYLOAD=13228`) | Figures in the flight envelope above; `run_probes.sh` and the logs in the check folder |
| Battery, Tornado: `--scenario '*-tornado*' --profile .local/m3-aircraft/profile --windows 1 --jobs 1` | 21 of 21 passed (run 20261111-140235, 269 s): a2g-jettison and a2g-jettison-rail, a2g-release-origin and a2g-release-origin-rail, ai-tornado-quick-mission-2v2, flight-animation, flight-livefire slots 1 to 4 and slot 4 at 67 degrees, flight-jettison slots 2 to 4, flight-liftoff, flight-liftoff-retail, flight-approach, flight-variety, menus-snap-quick-lobby-ordnance-laser-notes-tornado, render-tornado-skin, replay-rec-tornado-2v2 (flight, ai, menus and replay lanes; no Tornado scenario exists in the net lane) |
| Battery, net: `net-content-missing`, `net-window-lobby` (after rebuilding `tore-server` and `tore-bot`) | 2 of 2 passed, each run alone (runs 20261111-140817, 43.1 s, and 20261111-140901, 181.4 s) |
| `tore-app --validate-ordnance --no-audio` (`.local/m3-aircraft/profile`) | Passing: `TOR.PT: 5 source rows, 0 optional weapon rows, 56 compatible weapon placements, 8 tank placements; defaults and restart passed; catalog 36 normal, 83 Cheat`; `45 aircraft, 18 recovered weapon stations, 4581 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed`; 80 pod and 222 Cheat-only placements passed |
| `--headless-flight 1200 --aircraft tornado --no-audio` | `non_finite=0`, fuel 11,221 to 11,200 lb |
| `--aircraft tornado --animation-probe` | 291 neutral faces, 0 required and check failures |
| `--a2g-run UKR QUFACT --pilot ai --aircraft tornado --load MK82P3 --quick-wing` with a temporary rack-turn print | The AI finding above (`ai-swivel-strike.log`) |
| `cargo fmt --all -- --check` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` (and on `target/debug/tore-app`, `target/debug/tore-extract`) | 2,272 files; 1 file; 1 file |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| `python3 tools/check_docs.py` | Passing (381 files; the new baseline carries the header and is checked when staged) |

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, Tornado in live multiplayer flight, `ai-fuzz` or the
full AI lane was run. `--native-flight-tables` needs the extracted
`sine-q15.bin` and `atan-pa.bin`, which are not present locally, so it stays a
separate restricted path and was not run. The clean-load ground runs were not
made (a ground start always carries the default load). A loaded climb
measurement was not quoted (the probe's loaded schedule is a dead-mass
artefact). The stations were measured on the scaled shape's triangles for the
default load and by bounding box for the neighbour crossings; the real inner
pylon position and rail flank have no source. The frame of an AI Tornado's stores
from below was not made (the evidence is the log). The flight and rendered
evidence above comes from the Linux development host and the wrapped captures.
