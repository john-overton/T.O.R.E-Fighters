# F-5E Tiger acceptance record (M3 batch 5, 2026-11-10)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The F-5E Tiger (`F5E.PT`) is a selectable retail identity in
`AircraftId::ALL` (the 43rd entry) and `AircraftId::SELECTABLE` (the 44th),
appended at checkpoint code 43 and carried on the wire as `SELECTABLE` index 43
inside the 9-bit field (NET-ID9, protocol 83). It is not an alias for the
desert F-5E (`F5EE.PT`), the snow F-5E (`F5EV.PT`) or any other record: it has
its own `F5E.PT`, `F5E.SH` and rig, and it names shared resources only where
the record itself names them (`VIS340.SEE`, `M61.JT`, `F150.GAS`, `AIM9B.JT` and
`f29.HUD`). `F5EE.PT` and `F5EV.PT` stay separate parked-target records, out of
scope and checked unchanged (F5E-9). The record is not one template: thrust
(7,000 / 10,000 lbf total), two engines and the empty and maximum takeoff weights
are the real F-5E's, while the ceiling and sea-level speed (F-4 family), the G
row, the fuel (6,240 lb, 38 percent over the real), the F/A-18 radar and jammer
pair and the Sparrow on the wing pair are copies. John made decisions about it
in two rounds on 2026-11-10 and corrected the copies (below). No retail
comparison exists, so this record claims source-reviewed, extraction-supported,
headless-flight-supported and rendered-flight-supported status, not retail
parity.

## John's decisions

Every decision is opinionated and recorded as John's with its date; every other
choice in this record is an agent decision and is labelled as one. Round 1 was
the batch decision round (numbers 11 to 16 in the plan); round 2 and its
follow-ups are `.local/m3-aircraft/BATCH5-DECISIONS-2.md` ("for the f-5 keep",
follow-ups "follow recs").

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 11 | 2026-11-10 (round 1) | **HUD and cockpit follow the record**: `F29.HUD`, stem `F29`, `~F29H.PIC` and `~F29_P.PIC`, the X-29's panel (TORE flies no X-29). | [F-5E sensors, HUD and cockpit](../formats/aircraft-variety.md#f-5e-sensors-hud-and-cockpit-f5e-7-2026-11-10) |
| 12 | 2026-11-10 (round 1) | **Stabilators pitch only** (the ailerons roll). | [F-5E control surfaces](../formats/aircraft-control-surfaces.md#f-5e-control-surfaces-f5e-3-2026-11-10) |
| 13 | 2026-11-10 (round 1) | **Copied values follow the real F-5E** ("follow the RL F-5E"): fuel 4,400 lb, G top row +7 (the real +7.33 as a whole G), ceiling 51,800 ft, a closest-record radar, no internal jammer, no AIM-7 in the default load. Each cited. | [Aircraft records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10) |
| 14 | 2026-11-10 (round 1) | **Size stays 1:1.** | [F-5E size](../formats/aircraft-variety.md#f-5e-size-against-the-real-aircraft-for-john) |
| 15 | 2026-11-10 (round 1) | **Wing stations: keep the record's one pair.** | [F-5E stations](../spec/stores-on-pylons.md#f-5e-station-decisions-john-2026-11-10) |
| 16 | 2026-11-10 (round 1) | **Speed brake: rig the source's dorsal plate.** | [F-5E gear, speed brake and plume](../formats/aircraft-control-surfaces.md#f-5e-gear-speed-brake-and-plume-f5e-4-2026-11-10) |
| C1 | 2026-11-10 (round 2) | **Wingtip AIM-9B 0.32 ft tip entry: accepted.** | [F-5E station decisions](../spec/stores-on-pylons.md#f-5e-station-decisions-john-2026-11-10) |
| C2 | 2026-11-10 (round 2) | **Wing pair alternatives: no new refusals.** | same |
| C3 | 2026-11-10 (round 2) | **Flap droop kept as is** (0.40 rad). | same |
| D1 | 2026-11-10 (round 2) | **Radar: keep `F104R.SEE`** as the APQ-159 stand-in. | [records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10) |
| D2 | 2026-11-10 (round 2, "follow recs") | **Countermeasures: `F104.ECM`** (20 chaff, 20 flares, no deception jammer; the real F-5E carries an ALE-40 dispenser). | [stations](../spec/stores-on-pylons.md#batch-5-stations-tornado-outer-wing-mirage-f1-centreline-f-5e-defaults-b5-2-2026-11-10) |
| E4 | 2026-11-10 (round 2) | **The damage pairing stays** (nose A and B, tail C and D). | [F-5E shape, skin and damage looks](../formats/aircraft-variety.md#f-5e-shape-skin-and-damage-looks-f5e-2-2026-11-10) |

## Source identity and dependency closure

The source review is
[F-5E source review](../formats/aircraft-variety.md#f-5e-source-review-2026-11-10).
Retail media stays local; no extracted bytes are committed. The archive hashes
were recomputed on 2026-11-10 (X-10) and match the earlier reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |

The selected closure (the `FA_2.LIB` copies; the cockpit pictures are in
`FA_1.LIB`). Retail record values; B5-2 and B5-3 corrections are applied where
the record is read, never in the decoder or the catalog:

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `F5E.PT` | Record, 219 statements, `typeSize` 612, seven hardpoints | `7f0026d5059b1cc037be856d784dcbc7d9d4cca6c5f634994c1731ce105fbae2` |
| `F5E.SH` | Shape, 253 runtime faces (259 gear down), six state words | `3c7cbd3e403d95e4dd52a7669caed2999509377a75b4077b9cad0e115bd0845f` |
| `F5E_S.SH` | Shadow shape | `4c73623088802890f0655229984d463a1c85023fa2476d05cabcac30507da412` |
| `F5E_A.SH` | Damage body A (228 faces) | `4bf3099db775b99e19e210aaa889eb4ac20e36291c4952f6145e916f42c75b1c` |
| `F5E_B.SH` | Damage piece B (128 faces) | `660312074e9281b5c5749aa21069952abaaa3eab8ec4136bd3f9fa788613c15f` |
| `F5E_C.SH` | Damage body C (234 faces) | `6839fa53462ebabfe64091b0fae46f0e1c21ed657de41071b15638e7406f71b1` |
| `F5E_D.SH` | Damage piece D (24 faces) | `36ee7a9871313c6ce063521b68993a8a1e779228a427746f23bcc881b8db41df` |
| `F104R.SEE` | Radar (B5-2 stand-in for the record's `F18R.SEE`, John D1) | `aa7bf804a01918c05ff78bfb1c7f6adddd5cf5017b9545294e62f952d9cbcd15` |
| `F104.ECM` | Jammer row (B5-3, John D2; the record names `F18.ECM`) | `c8f5c8e4ab57d31674d99a06cbf2a4e48aaa59a8d8619bea04ec784db2833e27` |
| `VIS340.SEE` | Visual sensor | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `M61.JT` | Internal gun, 560 rounds | `0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d` |
| `F150.GAS` | Default centreline tank | `ca9054921aa93eb890801a5a1f64deb55c8bcc6d03854ff03f1b42833a9f4c95` |
| `AIM9B.JT` | Default wingtip store | `89d473fdaeae6a7e2b0b018bc90ee08d0cfedb94a03543216551d07ed0744260` |
| `F29.HUD` | HUD and cockpit pointers (named by the record as `f29.HUD`) | `1b6591c70dc7b166b7515f7014bd10c2a614184399a7003431bb99b9b900ef3c` |
| `~F29H.PIC` | Forward cockpit | `93cc88a61cf37dbf0bad0c4ef8fa54e30f563ee2c41930e37de33bd7fc92103f` |
| `~F29_P.PIC` | Instrument panel | `2723b952bfb9bd08d2dba4048fc6574d2e45b9a189c3fff21d90caaeb4ce7445` |
| `_F5E.PIC` | Shape texture | `62e8ee3a779683533996d19a337c5c491109573de86fce70ad154f14f2c2f44f` |

The record's default `AIM7.JT` on the wing pair is cleared by B5-2 (John's
decision 13), and the `F18R.SEE` and `F18.ECM` rows are replaced as above. The PT
names no `F5E.PTS` companion, so the absence is an explicit exception. The AI
script `f.BI` is present. The importer's `_F5E` prefix match also brings in the
byte-identical `_F5EE*` skins and the `_F5EV*` skins; they are never read, and
`F5EE.SH` and `F5EV.SH` differ from `F5E.SH` (a test pins it). `F5E.PT` is not a
parked target; the parked `F5EE.PT` (gear word `0x639c`) and `F5EV.PT` (`0x614c`)
rows are unchanged and verified (F5E-9). The dependency report and extracted
copies live only under `.local/m3-imports/scratch/f5e/`. The app import reported
no missing required reference and the usual five unrelated archive errors.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#f-5e-source-review-2026-11-10) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-f5e-skin` scenario |
| Systems partially supported | Yes | Reviewed radar stand-in, ECM, visual, HUD, cockpit and stores; no infrared sensor row; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `25a8c5b0`
(the aircraft branch `m3-f5e` was merged by `2c3d4d2b`).

### F5E-0 Source review (`67db9ed5`)

Research mode. Decoded `F5E.PT` (219 statements, `typeSize` 612, seven hardpoint
rows) against all 145 PT records: no single template (best 191 of 219 shared, the
Mirage 2000). Thrust, engine count and the two weights are real; the ceiling and
sea-level speed (F-4 family), the structure row, the G +8 row, fuel 6,240 lb, the
F/A-18 radar and jammer and the default AIM-7 are copies. `f29.HUD` is the X-29's
and no F-5E cockpit art exists. Shape within 10 percent of real (span +7.5
percent, +10 with rails; length -4.9; height +4.9). Six state words `63c0 63c6
63cc 63d8 63de 63e4`; the fold check found every moving surface a whole panel
with 0 fixed vertices; the stabilators are free flat plates. `F5E.PT` is not
parked.

### B5-1 Identity (`cbd9ce1e`) and B5-2, B5-3 corrections (`179cac30`, `4ba24b77`)

B5-1 appended `AircraftId::F5E` (checkpoint code 43, parse keys `f5e` and
`f5e.pt`, strike class Fighter by agent decision); `typeSize` 612 needed a new
arm in `aircraft.rs`. Interim rig spec 21,488 CODE bytes, 253 faces, gear word
`63cc` +6 faces. HUD, stem and picture bound by the record's name; the glass
aperture was empty (fail closed) until F5E-7. B5-2 applied decision 13: fuel
4,400 lb and `maxAlt` 51,800 ft as `CORRECTIONS` rows (the top edges of the 0 G
and 1 G polygons shift by +9,800 ft, guarded on the corrected `maxAlt`); the +8
row (a repeat of the -4 row) retired so the top row is +7; the AIM-7 cleared from
the wing pair; radar `F104R.SEE`; the jammer row cleared. B5-3 (D2) set row 2 to
`F104.ECM`. All guarded to the retail record and unit tested; the pins moved
once (a dated note each).

### F5E-2 Shape and skin (`89ac064b`)

No scale (decision 14). 253 runtime faces (259 gear down). 8 exact twin groups, all
with opposite stored normals (3 canopy centre plates, 2 stabilators, 2 fin plates,
the rudder). Two `E0` decal slots, both on the wings (12 by 12 units each), stay
omitted by the projector; no tail or nose art exists, so **no national marking
draws on the wings**. Damage looks A (228 faces), B (128), C (234), D (24); the
agent pairing registers nose and cockpit with A and B and the tail with C and D,
wings and core keeping the fitted tears (E4: John kept it). `render-f5e-skin`
passes.

### F5E-3 Control surfaces (`e1ad8fa5`)

Stabilators `-0.30 * elevator` both sides about a lateral spindle through the root
chord midpoint, no roll (decision 12). Ailerons `-0.20 * aileron` (positive
raises the right) about y -5, z -3.5. Flaps `+0.40 * flaps` about (7,-4,-3.5) to
(31,-5,-3.5). Rudder `0.35 * rudder` about (0,-31,4), quad both windings. All
fitted agent decisions with fleet magnitudes except the cited ones; the flap
words are pinned and never set (the strips stay drawn).

| Panel | Vertices a face | Turned | On axis | Fixed off axis |
| --- | --- | ---: | ---: | ---: |
| Stabilators (free flat plates) | 4 | all | 0 | 0 |
| Ailerons and flaps | 4 | all | hinge pair 0.5 unit off the mid axis | 0 |
| Rudder quad | 4 | 2 | 2 | 0 |

Probe: 253 neutral faces, 0 failures; the 25 pitch and roll poses pass; rigidity
6e-7 ft, skin gap 0, anchor gap 0. A mutation check (flipping the stabilator sign)
failed the probe and was reverted. Measured openings (not gated): aileron 0.20 ft
at full roll, flap root 0.93 ft at full droop (kept, C3), rudder top corner 0.35
ft. The real leading-edge flaps are not in the source and are not drawn.

### F5E-4 Gear, speed brake, burners (`c36f460a`, `507aa8f2`)

Every word is accounted for: `63c0` plume, `63c6` brake, `63cc` gear; `63d8`,
`63de`, `63e4` pinned (F5E-3). Mains turn 90 degrees about their top edge and
swing inward; the nose plate turns forward and up (an agent decision: secondary
sources conflict) and steers; gear 1 is the source pose, low point -18 units. The
source's one dorsal brake plate (decision 16) turns 0.866 rad (49.6 degrees)
about its front edge onto the dorsal ridge. Eight plume plates scaled from the
exit plane y -48; two glows and two contrails (`engine_material` faces `42a6`,
`4454`, `outlet_count` 2).

| Part (faces) | Vertices | Turned | On hinge | Fixed off axis |
| --- | ---: | ---: | ---: | ---: |
| Right main `49cc 49eb` | 4 | 2 | 2 | 0 |
| Left main `4a37 4a56` | 4 | 2 | 2 | 0 |
| Nose `4aa2 4ac1` | 4 | 3 | 1 | 0 |
| Brake `4802 4819` | 4 | 2 | 2 | 0 |
| Plume (8 plates) | scaled from y -48 | | | 0 |

Probe: 202 dense gear rows pass (rigidity at most 1.6e-6 ft, ground contact at
gear 1), 7 brake rows (closed free edge 2.5e-6 ft off the ridge). A mutation check
(wrong main direction) failed and was reverted. `507aa8f2` restored the F5E-3
notes as written (B11).

### F5E-5 Stations and pylons (`f9062f53`, `c6deb5ec`)

The first pass measured the rows under the standard pylon rules and stopped on
the tip clipping; round 2 recorded John's keep decisions (`c6deb5ec`). No station,
mask or pylon code changed.

| Row | Location (ft right / up / forward) | Default | Hang | Result |
| ---: | --- | --- | --- | --- |
| 0 to 3 | sensor, radar, jammer, gun | `VIS340`, `F104R`, `F104.ECM`, `M61` x560 | internal | nothing drawn, no pod row |
| 4 | 0 / -4.67 / 0 | `F150.GAS` x1 | 2 in rail | clean: 0.17 ft off the belly, 2.58 ft off the ground, 3 ft or more from the gear |
| 5 | 7.33 / -4.67 / 1.00 (wing pair, 4 stores) | empty (B5-2) | 1 ft twin-rail pylon | defaults clean; of 64 alternatives 8 enter the wing (0.07 to 0.23 ft), 13 cross the neighbour on the pylon, 4 touch the gear sheet (`AIM7`, `AIM7E`, `GBU16`, `GBU30`); no new refusals (C2) |
| 6 | 14.33 / -1.00 / -1.00 (tips) | `AIM9B` x2 | tip rule | **0.32 ft entry each side** (122 and 148 crossings): accepted (C1) |

At full flap (0.40 rad) the drooped flap crosses `R550`, `AGM45`, `GBU12` and `AA6`
on the wing row; the AIM-9B stays 0.14 ft clear (C3 kept the droop). Real stations
(cited): the real F-5E has two more underwing pylons; the record keeps one pair
(decision 15). Audit pins unchanged by this slice (4,652 weapon placements, 233
tank placements).

### F5E-6 Ordnance, default load, countermeasures (`f45e6f69`)

No gameplay code changed. Default load: M61 x560 (two M39A2 of 280 each), one
`F150.GAS` (990 lb), an empty wing pair, two `AIM9B`; 15,365 lb of 24,664 lb, fuel
4,400. The empty wing pair keeps a retained selection station (ammunition vector
`[560, 2, 0]`), so `--a2g-run` needs `--station 2`. `--validate-ordnance`:
`F5E.PT: 4 source rows, 1 optional weapon rows, 71 compatible weapon placements, 5
tank placements; defaults and restart passed; catalog 61 normal, 83 Cheat`.
Countermeasures 20 chaff and 20 flares from `F104.ECM`. Gun 320 shots, 47 hits, 1
kill; strafe key 64 rounds in 2 s; AIM-9B one shot, one hit, no kill; Mk 82 from the
wing pair at 8.02 then 6.64 ft out; AGM-65G kills the lead truck; jettison tank
then four Mk 82, no explosion. A new retail-reading test pins the seven rows and
the `F104.ECM` counts.

### F5E-7 Sensors, HUD, cockpit, instruments (`44ded78f`)

TORE flies no X-29, so the F-5E is the first use of the `~F29*` family: the cockpit
already drew; what was missing was the HUD glass (empty mask, no HUD) and the
mirror seeds (three dead mirror fills). The slice measured a 15-vertex glass
polygon from the art (58,556 glass pixels, fitted) and seeded the mirrors (centre
(640,70), left (120,420), right (1160,420), fitted). Sensors: radar `F104R.SEE`
search 50 nmi, track 25 nmi, BASIC; **no infrared sensor**; visual `VIS340.SEE`;
jammer `F104.ECM` EARLY strength 0.00 (no RF mode). Live fire: AIM-9B `TWS 10 ...
TRACK`, `shots=1 hits=1 kills=0`. Fuel reads `FUEL 4400 LBS (+ EXT 990 LBS)`.
The eye point is the fleet's one fixed offset, 3.7 ft above the F-5E's small
canopy.

### F5E-8 Flight model and validation (`e45a6442`)

Measurement only; no model, record or shared constant changed. See the
[flight envelope](#flight-envelope) and the
[variety flight baseline](variety-flight.md#northrop-f-5e-validation-2026-11-10).
The flight family passed 13 of 13; liftoff, liftoff-retail and approach scenarios
added.

### F5E-9 AI, Quick Mission, multiplayer, replay (`6f87eba6`)

No non-test game code changed. `--ai-roster-probe-ticks 3600`: four `F5E.PT`
lines `models=2 radar=true ir=false stores=3 gun=true dropped=0 PASS`.
`ai-f5e-quick-mission-2v2`, `replay-rec-f5e-2v2` and the menus lobby notes
scenario pass; the creator lists neither `F5EE.PT` nor `F5EV.PT`;
`the_f5e_round_trips_at_code_43_and_no_identity_shares_it` pins the checkpoint
code; `net-content-missing` and `net-window-lobby` pass after rebuilding
`tore-server` and `tore-bot`. The parked check (`--surface-parked`) shows two
`F5EE.PT` and two `F5EV.PT` with their own gear words, unchanged.

## Flight envelope

Measured on the default hybrid path (clean weight 13,958 lb with full internal
fuel; afterburner unless stated). Real figures are cited public pages and the
retail text for comparison only; none is a flight manual.

| Quantity | Measured | Retail record (corrected) | Real (cited) |
| --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,021 kt (Mach 1.78) | 1,090 kt (Mach 1.90) | Mach 1.63 (939 kn) |
| Top speed at sea level | 685 kt at 1,000 ft (Mach 1.04) | 731 kt | not found |
| Level speed to edge ratio | 0.937 at every altitude | | |
| Ceiling | level to 51,800 ft; zoom 52,214 ft | 51,800 ft | 51,800 ft |
| Peak Ps | 332 ft/s (19,900 ft/min); sustained 314 to 316 ft/s | | 34,500 ft/min (55 to 58 percent) |
| Climb | 36,000 ft in 138 to 159 s | | |
| Acceleration 363 to 590 kt at 1,000 ft | 37 s (460 kt in 12 s) | | |
| G at 461 kt, 1,000 ft, fuel 4,400 / 2,200 / 1,100 / 300 lb | 5.98 / 6.49 / 6.78 / 7.00; push -2.55 to -2.99 | +7 / -4 | +7.33 |
| Instantaneous turn | 17.8 deg/s at 330 kt (5.95 G); 12.8 deg/s at 461 kt | | not found |
| Sustained turn (scripted) | 4.13 G, 9.6 deg/s at 461 kt; 2.5 G at 593 kt; 1.6 G at 30,000 ft | | not found |
| Fuel | 73.3 min dry, 9.2 min afterburner | flows 1 / 8 lb/s | about 30 and 12.4 min |
| Liftoff | 153.4 kt, 1,763 ft | | ground run about 2,000 ft |
| Approach / touchdown | 166.2 / 159.8 kt (65 percent fuel), stopped 703 m | | stall 124 kt; landing roll 2,500 to 3,700 ft |

The hybrid default is the validated path; `--legacy-flight` and the restricted
`--native-flight-tables` path stay distinct and unchanged. Every air maneuver
reported `non_finite=0` (the scripted `dive`, `overspeed` and `climb` end in an
overspeed loss). The liftoff (135 to 165 kt) and approach (155 to 175 kt) battery
windows are fitted plausibility ranges from one published stall speed, with
recorded values 153.4 and 166.2 kt.

## Open findings for John

Nothing here was changed; each is a decision or a fleet-level model question.

1. **6.0 G at full fuel.** The 4,400 lb tank is 46 percent of the empty weight, so
   the pull at full fuel is 5.98 G through the shared loaded-elevator rule and
   reaches the +7 row only nearly dry. The real +7.33 sits 0.33 G above the
   whole-G top row.
2. **The AI never fires its AIM-9B.** The record has no infrared sensor row, so
   the AI's attack waits in ACQUIRING and falls to the gun (29 rounds, no hit, no
   kill in the 2 v 2). Options: give the F-5E an IR seeker row, or let the AI
   acquire without one (AI behaviour, out of scope here). A player's single
   AIM-9B hits but does not kill the practice aircraft.
   **Update (B5-AI, 2026-11-11): this finding's cause was wrong.** The AI does
   fire its AIM-9B, and no infrared sensor row is needed to launch any infrared
   missile (the Mirage F1 and Tornado AIs fire theirs with none). The AIM-9B is
   a rear-aspect record (`weapon_corrections::infrared_aspect`, like the
   MiG-21's AA-2), so on a head-on pass it has no usable envelope; the AI shoots
   after the merge and the turn: in the 2 v 2 head-on from 5 nm the F-5E AIs
   fire at 135, 143, 153 and 162 s (the Mirage F1's all-aspect R550 at 13 s, the
   Tornado's AIM-9M at 13 s, the MiG-21's AA-2 from 190 s), and the 60 s battery
   fight ends first. With the record made all-aspect (temporary, reverted) the
   F-5E fires at 13 s, tick for tick the Mirage F1. Details and the options
   (leave it; have the AI turn for the rear hemisphere; or an all-aspect AIM-9B,
   not recommended) in [lane AI, B5-AI](../testing/lane-ai.md#b5-ai-the-f-5es-aim-9b-the-f-15es-role-and-the-ai-tornados-racks-2026-11-11).
3. **Stand-ins.** The radar is `F104R.SEE` (alternatives `F4BR.SEE`,
   `YAK141R.SEE`) and the countermeasures are `F104.ECM`, with no deception
   jammer; the real APQ-159 and ALE-40 fit are cited second hand only (John kept
   both, D1 and D2).
4. **Wingtip AIM-9B enters the thin tip by 0.32 ft** (accepted, C1; inside the
   0.27 to 0.72 ft range of the default loads reviewed so far).
5. **Top speed is 17 percent over the real Mach 1.63** in the record (Mach 1.90) and
   9 percent in the model (1.78): the retail polygon shape. No sea-level figure
   exists in the sources.
6. **Weak fast pulls.** The record's G rows are narrow at speed and altitude (2.6 G
   at 593 kt, 1.7 G at 30,000 ft), a retail shape; the B5-2 top-edge shift leaves a
   steep fast edge above 39,000 ft (586 kt at 50,000 ft) and the scripted `climb`
   ends in an overspeed loss after the zoom.
7. **Climb is 55 to 58 percent of the published 34,500 ft/min** and the sustained
   turn (4.13 G at 461 kt, equal to the Mirage's) follows from the shared drag law:
   drag equals the reference thrust at the top speed and grows with speed squared,
   no ram thrust. Matching the climb would take about 1.5 times the engines' thrust
   by hand arithmetic (not run). A fleet-level drag or ram fit is John's call.
8. **Fuel flows** are the record's: dry 59 percent under and afterburner 35 percent
   over the J85's published consumption.
9. **Rig and look gaps.** The nose gear folds forward from conflicting secondary
   sources; the single dorsal brake stands where the real aircraft has paired
   ventral brakes (decision 16); the real leading-edge flaps are not in the source;
   the flap root opens 0.93 ft at full droop (kept); the retail nose foot sits 0.33
   ft above the mains' ground line; both wing decal slots are omitted, so no
   national marking draws on the wings.
10. **The default load carries no bomb station** (the empty wing pair is the
    decision's consequence), so `--a2g-run` needs `--station 2`. Enemy F-5Es in the
    2 v 2 carry no tank (the existing enemy-wing default rule).
11. **HUD glass and mirror seeds are fitted** polygons and rectangles on the X-29
    art; the eye point is the fleet's fixed offset, 3.7 ft above the small canopy.
    A seated eye per aircraft would be a fleet change.
12. **The RWR threat frame is not made.** No scripted path plots an emitter or
    missile on the RWR; the code is aircraft-agnostic and unit-tested.
13. **Gun cadence** (about 32 rounds a second, shared) was not compared; no real
    F-5E rate was sourced.
14. **An F-5E in live multiplayer flight was not played.** The net scenarios are
    aircraft-agnostic.
15. **Frames index.** The frame pack is indexed in the local `INDEX.md` (appended
    at X-10).

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extras on elevons, applied here to the
stabilators and ailerons).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 43; `typeSize` 612 handled; every named resource present (`F5E.PTS` an explicit exception); record audit done (copies listed and corrected by John); no alias of `F5EE.PT` or `F5EV.PT` (their parked rows unchanged); not a parked aircraft; import report clean |
| Look | Met | Own shape at 1:1 (decision 14), damage looks A to D with the kept pairing (E4), `render-f5e-skin` passes, no show-through or clipped marking in the frames. The destroyed whole-body variant is not registered; both wing decal slots stay omitted, so the wings carry no marking |
| Whole-surface gate | Met | Every stabilator, aileron, flap, rudder, brake, gear and plume piece is a whole panel; the fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity about 1e-6 ft in every pose. The flap root opening is inherent to rigid panels |
| Animation | Met | Signed pitch (pitch-only stabilators), roll, rudder; flaps; rigid gear with near-stow; the dorsal brake and two burners; neutral equals source; probe `reviewed-controls-and-attachments` 0 failures; the 25-pose grid passes with `skin_gap_ft=0`; `flight-animation-f5e` passes. No leading-edge flap, hook, canopy or door word exists in the source |
| Stations | Met | Decoded rows measured under the standard pylon rules, clipping stopped and decided by John (decision 15, C1 to C3); the 0.32 ft tip entry is accepted |
| Ordnance | Met | Every default store permitted live and released from its own station; compatibility, even pairs, tanks, jettison and the AI default checked; 20 / 20 countermeasures from `F104.ECM`; `--validate-ordnance` passes for all 45 aircraft. A single AIM-9B hits without killing the practice aircraft (finding 2) |
| Cockpit | Met | `F29.HUD` and the X-29 panel by name (decision 11); fitted glass polygon and mirror seeds, overlays and ten instrument pages checked; reviewed sensor presets only; absent systems absent (no infrared row). The RWR with a live threat was not demonstrated (finding 12) |
| Flight | Met | Own configuration on the hybrid default with John's corrections; envelope and top speed reviewed; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally); findings 1 and 5 to 8 are open for John |
| AI and modes | Met | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby; `F5EE.PT` and `F5EV.PT` not offered. The AI never fires its AIM-9B (finding 2); an F-5E in live multiplayer flight was not played |
| Docs | Met | Source review, specs with provenance, this record, inventory, features and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: stabilators and ailerons | Met | Pitch, roll and the 25-pose grid pass with `skin_gap_ft=0` and rigid panels; every moving surface has a rule 11 face table above |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `F5E-2` to `F5E-9`
directories (skin and damage views, stabilator, aileron, flap and rudder poses,
gear, brake and burner stages, loaded stores and clipping close-ups, ordnance and
countermeasure frames, cockpit, HUD and instrument pages, and the AI, Quick
Mission and replay frames). Observed points: no underside show-through or
clipped marking; no half-moved panel; the gear tucks as rigid plates; the dorsal
brake plate and two afterburner plumes draw; the centreline tank hangs clean
under the belly and the tip AIM-9B visibly straddles the thin tip; the cockpit
shows the X-29 panel with the HUD inside the glass; and the Quick Mission pages
name the exact "F- 5E Tiger" identity. The frames are local evidence and are not
committed.

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
| `tore-app --validate-ordnance --no-audio` (`.local/m3-aircraft/profile`) | Passing: 45 aircraft, 18 recovered weapon stations, 4644 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed; 80 pod and 220 Cheat-only placements; `F5E.PT: 4 source rows, 1 optional weapon rows, 71 compatible weapon placements, 5 tank placements` |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,256 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (377 tracked files; the new baseline carries the header and is checked when staged) |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| Battery, F-5E: `--scenario `*-f5e*`` (render, animation, flight, a2g, ai, replay and menus lanes) | 16 of 16 passed: render-f5e-skin; flight-animation-f5e, flight-variety-f5e, flight-liftoff-f5e, flight-liftoff-retail-f5e, flight-approach-f5e, live fire slots 1 to 3, jettison slot 2; a2g-release-origin-f5e, a2g-maverick-f5e, a2g-jettison-f5e; ai-f5e-quick-mission-2v2; replay-rec-f5e-2v2; menus-snap-quick-lobby-ordnance-laser-notes-f5e |
| Battery, shared pins: `menus-ordnance-availability`, `a2g-catalog`, `a2g-release-origin` | 3 of 3 passed |
| Battery, net (after rebuilding `tore-server` and `tore-bot`): `net-content-missing`, `net-window-lobby` | each passed, run one at a time |


## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, F-5E in live multiplayer flight or performance
measurement was run. `--native-flight-tables` needs the extracted `sine-q15.bin`
and `atan-pa.bin`, which are not present locally, so it stays a separate
restricted path and was not run. The tip rail drop options were estimated, not
measured. The flight and rendered evidence above comes from the Linux
development host and the wrapped captures.

## Update (B5-AI, 2026-11-11)

The AI pass found that the F-5E's AI fires its AIM-9B once it is behind its
target (finding 2's cause was the AIM-9B's rear-aspect record, not the missing
infrared sensor row; see the update under finding 2 and
[lane AI](../testing/lane-ai.md#b5-ai-the-f-5es-aim-9b-the-f-15es-role-and-the-ai-tornados-racks-2026-11-11)).
The earlier "the AI never fires its AIM-9B" lines above (the F5E-9 section, the
definition-of-done row) stand as written; the 60 s fight and the scripted
player's `no AIM-9B shot (ACQUIRING)` both show the head-on aspect, not a defect
in the F-5E's sensors or record. Whether the AI should seek the rear hemisphere
sooner is John's call (AI behaviour, fleet-wide).
