# Static-object event-service source checkpoint

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


2026-09-15, following `1a0f95d`. **NE-00.1k completes a bounded source ledger**,
not E019/E020/E021 or live contact. [Contract](../formats/native-strip.md#static-object-service-and-consuming-event-lookup-ne-001k),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-events-source
```

Initial and repeated extraction pass with the same reviewed EXE/SMS hashes:
**168 reviewed regions**, 107 symbol spans and 3829 symbols. Nine new aligned
regions cover the service snapshot, movement prefix, event-service caller,
mask/interceptor dispatch, consuming queue lookup and three bounded enqueue
slices. The complete enqueue/command/callback implementations are not accepted.
Original code is inspected as inert data, never executed or committed.

The source establishes that lookup can remove records and notify speech even
when no event is returned. Enqueue consumes shared bound-100 RNG before recipient
filtering and capacity checks; local delivery can change the scheduler and
current-object stores. These findings add E021 to the transaction dependency
ledger. The kind-0 collision query preserves the source F2 radius, masks and
snapshot/current endpoints; its producer remains unconnected. No test or
source argument promotes stationary objects to effect-free service calls.

Validation on Linux:

- Formatting, warnings-denied workspace/all-target Clippy, **361 Rust tests**,
  **24 Python tests** and locked workspace build pass. There is no new Rust
  behavior in this source-only change, so no new simulation acceptance is claimed.
- Repository/app/extractor asset guards, whitespace and changed-document local
  file-target checks pass. Anchor validity is not covered by the file-target check.
- Native live replay passes **28 cases / 33,600 updates**, using the new extracted
  sine/atan tables and existing validated F18.PT / RAFALE.PT inputs. This checks
  the unchanged airborne implementation, not execution of the recovered services.
- Creator `--quick-mission --smoke-test` presents on NVIDIA RTX 4070 / Vulkan /
  Immediate. No rendering changes or new runway, viewer, cockpit, handling or
  performance acceptance is claimed.

Logs remain ignored at `.local/native-environment/strip-events-{tests,replay,gpu}.log`.
Physical input/audio were not manually exercised; Windows/macOS build/runtime
and retail comparison remain unavailable/not run. PM control remains ignored.

Next: selected movement command initialization/body and E021 reset/routing/
observer/interceptor ownership; retain E016/E004 closure before staged E001/E002.
No AI, default-mode change, carrier activation or push.
