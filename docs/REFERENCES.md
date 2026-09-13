# Local reference material

Both directories below are ignored, user-supplied inputs. Neither is required to compile the Rust shell, and neither should be copied into tracked source or bundled in releases.

| Location | Use |
| --- | --- |
| `gameassets/fighters-anthology/` | User's local Fighters Anthology installation/media |
| `USNF-ATF/` | TypeScript/Three.js project, recovered specifications and comparison baselines |

The initial media folder contains `FA_1.LIB`, `FA_2.LIB`, `FA_4B.LIB`, `FA_4D.LIB`, `swpatch.lib`, executables, and mission files. This is a presence check, not the full disc/title census required by M0.

Reference checkout observed at commit `2d818054ff51db9f3353d0548dbd0e469b275a1a`, with a clean working tree. If missing, clone from the repository root:

```sh
git clone https://github.com/john-overton/USNF-ATF.git USNF-ATF
```

## Start with menus

Read these inside the local reference checkout:

- `Docs/menu-porting.md`, `Docs/game-shell-plan.md`: recovered menu/shell behavior.
- `Docs/formats/ealib.md`, `esa.md`, `dcl.md`: container/decompression specifications.
- `Docs/formats/pal.md`, `pic.md`, `fnt.md`, `mnu.md`: palette, image, font, and menu formats.
- `Docs/baselines/game-shell.md`, `menu-revision.md`, `mission-menu-revision.md`: comparison evidence.
- `tools/retail/retail/menu.py`, `ealib.py`, `pic.py`, `fnt.py`, `mnu.py`: decoders to consult for format behavior.

The reference's `Docs/` capitalization is intentional. Rebuild documentation belongs in lowercase `docs/` so paths work on case-sensitive Linux filesystems.

This is an initial orientation, not a completed spec-versus-implementation inventory. Validate recovered claims against the user's Fighters Anthology files as each importer format is implemented. Record provenance and synthetic test cases without committing extracted retail data.
