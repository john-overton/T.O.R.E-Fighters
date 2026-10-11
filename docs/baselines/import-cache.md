# Import cache cleanup validation

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode, 2026-09-18. Contract: [cache retention](../spec/import-cache.md).

- Three synthetic tests cover numeric generation ordering, preserving the active
  and newer packs, ignoring unrelated names/directories/symlinks, and retaining
  all prior packs when no cache loads successfully.
- An isolated cache seeded with two copies of a valid local import retained only
  the newer pack after `--import-only`. An unrelated settings probe was unchanged.
- A fresh import from the user's local media into that isolated cache retained
  only the newly written pack, again preserving the unrelated file. Validation
  used the normal on-disk decoder, without opening a window or audio device.
- Loading the user's application-data cache removed 20 older packs, retaining
  one valid pack and releasing 2,675,367,756 bytes (about 2.49 GiB). The local
  record is `.local/cache-cleanup-result.json`; import output is
  `.local/cache-cleanup-import.log`. Temporary copied test packs were removed.
- Formatting, Clippy with warnings denied, locked workspace tests/build, Python
  tests, source/binary asset guards and documentation headers passed.

Validated on Linux. Windows/macOS execution was not run. The import writer is
closed before read-back/cleanup to accommodate Windows file-handle behavior.
No renderer changes were made; GPU smoke testing was not required.

## Ground target data (slice IM1, 2026-10-10)

Implementation mode. Contract: [what is kept](../spec/import-cache.md#ground-target-data-slice-im1).
Measured on Linux with a debug build, importing into fresh folders.

| Source | Resources | Pack bytes | Surface selection |
| --- | --- | --- | --- |
| 1.02F install (`gameassets/fighters-anthology`) | 5,281 | 223,709,004 | 1,518 resources, nothing missing |
| 1.0 disc (`disc1`, SETUP.ESA) | 5,281 | 223,709,009 | the same 1,518, nothing missing |

- The two packs differ by 5 bytes (the source entry's build text). Every one of
  the 1,518 surface resources is byte for byte identical between the builds,
  and no name is in both archives with different bytes.
- The data adds 824 resources and 27.45 MB of resource bytes to the import
  (about +14 percent): 299 pictures (22.7 MB, the carriers' and damaged ships'
  textures are the largest), 284 shapes (3.4 MB), 129 templates (0.8 MB), 50
  surface unit and 27 object records, 28 aircraft records (0.35 MB), 6 sounds
  and a script. An import before it measured 196.9 MB (`mpb-data-95d402a0`).
- The selection is 129 templates, 84 unit records, 138 object types and 40
  aircraft types (12 the game flies, 28 kept as parked targets only).
- Each import takes about one minute in a debug build (51 s from the install,
  64 s from the disc), the same as before.
- `cargo test -p tore-import --test surface_data -- --ignored` runs both
  imports and checks that all 129 templates parse and that every type a
  template, an equipment list (every placeholder, all five groups) or a layout
  can name, every shape, damaged shape, weapon and sensor record they name and
  every texture the shape reader finds are in the pack.

