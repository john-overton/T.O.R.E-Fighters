# Saab Gripen acceptance record (M3 batch 5, 2026-11-10)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The Saab Gripen (`GRIPEN.PT`) is a selectable retail identity in
`AircraftId::ALL` (the 42nd entry) and `AircraftId::SELECTABLE` (the 43rd),
appended at checkpoint code 42 and carried on the wire as `SELECTABLE` index 42
inside the 9-bit field (NET-ID9, protocol 83). It is not an alias for the
Rafale, the F/A-18 or any other record: it has its own `GRIPEN.PT`, `GRI.SH`,
`GRIPEN.HUD` and `~GRI*.PIC` cockpit family and rig, and it names shared
resources only where the record itself names them (`VIS340.SEE`, `F18R.SEE`,
`F18.ECM`, `AAS38.SEE`, `BK27.JT`, `F150.GAS`, `AGM65G.JT`, `AIM120.JT` and
`AIM9M.JT`). Unlike the other two batch aircraft, the Gripen record is a
genuine JAS 39A, not a template: weights, fuel, engine count 1 and both thrusts
agree with the real aircraft. The template values are the 60,000 ft ceiling (the
F/A-18's; real 50,000), the -4 G row (every record; real -3), the 570-round gun
(the F/A-18's; real 120) and a 170-statement block shared with the Rafale and
F/A-18. John made decisions about it in two rounds on 2026-11-10 (below). No
retail comparison exists, so this record claims source-reviewed,
extraction-supported, headless-flight-supported and rendered-flight-supported
status, not retail parity.

## John's decisions

Every decision is opinionated and recorded as John's with its date; every other
choice in this record is an agent decision and is labelled as one. Round 1 was
the batch decision round (numbers 7 to 10 in the plan); round 2 is
`.local/m3-aircraft/BATCH5-DECISIONS-2.md` ("for the gripen keep - no changes").

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 7 | 2026-11-10 (round 1) | **HUD and cockpit follow the record**: `GRIPEN.HUD`, stem `GRI`, `~GRIH.PIC` and `~GRI_P.PIC` (13 own `~GRI*.PIC` pictures exist). | [Gripen sensors, HUD and cockpit](../formats/aircraft-variety.md#gripen-sensors-hud-and-cockpit-gri-7-2026-11-10) |
| 8 | 2026-11-10 (round 1) | **AAS-38 row draws no pod body**; the FLIR sensor stays. | [Gripen AAS-38 pod row](../formats/aircraft-variety.md#gripen-aas-38-pod-row-for-john) |
| 9 | 2026-11-10 (round 1) | **The canards pitch and roll differentially** ("Canards definitely roll"). John decided the roll; the magnitudes are the agent's fit. | [Gripen control surfaces](../formats/aircraft-control-surfaces.md#gripen-control-surfaces-gri-3-2026-11-10) |
| 10 | 2026-11-10 (round 1) | **Keep the 570 gun rounds**; leave the 60,000 ft ceiling and the -4 G limit. | [Gripen ordnance](../formats/aircraft-variety.md#gripen-ordnance-gri-6-2026-11-10) |
| B1 | 2026-11-10 (round 2) | **Inner wing stores against the open gear door wall (gear down): accepted** as measured. | [Gripen station decisions](../spec/stores-on-pylons.md#gripen-station-decisions-john-2026-11-10) |
| B2 | 2026-11-10 (round 2) | **Centreline tank against the left strut plate (gear down): accepted.** | same |
| B3 | 2026-11-10 (round 2) | **The gun row stays on the right**, as the record has it. | same |
| E2 | 2026-11-10 (round 2) | **Elevon root slit accepted** (up to about 1 ft at full pitch plus roll). | same, and the open finding below |

No real-size scaling decision was needed: the shape is within 1.6 percent of
the real aircraft on every axis, so the scale table stays `[1, 1, 1]` (agent
decision from the measured sizes).

## Source identity and dependency closure

The source review is
[Gripen source review](../formats/aircraft-variety.md#gripen-source-review-2026-11-10).
Retail media stays local; no extracted bytes are committed. The archive hashes
were recomputed on 2026-11-10 (X-10) and match the earlier reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |

The selected closure (the `FA_2.LIB` copies; the cockpit pictures are in
`FA_1.LIB`):

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `GRIPEN.PT` | Record, 219 statements, nine hardpoints | `9cc5c3f11d10d4ce9b291164d77102d00ec16685b874d2b56d81384527251487` |
| `GRI.SH` | Shape, 383 runtime faces (388 export), six state words | `ea05b2789f2c8b342517cd155f4f2e28c52aa77f0fab9ec1bb6cee6e885d96e3` |
| `GRI_S.SH` | Shadow shape | `80bfce4989c0e8318b9d1eccdfb29cca77dc5862e6544c45a4d3c6c69ced7134` |
| `GRI_A.SH` | Damage body A (409 faces) | `fecb39f8c46f5eee52dcc1d6b8d13c945e60c505c795b085fb9ba28b8e0bd091` |
| `GRI_B.SH` | Damage piece B (50 faces) | `591e28812902504a7abbd541f21c6c6024fbc472b3c825967f02dbf5b135d34c` |
| `GRI_C.SH` | Damage body C (296 faces) | `96163db969637ac59fbe795077192ac62d40b99a9a49f029b510fe28e3363c20` |
| `GRI_D.SH` | Damage piece D (256 faces) | `e17dbf501e1f77f2a7cf6a2ec2af0a833adc109fa8cd06efd6c9f4b0956b72d9` |
| `F18R.SEE` | Radar, ADVANCED (shared with the F/A-18D) | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `F18.ECM` | Jammer, LATE, chaff 30 and flare 30 | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `VIS340.SEE` | Visual sensor | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `AAS38.SEE` | Infrared channel (row 3, flags 0x8 at the origin) | `9e447523a2fda1612d17fa7d76d3beb0fb03f11115f9e855937ffd2875f5f75b` |
| `BK27.JT` | Internal gun (the station's own gun) | `0458077843fcc3c6252acfdb706f735cd7b26f80c0d36a74adbe80ab008a106f` |
| `F150.GAS` | Default centreline tank | `ca9054921aa93eb890801a5a1f64deb55c8bcc6d03854ff03f1b42833a9f4c95` |
| `AGM65G.JT` | Default inner-wing store | `b94ee0dae96dd2b7fc70a21e430444a1d72b565eec9611ecb749965ece2e5cca` |
| `AIM120.JT` | Default outer-wing store | `702d559376e544b49d2d118e1a8911227b0abe6560ff84340a7fd592c709d7a8` |
| `AIM9M.JT` | Default wingtip store | `fdac769d40e374ed691a2758c80f4deddf44ecb136d8973374ea053ca03723cf` |
| `GRIPEN.HUD` | HUD and cockpit pointers | `dab77fd5cc30007240e84f3c47b664a26a64f81444579bffd03a923e9df3ec80` |
| `~GRIH.PIC` | Forward cockpit (1279 x 490) | `e92ace44599d958179369a0ed7d7691b701c0006073dacbd830c9819fe886dc2` |
| `~GRI_P.PIC` | Instrument panel | `e1fb732b675b09642cb76b03e142ece2ee0a0c2907d547f2ff2b09e4fb46d865` |
| `_GRI.PIC` | Shape texture | `b1f65130b86336e1d68795802c29f2cafe050c004741301826a44309e98b2eb2` |

The Gripen is the one batch aircraft with a `.PTS` companion (B5-1 recorded it
and the identity reports `has_pts()` true; no slice needed it). The AI script
`f.BI` is present. The record's `typeSize` is 660 (made explicit in B5-1).
`GRIPEN.PT` is not a parked target, so `parked_aircraft.rs` needs no row and is
unchanged. The dependency report and extracted copies live only under
`.local/m3-imports/scratch/gripen/`. The selected CLI extraction used the
prebuilt `tore-extract` with `GRIPEN*`, `GRI*`, `_GRI*`, `~*`, the sensor and
JT names: 2,115 plus 36 resources and the usual five unrelated archive errors;
the app import reported no missing required reference.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#gripen-source-review-2026-11-10) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-gripen-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, infrared, HUD, cockpit, BK 27 and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `25a8c5b0`
(the Gripen branch `m3-gripen` was fast-forwarded into it at `f4afb766`).

### GRI-0 Source review (`7f9acd45`)

Research mode. Decoded `GRIPEN.PT` against the Rafale, F/A-18, F/A-18C and
Mirage records (27 statements differ from `RAFALE.PT` and `F18.PT`, 170 are
identical in all five); found a genuine JAS 39A record apart from the ceiling,
the -4 row, the gun count and the shared block; mapped the six state words of
`GRI.SH` (`8080 8086 808c 8098 809e 80a4`); measured the shape (-0.9 percent
span, -0.5 length, +1.6 height); ran the rule 11 fold check (every elevon,
rudder, airbrake, gear and canard face has 0 fixed vertices); found the canards
are fixed geometry (no state word), pentagonal whole plates plus a filler
triangle a side, and the `AAS38.SEE` row a placeholder.

### B5-1 Identity (`cbd9ce1e`, batch slice)

`AircraftId::Gripen` appended after `F15E`; checkpoint code 42; parse keys
`gripen` and `gripen.pt`; strike class Fighter (agent decision); HUD, stem and
picture bound by the record's name. New: `BK27.JT` is admitted as the station's
own internal gun (`own_gun`), in `guns()` and the live-fire lists and out of
`SUPPORTED_WEAPONS`, the catalog and the Cheat catalog (the earlier "no roster
aircraft mounts the decoded cannon" guard tests now expect exactly the Gripen and
the Tornado). Interim rig spec 28,848 CODE bytes, 383 faces, gear word `808c` +22
faces. Audit pins moved with one dated note (Gripen: 190 weapon placements, 12
tank placements, one recovered station).

### GRI-2 Shape and skin (`ac542c9c`)

True size, so no scale. 383 runtime faces; 19 exact twin groups (18 double-sided
panels with opposite stored normals, no MiG-21-style exception; the 19th is a
zero-area sliver that draws nothing). Five `E0` decal slots (nose art, two fin
sides, two wing marks) stay omitted by the projector; each lies on or within one
unit of drawn skin, so nothing is clipped, filled or blank. Damage looks A to D
load with only their own textures; no damage variant is registered (regional
cuts on the intact skin, as for the MiG-25, MiG-31, F-16A and Mirage 2000);
breakup pieces use variant 1. `render-gripen-skin` passes.

### GRI-4 Gear, airbrakes, burner (`6224e4b0`)

Word map: `8080` plume (`63af 63d6 63fd 6424`), `8086` airbrakes (`62ad 62c4`
right, `634a 6361` left), `808c` gear (22 faces), `8098` and `809e` flap words
and `80a4` rudder word pinned at load and never set (their strips stay drawn),
`8092` skipped in the source. No hook, slat, canopy, nozzle or canard word.
Gear (fitted): each main leg is one rigid piece turning 71.5 degrees about a
horizontal line at 35 degrees to X, folding forward and inboard; the door pair
folds a half turn plus 125.8 degrees; the nose group folds rearward 100 degrees
and steers; gear 1 is the source pose, low point -16 units. Each airbrake plate
turns whole 52.7 degrees about its forward vertical edge. One plume, one glow, one
contrail (`engine_material` faces `49a6`, `49d5`, `outlet_count` 1).

| Part (faces) | Vertices | Turned | On axis | Fixed off axis |
| --- | --- | ---: | ---: | ---: |
| Leg (4 faces a side) | 4 each | 4 | 0 | 0 |
| Door wall (2 faces a side) | 4 each | 4 | 2 | 0 |
| Door root plate (2 faces a side) | 4 each | 2 | 2 | 0 |
| Nose side plate | 3 | 3 | 1 | 0 |
| Other nose plates | 4 each | 4 | 0 | 0 |
| Airbrake plate (2 faces a side) | 4 each | 2 | 2 | 0 |
| Plume | scaled from y -61 | | | 0 |

Probe: 383 neutral faces, 0 failures, 202 dense gear rows (rigidity at most
1.4e-6 ft, ground contact at gear 1), 7 brake rows.

### GRI-3 Canards, elevons, rudder (`1a3c7b25`)

Per side one angle for the inboard and outboard elevon strip: `-0.30 * elevator`
minus or plus `0.20 * aileron`, clamped to 0.50 rad, plus `0.40 * flaps`, about
the hinge line y -27, z -1.5. Canards: `+0.25 * elevator` minus or plus
`0.15 * aileron`, sum clamped to 0.35 rad, plate plus root filler as one panel per
side about a spindle through the root chord midpoint (decision 9). Rudder:
`0.35 * rudder` about the vertical edge x 0, y -43. All magnitudes are fitted
agent decisions; the canards taking roll is John's.

| Panel | Vertices | Turned | On hinge | Fixed off axis |
| --- | --- | ---: | ---: | ---: |
| Canard plate | 5 | 5 | 0 | 0 |
| Canard filler | 3 | 3 | 0 | 0 |
| Elevon, inboard strip | 4 | 4 | 0 | 0 |
| Elevon, outboard strip | 3 | 3 | 0 | 0 |
| Rudder quad | 4 | 2 | 2 | 0 |

No face folds. Probe: 383 neutral faces, 0 required or check failures, elevator,
aileron, rudder, flaps, gear, brake and exhaust all `required moved`, anchor gap
at most 4e-8 ft, skin gap 0, rigidity at most 1.3e-6 ft, crossings 0, and all 25
pitch and roll poses pass. A mutation check (flipping the canard roll sign) made
the probe fail with 32 direction failures, then was reverted. Measured openings
(not gated): the **inboard elevon root slit** is 0.40 ft at roll, 0.60 ft at full
pitch, 0.80 ft at flaps, 0.99 ft at pitch plus roll and 1.75 ft at the extreme
corner (0.90 rad); canard root 0.10 ft at roll, 0.17 ft at pitch, 0.23 ft at the
bound; rudder 0.81 ft.

### GRI-5 Stations and pylons (`5a32d99e`, `d8c6725b`)

The first pass measured the decoded rows under the standard pylon rules and
stopped on clipping with frames; round 2 (`d8c6725b`) recorded John's keep
decisions. No station, mask, pylon or count changed.

| Row | Location (ft right / up / forward) | Default | Hang | Result |
| ---: | --- | --- | --- | --- |
| 0 to 3 | sensors, AAS-38 placeholder | `VIS340`, `F18R`, `F18.ECM`, `AAS38` | nothing drawn | AAS-38 body not drawn (decision 8) |
| 4 | gun, x +1.33 starboard | `BK27` x570 | internal gun | 1.1 ft inside the skin; kept on the right (B3) |
| 5 | 0 / -6.67 / 0 | `F150.GAS` x1 | 2 in rail | gear up clean (0.16 ft); gear down crosses the left main strut plate (0.25 ft; accepted, B2) |
| 6 | 5.00 / -5.00 / -4.67 | `AGM65G` x2 | 1 ft pylon | gear up clean (0.59 to 0.64 ft); gear down crosses the open door wall (accepted, B1) |
| 7 | 10.00 / -5.00 / -7.00 (two a side) | `AIM120` x4 | 1 ft twin bar | clean gear up and down; twin fins 0.02 ft apart (standard) |
| 8 | 14.67 / -0.67 / -8.67 (tips) | `AIM9M` x2 | tip rule | 0.15 ft fin entry into the thin tip (listed, below every reviewed default) |

Canards are at least 2.56 ft from every store at neutral. No store touches the
ground. Of 61 single-row alternatives, row 5 tanks and bombs enter the belly 0.13
to 0.97 ft, row 6 and row 7 have a few over the line (AA-6, AM-39, AS-30, AGM-88,
GBU-10) and every row 8 store enters the tip (0.15 to 1.33 ft); no refusals were
requested (round 2: keep, no changes). Real stations (seven in the in-game
encyclopedia, eight on public pages, conflicting) for plausibility only. The
AAS-38's 350 lb is not in the mass sum (rows flagged 0x8 are skipped, as for the
A-10 and F-16C). Audit pins unchanged by this slice.

### GRI-6 Ordnance, default load, countermeasures (`f3a3dbf5`)

No gameplay code changed: B5-1 had put `BK27.JT` in the gun and live-fire lists,
which is all the shared gun path needs. BK 27 values: 27 mm; 570 rounds (row 4,
John kept it; real 120); muzzle velocity 2,933 ft/s (the shared record value; real
about 3,600); 2 rounds per shot; the fitted shared cadence of 32 rounds a second
(real 1,000 to 1,700 rounds a minute, one gun); own sound `&FSTGUN3.11K`; red
tracer every third round. Default load (human, AI, saved, lobby): BK27 x570, one
`F150.GAS`, two `AGM65G`, four `AIM120`, two `AIM9M`; stores 4,210 lb with the
tank fuel. `--validate-ordnance`: `GRIPEN.PT: 5 source rows, 1 optional weapon
rows, 190 compatible weapon placements, 12 tank placements; defaults and restart
passed`. Gun 320 shots, 59 hits, 1 kill; strafe key 64 rounds in 2 s; AIM-9M and
AIM-120 hit and kill; AGM-65G designated, launched, killed; each store leaves its
own station, left then right; jettison tank then bombs, no explosion.
Countermeasures 30 chaff and 30 flares from `F18.ECM` (real fit unverified). A new
retail-reading test pins the nine rows and the BK27 fields.

### GRI-7 Sensors, HUD, cockpit, instruments (`db354d88`)

The Gripen's forward picture is 1,279 wide, so before this slice the mirror flood
and the HUD aperture (both demanding 1,280) left `NO MIRRORS FITTED` and no HUD
glass. The slice measured the HUD glass polygon from the imported art (fitted),
seeded three mirrors (centre x 481 y 8 w 316 h 77; left and right x 169 / 959 y
174 w 151 h 267; one flat fill) and let `mirrors::masks` accept 1,279 x 490.
Sensors: radar `F18R.SEE` ADVANCED, infrared `AAS38.SEE` 15 nmi, visual
`VIS340.SEE`, jammer `F18.ECM` LATE, signature radar 100 infrared 90. Live fire:
AIM-120 `TWS 10NM 1 CONTACTS TRACK` hit and kill, AIM-9M hit and kill, gun 59
hits. Fuel reads `FUEL 5000 LBS (+ EXT 990 LBS)`. The eye point is the fleet's one
fixed offset (canopy top 2.3 to 2.7 ft above the origin).

### GRI-8 Flight model and validation (`a141a4d3`)

Measurement only; no model, record or shared constant changed. See the
[flight envelope](#flight-envelope) and the
[variety flight baseline](variety-flight.md#saab-gripen-validation-2026-11-10).
`variety_flight`: `conventional=pass deterministic=pass landing=pass ticks=1200`.
Liftoff, liftoff-retail and approach scenarios added; the flight family passed 17
of 17.

### GRI-9 AI, Quick Mission, multiplayer, replay (`f4afb766`)

No non-test game code changed. `--ai-roster-probe-ticks 3600`: four
`GRIPEN.PT` lines `models=2 radar=true ir=true stores=5 gun=true dropped=0
PASS`. `ai-gripen-quick-mission-2v2`, `replay-rec-gripen-2v2` and the menus lobby
notes scenario pass; `net-content-missing` and `net-window-lobby` pass after
rebuilding `tore-server` and `tore-bot`. The seeded checkpoint round trips draw
from `SELECTABLE`, so code 42 is covered (135 tests).

## Flight envelope

Measured on the default hybrid path (clean weight 19,599 lb with full internal
fuel; afterburner unless stated). Real figures are cited public pages for
comparison only; none is a flight manual.

| Quantity | Measured | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,090 kt (Mach 1.90); 1,065 kt at 50,000 ft | 1,145 kt (Mach 2.0) | Mach 2 at 50,000 ft |
| Top speed at sea level | 704 kt at 1,000 ft (Mach 1.07) | 739 kt | Mach 1.15 to 1.2 |
| Level speed to edge ratio | 0.952 at every altitude (0.935 at 60,000 ft) | | |
| Ceiling | 66,385 ft zoom (68,470 on `climb`); level to 60,000 ft | 60,000 ft | 50,000 ft |
| Peak Ps | 445 ft/s (26,700 ft/min); best sustained 427 to 432 ft/s | | not found for the C and D |
| Climb | 36,000 ft in 107 to 123 s | | |
| Acceleration at 1,000 ft | 363 to 460 kt in 9 s, to 590 kt in 27 s | | |
| G at 461 kt, 1,000 ft, full / half / quarter / near-empty fuel | 8.00 / 8.52 / 8.80 / 9.03 (default load 7.26, push -3.22) | +9 / -4 | +9 / -3 |
| Instantaneous turn | 22.5 deg/s at 330 kt (7.06 G); 18.6 deg/s at 461 kt | | not found |
| Sustained turn (scripted) | 4.8 G, 10.4 deg/s at 461 kt | | not found |
| Fuel | 41.7 min dry, 6.0 min afterburner; 608 nm cruise | flows 2 / 14 lb/s | about 29 and 9.3 min |
| Liftoff | 132.1 kt, 1,097 ft (retail speeds 105.7 kt, 695 ft) | | takeoff 400 m (Saab minimum) |
| Approach / touchdown | 147.5 / 142.0 kt, rollout 589 m | | touchdown just under 162 kt; landing 500 to 600 m |

The hybrid default is the validated path; `--legacy-flight` and the restricted
`--native-flight-tables` path stay distinct and unchanged. Every air maneuver
reported `non_finite=0`. The liftoff (125 to 150 kt) and approach (140 to 160 kt)
battery windows are fitted plausibility ranges, with recorded values 132.1 and
147.5 kt.

## Open findings for John

Nothing here was changed; each is a decision or a fleet-level model question.

1. **The ceiling stays 60,000 ft** (John, decision 10). The model holds 60,000 ft
   level and zooms to 66,000 to 68,000 ft against a real 50,000 ft. A ceiling
   fit as for the F-5E would be one corrections row.
2. **Gun: 570 rounds kept** (decision 10; real 120). At the shared cadence the 570
   last 17.8 s against about 4 s for 120 real. Cadence (about 13 percent above
   the real top rate) and muzzle velocity (about 18 percent below) are shared gun
   fields, not Gripen specific.
3. **Elevon root slit.** The inboard elevon strips are 2 ft deep at the root, so
   the slit reaches 0.6 ft at full pitch and 1.75 ft at the extreme corner. John
   accepted it (E2); lowering the flap droop or the total bound would shrink it.
4. **Accepted clipping, gear down:** inner wing stores against the open door wall
   (B1), the centreline tank against the left strut plate (B2), and the 0.15 ft
   missile entry at the thin wingtip (listed, not decided).
5. **Gun row on the right.** Row 4 sits 1.1 ft inside the starboard skin; the real
   BK 27 is port. Kept (B3).
6. **Shared drag law and loaded-elevator rule.** The Gripen shows the Mirage's
   shape (peak Ps 445 ft/s at 0.92 thrust to weight; sustained turn two thirds of
   the F-16A's), and G rises 8.0 to 9.0 as fuel burns (highest of the three batch
   aircraft at full fuel). No published C or D climb rate exists, so the factor
   cannot be computed. A fleet-level drag or ram fit is John's call.
7. **Sea-level top speed** is Mach 1.07 in the model and 1.10 in the record
   against a published 1.15 to 1.2 (7 percent under at the edge); a
   `fitted_envelopes` fit is possible.
8. **Fuel flows** are the record's: dry 30 percent under and afterburner 56
   percent over the RM12's published consumption.
9. **Countermeasures** 30 and 30 are the retail `F18.ECM`; the real fit is
   unverified (BOL and BOP dispensers, no official total).
10. **Canard landing-brake bias** (GRI-0 option 2) was not applied; **nose gear
    direction** rests on one secondary source (the page 404s on direct fetch).
11. **The cockpit eye point is shared.** One fixed offset (7 ft up, 10 ft ahead)
    for every aircraft; against the Gripen's small canopy it sits about 4.5 ft
    above it. A seated eye per aircraft would be a fleet change. The HUD glass
    polygon is fitted, not retail-known.
12. **The RWR threat frame is not made.** No scripted path plots an emitter or
    missile on the RWR; the code is aircraft-agnostic and unit-tested.
13. **The AI uses the infrared row of `AAS38.SEE`** (`ir=true`); no AI behaviour
    was changed to use it.
14. **A Gripen in live multiplayer flight was not played.** The net scenarios are
    aircraft-agnostic.
15. **Frames index.** The frame pack is indexed in the local `INDEX.md` (appended
    at X-10).

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extras on elevons, applied here to the
elevons and canards).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 42; every named resource present (a `.PTS` exists, unlike the Mirage); record audit done (genuine JAS 39A, template values listed); no alias; import report clean; not a parked aircraft |
| Look | Met | Own shape at 1:1 (true size), damage looks A to D, `render-gripen-skin` passes, no show-through or clipped marking in the frames. The destroyed whole-body variant is unavailable (no damage variant registered); the five decal slots stay omitted without clipping |
| Whole-surface gate | Met | Every canard, elevon strip, rudder, airbrake, gear and plume piece is a whole panel; the fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity about 1e-6 ft in every pose. The elevon root slit is inherent to rigid panels (accepted, E2) |
| Animation | Met | Signed pitch, roll and rudder; canards pitch and roll (decision 9); elevons; flaps (a droop on the flap lever, since the flap words only remove the strips); rigid gear with near-stow; airbrakes and burner; neutral equals source; probe `reviewed-controls-and-attachments` 0 failures; the 25-pose grid passes with `skin_gap_ft=0`; `flight-animation-gripen` passes |
| Stations | Met | Decoded rows measured under the standard pylon rules, clipping stopped and decided by John (B1 to B3 accepted, round 2); the 0.15 ft tip entry is listed |
| Ordnance | Met | Every default store permitted live and released from its own station (BK 27 as the station's own gun); compatibility, even pairs, tanks, jettison and the AI default checked; 30 / 30 countermeasures from `F18.ECM`; `--validate-ordnance` passes for all 45 aircraft |
| Cockpit | Met | `GRIPEN.HUD` and the `~GRI*` family by name (decision 7); fitted HUD glass, mirrors, overlays and ten instrument pages checked; reviewed sensor presets only; no pod body on the AAS-38 row (decision 8). The RWR with a live threat was not demonstrated (finding 12) |
| Flight | Met | Own configuration on the hybrid default; envelope and top speed reviewed; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally); the ceiling and drag findings are open for John |
| AI and modes | Met | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby. A Gripen in live multiplayer flight was not played |
| Docs | Met | Source review, specs with provenance, this record, inventory, features and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: elevons and canards | Met | Pitch, roll and the 25-pose grid pass with `skin_gap_ft=0` and rigid panels; every moving surface has a rule 11 face table above |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `GRI-2` to `GRI-9`
directories (skin and damage views, canard, elevon and rudder poses, gear,
airbrake and burner stages, loaded stores and clipping close-ups, ordnance and
countermeasure frames, cockpit, HUD and instrument pages, and the AI, Quick
Mission and replay frames). Observed points: no underside show-through or
clipped marking; no half-moved panel; the canards tilt whole and stay attached;
the elevon strips tilt as wedges with the root slit visible as a dark triangle;
the gear tucks as rigid plates; the single plume and glow ring draw; the wingtip
AIM-9Ms straddle the thin tip; and the cockpit and Quick Mission pages name the
exact Gripen identity. The frames are local evidence and are not committed.

## Full checks, 2026-11-10

One run for the three batch aircraft (F15E-10, GRI-10 and F5E-10) on the merged
`m3-aircraft` head `2c3d4d2b` plus this slice's documentation changes. Every
command ran through `.local/m3-imports/light3.sh` from the repository root (away
mode), one heavy command at a time, and the windowed commands through
`tools/agent-run.sh`. The logs are under `.local/m3-aircraft/checks/X-10-batch5/`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passing |
| `cargo build --locked --workspace` (then `-p tore-server` and `-p tore-session --bin tore-bot`) | Passing |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passing |
| `cargo test --locked --workspace` | 7,605 passed, 0 failed, 108 ignored across 76 result lines (`net::standby` passed, no rule 13 rerun) |
| Profile refresh `tore-app --import gameassets/fighters-anthology --import-only --no-audio` | Passing |
| `tore-app --validate-ordnance --no-audio` (`.local/m3-aircraft/profile`) | Passing: 45 aircraft, 18 recovered weapon stations, 4644 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed; 80 pod and 220 Cheat-only placements; `GRIPEN.PT: 5 source rows, 1 optional weapon rows, 190 compatible weapon placements, 12 tank placements` |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,256 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (377 tracked files; the new baseline carries the header and is checked when staged) |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| Battery, Gripen: `--scenario `*-gripen*`` (render, animation, flight, a2g, ai, replay and menus lanes) | 20 of 20 passed: render-gripen-skin; flight-animation-gripen, flight-variety-gripen, flight-liftoff-gripen, flight-liftoff-retail-gripen, flight-approach-gripen, live fire slots 1 to 5, jettison slots 2 to 4; a2g-release-origin-gripen, a2g-maverick-gripen, a2g-jettison-gripen; ai-gripen-quick-mission-2v2; replay-rec-gripen-2v2; menus-snap-quick-lobby-ordnance-laser-notes-gripen |
| Battery, shared pins: `menus-ordnance-availability`, `a2g-catalog`, `a2g-release-origin` | 3 of 3 passed |
| Battery, net (after rebuilding `tore-server` and `tore-bot`): `net-content-missing`, `net-window-lobby` | each passed, run one at a time |


## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, Gripen in live multiplayer flight or performance
measurement was run. `--native-flight-tables` needs the extracted `sine-q15.bin`
and `atan-pa.bin`, which are not present locally, so it stays a separate
restricted path and was not run. Canard clearance at deflected poses was
estimated, not measured. The flight and rendered evidence above comes from the
Linux development host and the wrapped captures.
