"""Tests for the AI lane's output checks (tools/battery_scenarios/ai.py)."""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import ai  # noqa: E402

CLEAN = """AI probe: aircraft=F/A-18D actors=3 ticks=7200 enemy_skill=None mission=free
t=0 (0.0s) Enemy 1-1 F18: alive=true agl=5000 kt=450 x=1 z=2 hdg=197 terrain_agl=5000
t=1289 (10.7s) destroyed: Enemy 1-1
t=1289 (10.7s) Enemy 1-1 F18: alive=false agl=4185 kt=430 x=1 z=2 hdg=199 terrain_agl=4185
actor=1 Friendly 1-2 F18 activity=In formation alive=true rounds=582 x=1.0 y=2.0 z=3.0 hdg=4.0
actor=2 Enemy 1-1 F18 activity=Destroyed alive=false rounds=582 x=1.0 y=2.0 z=3.0 hdg=4.0
AI probe debrief: FAILURE [Destroy { destroyed: 1, total: 2 }] elapsed=60s player[Alive damage=0% kills=[1, 0, 0, 0, 0, 0, 0, 0, 0, 0] ff=0 a2a=0/1
AI probe radio: calls=2 heard=2
  5.5s YOU: 'Fox one' ["^FOXONE"]
  10.8s YOU: 'Splash' ["^SPLASH"]
AI probe attack: clicks=1 steps=1 presses=1 missiles=1 gun_bursts=0 gun_rounds=0 player_hits=1 player_kills=1 hits=1 destroyed=1 lost_friendly=0/1 lost_enemy=1/2 player_alive=true player_damaged=0 ejections=0 perceived=1
AI probe totals: wings=3 ticks=7200 shots=3 dropped=0 warnings=0 live_projectiles=0 player_hp=232 target_hp=[1] checksum=0
AI probe invariants: actors=3 samples=7200 anomalies=0 peak_heading_rate=1.0 peak_bank_rate=1.0 peak_kt=450 peak_alt_ft=5000 kinds=[]
"""


class ProbeCheckTests(unittest.TestCase):
    def test_clean_output_passes(self):
        self.assertEqual(ai.probe_problems(CLEAN), [])

    def test_missing_invariants_line(self):
        text = CLEAN.replace("AI probe invariants:", "AI probe other:")
        self.assertTrue(any("invariants" in p for p in ai.probe_problems(text)))

    def test_anomaly_lines_and_allowances(self):
        text = CLEAN + "AI probe anomaly: t=5 (0.0s) outside the world: Enemy 1-1 x=-5 z=1\n"
        self.assertTrue(ai.probe_problems(text))
        self.assertEqual(ai.probe_problems(text, allow_anomalies=("outside the world",)), [])

    def test_labels_and_final_state_must_agree(self):
        text = CLEAN.replace("activity=Destroyed alive=false", "activity=Searching alive=false")
        self.assertTrue(any("dead but" in p for p in ai.probe_problems(text)))
        text = CLEAN.replace("activity=Destroyed alive=false", "activity=Destroyed alive=true")
        problems = ai.probe_problems(text)
        self.assertTrue(any("alive but" in p for p in problems))
        self.assertTrue(any("destroyed at 10.7s but alive" in p for p in problems))

    def test_dropped_launches_and_objective_count(self):
        self.assertTrue(ai.probe_problems(CLEAN.replace("dropped=0", "dropped=2")))
        text = CLEAN.replace("destroyed: 1, total: 2", "destroyed: 2, total: 2")
        self.assertTrue(any("objective" in p for p in ai.probe_problems(text)))
        # Other missions choose their own objective group.
        self.assertEqual(ai.probe_problems(text.replace("mission=free", "mission=intercept")), [])

    def test_debrief_kills_never_below_the_combat_count(self):
        text = CLEAN.replace("kills=[1, 0,", "kills=[0, 0,")
        self.assertTrue(any("kills" in p for p in ai.probe_problems(text)))
        self.assertEqual(ai.probe_problems(CLEAN.replace("kills=[1, 0,", "kills=[2, 0,")), [])

    def test_undamaged_ground_collision(self):
        text = CLEAN.replace(
            "t=1289 (10.7s) destroyed: Enemy 1-1\n", ""
        ).replace("alive=false agl=4185 kt=430", "alive=false agl=8 kt=0")
        self.assertTrue(any("flew into the ground" in p for p in ai.probe_problems(text)))
        damaged = text + "AI damage: actor=2 faults=[(4, 1)] return=None\n"
        self.assertFalse(any("flew into the ground" in p for p in ai.probe_problems(damaged)))

    def test_repeated_radio_line(self):
        text = CLEAN + "".join(f"  {20 + i}.0s Red two: 'Contact' []\n" for i in range(3))
        self.assertTrue(any("radio repeats" in p for p in ai.probe_problems(text)))

    def test_scenarios_are_unique_and_named_for_the_lane(self):
        names = [s.name for s in ai.scenarios()]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(all(n.startswith("ai-") for n in names))
        self.assertGreater(len(names), 300)


if __name__ == "__main__":
    unittest.main()
