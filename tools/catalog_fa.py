"""Inventory the Fighters Anthology aircraft (PT) and weapon (JT) catalogs as CSV.

Reads user-owned archives through the existing extractor, decodes the BRF records
with the field order held in `crates/tore-formats/src/aircraft_schema.rs`, and
writes two research catalogs. No retail code runs and no retail media is copied
into the repository. See docs/formats/fa-catalog.md for the column contract.
"""
import argparse
import csv
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / 'crates/tore-formats/src/aircraft_schema.rs'
# Shared with crates/tore-sim: telemetry.rs FPS_PER_KNOT and missiles.rs NMI.
FPS_PER_KNOT = 1.6878098571
FEET_PER_NMI = 6076.
# One projectile timer unit, as the live host maps it. See docs/formats/missiles.md.
TIMER_SECONDS = 0.25
# Ordnance screen category bank one, from projectile flags. The two dial labels
# are not recovered; only the split is. See docs/formats/ordnance-menu.md.
BANK_ONE_FLAG = 0x10000


class Token:
    """One BRF statement: its kind, its literal operand and the caret marker."""
    __slots__ = ('kind', 'value', 'scaled')

    def __init__(self, kind, value, scaled):
        self.kind, self.value, self.scaled = kind, value, scaled

    def number(self):
        """Truncate and sign fold exactly as the Rust reader does: bytes stay
        unsigned, words and dwords are signed."""
        width = {'byte': 8, 'word': 16, 'dword': 32}.get(self.kind)
        if width is None:
            raise ValueError(f'expected a numeric BRF statement, got {self.kind}')
        raw = int(self.value[1:], 16) if self.value.startswith('$') else int(self.value)
        if not -(1 << 31) <= raw <= (1 << 32) - 1:
            raise ValueError('BRF number exceeds 32 bits')
        value = raw & ((1 << width) - 1)
        if width > 8 and value >= 1 << (width - 1):
            value -= 1 << width
        return value


def parse_brf(data):
    """Return the labelled statement blocks of one BRF resource."""
    text = data.decode('latin-1')
    if not text.startswith("[brent's_relocatable_format]"):
        raise ValueError('not BRF data')
    blocks, label, ended = {'': []}, '', False
    for line in text.splitlines()[1:]:
        line = line.split(';', 1)[0].strip()
        if not line:
            continue
        if ended:
            raise ValueError('BRF data after end')
        if line == 'end':
            ended = True
            continue
        if line.startswith(':'):
            label = line[1:]
            if not label or label in blocks:
                raise ValueError(f'invalid or duplicate BRF label: {label}')
            blocks[label] = []
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            raise ValueError(f'invalid BRF statement: {line}')
        kind, value = parts[0], parts[1].strip()
        if kind not in ('byte', 'word', 'dword', 'ptr', 'symbol', 'string'):
            raise ValueError(f'unknown BRF statement: {kind}')
        scaled = value.startswith('^')
        if scaled:
            value = value[1:]
        if kind == 'string':
            if not (value.startswith('"') and value.endswith('"')):
                raise ValueError('invalid BRF string')
            value = value[1:-1]
        blocks[label].append(Token(kind, value, scaled))
    if not ended:
        raise ValueError('unterminated BRF')
    return blocks


def layouts(path=SCHEMA):
    """Read the recovered field order from the Rust schema, its one home."""
    source = path.read_text()
    result = {}
    for name in ('OBJECT', 'NPC', 'PLANE', 'PROJECTILE', 'HARDPOINT', 'ENVELOPE'):
        match = re.search(r'pub const %s: &\[\(&str, &str\)\] = &\[(.*?)\n\];' % name, source, re.S)
        if not match:
            raise ValueError(f'{path}: missing {name} layout')
        result[name] = re.findall(r'\("(\w+)", "([^"]+)"\)', match.group(1))
    return result


def fields(tokens, layout):
    """Name a run of statements through one recovered layout."""
    if len(tokens) != len(layout):
        raise ValueError(f'expected {len(layout)} statements, found {len(tokens)}')
    return {name: token for token, (_, name) in zip(tokens, layout)}


def strings(blocks, token):
    """Resolve a pointer statement to the strings of the block it names."""
    if token.kind != 'ptr':
        return []
    return [t.value for t in blocks.get(token.value, []) if t.kind == 'string']


def knots(feet_per_second):
    return round(feet_per_second / FPS_PER_KNOT)


def miles(feet):
    return round(feet / FEET_PER_NMI, 2)


def envelope_rows(blocks, plane, layout):
    """Return the recovered G rows as (load, [(speed ft/s, altitude ft)])."""
    tokens = blocks.get('env', [])
    width = len(layout['ENVELOPE'])
    if not tokens or len(tokens) % width:
        raise ValueError('invalid G envelope block')
    rows = []
    for chunk in [tokens[i:i + width] for i in range(0, len(tokens), width)]:
        row = fields(chunk, layout['ENVELOPE'])
        count = row['count'].number()
        points = [(chunk[4 + 2 * i].number(), chunk[5 + 2 * i].number()) for i in range(count)]
        rows.append((row['gload'].number(), points))
    if [g for g, _ in rows] != list(range(plane['envMin'].number(), plane['envMax'].number() + 1)):
        raise ValueError('envelope rows do not cover the declared G range')
    return rows


def level_speeds(points, altitude):
    """Slowest and fastest envelope speed at one altitude, or None outside it."""
    low, high = None, None
    for i, (speed, alt) in enumerate(points):
        nxt = points[(i + 1) % len(points)]
        hits = [speed] if alt == altitude else []
        if (alt < altitude < nxt[1]) or (nxt[1] < altitude < alt):
            hits.append(speed + (nxt[0] - speed) * (altitude - alt) / (nxt[1] - alt))
        for hit in hits:
            low = hit if low is None else min(low, hit)
            high = hit if high is None else max(high, hit)
    return None if low is None else (low, high)


def feet(token):
    """Altitudes are stored in 1/256 foot unless the source scales them."""
    return token.number() if token.scaled else token.number() // 256


def weapon_kind(projectile):
    """Mechanical grouping from recovered fields only. See docs/formats/fa-catalog.md.

    Powered records carry a motor time; guided records carry a seeker signature;
    everything else separates by whether it leaves the rail with its own speed.
    This is a reading aid, not a recovered FA category.
    """
    powered = projectile['fuelT'].number() > 0
    guided = projectile['sig'].number() > 0
    if powered:
        return 'guided missile' if guided else 'rocket'
    if projectile['initialSpeed'].number() > 0:
        return 'gun round'
    return 'guided bomb' if guided else 'bomb'


def aircraft_row(source, archive, data, layout, guns):
    blocks = parse_brf(data)
    root = blocks['']
    sizes = (len(layout['OBJECT']), len(layout['NPC']), len(layout['PLANE']))
    if len(root) != sum(sizes):
        raise ValueError(f'unsupported aircraft layout: {len(root)} statements')
    obj = fields(root[:sizes[0]], layout['OBJECT'])
    npc = fields(root[sizes[0]:sizes[0] + sizes[1]], layout['NPC'])
    plane = fields(root[sizes[0] + sizes[1]:], layout['PLANE'])
    names = strings(blocks, obj['ot_names'])
    if len(names) != 3:
        raise ValueError('invalid aircraft identity block')
    hardpoints = blocks.get('hards', [])
    width = len(layout['HARDPOINT'])
    if npc['numHards'].number() * width != len(hardpoints):
        raise ValueError('hardpoint count does not match the station block')
    stores = []
    for i in range(npc['numHards'].number()):
        station = fields(hardpoints[i * width:(i + 1) * width], layout['HARDPOINT'])
        named = strings(blocks, station['defaultTypeName'])
        stores.extend(name.upper() for name in named)
    counted = {name: stores.count(name) for name in dict.fromkeys(stores)}
    rows = dict(envelope_rows(blocks, plane, layout))
    if 1 not in rows:
        raise ValueError('aircraft has no 1 G envelope row')
    cruise = rows[1]
    sea_level = level_speeds(cruise, 0)
    return {
        'resource': source,
        'short_name': names[0],
        'display_name': names[1],
        'year': obj['year'].number(),
        'object_class': f'0x{obj["obj_class"].number() & 0xffff:04x}',
        # Plane flag 8, read as "no lift" by the flight model: rotary and lighter
        # than air types. See crates/tore-formats/src/flight_model/diagnostic.rs.
        'no_lift': 'yes' if plane['flags'].number() & 8 else 'no',
        'engines': plane['engines'].number(),
        'empty_weight_lb': obj['weight'].number(),
        'internal_fuel_lb': plane['internalFuel'].number(),
        'max_takeoff_weight_lb': plane['maxTakeoffWeight'].number(),
        'military_thrust_lbf': plane['thrust'].number(),
        'afterburner_thrust_lbf': plane['aftThrust'].number(),
        'top_speed_kt': knots(max(speed for speed, _ in cruise)),
        'sea_level_min_kt': knots(sea_level[0]) if sea_level else '',
        'sea_level_max_kt': knots(sea_level[1]) if sea_level else '',
        'ceiling_ft': feet(obj['maxAlt']),
        'g_limit_min': plane['envMin'].number(),
        'g_limit_max': plane['envMax'].number(),
        'hit_points': obj['hitPoints'].number(),
        'hardpoints': npc['numHards'].number(),
        'internal_gun': ';'.join(name for name in counted if name in guns),
        'default_stores': ';'.join(f'{name}:{count}' for name, count in counted.items()),
        'hud_resource': ';'.join(strings(blocks, obj['hudName'])),
        'shape': ';'.join(strings(blocks, obj['shape'])),
        'archive': archive,
    }


def weapon_row(source, archive, data, layout):
    blocks = parse_brf(data)
    root = blocks['']
    size = len(layout['OBJECT'])
    if len(root) != size + len(layout['PROJECTILE']):
        raise ValueError(f'unsupported projectile layout: {len(root)} statements')
    obj = fields(root[:size], layout['OBJECT'])
    projectile = fields(root[size:], layout['PROJECTILE'])
    names = strings(blocks, projectile['si_names'])
    if len(names) != 3:
        raise ValueError('invalid store identity block')
    damage = [obj[f'damage[{i}]'].number() for i in range(5)]
    return {
        'resource': source,
        'short_name': names[0],
        'display_name': names[1],
        'derived_kind': weapon_kind(projectile),
        'year': obj['year'].number(),
        'weight_lb': obj['weight'].number(),
        'ordnance_bank': 'one' if projectile['flags'].number() & BANK_ONE_FLAG else 'two',
        'seeker_signature': projectile['sig'].number(),
        'launch_min_nmi': miles(projectile['zone1.minRange'].number()),
        'launch_max_nmi': miles(projectile['zone1.maxRange'].number()),
        'seeker_max_nmi': miles(projectile['zone0.maxRange'].number()),
        'motor_burn_s': round(projectile['fuelT'].number() * TIMER_SECONDS, 2),
        'lifetime_s': round(projectile['removeT'].number() * TIMER_SECONDS, 2),
        'initial_speed_fts': projectile['initialSpeed'].number(),
        'max_speed_fts': obj['_maxSpeed'].number(),
        'projectiles_in_pod': projectile['projsInPod'].number(),
        'rounds_per_shot': projectile['actualRoundsPerGame'].number(),
        'damage_by_class': ';'.join(str(value) for value in damage),
        'fuze_radius_ft': projectile['fuzeRadius'].number(),
        'aircraft_default_stations': 0,
        'surface_type_references': 0,
        'shape': ';'.join(strings(blocks, obj['shape'])),
        'fire_sound': ';'.join(strings(blocks, projectile['fireSound'])),
        'archive': archive,
    }


def surface_references(paths):
    """Count ground and ship type records naming each store in a string block."""
    counts = {}
    for path in paths:
        try:
            blocks = parse_brf(path.read_bytes())
        except ValueError:
            continue
        named = {token.value.upper() for block in blocks.values() for token in block
                 if token.kind == 'string' and token.value.upper().endswith('.JT')}
        for name in named:
            counts[name] = counts.get(name, 0) + 1
    return counts


def catalogs(extracted, layout):
    """Decode every extracted PT and JT, with their cross references resolved."""
    resources = sorted(p for p in Path(extracted).rglob('*') if p.is_file())
    weapons = {}
    for path in [p for p in resources if p.suffix.upper() == '.JT']:
        name = path.name.upper()
        # A name in two archives would need a documented build choice first.
        if name in weapons:
            raise ValueError(f'{name} appears in more than one archive')
        weapons[name] = weapon_row(name, path.parent.name, path.read_bytes(), layout)
    guns = {name for name, row in weapons.items() if row['derived_kind'] == 'gun round'}
    aircraft = []
    for path in [p for p in resources if p.suffix.upper() == '.PT']:
        if any(row['resource'] == path.name.upper() for row in aircraft):
            raise ValueError(f'{path.name.upper()} appears in more than one archive')
        row = aircraft_row(path.name.upper(), path.parent.name, path.read_bytes(), layout, guns)
        for store in filter(None, row['default_stores'].split(';')):
            name, count = store.rsplit(':', 1)
            if name in weapons:
                weapons[name]['aircraft_default_stations'] += int(count)
        aircraft.append(row)
    surface = surface_references(p for p in resources if p.suffix.upper() in ('.NT', '.OT'))
    for name, count in surface.items():
        if name in weapons:
            weapons[name]['surface_type_references'] = count
    return aircraft, [weapons[name] for name in sorted(weapons)]


def write_csv(path, rows):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('w', encoding='utf-8', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]), lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)
    print(f'{len(rows)} rows: {path}')


def extract(source, work):
    """Reuse the shared extractor so archive handling has one implementation."""
    command = [sys.executable, str(ROOT / 'tools/extract_assets.py'),
               '--source', str(source), '--out', str(work), '--overwrite',
               '--exclude-archive', 'disc1/LHX/*', '--exclude-archive', 'disc1/WB/*']
    for pattern in ('*.PT', '*.JT', '*.NT', '*.OT'):
        command.extend(['--include', pattern])
    subprocess.run(command, cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, default=ROOT / 'gameassets/fighters-anthology',
                        help='An archive or directory; default: local Fighters Anthology media')
    parser.add_argument('--work', type=Path, default=ROOT / '.local/catalog',
                        help='Extraction directory for the decoded records, outside the repository')
    parser.add_argument('--out', type=Path, default=ROOT / 'docs/formats',
                        help='Directory for fa-aircraft.csv and fa-weapons.csv')
    parser.add_argument('--extracted', type=Path,
                        help='Use an existing extraction directory instead of extracting again')
    args = parser.parse_args()
    extracted = args.extracted
    if not extracted:
        extract(args.source, args.work)
        extracted = args.work
    layout = layouts()
    try:
        aircraft, weapons = catalogs(extracted, layout)
    except ValueError as error:
        parser.exit(1, f'{error}\n')
    if not aircraft or not weapons:
        parser.exit(1, f'No PT or JT records under {extracted}\n')
    write_csv(args.out / 'fa-aircraft.csv', aircraft)
    write_csv(args.out / 'fa-weapons.csv', weapons)
    return 0


if __name__ == '__main__':
    sys.exit(main())
