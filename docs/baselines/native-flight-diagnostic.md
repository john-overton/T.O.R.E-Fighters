# Joined native flight diagnostic

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence, research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature; see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15. **Native translations, diagnostic connection; no live activation or
retail trajectory acceptance.** This completes the normal-control/departure/
force/movement/contact diagnostic continuation for F18.PT and RAFALE.PT.
Source identity and executable/build distinctions are recorded in
[native flight research](../formats/native-flight.md); aircraft identities and
PT hashes are in the [departure baseline](native-departure-stage.md).

Runtime follow-up: the [airborne live connection](native-live-flight.md) now
reuses this service. The results below remain diagnostic evidence; their original
no-live-activation scope does not describe the newer opt-in mode.

## Reproduction and results

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-flight-diagnostic/native
cargo run --locked -p tore-formats --example native_flight -- .local/native-flight-diagnostic/native/tables/sine-q15.bin .local/native-flight-diagnostic/native/tables/atan-pa.bin .local/flight-response/validated-f18/FA_2.LIB/F18.PT .local/flight-response/validated-rafale/FA_2.LIB/RAFALE.PT
```

Paths to user-owned extracted PTs can be replaced. No retail bytes are committed.
The extraction passes with 107 symbol spans and 3,829 symbols, plus reviewed
region slices. Output used here is `.local/native-departure-stage/joined-probe.txt`.

Both aircraft pass all 21 cases: level, pull, push, left/right roll, left/right
rudder, stall, left/right spin, ground, water, power, control disturbance, flaps,
brake, loading, damage, crosswind, ground steering and gear-up contact. Each case
advances 900 services, feeding the complete returned diagnostic state into the
next service. All **37,800 service updates** are repeated from identical prior
state and RNG and compare state, RNG and returned events. This establishes
repeatability of this translation, not agreement with the original executable.

Both directional spin cases assert entry and recovery; the stall case asserts
tumble. Water and gear-up cases assert native contact codes 7 and 5 respectively.
Control requests last 300 services before release; spin recovery uses full push
and opposite rudder. Aircraft source asymmetries are preserved, not fitted away.
Existing departure/force/movement snapshot regression probes also pass.

Synthetic tests cover loaded-G interpolation, load/difficulty adjustments,
rudder/slip/release/damage, ground steering ties and speed limits, auxiliary
scaling, passive low-speed fall, pitch-dependent idle drag, control-disturbance
selection/release and timer boundaries. Joined tests check query order and atomic
state/RNG rollback on a late query failure, rejection of unsupported environmental
turbulence, and the recovery tick's skipped normal controls followed by forces,
movement and contact. Contact clears combined rates before auxiliary subtraction.

## Explicit scope and provenance

**Native:** arithmetic and branch order come from the reviewed FA executable.
The [source contract](../formats/native-flight.md#joined-flight-diagnostic-2026-09-15)
identifies the translated consumers. Exact tumble composition PA words are kept
for subsequent forces instead of round-tripping through degree angles.

**Authored diagnostic inputs:** services use two native time units and scripted
commands, initial states and flat contact queries. This is not the live 120 Hz
clock or native scheduler. Air cases start at 15,000 feet; ordinary cases use
500 fps, spin uses 180 fps and stall uses 100 fps with 75° movement pitch.
Power/control-disturbance cases hold 100% throttle and 1,000 lb fuel; other cases
use zero fuel/throttle. Loading supplies 1,000 lb of stores; damage supplies
explicit subsystem percentages. Crosswind supplies 20 fps at 90°. These are
probe choices, not aircraft tuning or recovered mission conditions.

**Verified native bypass:** airborne services require global flag `0x01000000`
(environmental turbulence disabled). Native `0x477590` then skips environmental
motion and resets its scheduling words; those unused environmental timer words
are outside this diagnostic state. Grounded services follow the same source
bypass. The joined service rejects the unsupported airborne enabled branch.
The independently translated normal-control disturbance selector uses explicit
numeric codes; their gameplay producers remain external.

**Remaining implementation:** native setup refresh cadence, fuel transfer/burn,
engine/device actuator and damage lifecycle producers, terrain/object/carrier
query production, environmental turbulence, and event dispatch are external.
The diagnostic returns departure/contact/high-G events without executing sound,
damage or ejection callbacks. Flat samples do not validate theater runways.
Native scheduling/global RNG order and live adapter replacement remain open.
A useful retail comparison is unavailable per the user; it does not block this
diagnostic completion and no retail parity is claimed.

## Validation

Passed formatting, workspace Clippy with warnings denied, workspace tests
(332 tests), workspace build with `--locked`, all 24 Python tests, static native
extraction and both native examples. Repository and app/extractor asset guards
pass. Logs: `.local/native-departure-stage/checks/joined-*.txt`.
No renderer or live adapter changed, so no new GPU/audio/controller check was
required. Windows/macOS acceptance was not run.
