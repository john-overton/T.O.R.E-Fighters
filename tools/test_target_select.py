"""The live targeting checks must reject selections the host drops."""
import unittest
from battery_scenarios.target_select import selection_problems


class TargetSelectionChecks(unittest.TestCase):
    def test_accepts_retention_and_rejects_a_host_drop_or_dead_cycle(self):
        text = "\n".join([
            "Target selection: radar before=None requested=Some(DesignateTarget(1))",
            "Target selection: radar before=Some(1) requested=Some(DesignateTarget(2))",
            "Target selection: radar before=Some(2) requested=Some(DesignateTarget(1))",
            "Target selection: radar before=None requested=None",
            "Target selection: visual centre=false before=None requested=Some(1)",
            "Target selection: visual centre=false before=Some(1) requested=Some(2)",
            "Target selection: visual centre=true before=Some(2) requested=Some(3)",
            "Target selection: visual centre=false before=Some(3) requested=Some(1)",
        ])
        self.assertEqual(selection_problems(text), [])
        one_lost = text.replace("radar before=Some(1)", "radar before=None")
        self.assertEqual(selection_problems(one_lost), [], "a radar target may leave the scan")
        all_lost = one_lost.replace("radar before=Some(2)", "radar before=None")
        self.assertTrue(selection_problems(all_lost))
        self.assertTrue(selection_problems(text.replace("before=Some(2)", "before=None")))
        self.assertTrue(selection_problems(text.replace("requested=Some(2)", "requested=Some(1)")))
        self.assertTrue(selection_problems(text + "\nTarget selection: visual centre=false before=Some(1) requested=Some(2)"))
