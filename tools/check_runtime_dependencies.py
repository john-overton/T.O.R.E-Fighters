#!/usr/bin/env python3
"""Fail packaging when native runtime dependencies violate the release policy."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


I386 = 0x014C  # IMAGE_FILE_MACHINE_I386, a 32-bit Windows executable


def pe_machine(binary):
    """The machine field of a Windows executable's header, or None."""
    try:
        with open(binary, "rb") as file:
            dos = file.read(0x40)
            if len(dos) < 0x40 or dos[:2] != b"MZ":
                return None
            file.seek(int.from_bytes(dos[0x3C:0x40], "little"))
            header = file.read(6)
    except OSError:
        return None
    if len(header) < 6 or header[:4] != b"PE\0\0":
        return None
    return int.from_bytes(header[4:6], "little")


def glibc_versions(binary):
    """The glibc symbol versions an ELF binary asks for, as sorted tuples."""
    data = Path(binary).read_bytes()
    found = {tuple(int(part) for part in match.split(b"."))
             for match in re.findall(rb"GLIBC_(\d+(?:\.\d+)+)", data)}
    return sorted(found)


def violations(platform, output, binary, system_root=None, max_glibc=None):
    if platform == "win32":
        dependencies = re.findall(r"^\s*([\w.+-]+\.dll)\s*$", output, re.M | re.I)
        if not dependencies:
            return ["dumpbin reported no DLL dependencies"]
        root = Path(system_root or os.environ["SystemRoot"])
        # A 32-bit program on 64-bit Windows loads system DLLs from SysWOW64.
        system = root / "System32"
        if pe_machine(binary) == I386 and (root / "SysWOW64").is_dir():
            system = root / "SysWOW64"
        errors = []
        for name in dependencies:
            lower = name.lower()
            if lower.startswith(("vcruntime", "msvcp", "concrt", "vcomp")):
                errors.append(f"dynamic Visual C++ runtime forbidden: {name}")
            elif lower.startswith(("api-ms-win-", "ext-ms-win-")):
                continue  # Windows API set contracts are not physical DLLs.
            elif not ((binary.parent / name).exists() or (system / name).exists()):
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
    # The binary uses the player's own C library, so a build host with a newer
    # glibc than the oldest supported distribution produces a program that
    # exits at load time there.
    if max_glibc:
        ceiling = tuple(int(part) for part in max_glibc.split("."))
        newer = [version for version in glibc_versions(binary) if version > ceiling]
        if newer:
            wanted = ".".join(str(part) for part in newer[-1])
            return [f"requires glibc {wanted}, newer than the {max_glibc} release floor"]
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
    parser.add_argument("--max-glibc", metavar="VERSION",
                        help="Linux only: fail when a binary needs a newer glibc, e.g. 2.35")
    args = parser.parse_args()
    command = [dumpbin(), "-dependents"] if sys.platform == "win32" else (["otool", "-L"] if sys.platform == "darwin" else ["ldd"])
    for binary in args.binaries:
        binary = binary.resolve(strict=True)
        result = subprocess.run([*command, str(binary)], capture_output=True, text=True, check=True)
        print(result.stdout)
        errors = violations(sys.platform, result.stdout, binary, max_glibc=args.max_glibc)
        if errors:
            raise RuntimeError(f"{binary}: " + "; ".join(errors))
    print("Runtime dependencies passed.")


if __name__ == "__main__":
    main()
