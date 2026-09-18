"""Synthetic checks of the developer kit's distribution boundary and manifest."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from package_faxx import PREFIX, collect, write_archive


class PackageTests(unittest.TestCase):
    def test_local_media_and_unselected_files_are_excluded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Cargo.toml").write_text("[workspace]\n")
            files = collect(root, ["Cargo.toml", ".local/private.txt", "gameassets/F22.SH",
                                   "USNF-ATF/source.ts", "target/debug/tore-app", ".git/config"])
            self.assertEqual(list(files), ["source/Cargo.toml"])

    def test_retail_and_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "assets").mkdir()
            (root / "assets/test.lib").write_bytes(b"synthetic")
            with self.assertRaises(ValueError):
                collect(root, ["assets/test.lib"])
            try:
                (root / "assets/link").symlink_to(root / "assets/test.lib")
            except OSError:
                return  # Windows may not grant symlink creation.
            with self.assertRaises(ValueError):
                collect(root, ["assets/link"])

    def test_manifest_reproducibility_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            a, b = [Path(directory) / name for name in ("a.zip", "b.zip")]
            files = {"source/Cargo.toml": b"synthetic source"}
            write_archive(a, files, "synthetic-revision")
            write_archive(b, files, "synthetic-revision")
            self.assertEqual(a.read_bytes(), b.read_bytes())
            with zipfile.ZipFile(a) as archive:
                manifest = json.loads(archive.read(f"{PREFIX}/MANIFEST.json"))
                for name, record in manifest["files"].items():
                    data = archive.read(f"{PREFIX}/{name}")
                    self.assertEqual(hashlib.sha256(data).hexdigest(), record["sha256"])
                    self.assertEqual(len(data), record["bytes"])
            with self.assertRaises(FileExistsError):
                write_archive(a, files, "synthetic-revision")


if __name__ == "__main__":
    unittest.main()
