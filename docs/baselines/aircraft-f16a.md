# F-16A acceptance record (M3, 2026-11-08)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The F-16A Falcon (`F16.PT`) is a selectable retail identity in `AircraftId::ALL`
and `AircraftId::SELECTABLE`, appended at checkpoint code 38. It is not an alias
for `F16C.PT` or a later variant: it shares `F16.SH`, the `F16C.HUD`, the `F16`
cockpit stem and the F-18 radar and jammer by naming the same resources, and it
flies its own record. No retail comparison exists, so this record claims
source-reviewed, extraction-supported, headless-flight-supported and
rendered-flight-supported status, not retail parity.

## Source identity and dependency closure

The source review is
[F-16A source review](../formats/aircraft-variety.md#f-16a-source-review-m3-2026-11-08).
Retail media stays local; no extracted bytes are committed. The user supplied
the same build pair as the 2026-10-05 variety and MiG-25 reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `F16.PT` | `46018763b5e823df1d29c37c78d1cb9fdb7e7075b5235e23001bcc001b55b010` |

The selected closure (the `FA_2.LIB` copies; the cockpit art is in `FA_1.LIB`):

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `F16.SH` | Shape, exponent 8 | `7d570f1f705cfaa6b1288653e47e4c502d7606109044f306972bd6c63161f315` |
| `F16_S.SH` | Shadow shape | `ad87c11f0897eb9bc1e451f539795843d08e501fc69c3fd8c0c195aa3f050e17` |
| `F16_A.SH` | Damage body A | `0bf3a39736fe6a89dd53170a0fcc8b30298a812f534b72befb32ddf1779c9108` |
| `F16_B.SH` | Damage piece B | `8b03394f3bb7408c34eb0354a50453c9ee1af2ee95df0a8845af48f002688a1c` |
| `F16_C.SH` | Damage body C | `237a61d3d8417dc1bdccb270aed6614970d4b70e73783ebfeea3a88b990f89c2` |
| `F16_D.SH` | Damage piece D | `2b0be00c5f6c76e87f76867f0152fa67d4b40c7933bb9947ea3480856c3363f2` |
| `F16C.HUD` | HUD and cockpit pointers | `f18d6b5cb8c16116cc012429fb2efb074a1bb1c25b07b1718238b00379564cd5` |
| `F18R.SEE` | Radar, ADVANCED | `b00861010ec23c39a082cd2b313a4a5223daf8ff6e7769f95353f77bbae01682` |
| `F18.ECM` | Jammer, LATE, chaff 30 and flare 30 | `0fb851d57d7f88f51a017d9663101b52c681c6a6d5ac55823588f25a942d31a9` |
| `VIS340.SEE` | Visual sensor, signature 0 | `445c35d1cc778513b0d38b73d36e10b47b7a4f210ffef87f54a53b17e6cd9efe` |
| `M61.JT` | Built-in gun | `0b79689a728911d1097569f6e6612050e414ba9703c9f131a51fe1ce9a7eb33d` |
| `F150.GAS` | Centreline tank | `ca9054921aa93eb890801a5a1f64deb55c8bcc6d03854ff03f1b42833a9f4c95` |
| `AGM65G.JT` | Default store | `b94ee0dae96dd2b7fc70a21e430444a1d72b565eec9611ecb749965ece2e5cca` |
| `AIM120.JT` | Default store | `702d559376e544b49d2d118e1a8911227b0abe6560ff84340a7fd592c709d7a8` |
| `AIM9M.JT` | Default store | `fdac769d40e374ed691a2758c80f4deddf44ecb136d8973374ea053ca03723cf` |
| `~F16H.PIC` | Forward cockpit (`FA_1.LIB`) | `5162b8a467e54d546f3a066f34befecdaabd82a154d3482e88521a055386d851` |
| `~F16_P.PIC` | Instrument panel (`FA_1.LIB`) | `223cf78dfab87fb5f9d1eaf3e4ba055525d7b8f546663b2cc146e07fa358b841` |

The shape's `_F16.PIC` texture is in `FA_2.LIB`
(`d7822eded62964011b552a80aca1c8fbfe1faf34a0323c865fc572b4248d031b`) and the four
damage looks name `_F16_A.PIC` to `_F16_D.PIC`, all present. The PT names no
`F16.PTS` companion, unlike `F16C.PTS`; the absence is an explicit exception and
`has_pts()` returns false. The cockpit stem is `F16`: forward `~F16H.PIC`,
panel `~F16_P.PIC`, base `~F16.PIC` and surround `~F16S.PIC`, with no left,
centre or right overlay. The dependency report and extracted copies live only
under `.local/m3-imports/scratch/f16a/`. The app import command used for the
profile was:

```sh
TORE_DATA_DIR=.local/m3-aircraft/profile cargo run --locked -p tore-app -- \
  --import gameassets/fighters-anthology --import-only --no-audio
```

The selected CLI extraction was:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/f16a --include 'F16.PT' --include 'F16*' \
  --include 'F16C.HUD' --include 'F18R.SEE' --include 'F18.ECM' \
  --include 'VIS340.SEE' --include 'M61.JT' --include 'F150.GAS' \
  --include 'AGM65G.JT' --include 'AIM120.JT' --include 'AIM9M.JT' \
  --include '~F16*' --include '_F16*' --include 'F16_*' --include 'F16*.11K' \
  --include 'F16*.CB8'
```

The import report listed every required resource and no missing required
reference. Extraction success alone does not enable flight; the reviewed
identity does.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#f-16a-source-review-m3-2026-11-08) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-f16a-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, HUD, cockpit and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Implemented, fitted and unavailable subsystems

| Component | Status | Provenance |
| --- | --- | --- |
| Identity, `F16.PT`, size 660, checkpoint 38, `SELECTABLE` index 38 | Implemented | Spec-derived |
| `F16.SH`, exponent 8, 356 runtime neutral faces | Implemented | Spec-derived |
| Damage looks A to D and the shadow | Implemented, drawn through the shared damage path | Spec-derived |
| Flaps, ailerons, tail, rudder, gear, brake and burner | Implemented as whole source panels, the reviewed F-16C rig | Fitted hinges and roles over source geometry; angles fitted |
| Decoded stations, pylons and stores | Implemented | Spec-derived positions and masks; shared pylon rules opinionated (John, 2026-10-13) |
| `F150.GAS`, `AGM65G.JT`, `AIM120.JT` and `AIM9M.JT` default load | Implemented | Spec-derived |
| Chaff 30 and flares 30 | Implemented | Spec-derived from `F18.ECM` |
| `F16C.HUD`, `~F16_P.PIC`, the F-16 aperture and no mirrors | Implemented | Spec-derived shared cockpit |
| Radar `F18R.SEE` ADVANCED, ECM `F18.ECM` LATE, visual `VIS340.SEE` | Implemented and reviewed | Spec-derived |
| `M61.JT` gun | Implemented | Spec-derived |
| Hybrid researched flight on the aircraft's own `F16.PT` configuration | Implemented | Spec-derived record, fitted host response |
| AI and Quick Mission | Implemented as Fighter | Spec-derived family binding |
| Wire, checkpoint and replay | Implemented append-only | Spec-derived 6-bit selectable index |
| Destroyed whole-body variant | Unavailable | Shared with the MiG-25 and F-104; no damage variant registered for the identity |
| Pave Spike designator row | Absent by record | The `F16.PT` record has no designator row; the two wing rows admit a pod through the decoded mask and the Load Ordnance page warns |
| Retail comparison | Unavailable | No running retail copy |

The listed retail values are in the
[source review](../formats/aircraft-variety.md#f-16a-source-review-m3-2026-11-08):
the empty weight 14,567 lb and internal fuel 6,972 lb match published F-16A
figures, the 33,000 lb maximum is a published loaded weight, and the thrust pair
(17,687 / 32,000 lbf) runs about 19 and 34 percent above the F100-PW-200 because
it is the fleet template shared word for word with `F16C.PT`. Under the record
rule nothing is corrected and the thrust stays retail and listed for John.

## Flight suite

Measured on the default hybrid `--researched-flight` path with the aircraft's own
`F16.PT` configuration. The record equals `F16C.PT` in every mass, thrust, fuel,
engine and envelope field, so the numbers equal the F-16C's. Details are in
[F-16A validation](variety-flight.md#f-16a-validation-2026-11-08) and the
[source and flight review](../formats/aircraft-variety.md#f16a-6-flight-review-2026-11-08).

| Case | Result |
| --- | --- |
| `variety_flight` | `aircraft=F-16 conventional=pass deterministic=pass landing=pass ticks=1200` |
| 1 G top speed | 1,145 kt at 36,000 ft decoded, 2 percent over the published 1,120 kt |
| Envelope level speeds | 749 / 809 / 885 / 1,001 / 1,040 / 716 kt at 1,000 to 60,000 ft, ratio 0.935 to 0.921 of the edge |
| Full-stick pull | 7.66 / 6.80 / 5.09 G at 0.50 of the edge, limits 7.55 / 6.71 / 5.04 |
| Level, 7,200 ticks | 539.1 kt, no crash |
| Pull, 7,200 ticks | no crash, identical to the F-16C apart from the name |
| Loop, 7,200 ticks | 560.1 kt at the top, no crash |
| Sprint, 36,000 ticks | 810.1 kt, no crash |
| Climb, 216,000 ticks | 69,706 ft peak (1.162 of its ceiling), no crash |
| Dive, 9,000 ticks | overspeed loss at 1.12 of the edge |
| Overspeed, 1,200 ticks | 1,041.3 kt, loss at 1.10 |
| Rudder, 360 ticks | no crash |
| Devices, 3,600 ticks | no crash |
| Stall recovery | alert at 1,465, recovered at 2,187, 0 ft lost, no crash |
| Spin recovery | entered at 3, recovered at 718, 0.39 revolutions, 902 ft lost, no crash |
| Takeoff | liftoff tick 500 at 96.2 kt over 343 ft |
| Landing | touchdown 97.7 kt at 3.1 ft/s sink, stopped tick 17,909 with 2,832 ft left, score 100 |
| Hard landing | touchdown 109.7 kt at 33.3 ft/s sink |
| Landing gear up | crashed for the gear |
| Landing off runway | crashed for the not-landable ground |
| Nosewheel | no crash |

The eleven air maneuvers are byte-identical to the F-16C's apart from the
`Aircraft:` name line. The seven ground probes differ only in the default load:
the F-16A carries 3,894 lb (25,433 lb gross) against the F-16C's 4,854 lb
(26,393 lb gross); re-run with `--loadout none` all seven are byte-identical
apart from the name line, so the difference is stores, not a flight field. The
full flight battery family passed 15 of 15:
`flight-animation-f16a`, `flight-liftoff-f16a`, `flight-liftoff-retail-f16a`,
`flight-approach-f16a`, `flight-variety-f16a`, the five live-fire slots, the four
jettison slots and `render-f16a-skin`. The hybrid researched default is the
validated path; the legacy `--legacy-flight` adapter and the restricted
`--native-flight-tables` path stay distinct and unchanged.

## Controls, devices, equipment and ordnance

- The animation probe reports 356 neutral faces and 0 required or check failures
  in the `reviewed-controls-and-attachments` scope. Every panel moves whole:
  anchor and skin gaps are 0 in all 25 flap and roll combinations, and all four
  faces of each wheel sheet and each brake panel sit in one witness group, so no
  triangle is left behind a hinge.
- Probe coverage reads `gear required moved true 22` and `brake`/`exhaust`
  required and moved, matching the F-16C's byte-identical geometry.
- The decoded station block has nine rows: the fixed visual sensor, radar and
  jammer in rows 0 to 2, the built-in `M61.JT` gun in row 3, and the five
  editable carriers in rows 4 to 8.

| Row | Location | Mask | Accepts | Default |
| ---: | --- | --- | --- | --- |
| 4 | Centerline | `0x0605` | `F150.GAS` and `F250.GAS`, 1 a row | `F150.GAS` x1 |
| 5 | Wing (paired) | `0x07e5` | 56 supported weapon records, 2 a row | `AGM65G.JT` x2 |
| 6 | Wing (paired) | `0x07e5` | the same 56, 2 a row | `AIM120.JT` x2 |
| 7 | Wing (paired) | `0x0085` | 10 infrared records, 2 a row | `AIM9M.JT` x2 |
| 8 | Wingtip (paired) | `0x0085` | the same 10, 2 a row | `AIM9M.JT` x2 |

- `--validate-ordnance` passes 38 aircraft with 3,817 weapon placements, 188 tank
  placements, 67 pod placements and 182 Cheat-only novelty placements. The F-16A
  line is `F16.PT: 6 source rows, 0 optional weapon rows, 133 compatible weapon
  placements, 6 tank placements; defaults and restart passed; catalog 57 normal,
  83 Cheat`.
- Live fire releases the gun from slot 1, `AIM120.JT` from slot 3 and `AIM9M.JT`
  from slots 4 and 5; the ammo vector is `[500, 2, 2, 2, 2]`, matching the source
  rows. Slot 2 is the `AGM65G.JT` air-to-ground row, which refuses the air
  practice fixture (the same `LAUNCHER LOST` result the Su-25's AS-7 shows), so
  it is listed in `REFUSED_SLOTS` and covered by `--validate-ordnance` and the
  jettison scenario.
- A flare release carries 29 flares with capacity 30; a chaff release carries 29
  chaff with capacity 30. Both come from `F18.ECM`.
- The stores preview reads 0.00 ft of clip on rows 5, 6 and 7 and 0.42 ft at the
  wingtip row 8, exactly the F-16C's tip result at the same place, so there is no
  clipping beyond the F-16C. The decoded positions ship with no `station_move`
  and no real-station table.
- The default load is the source default for the human and the AI alike, so no
  separate AI default and no retained-selection seed.

## Cockpit, sensors and instruments

The `F16` cockpit loads with `F16C.HUD` naming `~f16_p`, whose `panel_resource()`
is `~F16_P.PIC`, the identity's instrument panel. `--cockpit-sheet f16a` reported
`stem F16 cockpit ~F16H.PIC art 1280x490 NO MIRRORS FITTED`, and the F-16A render
and art-mask tile are pixel-identical to the F-16C's. The forward HUD has no gun
reticle beyond the M61 gun row. All ten instrument pages draw on the shared
bezel: RCS, Envelope (5000 FT, 539 KTS), Front View and Other View (CAMERA
LOADING), Target Cam (NO TARGET), RWR (scale 10), Nav Info (NO WAYPOINTS, 1
MISSION), Systems (THR 70%, TEMP 0%, OIL 100%, HYD 100%, FUEL 6972 LBS, + EXT 990
LBS), Weapons (the live capture lists the default load, `30 CHAFF`, `30 FLARE`)
and Radar (the live capture shows TWS at 10 miles, the cold start RADAR OFF).
Absent systems stay absent: no separate targeting sight and no Pave Spike
designator row. The sensors are the reviewed F-16C presets reached by the same
record names: radar `F18R.SEE` ADVANCED, jammer `F18.ECM` LATE, visual
`VIS340.SEE`. `--sensor-summary --aircraft f16a` prints an F16A line
byte-for-byte equal to the F-16C's bar the identity. See
[cockpit confirmation](../formats/aircraft-variety.md#f16a-5-cockpit-and-hud-confirmation)
and the [mirror baseline](mirrors.md).

## AI, Quick Mission, wire and replay

The F-16A is offered as a Quick Mission player and wingman and flies as a Fighter
in `strike_class.rs`. `--ai-roster-probe-ticks 3600` prints four `F16.PT` lines,
Novice, Average, Experienced and Ace, each `ticks=3600 models=2 radar=true
ir=false stores=5 gun=true projectiles=0 dropped=0 ... PASS`, so the AI input is
finite and replay-exact. The 6-bit wire codes the identity by its `SELECTABLE`
index 38; `checkpoint_shared.rs` holds `F16A = 38`. Session, checkpoint and
replay round trips cover it generically through `SELECTABLE`, and
`wire-golden.txt` did not change. Quick Mission restart passes and the creator
carries the exact identity in every wing field. The lobby's Load Ordnance page
shows the decoded default load and warns when a laser store has no designator,
showing the Pave Spike pod helper because the F-16A's decoded mask admits a pod.

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `F16A-2` to `F16A-7`
directories (skin and damage views, control and device poses, store views,
ordnance and countermeasure frames, cockpit, HUD and instrument pages, and the
Quick Mission and lobby frames). Observed points: no underside show-through or
clipped marking in smooth and stepped modes; no half-moved panel; the gear tucks
rigidly; the brake and burner draw; the stores hang on their pylons with no body
inside the wing and nothing in space; the F-16A and F-16C under views match bar
the load; and the cockpit and Quick Mission pages name the exact F-16A identity.
The frames are local evidence and are not committed.

## Full checks, 2026-11-08

Every command ran through `.local/m3-imports/light.sh` from the repository root.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passing |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passing |
| `cargo test --workspace --locked` | 7,469 passed, 0 failed, 108 ignored across 64 test binaries |
| `cargo build --workspace --locked` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,230 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (374 tracked files; the new baseline carries the header and is checked when staged) |

This slice is documentation only, so no GPU `--smoke-test` was run. The rendered
frames above were captured in the earlier slices that changed presentation.

## Roster capacity finding

The wire codes an aircraft identity as its `AircraftId::SELECTABLE` index in 6
bits ([the wire format](../formats/net-protocol.md)), so the protocol can carry
64 identities at most. With the F-16A the list holds 39: 38 retail aircraft and
the F/A-XX concept. Only 25 more aircraft fit before a protocol change. Porting
all 80 remaining decoded PT records needs a wider identity field first; that is a
protocol decision for John, not an aircraft-import decision.

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight or performance measurement was run.
`--native-flight-tables` needs the extracted `sine-q15.bin` and `atan-pa.bin`,
which are not present locally, so it stays a separate restricted path and was not
run. The flight and rendered evidence above comes from the Linux development host
and the wrapped captures.
