"""Baseline retail-data guard. Uses only the Python standard library."""

import argparse
import re
from pathlib import Path
import struct
import subprocess
import sys


RETAIL_SUFFIXES = {".lib", ".esa", ".pic", ".pal", ".fnt", ".dlg", ".mnu", ".lay", ".11k", ".5k", ".xmi", ".mus", ".pack"}
LOCAL_ROOTS = {"gameassets", "USNF-ATF", ".local"}


def looks_like_pic(data, start=0):
    """Recognize the recovered 64-byte PIC header; PIC has no ASCII magic."""
    if len(data) - start < 64:
        return False
    kind = int.from_bytes(data[start:start + 2], "little")
    width, height, offset, size = struct.unpack_from("<4I", data, start + 2)
    return (
        kind in (0, 1)
        and 0 < width <= 16384
        and 0 < height <= 16384
        and offset == 64
        and 0 < size <= len(data) - start - 64
        and (kind == 1 or size == width * height)
        and data[start + 50:start + 64] == bytes(14)
    )


def violation(path, data):
    if path.suffix.lower() in RETAIL_SUFFIXES:
        return "retail asset extension"
    if looks_like_pic(data):
        return "PIC header"
    # Format names in source and documentation are allowed. Binary payloads are not.
    try:
        text = data.decode("utf-8")
        is_text = "\0" not in text
    except UnicodeDecodeError:
        is_text = False
    if not is_text:
        # A decoder legitimately contains the format name. Require a plausible
        # directory and sentinel, not just its compiled string constant.
        for match in re.finditer(b"EALIB", data):
            start = match.start()
            if start + 7 > len(data):
                continue
            count = int.from_bytes(data[start + 5:start + 7], "little")
            sentinel = start + 7 + count * 18
            directory_size = 7 + (count + 1) * 18
            if count and sentinel + 18 <= len(data) and data[sentinel:sentinel + 14] == bytes(14):
                end = struct.unpack_from("<I", data, sentinel + 14)[0]
                first = struct.unpack_from("<I", data, start + 21)[0]
                if directory_size <= first <= end <= len(data) - start:
                    return "embedded EALIB archive"
        # Locate the invariant pixel-block offset, then validate a PIC header.
        for match in re.finditer(b"\x40\x00\x00\x00", data):
            start = match.start() - 10
            if start >= 0 and looks_like_pic(data, start):
                return "embedded PIC header"
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("paths", nargs="*", type=Path,
                        help="Explicit files/directories, e.g. release artifacts. Default: Git-visible files.")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.paths:
        paths = []
        for path in args.paths:
            if not path.exists():
                parser.error(f"Path does not exist: {path}")
            if path.is_dir():
                paths.extend(p for p in path.rglob("*") if p.is_file())
            else:
                paths.append(path)
    else:
        output = subprocess.check_output(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root
        )
        paths = [root / name.decode("utf-8") for name in output.split(b"\0") if name]
    failures = []
    checked = 0
    for path in sorted(set(paths)):
        try:
            relative = path.resolve().relative_to(root)
        except ValueError:
            relative = path
        if not args.paths and relative.parts[0] in LOCAL_ROOTS:
            failures.append(f"{relative}: local-only content is visible to Git")
            continue
        if not path.exists():  # Tracked file deleted in the working tree.
            continue
        if path.is_symlink() or not path.is_file():
            failures.append(f"{relative}: expected a regular file")
            continue
        checked += 1
        reason = violation(path, path.read_bytes())
        if reason:
            failures.append(f"{relative}: {reason}")
    if failures:
        print("Asset check failed:\n" + "\n".join(failures), file=sys.stderr)
        return 1
    print(f"Asset check passed ({checked} files).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
