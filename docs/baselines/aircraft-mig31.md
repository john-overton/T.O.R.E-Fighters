# MiG-31 acceptance record (M3, 2026-11-08)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The MiG-31 Foxhound (`MIG31.PT`) is a selectable retail identity in
`AircraftId::ALL` and `AircraftId::SELECTABLE`, appended at checkpoint code 39.
It is not an alias for the MiG-25 or any other record: it has its own
`MIG31.PT`, `MIG31.SH` and rig, and it names shared resources only where the
record itself names them (`VIS240.SEE`, `GSH6_30.JT`, `AA9.JT` and `AA8.JT`).
John decided on 2026-11-08 that it flies the Yak-141 HUD and cockpit, because
the record's own `y141.HUD` is absent from every archive, and that its radar and
jammer are reviewed as `Preset::Advanced` and `Generation::LateColdWar`. No
retail comparison exists, so this record claims source-reviewed,
extraction-supported, headless-flight-supported and rendered-flight-supported
status, not retail parity.

## Source identity and dependency closure

The source review is
[MiG-31 source review](../formats/aircraft-variety.md#mig-31-source-review-m3-2026-11-08).
Retail media stays local; no extracted bytes are committed. The user supplied
the same build pair as the MiG-25 and F-16A reviews:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `FA.EXE` | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| `MIG31.PT` | `fea8d9cf5c645ee9bf800d24cf10f3bf143c171369c7b4c110a622e1b2325320` |

The selected closure (the `FA_2.LIB` copies; the cockpit art is in `FA_1.LIB`):

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `MIG31.SH` | Shape, exponent 8 | `6dc4ed9cb6c99675cf7076dd38c90146e2e6300acd3c11bc565a5269f8a46a89` |
| `MIG31_S.SH` | Shadow shape | `063b9f28070af4acd9953c716c93264ad0c9adc843a678e64ef2a60befb289d2` |
| `MIG31_A.SH` | Damage body A | `374e24798d954b0cda67a54269b68495dc480d4f4150c9fadc3815ae126631ce` |
| `MIG31_B.SH` | Damage piece B | `6aff8fdab918c6fbfffb53e203c1f94423787fc53352efe2b0cad75057a384f0` |
| `MIG31_C.SH` | Damage body C | `4199aa5ed4ab3fc02de433937bab64790cdd69212579abe71a32c1e41ef4f710` |
| `MIG31_D.SH` | Damage piece D | `9ab111e2920423ea579c2cf82adc5a5ffca6afa74e2907322ac04a6913116c53` |
| `MIG31R.SEE` | Radar, ADVANCED | `e7aa12bd307d1b24b0b8fd9594cdadece542e27b6343656ae0eff5cb3a0ca669` |
| `MIG31.ECM` | Jammer, LATE, chaff 60 and flare 60 | `874f7b364ce6068341d6a17e10c4b73b37efcf86ec0d528a62f93e9c0296e56e` |
| `VIS240.SEE` | Visual sensor, signature 0 | `a3224581abef75e0fe4e6dbb37d2d47d5ab4704bb1043e09dd6aa2e5a4510ee1` |
| `GSH6_30.JT` | Internal gun | `4fc94198cbd0e40de6fc314dc73502dc662d106ffa9d14291939f4f779120cf4` |
| `AA9.JT` | Default A2A store | `38adf9f9cfc00aee15ed729553c98597c7a92ccfb6323b12a6c26a8016192f4c` |
| `AA8.JT` | Default A2A store | `c656407564e307f9eba70b853ef7cf226ea4a91a4d3cabae88062e6128cf7008` |
| `YAK141.HUD` | HUD and cockpit pointers | `8da00e871d1cd20aa49fd1d18f11c4673533a3fd64b669b97f9ae245243f9997` |
| `~Y141H.PIC` | Forward cockpit (`FA_1.LIB`) | `48af8b8365bdbdaae338068fc0c6867925a69bb90d39d2fc831d8634b109d7d4` |
| `~Y141_P.PIC` | Instrument panel (`FA_1.LIB`) | `c4700dc26dd0caf6dc39f6ed15c7def8a1961ed40922827aa4c550eeb9341cdd` |

The shape's `_MIG31.PIC` texture is in `FA_2.LIB`
(`7bf593b047b6cce5b8b154f375843cb2075efd3b068116c916849631b305ce1c`) and the
four damage looks name `_MIG31_A.PIC` to `_MIG31_D.PIC`
(`f4ad06bab197c7f68b9afb3b76df3521f56ef5412ad664558d5a25a426e269b8`,
`7c18a51735d4853140ce30e321fa492c91d1b6987c97d9c29d360766ab659e4c`,
`3ecf5a62cc23eea831feb1d4b60bbd9d5cfdcb8d406dcf7ef13d4bd215429279` and
`a2a1d13913d99a582da8f1686fc56e91f476e1fea2ac60f3e73bb0fc166a67db`), all
present. The PT names no `MIG31.PTS` companion, so the absence is an explicit
exception and `has_pts()` returns false. The record's own `y141.HUD` is absent
from every archive; John's decision uses `YAK141.HUD` with the `Y141` cockpit
stem (forward `~Y141H.PIC`, panel `~Y141_P.PIC`, base `~Y141.PIC` and surround
`~Y141S.PIC`, with left and right overlays), which is opinionated and recorded
in the [source review](../formats/aircraft-variety.md#sensors-and-stores).
The dependency report and extracted copies live only under
`.local/m3-imports/scratch/mig31/`. The app import command used for the profile
was:

```sh
TORE_DATA_DIR=.local/m3-aircraft/profile cargo run --locked -p tore-app -- \
  --import gameassets/fighters-anthology --import-only --no-audio
```

The selected CLI extraction was:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/mig31 --include 'MIG31*' --include 'Y141*' \
  --include 'VIS240.SEE' --include 'GSH6_30.JT' --include 'AA9.JT' \
  --include 'AA8.JT' --include 'F250.GAS' --include '~Y141*' --include '*.HUD'
```

The import report listed every required resource and no missing required
reference. Extraction success alone does not enable flight; the reviewed
identity does.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#mig-31-source-review-m3-2026-11-08) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-mig31-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, HUD, cockpit and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Implemented, fitted and unavailable subsystems

| Component | Status | Provenance |
| --- | --- | --- |
| Identity, `MIG31.PT`, size 588, checkpoint 39, `SELECTABLE` index 39 | Implemented | Spec-derived |
| `MIG31.SH`, exponent 8, 214 runtime neutral faces | Implemented | Spec-derived |
| Damage looks A to D and the shadow | Implemented, drawn through the shared damage path | Spec-derived |
| Stabilators, outboard ailerons, inboard flaps and twin rudders | Implemented as whole source panels | Fitted roles and hinges over source geometry; angles fitted |
| Three rigid gear assemblies, dorsal airbrake and afterburner plume | Implemented | Source words decoded; retraction, brake angle and plume scaling fitted |
| Opinionated MiG-31 station table (AA-9 tunnel pairs, AA-8 wing pylons) | Implemented | John, 2026-11-08; coordinates fitted from the model |
| Row 4 `AA9.JT` x4 and row 5 `AA8.JT` x2 default load | Implemented | Spec-derived masks, capacities and defaults |
| `GSH6_30.JT` internal gun, 260 rounds | Implemented | Spec-derived |
| Chaff 60 and flares 60 | Implemented | Spec-derived from `MIG31.ECM` |
| `YAK141.HUD`, `~Y141_P.PIC`, the Yak-141 aperture and two mirrors | Implemented | Opinionated, John, 2026-11-08 |
| Radar `MIG31R.SEE` ADVANCED with its own 124 nmi search, ECM `MIG31.ECM` LATE, visual `VIS240.SEE` | Implemented and reviewed | John, 2026-11-08; record values spec-derived |
| Hybrid researched flight on the aircraft's own `MIG31.PT` configuration | Implemented | Spec-derived record, fitted host response |
| AI and Quick Mission | Implemented as Fighter | Spec-derived family binding |
| Wire, checkpoint and replay | Implemented append-only | Spec-derived 6-bit selectable index |
| Destroyed whole-body variant | Unavailable | Shared with the MiG-25 and F-16A; no damage variant registered for the identity |
| Gear doors | Unavailable | The `0x612c` branch adds twelve wheel cards and removes nothing, so the source names no separate door faces |
| Pave Spike designator row | Absent by record | The `MIG31.PT` record has no designator row; row 4's decoded pod bit admits a pod and the Load Ordnance page warns |
| Retail comparison | Unavailable | No running retail copy |

The listed retail values are in the
[source review](../formats/aircraft-variety.md#mig-31-source-review-m3-2026-11-08):
the empty weight 48,115 lb equals the in-game encyclopedia, the internal fuel
30,865 lb runs 14.4 percent under the published figure, the dry thrust 33,510
lbf 19.8 percent under and the afterburner 48,500 lbf 29.0 percent under the
D-30F6. The fuel flows (6 and 36 lb/s) run far above the real engine's specific
fuel consumption. Under the record rule nothing is corrected and these stay
retail and listed for John.

## Flight suite

Measured on the default hybrid `--researched-flight` path with the aircraft's own
`MIG31.PT` configuration. Details are in
[MiG-31 validation](variety-flight.md#mig-31-validation-2026-11-08) and the
[flight review](../formats/aircraft-variety.md#m31-8-flight-review-2026-11-08).

| Case | Result |
| --- | --- |
| `variety_flight` | `aircraft=MiG-31 conventional=pass deterministic=pass landing=pass ticks=1200` |
| 1 G top speed | 1,625 kt at 36,000 ft decoded, 0.3 percent over the published 1,620 kt; 812 kt at sea level |
| Envelope level ratios | 0.915 to 0.916 of the edge |
| Full-stick pull | 3.30 / 3.29 / 3.28 G at 0.50 of the edge, limits 3.25 |
| Level, 7,200 ticks | 571.3 kt, no crash |
| Sprint, 36,000 ticks | 842.3 kt, 0.940 of the 5,000 ft edge, no crash |
| Climb, 216,000 ticks | 77,009 ft peak (1.132 of its 68,000 ft ceiling) on a dry tank, no crash |
| Dive, 9,000 ticks | overspeed loss at 1.20 of the edge |
| Overspeed, 1,200 ticks | overspeed loss at 1.10 of the edge |
| Rudder, 360 ticks | no crash |
| Devices, 3,600 ticks | no crash |
| Stall recovery | alert at 2,765, recovered at 3,245 with 0 ft lost, no crash |
| Spin recovery | entered at 3, recovered at 1,423 after 0.73 revolutions, 3,146 ft lost, no crash |
| Takeoff | liftoff tick 1,949 at 165.8 kt over 2,309 ft (rotation 146.3 kt) |
| Landing | touchdown 167.9 kt at 12.0 ft/s sink, stopped with 1,709 ft left, score 100 |
| Hard landing | touchdown 174.6 kt at 34.1 ft/s sink |
| Landing gear up | crashed for the gear |
| Landing off runway | crashed for the not-landable ground |
| Nosewheel | no crash |

Every air maneuver reported `non_finite=0`. The flight battery family passed 11
of 11: `flight-animation-mig31`, the three live-fire slots, the two jettison
slots, `flight-liftoff-mig31`, `flight-liftoff-retail-mig31`,
`flight-approach-mig31`, `flight-variety-mig31` and `render-mig31-skin`. The
hybrid researched default is the validated path; the legacy `--legacy-flight`
adapter (140.6 kt against 246.6 kt at 7,200 ticks on the pull probe) and the
restricted `--native-flight-tables` path stay distinct and unchanged. The
liftoff and approach ranges are fitted: the approach range comes from the
published 280 km/h landing speed (airbase.ru, read 2026-11-08), and no separate
MiG-31 takeoff figure was found, so the liftoff range is the shared
heavy-interceptor plausibility band and the measured values are regression
windows.

## Controls, devices, equipment and ordnance

- The animation probe reports 214 neutral faces and 0 required or check failures
  in the `reviewed-controls-and-attachments` scope. Every moving panel is one
  whole source panel: a flat double-sided quad for each control surface, one
  dorsal brake quad, crossed plume quads and rigid gear assemblies. Anchor and
  skin gaps are 0 in every pose, and all 25 flap and roll combinations report
  `finite=true`, `skin_gap_ft=0`, `crossings=0` and `direction_failures=0`.
- Probe coverage reads `gear required moved true 22`, `brake required moved true
  5` and `exhaust required moved true 5`. The 203-line dense gear sweep passes
  with a maximum assembly rigidity error of 0.0000032 ft.
- The decoded station block has six rows: the fixed visual sensor, radar and
  jammer in rows 0 to 2, the built-in `GSH6_30.JT` gun in row 3, and the two
  editable carriers in rows 4 and 5.

| Row | Location | Mask | Accepts | Default |
| ---: | --- | --- | --- | --- |
| 3 | Internal Gun | `0x0008` | its own `GSH6_30.JT`, 260 rounds | `GSH6_30.JT` x260 |
| 4 | Wing (paired), tunnel pairs | `0x07e5` | 61 supported records, four tanks and a `PAVESPK.SEE` pod, 4 a row | `AA9.JT` x4 |
| 5 | Wingtip (paired), wing pylons | `0x0085` | 13 infrared records, 2 a row | `AA8.JT` x2 |

- Row 4 draws four AA-9s as two tandem pairs under the fuselage at 2.0 ft either
  side of the centreline, the front pair at 8.43 ft and the rear at -5.42 ft,
  with a 0.27 ft rail. Row 5 draws the AA-8s on 1 ft wing pylons at the record's
  16.67 ft out. The measured clip is 0.48 ft on the rear pair's outer upper fin
  into the belly skin, under the 0.5 ft target and below the Su-27's 0.72 ft;
  row 5 measures 0.00 ft. No store touches the gear, the outermost fin being 2.3
  ft inboard of the main gear and the front store's nose 9.2 ft behind the nose
  gear. The positions are John's opinionated 2026-11-08 table; the decoded rows,
  masks, capacities and defaults are unchanged
  ([stations](../spec/stores-on-pylons.md#the-mig-31)).
- `--validate-ordnance` passes 39 aircraft with 3,892 weapon placements, 192 tank
  placements, 68 pod placements and 186 Cheat-only novelty placements. The MiG-31
  line is `MIG31.PT: 3 source rows, 0 optional weapon rows, 75 compatible weapon
  placements, 4 tank placements; defaults and restart passed; catalog 62 normal,
  83 Cheat`.
- Live fire releases the gun from slot 1 (`shots=107 hits=51 kills=1`, ammo
  `[153, 4, 2]`), the `AA9.JT` row from slot 2 (one 1,080 lb store, payload
  4,515 to 3,435 lb) and the `AA8.JT` row from slot 3 (one store, a hit on the
  practice aircraft). With the M31-7 radar review the AA-9 guides and kills the
  range fixture in a 3,600-tick window; before the review the same slot read
  `RADAR OFF`, which was the unreviewed radar, not the row.
- A flare release carries 59 flares with capacity 60; a chaff release carries 59
  chaff with capacity 60. Both come from `MIG31.ECM`.
- The default load is the source default for the human and the AI alike, so no
  separate AI default and no retained-selection seed. Both carriers are paired,
  so their maxima and edits move in twos.

## Cockpit, sensors and instruments

`Airframe::load` accepts `YAK141.HUD` only because its panel field is `~y141_p`,
whose `panel_resource()` is `~Y141_P.PIC`, the identity's instrument panel. The
`--cockpit-sheet mig31` run reported `stem MIG31 cockpit ~Y141H.PIC art 1280x490
FITTED LR`, with two reviewed mirrors at left x 244 y 9 w 272 h 182 and right x
763 y 9 w 272 h 182. The ten instrument pages draw on the Yak-141 bezel; the
cold start shows RADAR OFF and the live-fire frames show TWS with a track. The
sensors are John's reviewed assignments reached by the record names: radar
`MIG31R.SEE` ADVANCED with its own 124 nmi search and 100 nmi track, look-down
30; jammer `MIG31.ECM` LATE with strength 0.30; visual `VIS240.SEE` 10 nmi
search. `--sensor-summary --aircraft mig31` prints `Mig31: radar MIG31R.SEE
search 124 nmi 60x60 deg track 100 nmi 45x45 deg look-down 30 ADVANCED;
infrared none; visual VIS240.SEE search 10 nmi 120x90 deg track 5 nmi 120x90
deg; jammer MIG31.ECM LATE strength 0.30; signature radar 100 infrared 100`.
Absent systems stay absent: no separate targeting sight and no Pave Spike
designator row. See the [cockpit confirmation](mirrors.md#2026-11-08-mig-31-cockpit-confirmation)
and the [radar baseline](radar.md#mig-31-radar-and-jammer-review-2026-11-08-m31-7).

## AI, Quick Mission, wire and replay

The MiG-31 is offered as a Quick Mission player and wingman and flies as a
Fighter in `strike_class.rs`. `--ai-roster-probe-ticks 3600` prints four
`MIG31.PT` lines, Novice, Average, Experienced and Ace, each `ticks=3600
models=2 radar=true ir=false stores=3 gun=true projectiles=0 dropped=0 ...
PASS`, so the AI input is finite and replay-exact. `stores=3` is its three source
rows. The 6-bit wire codes the identity by its `SELECTABLE` index 39;
`checkpoint_shared.rs` holds `Mig31 = 39`. Session, checkpoint and replay round
trips cover it generically through `SELECTABLE`, and `wire-golden.txt` did not
change. Quick Mission restart passes and the creator carries the exact identity
in every wing field. The lobby's Load Ordnance page shows the decoded default
load and warns when a laser store has no designator, showing the Pave Spike pod
helper because row 4's decoded mask admits a pod.

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, under the `M31-2` to `M31-9`
directories (skin and damage views, control poses, gear, brake and burner
stages, store views, ordnance and countermeasure frames, cockpit, HUD and
instrument pages, and the Quick Mission and lobby frames). Observed points: no
underside show-through or clipped marking in smooth and stepped modes; no
half-moved panel; the gear tucks as three rigid assemblies; the dorsal brake and
the two afterburner plumes draw; the four AA-9s hang in two tunnel pairs and the
AA-8s on their wing pylons with no body inside the wing and nothing in space;
and the cockpit and Quick Mission pages name the exact MiG-31 identity. The
frames are local evidence and are not committed.

## Full checks, 2026-11-08

Every command ran through `.local/m3-imports/light.sh` from the repository root,
and the windowed smoke test through `tools/agent-run.sh`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passing |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passing |
| `cargo test --workspace --locked` | 7,477 passed, 0 failed, 108 ignored across 64 test binaries |
| `cargo build --workspace --locked` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 559 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,233 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (375 tracked files; the new baseline carries the header and is checked when staged) |
| `tools/agent-run.sh target/debug/tore-app --smoke-test` | `requested screen presented successfully` |

The smoke test also reported `Cockpit mirrors: 3 reviewed regions, 768x384 rear
feed, every other visible frame`, the shared startup screen. The workspace test
log is at `.local/m3-aircraft/checks/workspace-test.log`.

## Roster capacity finding

The wire codes an aircraft identity as its `AircraftId::SELECTABLE` index in 6
bits ([the wire format](../formats/net-protocol.md)), so the protocol can carry
64 identities at most. With the MiG-31 the list holds 40: 39 retail aircraft and
the F/A-XX concept. Only 24 more aircraft fit before a protocol change. Porting
all 79 remaining decoded PT records needs a wider identity field first; that is
a protocol decision for John, not an aircraft-import decision.

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight or performance measurement was run.
`--native-flight-tables` needs the extracted `sine-q15.bin` and `atan-pa.bin`,
which are not present locally, so it stays a separate restricted path and was not
run. The flight and rendered evidence above comes from the Linux development host
and the wrapped captures.
