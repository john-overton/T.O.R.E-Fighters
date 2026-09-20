# F/A-XX original-format packaging review

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

Research mode, 2026-09-18. The supplied V-22 package establishes a loose SH/PIC
handoff example. The supplied OpenFA source contains SH compilation and LIB
packing code. OpenFA now passes local unchanged-shape and LIB round trips. This establishes
a working file-writing route, not an installed or animated F/A-XX in original FA.

## Inputs

- `.local/openfa-review`, Git revision
  `7507fef5bbb126302a59cb413e80cadf5c547f9d`, workspace version 0.2.14.
- `.local/aircraft-fa/V-22 Dark Grey/V22.SH`, 20,992 bytes, SHA-256
  `dcce0de4d24b80782055c96f114e09afb3c7f60dc0dd0244dd7cbd8af44034ee`.
- That folder's `V-22 Dark Grey.txt`, 144 bytes, SHA-256
  `c0e0788379c3d763dcc76324f344f9d25328219873516261b4b3e8325ecc53eb`.
- `.local/roster-review/FA_2.LIB/F22.SH`, SHA-256
  `eba06b716e45431fa7401d99579eb0d4c8f1882eb80928c726186907c170f8b6`.
  Original build/media identity: [roster baseline](aircraft-roster-expansion.md).

The V-22 credits name USNRaptor for textures and CAG Hotshot for the shape,
dated 25 AUG 2026. The folder has one SH, five PICs, seven BMPs, one JPG and
the credit text. There is no PT, LIB, installer or installation instruction.
It is an appearance-resource example; standalone aircraft registration is not
established. The credit text is not a redistribution license. None of its assets
was copied into the source kit or repository.

## Static checks

Read `apps/ofa-tools/src/sh.rs`, `main.rs`, `lib_ext.rs`,
`crates/asset/sh/src/sh_code.rs` and `crates/asset/lib/src/writer.rs` in the
identified OpenFA checkout:

- The direct-file dispatch converts `.sh` to `.SH.yaml`, and `.sh.yaml` back
  to SH through `ShCode::from_yaml` and `compile_pe`.
- `compile_pe` reconstructs code, relocations, imports and a PE container.
  The included `can_roundtrip_all_shapes` test requires complete byte equality
  after SH/YAML/SH conversion. Its presence is source evidence, not a passing
  test result from this review.
- `lib pack` calls `LibWriter`; the writer can compress entries using PKWare.
  Our repository's extractor supports only part of that compression grammar.
  An OpenFA-generated archive is not automatically a valid regression fixture
  for our reader.
- The SH compiler writes the destination directly. Perform conversions only in
  a scratch copy, since converting back targets the original SH filename.

Ran our inert `tools/inspect_shape_effects.py` on both SH files. V22 imports
gear, left/right flap and thrust-vector angle state, plus `do_start_interp`.
F22 imports gear, left/right flap, canard, brake, bay, afterburner state and
`do_start_interp`. Neither import list contains a named rudder or hook state.
This is an import-table observation, not proof that the game cannot expose
those controls or that the candidates execute. No module was executed.

## Corrected static toolchain

The initial unpatched OpenFA conversion did run its emulated-x86 analysis pass.
Earlier session wording that it was purely static was incorrect. Source review
found `ShCode::from_bytes` calling `ShAnalysis::analyze`, which invokes an x86
interpreter. Those initial runs are not evidence of a non-executing workflow.

The corrected build uses `tools/openfa/static-export.patch`, which bypasses that
analysis under the `sh/static-export` feature. It retains bounded decoding,
disassembly and compilation. CLI errors are nonzero without modal dialogs, and
a marker probe verifies that the static feature was compiled. Our wrappers reject
an ordinary upstream executable. Patch and original command source provenance
are recorded under `tools/openfa/`.

Restored Nitrogen at `0691b37c66f0c8668a2c197a9c49b8c75f753c21` and linked
`crates/nitrogen` to `../nitrogen/crates`. `tools/openfa_tools.py setup --source
.local/openfa-review` passed on Linux with Rust 1.91.1. Upstream warnings remain.
The external checkout had no committed lockfile; initial dependency resolution
created one, and the final build used `--locked`. No reproducible initial
dependency resolution is claimed.

Repeated all four unchanged-input checks with the corrected build:

| Input | Bytes | SH/YAML/SH result |
| --- | ---: | --- |
| F22.SH | 25,088 | Exact byte equality |
| F22_A.SH | 16,896 | Exact byte equality |
| F22_C.SH | 12,800 | Exact byte equality |
| Supplied V22.SH | 20,992 | Exact byte equality |

Additional donor identities:

- F22_A.SH: `4d06b11b332f68981fd5613ca048190a601c84974b4e3edf79ad238a60a24fdb`.
- F22_C.SH: `c9a280ee6470fad95cd65fe497af2cb5c9543aeb6a7a3fd7d332c2c0922dfdf3`.

The corrected report is `.local/faxx-static-final-roundtrip/report.json`.
Original source files stayed unchanged. No original game was launched. The
upstream all-shapes tests were not run.

## Exported candidate

Implementation mode continuation, 2026-09-18. Contract:
[F/A-XX export](../spec/fa-xx-export.md). The final local candidate is under
`.local/exports/fa-xx-hook-enabled/`. It contains a separate FAXX.PT
and six FAXX-named shapes, with no F22-named resource entries. The earlier
replacement is retained as explicit `--identity f22` output.

| Output | Bytes | SHA-256 |
| --- | ---: | --- |
| FAXX.SH | 33280 | `e19463432c402a67c3d3d1084170cf1871462787832a750f026baa5fa8c52ea0` |
| FAXX_A.SH | 16896 | `1c272a2b9b37044118e1ce4d2f727b9f078500563532d52861cc080ce5c3e973` |
| FAXX_C.SH | 12800 | `aeed6e76db138ff0762ef26009f8bda4e96a6b9dcdfaffc0663bc4195242b04a` |

`tools/validate_faxx_export.py` checked 24 combinations: gear 0/1, flap 0/-1,
rudder -1/0/+1 and hook 0/1. It compares decoded geometry to the concept angles
with the documented integer rounding. With indexed materials retained, the neutral donor has 250 faces. The output
has 243: five fin faces and two separate fin-decal faces are removed. The other
three indexed decal faces remain. UVs and colors of retained faces are unchanged. Positive rudder adds four
right leaves; negative adds three left leaves. Hook deployment adds twelve
faces, all palette index 55, with minimum source z=-23. Shared vertex restoration
is covered by whole-model geometry comparisons. Both damaged bodies match their
donors with only the reviewed fin masks omitted.

The seven separate-aircraft resources are packed as stored EALIB entries by
`tools/fa_lib.py`, including the zero-name/flag sentinel with its EOF offset.
`check_lib.rs` validates the directory and compares every payload using the
independent tore-formats reader; OpenFA also unpacks matching payloads. The
ordinary extractor lists seven entries with zero errors. The generic BRF reader
checks names, geometry references and the hook capability bit, and confirms
all other donor tokens are unchanged; B/D/S aliases match their donors.

The previous archive was invalid: OpenFA's writer reserved no sentinel directory
entry and its own reader accepted that omission. Our earlier OpenFA-only round
trip therefore did not establish retail-format validity. After John reported
that F/A-XX was absent from the Windows aircraft list, the independent reader
rejected the previous FAXX.LIB with `invalid archive sentinel`. That is a verified
packaging fault and a possible explanation for the missing entry, not a confirmed
original-game trace. The corrected file is accepted by both readers. The generic
pack wrapper and exporter now use the corrected stored-entry writer. Earlier
ZIPs under `.local/exports/` are superseded. The checked definition contains
F/A-XX / F/A-XX Concept, not F-22N; the origin of John's F-22N list entry is unknown.

Reports include donor/output and exporter/tool hashes. The full ZIP contains the
hook-enabled PT, six SH files, equivalent LIB, notes and reports. The corrected Windows test
ZIP contains only FAXX.LIB and INSTALL.txt. No donor data is committed.

Synthetic tests cover the 0.6-radian leaf geometry, fixed hinge, integer slot
writes, retained UV/index bytes, branch-target padding, static-tool rejection,
no-op encoder detection, manifest hashes and retail/source separation. The
bounded Rust export projection tests valid jumps, invalid targets and cycles.
Its explicit-jump option does not change the gameplay projection.

The earlier reusable wrapper smoke test in `.local/faxx-toolset-smoke/` used
OpenFA on both sides and missed the same sentinel fault. Current export validation
uses the independent Rust archive reader in addition to OpenFA. Repository
formatting, Clippy with warnings denied, all 878 Rust tests, locked workspace
build, all 58 Python tests, documentation checks and source/binary asset checks
passed. No renderer changes were made, so a new GPU smoke test was not run.

## Separate aircraft registration evidence

Research mode, 2026-09-18, same reviewed FA.EXE/FA.SMS build as the roster
baseline. Static inspection, no original code execution:

- `@GetNames@4` at 0x41c840 selects the aircraft filename pattern through
  0x4eea38 at 0x41c8cd. That pointer resolves to `*.PT` at 0x4eea28;
  the associated fallback string is `types\\*.PT` at 0x4eea1c.
- The common enumeration path passes the pattern to `__FindFirst@8`
  (0x479a60) at 0x41ca19. Catalog processing loads the returned type resource
  around 0x41cad4–0x41caff and checks type byte 5 for aircraft.
- The donor PT's `ot_names` block carries short name, long name and F22.PT
  identity; `shape` and `shadowShape` are separate resource references.
- `_SetupOT` uses the shadow pointer at record +0x13 to derive the related
  shape names. For example 0x4a70ef–0x4a7138 derives the C body by replacing
  the character before the extension with `c`; 0x4a7146–0x4a7188 similarly
  derives D. This is why the new PT names FAXX_S.SH rather than retaining
  F22_S.SH while only renaming the intact model.

Source evidence is the existing hash-reviewed
`.local/weapons-research/native/fa-disassembly.txt`, its `symbols.json`, the
original EXE data strings, and the extracted F22.PT. The independent reader
comparison verifies the three identity/geometry blocks and the single hook-bit
change, with all remaining donor tokens preserved.
An external corroborating [PT reference](https://fighterscodex.com/fa/formats/PT/)
documents those record fields; the local address observations above establish
this build's catalog mechanism. No separate hard-coded aircraft-slot manifest
was found in the reviewed enumeration path. Campaign-specific aircraft pools
are separate and are not edited by this export.

Additional identity donor SHA-256 values:

- F22.PT: `e6b0009e2cfd48b53f18a80abcf2ae41d2c77bb8c66a5d7401b5ad0a62f86a15`.
- F22_B.SH: `69caaac4a05f41e52d85774fe9e95f4ad4368899aae5ea9305163a312ff19460`.
- F22_D.SH: `68d7c820cf0bfea4da15d97493ede5fd5f2175181f0e3b22dea038a4845018a8`.
- F22_S.SH: `4be45511cb9b2d7d2a72a9eb2ee516d584115b375328eb18b352d2f1368726ed`.

## Windows library discovery

Research mode, same reviewed build. `_LibStartUp` at 0x478bc0 scans the working
directory using `*.*`: the pattern at 0x4f7fdc is passed at 0x478cc2. It tests
filenames against `.LIB` (0x4f7fd4) around 0x478db9 and checks EALIB header bytes
around 0x478e13. This supports a separately named FAXX.LIB beside FA.EXE when
launched from that directory; no FA_5 naming convention or merge into a stock
archive is required for the unique FAXX resources. The
[external memory/resource analysis](https://fighterscodex.com/fa/memory-resource/)
independently documents this startup indexing behavior. Earlier handoff wording
that arbitrary LIB discovery was unknown is superseded by this inspected path.

The Windows test ZIP contains only FAXX.LIB and installation notes. Its library
bytes are identical to the validated separate-aircraft package. No original-game execution was performed by the agent; John's subsequent
flight report is recorded below.

## User flight evidence and decal correction

John reported that the corrected package works in original FA on his Windows
machine and provided a screenshot showing the finless aircraft in flight with
a floating wolf decal. The image is retained locally as
`.local/exports/fa-xx-no-floating-decals/user-flight-before-decal-fix.png`.
The exact installed game/mod build and archive hash were not supplied, so this
is user-reported flight evidence, not a directly instrumented acceptance run.

Static donor inspection locates two indexed-texture decal quads on the fin
planes, separate from the five removed fin faces. Their addresses have one home
in [objects and shapes](../formats/objects-and-shapes.md). Both are now removed
by the exporter. The previous bounded projection skipped indexed-texture faces,
so its successful pose checks could not detect this floating artwork. The
export-only projection now includes them with unresolved texture labels, and a
synthetic regression covers that distinction. The game's existing projection
is unchanged. All 24 pose checks now compare the indexed decal geometry too.
John subsequently confirmed that the decal-fixed package works in the same
session on 2026-09-18. This closes the reported floating-decal issue at the
user-observed level. It does not establish a full control/damage test matrix or
Kapset compatibility.

## Hook command correction

John reported that the working aircraft did not show the hook. The export
already had twelve hook faces conditional on `_PLhook == 1`, but its PT still
inherited the donor's disabled hook capability. The previous geometry tests
supplied the state directly, so they did not test whether FA could enable it.

Static review of the same hash-reviewed original build establishes:

- `_cpt` / `_curThingType` is at 0x50d268. The PLANE_TYPE flags are at +0xba,
  matching the field read at 0x50d322 by `@FMHook@4` at 0x451c30.
- 0x451c35 tests flag bit 0x02 and 0x451c37 returns without operating the hook
  when it is absent. The remaining command sets/clears instance state bit 0x400
  at 0x451c6b / 0x451c80.
- The shape-state producer zeros BX at 0x4ab45c, clears `_PLhook` (0x580ba4)
  at 0x4ab496, then sets it to 1 at 0x4ab724 when the instance has bit 0x400.

The [aircraft format note](../formats/aircraft.md#hook-capability-in-original-game-exports)
records the flag values. Both export identities now include a PT with hook
capability enabled. The independent BRF comparison permits precisely this bit
change plus the separate identity's names/references, and rejects other field
changes. Synthetic tests check preservation of other sections, idempotence and
rejection of another donor's flags. No original code was executed for this review.
The previous flight/decal success remains user-confirmed; this hook correction
needs a new Windows check with the hook command. The rebuilt separate
aircraft passed all 24 geometry combinations, PT capability verification and both
archive readers. The F-22 replacement mode also passed with its new PT payload.
All six separate-aircraft SH files are byte-identical to the decal-fixed package;
FAXX.PT differs by one ASCII byte, changing `$91` to `$93`. The Windows handoff is
`.local/exports/fa-xx-hook-enabled/F-A-XX-FA-Windows-hook-fix.zip`.

## Remaining validation limits

The candidate uses discrete poses, wider quantized hook geometry, neutral donor
flap skins and bypassed C8 LOD branches, as documented in the export contract.
F31/F14 import-table inspection identifies existing rudder/hook symbol names;
OpenFA's state table informs their selected values. Original-game producer values,
rudder sign, live hook appearance after the capability correction, palette appearance, draw order, long-distance
behavior, a repeatable live-game acceptance run and Kapset compatibility were not validated.

Static conversion and bounded pose checks alone do not demonstrate playability.
John's successful flight report supplies limited live-game evidence; remaining
controls and damage still need detailed recipient checks; John confirmed the
decal correction works. The next research targets are those control producers and
loading conventions, rather than assuming that a successful compiler run proves
the complete original-engine contract.

## Kapset identity

John does not have the recipient's Kapset 3.0 files. Public community evidence
identifies Kap's Kapset in the [Korea campaign credits](https://myplace.frontier.com/~ocsdor2/FA_Korea.htm),
and the [USNRaptor site](https://myplace.frontier.com/~usnraptor/) cites weapon
content from Kapset 3. Neither page establishes the exact recipient archive,
load order or F22 resource identity. That compatibility remains unknown, but
it does not block the demonstrated stock-donor conversion work.
