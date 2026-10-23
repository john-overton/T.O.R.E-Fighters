#!/usr/bin/env python3
"""Build the dedicated server's own archive for this platform.

    cargo build --release --locked -p tore-server
    python3 tools/package/package_server.py [--version 0.2.0] [--target TRIPLE] [--max-glibc 2.35]

The archive is tore-server-<version>-<platform>.tar.gz on Linux and macOS and
tore-server-<version>-windows-x86_64.zip on Windows (windows-x86 for an
i686-pc-windows-msvc build), with <archive>.sha256 beside it in sha256sum's
format. Both land in dist/server/, apart from the game's packages in dist/.
The platform names follow the game's packages: linux-x86_64, macos-arm64,
macos-x86_64.

The archive holds one folder, named like the archive, with the server
(tore-server or tore-server.exe), DEDICATED-SERVER.md (the server guide),
LICENSE, THIRD_PARTY_NOTICES.md, and the samples server.conf and mission.txt
from crates/tore-server/samples/. The game is not in it: an operator needs
only the server and their own Fighters Anthology import.

Before it writes the archive the script scans the staged folder for retail
data (tools/check_assets.py), checks the server's runtime libraries
(tools/check_runtime_dependencies.py, with the glibc ceiling --max-glibc
gives) and runs `tore-server --version`. Afterwards it scans the archive,
unpacks it into a folder whose name has spaces and runs `--version` again from
there. When TORE_BUILD_VERSION is set, the version line must carry it, which
proves the build was stamped. On macOS the staged server is ad-hoc signed and
verified, then verified again after unpacking. No Developer ID or notarization
is used; Windows and Linux packages remain unsigned.

The version is --version, then the tag the release workflow is running for,
then `git describe`, then 0.0.0-dev, with a leading "v" stripped: the same
order as tools/package/version.sh.
"""

import argparse
import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import zipfile


ROOT = Path(__file__).resolve().parents[2]

# (source relative to the repository, name in the archive)
DOCUMENTS = [
    ("docs/DEDICATED-SERVER.md", "DEDICATED-SERVER.md"),
    ("LICENSE", "LICENSE"),
    ("THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"),
    ("crates/tore-server/samples/server.conf", "server.conf"),
    ("crates/tore-server/samples/mission.txt", "mission.txt"),
]


def resolve_version(explicit, environ, describe):
    """The package version, in version.sh's order. `describe` is a callable
    giving `git describe` output or None."""
    version = explicit or ""
    if not version and environ.get("GITHUB_REF_TYPE") == "tag":
        version = environ.get("GITHUB_REF_NAME", "")
    if not version:
        version = describe() or ""
    version = version.strip()
    if version.startswith("v"):
        version = version[1:]
    return version or "0.0.0-dev"


def git_describe():
    try:
        result = subprocess.run(
            ["git", "-C", str(ROOT), "describe", "--tags", "--always", "--dirty"],
            capture_output=True, text=True, check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    return result.stdout.strip() or None


def platform_name(system, machine, target):
    """The archive's platform suffix, as the game's packages name theirs."""
    if system == "win32":
        if target and target.startswith("i686-"):
            return "windows-x86"
        return "windows-x86_64"
    if system == "darwin":
        # uname -m, as package-macos.sh names the DMG: arm64 or x86_64.
        return f"macos-{machine}"
    return f"linux-{machine}"


def archive_kind(system):
    return "zip" if system == "win32" else "tar.gz"


def stage(stage_dir, binary, executable_name):
    """Copy the server and its documents into a fresh stage folder."""
    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    stage_dir.mkdir(parents=True)
    shutil.copyfile(binary, stage_dir / executable_name)
    os.chmod(stage_dir / executable_name, 0o755)
    for source, name in DOCUMENTS:
        shutil.copyfile(ROOT / source, stage_dir / name)
        os.chmod(stage_dir / name, 0o644)


def _tar_member(info):
    # The packager's own user and group mean nothing on the operator's machine.
    info.uid = info.gid = 0
    info.uname = info.gname = ""
    if info.isdir():
        info.mode = 0o755
    elif info.isfile():
        info.mode = 0o755 if info.mode & 0o100 else 0o644
    return info


def write_archive(stage_dir, archive, kind, executable):
    """Write the stage folder, under its own name, into a tar.gz or a zip.
    `executable` is the one member marked executable in a zip."""
    archive.unlink(missing_ok=True)
    if kind == "tar.gz":
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(stage_dir, arcname=stage_dir.name, filter=_tar_member)
        return
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as zipped:
        for path in sorted(stage_dir.rglob("*")):
            if not path.is_file():
                continue
            name = f"{stage_dir.name}/{path.relative_to(stage_dir).as_posix()}"
            info = zipfile.ZipInfo.from_file(path, name)
            info.compress_type = zipfile.ZIP_DEFLATED
            # Windows has no mode bits; this keeps the server runnable when
            # the zip is unpacked by a Unix tool.
            mode = 0o755 if path.name == executable else 0o644
            info.external_attr = (0o100000 | mode) << 16
            zipped.writestr(info, path.read_bytes())


def members(archive, kind):
    """The archive's file members, sorted, for the listing and the tests."""
    if kind == "tar.gz":
        with tarfile.open(archive, "r:gz") as tar:
            return sorted(member.name for member in tar.getmembers() if member.isfile())
    with zipfile.ZipFile(archive) as zipped:
        return sorted(info.filename for info in zipped.infolist() if not info.is_dir())


def unpack(archive, kind, into):
    if into.exists():
        shutil.rmtree(into)
    into.mkdir(parents=True)
    if kind == "tar.gz":
        with tarfile.open(archive, "r:gz") as tar:
            tar.extractall(into, filter="data")
    else:
        with zipfile.ZipFile(archive) as zipped:
            zipped.extractall(into)


def write_checksum(archive):
    """<archive>.sha256 in sha256sum's format, so `sha256sum -c` checks it."""
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum = archive.with_name(archive.name + ".sha256")
    with open(checksum, "w", encoding="ascii", newline="\n") as file:
        file.write(f"{digest}  {archive.name}\n")
    return checksum


def run_tool(script, *arguments):
    command = [sys.executable, str(ROOT / "tools" / script), *[str(a) for a in arguments]]
    if subprocess.run(command).returncode != 0:
        raise SystemExit(f"{script} failed on {' '.join(str(a) for a in arguments)}")


def smoke(binary, stamped):
    """Run `--version`: it must start and say it is the server, with the
    stamped version when the build was stamped."""
    result = subprocess.run([str(binary), "--version"], capture_output=True, text=True, timeout=60)
    line = (result.stdout or result.stderr).strip()
    print(f"{binary.name} --version: {line}")
    if result.returncode != 0:
        raise SystemExit(f"{binary} --version exited with {result.returncode}")
    # `tore-server --version` prints "T.O.R.E server - v<version> (code ...,
    # commit ...)" since the compatibility code (0.2.1).
    expected = f"T.O.R.E server - v{stamped} (" if stamped else "T.O.R.E server - v"
    if not line.startswith(expected):
        raise SystemExit(f"{binary} --version printed {line!r}, expected it to start with {expected!r}")


def verify_macos_signature(binary):
    """Fail packaging on an invalid signature and show its details in CI."""
    subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(binary)], check=True)
    subprocess.run(["codesign", "-dv", "--verbose=4", str(binary)], check=True)


def main():
    # The checks print through their own processes; keep this script's lines
    # in order with theirs in a CI log.
    sys.stdout.reconfigure(line_buffering=True)
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--version", default="", help="Package version; default the tag, then git describe")
    parser.add_argument("--target", default=os.environ.get("CARGO_BUILD_TARGET", ""),
                        help="Rust target the server was built for; default CARGO_BUILD_TARGET")
    parser.add_argument("--max-glibc", metavar="VERSION",
                        help="Linux only: fail when the server needs a newer glibc, e.g. 2.35")
    args = parser.parse_args()

    version = resolve_version(args.version, os.environ, git_describe)
    system = sys.platform
    name = f"tore-server-{version}-{platform_name(system, platform.machine(), args.target)}"
    kind = archive_kind(system)
    executable = "tore-server.exe" if system == "win32" else "tore-server"
    release = ROOT / "target" / args.target / "release" if args.target else ROOT / "target" / "release"
    binary = release / executable
    if not binary.is_file():
        build = "cargo build --release --locked -p tore-server" + (f" --target {args.target}" if args.target else "")
        raise SystemExit(f"Missing {binary}. Run: {build}")
    stamped = os.environ.get("TORE_BUILD_VERSION", "")
    dist = ROOT / "dist" / "server"
    stage_dir = ROOT / "dist" / "stage" / "server" / name

    print(f"Staging {name}")
    stage(stage_dir, binary, executable)
    print("Checking the staged folder for retail data")
    run_tool("check_assets.py", stage_dir)
    glibc = ["--max-glibc", args.max_glibc] if args.max_glibc else []
    run_tool("check_runtime_dependencies.py", *glibc, stage_dir / executable)
    if system == "darwin":
        # A standalone Mach-O needs no bundle seal. Sign explicitly on both
        # architectures instead of relying on the linker's signing defaults.
        # No hardened runtime, Developer ID or notarization. No later step
        # may modify the staged executable.
        print("Ad-hoc signing the staged macOS server")
        subprocess.run(["codesign", "--force", "--sign", "-", str(stage_dir / executable)], check=True)
        verify_macos_signature(stage_dir / executable)
    smoke(stage_dir / executable, stamped)

    dist.mkdir(parents=True, exist_ok=True)
    archive = dist / f"{name}.{kind}"
    write_archive(stage_dir, archive, kind, executable)
    print(f"Wrote {archive}")
    for member in members(archive, kind):
        print(f"  {member}")
    run_tool("check_assets.py", archive)
    unpacked = ROOT / "dist" / "stage" / "server" / "server check with spaces"
    unpack(archive, kind, unpacked)
    if system == "darwin":
        print("Verifying the macOS server from the finished archive")
        verify_macos_signature(unpacked / name / executable)
    smoke(unpacked / name / executable, stamped)
    checksum = write_checksum(archive)
    print(f"Wrote {checksum}: {checksum.read_text().strip()}")
    print(f"Dedicated server packaging complete for version {version}")


if __name__ == "__main__":
    main()
