# Airport slot and attachment source checkpoint

> **T.O.R.E — we trace what the player does, not what the code did.**
> This project reverse-engineers *player interaction*: what you press, see, hear
> and feel in Fighters Anthology, and the numbers behind it. It does not
> reproduce the original program byte by byte. Anything here about the original
> executable is evidence toward a behaviour spec — never a specification for what
> we build. If a sentence below reads like an instruction to reproduce the
> original's internals, it is out of date.
> <!-- tore-header v1 -->

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `a9abebb`. **NE-00.1i completes a bounded source ledger**;
E016/E019/E020 remain open. [Contract](../formats/native-strip.md#airport-slot-and-attachment-producers--ne-001i),
[frozen plan](../research/native-environment-systems-plan.md).

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-slots-source
python3 -m unittest discover -s tools -p 'test_*.py'
```

The unchanged reviewed EXE/SMS hashes gate **154 reviewed regions**, 107 selected
symbol spans and 3829 symbols. Eight additional aligned slices cover the plane
selector, two airport callback entry gates, three slot operations, state entry
and the attachment refresh tail. Selector-table dwords were independently read
from the hash-reviewed executable as data; the reviewed selector code stops
before the table. The existing inert STRIP template supplies slot count 9.
Repeat extraction passes. All generated/source data remains ignored.

The important new failure case is a destination with no free slot: native reserve
first clears all of the current ID's old slots, then can return -1. A duplicate
in the destination bypasses that cleanup. These facts constrain future staged
world updates; no rollback or complete allocator implementation is claimed here.
Plane registration is separate from STRIP. State entry has notification, slot
and device effects; it cannot be replaced with a byte assignment. Full takeoff/
landing bodies and full aircraft field updates remain unaccepted. No AI or
callback execution is added.

Linux fmt, warnings-denied workspace/all-target Clippy, **360 Rust tests**, locked
workspace build and **24 Python tests** pass. Repository/app/extractor asset guards
and whitespace checks pass. Workspace results are retained locally at
`.local/native-environment/strip-slots-tests.log`. Fresh both-aircraft native replay passes 28 cases / 33,600 updates using the
new extraction directory, and creator smoke presents on RTX 4070 / Vulkan /
Immediate. Logs are `strip-slots-replay.log` and `strip-slots-gpu.log` in the same
local directory. These check the unchanged runtime, not execution of the newly
documented routines. No new runway rendering, handling or performance acceptance. Physical input/audio, Windows/macOS runtime
and retail comparison remain unavailable or not run.

Next: E019 bounded static-object service branch and E020 speech/current-object
ownership, then remaining required E016/default and E004 drawing closure before
staged E001/E002. Live contact, carrier support and all unreviewed callbacks stay
gated. No push.
