"""Export reviewed F22 donors as a separate experimental FA F/A-XX aircraft. No retail code execution.

Requires the patched static OpenFA build. Outputs are local retail derivatives.
The original drawing records stay at their addresses; authored endpoint routines
are appended before the end marker. See docs/spec/fa-xx-export.md.
"""
import argparse
import ast
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import subprocess

from inspect_shape_effects import inspect
from openfa_tools import require_static
from fa_lib import pack_stored

ROOT = Path(__file__).resolve().parents[1]
DONORS = {
    'F22.SH': ('eba06b716e45431fa7401d99579eb0d4c8f1882eb80928c726186907c170f8b6', [0x34f7, 0x351e, 0x354a, 0x37ff, 0x381e, 0x3841, 0x386f]),
    'F22_A.SH': ('4d06b11b332f68981fd5613ca048190a601c84974b4e3edf79ad238a60a24fdb', [0x3365, 0x3388]),
    'F22_C.SH': ('c9a280ee6470fad95cd65fe497af2cb5c9543aeb6a7a3fd7d332c2c0922dfdf3', [0x2abf, 0x2ae2, 0x2c8a, 0x2cad]),
}
FLAPS = [0x437f, 0x439e, 0x43fb, 0x4416, 0x4576, 0x4599, 0x45f6]


def field(block, name):
    return re.search(r'^  ' + name + r': (.*(?:\n    [^\n]+)*)', block, re.M).group(1).replace('\n    ', ' ')


def block(name, data, old=''):
    uid = re.search(r'^  uuid: .*$', old, re.M)
    return f'- name: {name}\n' + (uid.group(0) + '\n' if uid else '') + '  data: ' + data.hex(' ').upper() + '\n'


def jump(start, target):
    return b'\x48\x00' + struct.pack('<h', target - start - 4)


def stub(offset, size, target, old):
    # The unreachable final jump separates padding, preserving following Pad targets.
    if size < 8:
        return block('X86Unknown', jump(offset, target), old)
    return block('X86Unknown', jump(offset, target) + b'\x1e'*(size-8) +
                 jump(offset+size-4, offset+size), old)


def vb(indices, positions):
    # Individual records preserve non-contiguous donor slot numbering.
    return b''.join(b'\x82\x00' + struct.pack('<HHhhh', 1, i * 8, *map(round, p))
                    for i, p in zip(indices, positions))


def turn(p, angle, pivot=(0, -22, 0)):
    x, y, z = [p[i] - pivot[i] for i in range(3)]
    c, s = math.cos(angle), math.sin(angle)
    return [x + pivot[0], c*y - s*z + pivot[1], s*y + c*z + pivot[2]]


def moved_face(raw, positions, angle):
    result = bytearray(raw)
    if raw[1] & 0x60:
        n = struct.unpack_from('<hhh', raw, 5)
        n = turn([n[0], n[2], n[1]], angle, (0, 0, 0))
        struct.pack_into('<hhh', result, 5, round(n[0]), round(n[2]), round(n[1]))
        center = [round(sum(p[i] for p in positions)/len(positions)) for i in (0, 2, 1)]
        struct.pack_into('<bbb' if raw[2] & 2 else '<hhh', result, 11, *center)
    return bytes(result)


class Extension:
    def __init__(self, start):
        self.start = start
        self.parts = []
        self.size = 0

    @property
    def at(self):
        return self.start + self.size

    def raw(self, data):
        self.parts.append(('raw', data))
        self.size += len(data)

    def guard(self, symbol, value, yes, no):
        # Two short re-entry blocks, following the reviewed SH state guard grammar.
        self.parts.append(('guard', (symbol, value, yes, no)))
        self.size += 36

    def yaml(self, aliases):
        out = ''
        for kind, data in self.parts:
            if kind == 'raw':
                out += block('X86Unknown', data)
            else:
                symbol, value, yes, no = data
                for code, relocs in [
                    (b'\x66\x83\x3d' + struct.pack('<I', 0xaa000000 + aliases[symbol]) +
                     bytes([value & 255, 0x75, 13]) + b'\x68' + struct.pack('<I', 0xaa000000 + yes) +
                     b'\x68' + struct.pack('<I', 0xaa000000 + aliases['do_start_interp']) + b'\xc3', [5, 13, 18]),
                    (b'\x68' + struct.pack('<I', 0xaa000000 + no) + b'\x68' +
                     struct.pack('<I', 0xaa000000 + aliases['do_start_interp']) + b'\xc3', [3, 8]),
                ]:
                    out += ('- name: X86Code\n  usage:\n    static: true\n  have_header: true\n'
                            f'  bytecode: {code.hex(" ").upper()}\n  x86_relocs: {relocs}\n')
        return out


def hook_geometry():
    length = (18 - 1.3*math.cos(.75))/math.sin(.75)-1
    result = bytearray(b'\xe0\x00\x00\x00')
    for lo, hi in [([-.45, -26-length, -5.45], [.45, -26, -4.55]),
                   ([-.9, -27-length, -6.3], [.9, -23-length, -5])]:
        for axis in range(3):
            u, v = (axis+1)%3, (axis+2)%3
            for upper in (False, True):
                points = []
                for a, b in [(0, 0), (1, 0), (1, 1), (0, 1)]:
                    p = lo.copy()
                    p[axis] = hi[axis] if upper else lo[axis]
                    p[u] = hi[u] if a else lo[u]
                    p[v] = hi[v] if b else lo[v]
                    p = turn(p, .75, (0, -26, -5))
                    # Keep the thin shank from collapsing at integer SH precision.
                    p[0] = math.copysign(max(1, round(abs(p[0]))), p[0])
                    points.append(p)
                if not upper:
                    points.reverse()
                n = [0, 0, 0]
                n[axis] = 32765 if upper else -32765
                n = turn(n, .75, (0, 0, 0))
                center = [round(sum(p[i] for p in points)/4) for i in (0, 2, 1)]
                result += vb(range(4), points)
                result += b'\xfc\x60\x02\x37\x00' + struct.pack('<hhhbbb',
                    round(n[0]), round(n[2]), round(n[1]), *center) + b'\x04\x00\x01\x02\x03'
    return bytes(result)


def modify(text, original, faces, main):
    header, *blocks = re.split(r'(?=^- name: )', text, flags=re.M)
    offsets = [int(field(b, 'code_offset0'), 16) for b in blocks]
    sizes = [b-a for a, b in zip(offsets, offsets[1:])] + [6]
    _, code = inspect(original)
    start_index = next(i for i, b in enumerate(blocks) if b.startswith('- name: EndShape\n'))
    start = offsets[start_index]
    extension = Extension(start)
    replaced = {}
    masks = DONORS[main][1]
    for i, (b, offset, size) in enumerate(zip(blocks, offsets, sizes)):
        if offset + 0x1000 in masks or b.startswith('- name: JumpToLOD\n'):
            following = i + 1
            while blocks[following].startswith('- name: Pad\n'):
                following += 1
            replaced[i] = stub(offset, size, offsets[following], b)
        if main == 'F22.SH' and offset + 0x1000 in FLAPS:
            raw = code[offset:offset+size]
            following = i + 1
            while blocks[following].startswith('- name: Pad\n'):
                following += 1
            resume = offset + size
            indices = ([int(x) for x in re.findall(r'^  - (\d+)$', b.split('  indices:\n', 1)[1].split('  tex_coords:', 1)[0], re.M)]
                       if '  indices:\n' in b else ast.literal_eval(field(b, 'indices')))
            points = faces[offset+0x1000]['positions']
            sign = 1 if sum(p[0] for p in points) > 0 else -1
            routine = extension.at
            def geometry(angles):
                body = bytearray()
                for angle in angles:
                    moved = [turn(p, angle) for p in points]
                    body += vb(indices, moved) + moved_face(raw, moved, angle)
                return bytes(body + vb(indices, points))
            poses = [(geometry([mid-.6, mid+.6]), geometry([mid])) for mid in (0., .4)]
            up_length = 36 + len(poses[0][0]) + 4 + len(poses[0][1]) + 4
            extension.guard('_PLrightFlap' if sign > 0 else '_PLleftFlap', -1,
                            routine+36+up_length, routine+36)
            for split, closed in poses:
                current = extension.at
                normal = current + 36 + len(split) + 4
                extension.guard('_PLrudder', sign, current+36, normal)
                extension.raw(split + jump(extension.at + len(split), resume))
                extension.raw(closed + jump(extension.at + len(closed), resume))
            replaced[i] = stub(offset, size, routine, b)
        if main == 'F22.SH' and b.startswith('- name: SourceName\n'):
            routine = extension.at
            geometry = hook_geometry()
            extension.guard('_PLhook', 1, routine+36, offset+size)
            extension.raw(geometry + jump(extension.at+len(geometry), offset+size))
            replaced[i] = stub(offset, size, routine, b)
    if main != 'F22.SH':
        return header + ''.join(replaced.get(i, b) for i, b in enumerate(blocks)), {'fins': masks}
    # Preserve original instruction offsets. Only the end marker and import aliases move.
    shift = extension.size
    tramps = [(i, field(b, 'trampoline')) for i, b in enumerate(blocks) if b.startswith('- name: X86Trampoline\n')]
    aliases = {name: offsets[i]+shift for i, name in tramps}
    extra = ['_PLhook', '_PLrudder']
    last = offsets[-1]+shift+6
    aliases.update({name: last+i*6 for i, name in enumerate(extra)})
    import_count = len(tramps)+len(extra)
    code_length = last+len(extra)*6
    idata = ((0x1000+code_length+4095)//4096)*4096
    iat = idata + 40 + (import_count+2)*4
    for i, b in enumerate(blocks):
        if b.startswith('- name: X86Code\n'):
            raw = bytearray(bytes.fromhex(field(b, 'bytecode')))
            header_size = 2 if field(b, 'have_header') == 'true' else 0
            for reloc in ast.literal_eval(field(b, 'x86_relocs')):
                at = reloc-header_size
                address = struct.unpack_from('<I', raw, at)[0]
                if 0xaa000000+start <= address < 0xaa000000+len(code):
                    struct.pack_into('<I', raw, at, address+shift)
            if raw[:3] == b'\x66\x83\x3d':
                referenced = struct.unpack_from('<I', raw, 3)[0]
                if referenced in [0xaa000000+aliases[n] for n in ('_PLleftFlap', '_PLrightFlap')]:
                    if raw[7] not in (0, 1, 255):
                        raise ValueError('unreviewed donor flap guard')
                    if raw[7] == 0:
                        raw[8] = 0x74 if raw[8] == 0x75 else 0x75
                    raw[7] = 127
            b = re.sub(r'^  bytecode: .*?(?=\n  x86_relocs:)', '  bytecode: '+raw.hex(' ').upper(), b, flags=re.M|re.S)
            blocks[i] = b
    for j, (i, name) in enumerate(tramps):
        blocks[i] = block('X86Trampoline', b'\xff\x25'+struct.pack('<I', iat+j*4)) + f'  trampoline: {name}\n'
    tail = ''.join(block('X86Trampoline', b'\xff\x25'+struct.pack('<I', iat+(len(tramps)+j)*4)) +
                   f'  trampoline: {name}\n' for j, name in enumerate(extra))
    result = header
    for i, b in enumerate(blocks):
        if i == start_index:
            result += extension.yaml(aliases)
        result += replaced.get(i, b)
    return result+tail, {'fins': masks, 'flaps': FLAPS, 'aliases': {k: v+0x1000 for k,v in aliases.items()},
                         'extension_bytes': shift}


IDENTITY_DONORS = {
    'F22.PT': 'e6b0009e2cfd48b53f18a80abcf2ae41d2c77bb8c66a5d7401b5ad0a62f86a15',
    'F22_B.SH': '69caaac4a05f41e52d85774fe9e95f4ad4368899aae5ea9305163a312ff19460',
    'F22_D.SH': '68d7c820cf0bfea4da15d97493ede5fd5f2175181f0e3b22dea038a4845018a8',
    'F22_S.SH': '4be45511cb9b2d7d2a72a9eb2ee516d584115b375328eb18b352d2f1368726ed',
}


def hook_equipped_pt(data):
    """Enable the reviewed PLANE_TYPE hook bit, preserving all other flags."""
    text = data.decode('ascii')
    pattern = r'(?m)(^;[- ]*START OF PLANE_TYPE[- ]*\r?\n[ \t\r\n]*dword[ \t]+\$)([0-9A-Fa-f]+)(?=[ \t\r\n;]|$)'
    matches = list(re.finditer(pattern, text))
    if len(matches) != 1 or int(matches[0][2], 16) not in (0x91, 0x93):
        raise ValueError('unreviewed F22 plane capability flags')
    flags = matches[0]
    return (text[:flags.start(2)] + '93' + text[flags.end(2):]).encode('ascii')


def independent_pt(data):
    """Give the concept its own identity/shape family and enable its hook."""
    text = data.decode('ascii')
    replacements = {
        'ot_names': ['F/A-XX', 'F/A-XX Concept', 'FAXX.PT'],
        'shape': ['FAXX.SH'],
        'shadowShape': ['FAXX_S.SH'],
    }
    for label, values in replacements.items():
        pattern = r'(?m)^:' + label + r'\r?\n(?:(?:[ \t]*string "[^"\r\n]*"[ \t]*\r?\n))+'
        match = re.search(pattern, text)
        if match is None or len(re.findall(r'string "', match[0])) != len(values):
            raise ValueError(f'unreviewed PT block: {label}')
        newline = '\r\n' if '\r\n' in match[0] else '\n'
        replacement = ':' + label + newline + ''.join('\tstring "'+v+'"'+newline for v in values)
        text = text[:match.start()] + replacement + text[match.end():]
    return hook_equipped_pt(text.encode('ascii'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tool', type=Path, required=True)
    parser.add_argument('--donors', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--identity', choices=['faxx','f22'], default='faxx',
                        help='Separate F/A-XX (default), or the earlier F-22 replacement')
    args = parser.parse_args()
    tool, out = args.tool.resolve(), args.out.resolve()
    require_static(tool)
    inputs = {name: (args.donors/name).read_bytes() for name in DONORS}
    for name, data in inputs.items():
        if hashlib.sha256(data).hexdigest() != DONORS[name][0]:
            parser.error(f'unreviewed donor {name}')
    identity_inputs = {}
    for name, expected in IDENTITY_DONORS.items():
        if args.identity == 'faxx' or name == 'F22.PT':
            data = (args.donors/name).read_bytes()
            if hashlib.sha256(data).hexdigest() != expected:
                parser.error(f'unreviewed identity donor {name}')
            identity_inputs[name] = data
    out.mkdir(parents=True, exist_ok=False)
    faces = json.loads(subprocess.check_output(['cargo', 'run', '--locked', '-q', '-p', 'tore-extract',
        '--example', 'shape_json', '--', str((args.donors/'F22.SH').resolve())], cwd=ROOT))
    faces = {f['address']: f for f in faces}
    report = {'status': 'experimental, original-game operation unverified',
              'tool_sha256': hashlib.sha256(tool.read_bytes()).hexdigest(),
              'exporter_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              'identity': args.identity, 'shapes': {}}
    for name, original in inputs.items():
        shape = out/name
        shape.write_bytes(original)
        subprocess.run([str(tool), str(shape)], check=True, timeout=60)
        yaml = shape.with_suffix('.SH.yaml')
        updated, details = modify(yaml.read_text(), original, faces, name)
        yaml.write_text(updated)
        shape.unlink()
        subprocess.run([str(tool), str(yaml)], check=True, timeout=60)
        details['donor_sha256'] = hashlib.sha256(original).hexdigest()
        details['sha256'] = hashlib.sha256(shape.read_bytes()).hexdigest()
        report['shapes'][name] = details
    resources = list(DONORS)
    if args.identity == 'faxx':
        resources = []
        shapes = {}
        for name, details in report['shapes'].items():
            renamed = name.replace('F22', 'FAXX', 1)
            (out/name).rename(out/renamed)
            shapes[renamed] = details
            resources.append(renamed)
        report['shapes'] = shapes
        report['identity_resources'] = {}
        for name, original in identity_inputs.items():
            renamed = name.replace('F22', 'FAXX', 1)
            data = independent_pt(original) if name.endswith('.PT') else original
            (out/renamed).write_bytes(data)
            resources.append(renamed)
            report['identity_resources'][renamed] = {
                'donor_sha256': hashlib.sha256(original).hexdigest(),
                'sha256': hashlib.sha256(data).hexdigest(),
            }
    else:
        original = identity_inputs['F22.PT']
        data = hook_equipped_pt(original)
        (out/'F22.PT').write_bytes(data)
        resources.append('F22.PT')
        report['identity_resources'] = {'F22.PT': {
            'donor_sha256': hashlib.sha256(original).hexdigest(),
            'sha256': hashlib.sha256(data).hexdigest(),
        }}
    report['hook_capability'] = {'donor_flags': '0x91', 'exported_flags': '0x93', 'enabled_bit': '0x02'}
    (out/'export-report.json').write_text(json.dumps(report, indent=2)+'\n')
    # Import here to avoid a cycle in the independent validator's CLI.
    from validate_faxx_export import validate
    validate(args.donors.resolve(), out)
    pack_stored([out/name for name in resources], out/'FAXX.LIB')
    subprocess.run(['cargo', 'run', '--locked', '-q', '-p', 'tore-extract', '--example',
                    'check_lib', '--', str(out/'FAXX.LIB'),
                    *[str(out/name) for name in resources]], cwd=ROOT, check=True, timeout=60)
    unpacked = out/'archive-check'
    subprocess.run([str(tool), 'lib', 'unpack', '-o', str(unpacked), str(out/'FAXX.LIB')],
                   check=True, timeout=60)
    for name in resources:
        if (unpacked/name).read_bytes() != (out/name).read_bytes():
            raise ValueError(f'archive round trip failed: {name}')
    readme_name = 'faxx-independent-readme.txt' if args.identity == 'faxx' else 'faxx-package-readme.txt'
    readme = (ROOT/'tools/openfa'/readme_name).read_text()
    (out/'README.txt').write_text(readme)
    import zipfile
    payloads = ['README.txt', 'export-report.json', 'validation.json', 'FAXX.LIB', *resources]
    with zipfile.ZipFile(out/'F-A-XX-FA-experimental.zip', 'x', zipfile.ZIP_DEFLATED) as archive:
        for name in payloads:
            archive.write(out/name, arcname='F-A-XX-FA-experimental/'+name)
    print(out/'F-A-XX-FA-experimental.zip')


if __name__ == '__main__':
    main()
