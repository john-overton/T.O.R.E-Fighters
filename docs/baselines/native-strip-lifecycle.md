# STRIP candidate lifetime and source completion checkpoint

> **Measured evidence — research mode.** A record of what was run and what it
> produced, kept as evidence. Provenance labels and any remaining gates named
> here are research-mode scope; they are not acceptance gates for gameplay.
> Parity is measured by expression of feature — see [AGENTS.md](../../AGENTS.md).
> Player-visible behaviour is specified in [docs/spec/](../spec/).


2026-09-15, following `ad92851`. **NE-00.1e complete only as a diagnostic
candidate-lifetime/source precursor.** Parent NE-00.1 / NE-01.1 / NE-03.1 remain
researching. No new runtime contact branch; retail comparison unavailable.
[Contract](../formats/native-strip.md), [frozen plan](../research/native-environment-systems-plan.md).

## Reproduction and source evidence

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/strip-lifecycle-source
cargo test --locked -p tore-sim native_objects
```

The static pass emits **123 reviewed regions**, 107 selected symbol spans and
3829 symbols under the unchanged reviewed FA EXE/SMS hashes. Twelve new bounded
slices cover selected type setup/name resolution, final creation/store, last
allocation release, airport attachment gate/reset/delete and candidate removal.
Directly called or adjacent unaccepted branches remain explicit research edges;
disassembly is never executed. Repeating extraction into the same directory passes
without changed output conflicts.

`tables/strip-template.bin`: VA **0x50ccc8**, **308 bytes**, SHA-256
`091caae2916e2e32a1abc0d43be7979431df229c92c01b9ac7271894609c735e`.
This is a file-backed inert record extracted only for the reviewed pair. It has
nonzero defaults and five initial callback pointers. No unknown field has been
assigned gameplay meaning and no callback is activated. Data and source artifacts
remain ignored; no retail fixture or generated derivative is committed.

The source separates normal candidate removal from failed creation. After a
nonzero low-word callback result, the bounded failed-add path stores the object
and releases the last allocation, with no candidate-list or owned-name cleanup
there. The remaining error handler is unaccepted. Host atomic construction must
therefore stage all affected state; these list helpers alone do not supply it.

## Validation

Three new synthetic tests cover registration flag gates, duplicate suppression,
stable removal/reinsertion, independent 900/450 capacities, lack of duplicate
backfill, and type-flag changes that leave secondary entries until explicitly
removed. These establish the reviewed list operations, not world rollback.

Linux formatting, warnings-denied workspace/all-target Clippy, **351 Rust tests**,
locked workspace build, **24 Python tests**, repository/app/extractor asset guards
and whitespace checks pass. Logs: `.local/native-environment/strip-lifecycle-checks/`.

Existing native airborne replay passes **28 cases / 33600 updates**, using both
reviewed PTs and `strip-lifecycle-source/tables` with the
[foundation command](native-land-foundation.md). This is compatibility evidence;
the new diagnostic list state is not connected to either aircraft.

Creator, viewer and both native cockpit startup smokes pass on NVIDIA RTX 4070 /
Vulkan / Immediate using the foundation commands with the new table directory.
No rendering changed; these checks do not establish runway visuals, ground
handling or performance. The evdev unreadable-device warning persists. Physical
input/audio, Windows/macOS build/runtime and retail comparison are unavailable
or not run. Local file targets in the changed Markdown set were checked (227);
this is not exhaustive heading-anchor validation.

## Remaining work

Finish type-load/BRF closure, template field consumers/ownership, required
scheduling (E015) and remaining placement fields. Resolve complete E004
drawing/LOD/palette/visual dependencies. Then implement staged E001/E002 world
queries and late-failure rollback before any live contact connection. Unknown
events, carrier activation and unsupported modes remain gated. No AI or push.
