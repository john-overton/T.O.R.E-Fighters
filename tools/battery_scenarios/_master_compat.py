"""Independent legacy/current wire probes for the mixed-master battery scenario.

Only synthetic data is sent. Keeping this tiny encoder separate from the Rust
codec catches accidental protocol-2 fields in replies to released clients.
"""
import socket
import struct
import zlib
from contextlib import ExitStack


class Bits:
    def __init__(self, data=None):
        self.value = int.from_bytes(data or b"", "little")
        self.at = 0
        self.limit = None if data is None else len(data) * 8

    def put(self, value, width):
        assert 0 <= value < 1 << width
        self.value |= value << self.at
        self.at += width

    def get(self, width):
        assert self.limit is not None and self.at + width <= self.limit
        value = (self.value >> self.at) & ((1 << width) - 1)
        self.at += width
        return value

    def text(self, text):
        data = text.encode()
        self.put(len(data), 8)
        for byte in data:
            self.put(byte, 8)

    def read_text(self):
        return bytes(self.get(8) for _ in range(self.get(8))).decode()

    def bytes(self):
        return self.value.to_bytes((self.at + 7) // 8, "little")

    def end(self):
        assert self.limit - self.at < 8 and self.value >> self.at == 0


def packet(kind, version, body, length):
    rest = struct.pack("<BH", kind, version) + body.bytes()
    assert len(rest) + 4 <= length
    rest = rest.ljust(length - 4, b"\0")
    return struct.pack("<I", zlib.crc32(b"TORE-MASTER" + rest)) + rest


def identity(bits, version, commit):
    bits.put(67 if version == 1 else 68, 16)
    bits.text("0.2.0")
    bits.text(commit)
    if version == 2:
        bits.put(0x3FA9C21B, 32)
    bits.put(1, 8)


def registration(version, cookie):
    bits = Bits()
    bits.put(version, 64)
    bits.put(cookie, 64)
    identity(bits, version, "synthetic-host")
    bits.put(1, 8)  # Dedicated, telemetry off.
    bits.put(0, 64)  # No install ID.
    bits.put(3, 8)  # Linux.
    bits.put(0, 4)  # No candidates.
    bits.put(67 if version == 1 else 68, 16)
    bits.put(0, 8)  # Lobby, open, not full.
    bits.put(0, 8)
    bits.put(6, 8)
    bits.put(version, 64)
    bits.text("0.2.0")
    bits.text("synthetic-host")
    if version == 2:
        bits.put(0x3FA9C21B, 32)
    bits.text(f"Mixed host {version}")
    bits.text("Synthetic mission")
    bits.text("")
    bits.put(0, 8)  # No callsigns.
    return packet(3, version, bits, 1200)


def drive_master_mixed(d):
    from battery_scenarios.net import LOCALHOST, start_master
    master, port = start_master(d)
    target = (LOCALHOST, port)

    def exchange(sock, version, data, kind):
        sock.sendto(data, target)
        reply, source = sock.recvfrom(1500)
        assert source == target
        assert struct.unpack("<I", reply[:4])[0] == zlib.crc32(b"TORE-MASTER" + reply[4:])
        assert struct.unpack("<BH", reply[4:7]) == (kind, version), "wrong reply layout/version"
        return Bits(reply[7:])

    with ExitStack() as sockets:
        hosts = {}
        for version in (1, 2):
            sock = sockets.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
            sock.bind((LOCALHOST, 0))
            sock.settimeout(5 * d.scale)
            challenge = exchange(sock, version, registration(version, 0), 1)
            assert challenge.get(64) == version
            cookie = challenge.get(64)
            listed = exchange(sock, version, registration(version, cookie), 4)
            assert listed.get(64) == version
            hosts[version] = listed.get(64)
        for version in (1, 2):
            browser = sockets.enter_context(socket.socket(socket.AF_INET, socket.SOCK_DGRAM))
            browser.bind((LOCALHOST, 0))
            browser.settimeout(5 * d.scale)
            for other in (False, True):
                query = Bits()
                query.put(99, 64)
                identity(query, version, "different-client-commit")
                query.put(2 | int(other), 8)
                query.put(0, 32)
                page = exchange(browser, version, packet(10, version, query, 1200), 11)
                assert (page.get(64), page.get(16), page.get(32), page.get(8)) == (99, 1, 0, 1)
                if version == 2:
                    assert page.get(32) == 0x3FA9C21B
                assert page.get(64) == hosts[version]
                flags = page.get(8)
                assert not flags & 8, "same code or released version must match"
                assert (page.get(2), page.get(8), page.get(8), page.get(3)) == (0, 0, 6, 3)
                assert page.read_text() == f"Mixed host {version}"
                page.end()
            for own in (True, False):
                listing = hosts[version if own else 3 - version]
                query = Bits()
                query.put(100, 64)
                query.put(listing, 64)
                details = exchange(browser, version, packet(12, version, query, 1000), 13)
                assert (details.get(64), details.get(64), details.get(8)) == (100, listing, int(own))
            d.log(f"PASS: master protocol {version} client sees only its own population; other versions cannot cross")
    master.send("listings")
    master.send("quit")
    master.finish(20, 0)
    master.expect(r"^listings=2$", "both generations registered on one master")
