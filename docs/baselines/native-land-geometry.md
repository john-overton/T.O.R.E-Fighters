# Native vertical land geometry checkpoint

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


2026-09-15. NE-00.1b, following clean precursor `24e3106`.
**Native arithmetic translated/tested; no runtime contact activation.**
[Contract](../formats/native-land-contact.md),
[frozen plan](../research/native-environment-systems-plan.md).

## Recovery and implementation

Eight aligned reviewed ranges extend the static pass to **92 regions**;
107 selected symbol spans and 3,829 symbols are unchanged. Both reviewed EXE/SMS
identities remain hash-gated. No native module is executed.

New external table: VA 0x51d624, 1024 unsigned little-endian dwords,
SHA-256 `74353d3f0d10822035dc05fb6d5ce45974a917876ca26138c9ffbf5d35567864`.
The static extractor writes `tables/sqrt-seed.bin` and its inventory entry.
No retail table bytes or generated retail fixtures are committed.

The `terrain_contact` diagnostic preserves source normal winding/scaling,
integer square-root seed/refinement, flat/sloped intersections, cell bounds,
diagonal ownership and vertical-cell selection. `shape::contact_offset` bounds
the F2 relative link and consumed signed offset field. It deliberately leaves
full collision records, placement and lifecycle decoding open.

Seven new synthetic Rust tests cover seed branches, normal winding/halving,
degenerate faults, exact-plane endpoints, type offsets, ratio precision loss,
cell seams/diagonal equality, flat versus split cells, nonvertical rejection and
malformed shape links. Existing Python table extraction tests now also cover
unsigned dword width and section bounds. Unit fixtures contain no retail data.

## Reproduction and original-asset diagnostic

```sh
python3 tools/extract_native_flight.py --source gameassets/fighters-anthology --out .local/native-environment/geometry-source
cargo run --locked -p tore-formats --example native_land_geometry -- .local/native-environment/geometry-source/tables .local/native-environment/land-discovery/FA_2.LIB/UKR.T2 .local/native-environment/land-discovery/FA_2.LIB/RUNWAY.SH
```

The T2/SH hashes are recorded in the [selected extraction baseline](native-land-foundation.md).
The probe samples four positions in each of 208×200 cells, including the outer
edge's native zero-elevation fallback. **166,400 cases run twice identically**;
53,587 have nonvertical normals; returned fixed8 Y spans 0–2,031,616
(0–7,936 feet). RUNWAY.SH resolves `Some(0)` from its own F2 record, not an absent
record. This validates decoder coverage and repeatability; it is not an independent
retail trajectory oracle or a runway placement/handling acceptance test.

The current probe accepts the table directory; the [angle follow-up](native-land-angles.md)
adds candidate/projection checks using its sine and atan files. The counts above
record the NE-00.1b geometry run.

## Validation and limitations

Linux checks pass: formatting, warnings-denied workspace/all-target Clippy,
**345 Rust tests**, locked build, **24 Python tests**, repository/app/extractor
asset guards and `git diff --check`. Local logs:
`.local/native-environment/geometry-checks/`.

The existing F18/Rafale airborne live probe again passes **28 cases / 33,600
updates**. Creator, viewer and both native-airborne cockpit smoke tests pass on
Linux / NVIDIA RTX 4070 / Vulkan / Immediate. These are single-frame startup
regressions; there is no new visual feature or active handling/performance claim.
The controller evdev warning remains; physical input/audio, Windows/macOS
build/runtime and retail comparisons are unavailable/not run.

The live stop, legacy default and hybrid path remain unchanged. E008 angle arithmetic is now closed by the [follow-up](native-land-angles.md).
Complete STRIP placement/init/callback/resource closure,
staged query/cache/RNG ownership and late-failure rollback still block the first
live land branch for both aircraft. General segment traversal, full collision
record parsing and carrier activation remain outside this completed precursor.
