"""Synthetic selector dispatch and table bounds; no retail fixtures."""
import struct
import unittest
from native_menu_tables import read_va, string_list, literal_list


class MenuTableTests(unittest.TestCase):
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
