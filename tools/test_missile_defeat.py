"""The battery must reject silent or damaging defeat runs, not merely finish."""
import tempfile
import unittest
from pathlib import Path

from battery_scenarios.flight import check_missile_defeat


class DefeatBatteryTests(unittest.TestCase):
    def check_rows(self, rows, serial=None):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, data in [("defeat", rows), ("defeat-serial", rows if serial is None else serial)]:
                (root / name).mkdir()
                (root / name / "runs.csv").write_text("hit,defeat_s\n" + data)
            return check_missile_defeat(root)

    def test_requires_an_observed_inert_defeat(self):
        self.assertEqual(self.check_rows("0,12.5\n1,\n"), [])
        self.assertTrue(self.check_rows("0,\n1,\n"))
        self.assertTrue(self.check_rows("1,12.5\n"))

    def test_rejects_worker_dependent_results(self):
        self.assertTrue(self.check_rows("0,12.5\n", "0,12.6\n"))


if __name__ == "__main__":
    unittest.main()
