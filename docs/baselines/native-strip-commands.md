# STRIP command and default-event checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `bc709c1`. **NE-00.1l completes bounded source recovery
and one diagnostic deadline helper**, not full movement/event execution.
[Contract](../formats/native-strip.md#initial-commands-and-default-event-response--ne-001l),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-commands-aligned-source
cargo test --locked -p tore-sim command_deadline
```

Twelve additional reviewed ranges establish initial-command consumers, completion
conditions, command constructor/reset, script entry gates and default event
response. Initial command completion can replace flags/mask/deadlines even at
zero speed. Command time saturates at 0x7fff; it does not wrap like service and
speech deadlines. The helper test covers zero, the selected 240-unit delay,
equality/overflow at 0x7fff, unsigned high-bit operands and sums exceeding a word.
No command/script interpreter or scheduler is activated.

The first extraction attempt correctly failed because the global linear sweep
missed aligned entry 0x4382d0 after an embedded jump table. The extractor now
disassembles every reviewed flight region independently from its reviewed bounds,
checks returned addresses and retains the global sweep only as an exploratory
reference inventory. Two synthetic Python tests cover the missed-entry case and
empty/wrong-entry/duplicate/out-of-range output. The previous **168** regions'
normalized instruction text is unchanged (symbol annotations ignored).

Fresh and repeated extraction pass: **180 reviewed regions**, 107 selected spans,
3829 symbols and unchanged reviewed EXE/SMS identities. Code/data jump tables
were inspected separately, never treated as executable handlers. Failed-attempt
output directories are not claimed as completed evidence; use the aligned-source
directory above. No retail code is executed or committed.

Validation on Linux: fmt, workspace/all-target Clippy with warnings denied,
**362 Rust tests**, **26 Python tests**, locked workspace build, repository/app/
extractor asset guards, whitespace and changed-document local file-target checks
pass. Link checks do not validate anchors. Both-aircraft native live replay passes
**28 cases / 33,600 updates** with these extracted tables and the existing
validated F18.PT/RAFALE.PT. Creator smoke presents on RTX 4070 / Vulkan / Immediate.
Logs are ignored at `.local/native-environment/strip-commands-{tests,replay,gpu}.log`.

Replay/GPU checks cover the unchanged airborne/creator paths. No new runway,
viewer/cockpit, handling or performance acceptance is claimed. Physical input/
audio were not manually tested; Windows/macOS build/runtime and retail comparison
remain unavailable/not run. No AI, carrier activation, default change or push.

Next: complete selected intermediate movement/type-field consumers, then queue
reset/routing and observer/interceptor/damage closure, retaining E016/E004 gates
before staged E001/E002. PM control remains ignored and outside commits.
