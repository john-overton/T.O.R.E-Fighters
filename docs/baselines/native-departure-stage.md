# Joined native departure stage — 2026-09-15

## Scope

Native source contracts now run together in a renderer-independent diagnostic
stage, with separate movement angles, display offsets, body rates, signed speed,
warning/spin/tumble state and explicit clock/RNG inputs. This advances native
implementation; it does not complete a flight adapter or demonstrate retail
trajectory parity. The legacy and hybrid runtime adapters were not changed.

Source identities are the EXE/SMS and F18/RAFALE hashes recorded in
[flight response](flight-response.md#source-identity). New fixed-address slices
are hash-gated in the repeatable static extraction pass. All original tables,
resources, disassembly and probe logs remain ignored locally.

## Reproduce

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-departure-stage/native
cargo run --locked -p tore-formats --example native_departure -- .local/native-departure-stage/native/tables/sine-q15.bin .local/native-departure-stage/native/tables/atan-pa.bin .local/flight-response/validated-f18/FA_2.LIB/F18.PT .local/flight-response/validated-rafale/FA_2.LIB/RAFALE.PT
```

The PT paths are outputs of the existing `--validate-flight` workflow; substitute
your extracted paths as needed. The example uses bounded file reads and reports
its diagnostic scope. It rejects nonzero source `vtLimitDown`, whose full support
path is outside this two-aircraft probe.

## Conditions and results

Eight cases pass: left/right tumble and left/right spin for each aircraft.
Every stage update and RNG state matches replay from identical initial state.
These are scripted component conditions, not measured flight trajectories:

- Fixed native service input of two time units; current time starts at 1,000.
  This is an explicit probe cadence, not a recovered scheduler or 120 Hz claim.
- Envelope queries use 1G, altitude 15,000 ft, clean devices, source structure
  limits and zero global flags. Both profiles have `vtLimitDown=0`. The isolated
  probe explicitly supplies bank from movement; the stage itself requires the
  caller's native body-bank word and does not assume that equivalence.
- Tumble starts at 75° movement pitch, ±5° movement roll and signed 100 fps,
  with warning two units before expiry. No force stage changes its speed.
- Spin starts at 180 fps with warning active and ±5° movement roll; full positive
  pitch/directional rudder for 300 updates, then negative pitch/opposite rudder.
  Speed changes only through the translated native spin slew. Throttle is zero.

| Observed diagnostic result | F/A-18D | Rafale C |
| --- | --- | --- |
| Tumble deadline from time 1,000 / 75° pitch | 1,406, either direction | 1,406, either direction |
| Tumble movement orientation changes | Yes, both directions | Yes, both directions |
| Spin enters | Both directions | Both directions, stricter source entry profile |
| Spin recovery time | 1,854 native units | 1,854 native units |
| Movement pitch at recovery | −22,528 fixed8 (−88°) | −22,528 fixed8 (−88°) |
| Speed at recovery | 88,880 fixed8 | 88,880 fixed8 |
| Normal controls on recovery tick | Skipped | Skipped |

The last three rows are source branch behavior under the stated scripted inputs,
not proof that real retail flight reaches the same state without the omitted
normal forces. After recovery the example does not invent normal-control motion.

Six synthetic joined-stage tests cover distinct envelope roles and flap/difficulty
branches; ground-control boundaries and cached body-bank direction; warning→spin/recovery ordering; warning-expiry
tumble plus atomic error handling; current-G severity and pre-increment timers.
The original tumble, departure, matrix/angle and clock tests remain in place.

## Remaining gates

Normal control and force producers, stalled force-G substitution, full movement
and collision/event order are not connected in a live native flight tick.
Matched retail flight recordings are unavailable. Their absence is a comparison
gap, separate from the known missing implementation. No fitted law or gameplay
addition is used to bridge these gaps. See [native contracts](../formats/native-flight.md#joined-native-departure-stage--2026-09-15).

## Validation

Passed `cargo fmt --all -- --check`, workspace Clippy with warnings denied,
workspace tests and build (all Cargo validation uses `--locked`), all 24 Python
tool tests, and asset guards for the repository, `tore-app` and `tore-extract`.
The static extraction and all eight native-data probe cases passed. Local logs
are under `.local/native-departure-stage/`; no retail derivatives are committed.
No live simulation or rendering behavior changes in this slice. Fresh GPU,
audio/controller, Windows and macOS acceptance checks were not run.
