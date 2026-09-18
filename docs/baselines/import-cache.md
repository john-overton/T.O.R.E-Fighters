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
