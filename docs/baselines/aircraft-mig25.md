# MiG-25 acceptance record (M3, 2026-11-07)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, acceptance record for gate F of the
[aircraft import guide](../aircraft-import.md#f-acceptance-record-and-support-status).
The MiG-25 Foxbat (`M25.PT`) is a selectable retail identity in
`AircraftId::ALL` and `AircraftId::SELECTABLE`, appended at checkpoint code 37.
It is not an alias for `MIG31.PT` or a later variant. No retail
comparison exists, so this record claims source-reviewed, extraction-supported,
headless-flight-supported and rendered-flight-supported status, not retail
parity.

## Source identity and dependency closure

The source review is
[MiG-25 source review](../formats/aircraft-variety.md#mig-25-source-review-m3-2026-11-07).
Retail media stays local; no extracted bytes are committed. The user supplied
the same build pair as the 2026-10-05 variety review:

| Archive | SHA-256 |
| --- | --- |
| `FA_1.LIB` | `657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f` |
| `FA_2.LIB` | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |
| `M25.PT` | `63b968033f7cfb410277e7e9b45edf3e57a7639932ee80c1ebd2a3e99eba826a` |

The selected closure (14 resources in `FA_2.LIB` and the same 14 in the
`disc1/SETUP.ESA/FA_2.LIB` installer copy):

| Resource | Role | SHA-256 |
| --- | --- | --- |
| `MIG25.SH` | Shape | `d76f8f7384305426b58543334b629aff757bedddfe2a78e6be6ccdd2393814bb` |
| `MIG25_S.SH` | Shadow shape | `08bb496add40ffc89f79823bb87504f6bfad4bc57d4aa9e86569f47208d785fe` |
| `MIG25_A.SH` | Damage body A | `29479ec7e179f67099b7500c7dd8d62a5f366d30df6c2c24c231103f5861c945` |
| `MIG25_B.SH` | Damage piece B | `7f51676be47af4bd6f22819e32bc1a311554b9c79dd8fe9ca5fd28cd59f47f6b` |
| `MIG25_C.SH` | Damage body C | `b5a55cfec9a888a64e61456af014e69d4f605c44c73a9c1ebe7ea3f7212b5a31` |
| `MIG25_D.SH` | Damage piece D | `1a18c67126b9872222f0af4736d8797f0324a29baa48f3a5057929c13584b51b` |
| `SU33_C.HUD` | HUD and cockpit pointers | `d22b4cd527bd4aaa23eb690fb1a75a7e92e797367f2eded97c3a89f56a3751ef` |
| `MIG27R.SEE` | Radar, BASIC | `33846cc54be0774b243838cba435c844d079525f970f53e89ce66ba9e104475f` |
| `MIG21.ECM` | Jammer, EARLY, chaff 20 and flare 20 | `2d194e3b843aed2bee616278208667bbb68ec5eed06457d081f3f8580943cc3e` |
| `VIS240.SEE` | Visual sensor, signature 0 | `a3224581abef75e0fe4e6dbb37d2d47d5ab4704bb1043e09dd6aa2e5a4510ee1` |
| `AA6.JT` | Default A2A store | `83f67aa812bcac9080186193a84a4812a045458ce46c848d76f516f9e8cd2a90` |
| `AA2.JT` | Default A2A store | `94e415550095ee4323c3d70277e13d825bc49639540d1160f83e1629e73f06cc` |
| `F250.GAS` | Centreline tank, 198 lb empty and 1,650 lb fuel | `974e49de6eb22237bf7a1bd24b3ed9308300bbb95270a9c0fe7104892514f0de` |

The PT names no `M25.PTS` companion; the absence is an explicit exception. The
shape and damage textures are `_MIG25.PIC` and `_MIG25_A.PIC` to `_MIG25_D.PIC`;
the cockpit supplies `~SU33H.PIC`, `~SU33_P.PIC`, the overlays and the fonts. The
HUD also names `~SU33_W.PIC`, which has no archive entry; no code path reads it
and it stays optional and unresolved.

The dependency report and extracted copies live only under
`.local/m3-imports/scratch/mig25/`. The app import command used for the profile
was:

```sh
TORE_DATA_DIR=.local/m3-aircraft/profile cargo run --locked -p tore-app -- \
  --import gameassets/fighters-anthology --import-only --no-audio
```

The selected CLI extraction was:

```sh
cargo run --locked -p tore-extract -- --source gameassets/fighters-anthology \
  --out .local/m3-imports/scratch/mig25 --include 'M25.PT' --include 'MIG25*' \
  --include 'SU33_C.HUD' --include 'MIG27R.SEE' --include 'MIG21.ECM' \
  --include 'VIS240.SEE' --include 'F250.GAS' --include 'AA6.JT' --include 'AA2.JT'
```

The import report listed every required resource and no missing required
reference. Extraction success alone does not enable flight; the reviewed
identity does.

## Support status

| Term | Status | Evidence |
| --- | --- | --- |
| Source reviewed | Yes | [Source review](../formats/aircraft-variety.md#mig-25-source-review-m3-2026-11-07) |
| Extraction supported | Yes | Closure above and the app import report |
| Headless flight supported | Yes | `--headless-flight`, `variety_flight`, `envelope_probe` and the battery family |
| Rendered flight supported | Yes | Frame pack below and the `render-mig25-skin` scenario |
| Systems partially supported | Yes | Reviewed radar, ECM, visual, HUD, cockpit and stores; retail systems comparison unavailable |
| Retail validated for named cases | No | Retail comparison is unavailable |

## Implemented, fitted and unavailable subsystems

| Component | Status | Provenance |
| --- | --- | --- |
| Identity, `M25.PT`, size 588, checkpoint 37 | Implemented | Spec-derived |
| `MIG25.SH`, exponent 8, 320 runtime neutral faces | Implemented | Spec-derived |
| Damage looks A to D and the shadow | Implemented, drawn through the shared damage path | Spec-derived |
| Stabilators, ailerons, flaps and twin rudders | Implemented as whole source panels | Fitted hinges and roles over source geometry; angles fitted |
| Gear retraction and nose steering | Implemented as three rigid assemblies | Fitted motion over source panels; no source gear doors |
| Airbrake and afterburner plume | Implemented | Roles and roots source-derived; travel and scale fitted |
| Decoded stations, pylons and stores | Implemented | Spec-derived positions and masks; shared pylon rules opinionated (John, 2026-10-13) |
| `AA6.JT`, `AA2.JT` and `F250.GAS` default load | Implemented | Spec-derived |
| Chaff 20 and flares 20 | Implemented | Spec-derived from `MIG21.ECM` |
| `SU33_C.HUD`, `~SU33_P.PIC`, aperture, overlays and mirrors | Implemented | Spec-derived shared cockpit |
| Radar `MIG27R.SEE` BASIC, ECM `MIG21.ECM` EARLY, visual `VIS240.SEE` | Implemented and reviewed | Spec-derived |
| No gun | Implemented as absent | Spec-derived |
| Hybrid researched flight on the aircraft's own `variety` configuration | Implemented | Spec-derived record, fitted host response |
| AI and Quick Mission | Implemented as Fighter | Spec-derived family binding |
| Wire, checkpoint and replay | Implemented append-only | Spec-derived 6-bit selectable index |
| Destroyed whole-body variant | Unavailable | Shared with the F-104; no damage variant registered for the identity |
| `~SU33_W.PIC` | Unavailable and optional | No archive entry; no code path reads it |
| Retail comparison | Unavailable | No running retail copy |

Open decisions and listed retail values are in the
[source review](../formats/aircraft-variety.md#mig-25-source-review-m3-2026-11-07):
the empty weight, internal fuel, maximum takeoff weight and thrust differ from
published figures, the military and afterburner fuel flows are low, and the
values remain retail. The gear doors, the retraction timing and the steering
rate stay fitted or unmapped rather than silently animated.

## Flight suite

Measured on the default hybrid `--researched-flight` path with the aircraft's own
`variety` configuration. Details and provenance are in
[MiG-25 validation](variety-flight.md#mig-25-validation-2026-11-07) and the
[source and flight review](../formats/aircraft-variety.md#m25-8-flight-review-2026-11-07).

| Case | Result |
| --- | --- |
| `variety_flight` | `aircraft=MiG-25 conventional=pass deterministic=pass landing=pass ticks=1200` |
| 1 G top speed | 1,623 kt at 36,000 ft decoded, matching the published 1,620 kt |
| Envelope level speeds | 611 / 830 / 1,073 / 1,316 / 1,462 kt at 1,000 to 36,000 ft, ratio 0.901 to 0.900 of the edge |
| Full-stick pull | 3.89 to 3.90 G at 1,000 ft; the 5 G row gives 3.86 G |
| Level, 7,200 ticks | 517.4 kt, 1.01 G, no crash |
| Pull, 7,200 ticks | 511.3 kt, 3.89 G, no crash |
| Loop, 7,200 ticks | completed at 4,635, 597.1 kt, 3.91 G, no crash |
| Sprint, 36,000 ticks | 714.6 kt, 1.00 G, no crash |
| Climb, 216,000 ticks | 1,078.1 kt peak, 69,592 ft, overspeed loss at 66,028 ft |
| Dive, 9,000 ticks | 1,047.7 kt peak, overspeed loss at 6,440 |
| Overspeed, 1,200 ticks | 1,310.7 kt, loss at 720 |
| Rudder, 360 ticks | 13.0 degrees yaw, 3.8 degrees bank, no crash |
| Devices, 3,600 ticks | 511.7 kt, no crash |
| Nosewheel, 360 ticks | 11.2 kt, no crash |
| Stall recovery | alert at 2,524, recovered at 3,004, 0 ft lost, no crash |
| Spin recovery | entered at 3, recovered at 1,292, 0.66 revolutions, 2,328 ft lost, no crash |
| Takeoff, `UKR` ground start | liftoff 1,952 at 153.9 kt over 2,161 ft, 2,776 ticks to 100 ft |
| Takeoff with gear airborne | 2,773 ticks |
| Landing | touchdown 164.0 kt, 8.5 ft/s sink, 1,689 ft left, score 100 |
| Landing gear up | crashed for the gear |
| Hard landing | touchdown 172.4 kt, 33.9 ft/s sink, 2,295 ft left |
| Landing off runway | crashed for the not-landable ground |

G limits are retail +5 / -2. The default load gross is 89,827 lb, 1,003 lb
below the record's 90,830 lb maximum. Wind, buffet and departure were not probed
as separate MiG-25 cases; the shared weather, response and contact paths are
unchanged by this port. Push, a steady turn, roll release and a dedicated
fuel-endurance case were also not run as separate MiG-25 scenarios; the same
flight suite covers them on the other identities. The full
flight battery family passed 9 of 9: `flight-animation-mig25`,
`flight-liftoff-mig25`, `flight-liftoff-retail-mig25`,
`flight-approach-mig25`, `flight-variety-mig25`,
`flight-jettison-mig25-slot2`, `flight-livefire-mig25-slot1`,
`flight-livefire-mig25-slot2` and `render-mig25-skin`.

## Controls, devices, equipment and ordnance

- The animation probe reports 320 neutral faces and 0 required or check
  failures in the `reviewed-controls-and-attachments` scope. Every panel moves
  whole: anchor and skin gaps are 0 in all 25 flap and roll combinations, and no
  triangle is left behind a hinge.
- The dense gear sweep holds 203 rows with assembly rigidity error at most
  0.000001 ft, 0 anchor and skin gaps, 0 crossings and a minimum main wheel gap
  of 5.33 ft.
- Probe coverage reads `gear required moved true 22`, `brake required moved
  true 5` and `exhaust required moved true 5`.
- Live fire releases `AA6.JT` from slot 1 and `AA2.JT` from slot 2; the ammo
  vector is `[2, 2]`, matching the two source wing rows. Jettison of the outer
  row passes.
- A flare release carries 19 flares after one press with capacity 20; a chaff
  release carries 19 chaff with capacity 20. Both come from `MIG21.ECM`.
- `--validate-ordnance` passes 37 aircraft with 3,684 weapon placements, 182 tank
  placements, 65 pod placements and 174 Cheat-only novelty placements. The
  MiG-25 line is `M25.PT: 3 source rows, 0 optional weapon rows, 58 compatible
  weapon placements, 2 tank placements; defaults and restart passed; catalog 29
  normal, 83 Cheat`.
- The decoded wing mask admits the 29 supported records listed in
  [the ordnance review](../formats/aircraft-ordnance.md#the-mig-25-m3-2026-11-07);
  the default load is `AA6.JT` x2, `AA2.JT` x2 and `F250.GAS` x1.
- The AA-6 upper fins enter the wing underside by 0.27 to 0.30 ft on the
  standard 1 ft pylons. John accepted it on 2026-11-07, like the F-14 Phoenix
  clip; the decoded positions ship and no real-station table or `station_move`
  entry was added.

## Cockpit, sensors and instruments

The `SU33` cockpit loads with `SU33_C.HUD` naming `~SU33_P.PIC`, the reviewed
glass aperture, left and right overlays and two mirrors. The forward HUD has no
gun reticle because `guns()` is empty. The systems, target cam, RWR, radar, nav,
weapons and envelope pages draw; absent systems stay absent. With the engine up
the radar page shows TWS and a track from `MIG27R.SEE`; a cold start shows RADAR
OFF. See
[cockpit confirmation](../formats/aircraft-variety.md#m25-7-cockpit-and-hud-confirmation-2026-11-07)
and the [mirror baseline](mirrors.md).

## AI, Quick Mission, wire and replay

The MiG-25 is offered as a Quick Mission player and wingman and flies as a
Fighter in `strike_class.rs`. `--ai-roster-probe-ticks 3600` prints four
`M25.PT` lines, Novice, Average, Experienced and Ace, each `ticks=3600
models=2 radar=true ir=false stores=2 gun=false ... PASS`, so the AI input is
finite and replay-exact. The 6-bit wire codes the identity by its
`SELECTABLE` index 37; `checkpoint_shared.rs` holds `Mig25 = 37`. Session,
checkpoint and replay round trips cover it generically through `SELECTABLE`, and
`wire-golden.txt` did not change. Quick Mission restart passes. Aircraft-switch
state isolation is covered by the shared session tests over `SELECTABLE`, not by
a MiG-25-only capture. The lobby's Load
Ordnance page shows the decoded default load and warns when a laser store has no
designator.

## Visual captures

The frame pack is `.local/m3-aircraft/frames/`, indexed in
`.local/m3-aircraft/frames/INDEX.md`. It holds the M25-2 skin and damage views,
the M25-3 control-surface poses, the M25-4 gear, brake and burner stages, the
M25-5 store views, the M25-6 ordnance and countermeasure frames, the M25-7
cockpit, HUD and instrument pages, and the M25-9 Quick Mission and lobby frames.
Observed points: no underside show-through or clipped marking in smooth and
stepped modes; no half-moved panel; gear tucks rigidly to the belly; the dorsal
airbrake rises; two afterburner plumes show; the stores hang on their pylons with
no body inside the wing; and the cockpit and Quick Mission pages name the exact
MiG-25 identity. The frames are local evidence and are not committed.

## Full checks, 2026-11-07

Every command ran through `.local/m3-imports/light.sh` from the repository root.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passing |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passing |
| `cargo test --workspace --locked` | 7,467 passed, 0 failed, 108 ignored across 64 test binaries |
| `cargo build --workspace --locked` | Passing |
| `python3 -m unittest discover -s tools -p 'test_*.py'` | 558 tests, OK, 5 skipped |
| `python3 tools/check_assets.py` | 2,228 files |
| `python3 tools/check_assets.py target/debug/tore-app` | 1 file |
| `python3 tools/check_assets.py target/debug/tore-extract` | 1 file |
| `python3 tools/check_docs.py` | Passing (373 tracked files; the new baseline carries the header and is checked when staged) |

This slice is documentation only, so no GPU `--smoke-test` was run. The rendered
frames above were captured in the earlier slices that changed presentation.

## Roster capacity finding

The wire codes an aircraft identity as its `AircraftId::SELECTABLE` index in 6
bits ([the wire format](../formats/net-protocol.md)), so the protocol can carry
64 identities at most. With the MiG-25 the list holds 38: 37 retail aircraft and
the F/A-XX concept. Only 26 more aircraft fit before a protocol change. Porting
all 81 remaining decoded PT records needs a wider identity field first; that is a
protocol decision for John, not an aircraft-import decision.

## Not run

Retail gameplay comparison is unavailable, so no claim of retail parity is made.
No Windows or macOS runtime check, physical controller or HOTAS flight, audible
review, human LAN flight or performance measurement was run. The flight and
rendered evidence above comes from the Linux development host and the wrapped
captures.