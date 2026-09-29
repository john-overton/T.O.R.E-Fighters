import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import _replay_checks as rc  # noqa: E402

GOOD_ACMI = """FileType=text/acmi/tacview
FileVersion=2.2
0,ReferenceTime=2026-09-29T09:44:00Z
#0
10000000000,T=0.8|-0.6|1524|0|0|17|326|179|17,Type=Air+FixedWing,Name=F/A-18D
#1
10000000000,T=0.8|-0.6|1500|0|0|17|326|179|17
0,Event=Destroyed|10000000000|gone
-10000000000
"""


class AcmiTests(unittest.TestCase):
    def test_good_file_passes(self):
        self.assertEqual(rc.check_acmi(GOOD_ACMI), [])

    def test_header_time_and_lifetime_problems_are_found(self):
        self.assertTrue(rc.check_acmi("nonsense\n"))
        self.assertTrue(rc.check_acmi(GOOD_ACMI.replace("#1\n", "#-1\n#0.5\n#0.4\n")))
        self.assertTrue(rc.check_acmi(GOOD_ACMI + "10000000000,T=0|0|0|0|0|0|0|0|0\n"))
        self.assertTrue(rc.check_acmi(GOOD_ACMI.replace("1500", "nan")))
        self.assertTrue(rc.check_acmi(GOOD_ACMI.replace("-10000000000\n", "-10000000009\n")))
        self.assertTrue(rc.check_acmi(GOOD_ACMI.split("#0\n")[0]))


class JsonlTests(unittest.TestCase):
    def test_lines_must_be_json_with_one_header(self):
        header = '{"type":"header","format":1}\n{"type":"aircraft","id":0}\n'
        self.assertEqual(rc.check_jsonl(header + '{"type":"sample","t":0}\n{"type":"sample","t":1}\n'), [])
        self.assertTrue(rc.check_jsonl(header + "{broken\n"))
        self.assertTrue(rc.check_jsonl(header + '{"type":"sample","t":2}\n{"type":"sample","t":1}\n'))
        self.assertTrue(rc.check_jsonl(header + '{"type":"aircraft","id":0}\n'))
        self.assertTrue(rc.check_jsonl(header + '{"type":"sample","t":NaN}\n'))


class SummaryTests(unittest.TestCase):
    def test_sections_are_required(self):
        text = "T.O.R.E mission summary\n\nAircraft\n--------\nYou (F/A-18D, F18.PT; friendly wing 1-1; Human)\n\nTimeline\n--------\n"
        self.assertEqual(rc.check_summary(text, expect_aircraft=1), [])
        self.assertTrue(rc.check_summary(text, expect_aircraft=2))
        self.assertTrue(rc.check_summary("nothing"))


if __name__ == "__main__":
    unittest.main()
