"""Tests for the menus lane's image checks (tools/battery_scenarios/menus.py)."""
import sys
import json
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import menus, render, vegetation  # noqa: E402
from battery_scenarios._strips import is_enemy_field  # noqa: E402


def write_ppm(directory, name, width, height, pixel):
    path = Path(directory) / name
    body = bytearray()
    for y in range(height):
        for x in range(width):
            body += bytes(pixel(x, y))
    path.write_bytes(f"P6\n{width} {height}\n255\n".encode() + bytes(body))
    return path


class ShadowDistanceCoverageTests(unittest.TestCase):
    def test_every_distance_has_a_window_capture(self):
        covered = set()
        scenarios = menus.scenarios()
        for scenario in scenarios:
            if "--shadow-distance" in scenario.args:
                self.assertTrue(scenario.window)
                self.assertIn("--capture-flight", scenario.args)
                covered.add(scenario.args[scenario.args.index("--shadow-distance") + 1])
        self.assertEqual(covered, {"trees", "1", "2", "4", "6", "8"})
        self.assertTrue(any("graphics-shadows" in s.args for s in scenarios))


class ImageCheckTests(unittest.TestCase):
    def check(self, condition, pixel):
        with tempfile.TemporaryDirectory() as d:
            path = write_ppm(d, "a.ppm", 96, 72, pixel)
            return menus.scene_problems(condition)(f"Scene capture: {path}\n")

    @staticmethod
    def scene(sky, ground):
        def pixel(x, y):
            base = sky if y < 36 else ground
            # A little texture so the frame is not one flat colour.
            wobble = (x * 7 + y * 13) % 48
            tint = ((x * 37) % 30, (y * 53) % 30, ((x + y) * 29) % 30)
            return tuple(min(255, c + wobble // 2 + t) for c, t in zip(base, tint))

        return pixel

    def test_a_bright_day_scene_passes(self):
        self.assertEqual(self.check(0, self.scene((90, 120, 200), (60, 110, 50))), [])

    def test_a_dark_night_scene_passes(self):
        self.assertEqual(self.check(5, self.scene((5, 8, 30), (2, 2, 6))), [])

    def test_a_black_frame_is_reported(self):
        problems = self.check(0, lambda x, y: (0, 0, 0))
        self.assertTrue(any("black" in p for p in problems), problems)

    def test_a_single_colour_frame_is_reported(self):
        problems = self.check(0, lambda x, y: (200, 200, 200))
        self.assertTrue(any("one colour" in p for p in problems), problems)

    def test_a_bright_night_and_a_dark_day_are_reported(self):
        self.assertTrue(any("bright" in p for p in self.check(5, self.scene((120, 140, 200), (90, 120, 80)))))
        self.assertTrue(any("dark" in p for p in self.check(0, self.scene((10, 10, 40), (5, 20, 5)))))

    def test_fog_may_be_mostly_pale_but_not_all_one_colour(self):
        pale_and_cockpit = lambda x, y: (215, 215, 220) if y < 45 else ((x * 5) % 200, (y * 9) % 200, (x + y) * 3 % 200)
        self.assertEqual(self.check(2, pale_and_cockpit), [])
        problems = self.check(2, lambda x, y: (215, 215, 220))
        self.assertTrue(any("one colour" in p for p in problems), problems)

    def test_a_missing_capture_is_reported(self):
        self.assertEqual(menus.scene_problems(0)("nothing here"), ["no capture written"])


class ReplaysBarTests(unittest.TestCase):
    def picture(self, directory, offset, raised=False, missing=False):
        def pixel(x, y):
            # Synthetic letter cores, never imported retail pixels.
            if x == 161 + offset and 42 <= y <= 50:
                return (24, 40, 154)
            if not missing and x == 211 + offset and 42 - raised <= y <= 50 - raised:
                return (24, 40, 154)
            shade = 180 + (x + y) % 50
            return (shade, shade, shade)
        return write_ppm(directory, "bar.ppm", 640, 480, pixel)

    def test_shared_baseline_on_every_bar_origin(self):
        with tempfile.TemporaryDirectory() as directory:
            for offset in (0, -6, 109):
                path = self.picture(directory, offset)
                self.assertEqual(menus.replays_bar_picture(offset)(f"Menu preview: {path}"), [])

    def test_old_one_pixel_lift_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.picture(directory, 0, raised=True)
            self.assertEqual(menus.replays_bar_picture()(f"Menu preview: {path}"),
                             ["Replays baseline or letter weight differs from Multi"])

    def test_missing_label_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self.picture(directory, 0, missing=True)
            self.assertEqual(menus.replays_bar_picture()(f"Menu preview: {path}"),
                             ["Multi or Replays letter core is missing"])

    def test_all_backgrounds_and_states_are_covered(self):
        covered = {s.name for s in menus.scenarios() if s.name.startswith("menus-replays-bar-")}
        self.assertEqual(covered, {
            f"menus-replays-bar-{background}-{state}"
            for background in ("choosev", "chooseu", "choosem", "chooseac", "choose3")
            for state in ("normal", "bar-multi-hover", "bar-replays-hover",
                          "bar-replays-pressed", "bar-replays-focus", "multi")
        })



class WingLoadoutRecordingTests(unittest.TestCase):
    def test_records_are_checked_for_every_plane_and_start(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for label in ("before", "after", "ground"):
                events = []
                for plane in range(4):
                    stores = [(6, "MK82.SH", 2), (8, "SIDEM.SH", 2)]
                    if label == "before" and plane:
                        stores = [(6, "MAVRK.SH", 4), (7, "AMRAM.SH", 2), (8, "SIDEM.SH", 2), (5, "tank", 1)]
                    for hardpoint, shape, items in stores:
                        events.append({"kind": "stores.station", "tick": 0, "subject": plane,
                                       "fields": {"hardpoint": hardpoint, "shape": shape, "look": "store", "items": items}})
                path = root / "wing-loadout" / f"{label}.toreplay-log" / "log.jsonl"
                path.parent.mkdir(parents=True)
                path.write_text("\n".join(json.dumps(e) for e in events))
            self.assertEqual(menus.wing_loadout_recording_problems(root, ""), [])
            events[-1]["fields"]["items"] = 1
            path.write_text("\n".join(json.dumps(e) for e in events))
            self.assertTrue(any("ground plane 3" in p for p in menus.wing_loadout_recording_problems(root, "")))

    def test_missing_export_is_a_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(len(menus.wing_loadout_recording_problems(Path(directory), "")), 3)

class GroundStartFieldTests(unittest.TestCase):
    def test_window_ground_starts_are_at_blue_fields(self):
        # --ground-start refuses an enemy field (6c83d6c1); free flight is Blue
        # and starts in UKR unless --theater says otherwise.
        checked = 0
        for s in menus.scenarios() + render.scenarios():
            if "--ground-start" not in s.args or "--capture-flight" not in s.args:
                continue
            theater = s.args[s.args.index("--theater") + 1] if "--theater" in s.args else "UKR"
            airport = int(s.args[s.args.index("--ground-start") + 1])
            self.assertFalse(is_enemy_field(theater, airport), f"{s.name}: {theater} airport {airport} is Redfor")
            checked += 1
        self.assertGreaterEqual(checked, 14)


class TopCrownAuditTests(unittest.TestCase):
    def test_an_import_without_vegetation_art_is_an_environment_problem(self):
        output = "vegetation art: the import predates the retail vegetation art; re-import media\n"
        with tempfile.TemporaryDirectory() as d:
            problems = vegetation.check_top_crowns(Path(d), output)
        self.assertEqual(len(problems), 1)
        self.assertTrue(problems[0].startswith("environment:"), problems)

    def test_a_missing_audit_without_a_reason_is_still_a_failure(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(vegetation.check_top_crowns(Path(d), ""), ["no retail grove crown audit"])


if __name__ == "__main__":
    unittest.main()
