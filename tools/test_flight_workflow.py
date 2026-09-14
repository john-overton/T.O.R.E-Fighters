"""Validate CLI selection guards without Cargo or retail media."""
import subprocess
import sys
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name('extract_assets.py')


class FlightWorkflowTests(unittest.TestCase):
    def test_validation_requires_complete_aircraft_extraction(self):
        for options in [[], ['--aircraft', 'rafale', '--list'],
                        ['--aircraft', 'f18', '--dry-run'],
                        ['--aircraft', 'rafale', '--include', '*.PT'],
                        ['--aircraft', 'f18', '--native-flight']]:
            result = subprocess.run([sys.executable, str(SCRIPT), '--validate-flight', *options],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)
            self.assertIn('--validate-flight requires', result.stderr)

    def test_unreviewed_rafale_variant_is_not_an_alias(self):
        result = subprocess.run([sys.executable, str(SCRIPT), '--aircraft', 'rafalee', '--dry-run'],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 2)
        self.assertIn('invalid choice', result.stderr)


if __name__ == '__main__':
    unittest.main()
