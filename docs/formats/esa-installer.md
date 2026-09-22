# Fighters Anthology disc installer container (SETUP.ESA)

> **T.O.R.E: Tasteful Opinionated Reverse Engineered.**
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v2 -->

> **Research notes, research mode.** Recovered facts about the original
> game's data and code, kept as evidence. Requirements, gates and remaining
> work described here are research-mode scope; they are not acceptance gates
> for gameplay. Parity is measured by expression of feature, see
> [AGENTS.md](../../AGENTS.md). Player-visible behaviour is specified in
> [docs/spec/](../spec/).

Research pass of 2026-09-21 for the [installer and first-run import
milestone](../ROADMAP.md#1g-installer-and-first-run-import). It answers two
questions: what a player's retail disc contains that the importer needs, and
whether the disc's unpatched executable can feed the same inert tables the
importer already reads from the patched one.

## Why this matters

The app importer needs `FA_1.LIB`, `FA_2.LIB` and `FA.EXE`, and optionally
`FA_4B.LIB` and `FA_4D.LIB` for recorded music. None of those are loose files on
the retail discs. They are packed inside `disc1/SETUP.ESA`, the Electronic Arts
installer container. Decoding that container is what lets a player import
straight from a mounted disc or ISO without running the 1998 installer.
Disc 2 is not needed by the importer.

## Container layout

Source: `disc1/SETUP.ESA`, 109,979,167 bytes. Everything below was read as
bounded data; no installer code was executed.

The file is a header followed by one contiguous data area.

1. Magic: the NUL-terminated ASCII string `ELECTRONIC_ARTS_ARCHIVE_FILE`.
2. A directory of entries, each:

   | Field | Encoding |
   | --- | --- |
   | name | NUL-terminated ASCII, e.g. `FA_1.LIB` |
   | group | NUL-terminated ASCII, e.g. `FA_LIBS`, `FA_EXECUTABLE_FILES` |
   | attributes | u32 little-endian; `0x211` for game files, `0x221` for the installer's own tools |
   | decoded size | u32 little-endian |
   | timestamp | u32 little-endian, DOS-era seconds; not needed |
   | method | NUL-terminated ASCII, `NULL` (stored) or `PKWA` (PKWare DCL) |
   | packed size | u32 little-endian |
   | offset | u32 little-endian, absolute file offset of the packed bytes |

3. The directory ends with an entry whose name is empty (a lone NUL). That
   terminator sits at offset 1,183, which is also the first entry's data offset.
4. Entries are stored back to back with no gaps: each entry's `offset + packed
   size` is the next entry's offset, and the last one ends at the file's end.

`PKWA` streams begin with the two-byte DCL header `00 06`: binary literals,
4 KiB dictionary. That is the mode `tore-formats/src/dcl.rs` already decodes
for the LIB archives. All sixteen compressed entries decoded to exactly their
declared sizes with a research-only Python port of the same algorithm. The
`NULL` entries are byte slices; the four LIB archives are stored, not
compressed, so a reader can hand them to the existing EALIB reader by offset
without copying.

## Directory of the supplied disc

| Name | Group | Method | Decoded bytes |
| --- | --- | --- | --- |
| FA.EXE | FA_EXECUTABLE_FILES | PKWA | 1,299,968 |
| FA.SMS | FA_EXECUTABLE_FILES | PKWA | 104,452 |
| JANE'S HOME PAGE.URL | FA_INTERNET | NULL | 49 |
| EAHELP.HLP, README.TXT, IP.EXE, IP.CFG | FA_README | mixed | |
| FA_1.LIB | FA_LIBS | NULL | 28,501,531 |
| FA_2.LIB | FA_LIBS | NULL | 31,546,576 |
| FA_4B.LIB | FA_LIBS | NULL | 34,670,738 |
| FA_4D.LIB | FA_LIBS | NULL | 13,756,838 |
| CHAT.TXT, BRIEFING.TXT, EXAMPLE.MT, LICENSE.TXT | FA_MISC | PKWA | |
| WAIL32.DLL | FA_SOUND_DRIVER_FILES | PKWA | 135,680 |
| CDRVDL32/CDRVHF32/CDRVXF32/COMMSC32.DLL | COMMDRV_DLLS_FILES | PKWA | |
| EAREMOVE.EXE, EAEXEC.EXE | remover / exec | PKWA | |
| PKCOMP.IDKDECODLL | SETUP_SPECIAL_FILES | NULL | 19,968 |

Twenty-three entries. Only the first row and the four `FA_LIBS` rows matter to
the importer. Nothing in the `.EXE`/`.DLL` rows is ever run.

## Disc build versus patched build

The supplied installed directory carries patch 1.02F. The disc carries 1.0.
Compared entry by entry after extracting both archive versions:

- `FA_4B.LIB` and `FA_4D.LIB`: identical SHA-256 in both builds.
- `FA_2.LIB`: same 5,405 resources. 168 of them differ by exactly four bytes at
  resource offset 136, the PE link timestamp of the DLG/MNU/HUD/MC module
  resources. Their content is otherwise identical.
- `FA_1.LIB`: 1.02F adds five resources, `H3D_OFF.PIC`, `H3D_ON.PIC`,
  `HUI11.FNT`, `HUISYM11.FNT`, `WII11.FNT`. Nothing in the app or its profiles
  references them. The other 1,996 resources are identical.
- `FA.EXE`: a different build. Disc SHA-256
  `c7d2c1cc9d27a6b364eca4245892cb6ca61ca72afc9a46e5760ee1fe7d75ba9b`
  (1,299,968 bytes); patched, reviewed SHA-256
  `e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c`
  (1,319,424 bytes).

So a disc import yields the same gameplay data as an installed 1.02F import.
The only reader that must know the build is the executable-table reader.

## Inert tables in the 1.0 executable

The importer reads four inert table families from the reviewed executable
through `tore-formats`. Each was located in the 1.0 build by searching for its
1.02F content, then decoded with the same rules and compared. Every list,
record and phrase is identical between builds.

| Table | Reader | 1.02F address | 1.0 address | Check |
| --- | --- | --- | --- | --- |
| Creator field dispatch (30 words) | `ui::creator` | `0x42e86c` | `0x42e39c` | all 30 words are 1.02F values minus `0x4d0`; lists identical |
| Creator field sentinels | `ui::creator` | `0x42e747`, `0x42e799` | `0x42e277`, `0x42e2c9` | same positions |
| Creator target dispatch (16 words) | `ui::creator` | `0x42e95c` | `0x42e48c` | 16 lists identical |
| Cloud repeat call (7 bytes) | `weather::clouds` | `0x4a8bda` | `0x4a5a4a` | bytes `6a 02 68 00 00 00 02` in both |
| Cloud placement records (26 bytes each) | `weather::clouds` | `0x50c298` | `0x507b88` | 9 records identical |
| Lens-flare descriptors (12 bytes each) | `weather::flare` | `0x50c8d8` | `0x508190` | 9 records identical |
| Radio phrase pointer pairs | `radio` | `0x4ff170` .. `0x4ff990` | each 1.02F address minus `0x4608` | all 26 stems and texts identical |

Code moved by a uniform `0x4d0` in the creator region; `.data` moved by
`0x4608` for the radio table, `0x4710` for the cloud table and `0x4748` for
the lens-flare table, so the readers carry a per-build address set rather
than a single delta.

## Implementation notes for the readers

- The build table lives in `tore-formats/src/executable.rs`: a `Build` enum, a
  `Layout` of per-build table addresses, the `LAYOUTS` array of the two reviewed
  builds, and `identify`, which maps an executable's SHA-256 to its layout.
- All four readers gate on `identify` and take their addresses from the returned
  layout, keeping their bounded reads, size caps and content validation
  unchanged. Each also exposes a `parse_with`/`phrases_with` entry point that
  takes a layout directly, so synthetic fixtures can exercise both address sets.
- The import report should name the build it read, `1.0 (disc)` or `1.02F`.
- Do not accept unknown builds. `identify` refuses one by name, quoting the
  computed hash; a third hash is a new research pass.

## Not covered

- Other retail pressings or localised discs: not seen. Their `SETUP.ESA`
  directory may list different sizes; the reader must trust the directory, not
  this table.
- `PKCOMP.IDKDECODLL` and the DCL variant used by other EA installers: not
  examined. If a future disc uses a DCL header other than `00 06`, the reader
  reports it as unsupported.
- Reading a raw `.iso` file: out of scope. The player mounts the disc or image
  and points the app at the folder that contains `SETUP.ESA`.
