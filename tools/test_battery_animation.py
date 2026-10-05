"""Guard against mistaking a motion-only report for animation acceptance."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios.animation import check_report


class AnimationReportTests(unittest.TestCase):
    def report(self, scope="reviewed-controls-and-attachments", failed=None):
        return dict(validation_scope=scope, controls=[
            dict(control=name, checks_passed=name != failed)
            for name in ("elevator", "aileron", "rudder", "flaps", "gear")
        ])

    def check(self, report):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "poses").mkdir()
            (work / "poses/report.json").write_text(json.dumps(report))
            return check_report(work, "0 required/check failures")

    def test_accepts_only_reviewed_passing_controls(self):
        self.assertEqual(self.check(self.report()), [])
        self.assertTrue(self.check(self.report(scope="motion-survey")))
        self.assertTrue(self.check(self.report(failed="gear")))

    def test_rejects_missing_primary_controls_even_with_zero_exit(self):
        report = self.report()
        report["controls"].pop()
        self.assertTrue(self.check(report))

    def test_rejects_unreadable_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertTrue(check_report(Path(tmp), "0 required/check failures"))
