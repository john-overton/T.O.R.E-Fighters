# Extracting local game archives

`tools/extract_assets.py` is the shared, cross-platform extraction entry point. It runs the repository's Rust `tore-extract` tool and adds SHA-256 provenance to its report. It requires Python 3.10+ and the pinned Rust toolchain. There is no dependency on USNF-ATF, Bun/Node, graphics drivers, an audio device, or third-party Python packages.

## Ground and sea/ocean pass

NE-01/02 in the [living environment/systems plan](native-environment-systems-plan.md)
define the next full discovery/catalog/import pass, including conditional/dynamic
references, missing-asset reason chains and separate visual/collision acceptance.
Implementation research has started: the selected Ukraine/STRIP discovery
extracts five resources across two filtered runs with provenance, including
_RUNWAY.PIC. STRIP callback/box metadata is partly recovered; full initialization,
shape and contact closure remain unresolved. This is not a full census or new OT/runtime support.
[First evidence](baselines/native-land-foundation.md),
[STRIP continuation](baselines/native-strip.md).

## Two different workflows

| Task | Command | Output |
| --- | --- | --- |
| Run the menu/viewer | `cargo run --locked -p tore-app -- --import gameassets/fighters-anthology` | Selective menu/theater cache in application data; launches the app |
| Explore/extract any supported archives | `python3 tools/extract_assets.py` | All resources under ignored `.local/extracted/` |
| Inspect archive metadata only | `python3 tools/explore_assets.py` | Inventory under ignored `.local/exploration/` |

The app does not need a full extraction. It reads the original archives directly through the same format library and imports its menu and all defined theater profiles. Full extraction is for research and future format development.

## Common commands

From the repository root on macOS or Linux:

```sh
# Preview all matches without writing extracted output.
python3 tools/extract_assets.py --dry-run

# Extract the default local Fighters Anthology installation.
python3 tools/extract_assets.py

# Point at another installation/disc folder; archives are discovered recursively.
python3 tools/extract_assets.py --source /path/to/local/media --out .local/extracted-other

# Extract a single archive regardless of its filename or title prefix.
python3 tools/extract_assets.py --source /path/to/USNF_1.LIB --out .local/usnf

# List or extract a subset. Quote globs so the shell does not expand them.
python3 tools/extract_assets.py --include "CHOOSE*.PIC" --list
python3 tools/extract_assets.py --include "*.PT" --include "*.SH" --out .local/aircraft-research
```

On Windows, use PowerShell with `python` or the Python launcher:

```powershell
py -3 tools/extract_assets.py --source "D:\Fighters Anthology" --out ".local\extracted"
```

Explicit source/output paths resolve relative to your invoking directory. Defaults resolve relative to the repository, so the script can also be invoked by absolute path from another working directory. Spaces and punctuation are passed as structured subprocess arguments, not shell commands.

The wrapper builds the small native extractor in release mode; first use may download locked Rust dependencies/toolchain components. It prefers rustup's Cargo when present. Normal repository development continues to use debug builds of the app.

## Output and repeat runs

Example output:

```text
.local/extracted/
  FA_1.LIB/
    CHOOSEV.PIC
    FONTACT.PIC
    ...
  FA_2.LIB/
    CHOOSEAC.DLG
    F14.PT
    ...
  swpatch.lib/
    F14.SH
    ...
  extraction-report.json
```

Subdirectories below the source root are retained. Archive boundaries are retained as directories, so base and patch resources cannot silently overwrite one another. No patch precedence is applied.

Names within one archive are case-insensitive. Duplicate directory names use the last occurrence, matching the shared reader; the report records that selected entry's offset. Conflicting output paths from different archives are rejected. Use the inventory tool when researching duplicate directory records themselves.

Existing byte-identical files are reused. Differing existing files are left untouched and recorded as errors; add `--overwrite` only when deliberately refreshing them. New data is written to a temporary file before replacement. Recursive output symlinks, traversal paths, unsafe cross-platform names, and case-insensitive output collisions are rejected. Keep the output directory outside the source media tree.

`extraction-report.json` describes the **latest invocation/filter selection**, not a cumulative catalog. It contains source paths, resource names, offsets, stored/decoded byte counts, status (`written`, `replaced`, `unchanged`, `error`), errors, archive hashes, and hashes of successful outputs. Errors produce a nonzero exit code and `complete: false`; successful resources remain available for inspection. A no-match filter is also an error. `--list` and `--dry-run` do not create extraction output or a report and do not decompress payloads.

Default decoded-resource cap: 256 MiB. Raise deliberately with `--max-entry-mib 512` (allowed range 1–1024) for large recordings. Archive directories are read separately and resources are read by file offset; the entire archive is not loaded into memory. Compressed and decoded buffers for the current entry still use memory. The app's selective menu reader retains its smaller 16 MiB per-resource cap.

## What “universal” covers

The extractor is format-based, not tied to `FA_1.LIB` or another title's filenames. It discovers files with an EALIB signature, validates directories/sentinels, and handles stored entries plus raw-literal PKWare DCL compression. This covers the archive structure used by the supplied Fighters Anthology installation and the USNF/ATF format references. Actual other-title media has not been tested in this session.

It unpacks **all resource types** as their original decompressed bytes. It does not claim to decode every resource: a `.SH` remains a shape resource, `.PIC` remains an indexed game image, `.FNT` remains a compiled resource, and `.11K` remains PCM. Nothing extracted is executed. The app interprets its menu subset and the initial T2/mission/weather data subset described in [theater recovery](formats/theater.md).

ISO images, ESA installer containers, coded-literal DCL mode 1, missing/truncated media repair, general PNG/WAV/model conversion, and cross-title gameplay import are not implemented by this command. The explicit `--music --wav-previews` option described below supports lossless music PCM WAV wrapping. Supply loose archives from your own installed or extracted media. Unknown/non-EALIB `.LIB` files are reported as errors rather than silently accepted.

## Native command and tests

The native CLI is also available without Python. It writes the extraction report with names/offsets/statuses, but SHA-256 enrichment is supplied by the Python wrapper:

```sh
cargo run --release --locked -p tore-extract -- --source gameassets/fighters-anthology --out .local/extracted
cargo test --locked -p tore-extract
```

Synthetic integration tests cover stored and compressed data, signature discovery with arbitrary filenames, filtered dry runs, no-match errors, repeat extraction, conflicting output protection, explicit overwrite, traversal/reserved-name rejection, size caps, and output symlink containment on Unix. No retail fixtures are committed.

The first full local run extracted **7,520 resources / 301,951,459 decoded bytes** from five archives with zero errors. All five source hashes remained unchanged. Of these, 7,372 entries were DCL-compressed and 148 stored. The detailed report remains in ignored `.local/extracted/extraction-report.json`.

A repeat full run reused all 7,520 outputs as `unchanged`, with zero errors. Running the wrapper from `/tmp` against one explicit archive also passed. Linux and Windows are configured in CI; local runtime validation was performed on the M3 Mac.

All extracted media, caches, snapshots, and derivative assets stay local. Commit parser code, synthetic tests, and research notes only.

## Ukraine and shared environment profile

```sh
python3 tools/extract_assets.py --theater UKR --out .local/ukraine-import
python3 tools/extract_assets.py --theater UKR --list
```

`--theater UKR` selects the Ukraine profile; `--theater all` selects all 16 defined profiles. The general extractor still handles arbitrary resource names and supported archives. Additional `--include` filters intersect the profile; unknown theater codes are rejected explicitly. The profile selects all T2 files for the selector catalog, UKR/~UKR resources and maps, every LAY module, standalone palette, ground/fallback textures, nine sky textures, sun/moon/stars/cloud shapes and their named textures, and Quick Mission artwork. It does not recursively resolve every SH, mission-object or campaign alias dependency.

The supplied installation yields **213 resources with zero errors**. Other theaters' height grids are available for inventory; their complete texture/map/object bundles are not imported into the viewer. For T2 entries, the report's `analysis` contains dimensions, sample scales and elevation range. Selected M/MM entries include top-level weather and texture placements; unknown fields remain in the raw extracted files. Other entries have null analysis. LAY and SH remain original bytes; the app separately decodes a bounded weather palette subset.

The app uses the same profile predicate at import time, reading FA_1/FA_2 directly. It does not consume `.local/ukraine-import` as a runtime directory. Refresh an external-media cache with `--import`; a missing theater resource invalidates an older menu-only cache and triggers local automatic import when default media exists. The app pack now permits 2,048 entries and 128 MiB total; it is still a development cache, not an interchange format.

See [theater recovery](formats/theater.md) for native addresses, sky/celestial dependencies, corrected T2 fields and remaining weather-engine work.

## All defined theaters and the retail discs

```sh
python3 tools/extract_assets.py --theater all --exclude-archive 'disc1/LHX/*' --out .local/all-theaters
# Individual profiles use the same rules (codes are case-insensitive):
python3 tools/extract_assets.py --theater TVIET --exclude-archive 'disc1/LHX/*' --out .local/vietnam
```

Defined codes live in Rust's shared `THEATERS` table: APA, BAL, CUB, EGY, FRA, GRE, IRA, KURILE, LFA, NSK, PGU, SPA, TVIET, UKR, VLA, WTA. Aliases include KURIL map resources and VIET maps/campaign names and TVI numbered textures. Each profile selects its named resource family, `~` variants, IFM maps and shared environment resources. All T2 grids remain included for catalog use, even with one profile. This is conservative filename-based dependency selection, not complete recursive object/SH resolution. The app now imports and renders all 16 defined base theaters.

Discovery recursively includes `disc1/` and `disc2/`. Directory-based theater scans print and skip non-EALIB files such as the bundled MPlayer `_SETUP.LIB`; corrupt/unsupported EALIB archives still fail. Explicit archive inputs and generic extraction remain strict. The bundled LHX demo has unsupported EALIB compression flags, so exclude that separate game explicitly with `--exclude-archive 'disc1/LHX/*'`. The option is repeatable, case-insensitive, matches source-relative archive paths using `/`, and also works for generic extraction. Adjust the path when using a different source root; exclusions are printed and excluded resources are absent from the report.

On the supplied installation plus both disc folders, the command above extracted **1,129 resources with zero errors**: 852 from FA_1 and 277 from FA_2, including 16 T2 grids and 75 MM layouts. No additional terrain-profile matches came from the added disc archives. They add reference pictures, video and audio for later work; keep them available. `disc1/SETUP.ESA` is also present but is not decoded by this tool. Its presence alone does not establish package completeness.

Pakistan and Persian Gulf layouts contain `tmap` coordinates of -4 along grid borders. These are preserved as signed values in metadata, not rejected or converted to large unsigned positions. Rendering those border patches and native edge semantics remains future work.

The runtime integration pass found the TVI texture alias for TVIET. All-theater extraction now selects **1,171 resources** (894 FA_1, 277 FA_2), including the previously omitted 42 Vietnam textures. Earlier 1,129-resource counts describe the prior extraction checkpoint. App import uses this corrected shared profile. Rendering is a base-theater preview; recursive object dependencies and campaign-generated surfaces remain incomplete.

## F/A-18D and weapons

`--weapons` selects the complete JT/SEE/ECM/GAS catalog and reviewed shared
combat graphics/audio dependencies. Both reviewed aircraft can be exported in
one invocation with `--aircraft f18 --aircraft rafale --weapons`. The supplied
media yields 561 resources with zero errors; a repeat reuses all files.
[Scope, unresolved dependencies and validation](baselines/combat-components.md).

```sh
python3 tools/extract_assets.py --aircraft f18 --exclude-archive 'disc1/LHX/*' --out .local/f18-import
python3 tools/extract_assets.py --aircraft f18 --weapons --exclude-archive 'disc1/LHX/*' --out .local/f18-import
python3 tools/extract_assets.py --theater all --aircraft f18 --weapons --exclude-archive 'disc1/LHX/*' --out .local/flight-import
```

The aircraft profile automatically includes its default weapons, sensors, tank, shapes, textures, cockpit variants, instrument fonts/chrome and available audio dependencies. `--weapons` expands to all projectiles, sensors, ECM and tanks plus shared combat effects. Profiles combine as a union; optional `--include` globs filter that union. Native Rust readers and the dependency resolver are shared with app startup; no reference checkout, Bun or extra Python packages are needed. Dry-run/list performs dependency reads but writes nothing. Keep the report alongside the extracted files for source hashes and named fields, envelopes and hardpoint evidence.

This preserves/imports data; it does not establish full flight, radar, instrument or weapon behavior. See [aircraft format and runtime coverage](formats/aircraft.md). F/A-18C is a separate variant, not an alias for this F/A-18D profile. The app imports both reviewed aircraft and the armament catalog from FA_1/FA_2 directly into its versioned cache. It does not consume the CLI output directory. Importing the catalog alone does not enable combat: `--live-fire` selects the two-aircraft PT-default manual range. The existing cache supplies the typed ECM and damage fields used by the [systems pass](baselines/weapons-systems.md); extracted alternatives are not automatically playable.


The cockpit/control follow-up adds mandatory `HUD11.FNT` and `FMENUD.MNU` to `--aircraft f18`, and preserves all available HUD mode fonts. Re-run the same extraction command to extend an existing output; unchanged files remain untouched. The runtime cache detects the newly required font and can refresh itself from the local media. The recovered menu tree is interpreted as data; no native module is executed.

## Repeatable native flight research

`python3 tools/extract_assets.py --native-flight --source gameassets/fighters-anthology --out .local/native-flight/repro`
performs static PE/SMS inventory and disassembly, independently of archive extraction.
Add `--dry-run` to inspect metadata first. It requires LLVM `objdump`; it never runs
retail code. Hash-gated PT references, bounds, overwrite rules, limitations and the
Rust helper probe are documented in [native flight research](formats/native-flight.md).

The reviewed native-flight research mode also emits `reviewed-components.json` and explicit `reviewed/*.txt` slices for departure, contact and integration routines. These include direct outgoing branches/calls and partial instance offsets; they do not extract a runnable engine. Both reviewed EXE/SMS hashes are required. Use a new output directory for expanded research versions, then repeat the same command to check unchanged output. [Details](formats/native-flight.md#second-pass-departure-ground-and-integration-components).

Flight `reviewed/*.txt` regions now disassemble independently from their reviewed
start/end addresses. This prevents an embedded jump table in a global linear
sweep from misaligning the next code entry. Empty, wrong-entry, duplicate,
unordered or out-of-range decoded addresses fail validation. The global
`fa-disassembly.txt`, symbol spans and incoming-reference inventory remain a
linear exploratory view, not a complete code/data or indirect-call graph.
Use a fresh output directory after this extractor change; prior manifests have
a different method label. [STRIP command evidence](baselines/native-strip-commands.md).

The reviewed native-flight pass now also extracts `tables/sine-q15.bin` (321 signed little-endian words) and its source/table hashes. Probe it with `cargo run --locked -p tore-app -- --native-flight-trig PATH`. This is inert lookup data, not executable code; table extraction is unavailable for unreviewed builds. [Third-pass notes](formats/native-flight.md#third-pass-extracted-trigonometry-forces-and-loading).

Native-flight fourth pass also exports `tables/atan-pa.bin` (514 unsigned words)
alongside `sine-q15.bin`. Both are bounded data reads gated by the reviewed FA
executable hash, with provenance/hashes in `tables/inventory.json`. Reviewed
regions now include matrix/cockpit composition, contact predicates/latch, loaded
controls/equipment resolution, and RNG/frame/counter clocks. Reuse identical
outputs safely; choose a fresh output directory when extending the research
inventory, or explicitly request `--overwrite`. See the headless composition
example in [DEVELOPMENT.md](DEVELOPMENT.md).

Fifth-pass native output uses schema 2 in `reviewed-components.json`: each of
52 regions includes `entry_references` listing direct incoming calls/jumps.
This is a static reference index, not execution order or an indirect call graph.
For reproducibility, use a new directory such as `.local/native-flight/queries-final`.

## Rafale C profile

```sh
python3 tools/extract_assets.py --aircraft rafale --exclude-archive 'disc*/*' --out .local/rafale-import
```

This selects the reviewed loose-installation Rafale C and its available transitive
cockpit, shape, equipment, store and audio dependencies. Omit the archive exclusion
to scan disc archives too. `--aircraft f18` remains supported. The Rust CLI accepts
repeated `--aircraft` flags to form a union; the app imports both profiles through
the same resolver. `RAFALEF.PT` and `RAFALEE.PT` are not aliases. Extraction is
complete for the selected dependency closure, not native flight/animation/system
parity. See [profile coverage](formats/aircraft.md#rafale-c-import-and-runtime-selection--2026-09-14).

Reviewed aircraft selection now supports `--aircraft rafale` as well as `f18`.
Rafale C starts from RAFALE.PT, RAFALE.HUD, RAF.SH and ~RAFH.PIC and follows the
same bounded dependency closure. RAFALEE/RAFALEF are not aliases. Named PT
analysis is included in the extraction report. `--validate-flight` runs the
shared headless hybrid-model suite after successful full extraction; it does
not execute imported code or certify visual/native parity. Full examples:
[FLIGHT-MODEL.md](FLIGHT-MODEL.md).


## Recorded music profile

```sh
python3 tools/extract_assets.py --music --out .local/music
python3 tools/extract_assets.py --music --wav-previews --out .local/music
python3 tools/extract_assets.py --music --list
```

`--music` selects the shared FA music resource profile: recorded PCM and all nine
MUS scripts. It does not select MIDI or synthesize audio. The supplied installation
has 108 matching resources (99 recordings, nine scripts). Music combines as a union
with aircraft/theater profiles; `--include` subsequently narrows that union.
Directory scans skip non-EALIB installer files, while explicit archive inputs remain
strict. App and CLI share resource selection; the app still imports from source
archives into application data, not from the research output directory.

`--wav-previews` requires `--music`. It creates `NAME.11K.wav` beside each original
recording with byte-identical PCM samples. Both originals and previews retain the
normal safe-path, conflict and overwrite protections. `preview_output` and
`preview_sha256` in the extraction report identify each preview. PCM analysis
records rate, sample count and duration; MUS analysis records PCM references,
unreachable byte count and `missing_pcm` against the non-excluded source catalog.
A successful extraction is not a promise that every score reference exists or a
narrowed `--include` selection is playable. All WAVs and source resources remain
local and ignored. [Music evidence and runtime scope](formats/music.md).

## Static weapon research and component probes

```sh
python3 tools/extract_assets.py --native-weapons --out .local/native-weapons/repro
python3 tools/extract_assets.py --aircraft f18 --aircraft rafale --weapons --exclude-archive 'disc1/LHX/*' --out .local/combat-import
cargo run --locked -p tore-sim --example weapon_probe -- .local/combat-import/FA_2.LIB/M61.JT .local/combat-import/FA_2.LIB/DEFA.JT
```

Native weapon research is separate from archive selection and `--native-flight`.
It uses LLVM objdump without executing retail code. Fixed-address artifacts require
both reviewed EXE/SMS hashes; other builds receive inventory/disassembly only.
Choose a fresh output directory after expanding the reviewed regions.

Profile extraction reports contain `dependencies.edges` and `providers`: each
edge records its source, target, reason, availability and inclusion in this run;
providers record all candidate archives and the last archive used to read a
resource for discovery. Extraction still retains archive boundaries for all
matches. `complete` means the requested writes succeeded, **not** full combat
coverage. `dependencies.filtered` identifies a closure reduced by `--include`;
`native_parity` is always false. Unknown callback/art/module edges remain explicit.
PTS modules are inert and their absent icon candidates are unresolved; required
reviewed shape/texture/audio dependencies still fail with a source reason chain.

## Static creator and ordnance research

```sh
python3 tools/extract_assets.py --native-menus --out .local/menu-contract-pass
python3 tools/extract_assets.py --include 'QM_MENU.MNU' --include 'ARMPLANE.MNU' --include 'QUIKMISS.DLG' --include 'LOADORD.DLG' --exclude-archive 'disc1/LHX/*' --exclude-archive 'disc1/WB/*' --out .local/menu-resources
cargo run --locked -p tore-formats --example menu_tree -- .local/menu-resources/FA_2.LIB/QM_MENU.MNU .local/menu-resources/FA_2.LIB/ARMPLANE.MNU
```

`--native-menus` is exclusive with other native domains and archive profiles.
It reuses the bounded EXE/SMS research pass and emits named symbol spans, six
reviewed subregions, direct edges and candidate string references. Fixed-address
artifacts require both reviewed hashes. Candidate strings are not complete active
option tables; disassembly may include data interpreted as instructions, and
symbol boundaries may include unnamed routines. Imported code is never run.

`menu_tree` reads extracted MNU files through `tore-formats::ui::menu_tree`, the
same bounded grammar used by flight menus. It prints source hierarchy/shortcuts;
it does not evaluate native visibility, check-state or action callbacks. Keep
output and source resources ignored. [First-pass evidence](baselines/menu-contract-pass.md).

The menu pass also emits `creator-options.json`: all 60 selector dispatch entries,
16 theater-specific target lists and 29 briefing geometry rows. Dynamic aircraft
producers are identified rather than replaced with guessed lists. Three aligned
consumer disassemblies are emitted separately from the full linear disassembly.

Inspect extracted static dialog geometry with:

```sh
cargo run --locked -p tore-formats --example dialog_geometry -- .local/menu-resources/FA_2.LIB/QUIKMISS.DLG .local/menu-resources/FA_2.LIB/LOADORD.DLG
```

This resolves imported draw references as inert data. Printed coordinates are
static local parameters; native runtime placement/hit testing remains separate.
[Active options and geometry](formats/quick-mission.md).

## Creator metadata and ordnance UI

`--creator` adds all PT/JT metadata, weapon thumbnails and original creator/ordnance
UI resources through the same profile used by the app. Combine it with
`--aircraft f18 --aircraft rafale` for supported flight dependencies. Archive boundaries,
conflict checks and provenance are retained; other aircraft metadata does not enable flight.

The app additionally requires the reviewed FA.EXE during import to recover active
selector tables. It checks the complete fingerprint and reads bounded inert lists;
it never executes or caches the executable. The standalone equivalent is:

```sh
cargo run --locked -p tore-formats --example creator_options -- gameassets/fighters-anthology/FA.EXE .local/creator-options.bin
```

The output is create-new and must remain ignored. `--creator` itself extracts archive
resources; it does not implicitly read executable tables. Unknown executable builds
are rejected until independently reviewed. Old app caches re-import when media is available.

The app's reviewed-EXE import also preserves the inert cloud placement table as
`TORE_CLOUDS_V1`; its shared bounded reader lives in `tore-formats::weather::clouds`.
The standalone archive extractor includes original cloud SH/PIC and ocean PIC
resources through the same theater dependency predicate. It does not generate
app cache records from executables.

The app also imports the reviewed executable's nine lens-flare descriptors into
`TORE_FLARE_V1` using the bounded `weather::flare` reader. All LAY modules supply
fill remaps 265/266 as well as the existing sun-glow remap 267. Older caches
without this inert layout re-import from available reviewed media.

## Aircraft implementation guide

The [aircraft import and acceptance guide](aircraft-import.md) joins extraction,
existing flight/presentation/systems coverage and all per-aircraft acceptance
gates. F-14, A-4E and X-31 are scheduled after the flight-response slice; they
are not supported identities yet.

### Native geometry research data — 2026-09-15

The hash-gated `extract_native_flight.py` pass also emits `tables/sqrt-seed.bin`
(1024 little-endian unsigned dwords) and its hash/consumer in the table inventory.
It is diagnostic input for terrain normals, not a new requirement for the existing
airborne option. Use a fresh `--out` directory when earlier research manifests
differ. [Geometry command and evidence](baselines/native-land-geometry.md).

The same reviewed-build pass now exports `tables/strip-template.bin`, the bounded
0x134-byte static airport template, with source address and hash in the inventory.
It contains inert pointer words and uninterpreted defaults; it is diagnostic
data, not an executable callback table or a live-world input. No new requirement
is added to the airborne option. [Lifecycle evidence](baselines/native-strip-lifecycle.md).

Inspect the selected extracted definition and shape together with
`cargo run --locked -p tore-formats --example native_strip -- RUNWAY.SH STRIP.OT`.
The shared formats reader validates the reviewed static STRIP/166 metadata;
the diagnostic checks the shape basename against its explicit reference. It does
not extend app/CLI extraction profiles, resolve the full drawing program or place
an airport. [Definition-reader scope](baselines/native-strip-definition.md).

The optional third argument `ISOLATED-PLACEMENT` reads a single selected STRIP
`obj`/`.` record and prints native-width placement inputs. Preserve extraction
provenance when isolating a record locally; do not pass a whole MM file. Unknown
fields fail, and zero Y is not treated as a placed runway. No extraction profile
or runtime import changes. [Record-reader evidence](baselines/native-strip-record.md).
