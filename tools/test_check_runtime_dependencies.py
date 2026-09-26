import tempfile
from pathlib import Path
import unittest

from check_runtime_dependencies import violations


class RuntimeDependencyTests(unittest.TestCase):
    def test_missing_linux_library_fails(self):
        self.assertTrue(violations("linux", "libasound.so.2 => not found", Path("app")))
        self.assertFalse(violations("linux", "libc.so.6 => /lib/libc.so.6", Path("app")))

    def test_macos_developer_library_fails(self):
        self.assertTrue(violations("darwin", "\t/opt/homebrew/lib/libthing.dylib (compatibility version 1.0.0)", Path("app")))
        self.assertFalse(violations("darwin", "\t/System/Library/Frameworks/AppKit.framework/AppKit (compatibility version 1.0.0)", Path("app")))

    def test_windows_crt_and_missing_libraries_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "System32").mkdir()
            (root / "System32/KERNEL32.dll").touch()
            binary = root / "app.exe"
            self.assertFalse(violations("win32", "    KERNEL32.dll\n    api-ms-win-core-file-l1-1-0.dll", binary, root))
            self.assertTrue(violations("win32", "    VCRUNTIME140.dll", binary, root))
            self.assertTrue(violations("win32", "    absent.dll", binary, root))

    def test_windows_32_bit_programs_resolve_against_syswow64(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for folder, names in (("System32", ("KERNEL32.dll", "only64.dll")), ("SysWOW64", ("KERNEL32.dll",))):
                (root / folder).mkdir()
                for name in names:
                    (root / folder / name).touch()
            binary = root / "app.exe"
            dos = bytearray(0x40)
            dos[:2] = b"MZ"
            dos[0x3C:0x40] = (0x40).to_bytes(4, "little")
            binary.write_bytes(bytes(dos) + b"PE\0\0" + (0x014C).to_bytes(2, "little"))
            self.assertFalse(violations("win32", "    KERNEL32.dll", binary, root))
            self.assertTrue(violations("win32", "    only64.dll", binary, root))
            # The same DLL resolves for a 64-bit program.
            binary.write_bytes(bytes(dos) + b"PE\0\0" + (0x8664).to_bytes(2, "little"))
            self.assertFalse(violations("win32", "    only64.dll", binary, root))

    def test_unrecognized_output_fails_closed(self):
        for platform in ("linux", "darwin", "win32"):
            self.assertTrue(violations(platform, "", Path("app")))
