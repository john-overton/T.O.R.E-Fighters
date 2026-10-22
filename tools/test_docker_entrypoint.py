"""Container stops must reach the server's graceful console shutdown."""
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import tempfile
import unittest


@unittest.skipUnless(os.name == "posix" and shutil.which("nice"), "POSIX container entrypoint")
class DockerEntrypointTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        binary = Path(self.directory.name) / "tore-server"
        binary.write_text(
            '#!/bin/sh\n'
            'if [ "${1:-}" = "--fail" ]; then exit 7; fi\n'
            'echo READY\n'
            'while IFS= read -r command; do\n'
            '  if [ "$command" = quit ]; then echo CLEANUP; exit 0; fi\n'
            '  echo "console: $command"\n'
            'done\n'
            'exit 9\n'
        )
        binary.chmod(0o755)
        self.environment = dict(os.environ, PATH=self.directory.name + os.pathsep + os.environ["PATH"])
        self.entrypoint = Path(__file__).resolve().parent / "docker-entrypoint.sh"

    def launch(self, *args, stdin=subprocess.PIPE):
        process = subprocess.Popen(
            ["sh", str(self.entrypoint), *args], env=self.environment,
            stdin=stdin, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        self.addCleanup(self.reap, process)
        return process

    @staticmethod
    def reap(process):
        if process.poll() is None:
            process.terminate()
        process.communicate(timeout=5)

    def ready(self, process):
        self.assertTrue(select.select([process.stdout], [], [], 5)[0], "server did not start")
        self.assertEqual(process.stdout.readline(), "READY\n")

    def test_stop_without_attached_stdin_runs_cleanup(self):
        process = self.launch(stdin=subprocess.DEVNULL)
        self.ready(process)
        process.send_signal(signal.SIGTERM)
        output, errors = process.communicate(timeout=5)
        self.assertEqual(process.returncode, 0, errors)
        self.assertIn("CLEANUP", output)

    def test_attached_console_and_quit(self):
        process = self.launch()
        self.ready(process)
        output, errors = process.communicate("status\nquit\n", timeout=5)
        self.assertEqual(process.returncode, 0, errors)
        self.assertIn("console: status", output)
        self.assertIn("CLEANUP", output)

    def test_startup_failure_preserves_exit_status(self):
        process = self.launch("--fail")
        process.communicate(timeout=5)
        self.assertEqual(process.returncode, 7)
