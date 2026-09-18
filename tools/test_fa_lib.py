"""Synthetic archive layout contract, including the previously omitted sentinel."""
import struct
import tempfile
import unittest
from pathlib import Path

from fa_lib import archive_bytes, pack_stored


class ArchiveTests(unittest.TestCase):
    def test_known_single_entry_layout(self):
        expected = (b'EALIB\x01\x00' + b'TEST.TXT'+bytes(6) + struct.pack('<I',43)
                    + bytes(14)+struct.pack('<I',46)+b'abc')
        self.assertEqual(archive_bytes({'TEST.TXT':b'abc'}),expected)

    def test_last_entry_ends_at_sentinel_eof(self):
        b=archive_bytes({'B.SH':b'bbbb','A.PT':b'a'})
        self.assertEqual(struct.unpack_from('<I',b,21)[0],61)
        self.assertEqual(struct.unpack_from('<I',b,39)[0],62)
        self.assertEqual(b[43:57],bytes(14))
        self.assertEqual(struct.unpack_from('<I',b,57)[0],66)
        self.assertEqual(len(b),66)
        self.assertEqual(b[61:],b'abbbb')

    def test_reject_unsafe_and_duplicate_names(self):
        for records in [{'../X.SH':b'x'}, {'A.SH':b'a','a.sh':b'b'}, {}]:
            with self.assertRaises(ValueError):
                archive_bytes(records)

    def test_output_never_overwrites(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'A.SH'; p.write_bytes(b'synthetic')
            out=Path(d)/'TEST.LIB'
            pack_stored([p],out)
            with self.assertRaises(FileExistsError):
                pack_stored([p],out)
