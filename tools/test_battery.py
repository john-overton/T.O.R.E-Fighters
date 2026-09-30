import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import battery  # noqa: E402


def scenario(**kw):
    return battery.Scenario(name="t", lane="ai", args=[], **kw)


class JudgeTests(unittest.TestCase):
    def judge(self, s, output, code=0, timed_out=False):
        with tempfile.TemporaryDirectory() as d:
            return battery.judge(s, output, code, timed_out, Path(d))

    def test_clean_run_passes(self):
        self.assertEqual(self.judge(scenario(), "all fine\ninfo: done\n"), [])

    def test_exit_code_and_timeout(self):
        self.assertTrue(self.judge(scenario(), "", code=101))
        self.assertIn("timed out", self.judge(scenario(timeout=5), "", code=None, timed_out=True)[0])

    def test_generic_problems(self):
        self.assertTrue(self.judge(scenario(), "thread 'main' panicked at src/x.rs:1"))
        self.assertTrue(self.judge(scenario(), "speed=NaN"))
        self.assertTrue(self.judge(scenario(), "x=inf y=1"))
        self.assertEqual(self.judge(scenario(allow_generic=["NaN in output"]), "speed=NaN"), [])

    def test_expect_forbid_check(self):
        self.assertTrue(self.judge(scenario(expect=[r"totals"]), "nothing"))
        self.assertEqual(self.judge(scenario(expect=[r"totals"]), "totals: 1"), [])
        self.assertTrue(self.judge(scenario(forbid=[r"dropped=[1-9]"]), "dropped=3"))
        self.assertEqual(self.judge(scenario(check=lambda o: ["custom"]), "x"), ["custom"])

    def test_check_work_sees_the_work_folder_and_output(self):
        with tempfile.TemporaryDirectory() as d:
            (Path(d) / "made.txt").write_text("x")
            s = scenario(check_work=lambda work, out: [] if (work / "made.txt").exists() and "ok" in out else ["bad"])
            self.assertEqual(battery.judge(s, "ok", 0, False, Path(d)), [])
            self.assertEqual(battery.judge(s, "no", 0, False, Path(d)), ["bad"])

    def test_follow_up_steps_append_their_output_and_check_exit_codes(self):
        import argparse

        with tempfile.TemporaryDirectory() as d:
            # The running Python stands in for the game and for other commands, so this runs on every platform.
            opts = argparse.Namespace(bin=sys.executable, timeout_scale=1.0)
            fail = [sys.executable, "-c", "raise SystemExit(1)"]
            steps = [
                battery.Step(["-c", "print('one')"]),
                battery.Step(fail, app=False, expect_exit=1),
                battery.Step(fail, app=False, expect_exit=None),
                battery.Step(fail, app=False),
            ]
            problems: list[str] = []
            out = battery.run_steps(scenario(then=steps), opts, {}, Path(d), "main", problems)
            self.assertIn(f"$ then 1: {sys.executable} -c print('one')", out)
            self.assertIn("one", out)
            self.assertEqual(problems, ["step 4 exit code 1, expected 0"])

    def test_env_values_can_name_the_work_folder_and_known_failures_are_flagged(self):
        import argparse
        import shutil

        with tempfile.TemporaryDirectory() as d:
            profile = Path(d) / "profile"
            profile.mkdir()
            opts = argparse.Namespace(
                bin=sys.executable, profile=str(profile), keep_data=True, timeout_scale=1.0, out=d,
            )
            run_dir = Path(d) / "run"
            slots = __import__("threading").Semaphore(1)
            sc = battery.Scenario(
                name="t", lane="ai", args=["-c", "import os; print(os.environ['MARK']); raise SystemExit(3)"], env={"MARK": "{work}/x"}, expect_exit=0,
                known_failure="on purpose",
            )
            result = battery.run_one(sc, opts, run_dir, slots)
            self.assertTrue(result.ok)
            self.assertIn("known failure (on purpose)", result.problems[0])
            log = (run_dir / result.log).read_text()
            self.assertIn(str(run_dir / "work" / "t") + "/x", log)
            sc2 = battery.Scenario(name="t2", lane="ai", args=["-c", "pass"], known_failure="on purpose")
            result2 = battery.run_one(sc2, opts, run_dir, slots)
            self.assertFalse(result2.ok)
            self.assertIn("now passes", result2.problems[0])
            shutil.rmtree(run_dir, ignore_errors=True)

    def test_scenario_names_are_unique_and_lanes_valid(self):
        names = [s.name for s in battery.load_scenarios()]
        self.assertEqual(len(names), len(set(names)))


if __name__ == "__main__":
    unittest.main()
