"""Guard against mistaking a motion-only report for animation acceptance."""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios.animation import check_report, scenarios


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


    def test_rotor_acceptance_requires_complete_combined_sweep(self):
        report = self.report()
        report["aircraft"] = "CH47.PT"
        self.assertTrue(self.check(report))
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "poses").mkdir()
            (work / "poses/report.json").write_text(json.dumps(report))
            combined = dict(poses=6000, checks_passed=True)
            target = work / "poses/rotor-combinations.json"
            target.write_text(json.dumps(combined))
            self.assertEqual(check_report(work, ""), [])
            combined["poses"] = 5999
            target.write_text(json.dumps(combined))
            self.assertTrue(check_report(work, ""))

    def test_mixed_flaperons_require_all_twenty_five_source_poses(self):
        report = self.report()
        report["aircraft"] = "E3.PT"
        self.assertTrue(self.check(report))

    def test_v22_requires_both_independent_combination_artifacts(self):
        report = self.report()
        report["aircraft"] = "V22.PT"
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            poses = work / "poses"
            poses.mkdir()
            (poses / "report.json").write_text(json.dumps(report))
            (poses / "flaperon-combinations.csv").write_text("pose\n" * 26)
            self.assertTrue(check_report(work, ""))
            (poses / "conversion-rotor-combinations.csv").write_text("pose\n" * 26)
            self.assertEqual(check_report(work, ""), [])

    def test_rafale_requires_all_three_axis_combinations(self):
        report = self.report()
        report["aircraft"] = "RAFALE.PT"
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            poses = work / "poses"
            poses.mkdir()
            (poses / "report.json").write_text(json.dumps(report))
            target = poses / "flap-pitch-roll-combinations.csv"
            target.write_text("pose\n" * 26)
            self.assertTrue(check_report(work, ""))
            target.write_text("pose\n" * 126)
            self.assertEqual(check_report(work, ""), [])

    def test_concept_selection_requires_its_own_split_sweep(self):
        report = self.report()
        report["aircraft"] = "F22N.PT"
        report["selection_key"] = "faxx"
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            poses = work / "poses"
            poses.mkdir()
            (poses / "report.json").write_text(json.dumps(report))
            (poses / "pitch-roll-combinations.csv").write_text("pose\n" * 26)
            self.assertTrue(check_report(work, ""))
            (poses / "flap-yaw-combinations.csv").write_text("pose\n" * 26)
            self.assertEqual(check_report(work, ""), [])

    def test_scenario_output_requires_exact_identity_and_zero_failures(self):
        pattern = scenarios()[0].expect[0]
        self.assertRegex("animation probe A7.PT: 261 neutral faces, 0 required/check failures; poses", pattern)
        self.assertNotRegex("animation probe A7.PT: 261 neutral faces, 10 required/check failures; poses", pattern)
        self.assertNotRegex("animation probe F18.PT: 261 neutral faces, 0 required/check failures; poses", pattern)
