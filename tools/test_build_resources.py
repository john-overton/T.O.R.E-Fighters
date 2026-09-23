"""Keep Windows resource-layout tests in the documented, cross-platform suite.

Cargo does not execute build.rs unit tests, so compile the build script as a
standalone test harness. It uses only synthetic message bytes, no retail data.
"""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest


class BuildResourceTests(unittest.TestCase):
    def test_windows_event_message_resource_layout(self):
        root = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory(prefix="tore resource tests ") as directory:
            harness = Path(directory) / ("resources.exe" if os.name == "nt" else "resources")
            subprocess.run(
                ["rustc", "--edition=2024", "--test", str(root / "crates/tore-app/build.rs"),
                 "-o", str(harness)],
                cwd=root, check=True, capture_output=True, text=True, timeout=120,
            )
            result = subprocess.run(
                [str(harness)], cwd=root, check=True, capture_output=True,
                text=True, timeout=30,
            )
            self.assertIn("event_message_has_id_1000_and_one_unicode_insertion ... ok", result.stdout)
            self.assertIn("1 passed", result.stdout)
