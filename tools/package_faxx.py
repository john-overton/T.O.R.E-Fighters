"""Build a retail-free F/A-XX developer source kit using Python's standard library."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import zipfile

from check_assets import violation

ROOT = Path(__file__).resolve().parents[1]
PREFIX = "fa-xx-developer-kit"
ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "LICENSE", "MODS.md",
    "THIRD_PARTY_NOTICES.md", "README.md", "AGENTS.md", ".gitignore",
}
SOURCE_DIRS = {"crates", "tools", "docs", "assets", ".githooks"}
ADDITIONS = {
    "tools/package_faxx.py", "tools/test_package_faxx.py", "docs/fa-xx-developer-kit.md",
    "docs/baselines/fa-xx-packaging.md",
    "tools/check_shape_roundtrip.py", "tools/test_check_shape_roundtrip.py",
    "tools/export_faxx.py", "tools/test_export_faxx.py", "tools/validate_faxx_export.py",
    "tools/openfa_tools.py", "tools/fa_lib.py", "tools/test_fa_lib.py",
    "crates/tore-extract/examples/check_lib.rs", "docs/spec/fa-xx-export.md",
    "crates/tore-extract/examples/shape_json.rs", "crates/tore-extract/examples/check_faxx_pt.rs",
    "tools/openfa/README.md", "tools/openfa/static-export.patch",
    "tools/openfa/faxx-package-readme.txt", "tools/openfa/faxx-independent-readme.txt", "tools/openfa/upstream/LICENSE",
    "tools/openfa/upstream/sh.rs", "tools/openfa/upstream/lib_ext.rs",
    "tools/openfa/upstream/provenance.json",
}


def collect(root, names):
    """Allow only project source paths; reject links and recognized retail payloads."""
    files = {}
    for name in sorted(set(names)):
        path = Path(name)
        if path.is_absolute() or ".." in path.parts:
            raise ValueError(f"unsafe path: {name}")
        if not path.parts or (name not in ROOT_FILES and path.parts[0] not in SOURCE_DIRS):
            continue
        source = root / path
        if source.is_symlink() or not source.resolve().is_relative_to(root.resolve()):
            raise ValueError(f"unsafe source: {name}")
        if not source.is_file():
            raise ValueError(f"missing source: {name}")
        data = source.read_bytes()
        reason = violation(path, data)
        if reason:
            raise ValueError(f"{name}: {reason}")
        files[f"source/{path.as_posix()}"] = data
    return files


def write_archive(output, files, revision):
    files = dict(files)
    files["START-HERE.md"] = (
        "# F/A-XX developer kit\n\n"
        "Start with [the developer handoff](source/docs/fa-xx-developer-kit.md).\n\n"
        "Contains source, specifications, tests and extraction tools. "
        "No retail assets or compiled executables are included. "
        "This is not an installable mod for the original Fighters Anthology.\n\n"
        "Code and documentation: [GPL-3.0](source/LICENSE). "
        "See [third-party notices](source/THIRD_PARTY_NOTICES.md).\n"
    ).encode()
    manifest = {
        "format": 1,
        "base_git_revision": revision,
        "snapshot": "working-tree contents, including uncommitted edits to selected files",
        "files": {name: {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
                  for name, data in sorted(files.items())},
    }
    files["MANIFEST.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
    output.parent.mkdir(parents=True, exist_ok=True)
    # Exclusive creation prevents accidentally replacing a previous handoff.
    with output.open("xb") as stream, zipfile.ZipFile(stream, "w") as archive:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(f"{PREFIX}/{name}", date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            mode = 0o755 if name.startswith("source/.githooks/") else 0o644
            info.external_attr = (0o100000 | mode) << 16
            archive.writestr(info, data)
    return len(files)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / ".local/packages/fa-xx-developer-kit.zip")
    args = parser.parse_args()
    names = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    files = collect(ROOT, [n for n in names if n] + sorted(ADDITIONS))
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip()
    count = write_archive(args.out, files, revision)
    digest = hashlib.sha256(args.out.read_bytes()).hexdigest()
    print(f"Created {args.out} ({count} files)\nSHA-256: {digest}")


if __name__ == "__main__":
    main()
