"""Tests for the menus lane's image checks (tools/battery_scenarios/menus.py)."""
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from battery_scenarios import menus  # noqa: E402


def write_ppm(directory, name, width, height, pixel):
    path = Path(directory) / name
    body = bytearray()
    for y in range(height):
        for x in range(width):
            body += bytes(pixel(x, y))
    path.write_bytes(f"P6\n{width} {height}\n255\n".encode() + bytes(body))
    return path


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


if __name__ == "__main__":
    unittest.main()
