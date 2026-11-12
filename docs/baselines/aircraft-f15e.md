# F-15E Strike Eagle acceptance record (M3 batch 5, 2026-11-10)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The F-15E Strike Eagle (`F15E.PT`) is a selectable retail identity in
`AircraftId::ALL` (the 41st entry) and `AircraftId::SELECTABLE` (the 42nd),
appended at checkpoint code 41 and carried on the wire as `SELECTABLE` index 41
inside the 9-bit field (NET-ID9, protocol 83). It is not an alias for the F-15C
(`F15.PT`) or any other record: it has its own `F15E.PT`, `F15E.SH` and rig, and
it names shared resources only where the record itself names them (`VIS340.SEE`,
`F15R.SEE`, `F15.ECM`, `AV8IR.SEE`, `PAVESPK.SEE`, `M61.JT`, `F350.GAS`,
`MK82.JT`, `AIM120.JT` and `av8.HUD`). The record is the F-15C record with 17 of
219 statements changed (type and instance size, hardpoint count, year, flags,
debris, pitch-rate and puff words); mass, fuel, both thrusts, the 14 G rows and
the hit points are the F-15C's. John made decisions about it in two rounds on
2026-11-10 (below). No retail comparison exists, so this record claims
source-reviewed, extraction-supported, headless-flight-supported and
rendered-flight-supported status, not retail parity.

## John's decisions

Every decision is opinionated and is recorded as John's with its date. Every
other choice in this record is an agent decision and is labelled as one. Round 1
was the batch decision round ("F15: go with recs"); round 2 is
`.local/m3-aircraft/BATCH5-DECISIONS-2.md` ("The f-15 do recs", follow-ups
"follow recs").

| # | Date | Decision | Where it lives |
| ---: | --- | --- | --- |
| 1 | 2026-11-10 (round 1) | **Copied weights and thrust stay retail.** 28,600 lb empty, 32,558 / 47,860 lbf, ceiling 100,000 ft: the F-15C template, plausible for the early F100-PW-220 jets. | [Aircraft records](../spec/aircraft-records.md#batch-5-record-corrections-tornado-ids-mirage-f1-f-5e-b5-2-2026-11-10) (no F-15E correction row) |
| 2 | 2026-11-10 (round 1) | **HUD and cockpit follow the record**: `av8.HUD`, the same as TORE's F-15C (stem `AV8`, `~AV8H.PIC`, `~AV8_P.PIC`). No F-15E cockpit art exists. | [F-15E sensors, HUD and cockpit](../formats/aircraft-variety.md#f-15e-sensors-hud-and-cockpit-f15e-7-2026-11-10) |
| 3 | 2026-11-10 (round 1) | **Pave Spike row draws no pod body**; the sensor stays. | [F-15E pods](../spec/stores-on-pylons.md#the-f-15e-pods) |
| 4 | 2026-11-10 (round 1) | **No rudder motion** (no rudder geometry in the source). | [F-15E control surfaces](../formats/aircraft-control-surfaces.md#f-15e-control-surfaces-f15e-3-2026-11-10) |
| 5 | 2026-11-10 (round 1) | **Whole-panel stabilators with roll** (fleet standard taileron). | same |
| 6 | 2026-11-10 (round 1) | **Size stays 1:1** (within 6 percent of the real aircraft). | [F-15E shape](../formats/aircraft-variety.md#f-15e-shape-damage-looks-and-decals-f15e-2-2026-11-10) |
| A1 | 2026-11-10 (round 2) | **Belly bomb row (row 7) takes the 0.1 ft rail drop** (the Mirage 2000 precedent). | [F-15E decisions](../spec/stores-on-pylons.md#the-f-15e-decisions-f15e-5-2026-11-10-round-2) |
| A2 | 2026-11-10 (round 2) | **Refuse the large stores that enter the belly more than about 0.3 ft** on row 7: `GBU29`, `GBU29P`, `GBU30`, `AGM88`, `AGM45`, `AT2`, `GBU38`. | same |
| A3 | 2026-11-10 (round 2) | **Refuse the AA-6 on the wing row (row 8).** | same |
| E1 | 2026-11-10 (round 2) | **Flaps deflect 30 degrees** (cited), not the source branch's 46. | [flaps at 30 degrees](aircraft-animations.md#f-15e-flaps-at-30-degrees-f15e-5-2026-11-10) |

## Source identity and dependency closure

The source review is
[F-15E source review](../formats/aircraft-variety.md#f-15e-source-review-2026-11-10).
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
| `F15E.PT` | Record, 219 statements, nine hardpoints | `6c92d010665d8e23da21269783752568e30571660dadcec39688ba56e1e76989` |
| `F15E.SH` | Shape, 350 runtime faces (352 export), five state words | `d67183064e083578a9ac609d60e9cefe4b6973775ff6b9689f8495f97f3f4ba7` |
| `F15E_S.SH` | Shadow shape | `c83286459c491f2cadb05de3d906c2dca3360a8ae393397c3f7700c6566e8b89` |
| `F15E_A.SH` | Damage body A (311 faces) | `05aea1207c78d2406d1b86eff20837e7bb520a885a96f12ab471cf3cd8923326` |
| `F15E_B.SH` | Damage piece B (138 faces) | `60a3ed7292268d2c13ef1b9878a376e8c254c34956ff7a8b445de3de8184c7ea` |
| `F15E_C.SH` | Damage body C (350 faces) | `e921ecd8a47b8002267545766185a1f8166264240415f71aec60700d6c980669` |
| `F15E_D.SH` | Damage piece D (33 faces) | `aef081cf423ef05429bf4346e0f9fba83b18ac255551a5eeb96a91e40ddbef5b` |
| `F15R.SEE` | Radar (the F-15C's) | `ca8a70fcafc168f75e4dde71a2319fcda7ed9d81bbe9c499e4a0d30851e630fd` |
| `F15.ECM` | Jammer, chaff 30 and flare 30 (the F-15C's) | `4ad24a665be870b84cf0839dad1eed3ffc78142ff2dc6918705b874c3a54c693` |
| `VIS340.SEE` | Visual sensor | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `AV8IR.SEE` | Infrared channel (row 3, fixed placeholder) | `d4791bcb3faf93799cb49472757801834c49c9d0566da6ceb59ef40847ad0f60` |
| `PAVESPK.SEE` | Laser designator (row 4, fixed placeholder) | `7fd9c6d3e909015e8715056e5c0f13ce472cdb7fb70ac10bdcd4a54cb6e53b91` |
| `M61.JT` | Internal gun, 450 rounds | `0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d` |
| `F350.GAS` | Default centreline tank | `fde47be5904b2cb65a8dc2a33c7487ad0f8e1142f75748e358d9e1b4910233e6` |
| `MK82.JT` | Default belly store | `dc548de6c7bb72bed20919b2452d4e4666bc426ea0e1aeca412e4a875a380fe6` |
| `AIM120.JT` | Default wing store | `702d559376e544b49d2d118e1a8911227b0abe6560ff84340a7fd592c709d7a8` |
| `AV8.HUD` | HUD and cockpit pointers (named by the record as `av8.HUD`) | `369e32a63ac1e7b7cb54b8fe47407b44ea86e55c66de42a99f9245af8dc2ca75` |
| `~AV8H.PIC` | Forward cockpit | `46b08a208c44c386f42a96b98e5f033660bd0437bf49321374f3e0b5eba2d352` |
| `~AV8_P.PIC` | Instrument panel | `2bf9b792939d4afd845bb0b9a098c274c93c835352a12ee5a190a3c982d5d3c6` |
| `_F15E.PIC` | Shape texture | `0431a9f36124787d24641e587207a2e11b645b9ac6aadde5a05c44366978df6a` |

The PT names no `F15E.PTS` companion, so the absence is an explicit exception.
The AI script `f.BI` is present. The record's `typeSize` is 660 (the default,
made explicit in B5-1). `F15E.PT` is not a parked target, so `parked_aircraft.rs`
needs no row and is unchanged. The dependency report and extracted copies live
only under `.local/m3-imports/scratch/f15e/`. The selected CLI extraction was:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/f15e --include 'F15E.PT' --include 'F15.PT' \
  --include 'f15e*' --include 'F15*' --include '*.HUD'
```

It found 156 resources and the usual five unrelated archive errors; the app
import (`--import gameassets/fighters-anthology --import-only --no-audio`)
reported no missing required reference.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#f-15e-source-review-2026-11-10) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe`, `performance_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-f15e-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, infrared, laser designation, HUD, cockpit and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Acceptance by slice

One section per slice. Commits are on branch `m3-aircraft` since `25a8c5b0`
(the aircraft branch `m3-f15e` was merged by `6f0efcc9`).

### F15E-0 Source review (`18da52fe`)

Research mode. Decoded `F15E.PT` and `F15.PT` statement by statement (17 of 219
differ); found the record to be the F-15C template; measured the shape against
the real aircraft (span +5.9 percent, length -1.2, height +1.1 gear up and +26
gear down); mapped the five state words of `F15E.SH` (`80c0` plumes, `80c6`
dorsal brake, `80cc` gear, `80d8` and `80de` the flap words); ran the rule 11 fold
check (every moving surface a whole panel, 0 fixed vertices); listed the template
values, HUD, pod, taileron and size questions for John.

### B5-1 Identity (`cbd9ce1e`, batch slice for all five aircraft)

`AircraftId::F15E` appended after `M2000` in the enum, `ALL` (to 45),
`SELECTABLE` (to 46), the checkpoint code table (41), the parse keys `f15e` and
`f15e.pt`, the help texts, the live-fire default-store list, the strike class
(Fighter, agent decision), `stores_preview.rs`, the extraction key and the
interim rig spec (28,906 CODE bytes, 350 faces, gear word `80cc` +18 faces,
agent decision). HUD, stem and picture bound by the record's name. No record
correction applied. Audit pins moved with one dated note (F-15E: 65 weapon
placements, 3 tank placements).

### F15E-2 Shape and skin (`6c875f41`)

No scale (decision 6). 350 runtime faces, 17 exact twin groups, all with opposite
stored normals (no MiG-21-style exception). Two `E0` decal slots (3 on the left
wing, 4 on the right) stay omitted by the projector; nothing fills them and no
tail or nose art exists. The conformal tanks are not in the shape: the outline
matches the F-15C's within two units, and the only extra is a flat lower
fuselage shelf (x 16 to 21, about 1.7 ft), so tanks are stores. Damage looks A to
D load; no `damage_variant` is registered (as for the F-15C, MiG-25, MiG-31,
F-16A and Mirage 2000), so destruction draws the intact skin with regional cuts.
`render-f15e-skin` passes.

### F15E-4 Gear, speed brake, burners (`f4adfe65`)

Word map: `80c0` two flames (8 plates), `80c6` dorsal brake (4 faces), `80cc`
gear (18 faces), `80d8` and `80de` the **signed** flap words (+1 deletes the
inboard wedge, -1 swaps it for a lowered copy; pinned at load, never set). No
rudder, hook, slat, canopy, nozzle or gear-door word exists and none was
invented. Gear (fitted): each main leg is one rigid body folding 90 degrees
forward and flat into a well; the nose folds forward (cited, one source); inboard
doors close outward; gear 1 is the source pose with the low point 9 ft. The brake
turns 0.774 rad about y 25, z 11 so its free edge lies on the dorsal ridge. Two
plumes, four nozzle faces in `engine_material`, `outlet_count` 2 (two glows, two
contrails).

| Part (faces) | Vertices | Turned | On hinge | Fixed off axis |
| --- | ---: | ---: | ---: | ---: |
| Inboard doors `524f 5266`, `50e3 50fa` | 4 each | 2 | 2 | 0 |
| Main legs (4 faces a side) | 4 each | all | 0 | 0 |
| Nose (6 faces) | all | all | 0 | 0 |
| Brake `568c 56a3 56ba 56d1` | 4 each | rest | 2 on y 25 | 0 |
| Plumes (8 faces) | scaled from y -61 | | | 0 |

Probe: 350 neutral faces, 0 failures, 202 dense gear rows (rigidity at most
2.2e-6 ft, ground contact at gear 1), 7 brake rows.

### F15E-3 Control surfaces (`15e63547`)

Stabilators: five faces a side, whole panel about a lateral spindle through the
root chord midpoint, angle `(-0.30 * elevator - side * 0.10 * aileron)` clamped to
0.40 (the F-15C's law, decision 5). Ailerons: two faces a side, whole panel about
the line from the root pair's midpoint to the tip vertex, `-0.20 * aileron`.
Flaps: wedges as whole panels, `0.80 * flaps` at the time, later 30 degrees (E1),
with the `-1` branch's end triangle re-seated on the wedge at load. Rudder: none
(decision 4); the probe records the rudder as not applicable. All fitted agent
decisions except the decisions cited.

| Panel (faces) | Vertices | Turned | On axis | Fixed off axis |
| --- | --- | ---: | ---: | ---: |
| Stabilators `4b5f 4b86 4ba7 4c5d 4c8a` right, `42f1 4318 4339 43ef 441c` left | 4,3,4,5,4 | all | 0 | 0 |
| Ailerons `4b33 4ac4`, `42c5 41d9` | 4 each | all | 1 (the tip) | 0 |
| Flap wedges `5470 548f`, `5557 5576` and end triangles `5517`, `55fe` | 4 and 3 | all | 0 | 0 |

No face folds. Measured seam openings (not gated): aileron root pair 0.0998 ft at
full travel, outboard trailing corner 0.46 ft, flap front edge 0.39 ft (0.26 ft
after E1), stabilator aft root corner 1.44 ft at pitch 0.30 rad and 1.92 ft at
the 0.40 clamp. Probe: 0 failures, 25 of 25 pitch and roll poses and the three
21-value sweeps pass with anchor gap at most 8e-8 ft, skin gap 0, rigidity at
most 2.4e-6 ft.

### F15E-5 Stations and pylons (`c1f5f14d`, `422743b8`)

The first pass measured the decoded rows under the standard pylon rules and
stopped on clipping with frames. Round 2 (A1, A2, A3, E1) then settled it
(`422743b8`).

| Row | Location (ft right / up / forward) | Default | Hang | Result |
| ---: | --- | --- | --- | --- |
| 0 to 4 | fixed sensors and jammer | `VIS340`, `F15R`, `F15.ECM`, `AV8IR`, `PAVESPK` | nothing drawn | Pave Spike body not drawn (decision 3) |
| 5 | 2.3 / -1.7 / 24.3 | `M61` x450 | internal gun | not measured |
| 6 | 0 / -8.7 / 0 | `F350.GAS` x1 | 2 in rail | clean: belly 0.17 ft, gear 1.29, ground 3.74 |
| 7 | 6.3 / -8.7 / -1.7 | `MK82` x12 (two triple racks) | 2 in rails, 0.1 ft drop (A1) | airframe entry 0.00 ft (aft inboard bombs 0.049 ft clear), 3.24 to 3.82 ft above ground; rack bombs' fin tips touch (0.005 ft); **the gear leg crosses the aft inboard bombs in transit (28 crossings at gear 0.4)** |
| 8 | 14.3 / -4.7 / -4.7 | `AIM120` x4 (twin rails) | 1 ft wing pylon | clean (0.55 to 0.64 ft from the wing, gear 3.0, ground 7.6) |

Refused on row 7 (A2): GBU-29 and GBU-29P (2.04 ft after the drop), GBU-30
(1.83), AGM-88 and AGM-45 (0.92), AT-2 (0.46), GBU-38 (0.32, just over the line).
Kept: CBU-89 (0.24), Mk 20 and Mk 82 HD (0.13). Refused on row 8 (A3): AA-6; the
GBU-30 on row 8 (0.62 ft) is kept. Code: `rail_drop` `(F15E, 7) => 0.1`,
`F15E_BELLY_REFUSED` in `aircraft_stations.rs`, a normal-loading check in
`loadout.rs` (Cheat loading skips it). Audit pin moved with a dated note:
weapon placements 4,652 to 4,644. Flaps 30 degrees: trailing corner 1.98 units
(0.66 ft) from the source's lowered copy, openings 0.26 ft at the hinge edge and
1.04 ft at the trailing corners. Real stations (cited, plausibility only):
centreline, two wing pylons, tangential CFT stations, LANTIRN stations aft of the
intakes. No pod on a LANTIRN position (none decoded, none invented).

### F15E-7 Sensors, HUD, cockpit, instruments (`e194768e`)

No behaviour changed; bindings were wired in B5-1. Radar `F15R.SEE` and jammer
`F15.ECM` equal the F-15C's; `AV8IR.SEE` is an infrared channel; `fixed_sight`
takes the laser first, so the sight is the Pave Spike (hemisphere, 10 nmi) and the
FLIR stays a scope channel. Laser GBU-12: sight kind Laser, release at 15.2 s,
impact 2.9 ft. Cockpit sheet centre x 531 y 5 w 208 h 51, left and right
x 156 / 1005 y 180 w 109 h 171, identical to the F-15C and the Harrier. Front,
side, up and back views and ten instrument pages draw. Fuel reads `FUEL 13662
LBS (+ EXT 2300 LBS)`, the F-15C record's copy. The eye point is the fleet's one
fixed offset (7 ft up, 10 ft ahead).

### F15E-6 Ordnance, default load, countermeasures (`6415c318`)

No gameplay code changed. Default load: M61 x450, one `F350.GAS` (2,300 lb of
fuel), 12 `MK82`, 4 `AIM120`; stores 9,940 lb, Load Ordnance 52,202 of 68,000 lb.
`--validate-ordnance`: `F15E.PT: 4 source rows, 0 optional weapon rows, 57
compatible weapon placements, 3 tank placements; defaults and restart passed;
catalog 48 normal, 83 Cheat`. Gun 450 rounds (real 512), 320 shots, 52 hits, 1
kill; strafe key 64 rounds in 2 s. Countermeasures 30 chaff and 30 flares from
`F15.ECM`. Live fire: Mk 82 one shot, hit, kill; AIM-120 hit and kill by 1,800
ticks; laser GBU-10 from 15,000 ft impact 3.3 ft. Each store leaves its own
station, left then right; jettison tank then twelve bombs, no explosion. A new
retail-reading test pins the nine rows.

### F15E-8 Flight model and validation (`138976a9`)

Measurement only; no model, record or shared constant changed (decision 1). See
the [flight envelope](#flight-envelope) and the
[variety flight baseline](variety-flight.md#f-15e-validation-2026-11-10). Battery liftoff, liftoff-retail and
approach scenarios added; the flight family passed 14 of 14.

### F15E-9 AI, Quick Mission, multiplayer, replay (`fbe422e1`)

No non-test game code changed. `--ai-roster-probe-ticks 3600`: four `F15E.PT`
lines `models=2 radar=true ir=true stores=3 gun=true dropped=0 PASS`.
`ai-f15e-quick-mission-2v2`, `replay-rec-f15e-2v2` and the menus lobby notes
scenario pass; `net-content-missing` and `net-window-lobby` pass after rebuilding
`tore-server` and `tore-bot` (a stale binary of either refuses the other with a
compatibility-code mismatch). AI finding: `--a2g-run ... --pilot ai --load MK82`
gives `ROLE role=cap` with 12 bombs left after 300 s; with the player's
`engage-my-target` order it releases and hits 123 ft from the aim.

## Flight envelope

Measured on the default hybrid path (clean weight 42,262 lb with full internal
fuel; default load 52,202 lb; afterburner unless stated). Every number equals the
F-15C's on the same load (the F-15E's flight words are the F-15C's). Real figures
are cited public pages and the retail text for comparison only.

| Quantity | Measured (clean / default load) | Retail record | Real (cited) |
| --- | --- | --- | --- |
| Top speed at 36,000 ft | 1,313 kt, Mach 2.29 / 1,243 kt | 1,434 kt | Mach 2.5 (`F15E.INF`) |
| Top speed at 1,000 ft | 750 kt / 710 kt | 819 kt | about Mach 1.2 |
| Level speed to edge ratio | 0.916 at every altitude | | |
| Ceiling | level to about 95,000 ft, zoom 108,921 ft | 100,000 ft | 60,000 to 65,000 ft |
| Best climb | 36,000 ft/min; 36,000 ft in 84 s / 104 s | | more than 50,000 ft/min |
| Peak Ps | 588 to 594 ft/s / 436 to 453 | | |
| Acceleration 363 to 590 kt at 1,000 ft | 19 s / 26 s | | none found |
| G at 461 kt, 1,000 ft, fuel 13,662 / 6,831 / 3,416 / 300 lb | 7.57 / 8.28 / 8.69 / 9.10 clean; 6.73 / 7.29 / 7.60 / 7.91 loaded | +9 / -4 | +9 / -3 |
| Sustained turn 461 kt, 1,000 ft | 5.74 G, 12.8 deg/s / 4.80 G | | none found |
| Fuel | 56.9 min dry, 9.5 min afterburner | flows 4 / 24 lb/s | military about 35 min |
| Liftoff | 127.3 kt, 673 ft clean / 149.1 kt, 1,155 ft | | take-off run 274 m |
| Approach (65 percent fuel) | 140.4 kt / 156.1 kt; rollout 1,808 / 2,247 ft | | 125 kt CAS |

The hybrid default is the validated path; `--legacy-flight` and the restricted
`--native-flight-tables` path stay distinct and unchanged. Every air maneuver
reported `non_finite=0`. The liftoff (130 to 150 kt) and approach (125 to 145 kt)
battery windows are fitted plausibility ranges, with the recorded values 149.1
and 156.1 kt (the loaded approach sits 7.7 percent over, inside the shared 10
percent allowance).

## Open findings for John

Nothing here was changed; each is a decision or a fleet-level model question.

1. **Ceiling is the retail 100,000 ft** (level to about 95,000 ft, zoom to 108,921
   ft) against a real service ceiling of 60,000 to 65,000 ft. It is the F-15C's
   copied value; John kept the record (decision 1). A ceiling fit as for the
   F-5E would be one corrections row.
2. **+9 G is reached only near empty.** Full fuel pulls 7.57 G through the shared
   loaded-elevator rule (the Mirage's M2K-11 treatment would apply the same way).
3. **Climb is about 71 percent of 50,000 ft/min clean and 53 percent loaded**,
   and the sustained turn follows: the shared drag law (drag equals the reference
   thrust at the top speed and grows with speed squared, no ram thrust). A
   fleet-level drag or ram fit is John's call.
4. **The AI flies CAP with bombs.** The strike class is Fighter, so an AI F-15E
   defaults to `role=cap` and bombs only when ordered or given the Strike role.
   Whether bomb-carrying fighters should default to Strike is AI behaviour and
   John's call.
   **Update (B5-AI, 2026-11-11): not a bug; the record's class word.** The role
   rule (`AiWings::apply_strike_roles`) sends a friendly wing 2 or 3 with a
   ground target when its objective is Strike, or by default when the record's
   class word has the bomber bit `0x4000` (retail's own test); `F15E.PT` carries
   `0x8000`, so the default is CAP (printed: `class_word=0x8000`, `ROLE
   role=cap`, no release; the Tornado's `0x4000` gives `role=strike`, three
   impacts). "Strike the ground target" in the creator sends an F-15E. Options
   for John: leave it (retail's rule, fleet-wide); a per-aircraft class-word row
   for the F-15E in the corrections (record data, so his call); or an
   armed-for-ground rule for every fighter-class bomb carrier (fleet-wide
   behaviour change). Recommendation: leave it. Detail in [lane AI,
   B5-AI](../testing/lane-ai.md#b5-ai-the-f-5es-aim-9b-the-f-15es-role-and-the-ai-tornados-racks-2026-11-11).
5. **The gear leg crosses the aft inboard bombs while retracting** (28 crossings
   at gear 0.4). John's round 2 answers did not cover it; the real fix would be a
   station move or a rail change. Also open: the rack fin tips touch by 0.005 ft
   (generic three-a-rack layout), the wing row sits at 14.3 ft, and whether to
   keep `GBU38` (0.32 ft, just over the 0.3 line).
6. **Copied record values** (listed for John, kept retail): fuel 13,662 lb and
   `FUEL 13662` on the panel, gun 450 rounds against the real 512, max takeoff
   68,000 lb against the real 81,000, military fuel flow 39 percent under the
   real F100, 30 chaff and 30 flares with no verified real fit.
7. **No LANTIRN pods.** The Pave Spike row is a fixed placeholder at the origin;
   the real intake stations carry no decoded row. A pod loaded through Load
   Ordnance on rows 6 or 7 follows the standing retail rule.
8. **The cockpit eye point is shared.** `mirrors::pilot_eye` is one fixed offset
   (7 ft up, 10 ft ahead) for every aircraft; the F-15E's canopy top is 5.3 ft.
   A seated eye per aircraft would be a fleet change.
9. **The RWR threat frame is not made.** No scripted path plots an emitter or
   missile on the RWR; the code is aircraft-agnostic and unit-tested.
10. **Rig gaps.** The nose gear folds forward (one cited source) while the
    F-15C's was fixed to fold forward in B5-3; the flap `-1` branch is pinned but
    never set (the neutral wedge stays drawn); the aileron root pair opens 0.0998
    ft at full travel; the destroyed whole-body damage variant is not registered.
11. **Gun cadence.** The shared gun fields fire 32 rounds a second; not F-15E
    specific.
12. **An F-15E in live multiplayer flight was not played.** The net scenarios are
    aircraft-agnostic and exercise the identity list and content code only.
13. **Frames index.** The frame pack is indexed in the local `INDEX.md`
    (appended at X-10).

## Definition of done

Checked item by item against the per-aircraft definition of done (the MiG-31
list in `PLAN-MIG31.md`, plus the Mirage extras on elevons, applied here to
the stabilators).

| Item | Verdict | Reason |
| --- | --- | --- |
| Identity | Met | Exact PT, `AircraftId` appended to the enum, `ALL`, `SELECTABLE` and checkpoint code 41; every named resource present (`F15E.PTS` an explicit exception); record audit done (F-15C template listed); no alias of the F-15C; import report clean; not a parked aircraft |
| Look | Met | Own shape at 1:1 (decision 6), damage looks A to D, `render-f15e-skin` passes, no show-through or clipped marking in the frames. The destroyed whole-body variant is unavailable (no damage variant registered, as for the F-15C); both wing decal slots stay omitted |
| Whole-surface gate | Met | Every stabilator, aileron, flap, brake, gear and plume piece is a whole panel; the rule 11 fold tables above show 0 fixed vertices off any axis; `skin_gap_ft` 0 and rigidity about 1e-6 ft in every pose. Seam openings (aileron 0.0998 ft, flap, stabilator root corner) are inherent to rigid panels |
| Animation | Met | Signed pitch and roll (no rudder, decision 4); flaps at 30 degrees; rigid gear with near-stow; brake and burner; neutral equals source; probe `reviewed-controls-and-attachments` 0 failures; the 25-pose grid passes; `flight-animation-f15e` passes |
| Stations | Not met (one open finding) | Decoded rows measured under the standard rules, John's decisions A1 to A3 applied, preview audit clean apart from the rack fin touch (0.005 ft, generic). The gear leg still crosses the aft inboard bombs in transit (open finding 5); John has not decided it |
| Ordnance | Met | Every default store permitted live and released from its own station; compatibility, even pairs, tanks, jettison and the AI default checked; 30 / 30 countermeasures from `F15.ECM`; `--validate-ordnance` passes for all 45 aircraft |
| Cockpit | Met | `AV8.HUD` and the Harrier-art cockpit by name (decision 2); aperture, overlays, mirrors and ten instrument pages checked; reviewed sensor presets only; no pod body on the Pave Spike row (decision 3). The RWR with a live threat was not demonstrated (finding 9) |
| Flight | Met | Own configuration on the hybrid default; envelope and top speed reviewed; stall, spin, takeoff, landing and touchdown probes and the flight battery family pass. `--native-flight-tables` was not run (tables not present locally); the ceiling, climb and full-fuel G findings are open for John |
| AI and modes | Met | Strike class row, Quick Mission, Load Ordnance, AI roster probe, wire, checkpoint and replay round trips, lobby. An F-15E in live multiplayer flight was not played; the AI defaults to CAP with bombs (finding 4) |
| Docs | Met | Source review, specs with provenance, this record, inventory, features and import guide updated; full check list below; the frame pack is indexed in the local `INDEX.md` |
| Extra: stabilators | Met | Pitch, roll and the 25-pose grid pass with `skin_gap_ft=0` and rigid panels; every moving surface has a rule 11 face table above |

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `F15E-2` to `F15E-9`
directories (skin and damage views, stabilator, aileron and flap poses, gear,
brake and burner stages, loaded stores and clipping close-ups, ordnance and
countermeasure frames, cockpit, HUD and instrument pages, and the AI, Quick
Mission and replay frames). Observed points: no underside show-through or
clipped marking; no half-moved panel; the gear tucks as rigid plates; the one
dorsal brake plate and two afterburner plumes draw; the Mk 82 racks sit tight
under the intake flanks after the rail drop; the centreline tank and the wing
AIM-120 pairs hang clean; the cockpit and Quick Mission pages name the exact
F-15E identity. The frames are local evidence and are not committed.

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
| `tore-app --validate-ordnance --no-audio` (`.local/m3-aircraft/profile`) | Passing: 45 aircraft, 18 recovered weapon stations, 4644 weapon placements, 233 tank placements, 82 weapon and 9 tank records; passed; 80 pod and 220 Cheat-only placements; `F15E.PT: 4 source rows, 0 optional weapon rows, 57 compatible weapon placements, 3 tank placements` |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,256 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (377 tracked files; the new baseline carries the header and is checked when staged) |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |
| Battery, F-15E: `--scenario `*-f15e*`` (render, animation, flight, a2g, ai, replay and menus lanes) | 17 of 17 passed: render-f15e-skin; flight-animation-f15e, flight-variety-f15e, flight-liftoff-f15e, flight-liftoff-retail-f15e, flight-approach-f15e, live fire slots 1 to 3, jettison slots 2 and 3; a2g-release-origin-f15e, a2g-jettison-f15e, a2g-laser-f15e; ai-f15e-quick-mission-2v2; replay-rec-f15e-2v2; menus-snap-quick-lobby-ordnance-laser-notes-f15e |
| Battery, shared pins: `menus-ordnance-availability`, `a2g-catalog`, `a2g-release-origin` | 3 of 3 passed |
| Battery, net (after rebuilding `tore-server` and `tore-bot`): `net-content-missing`, `net-window-lobby` | each passed, run one at a time |


## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight, F-15E in live multiplayer flight or performance
measurement was run. `--native-flight-tables` needs the extracted `sine-q15.bin`
and `atan-pa.bin`, which are not present locally, so it stays a separate
restricted path and was not run. A windowed frame of the gear leg crossing the
bombs in transit was inconclusive (the figures are numeric). The flight and
rendered evidence above comes from the Linux development host and the wrapped
captures.
