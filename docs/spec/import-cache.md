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
Anthology import is about 5,300 resources and 224 MB (4,150 and 180 MB before the
ground target data). The import refuses to
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

`TORE_SURFACE_V1` (value `SURF1`, slice IM1, 2026-10-10) marks a pack that keeps
the Quick Mission ground target data, so a ground target can resolve from the
pack alone ([what is kept](#ground-target-data-slice-im1)). Unlike the
multiplayer marker, `check_markers` asks for it: the game, the dedicated
server and the headless bot all build the surface from these records, so a pack
an older import made is refused with "cache predates the ground target data;
re-import media". The game re-imports from the remembered source on its own; a
dedicated server or bot is told to run `tore-server --import`.

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

## Ground target data (slice IM1)

Implementation mode, 2026-10-10. Agent decision (the original had no importer):
the import keeps everything the surface round needs to resolve a Quick Mission
ground target, so no consumer reads the retail media at run time. The list is
worked out from the data, not typed in, by `tore_formats::surface_set::select`
(the same code serves the importer and the completeness check), starting from:

- all 129 templates (`~Q*.M`): the 124 the creator offers and the 5 it does not,
  from the executable's table in `quick_template::tables` and from the archive;
- every record type a template names, every unit in the executable's equipment
  lists (`tables::LISTS`) and the night rule's guns, and the object type of
  every placement in the base theater layouts;
- every `.NT` surface unit record in the library (84), named or not, so the
  pilots, the deck crew and the carriers the lists reach only by placeholder are
  there; and `DEST.OT`, the wreck a destroyed vehicle leaves.

From each record the walk follows its shape, shadow shape, damaged shape, weapon
and sensor records (and their shapes and sounds), the AI script and sound the
record names, the pictures a shape names, and the sub-shapes a shape names. Four
naming rules, each seen in the retail library and applied only when the name
exists, add the rest: a ship or object shape `S` keeps its damaged look
`S_A.SH`; an object `X.OT` keeps `~X.OT` (the damaged bunkers `~BNK5`, `~BNK6`,
`~BNK8`); a carrier (callback `_CARRIERProc`: Nimitz, Kitty Hawk, Clemenceau,
Wasp) keeps its tower `<stem>T.SH` with `~<stem>T.OT` and its far shape
`X<stem>.SH`; and a parked aircraft keeps its PT record, main shape and look
variants `S_S`, `S_A` to `S_D` with their pictures. The PT file is kept whole
(it is one data module; nothing reads its flight block): the importer keeps no
cockpit, HUD, sound or weapon record for an aircraft that is only a target.
Aircraft the creator offers keep all of those through their own selection.

The import report says "Surface data: 129 templates, N units, ..." and lists any
required name the library lacked ("Surface data unavailable: NAME (needed by
WHO)"); a gap does not stop an import and is repeated in the locate screen's
summary. [`tore_import::surface`](../../crates/tore-import/src/surface.rs) is
the lookup on a loaded pack: `present`, `template`, `templates`, `missing` and
`check` (the completeness check, for tests and tools). Records are stored under
their retail names, so loaders read them like any other resource. Measured size
and the check on both media: [import cache baseline](../baselines/import-cache.md).
