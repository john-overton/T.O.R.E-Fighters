"""Synthetic checks for the promo timeline and replay camera input compiler."""
import importlib.util
import math
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('reel_render', ROOT / 'tools/reel/render.py')
reel = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reel)


class ReelPlanTests(unittest.TestCase):
    def setUp(self):
        self.config = reel.read_config(ROOT / 'tools/reel/shots.toml')

    def test_exact_timeline_rosters_and_cutdown(self):
        shots = self.config['shot']
        reel_info = self.config['reel']
        self.assertEqual(sum(s['frames'] for s in shots), reel_info['duration'] * reel_info['fps'])
        self.assertEqual(shots[0]['layout'], 'boot')
        self.assertEqual(shots[-1]['layout'], 'end')
        self.assertGreaterEqual(shots[-1]['frames'], 150)
        self.assertEqual(len(self.config['reel']['aircraft_roster']), 14)
        self.assertEqual(len(self.config['reel']['theater_roster']), 16)
        self.assertEqual(sum(n for _, _, n in self.config['cutdown']['segments']), 900)
        segments = [s for s, _, _ in self.config['cutdown']['segments']]
        self.assertIn('cockpit', segments)
        self.assertTrue(any(s.startswith('replay') for s in segments))

    def test_cuts_follow_the_music_beat_grid(self):
        music = self.config['music']
        beat = 60 * 60 / music['grid_bpm']
        for shot in self.config['shot'][1:]:
            beats = (shot['start'] - music['downbeat_frame']) / beat
            self.assertLessEqual(abs(beats - round(beats)) * beat, 1.0, shot['id'])
        self.assertIn(music['kick_shot'], [s['id'] for s in self.config['shot']])

    def test_fixed_ticks_camera_endpoints_reverse_freeze_and_tails(self):
        for clip in self.config['capture']:
            lines = [[float(v) for v in line.split()] for line in reel.frame_plan(clip).splitlines()]
            self.assertEqual(len(lines), clip['frames'] + clip.get('tail', 0))
            self.assertEqual(lines[0][2:5], clip['camera'][0]['eye'])
            self.assertEqual(lines[clip['frames'] - 1][2:5], clip['camera'][-1]['eye'])
            expected = clip.get('freeze_tick', clip['in_tick'] + 2 * (clip['frames'] - 1) * clip.get('speed', 1))
            self.assertEqual(lines[clip['frames'] - 1][0], expected)
            self.assertTrue(all(5 <= line[8] <= 120 and len(line) == 13 for line in lines))

    def test_aim_places_the_subject_on_screen(self):
        eye, point, fov = [96.0, 34.0, 112.0], [0.0, -2.0, 4.0], 29.0
        target = reel.aim_target(eye, point, fov, -0.33, 0.12)
        d = [t - e for t, e in zip(target, eye)]
        yaw, pitch = math.atan2(d[0], d[2]), math.atan2(d[1], math.hypot(d[0], d[2]))
        f = (math.sin(yaw) * math.cos(pitch), math.sin(pitch), math.cos(yaw) * math.cos(pitch))
        r = (math.cos(yaw), 0.0, -math.sin(yaw))
        u = (-math.sin(yaw) * math.sin(pitch), math.cos(pitch), -math.cos(yaw) * math.sin(pitch))
        v = [p - e for p, e in zip(point, eye)]
        depth = sum(a * b for a, b in zip(v, f))
        ty = math.tan(math.radians(fov / 2))
        self.assertAlmostEqual(sum(a * b for a, b in zip(v, r)) / depth / (ty * 16 / 9), -0.33, places=4)
        self.assertAlmostEqual(sum(a * b for a, b in zip(v, u)) / depth / ty, 0.12, places=4)

    def test_monotone_keys_never_overshoot(self):
        keys = [{'frame': 0}, {'frame': 76}, {'frame': 151}, {'frame': 301}]
        values = [2.5, 2.5, 1.0, 1.0]
        samples = [reel.interpolate(keys, values, i, 'pchip') for i in range(302)]
        self.assertTrue(all(1.0 - 1e-9 <= s <= 2.5 + 1e-9 for s in samples))
        self.assertTrue(all(a >= b - 1e-9 for a, b in zip(samples, samples[1:])))
        self.assertAlmostEqual(reel.fov_of(reel.zoom_of(26.0078)), 26.0078, places=6)
        self.assertAlmostEqual(math.exp(reel.zoom_of(26.0078)), 2.5, places=3)


if __name__ == '__main__':
    unittest.main()
