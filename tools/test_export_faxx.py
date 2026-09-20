"""Synthetic export contracts: geometry numbers, relocation padding and tool boundary."""
import math
import struct
import unittest
from unittest.mock import patch

from export_faxx import hook_equipped_pt, independent_pt, jump, moved_face, stub, turn, vb
from openfa_tools import require_static


class ExportTests(unittest.TestCase):
    def test_split_leaf_numbers_and_fixed_hinge(self):
        self.assertEqual(turn([17,-22,0], .6), [17,-22,0])
        actual = turn([42,-34,0], .6)
        self.assertAlmostEqual(actual[1], -31.9040273791, places=8)
        self.assertAlmostEqual(actual[2], -6.7757096807, places=8)
        self.assertAlmostEqual(math.dist(actual,[17,-22,0]), math.dist([42,-34,0],[17,-22,0]))

    def test_slot_writes_are_noncontiguous_and_quantized(self):
        data = vb([12,30], [[1.4,-4.6,2],[3,8,9]])
        self.assertEqual(struct.unpack('<HHhhh',data[2:12]), (1,96,1,-5,2))
        self.assertEqual(struct.unpack('<HHhhh',data[14:24]), (1,240,3,8,9))

    def test_jump_stub_keeps_following_padding_addressable(self):
        rendered = stub(100,19,1000,'')
        raw = bytes.fromhex(rendered.split('data: ')[1])
        self.assertEqual(len(raw),19)
        self.assertEqual(raw[:4],jump(100,1000))
        self.assertEqual(raw[-4:],jump(115,119))
        with self.assertRaises(struct.error):
            jump(0,100000)

    def test_face_edit_preserves_uv_and_indices(self):
        raw = b'\xfc\x64\x02\x37\x00'+struct.pack('<hhhbbb',0,32765,0,0,0,0)+b'\x03\x00\x01\x02'+bytes(range(12))
        result = moved_face(raw,[[0,-22,0],[1,-22,0],[0,-23,-1]], .6)
        self.assertEqual(raw[14:],result[14:])
        self.assertNotEqual(raw[5:11],result[5:11])

    def test_separate_identity_preserves_donor_except_names_and_hook(self):
        fixture = (b"[brent's_relocatable_format]\r\nword 636\r\n"
                   b";--- START OF PLANE_TYPE ---\r\n\r\n    dword $91\r\n"
                   b':ot_names\r\n string "F-22"\r\n string "Donor"\r\n string "F22.PT"\r\n'
                   b':shape\r\n string "f22.SH"\r\n:shadowShape\r\n string "f22_s.SH"\r\n'
                   b':hudName\r\n string "f22.HUD"\r\nend\r\n')
        result = independent_pt(fixture)
        self.assertIn(b'dword $93\r\n', result)
        self.assertIn(b'string "FAXX.PT"', result)
        self.assertIn(b'string "FAXX_S.SH"', result)
        self.assertIn(b':hudName\r\n string "f22.HUD"\r\nend\r\n', result)
        self.assertTrue(result.startswith(b"[brent's_relocatable_format]\r\nword 636\r\n"))
        self.assertIn(b'string "F22.PT"', fixture)

    def test_hook_flag_is_idempotent_and_preserves_other_sections(self):
        source = b"dword $91\n;--- START OF PLANE_TYPE ---\n\n dword $91\nword 17\n"
        expected = b"dword $91\n;--- START OF PLANE_TYPE ---\n\n dword $93\nword 17\n"
        self.assertEqual(hook_equipped_pt(source), expected)
        self.assertEqual(hook_equipped_pt(expected), expected)
        with self.assertRaises(ValueError):
            hook_equipped_pt(source.replace(b' dword $91', b' dword $57'))

    def test_identity_rejects_missing_blocks(self):
        with self.assertRaises(ValueError):
            independent_pt(b"[brent's_relocatable_format]\nend\n")

    def test_unpatched_tool_is_rejected(self):
        with patch('openfa_tools.run',return_value='OpenFA 0.2.14'):
            with self.assertRaises(ValueError):
                require_static('fake')
        with patch('openfa_tools.run',return_value='tore-static-export-v1\n'):
            require_static('fake')


if __name__ == '__main__':
    unittest.main()
