"""Hash-gated FA creator/ordnance research; no retail code execution."""
import json
import re

KEYWORDS = ('quickmission', 'quickbutton', 'armplane', 'hardload', 'hardcanload',
            'hardunload', 'storeweight', 'getnames')

# Reviewed subregions, not a complete screen interpreter or callback contract.
REGIONS = (
    ('creator_aircraft_filters', 0x42ed13, 0x42edd3, 'creator'),
    ('creator_filter_checkmarks', 0x42ee15, 0x42eeaa, 'creator'),
    ('ordnance_overweight', 0x41b2d6, 0x41b2fd, 'ordnance'),
    ('ordnance_quantity_sound', 0x41b686, 0x41b8cc, 'ordnance'),
    ('ordnance_fuel_sound', 0x41b1e1, 0x41b2d6, 'ordnance'),
    ('load', 0x452c20, 0x452d10, 'loading'),
    ('can_load', 0x452980, 0x452c20, 'loading'),
    ('store_weight', 0x452940, 0x452980, 'loading'),
)


def strings_in_region(exe, rows, start, end):
    """Read a reviewed file-backed data range with bounded ASCII candidates.

    These are string locations, not proof of active UI options or table order.
    """
    if not 0 < end - start <= 16384:
        raise ValueError('menu string range exceeds bound')
    section = next((r for r in rows if not r['executable'] and
                    r['va'] <= start < end <= r['va'] + r['size']), None)
    if section is None:
        raise ValueError('menu strings outside file-backed data')
    raw = section['raw'] + start - section['va']
    if raw < 0 or raw + end - start > len(exe):
        raise ValueError('truncated menu strings')
    data = exe[raw:raw + end - start]
    return [{'va': start + m.start(), 'text': m[0][:-1].decode('ascii')}
            for m in re.finditer(rb'[\x20-\x7e]{2,160}\x00', data)
            if m.start() == 0 or data[m.start()-1] == 0]


def artifacts(exe, rows, instructions):
    # Called only after the shared extractor verifies both EXE and SMS hashes.
    from extract_native_flight import reviewed_regions
    result = reviewed_regions(exe, rows, instructions, REGIONS)
    manifest = json.loads(result['reviewed-components.json'])
    manifest['instance_state'] = []
    manifest['open_contracts'] = [
        'dynamic catalog flag construction and runtime-populated dialog records',
        'complete ordnance art/text geometry and physical input/repeat gestures',
        'stock/year/airbase eligibility and full load initialization',
        'standard/custom mission flow and cancel/commit behavior',
        'original-game visual and interaction acceptance',
    ]
    result['reviewed-components.json'] = json.dumps(manifest, indent=2) + '\n'
    candidates = []
    for group, start, end in [('ordnance', 0x4ee770, 0x4ee9d0),
                              ('creator', 0x4ef200, 0x4ef900)]:
        for row in strings_in_region(exe, rows, start, end):
            row['group'] = group
            row['direct_references'] = []
            candidates.append(row)
    indexed = {row['va']: row for row in candidates}
    for at, line in instructions:
        for address in {int(s, 16) for s in re.findall(r'\b0x([0-9a-fA-F]+)\b', line)}:
            if address in indexed:
                indexed[address]['direct_references'].append(at)
    result['menu-string-references.json'] = json.dumps({
        'schema_version': 1,
        'status': 'research candidates; not active option tables',
        'strings': candidates,
    }, indent=2) + '\n'
    from native_menu_tables import artifacts as table_artifacts
    result.update(table_artifacts(exe, rows))
    return result
