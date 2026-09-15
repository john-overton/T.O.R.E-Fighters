"""Reviewed inert creator table grammar. Called only for the verified FA pair."""
import hashlib
import json
import struct

# Explicit code boundaries avoid linear-disassembler misalignment after jump tables.
ALIGNED_REGIONS = (
    ('option_dispatch', 0x42e720, 0x42e86c),
    ('selector_interaction', 0x430680, 0x43089c),
    ('briefing_geometry', 0x42fde0, 0x4300d1),
    ('creator_initialization', 0x42f2e0, 0x42f877),
    ('creator_nationalities', 0x4308a0, 0x4309f9),
    ('creator_field_updates', 0x4301a0, 0x4303d5),
    ('catalog_filter', 0x41d209, 0x41d384),
    ('ordnance_controls', 0x41b0d1, 0x41b316),
    ('ordnance_quantity', 0x41b4d2, 0x41b686),
    ('ordnance_cheat_toggle', 0x41be33, 0x41be90),
    ('ordnance_capacity_wrapper', 0x41c3a9, 0x41c457),
    ('ordnance_station_hit', 0x41c460, 0x41c4e1),
    ('ordnance_catalog_hit', 0x41c4f0, 0x41c58a),
    ('ordnance_entry_wrapper', 0x47fa50, 0x47fa97),
    ('ordnance_catalog_eligibility', 0x419cfa, 0x419f4c),
    ('ordnance_card_draw', 0x41c610, 0x41c6f7),
    ('ordnance_catalog_sort', 0x41c700, 0x41c81d),
)


def read_va(data, sections, va, size, *, executable):
    if not 0 < size <= 16384:
        raise ValueError('menu data read exceeds bound')
    section = next((s for s in sections if s['executable'] == executable and
                    s['va'] <= va < va + size <= s['va'] + s['size']), None)
    if section is None:
        raise ValueError('menu data address outside expected section')
    raw = section['raw'] + va - section['va']
    if raw < 0 or raw + size > len(data):
        raise ValueError('truncated menu table')
    return data[raw:raw + size]


def string_list(data, sections, va):
    values, item = [], bytearray()
    for offset in range(8192):
        value = read_va(data, sections, va + offset, 1, executable=False)[0]
        if value == 0:
            if not item:
                if not values:
                    raise ValueError('empty option list')
                return values
            values.append(item.decode('ascii'))
            item.clear()
            if len(values) > 256:
                raise ValueError('too many menu options')
        elif not 32 <= value <= 126 or len(item) >= 160:
            raise ValueError('invalid option string')
        else:
            item.append(value)
    raise ValueError('unterminated menu option list')


def literal_list(data, sections, branch):
    # Interpret only MOV EAX,imm32; RET as a constant pointer record.
    code = read_va(data, sections, branch, 6, executable=True)
    if code[0] != 0xb8 or code[5] != 0xc3:
        raise ValueError('unreviewed option pointer grammar')
    pointer, = struct.unpack_from('<I', code, 1)
    return {'table_va': pointer, 'values': string_list(data, sections, pointer)}


def ordnance_controls(data, sections):
    """Read the reviewed five-entry action dispatch, without invoking handlers."""
    raw = read_va(data, sections, 0x41b336, 20, executable=True)
    meanings = {
        0x41b0d1: 'page rocker',
        0x41b139: 'category bank one',
        0x41b18d: 'category bank two',
        0x41b1e1: 'internal fuel rocker',
        0x41b2d6: 'Fly weight check',
    }
    branches = struct.unpack('<5I', raw)
    if set(branches) != set(meanings):
        raise ValueError('unreviewed ordnance action dispatch')
    return {'dispatch_va': 0x41b336, 'sha256': hashlib.sha256(raw).hexdigest(),
            'controls': [{'action_id': i, 'branch_va': branch,
                          'meaning': meanings[branch]}
                         for i, branch in enumerate(branches, 1)]}


def artifacts(data, sections):
    dispatch = read_va(data, sections, 0x42e86c, 60 * 4, executable=True)
    targets = read_va(data, sections, 0x42e95c, 16 * 4, executable=True)
    geometry = read_va(data, sections, 0x4f1d30, 29 * 10, executable=False)
    fields = []
    dynamic = {0x42e747: 'aircraft catalog: player filter for ID 6, other-wing filter otherwise',
               0x42e80e: 'aircraft catalog: multiplayer player filter',
               0x42e799: 'ground targets indexed by theater'}
    for field, branch in enumerate(struct.unpack('<60I', dispatch), 3):
        row = {'field_id': field, 'branch_va': branch,
               'mode': 'single-player briefing' if field <= 32 else 'multiplayer extension'}
        if branch in dynamic:
            row['producer'] = dynamic[branch]
        else:
            row.update(literal_list(data, sections, branch))
        fields.append(row)
    ground = []
    for theater, branch in enumerate(struct.unpack('<16I', targets)):
        ground.append({'theater_index': theater, 'branch_va': branch,
                       **literal_list(data, sections, branch)})
    rectangles = []
    for line, words in enumerate(struct.iter_unpack('<5h', geometry)):
        x, solo_y, multi_y, width, height = words
        if min(x, solo_y, multi_y) < 0 or width <= 0 or height <= 0:
            raise ValueError('invalid reviewed briefing rectangle')
        rectangles.append({'line': line, 'x': x, 'single_y': solo_y,
                           'multi_y': multi_y, 'width': width, 'height': height})
    report = {
        'schema_version': 1,
        'status': 'static selector dispatch verified; defaults/eligibility are separate',
        'consumer_va': 0x42e720, 'dispatch_va': 0x42e86c,
        'dispatch_sha256': hashlib.sha256(dispatch).hexdigest(),
        'fields': fields, 'ground_targets': ground,
        'geometry': {'table_va': 0x4f1d30, 'consumer_va': 0x42fde0,
                     'sha256': hashlib.sha256(geometry).hexdigest(),
                     'coordinates': 'relative to current dialog origin; text draw uses y minus one',
                     'lines': rectangles},
    }
    return {'creator-options.json': json.dumps(report, indent=2) + '\n',
            'ordnance-controls.json': json.dumps(ordnance_controls(data, sections),
                                                indent=2) + '\n'}
