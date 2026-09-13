from pathlib import Path
import struct
import unittest

from check_assets import violation


class AssetCheckTests(unittest.TestCase):
    def test_format_documentation_is_allowed(self):
        self.assertIsNone(violation(Path("formats.md"), b"EALIB and PIC formats"))

    def test_archive_embedded_in_binary_is_rejected(self):
        self.assertIsNotNone(violation(Path("application"), b"\x00prefixEALIB\x01\x00"))

    def test_renamed_pic_is_rejected(self):
        header = bytearray(64)
        struct.pack_into("<H4I", header, 0, 0, 2, 2, 64, 4)
        self.assertIsNotNone(violation(Path("renamed.bin"), bytes(header) + bytes(4)))

    def test_asset_extension_is_rejected(self):
        self.assertIsNotNone(violation(Path("MENU.PIC"), b"placeholder"))


if __name__ == "__main__":
    unittest.main()
