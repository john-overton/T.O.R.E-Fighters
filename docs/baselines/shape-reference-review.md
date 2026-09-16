# Local shape-reference review — 2026-09-15

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


Outcome: added [objects and shapes](../formats/objects-and-shapes.md), explaining
resource ownership, shape families, detail/component groups, material state,
attachment points and editing dependencies. This is documentation research;
no importer, renderer, aircraft support or simulation behavior changed.

## Inputs and method

Reviewed the user-supplied `.local/fa-shape-file-explination/` collection:

- Plurry's v2.01 introduction and Tools guide.
- Chapters 03-02 through 03-11: textures, vertices, normals, face properties,
  visibility structures, internal links, native re-entry, vapor and gun origins.
- `03-01 - F18C.SH - HEX v2.01.xlsx`: annotated shape layout and component links.
- `03-071 - FC - Face Normals - Resolved!!!.xlsx`: signed-word normal examples
  and the included historical discussion of the older export interpretation.
- Sample F18C SH, textual PT, original/corrected CSV, YAML representations and
  three OBJ detail exports. Bundled executables were not run.

DOCX paragraphs and XLSX cells were read directly from their ZIP/XML contents
using Python's standard library. This includes text/table content and workbook
values; it is not a visual acceptance review of embedded diagrams or screenshots.
Derived text, hashes, inventories and check logs are ignored under
`.local/shape-doc-review/`. Original documents and retail-derived examples remain
local; none is copied into tracked files.

The sample `03 - Sample Files/F18C.SH` is 33,280 bytes, SHA-256:

`6a5b68c1415d9b6edd90a9c1ca5cf4df6af2f4e9049c6d23db938ac8135f870d`

This identifies the supplied example, not a verified extraction from the
current FA archives and not our supported F18.SH identity.

## Checks and findings

| Check | Result and limit |
| --- | --- |
| Corrected CSV versus SH | All 1,799 data rows match their declared byte ranges; no size mismatch, gap or overlap; coverage ends at byte 33,280. This validates byte preservation, not opcode labels. |
| Original CSV | Different schema and 1,585 data rows; its grouping must not be compared by row number to the corrected export. |
| Near/medium/far OBJ | Respectively 17/3/1 object groups, 452/215/97 vertices and 326/172/60 faces. No UV or normal records. These are export counts, not retail visible-face counts. |
| Main vertex block | 330 vertices at file offset 1153; the guide's later 0–229 enumeration is inconsistent with that count. |
| C8 links | Targets 25225 and 18501 agree with the raw sample and annotated guide. No native distance-threshold validation. |
| C4 gear link | File 15374 targets 16398 after its 16-byte record and displacement 1008. The translation is present; native rotation/timing remains unvalidated. |
| Import inspection | Existing inert inspector reports nine imports, including separate gear-down/gear-position state and `do_start_interp`; scanning candidates does not prove executed paths. |
| Vapor coordinates | Raw CE points divided by 256 differ from chapter 03-10's example; preserve the version distinction and existing native-backed CE layout. |
| Reader comparison | Vertex destination slots, FC word normals and face-width flags already exist. F6 normal vectors, full C4 rotations, runtime decal slots, shadow semantics and general visibility/LOD remain incomplete. |

To repeat the CSV identity check without installing the bundled tools:

```sh
python3 - <<'PY'
import csv
from pathlib import Path

root = Path('.local/fa-shape-file-explination/03 - Sample Files')
data = (root / 'F18C.SH').read_bytes()
end = count = 0
with (root / 'F18C_DECOMP.SH.CSV').open(encoding='utf-8-sig', newline='') as f:
    for row in csv.DictReader(f):
        offset, size = int(row['file_offset']), int(row['instr_size'])
        payload = bytes.fromhex(row['raw_data'])
        assert offset == end
        assert len(payload) == size
        assert data[offset:offset + size] == payload
        end = offset + size
        count += 1
assert end == len(data)
print(count, 'matching rows;', end, 'bytes covered')
PY
```

## Validation and remaining acceptance

Linux workspace formatting, Clippy with warnings denied, tests and build passed
with the locked dependency set; Python tool tests passed. The asset guard passed
for all 211 Git-visible files and both debug executables. All 134 local file links
in the five edited documents resolved, and `git diff --check` passed.

No rendering changed, so no new GPU smoke/capture acceptance was needed for
this documentation pass. Windows/macOS builds and retail visual comparisons
were not run. Existing unrelated workspace changes were left intact; the
workspace checks describe the combined working tree.

Remaining research: verified object/damage selection and M/MM placements,
general SH visibility/LOD, exact material/Gouraud behavior, native transformed
parts, and round-trip writer contracts. The guide's naming patterns, proposed
coordinate edits and conjectured visibility grammar do not close these gates.
