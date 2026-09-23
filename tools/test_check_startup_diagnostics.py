import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from check_startup_diagnostics import check


class StartupCheckTests(unittest.TestCase):
    def exercise(self, clear_error=False, metadata=True):
        runs = []

        def fake_run(command, **kwargs):
            env = kwargs["env"]
            self.assertEqual(env["TORE_NO_ERROR_DIALOG"], "1")
            self.assertNotIn("RUST_BACKTRACE", env)
            self.assertIn(" ", env["TORE_DATA_DIR"])
            self.assertTrue(kwargs["timeout"] > 0)
            mode = command[-1].partition("=")[2]
            runs.append(mode)
            logs = Path(env["TORE_LOG_DIR"])
            (logs / f"session-{len(runs)}.log").write_text(
                "version=1 commit=abc target=test Thread: worker Backtrace: test" if metadata else "no identity", encoding="utf-8")
            last_error = Path(env["TORE_DATA_DIR"]) / "last-error.txt"
            if mode:
                last_error.write_text(f"deliberate {mode}\nThread: worker\nBacktrace: test\nSession log: {logs}", encoding="utf-8")
            elif clear_error and last_error.exists():
                last_error.unlink()
            return subprocess.CompletedProcess(command, int(bool(mode)), "", "")

        with tempfile.TemporaryDirectory() as directory:
            with patch("check_startup_diagnostics.subprocess.run", side_effect=fake_run):
                with patch.dict(os.environ, {"RUST_BACKTRACE": "1"}):
                    check(Path("app"), Path(directory))
        return runs

    def test_all_modes_and_waited_success_after_each_failure(self):
        self.assertEqual(self.exercise(), ["", "error", "", "panic", "", "worker-panic", ""])

    def test_missing_metadata_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "metadata"):
            self.exercise(metadata=False)

    def test_erased_previous_error_rejected(self):
        with self.assertRaises((RuntimeError, FileNotFoundError)):
            self.exercise(clear_error=True)
