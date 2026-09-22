# F/A-XX export and developer tools

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Implementation mode. The local export produces a separate original-format
F/A-XX definition, shape family and LIB from the retail **F-22N** donors, with
fin removal, split flap leaves and the F-22N's own hook. It contains no F22- or
F22N-named entries. It remains an **experimental original-game aircraft**. John
confirmed that the earlier F-22A-based package worked in original FA on
Windows, including the decal cleanup, on 2026-09-18; the F-22N-based package
(2026-09-22) has not been flown in original FA yet. Read the
[export contract](spec/fa-xx-export.md) for endpoint animation and other fitted
differences. [Validation and input identities](baselines/fa-xx-packaging.md).

## Reusable local toolset

All scripts live in `tools/` and use Python's standard library. Git, Rust 1.91.1
and the host C/C++ toolchain are needed to build the external OpenFA utility.
The first setup downloads source and dependencies; it is not an offline bundle.

```sh
python3 tools/openfa_tools.py setup
```

This creates `.local/tools/openfa/`, pins OpenFA and Nitrogen, applies
`tools/openfa/static-export.patch`, and builds the static feature. It records
binary, patch and lockfile hashes. The initial dependency resolution creates an
upstream Cargo.lock; subsequent builds use `--locked`. The source revisions are
pinned, but first-time dependency resolution is not claimed reproducible.
For the existing reviewed checkout use `setup --source .local/openfa-review`.
Its executable can be selected with `--tool` before a workflow subcommand.

The patch disables upstream's emulated-x86 shape analysis. An ordinary upstream
build is rejected by our wrappers. It also removes modal error dialogs and gives
conversion failures nonzero exit codes. The patched build is for export tooling,
not running OpenFA's interactive application.

Example commands with a newly built default tool:

```sh
python3 tools/openfa_tools.py decode --out .local/shape-edit /path/to/F22.SH
python3 tools/openfa_tools.py encode --out .local/shape-built .local/shape-edit/F22.SH.yaml
python3 tools/openfa_tools.py pack --out .local/candidate.LIB .local/shape-built/F22.SH
python3 tools/openfa_tools.py unpack --out .local/archive-check .local/candidate.LIB
```

All output paths must be new. Conversion copies inputs into scratch output
folders, so editing/recompiling never overwrites the donor. Packing uses the local `fa_lib.py` stored-entry writer with the required EALIB
sentinel, then creates the output exclusively. OpenFA's original packer is not
used because its own round trip missed an omitted sentinel. Repack only resources the
recipient intends to override; do not replace a whole stock archive with a
small candidate archive. Actual installation and library precedence are outside
these tools' verified scope.

## Use the tools for another asset export

1. Extract from the recipient's own media into `.local/`. Keep archive boundaries
   and donor hashes; the [extraction guide](EXTRACTION.md) covers filters.
2. For SH edits, run `check_shape_roundtrip.py` on an untouched donor before
   editing its YAML. Geometry, visibility branches, imported control bindings,
   relocations and detail levels are part of the file contract. An OBJ mesh is
   not a complete original-game aircraft.
3. For a separate aircraft, create a unique PT identity and matching shape family.
   Keep shared cockpit/equipment/texture references explicit. The
   [objects and shapes guide](formats/objects-and-shapes.md) and
   [F/A-XX export contract](spec/fa-xx-export.md) describe the reviewed naming
   relationships. Do not globally replace a donor prefix in every reference.
4. Build into a fresh output directory. Pack the intended resource names with
   `openfa_tools.py pack`, which uses the corrected local EALIB writer.
5. Independently validate the resulting directory, sentinel and every payload.
   For example, with two synthetic or user-owned files:

   ```sh
   cargo run --locked -p tore-extract --example check_lib -- .local/candidate.LIB .local/output/NEW.PT .local/output/NEW.SH
   ```

6. Validate decoded geometry and control states separately. The F/A-XX validator
   is specific to its donor and spec; it is not a general acceptance test for
   another aircraft. Retain indexed decal polygons during export inspection.
7. Record donor identity, output hashes, retained dependencies, fitted differences
   and tests. Keep generated retail derivatives in ignored local directories.
8. Test the new LIB in the recipient's original FA installation, with explicit
   install/remove instructions. A successful compiler or reader test alone does
   not establish correct original-game rendering and controls.

For new aircraft or changed definitions, choose unused resource names and check
for collisions in the target mod set. Ship only the intended additions or overrides,
not a replacement for a whole stock library. The current tooling creates files;
it never installs them into a user's game automatically.

## Export the concept

Extract your own stock F-22N and dependencies if needed:

```sh
python3 tools/extract_assets.py --source /path/to/fighters-anthology --aircraft f22n --out .local/faxx-donor
```

Locate the F22N.SH/F22N_A.SH/F22N_C.SH directory in that extraction. Then run:

```sh
python3 tools/export_faxx.py --tool .local/tools/openfa/target/debug/ofa-tools --donors /path/to/FA_2.LIB-directory --out .local/exports/faxx
```

On Windows use `python` and append `.exe` to the tool executable. Building/exporting is designed to use portable Python/Rust APIs and was tested
on Linux. Installing and flying the generated package was confirmed by John on
Windows; the Windows tool-building workflow itself has not been exercised.

The exporter creates FAXX.PT and six FAXX-named shapes. It needs F22N.PT and
F22N_B/D/S.SH alongside the three reviewed edited donors F22N.SH, F22N_A.SH and
F22N_C.SH. The exporter rejects unreviewed donor hashes and a donor PT without
the hook capability bit. It writes editable YAML, compiled SH files, reports,
FAXX.LIB and `F-A-XX-FA-experimental.zip`. The earlier `--identity f22`
replacement mode was removed on 2026-09-22. It automatically runs
`tools/validate_faxx_export.py` and checks the LIB with an independent Rust
reader and also compares OpenFA-unpacked payloads. The ZIP contains the new PT,
six SH files, equivalent LIB, reports and recipient notes. Stock donor
textures, cockpit and equipment remain shared with the recipient's
installation. No installation is modified automatically, and Kapset
integration remains unverified. The README distinguishes loose-file and LIB
alternatives.

Validation can be repeated independently:

```sh
python3 tools/validate_faxx_export.py --donors /path/to/FA_2.LIB-directory --export .local/exports/faxx
python3 tools/check_shape_roundtrip.py --tool .local/tools/openfa/target/debug/ofa-tools --out .local/roundtrip /path/to/F22N.SH
```

`crates/tore-extract/examples/shape_json.rs` supplies the bounded geometry
projection used by validation. `check_faxx_pt.rs` independently parses the new
PT and checks that only the intended identity/reference fields differ from the
donor and that the donor already carries the hook capability bit. Set `TORE_EXPORT_BRANCHES=1` for explicit SH jump
handling. The gameplay reader's existing projection is unchanged. This is not
a general SH virtual machine and never executes x86. Generated JSON, YAML,
meshes, SH and LIB files remain local retail derivatives.

## Test the separate aircraft on Windows

Close FA, extract the candidate ZIP outside the game directory, and copy **only
FAXX.LIB** beside FA.EXE. Keep the filename and leave stock libraries unchanged.
Do not also copy the duplicate loose PT/SH files. If that library name already
exists, check the conflict before copying. Start FA normally; the shortcut's
"Start in" directory should be the folder containing FA.EXE.

Look for F/A-XX / F/A-XX Concept in Create Quick Mission. Selection inherits the
F-22N's availability/filter settings. First test menu selection and flight startup,
then external geometry, both rudder directions and the hook. Animation changes
are discrete endpoints. To undo, close FA and remove the added FAXX.LIB.

The [packaging baseline](baselines/fa-xx-packaging.md) records the verified
startup scan and John's successful original FA flight/decal-fix reports for the
earlier F-22A-based package. The F-22N-based package, Kapset compatibility and
detailed control/damage acceptance remain unverified in original FA.

## Included source and porting information

The retail-free source kit contains the complete supporting Rust workspace,
format readers, extraction scripts, tests, specs and documentation. It builds
without retail media. Running the simulator or generating donor-based exports
requires the recipient's own files. See [development setup](DEVELOPMENT.md),
[extraction](EXTRACTION.md), [aircraft import](aircraft-import.md) and the
[concept contract](spec/fa-xx.md).

| Concern | Source |
| --- | --- |
| Concept identity and F-22N resource reuse | `crates/tore-formats/src/aircraft.rs` |
| Fin masks and split flap geometry | `crates/tore-app/src/roster_animation.rs` |
| Native hook rig and stow animation | `crates/tore-app/src/additional_animation.rs` |
| Hook travel and donor flight response | `crates/tore-sim/src/flight.rs` |
| Damaged-body fin masks | `crates/tore-app/src/damage_art.rs` |
| Original-format export adapter | `tools/export_faxx.py` |
| Static conversion/packaging setup | `tools/openfa_tools.py`, `tools/openfa/static-export.patch` |

OpenFA's relevant original conversion/packing command sources are captured in
`tools/openfa/upstream/`, with GPL license and provenance hashes. They are source
references, not standalone scripts. Full OpenFA and Nitrogen checkouts remain
local and can be restored by setup. No upstream binary is redistributed in the
source kit. [Tool reference notes](../tools/openfa/README.md).

The original-format candidate is separate from the source kit: its modified SH
files are donor-derived, whereas the source ZIP includes no retail assets.
Build the source ZIP from a Git checkout with:

```sh
python3 tools/package_faxx.py --out .local/packages/fa-xx-source.zip
```

It packages approved tracked source paths and explicitly listed handoff files,
rejects links and recognized retail payloads, and includes a per-file hash
manifest. It uses current working-tree contents and records the base commit.
Untracked local research and generated exports are excluded. Output must be new.
Code/docs retain [GPL-3.0](../LICENSE); see the
[third-party notices](../THIRD_PARTY_NOTICES.md). References to `.local/` and
community attachments describe evidence not shipped in the source kit.
