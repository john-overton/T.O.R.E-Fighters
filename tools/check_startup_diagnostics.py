#!/usr/bin/env python3
"""Exercise media-free diagnostics on a real staged or unpacked executable.

Uses subprocess waiting on all platforms, including the Windows GUI subsystem.
This does not prove desktop shortcut, Finder, dialogs, GPU or OS crash handling.
"""
import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def check(binary, directory):
    directory.mkdir(parents=True, exist_ok=True)
    profile = directory / "isolated profile with spaces"
    logs = directory / "logs with spaces"
    profile.mkdir()
    logs.mkdir()
    env = os.environ.copy()
    env.update(TORE_DATA_DIR=str(profile), TORE_LOG_DIR=str(logs), TORE_NO_ERROR_DIALOG="1")
    env.pop("RUST_BACKTRACE", None)
    last_error = profile / "last-error.txt"

    def launch(mode):
        arg = "--diagnostics-self-test" + (f"={mode}" if mode else "")
        result = subprocess.run([str(binary), arg], cwd=directory, env=env,
                                capture_output=True, text=True, errors="replace", timeout=90)
        require((result.returncode == 0) == (not mode),
                f"{arg}: unexpected exit {result.returncode}\n{result.stdout}\n{result.stderr}")

    launch("")
    require(not last_error.exists(), "successful fresh launch wrote last-error.txt")
    for mode in ("error", "panic", "worker-panic"):
        before = {p for p in logs.iterdir() if p.is_file()}
        launch(mode)
        require(last_error.is_file(), f"{mode}: missing last-error.txt")
        report = last_error.read_text(encoding="utf-8")
        lowered = report.lower()
        require("deliberate" in lowered and ("panic" if "panic" in mode else "error") in lowered,
                f"{mode}: report does not identify the deliberate failure: {report}")
        require(str(logs) in report, f"{mode}: report missing actual log location")
        new_files = [p for p in logs.iterdir() if p.is_file() and p not in before]
        require(new_files, f"{mode}: no new session report")
        contents = "\n".join(p.read_text(encoding="utf-8") for p in new_files).lower()
        if "panic" in mode:
            require("backtrace" in contents and "thread" in contents,
                    f"{mode}: logs missing thread or backtrace")
        for field in ("version", "commit", "target"):
            require(field in contents, f"{mode}: logs missing {field} metadata")
        launch("")
        require(last_error.read_text(encoding="utf-8") == report,
                "successful launch replaced or cleared previous failure")
    print(f"Startup diagnostics passed: {binary}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="tore diagnostics check ") as directory:
        check(binary, Path(directory))


if __name__ == "__main__":
    main()
