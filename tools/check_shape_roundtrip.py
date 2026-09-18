"""Check OpenFA SH/YAML/SH conversion in fresh scratch copies, never original media."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from openfa_tools import require_static


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check(tool, source, directory):
    before = source.read_bytes()
    directory.mkdir()
    scratch = directory / source.name
    shutil.copyfile(source, scratch)
    record = {"source": str(source), "input_sha256": digest(before), "input_bytes": len(before)}
    try:
        yaml = scratch.with_suffix(".SH.yaml")
        for stage, path in [("decode", scratch), ("encode", yaml)]:
            result = subprocess.run([str(tool), str(path)], cwd=directory,
                                    capture_output=True, timeout=60, check=False)
            (directory / f"{stage}.log").write_bytes(result.stdout + result.stderr)
            if result.returncode:
                raise ValueError(f"{stage} exited {result.returncode}; see {stage}.log")
            expected = yaml if stage == "decode" else scratch
            if not expected.is_file():
                raise ValueError(f"{stage} did not create {expected.name}")
            if stage == "decode":
                # Do not let a no-op encoder appear to pass by leaving the input in place.
                scratch.unlink()
        after = scratch.read_bytes()
        record.update(output_sha256=digest(after), output_bytes=len(after), identical=after == before)
        if after != before:
            record["first_difference"] = next(
                (i for i, (a, b) in enumerate(zip(before, after)) if a != b),
                min(len(before), len(after)),
            )
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        record.update(identical=False, error=str(error))
    record["source_unchanged"] = source.read_bytes() == before
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", required=True, type=Path, help="Built ofa-tools executable")
    parser.add_argument("--out", required=True, type=Path, help="New local scratch directory")
    parser.add_argument("shapes", nargs="+", type=Path)
    args = parser.parse_args()
    tool = args.tool.resolve()
    require_static(tool)
    sources = [p.resolve() for p in args.shapes]
    if not tool.is_file() or any(not p.is_file() or p.suffix.lower() != ".sh" for p in sources):
        parser.error("tool and SH inputs must exist")
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=False)
    report = {"tool": str(tool), "tool_sha256": digest(tool.read_bytes()), "results": []}
    for i, source in enumerate(sources):
        record = check(tool, source, output / f"{i:02d}-{source.stem}")
        report["results"].append(record)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(f"{source.name}: {'PASS' if record['identical'] and record['source_unchanged'] else 'FAIL'}")
    print(f"Report: {output / 'report.json'}")
    return 0 if all(r["identical"] and r["source_unchanged"] for r in report["results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
