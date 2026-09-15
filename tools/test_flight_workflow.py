"""Validate CLI selection guards without Cargo or retail media."""
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch
import extract_assets

SCRIPT = Path(__file__).with_name('extract_assets.py')


class FlightWorkflowTests(unittest.TestCase):
    def test_both_aircraft_and_weapons_are_forwarded_without_shell(self):
        with patch.object(sys, 'argv', [str(SCRIPT), '--aircraft', 'f18', '--aircraft', 'rafale', '--weapons', '--dry-run']), \
             patch.object(extract_assets.subprocess, 'run') as run:
            run.return_value.returncode = 0
            self.assertEqual(extract_assets.main(), 0)
            command = run.call_args.args[0]
            choices = [command[i+1] for i, value in enumerate(command) if value == '--aircraft']
            self.assertEqual(choices, ['f18', 'rafale'])
            self.assertIn('--weapons', command)
            self.assertNotIn('shell', run.call_args.kwargs)

    def test_native_domains_are_separate_from_archive_profiles(self):
        for options in [['--native-flight'], ['--native-menus'], ['--weapons'], ['--aircraft', 'f18']]:
            result = subprocess.run([sys.executable, str(SCRIPT), '--native-weapons', *options],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)

    def test_menu_research_forwarding_and_exclusive_selection(self):
        with patch.object(sys, 'argv', [str(SCRIPT), '--native-menus', '--dry-run']), \
             patch('extract_native_flight.extract') as extract:
            self.assertEqual(extract_assets.main(), 0)
            self.assertEqual(extract.call_args.kwargs['domain'], 'menus')
            self.assertTrue(extract.call_args.kwargs['preview'])
        for options in [['--native-flight'], ['--music'], ['--theater', 'UKR'],
                        ['--aircraft', 'f18', '--validate-flight']]:
            result = subprocess.run([sys.executable, str(SCRIPT), '--native-menus', *options],
                                    capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 2)

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
