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

    def test_scenario_names_are_unique_and_lanes_valid(self):
        names = [s.name for s in battery.load_scenarios()]
        self.assertEqual(len(names), len(set(names)))


if __name__ == "__main__":
    unittest.main()
