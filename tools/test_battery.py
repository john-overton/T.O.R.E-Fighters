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
            opts = argparse.Namespace(bin="/bin/echo", timeout_scale=1.0)
            steps = [
                battery.Step(["one"]),
                battery.Step(["/bin/false"], app=False, expect_exit=1),
                battery.Step(["/bin/false"], app=False, expect_exit=None),
                battery.Step(["/bin/false"], app=False),
            ]
            problems: list[str] = []
            out = battery.run_steps(scenario(then=steps), opts, {}, Path(d), "main", problems)
            self.assertIn("$ then 1: /bin/echo one", out)
            self.assertIn("one", out)
            self.assertEqual(problems, ["step 4 exit code 1, expected 0"])

    def test_scenario_names_are_unique_and_lanes_valid(self):
        names = [s.name for s in battery.load_scenarios()]
        self.assertEqual(len(names), len(set(names)))


if __name__ == "__main__":
    unittest.main()
