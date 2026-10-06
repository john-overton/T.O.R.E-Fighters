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

## Pack bounds

Agent implementation choice, 2026-09-28: a pack holds at most 32,768
resources, 2 MiB each, with names of 1 to 32 bytes, and 1 GiB in all. These
are sanity bounds that let a corrupt file be refused; a full Fighters
Anthology import is about 4,150 resources and 180 MB. The import refuses to
write a pack the reader would reject, and the reader streams the file rather
than holding a second copy of it in memory. The format is unchanged, so packs
written before the bounds were raised still load.

## Markers

Implementation mode. The import writes small `TORE_*` resources last, each
naming a revision of one kind of derived data (`TORE_MUSIC_V1`,
`TORE_COMBAT_V1`, `TORE_AIRPORTS_V1`, `TORE_SPEECH_V1`). A pack that lacks one
predates it and must be re-imported, and the error says "re-import media". The
shared check is `tore_import::check_markers`; the game, the dedicated server and
the headless bot all call it.

`TORE_MULTIPLAYER_V1` (value `ART1`, EF1, 2026-10-01) marks a pack that keeps
the multiplayer screens' art, dialogs, menus and `CHAT.TXT`
([what is kept](../formats/menu.md#what-the-import-keeps-for-these-screens-ef1-2026-10-01)).
Only the game asks for it, through `tore_import::check_multiplayer_marker`
in its pack check (*agent decision*): the dedicated server draws no art, so it
keeps running on a pack an older import made, and `check_markers` does not
require it. A player's existing import therefore asks to be redone once, and the
server's does not.

`TORE_SOURCE_V1` (slice L1, 2026-10-05) is not a marker: it records where the
import came from, three short text lines (`tore-source 1`, `build 1.02F`,
`tore <version> <commit>`). The build is the one `FA.EXE` identified as
(`1.0 (disc)` or `1.02F`); the other line is the T.O.R.E that made the import.
Nothing in single player reads it, and no pack is refused for lacking it: a
pack made before it reads its build from the `FA.EXE:` line of
`import-report.txt` beside the pack and its importer as unknown
(`tore_import::source::Source::read`). The multiplayer lobby uses it to show
each player's build and to name an import made by an earlier T.O.R.E
([compatibility](../ARCHITECTURE.md#compatibility)).

Validation: [import cache cleanup](../baselines/import-cache.md).
