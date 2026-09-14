"""Read-only PE32/SMS flight research. Disassembles data; never runs retail code."""
import argparse
import bisect
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess

REVIEWED_SMS = 'e550a67e2dca36c583a5e7963db96da7a833e79a2b5cd13e5da4c2d966168de0'
REVIEWED_FA = 'e31560c2a6d6adb4aa1493f0308f6ae5640f67a4e886dbdf5887489e6e99244c'


def unpack(data, fmt, offset):
    size = struct.calcsize(fmt)
    if offset < 0 or offset + size > len(data):
        raise ValueError('truncated native metadata')
    return struct.unpack_from(fmt, data, offset)


def symbols(data):
    count, = unpack(data, '<I', 0)
    pool = 4 + count * 8
    if not 0 < count <= 100000 or pool > len(data):
        raise ValueError('invalid SMS symbol count')
    result = []
    for i in range(count):
        offset, address = unpack(data, '<II', 4 + i * 8)
        start = pool + offset
        end = data.find(b'\0', start, start + 1025)
        if start >= len(data) or end < start:
            raise ValueError('invalid SMS string offset or terminator')
        name = data[start:end].decode('ascii')
        if not name or any(ord(c) < 32 or ord(c) > 126 for c in name):
            raise ValueError('invalid SMS name')
        result.append({'name': name, 'va': address})
    names = [s['name'] for s in result]
    if names != sorted(set(names)):
        raise ValueError('SMS names must be unique and sorted')
    return result


def sections(data):
    if data[:2] != b'MZ':
        raise ValueError('expected MZ executable')
    pe, = unpack(data, '<I', 60)
    if data[pe:pe+4] != b'PE\0\0':
        raise ValueError('expected PE signature')
    machine, count = unpack(data, '<HH', pe+4)
    opt_size, = unpack(data, '<H', pe+20)
    opt = pe+24
    magic, = unpack(data, '<H', opt)
    base, = unpack(data, '<I', opt+28)
    if machine != 0x14c or magic != 0x10b or not 1 <= count <= 96 or opt_size < 96:
        raise ValueError('expected bounded PE32 i386 image')
    rows = []
    for i in range(count):
        at = opt+opt_size+i*40
        vsize, rva, size, raw = unpack(data, '<IIII', at+8)
        flags, = unpack(data, '<I', at+36)
        if raw+size > len(data) or base+rva+max(size,vsize) > 2**32:
            raise ValueError('PE section outside file/address bounds')
        rows.append({'va': base+rva, 'size': size, 'raw': raw,
                     'name': data[at:at+8].rstrip(b'\0').decode('ascii'),
                     'executable': bool(flags & 0x20000000)})
    return rows


def field_addresses(repo, base):
    """Derive packed PT offsets from the same Rust schema used by extraction."""
    schema = (repo/'crates/tore-formats/src/aircraft_schema.rs').read_text()
    result, offset = [], 0
    sizes = {'byte': 1, 'word': 2, 'dword': 4, 'ptr': 4, 'symbol': 4}
    for section in ('OBJECT', 'NPC', 'PLANE'):
        block = schema.split('pub const '+section+':', 1)[1].split('];', 1)[0]
        fields = re.findall(r'\("(\w+)", "([^"]+)"\)', block)
        if not fields:
            raise ValueError('unrecognized aircraft schema')
        for kind, name in fields:
            result.append({'section': section, 'field': name, 'offset': offset,
                           'va': base+offset, 'width': sizes[kind]})
            offset += sizes[kind]
    return result


def extract(source, output, *, overwrite=False, preview=False):
    repo = Path(__file__).resolve().parents[1]
    source, output = source.resolve(), output.resolve()
    if not source.is_dir() or output.is_relative_to(source):
        raise ValueError('native research requires a media directory and an output outside it')
    files = {p.name.upper(): p for p in source.iterdir() if p.is_file()}
    if 'FA.EXE' not in files or 'FA.SMS' not in files:
        raise ValueError('source directory must contain FA.EXE and FA.SMS')
    for name in ('FA.EXE', 'FA.SMS'):
        if files[name].stat().st_size > 64*1024*1024:
            raise ValueError('native research input exceeds 64 MiB')
    exe, sms = (files[n].read_bytes() for n in ('FA.EXE', 'FA.SMS'))
    rows, names = sections(exe), symbols(sms)
    executable = lambda va: next((r for r in rows if r['executable'] and r['va'] <= va < r['va']+r['size']), None)
    code = sorted({s['va'] for s in names if executable(s['va'])})
    selected = [s for s in names if executable(s['va']) and any(t in s['name'].lower() for t in
        ('fm', 'plane', 'envelope', 'stickinput', 'gtoturn', 'matchf24', 'turntoward', 'ground', 'fuel', 'cobv', 'cobrv', 'cothrust', 'codrag', 'copull', 'cospeed', 'timeupdate', 'instaltimer', 'installtimer', 'stall', 'landing'))]
    report = {'schema_version': 1, 'method': 'static disassembly only; no retail execution',
              'exe_sha256': hashlib.sha256(exe).hexdigest(), 'sms_sha256': hashlib.sha256(sms).hexdigest(),
              'symbol_count': len(names), 'sections': rows, 'selected_symbol_count': len(selected)}
    report['reviewed_fa_build'] = (report['exe_sha256'] == REVIEWED_FA and report['sms_sha256'] == REVIEWED_SMS)
    if preview:
        print(json.dumps(report, indent=2))
        return
    objdump = shutil.which('llvm-objdump') or shutil.which('objdump')
    if not objdump:
        raise ValueError('install LLVM objdump (macOS command line tools include it)')
    result = subprocess.run([objdump, '-d', '--x86-asm-syntax=intel', str(files['FA.EXE'])],
                            capture_output=True, text=True, check=True, timeout=120)
    disassembly = result.stdout
    if len(disassembly) > 128*1024*1024:
        raise ValueError('disassembly exceeds bound')
    instructions = []
    for line in disassembly.splitlines():
        match = re.match(r'\s*([0-9a-fA-F]+):\s', line)
        if match:
            instructions.append((int(match[1], 16), line))
    if not instructions:
        raise ValueError('objdump produced no recognized instructions')
    addresses = [a for a, _ in instructions]
    artifacts = {'symbols.json': json.dumps(names, indent=2), 'fa-disassembly.txt': disassembly}
    spans = []
    for symbol in selected:
        va = symbol['va']; section = executable(va)
        i = bisect.bisect_right(code, va)
        end = min(code[i] if i < len(code) else section['va']+section['size'], section['va']+section['size'])
        raw = section['raw']+va-section['va']
        lines = instructions[bisect.bisect_left(addresses,va):bisect.bisect_left(addresses,end)]
        calls = sorted({int(m.group(1),16) for _,line in lines if (m:=re.search(r'\bcall\s+0x([0-9a-fA-F]+)',line))})
        spans.append({**symbol, 'end': end, 'sha256': hashlib.sha256(exe[raw:raw+end-va]).hexdigest(),
                      'direct_calls': calls, 'boundary': 'next exported SMS address, may include unnamed helpers'})
        artifacts[f'spans/{va:08x}.txt'] = '\n'.join(line for _,line in lines)+'\n'
    report['spans'] = spans
    # These addresses are only identified for the hash-reviewed FA build.
    if report['reviewed_fa_build']:
        cpt = next(s['va'] for s in names if s['name']=='_cpt')
        fields = field_addresses(repo,cpt)
        references = {}
        for address, line in instructions:
            for literal in set(re.findall(r'\b0x([0-9a-fA-F]+)\b', line)):
                references.setdefault(int(literal, 16), []).append(address)
        for field in fields:
            field['direct_references'] = references.get(field['va'], [])
        artifacts['pt-field-references.json'] = json.dumps(fields,indent=2)
    artifacts['inventory.json'] = json.dumps(report,indent=2)+'\n'
    # Preflight every output before changing anything. Never overwrite differing
    # research output unless explicitly requested; filenames never contain symbols.
    for name, content in artifacts.items():
        path = output/name
        if not path.resolve().is_relative_to(output):
            raise ValueError('output symlink escapes research directory')
        if path.exists() and path.read_text()!=content and not overwrite:
            raise ValueError(f'differing output {path}; choose a new --out or --overwrite')
    for name, content in artifacts.items():
        path=output/name;path.parent.mkdir(parents=True,exist_ok=True)
        path.write_text(content)
    print(f"Native flight research: {len(selected)} symbol spans, {len(names)} symbols; {output}")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--overwrite',action='store_true')
    parser.add_argument('--dry-run',action='store_true')
    args=parser.parse_args()
    try:
        extract(args.source,args.out,overwrite=args.overwrite,preview=args.dry_run)
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        parser.exit(1,f'{error}\n')

if __name__=='__main__':
    main()
