"""Reviewed FA weapon research boundaries. No native code execution."""
import json
import re

KEYWORDS = ('proj', 'hard', 'graphic', 'damage', 'npcweapons', 'cpupdateradar',
            'cpresetrwr', 'coll', 'objmove', 'cobv', 'cospeed', 'coturn')

# Entire named spans remain research. Only explicitly documented subregions
# have arithmetic translations; boundaries do not imply whole-routine parity.
REGIONS = (
    ('launch_speed', 0x4c1120, 0x4c1166, 'movement'),
    ('engine_state', 0x4c1170, 0x4c11a2, 'movement'),
    ('projectile_move', 0x4c11b0, 0x4c162e, 'movement'),
    ('unpowered_fall', 0x4c14e4, 0x4c153d, 'movement'),
    ('fov', 0x4c2860, 0x4c2b50, 'sensors'),
    ('lock', 0x4c2f20, 0x4c3250, 'sensors'),
    ('damage_category', 0x411470, 0x4114ec, 'damage'),
    ('damage_amount', 0x40f9b0, 0x40f9ec, 'damage'),
    ('station_failure', 0x4103f1, 0x4104bc, 'damage'),
    ('hit_chance', 0x4c3380, 0x4c39a0, 'damage'),
    ('fire', 0x4c2170, 0x4c26f0, 'weapons'),
    ('service_weapon', 0x4c4700, 0x4c5570, 'weapons'),
    ('graphics_init', 0x442c00, 0x442da0, 'graphics'),
    ('can_load', 0x452980, 0x452c20, 'loading'),
    ('store_weight', 0x452940, 0x452980, 'loading'),
    ('unload', 0x4527f0, 0x452860, 'loading'),
    ('radar_emission', 0x4c2eb0, 0x4c2f17, 'sensors'),
    ('altitude_performance', 0x477da0, 0x477e45, 'movement'),
    ('axial_speed', 0x438070, 0x4380b0, 'movement'),
    ('position_delta', 0x4120c0, 0x41214a, 'movement'),
    ('player_trigger', 0x416ef5, 0x41702c, 'weapons'),
    ('clock_conversion', 0x486bd7, 0x486be7, 'clock'),
)


def artifacts(exe, rows, instructions, repo):
    # Reuse bounded section/edge readers; this function is called only after
    # both FA source hashes have been checked by the shared extraction pass.
    from extract_native_flight import reviewed_regions, field_addresses
    result = reviewed_regions(exe, rows, instructions, REGIONS)
    manifest = json.loads(result['reviewed-components.json'])
    manifest['instance_state'] = []
    manifest['open_contracts'] = [
        'complete native service clock and player trigger dispatch',
        'generic object movement and target command producers',
        'terrain/collision, damage scaling/RNG and subsystem selection',
        'radar/signature/illumination and countermeasure state',
        'full effect tables and drawing module execution semantics',
        'whole engagement comparison against original-game observations',
    ]
    manifest['translation_scope'] = 'see docs/formats/weapons.md; spans are not complete translations'
    result['reviewed-components.json'] = json.dumps(manifest, indent=2)+'\n'
    fields = field_addresses(repo, 0x50d268, ('OBJECT', 'PROJECTILE'))
    references = {field['va']: [] for field in fields}
    for va, line in instructions:
        for address in {int(value, 16) for value in re.findall(r'\b0x([0-9a-f]+)\b', line)}:
            if address in references:
                references[address].append(va)
    for field in fields:
        field['direct_references'] = references[field['va']]
    result['jt-field-references.json'] = json.dumps(fields, indent=2)+'\n'
    return result
