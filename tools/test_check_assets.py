import io
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest

from check_assets import is_tarball, tarball_violations, violation


def synthetic_pic():
    """A 2x2 PIC header with its pixel block. No retail bytes."""
    header = bytearray(64)
    struct.pack_into("<H4I", header, 0, 0, 2, 2, 64, 4)
    return bytes(header) + bytes(4)


def write_tarball(path, entries):
    """Build a .tar.gz from {member name: bytes}."""
    with tarfile.open(path, "w:gz") as archive:
        for name, payload in entries.items():
            info = tarfile.TarInfo(name)
            info.size = len(payload)
            archive.addfile(info, io.BytesIO(payload))


class AssetCheckTests(unittest.TestCase):
    def test_format_documentation_is_allowed(self):
        self.assertIsNone(violation(Path("formats.md"), b"EALIB and PIC formats"))

    def test_archive_embedded_in_binary_is_rejected(self):
        archive = b"EALIB\x01\x00" + struct.pack("<13sBI", b"TEST.TXT", 0, 43)
        archive += struct.pack("<13sBI", b"", 0, 46) + b"abc"
        self.assertIsNotNone(violation(Path("application"), b"prefix\0" + archive))

    def test_decoder_format_constant_is_allowed(self):
        self.assertIsNone(violation(Path("application"), b"\x00EALIB\x00not an archive"))

    def test_renamed_pic_is_rejected(self):
        header = bytearray(64)
        struct.pack_into("<H4I", header, 0, 0, 2, 2, 64, 4)
        self.assertIsNotNone(violation(Path("renamed.bin"), bytes(header) + bytes(4)))
        self.assertIsNotNone(violation(Path("application"), b"prefix" + bytes(header) + bytes(4)))

    def test_asset_extension_is_rejected(self):
        self.assertIsNotNone(violation(Path("MENU.PIC"), b"placeholder"))


class TarballScanTests(unittest.TestCase):
    """The release tar.gz is scanned member by member, not as opaque bytes."""

    def test_recognized_archive_names(self):
        for name in ("release.tar.gz", "release.tgz", "release.tar", "RELEASE.TAR.GZ"):
            self.assertTrue(is_tarball(Path(name)), name)
        for name in ("package.msi", "package.dmg", "package.AppImage", "tore-app"):
            self.assertFalse(is_tarball(Path(name)), name)

    def test_clean_package_passes(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "clean.tar.gz"
            write_tarball(path, {
                "tore/tore-app": b"\x7fELF" + bytes(64),
                "tore/README.md": b"EALIB and PIC are format names\n",
            })
            self.assertEqual(tarball_violations(path), [])

    def test_retail_extension_inside_package_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "dirty.tar.gz"
            write_tarball(path, {"tore/MENU.PIC": b"placeholder"})
            reported = tarball_violations(path)
            self.assertEqual([name for name, _ in reported], ["tore/MENU.PIC"])

    def test_embedded_signature_inside_package_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "dirty.tar.gz"
            write_tarball(path, {"tore/tore-app": b"prefix" + synthetic_pic()})
            reported = tarball_violations(path)
            self.assertEqual(len(reported), 1)
            self.assertIn("PIC", reported[0][1])

    def test_symlink_member_is_reported(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "linked.tar.gz"
            with tarfile.open(path, "w:gz") as archive:
                info = tarfile.TarInfo("tore/Applications")
                info.type = tarfile.SYMTYPE
                info.linkname = "/Applications"
                archive.addfile(info)
            self.assertEqual(
                tarball_violations(path), [("tore/Applications", "expected a regular file")]
            )


if __name__ == "__main__":
    unittest.main()
