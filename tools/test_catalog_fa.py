"""Catalog decoding contract, checked against synthetic BRF records only."""
import tempfile
import unittest
from pathlib import Path

from catalog_fa import (Token, aircraft_row, catalogs, layouts, parse_brf,
                        surface_references, weapon_kind, weapon_row)

LAYOUT = layouts()


def record(sections, values, trailer):
    """Build one synthetic BRF resource from the recovered field order."""
    text = "[brent's_relocatable_format]\n"
    for section in sections:
        for kind, name in LAYOUT[section]:
            value = values.get(name)
            if value is None:
                value = '0' if kind != 'symbol' else '_Proc'
                kind = 'dword' if kind == 'ptr' else kind
            text += f'{kind} {value}\n'
    return (text + trailer + 'end\n').encode('ascii')


def plane(**overrides):
    values = {'structType': '5', 'typeSize': '660', 'obj_class': '$ffff8000',
              'ot_names': 'ot_names', 'shape': 'shape', 'hudName': 'hud',
              'year': '1986', 'weight': '23050', 'maxAlt': '^60000', 'hitPoints': '116',
              'internalFuel': '11220', 'maxTakeoffWeight': '49224', 'thrust': '17687',
              'aftThrust': '32000', 'engines': '2', 'numHards': '1', 'hards': 'hards',
              'env': 'env', 'envMin': '0', 'envMax': '1'}
    values.update(overrides)
    stations = ''.join(f'{kind} {"defaults" if name == "defaultTypeName" else 0}\n'
                       for kind, name in LAYOUT['HARDPOINT'])
    rows = ''
    for load in (0, 1):
        # Three points: 200 ft/s and 1743 ft/s at sea level, 1743 ft/s at 36,000 feet.
        row = {'gload': str(load), 'count': '3', 'speed[0]': '200', 'alt[0]': '0',
               'speed[1]': '1743', 'alt[1]': '36000', 'speed[2]': '1743', 'alt[2]': '0'}
        rows += ''.join(f'{kind} {row.get(name, "0")}\n' for kind, name in LAYOUT['ENVELOPE'])
    trailer = (':ot_names\nstring "F/A-18D"\nstring "F/A- 18D Hornet"\nstring "F18.PT"\n'
               ':shape\nstring "f18.SH"\n:hud\nstring "f18.HUD"\n'
               ':defaults\nstring "M61.JT"\n'
               f':hards\n{stations}:env\n{rows}')
    return record(('OBJECT', 'NPC', 'PLANE'), values, trailer)


def store(**overrides):
    values = {'structType': '7', 'typeSize': '315', 'si_names': 'si_names',
              'shape': 'shape', 'year': '1983', 'weight': '190', 'flags': '$1204f',
              'sig': '2', 'zone1.minRange': '4000', 'zone1.maxRange': '24000',
              'zone0.maxRange': '50000', 'fuelT': '44', 'removeT': '88',
              '_maxSpeed': '4400', 'projsInPod': '1', 'actualRoundsPerGame': '1',
              'damage[0]': '100', 'fuzeRadius': '100'}
    values.update(overrides)
    trailer = (':si_names\nstring "AIM-9M"\nstring "AIM-9M Sidewinder"\nstring "AIM9M.JT"\n'
               ':shape\nstring "sidem.SH"\n')
    return record(('OBJECT', 'PROJECTILE'), values, trailer)


class TokenTests(unittest.TestCase):
    def test_widths_match_the_rust_reader(self):
        self.assertEqual(Token('word', '$ffff8000', False).number(), -32768)
        self.assertEqual(Token('byte', '$ff', False).number(), 255)
        self.assertEqual(Token('dword', '-60', False).number(), -60)
        with self.assertRaises(ValueError):
            Token('string', 'x', False).number()
        with self.assertRaises(ValueError):
            Token('dword', '$1ffffffff', False).number()


class BrfTests(unittest.TestCase):
    def test_labels_strings_and_scale_markers(self):
        blocks = parse_brf(b'[brent\'s_relocatable_format]\nword ^60\n:names\n'
                           b'string "A"  ; comment\nend\n')
        self.assertTrue(blocks[''][0].scaled)
        self.assertEqual(blocks['names'][0].value, 'A')

    def test_malformed_records_are_refused(self):
        for text in (b'nope\n', b"[brent's_relocatable_format]\nword 1\n",
                     b"[brent's_relocatable_format]\nend\nword 1\n",
                     b"[brent's_relocatable_format]\nfloat 1\nend\n",
                     b"[brent's_relocatable_format]\n:a\n:a\nend\n",
                     b'[brent\'s_relocatable_format]\nstring "a\nend\n'):
            with self.assertRaises(ValueError):
                parse_brf(text)


class RowTests(unittest.TestCase):
    def test_aircraft_row_reports_source_values_and_derived_speeds(self):
        row = aircraft_row('F18.PT', 'FA_2.LIB', plane(), LAYOUT, {'M61.JT'})
        self.assertEqual(row['display_name'], 'F/A- 18D Hornet')
        self.assertEqual(row['empty_weight_lb'], 23050)
        self.assertEqual(row['object_class'], '0x8000')
        self.assertEqual(row['ceiling_ft'], 60000)
        self.assertEqual(row['no_lift'], 'no')
        # 1,743 ft/s is 1,033 knots; the slowest sea level point is 200 ft/s.
        self.assertEqual((row['top_speed_kt'], row['sea_level_min_kt']), (1033, 118))
        self.assertEqual(row['internal_gun'], 'M61.JT')
        self.assertEqual(row['default_stores'], 'M61.JT:1')
        self.assertEqual(row['hud_resource'], 'f18.HUD')

    def test_unscaled_altitude_and_no_lift_flag(self):
        row = aircraft_row('H.PT', 'FA_2.LIB', plane(maxAlt='1792000', flags='8'),
                           LAYOUT, set())
        self.assertEqual((row['ceiling_ft'], row['no_lift']), (7000, 'yes'))

    def test_aircraft_layout_and_station_count_are_checked(self):
        for broken in (plane(numHards='2'), plane() + b'byte 0\n', plane(envMin='2', envMax='3')):
            with self.assertRaises(ValueError):
                aircraft_row('F18.PT', 'FA_2.LIB', broken, LAYOUT, set())

    def test_weapon_row_converts_ranges_and_timers(self):
        row = weapon_row('AIM9M.JT', 'FA_2.LIB', store(), LAYOUT)
        self.assertEqual(row['short_name'], 'AIM-9M')
        self.assertEqual((row['launch_min_nmi'], row['launch_max_nmi']), (0.66, 3.95))
        self.assertEqual(row['seeker_max_nmi'], 8.23)
        # One projectile timer unit is a quarter second.
        self.assertEqual((row['motor_burn_s'], row['lifetime_s']), (11.0, 22.0))
        self.assertEqual(row['ordnance_bank'], 'one')
        self.assertEqual(row['damage_by_class'], '100;0;0;0;0')

    def test_bank_two_and_mechanical_kinds(self):
        self.assertEqual(weapon_row('MK82.JT', 'FA_2.LIB',
                                    store(flags='$22012', sig='0', fuelT='0'),
                                    LAYOUT)['ordnance_bank'], 'two')
        kinds = {'guided missile': {'fuelT': '44', 'sig': '2'},
                 'rocket': {'fuelT': '44', 'sig': '0'},
                 'gun round': {'fuelT': '0', 'sig': '0', 'initialSpeed': '2933'},
                 'bomb': {'fuelT': '0', 'sig': '0'},
                 'guided bomb': {'fuelT': '0', 'sig': '1'}}
        for expected, overrides in kinds.items():
            projectile = parse_brf(store(**overrides))['']
            named = dict(zip([name for _, name in LAYOUT['PROJECTILE']],
                             projectile[len(LAYOUT['OBJECT']):]))
            self.assertEqual(weapon_kind(named), expected)


class CatalogTests(unittest.TestCase):
    def test_cross_references_count_stations_and_surface_types(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'FA_2.LIB'
            source.mkdir()
            (source / 'F18.PT').write_bytes(plane())
            (source / 'M61.JT').write_bytes(store(initialSpeed='2933', fuelT='0', sig='0'))
            (source / 'T72.NT').write_bytes(b'[brent\'s_relocatable_format]\n'
                                            b':names\nstring "M61.JT"\nend\n')
            aircraft, weapons = catalogs(Path(directory), LAYOUT)
        self.assertEqual(aircraft[0]['internal_gun'], 'M61.JT')
        self.assertEqual(weapons[0]['aircraft_default_stations'], 1)
        self.assertEqual(weapons[0]['surface_type_references'], 1)
        self.assertEqual(weapons[0]['archive'], 'FA_2.LIB')

    def test_unreadable_surface_records_are_skipped(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'BAD.NT'
            path.write_bytes(b'MZ not a BRF record')
            self.assertEqual(surface_references([path]), {})


if __name__ == '__main__':
    unittest.main()
