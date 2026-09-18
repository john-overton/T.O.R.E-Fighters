"""Write stored EALIB entries with the required EOF directory sentinel."""
from pathlib import Path
import re
import struct


def archive_bytes(resources):
    if not 1 <= len(resources) <= 65535:
        raise ValueError('archive must contain 1..65535 resources')
    normalized = {}
    for name, data in resources.items():
        name = name.upper()
        if not re.fullmatch(r'[A-Z0-9_~&-]{1,8}\.[A-Z0-9]{1,3}', name):
            raise ValueError(f'expected an ASCII 8.3 resource name: {name}')
        if name in normalized:
            raise ValueError(f'duplicate resource name: {name}')
        normalized[name] = data
    cursor = 7 + 18 * (len(normalized) + 1)
    directory = bytearray(b'EALIB' + struct.pack('<H', len(normalized)))
    payloads = []
    for name, data in sorted(normalized.items()):
        directory += name.encode('ascii').ljust(13, b'\0') + b'\0' + struct.pack('<I', cursor)
        cursor += len(data)
        if cursor > 0xffffffff:
            raise ValueError('archive exceeds 32-bit offsets')
        payloads.append(data)
    # This is a directory entry, not a resource: zero name/flag, offset = EOF.
    directory += bytes(14) + struct.pack('<I', cursor)
    return bytes(directory) + b''.join(payloads)


def pack_stored(inputs, output):
    resources = {}
    for path in map(Path, inputs):
        name = path.name.upper()
        if name in resources:
            raise ValueError(f'duplicate resource name: {name}')
        resources[name] = path.read_bytes()
    data = archive_bytes(resources)
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('xb') as stream:
        stream.write(data)
