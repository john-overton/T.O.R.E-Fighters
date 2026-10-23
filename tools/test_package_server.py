"""Tests for tools/package/package_server.py, the dedicated server's archive."""

import hashlib
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent / "package"))

from package_server import (  # noqa: E402
    DOCUMENTS,
    ROOT,
    archive_kind,
    members,
    platform_name,
    resolve_version,
    stage,
    unpack,
    write_archive,
    write_checksum,
)


class VersionTests(unittest.TestCase):
    def test_order_matches_version_sh(self):
        tag = {"GITHUB_REF_TYPE": "tag", "GITHUB_REF_NAME": "v0.2.0"}
        self.assertEqual(resolve_version("v0.3.0", tag, lambda: "v9"), "0.3.0")
        self.assertEqual(resolve_version("", tag, lambda: "v9"), "0.2.0")
        branch = {"GITHUB_REF_TYPE": "branch", "GITHUB_REF_NAME": "release-test/x"}
        self.assertEqual(resolve_version("", branch, lambda: "v0.1.3-5-gabc"), "0.1.3-5-gabc")
        self.assertEqual(resolve_version("", {}, lambda: None), "0.0.0-dev")


class NameTests(unittest.TestCase):
    def test_platform_names_follow_the_game_packages(self):
        self.assertEqual(platform_name("linux", "x86_64", ""), "linux-x86_64")
        self.assertEqual(platform_name("darwin", "arm64", ""), "macos-arm64")
        self.assertEqual(platform_name("darwin", "x86_64", ""), "macos-x86_64")
        self.assertEqual(platform_name("win32", "AMD64", ""), "windows-x86_64")
        self.assertEqual(platform_name("win32", "AMD64", "x86_64-pc-windows-msvc"), "windows-x86_64")
        self.assertEqual(platform_name("win32", "AMD64", "i686-pc-windows-msvc"), "windows-x86")

    def test_windows_gets_a_zip_and_the_rest_a_tarball(self):
        self.assertEqual(archive_kind("win32"), "zip")
        self.assertEqual(archive_kind("linux"), "tar.gz")
        self.assertEqual(archive_kind("darwin"), "tar.gz")

    def test_every_document_is_committed(self):
        for source, _ in DOCUMENTS:
            self.assertTrue((ROOT / source).is_file(), source)


class ArchiveTests(unittest.TestCase):
    def check(self, kind, executable):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            binary = tmp / "built"
            binary.write_bytes(b"\x7fELF" + bytes(16))
            stage_dir = tmp / "stage" / "tore-server-1.2.3-test"
            stage(stage_dir, binary, executable)
            archive = tmp / f"tore-server-1.2.3-test.{kind}"
            write_archive(stage_dir, archive, kind, executable)
            expected = sorted(
                f"tore-server-1.2.3-test/{name}"
                for name in [executable] + [name for _, name in DOCUMENTS]
            )
            self.assertEqual(members(archive, kind), expected)
            unpacked = tmp / "check with spaces"
            unpack(archive, kind, unpacked)
            server = unpacked / "tore-server-1.2.3-test" / executable
            self.assertEqual(server.read_bytes(), binary.read_bytes())
            if kind == "tar.gz" and os.name != "nt":
                self.assertTrue(os.access(server, os.X_OK))
                self.assertFalse(os.access(unpacked / "tore-server-1.2.3-test" / "LICENSE", os.X_OK))
            checksum = write_checksum(archive)
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            self.assertEqual(checksum.read_bytes(), f"{digest}  {archive.name}\n".encode())

    def test_tarball(self):
        self.check("tar.gz", "tore-server")
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            binary = tmp / "built"
            binary.write_bytes(b"x")
            stage_dir = tmp / "tore-server-0-linux"
            stage(stage_dir, binary, "tore-server")
            archive = tmp / "a.tar.gz"
            write_archive(stage_dir, archive, "tar.gz", "tore-server")
            with tarfile.open(archive) as tar:
                for member in tar.getmembers():
                    self.assertEqual((member.uid, member.gid, member.uname, member.gname), (0, 0, "", ""))

    def test_zip(self):
        self.check("zip", "tore-server.exe")
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            binary = tmp / "built"
            binary.write_bytes(b"MZ")
            stage_dir = tmp / "tore-server-0-windows-x86_64"
            stage(stage_dir, binary, "tore-server.exe")
            archive = tmp / "a.zip"
            write_archive(stage_dir, archive, "zip", "tore-server.exe")
            with zipfile.ZipFile(archive) as zipped:
                modes = {info.filename.split("/")[-1]: (info.external_attr >> 16) & 0o777
                         for info in zipped.infolist()}
            self.assertEqual(modes["tore-server.exe"], 0o755)
            self.assertEqual(modes["server.conf"], 0o644)


if __name__ == "__main__":
    unittest.main()
