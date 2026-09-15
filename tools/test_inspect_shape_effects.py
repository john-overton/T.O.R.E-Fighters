"""Synthetic static modules; no retail bytes or native execution."""
import struct
import unittest

from inspect_shape_effects import inspect
from test_extract_native_flight import pe


def shape():
    data = bytearray(pe()) + bytearray(511)
    data[64:66] = b'PL'
    struct.pack_into('<H', data, 70, 2)
    data[184:192] = b'CODE\0\0\0\0'
    struct.pack_into('<IIII', data, 192, 256, 4096, 256, 512)
    data[224:232] = b'.idata\0\0'
    struct.pack_into('<IIII', data, 232, 256, 8192, 256, 768)
    struct.pack_into('<IIIII', data, 768, 0x2040, 0, 0, 0x2080, 0x2060)
    struct.pack_into('<II', data, 832, 0x2090, 0)
    data[896:905] = b'main.dll\0'
    data[914:930] = b'do_start_interp\0'
    # Native candidate returning via the local import alias at CODE+0x80.
    data[512:525] = b'\xf0\0\x68' + struct.pack('<I', 0x401020) + b'\x68' + struct.pack('<I', 0x401080) + b'\xc3'
    data[640:646] = b'\xff\x25' + struct.pack('<I', 0x402060)
    return bytes(data)


class ShapeEffectsTests(unittest.TestCase):
    def test_import_alias_resolves_reentry_without_execution(self):
        report, _ = inspect(shape())
        self.assertEqual(report['aliases'], {0x401080: 'do_start_interp'})
        self.assertEqual(report['native_candidates'][0]['reentry_va'], 0x401020)

    def test_unmatched_byte_pattern_stays_a_candidate(self):
        data = bytearray(shape())
        data[560:562] = b'\xf0\0'
        report, _ = inspect(bytes(data))
        self.assertIsNone(report['native_candidates'][1]['reentry_va'])

    def test_import_address_and_truncation_are_bounded(self):
        data = bytearray(shape())
        struct.pack_into('<I', data, 832, 0x9000)
        with self.assertRaises(ValueError):
            inspect(bytes(data))
        for size in [0, 64, 100, 512, 1000]:
            with self.assertRaises(ValueError):
                inspect(shape()[:size])
