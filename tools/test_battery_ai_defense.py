"""The missile defense regression must reject the original weak turn and push."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from battery_scenarios.ai_defense import defense_problems, head_break_problems


class DefenseChecks(unittest.TestCase):
    def check_case(self, g=3.0, turn=60, wrong=0, headings=(270, 271, 272, 273)):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "study").mkdir()
            (work / "study/runs.csv").write_text(
                f"fired,target_turn_deg,other_devices\n1,{turn},{wrong}\n")
            trace = "\n".join(
                f"missile-study: trace AIM9M seed 0 g={g} alt=5000 activity=Defending "
                f"defense=Some((Some(MotionSuggestion {{ maneuver: Beam, heading_deg: {h}"
                for h in headings)
            return defense_problems(work, trace)

    def test_positive_turn_passes(self):
        self.assertEqual(self.check_case(), [])

    def test_original_push_weak_turn_and_mixed_burst_fail(self):
        self.assertEqual(len(self.check_case(g=-1.5, turn=7, wrong=4)), 3)

    def test_notch_flip_fails(self):
        self.assertIn("beam/notch side flipped", self.check_case(headings=(270, 271, 90, 89)))


class HeadOnChecks(unittest.TestCase):
    def test_a_straight_command_cannot_count_as_a_head_on_break(self):
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp)
            (work / "study").mkdir()
            (work / "study/runs.csv").write_text("fired,target_turn_deg,other_devices\n1,10,0\n")
            trace = "\n".join(
                "missile-study: trace AIM9M seed 0 yaw=0 g=3 alt=5000 activity=Defending "
                "defense=Some((Some(MotionSuggestion { maneuver: Break, heading_deg: 0"
                for _ in range(4))
            self.assertEqual(head_break_problems(work, trace), ["head-on break straightened toward the missile"])
            self.assertEqual(head_break_problems(work, trace.replace("heading_deg: 0", "heading_deg: 90")), [])
