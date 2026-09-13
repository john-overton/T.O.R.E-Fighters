"""Baseline retail-data guard. Uses only the Python standard library."""

import argparse
from pathlib import Path
import struct
import subprocess
import sys


RETAIL_SUFFIXES = {".lib", ".esa", ".pic", ".pal", ".fnt", ".dlg", ".mnu", ".lay"}
LOCAL_ROOTS = {"gameassets", "USNF-ATF", ".local"}


def looks_like_pic(data):
    """Recognize the recovered 64-byte PIC header; PIC has no ASCII magic."""
    if len(data) < 64:
        return False
    kind = int.from_bytes(data[:2], "little")
    width, height, offset, size = struct.unpack_from("<4I", data, 2)
    return (
        kind in (0, 1)
        and 0 < width <= 16384
        and 0 < height <= 16384
        and offset == 64
        and 0 < size <= len(data) - 64
        and (kind == 1 or size == width * height)
        and data[50:64] == bytes(14)
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
    if not is_text and b"EALIB" in data:
        return "embedded EALIB archive marker"
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
            paths.extend(p for p in path.rglob("*") if p.is_file()) if path.is_dir() else paths.append(path)
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
