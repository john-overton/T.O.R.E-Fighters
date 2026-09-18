# Imported resource cache retention

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. **Opinionated** host behavior: John requested automatic
removal of previous imports on 2026-09-18. These packs are imported resource
snapshots, not executable builds or saved games.

After an import is completely written, synced, and successfully read back by the
normal pack decoder, delete regular `menu-<generation>.pack` files with an older
numeric generation in that same application-data directory. Normal startup also
performs this cleanup after successfully loading a pack, so existing accumulated
imports are cleaned without requiring another import. Startup cleanup is an
agent implementation choice supporting the requested behavior.

Normally one valid import remains. A failed write or decode does not authorize
cleanup. Startup can still fall back past an invalid newest pack. Keep the selected
valid pack and any files with newer generation numbers, including a possible
concurrent import. Do not recurse, follow symlinks, or delete unrelated files,
non-numeric pack names, settings, recordings, source media, or extracted assets.

Cleanup errors are reported to the terminal and do not prevent use of the loaded
assets. Remaining older packs can be retried on a later successful startup or
import. Cache location follows the existing platform default or TORE_DATA_DIR.
See [development setup](../DEVELOPMENT.md) for paths.

Validation: [import cache cleanup](../baselines/import-cache.md).
