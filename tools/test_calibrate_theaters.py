import unittest
from calibrate_theaters import fit, line_fit, placements


class CalibrationTests(unittest.TestCase):
    def test_center_and_spacing_are_recovered_independently(self):
        points = [dict(east_ft=x, north_ft=z, latitude=10 + z * 1e-5,
                       longitude=-60 + x * 2e-5)
                  for x, z in [(0, 0), (8192, 0), (0, 8192), (8192, 8192)]]
        result = fit(points, [2, 2])
        self.assertAlmostEqual(result['latitude'], 10.04096)
        self.assertAlmostEqual(result['longitude'], -59.91808)
        self.assertAlmostEqual(result['degrees_per_ft'][0], 1e-5)
        self.assertAlmostEqual(result['degrees_per_ft'][1], 2e-5)
        self.assertLess(result['max_km'], 1e-8)

    def test_degenerate_reversed_and_nonfinite_references_fail(self):
        for x, y in [([1], [2]), ([1, 1], [1, 2]), ([1, 2], [2, 1]), ([1, 2], [1, float('nan')])]:
            with self.assertRaises(ValueError):
                line_fit(x, y)

    def test_layout_parser_keeps_named_runways_only_and_rejects_ambiguity(self):
        runway = 'obj\n type STRIP2.OT\n pos 123 0 456\n name \x01Sample\x01\n .\n'
        self.assertEqual(placements(runway + runway.replace('STRIP2.OT', 'HANGR.OT')), {'Sample': [123, 456]})
        with self.assertRaises(ValueError):
            placements(runway + runway)


if __name__ == '__main__':
    unittest.main()
