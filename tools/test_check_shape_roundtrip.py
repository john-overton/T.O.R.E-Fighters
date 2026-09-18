"""Check that the converter probe cannot accept a failed or no-op encoder."""

from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from check_shape_roundtrip import check


class RoundtripTests(unittest.TestCase):
    def run_probe(self, mode):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "donor.SH"
            source.write_bytes(b"synthetic donor")

            def convert(argv, **kwargs):
                path = Path(argv[1])
                if path.suffix == ".SH":
                    path.with_suffix(".SH.yaml").write_bytes(path.read_bytes())
                elif mode != "noop":
                    data = path.read_bytes() if mode == "identical" else b"different"
                    path.with_suffix("").write_bytes(data)
                return subprocess.CompletedProcess(argv, 0, b"", b"")

            with patch("check_shape_roundtrip.subprocess.run", side_effect=convert):
                record = check(root / "fake-tool", source, root / "scratch")
            self.assertEqual(source.read_bytes(), b"synthetic donor")
            return record

    def test_exact_conversion_passes_without_touching_source(self):
        self.assertTrue(self.run_probe("identical")["identical"])

    def test_changed_conversion_fails(self):
        record = self.run_probe("changed")
        self.assertFalse(record["identical"])
        self.assertEqual(record["first_difference"], 0)

    def test_successful_noop_encoder_fails(self):
        record = self.run_probe("noop")
        self.assertFalse(record["identical"])
        self.assertIn("did not create", record["error"])


if __name__ == "__main__":
    unittest.main()
