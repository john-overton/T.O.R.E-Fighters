# STRIP clock and scheduler ownership checkpoint

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `a17de47`. **NE-00.1p completes bounded source recovery
and diagnostic clock arithmetic**, not a native scheduler or contact producer.
[Contract](../formats/native-strip.md#clock-and-scheduler-ownership--ne-001p),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-clock-final
cargo test --locked -p tore-formats clock_rng
```

Five new independently aligned regions cover the complete scheduler caller,
secondary merge, frame baseline setter, scale setter and peer-pause predicate.
The existing service-age, frame-clock and counter-clock regions are reused,
not duplicated. Fresh and repeated extraction pass with **213 reviewed regions**,
107 selected symbol spans, 3829 symbols and unchanged reviewed EXE/SMS hashes.
The current-object tail ends before the inert jump table at 0x462960.

Two new synthetic tests exercise existing diagnostic helpers: elapsed signed
word wrap/minimum; frame pause, raw-before-clamp scaling, x86 five-bit counts and
signed ratio narrowing before clamping. The frame helper's former +/-15 scale
restriction is removed using reviewed source semantics; its Result API remains
compatible. No wall clock, scheduler, actor, output or RNG state is introduced.

Linux checks pass: **366 Rust tests**, **26 Python tests**, fmt, workspace/all-target
Clippy with warnings denied, locked build, repo/app/extractor asset guards,
whitespace and changed-document local file-target checks. File-target checks do
not establish anchor validity. Both-aircraft native live replay passes **28 cases /
33,600 updates** with F18.PT = F/A-18D and RAFALE.PT = Rafale C. Fresh creator
smoke presents on RTX 4070 / Vulkan / Immediate. Logs:
`.local/native-environment/strip-clock-{tests,replay,gpu}.log`.

Replay and GPU checks cover unchanged airborne/creator behavior. No new runway,
handling, viewer/cockpit or performance acceptance; physical input/audio are not
manually tested. Windows/macOS build/runtime and retail comparison remain
unavailable/not run. No AI, carrier/default activation or push.

Next: trailing special services 0x462c91/0x462d40, notification consumers and
later dead-object service, followed by remaining comment/output/world ownership
before staged E001/E002. Parent phases remain researching.
