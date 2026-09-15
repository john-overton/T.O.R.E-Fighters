"""Static SH import/re-entry inventory. Candidates are not a control-flow proof.

Reads extracted user-owned modules; never loads or executes them. Optional GNU
objdump output must go to an ignored research directory, like extracted media.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from extract_native_flight import sections, unpack


def inspect(data):
    if len(data) > 16 * 1024 * 1024:
        raise ValueError('shape exceeds 16 MiB')
    pe, = unpack(data, '<I', 60)
    if data[pe:pe+4] not in (b'PL\0\0', b'PE\0\0'):
        raise ValueError('expected PL/PE shape')
    # Normalize the signature only in memory for the shared inert section reader.
    rows = sections(data[:pe] + b'PE' + data[pe+2:])
    base, = unpack(data, '<I', pe + 24 + 28)

    def read(va, size):
        row = next((r for r in rows if r['va'] <= va and
                    va + size <= r['va'] + r['size']), None)
        if row is None:
            raise ValueError('shape address outside file-backed sections')
        at = row['raw'] + va - row['va']
        return data[at:at+size]

    def word(va):
        return struct.unpack('<I', read(va, 4))[0]

    def string(va):
        out = bytearray()
        for i in range(256):
            value = read(va+i, 1)[0]
            if value == 0:
                return out.decode('ascii')
            out.append(value)
        raise ValueError('unterminated shape import name')

    imports = []
    directory = next((r for r in rows if r['name'] == '.idata'), None)
    if directory:
        for n in range(64):
            lookup, timestamp, chain, dll, iat = struct.unpack(
                '<IIIII', read(directory['va'] + 20*n, 20))
            if not any((lookup, timestamp, chain, dll, iat)):
                break
            library = string(base+dll)
            for i in range(1024):
                entry = word(base+(lookup or iat)+4*i)
                if not entry:
                    break
                name = (f'ordinal:{entry & 0xffff}' if entry & 0x80000000
                        else string(base+entry+2))
                imports.append({'library': library, 'name': name,
                                'iat_va': base+iat+4*i})
            else:
                raise ValueError('too many shape imports')
        else:
            raise ValueError('too many import descriptors')
    row = next(r for r in rows if r['name'] == 'CODE')
    code = read(row['va'], row['size'])
    # Link the local six-byte aliases used by native animation guards to IAT names.
    aliases = {}
    for i in range(len(code)-5):
        if code[i:i+2] == b'\xff\x25':
            target, = unpack(code, '<I', i+2)
            for entry in imports:
                if target == entry['iat_va']:
                    aliases[row['va']+i] = entry['name']
    candidates = []
    for i in range(len(code)-1):
        if code[i:i+2] != b'\xf0\0':
            continue
        if len(candidates) >= 4096:
            raise ValueError('too many native-entry candidates')
        end = min(i+130, len(code))
        reentry = None
        for j in range(i+2, end-10):
            if code[j] == 0x68 and code[j+5] == 0x68 and code[j+10] == 0xc3:
                shape_target, handler = unpack(code, '<I', j+1)[0], unpack(code, '<I', j+6)[0]
                if aliases.get(handler) == 'do_start_interp' and row['va'] <= shape_target < row['va']+len(code):
                    end = j+11
                    reentry = shape_target
                    break
        refs = [name for va, name in aliases.items()
                if struct.pack('<I', va) in code[i+2:end]]
        candidates.append({'code_offset': i, 'end_offset': end,
                           'reentry_va': reentry, 'import_reference_candidates': refs})
    return {'sha256': hashlib.sha256(data).hexdigest(), 'code_va': row['va'],
            'imports': imports, 'aliases': aliases, 'native_candidates': candidates,
            'limit': 'Byte-pattern candidates, not reachability or absence proof'}, code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('shape', type=Path)
    parser.add_argument('--disassembly', type=Path, help='new local output file; requires GNU objdump')
    args = parser.parse_args()
    if args.shape.stat().st_size > 16 * 1024 * 1024:
        raise ValueError('shape exceeds 16 MiB')
    report, code = inspect(args.shape.read_bytes())
    if args.disassembly:
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp)/'shape-code.bin'
            binary.write_bytes(code)
            with args.disassembly.open('x') as output:
                for entry in report['native_candidates']:
                    output.write(f"\nCandidate CODE+{entry['code_offset']:x}\n")
                    result = subprocess.run([
                        'objdump', '-D', '-b', 'binary', '-m', 'i386', '-M', 'intel',
                        f"--start-address={entry['code_offset']+2}",
                        f"--stop-address={entry['end_offset']}", str(binary)],
                        check=True, capture_output=True, text=True)
                    output.write(result.stdout.split('Disassembly of section .data:')[-1])
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
