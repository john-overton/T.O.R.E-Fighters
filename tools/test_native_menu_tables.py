"""Synthetic selector dispatch and table bounds; no retail fixtures."""
import struct
import unittest
from native_menu_tables import read_va, string_list, literal_list, ordnance_controls


class MenuTableTests(unittest.TestCase):
    def test_ordnance_dispatch_rejects_unknown_duplicate_and_truncated_targets(self):
        branches = [0x41b0d1, 0x41b139, 0x41b18d, 0x41b1e1, 0x41b2d6]
        data = struct.pack('<5I', *branches)
        rows = [{'va': 0x41b336, 'raw': 0, 'size': 20, 'executable': True}]
        result = ordnance_controls(data, rows)
        self.assertEqual([r['action_id'] for r in result['controls']], list(range(1, 6)))
        for invalid in [data[:-1], struct.pack('<5I', 0, *branches[1:]),
                        struct.pack('<5I', branches[1], *branches[1:])]:
            with self.assertRaises(ValueError):
                ordnance_controls(invalid, rows)

    def fixture(self):
        code = b'\xb8' + struct.pack('<I', 200) + b'\xc3'
        data = code + b'One\0Two\0\0'
        sections = [{'va':100, 'raw':0, 'size':6, 'executable':True},
                    {'va':200, 'raw':6, 'size':9, 'executable':False}]
        return data, sections

    def test_literal_pointer_grammar_and_order(self):
        data, sections = self.fixture()
        self.assertEqual(literal_list(data, sections, 100),
                         {'table_va':200, 'values':['One','Two']})
        self.assertEqual(string_list(data, sections, 200), ['One','Two'])
        for data in [b'\x90'+data[1:], data[:-1]]:
            with self.assertRaises(ValueError): literal_list(data, sections, 100)

    def test_lists_require_double_termination_ascii_and_bounds(self):
        for data in [b'\0', b'One\0', b'\xff\0\0', b'A'*161+b'\0\0', b'A\0'*257+b'\0']:
            rows = [{'va':200,'raw':0,'size':len(data),'executable':False}]
            with self.assertRaises(ValueError): string_list(data,rows,200)
        data, rows = self.fixture()
        for va, size, code in [(100,6,False),(200,9,True),(99,1,True),(100,16385,True)]:
            with self.assertRaises(ValueError): read_va(data,rows,va,size,executable=code)
