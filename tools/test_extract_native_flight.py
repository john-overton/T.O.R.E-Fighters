"""Synthetic fixtures only; no executable code is executed or embedded."""
import contextlib
import io
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import extract_native_flight as native


def sms():
    return struct.pack('<III', 1, 0, 0x401000) + b'_FMFlight\0'


def pe():
    data = bytearray(513)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 64)
    data[64:68] = b'PE\0\0'
    struct.pack_into('<HH', data, 68, 0x14c, 1)
    struct.pack_into('<H', data, 84, 96)
    struct.pack_into('<H', data, 88, 0x10b)
    struct.pack_into('<I', data, 116, 0x400000)
    data[184:189] = b'.text'
    struct.pack_into('<IIII', data, 192, 1, 4096, 1, 512)
    struct.pack_into('<I', data, 220, 0x20000000)
    return bytes(data)


class NativeResearchTests(unittest.TestCase):
    def test_metadata_bounds(self):
        self.assertEqual(native.symbols(sms())[0]['va'], 0x401000)
        self.assertTrue(native.sections(pe())[0]['executable'])
        for data in [b'', sms()[:10], sms()[:-1], struct.pack('<I', 100001)]:
            with self.assertRaises(ValueError):
                native.symbols(data)
        for data in [b'', pe()[:100], pe()[:-1]]:
            with self.assertRaises(ValueError):
                native.sections(data)

    def test_repeatable_static_output_and_conflicts(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root/'media'
            source.mkdir()
            (source/'FA.EXE').write_bytes(pe())
            (source/'FA.SMS').write_bytes(sms())
            output = root/'research'
            with patch.object(native.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()):
                native.extract(source, output, preview=True)
                run.assert_not_called()
                self.assertFalse(output.exists())
                run.return_value.stdout = '  401000: 00  synthetic instruction\n'
                with patch.object(native.shutil, 'which', return_value='/tool/objdump'):
                    native.extract(source, output)
                    first = {p.relative_to(output):p.read_bytes() for p in output.rglob('*') if p.is_file()}
                    native.extract(source, output)
                    self.assertEqual(first, {p.relative_to(output):p.read_bytes() for p in output.rglob('*') if p.is_file()})
                    self.assertEqual(run.call_args.args[0][0], '/tool/objdump')
                    self.assertFalse((output/'pt-field-references.json').exists())
                    (output/'inventory.json').write_text('keep')
                    with self.assertRaises(ValueError):
                        native.extract(source, output)
                    self.assertEqual((output/'inventory.json').read_text(), 'keep')
                    with self.assertRaises(ValueError):
                        native.extract(source, source/'bad')


if __name__ == '__main__':
    unittest.main()
