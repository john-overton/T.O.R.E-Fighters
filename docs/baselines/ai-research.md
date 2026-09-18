# Aircraft and surface AI research baseline

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research and implementation, 2026-09-17. John requested an FA aircraft AI
review, experience mapping, surface AI investigation where feasible, and a plan
to prepare behavior before later hookup, then authorized continued research
and implementation of specified behavior. No retail executable or module was
executed. Isolated components exist; no controller is hooked into missions.

## Inputs and identity

| Local input | Bytes | SHA-256 |
| --- | ---: | --- |
| `gameassets/fighters-anthology/FA.EXE` | 1319424 | `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c` |
| `gameassets/fighters-anthology/FA.SMS` | 106706 | `e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0` |
| `gameassets/fighters-anthology/FA_2.LIB` | 31546692 | `fb8b30216e739292489d4872cc440debec334e14f8b9a3d0e340092445246198` |

The installed-root resource census also inspected FA_1.LIB, SHA-256
`657254c5bb3bcf3609b3e84ee6499bf80395a2daffc60c12363e534cf408245f`;
FA_4B.LIB, `34b04faaae90b857c04cb0ab7fedb6d00f19523871a3c075cf0e357d66a19f62`;
and FA_4D.LIB, `247ff0fe70975d24f8f4187fe8d62e3833546b71ba199d8117fc0546865a453c`.

The EXE/SMS hashes match the existing flight research allowlist. SMS contains
3829 symbols. No marketing version number was inferred from filenames.
`FA - Copy.EXE` is a different, untraced build, 1299968 bytes with SHA-256
`c7d2c1cc9d27a6b364eca4245892cb6ca61ca72afc9a46e5760ee1fe7d75ba9b`.
Do not mix addresses between them.

The previously downloaded local manual text at
`.local/missile-update/manual.txt`, Quick Mission skill description on printed
page 19, corroborates per-wing selection and randomized individual skill.
This pass inspected text, not the page image. The USNF-ATF checkout was inspected
as a reference only, particularly `Docs/formats/ai.md`, `Docs/formats/mission.md`,
`engine/src/sim/ai/` and `engine/src/sim/combat/world.ts`.

## Static inspection

Commands run from the repository root:

```sh
target/debug/tore-extract --source gameassets/fighters-anthology/FA_2.LIB \
  --out .local/ai-research/media --include '*.AI' --include '*.BI' \
  --include '*.PT' --include '*.NT' --include '*.OT'
llvm-objdump -d --x86-asm-syntax=intel \
  gameassets/fighters-anthology/FA.EXE > .local/ai-research/fa.disasm
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x466e00 --stop-address=0x466e49 \
  gameassets/fighters-anthology/FA.EXE
```

Python standard-library inspection reused `symbols` and `sections` from
`tools/extract_native_flight.py`, calculated SHA-256 identities, enumerated
`:ctName` string bindings and `utilProc` symbols, and counted packed chance
constant occurrences in BI data. The local `census.json` records every source
and module hash, object binding and checked literal. All extracted content and
disassembly remain under ignored `.local/ai-research/`.

Selected spans were re-decoded from instruction boundaries with
`--start-address` and `--stop-address`. This matters: whole-section linear
disassembly misaligned `_GVProc` after preceding embedded data. The bounded
decode at `0x473db0` correctly starts with `xor ecx, ecx`. Symbol names and
whole-section text searches alone were not treated as behavioral proof.

Results: nine source/module pairs, 145 PT records, 84 NT records and 170 OT
records in the inspected archive. The current `AircraftId::ALL`, `pt()` and
`label()` definitions were also checked against the extracted PT bindings:
all twelve ported aircraft name `f.BI`. The exact roster has one home in the
[specification table](../spec/ai-experience.md#currently-ported-aircraft). All source chance constants occur as packed
push-immediate patterns in the paired modules. No complete bytecode decoding,
loose-file/disc override census or original-game execution was performed. Family
counts and routine conclusions have a single home in the
[source notes](../formats/ai.md); behavioral numbers are in the
[main specification](../spec/ai.md) and [experience specification](../spec/ai-experience.md).

The same include filters were used with `--list` on each of the four installed
root archives. FA_2 had 417 matching entries out of 5405 unique entries;
FA_1 had zero out of 2001, FA_4B zero out of 77 and FA_4D zero out of 22.
The three no-match calls returned exit code 1 with `No resources matched`,
which is an expected filtered-list result. The local
`behavior/archive-census.json` and `behavior/FA_*.LIB.list` retain the results.
This establishes installed-root coverage of these resource classes, not
original precedence or coverage of every file on other media.

The behavior pass inspected active handlers in all nine AI sources and bounded
executable spans for target geometry, distance/altitude, performance predicates,
motion input limits, engagement pitch and Quick Mission skill emission. The
[source map](../formats/ai.md#behavior-source-points) records the reviewed
entry points and separates located services from recovered behavior.
For example, the Quick Mission writer and its callers were inspected with:

```sh
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x432240 --stop-address=0x432610 \
  gameassets/fighters-anthology/FA.EXE
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x4316c2 --stop-address=0x43193b \
  gameassets/fighters-anthology/FA.EXE
```

Local slices in `behavior/` include exploratory decodes as well as reviewed
ones. In particular, the exploratory `speed.txt` starts inside an instruction
and is not evidence for a closed speed rule. The source map, rather than mere
presence of a local slice, defines the claims made by this pass. The Quick
Mission writer emits the selected skill for each member, but loader-side
variation remains unresolved. No dynamic comparison was performed.

The continued pass traced the clock-frequency producer through simulation time,
read movement/speed dispatch tables as bounded inert dwords, and followed the
pursuit target snapshot into speed regulation. It also inspected target
retention/ranking, weapon-service transitions, wing action setters and formation
motion. The reviewed claims and exact source spans are indexed in
[the follow-through source map](../formats/ai.md#clock-pursuit-and-service-follow-through).
Representative additional decodes were:

```sh
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x437ecb --stop-address=0x4382d0 \
  gameassets/fighters-anthology/FA.EXE
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x4c4100 --stop-address=0x4c4700 \
  gameassets/fighters-anthology/FA.EXE
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x4c4700 --stop-address=0x4c5000 \
  gameassets/fighters-anthology/FA.EXE
```

`continued/manifest.json` retains rechecked input identities, slice hashes and
the two dispatch tables; `continued/roster-npc.json` retains each PT hash and
its NPC values. OBJECT/NPC directive widths were checked against the repository
schema for all twelve records. A strict directive-name check initially rejected
`dword 0` in a pointer slot; the corrected check allows matching four-byte
representations and verifies widths through the NPC fields. An exploratory
direct-byte read was discarded after confirming that these extracted files are
BRF text. Documented values come from the schema-aligned directive stream.

This pass establishes nominal clock units, pursuit separation bands, partial
target-selection rules, per-aircraft service delays and formation variation.
It does not dynamically validate a trajectory, fire a weapon, or complete the
seeker, launch, wing-receiver or interruption contracts. No new synthetic AI
implementation was added, so repository tests are not evidence of those gameplay
rules running successfully.

## Steering and release inspection

The continuation inspected 13 bounded executable spans for steering axes,
rate helpers, lead, seeker geometry, lock/support, release, ammunition, wing
receivers and approach construction. The local `closure/spans.json` records
start/stop addresses and `closure/manifest.json` records the input hash and
SHA-256 of every saved decode. Commands use the same `llvm-objdump` options
shown above with those bounds. These are static inspections, not runtime tests.
No extracted bytes or disassembly are added to the repository.

Results and limits are recorded once in [B44 through B46](../spec/ai.md#b44-steering-execution-and-pursuit-lead)
and the [source map](../formats/ai.md#steering-seeker-release-and-receiver-follow-through).
This closes selected sender/consumer connections and corrects the proposed
inventory feedback contract. It does not establish complete steering, all
store profiles, in-flight support transitions or every wing radio order.

## Skill, performance, wing, threat and route inspection

The 2026-09-17 continuation ran five bounded research traces in parallel and
a parent trace of the surface event handler, all on the same hashed FA.EXE
(re-verified before each address was quoted). Bounded decodes are saved under
local `session3/` (`llvm-objdump -d --x86-asm-syntax=intel --start-address
--stop-address` as above; a SHA-256 manifest covers the performance set).
Representative commands:

```sh
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x481c10 --stop-address=0x482f30 \
  gameassets/fighters-anthology/FA.EXE   # mission text parser
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x42df80 --stop-address=0x42e0b6 \
  gameassets/fighters-anthology/FA.EXE   # terrain pitch floor
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x4c0f49 --stop-address=0x4c1022 \
  gameassets/fighters-anthology/FA.EXE   # launch warning delay
llvm-objdump -d --x86-asm-syntax=intel \
  --start-address=0x473f50 --stop-address=0x474300 \
  gameassets/fighters-anthology/FA.EXE   # surface event handler
```

Data-section reads (the formation table at `0x4f6cb8`, the experience delay
table at `0x50ce18`, the name and phrase tables) used the PE section map with
`.rdata` at file offset 950272 and `.data` at 955904. Local PT values quoted
in the source map (`minAlt`, `maxClimb`, `maxAlt`, roll limits) were read from
the ignored roster extraction by walking the directive stream in schema order.

Results: Quick Mission skill, saved-skill precedence and the G exemption are
closed; speed units, turn and roll capability, terrain avoidance and several
steering overrides are closed; formation geometry, names, player spacing
values, mode 9 speed, the wing order map, control side effects, radio rules
and rejoin are closed; launch-warning eligibility and delay, countermeasure
selection and inventory, reason ranking, waypoint execution and completion,
and fuel states are closed. The seeker and signature trace was interrupted
twice by session limits and is recorded as open. The surface handler dispatch
was inspected but its event meanings remain open. Every closed item is stated
in [the main spec](../spec/ai.md) and indexed in
[the source map](../formats/ai.md). No retail module was executed and no
dynamic comparison was performed.

## Components and runtime

The established rules are implemented in `crates/tore-sim/src/ai/` as
renderer-independent calculations with synthetic tests: experience, geometry,
tactics, motion, pursuit, targeting, steering, weapon service, wing, threat and
route. Each test checks a specified number or transition (for example B01 at
89, 90 and 91 degrees; B15 at each side of every band edge; B41 at 19999 and
20000 ft; B42 timing groups for all twelve aircraft; B43 spacing clamps at
511, 512, 20000 and 20001 ft; the warning delay examples in B47). Unresolved
rules still return an explicit unspecified-rule error.

On 2026-09-17 those components were joined into a runtime. `controller`
sequences them per actor and holds the persistent state and one seeded draw
stream. `fitted` supplies one named rule per unresolved branch so a live actor
cannot stall, and every use is recorded per actor. `steering_adapter` converts
a maneuver into controls for that actor's own flight model, then enforces the
B44 attitude request through a fitted integration boundary. The AI-only
experience G adjustment and fitted damage scaling feed its achieved bounds. `mission` gives each actor its own sensors,
stores, flight model and decision state, debits ammunition before emitting a
launch event, and does not duplicate missile physics. `launch` carries the
Quick Mission payload.

These tests check specified numbers, named fitted rules and deterministic
headless scenarios. They are not evidence of retail parity, and no retail
comparison was available.

### Headless scenarios exercised

The 2026-09-17 defect repair pass adds regression coverage at the controller,
mission and live combat boundaries. These tests establish the stated conditions,
not retail parity or full gameplay acceptance.

| Review item | Validation |
| --- | --- |
| AI-01 | Step combat then the real bridge for 1500 ticks, twice from fresh state; destroyed airframe falls from 1000 ft, reaches ground and stops |
| AI-02 | Lateral pursuit points toward the target and follows a moved target without changing maneuver identity; a break keeps its own heading |
| AI-03 | Fresh reports at every experience level, distances 0, 10559, 10560, 211200 and 211201 ft; no early reaction, one due delivery |
| AI-04 | Deplete one missile, then two ten-round synthetic gun releases through the bridge; each projectile retains its own weapon. A station absent from the player configuration still simulates safely |
| AI-05 | Negative angle, minimum/maximum range, emission, sensor support and terrain gates; a ridge between live mission actors prevents releases without ammunition loss |
| AI-06 | Individual releases at ticks 0 and 30, third stopped by depletion; live decoy affects only matching missiles targeting the releaser and creates an effect |
| AI-07 | All twelve distinct model variants tested over roll-in and reversal, at full and quarter health; achieved roll and heading changes stay inside supplied B44 bounds |
| AI-08 | Same-wing assignments change live ranking; another wing's assignment does not contribute |
| AI-09 | Aligned/failed-envelope station inputs and end-to-end equal-store selection, without rewarding narrower cones |
| AI-10 | All 36 saved/new priority pairs preserve or replace active motion as specified |
| AI-11 | Player keyboard dispatch, receiver motion and same-side/same-wing isolation; automatic requests dispatched after the decision pass |
| AI-12 | 48 synthetic combinations construct each exact aircraft model with distinct envelope data and stores; separate imported-media pass below |
| AI-13 | Claims corrected to distinguish isolated arithmetic, boundary regressions, imported-media probes and visual acceptance; authored rules indexed in provenance |
| AI-14 | Reconciled B47 prose with the existing source map: attack-state terms are base delays, with distance and experience added. No behavioral change |

Existing tests also cover deterministic repeated runs, 2v2, independent actors,
self/friendly exclusion, finite ammunition, unique request identities, bingo
fuel and out-of-fuel activity.

### Imported aircraft validation

`target/debug/tore-app --ai-roster-probe-ticks 3600 --no-audio` passed all 48
combinations of twelve exact aircraft identities and four experience levels.
Each case steps two imported AI models for 30 simulation seconds, through
combat and the bridge, checking finite position every tick and measuring
achieved heading and bank changes. Imported PT, weapon and sensor records are
used, including `F18.PT` for the F/A-18D and `RAFALE.PT` for the Rafale C.
The FA.EXE and FA_2.LIB hashes were rechecked against the build table above.

The pass created 87 projectiles and dropped none. Maximum measured bank rate
was 45.000 degrees/second and heading rate 12.018 degrees/second. All cases
loaded their own gun and default stations; radar and optional infrared fit
were reported per aircraft. Zero launches in some combinations are not a
failure or proof of combat effectiveness. This is finite-duration integration
validation, not seeker lifecycle, victory, or retail trajectory acceptance.

## What this establishes

There is enough FA-specific evidence to plan all aircraft families and a
separate surface workstream. Experience is a real per-object input with multiple
specific effects, not evidence for a universal difficulty multiplier. The
earlier checkout supplies useful leads, but its runnable pursuit controller and
unconnected interpreter do not establish FA tactical parity.

The work does not yet establish complete maneuver shapes, seeker envelopes and
store selection, surface class behavior, recovery sequences or full
source/compiled agreement.
Retail comparison remains unavailable and is not an acceptance blocker. Future
acceptance uses specified numbers and synthetic deterministic scenarios; any
unresolved component implemented by choice must be labeled fitted or opinionated.

## Repository validation

The completed 2026-09-17 repair pass passed the required checks on Linux:

- `cargo fmt --all -- --check`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo test --workspace --locked`: 836 Rust tests, none ignored.
- `cargo build --workspace --locked`.
- `python3 -m unittest discover -s tools -p 'test_*.py'`: 40 tests.
- `python3 tools/check_assets.py`, plus scans of `target/debug/tore-app`
  and `target/debug/tore-extract`.
- `python3 tools/check_docs.py`.
- `cargo run --locked -p tore-app -- --smoke-test`: requested screen presented
  on NVIDIA GeForce RTX 4070, Vulkan.

Repair logs are local in `.local/ai-defect-validation/`. The renderer smoke is
startup/presentation evidence, not visual acceptance of all wing maneuvers,
projectile shapes or countermeasure effects. Windows and macOS were not run.
Full retail combat comparison remains unavailable. AI missile seeker activation
and pitbull, remaining original maneuver shapes, and the B12 wing-approach
producer remain open. Fitted control coupling, damage authority, device visuals
and coasting are specified in the [live integration rules](../spec/ai.md#live-integration-and-authored-boundaries).
