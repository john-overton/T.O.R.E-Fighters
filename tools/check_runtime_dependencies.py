#!/usr/bin/env python3
"""Fail packaging when native runtime dependencies violate the release policy."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


def violations(platform, output, binary, system_root=None):
    if platform == "win32":
        dependencies = re.findall(r"^\s*([\w.+-]+\.dll)\s*$", output, re.M | re.I)
        if not dependencies:
            return ["dumpbin reported no DLL dependencies"]
        errors = []
        for name in dependencies:
            lower = name.lower()
            if lower.startswith(("vcruntime", "msvcp", "concrt", "vcomp")):
                errors.append(f"dynamic Visual C++ runtime forbidden: {name}")
            elif lower.startswith(("api-ms-win-", "ext-ms-win-")):
                continue  # Windows API set contracts are not physical DLLs.
            elif not ((binary.parent / name).exists() or
                      (Path(system_root or os.environ["SystemRoot"]) / "System32" / name).exists()):
                errors.append(f"unresolved DLL: {name}")
        return errors
    if platform == "darwin":
        errors = []
        dependencies = re.findall(r"^\s+(.+?) \(compatibility version", output, re.M)
        if not dependencies:
            return ["otool reported no library dependencies"]
        for name in dependencies:
            if not name.startswith(("/usr/lib/", "/System/Library/")):
                errors.append(f"non-system macOS dependency is not bundled: {name}")
        return errors
    if "not found" in output:
        return [line.strip() for line in output.splitlines() if "not found" in line]
    if not re.search(r"=>|ld-linux|statically linked", output):
        return ["ldd did not report recognizable dependencies"]
    return []


def dumpbin():
    found = shutil.which("dumpbin")
    if found:
        return found
    installer = Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Microsoft Visual Studio/Installer/vswhere.exe"
    result = subprocess.run([str(installer), "-latest", "-property", "installationPath"], check=True, capture_output=True, text=True)
    candidates = sorted((Path(result.stdout.strip()) / "VC/Tools/MSVC").glob("*/bin/Hostx64/x64/dumpbin.exe"))
    if not candidates:
        raise RuntimeError("dumpbin not found")
    return str(candidates[-1])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binaries", nargs="+", type=Path)
    args = parser.parse_args()
    command = [dumpbin(), "-dependents"] if sys.platform == "win32" else (["otool", "-L"] if sys.platform == "darwin" else ["ldd"])
    for binary in args.binaries:
        binary = binary.resolve(strict=True)
        result = subprocess.run([*command, str(binary)], capture_output=True, text=True, check=True)
        print(result.stdout)
        errors = violations(sys.platform, result.stdout, binary)
        if errors:
            raise RuntimeError(f"{binary}: " + "; ".join(errors))
    print("Runtime dependencies passed.")


if __name__ == "__main__":
    main()
