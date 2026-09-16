"""Documentation header guard. Uses only the Python standard library.

Every Markdown file in the covered directories carries the T.O.R.E header
directly under its title. `--check` (the default, and what CI runs) reports
files that are missing it or carrying an older revision. `--fix` writes it.

The header text lives in HEADER below. To reword it, edit HEADER, raise
HEADER_REVISION, and run `python3 tools/check_docs.py --fix`: the old block is
found by HEADER_PREFIX and replaced, never stacked. Rewording the opening line
is safe, because detection matches the prefix rather than the whole line.
"""

import argparse
from pathlib import Path
import subprocess
import sys


# Bump whenever HEADER changes, so existing files are rewritten rather than
# reported as already-headed.
HEADER_REVISION = 2

# An existing block is found by this prefix, not by the exact first line, so
# rewording the opening sentence replaces the old header instead of stacking a
# second one beneath it.
HEADER_PREFIX = "> **T.O.R.E"

# Keep the header free of links: it must be one identical string at every
# directory depth, and a relative path is not.
HEADER_MARKER = f"{HEADER_PREFIX}: Tasteful Opinionated Reverse Engineered.**"

HEADER = f"""{HEADER_MARKER}
> The thing being reverse engineered is the *experience*, not the executable. We
> trace what a player does and what the game does back, down to the numbers they
> would notice. How the original code achieved it is history: useful evidence,
> never a blueprint. If a sentence below reads like an instruction to reproduce
> the original's internals, it is out of date.
> <!-- tore-header v{HEADER_REVISION} -->"""

COVERED_DIRECTORIES = (
    "docs",
    "docs/formats",
    "docs/baselines",
    "docs/research",
    "docs/spec",
)

# README.md and AGENTS.md already open with the parity statement in full; a
# banner would only repeat them. The realignment report is a dated record of how
# the header came to exist and reads oddly underneath it.
EXEMPT = {
    "README.md",
    "AGENTS.md",
    "docs/doc-realignment-2026-09-15.md",
}


def covered(relative):
    """True when this tracked path is one of the documents we head."""
    path = Path(relative)
    return (
        path.suffix == ".md"
        and relative not in EXEMPT
        and path.parent.as_posix() in COVERED_DIRECTORIES
    )


def split_header(text):
    """Return (before, existing_header_or_None, after).

    The header sits directly under the first level-one heading, so a frozen
    archive's supersession banner above the title keeps its place.
    """
    lines = text.split("\n")
    title = next((i for i, line in enumerate(lines) if line.startswith("# ")), None)
    if title is None:
        return None
    start = title + 1
    while start < len(lines) and not lines[start].strip():
        start += 1
    if start < len(lines) and lines[start].startswith(HEADER_PREFIX):
        end = start
        while end < len(lines) and lines[end].startswith(">"):
            end += 1
        # Drop the blank lines between the title and the old block: the caller
        # re-adds exactly one, so repeated rewording cannot accumulate them.
        return lines[: title + 1], lines[start:end], lines[end:]
    return lines[: title + 1], None, lines[title + 1 :]


def apply_header(text):
    """Return the document with the current header in place, or None if unchanged."""
    split = split_header(text)
    if split is None:
        return None
    before, _existing, after = split
    while after and not after[0].strip():
        after = after[1:]
    updated = "\n".join(before + [""] + HEADER.split("\n") + [""] + after)
    # Comparing whole documents, not just the block, also normalizes the spacing
    # around a header that is otherwise current.
    return None if updated == text else updated


def tracked_documents():
    listed = subprocess.run(
        ["git", "ls-files", "*.md"], capture_output=True, text=True, check=True
    ).stdout.split()
    return [relative for relative in listed if covered(relative)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fix",
        action="store_true",
        help="write the header into files that need it, instead of reporting them",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="report files that need the header and exit non-zero (default)",
    )
    arguments = parser.parse_args()

    stale = []
    fixed = 0
    checked = 0
    for relative in tracked_documents():
        path = Path(relative)
        if not path.exists():  # Tracked file deleted in the working tree.
            continue
        text = path.read_text(encoding="utf-8")
        updated = apply_header(text)
        if updated is None:
            if split_header(text) is None:
                stale.append(f"{relative}: no level-one heading to place the header under")
                continue
            checked += 1
            continue
        if arguments.fix:
            path.write_text(updated, encoding="utf-8")
            fixed += 1
        else:
            stale.append(f"{relative}: missing or outdated T.O.R.E header")

    if stale:
        print("Documentation header check failed:\n" + "\n".join(stale), file=sys.stderr)
        print(f"\nRun `python3 {Path(__file__).name}` with --fix to write them.", file=sys.stderr)
        return 1
    if arguments.fix:
        print(f"Documentation header check passed ({checked} files, {fixed} updated).")
    else:
        print(f"Documentation header check passed ({checked} files).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
