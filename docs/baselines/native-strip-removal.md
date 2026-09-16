# STRIP removal and notification checkpoint

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


2026-09-15, following `9da133d`. **NE-00.1r completes bounded source recovery
only**, not complete object cleanup, allocation release or runtime deletion.
[Contract](../formats/native-strip.md#removal-caller-and-notification-exclusions-ne-001r),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-removal-source
```

Five additional aligned regions recover the removal caller, death/removal
notification wrappers, future-recipient event invalidation and retained-record
expiry marker. Fresh/repeated extraction passes with **223 reviewed regions**,
107 selected symbol spans, 3829 symbols and unchanged reviewed EXE/SMS hashes.
Remaining cleanup callees are explicitly unresolved; no native code is executed.

The ledger preserves observer effects during the due-event drain, later
invalidation without compaction, expiry rather than freeing, and source-backed
single-count notification exclusions. It does not add synthetic behavior or
infer complete native rollback from deletion. No simulation helper changes.

Fresh Linux validation passes: **366 Rust tests**, **26 Python tests**, fmt,
workspace/all-target Clippy with warnings denied, locked build, repo/app/extractor
asset guards, whitespace and changed-document local file-target checks (not
anchors). Creator smoke presents on RTX 4070 / Vulkan / Immediate. Logs:
`.local/native-environment/strip-removal-{tests,gpu}.log`.

The `5fd23d7` 28-case / 33,600-update both-aircraft native live replay remains
evidence for the unchanged runtime; no new replay was required for this
source-only change. No new runway, damage, removal, handling, viewer/cockpit or
performance acceptance. Physical input/audio not manually tested;
Windows/macOS build/runtime and retail comparison unavailable/not run.
No AI, carrier/default activation or push.

Next: unresolved removal callees, starting at 0x442da0/0x4c3ca0 and selected
kind-0 exclusions, then required effect/resource lifetime and remaining
comment/world closure before staged E001/E002. Parent gates remain open.
