# Aircraft and surface AI research baseline

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-17. John requested an FA aircraft AI review, experience
mapping, surface AI investigation where feasible, and a plan to prepare behavior
before later hookup. This pass changes documentation only. No retail executable
or module was executed and no AI controller was implemented.

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

## What this establishes

There is enough FA-specific evidence to plan all aircraft families and a
separate surface workstream. Experience is a real per-object input with multiple
specific effects, not evidence for a universal difficulty multiplier. The
earlier checkout supplies useful leads, but its runnable pursuit controller and
unconnected interpreter do not establish FA tactical parity.

The work does not yet establish complete flight maneuvers, threat awareness,
firing decisions, surface experience scaling, or full source/compiled agreement.
Retail comparison remains unavailable and is not an acceptance blocker. Future
acceptance uses specified numbers and synthetic deterministic scenarios; any
unresolved component implemented by choice must be labeled fitted or opinionated.

## Repository validation

All required repository checks passed on the current Linux host:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`: 502 Rust tests passed, none ignored.
- `cargo build --workspace --locked`
- `python3 -m unittest discover -s tools -p 'test_*.py'`: 40 tests passed.
- `python3 tools/check_assets.py`, including separate scans of
  `target/debug/tore-app` and `target/debug/tore-extract`.
- `python3 tools/check_docs.py`, plus direct use of its header validator on the
  four new untracked documents, which the tracked-file scan does not visit.

Logs are local in `.local/ai-research/checks/`. These checks protect the existing
repository; they do not validate newly implemented AI, since there is none.
No rendering smoke is required for this documentation-only change. No Windows
or macOS execution or visual/gameplay acceptance was performed.
