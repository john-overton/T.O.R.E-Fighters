# STRIP service and RNG dependency checkpoint

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


2026-09-15, following `031c31c`. **NE-00.1g is complete only as a dispatcher
source ledger and diagnostic kind-0 delay selector.** Parent land/world/contact
and scheduler acceptance remain open. [Contract](../formats/native-strip.md),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-service-source
cargo test --locked -p tore-sim native_objects
```

The same reviewed EXE/SMS identities produce **135 reviewed regions**, 107 selected
symbol spans and 3829 symbols. Three new aligned ranges cover service dispatch,
its priority helper and the 16-bit-bound RNG wrapper. Direct edges are retained
in the ignored manifest; no imported code is executed. Scheduler draws share
the already reviewed native generator's seed/output/shuffle table. Seed producers,
service callback effects and global interleaving remain unresolved.

Two new synthetic tests cover callback override before default gates, zero delay,
clock wrapping, unsigned deadline equality/order across 0x7fff/0x8000, each
independent no-draw predicate, controller-bit isolation and signed speed extrema.
The helper returns a draw request rather than consuming RNG; no state mutation,
world lookup, callback execution or native scheduler replay is claimed.

Validation performed on Linux:

- Formatting, warnings-denied workspace/all-target Clippy, **356 Rust tests** and
  locked workspace build pass.
- **24 Python tests**, repository/app/extractor asset guards and whitespace checks
  pass. PM control state is ignored locally and excluded from this commit.
- Existing native live replay passes **28 cases / 33600 updates** for F/A-18D
  and Rafale C using `strip-service-source/tables` and the
  [foundation replay command](native-land-foundation.md).
- `target/debug/tore-app --quick-mission --smoke-test` presents successfully on
  NVIDIA RTX 4070 / Vulkan / Immediate. No rendering changes; runway visuals,
  ground handling, performance and new flight/camera acceptance are not claimed.

Devices were enumerated but physical controls/audio were not manually tested.
Windows/macOS build/runtime and retail comparison remain unavailable/not run.
Original media and generated source artifacts remain ignored/local.

Next: resolve selected post-create optional-field predicates and E019 service
bodies/global producers, required template ownership and E004 drawing closure.
Then implement staged E001/E002 query/cache/RNG with late-failure rollback before
narrow live contact. No AI, default-mode change, carrier activation or push.
