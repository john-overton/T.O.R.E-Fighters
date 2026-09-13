# Extracting local game archives

`tools/extract_assets.py` is the shared, cross-platform extraction entry point. It runs the repository's Rust `tore-extract` tool and adds SHA-256 provenance to its report. It requires Python 3.10+ and the pinned Rust toolchain. There is no dependency on USNF-ATF, Bun/Node, graphics drivers, an audio device, or third-party Python packages.

## Two different workflows

| Task | Command | Output |
| --- | --- | --- |
| Play/test the menu | `cargo run --locked -p tore-app -- --import gameassets/fighters-anthology` | Selective menu cache in application data; launches the app |
| Explore/extract any supported archives | `python3 tools/extract_assets.py` | All resources under ignored `.local/extracted/` |
| Inspect archive metadata only | `python3 tools/explore_assets.py` | Inventory under ignored `.local/exploration/` |

The app does not need a full extraction. It reads the original archives directly through the same format library and imports only what its menu needs. Full extraction is for research and future format development.

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

It unpacks **all resource types** as their original decompressed bytes. It does not claim to decode every resource: a `.SH` remains a shape resource, `.PIC` remains an indexed game image, `.FNT` remains a compiled resource, and `.11K` remains PCM. Nothing extracted is executed. The app currently interprets only its menu subset.

ISO images, ESA installer containers, coded-literal DCL mode 1, missing/truncated media repair, PNG/WAV/model conversion, and cross-title gameplay import are not implemented by this command. Supply loose archives from your own installed or extracted media. Unknown/non-EALIB `.LIB` files are reported as errors rather than silently accepted.

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
