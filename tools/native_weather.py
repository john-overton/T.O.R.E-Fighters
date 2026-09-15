"""Reviewed FA clock/weather/turbulence research slices; no retail execution."""
import json

KEYWORDS = ('_time', '_wr', 'turbulence', 'cloud', 'graphicaddsmoke')

# Half-open, instruction-aligned regions in the reviewed EXE/SMS pair only.
REGIONS = (
    ('time_init', 0x486a10, 0x486a81, 'clock'),
    ('time_update', 0x486aa0, 0x486bea, 'clock'),
    ('counter_clock', 0x486bf0, 0x486c53, 'clock'),
    ('weather_view_update', 0x4b3480, 0x4b3746, 'weather'),
    ('weather_time_selection', 0x4b3750, 0x4b3817, 'weather'),
    ('weather_interpolation', 0x4b3820, 0x4b3b60, 'weather'),
    ('fog_callback', 0x4b4320, 0x4b4370, 'weather'),
    ('physical_turbulence', 0x477590, 0x477d07, 'turbulence'),
    ('flight_turbulence_call', 0x47c7b6, 0x47c7fa, 'turbulence'),
    ('sound_turbulence_level', 0x434550, 0x434620, 'audio'),
    ('sound_turbulence_dispatch', 0x434d63, 0x434dc2, 'audio'),
)


def artifacts(exe, rows, instructions):
    # Shared extractor gates entry on BOTH source hashes.
    from extract_native_flight import reviewed_regions
    result = reviewed_regions(exe, rows, instructions, REGIONS)
    manifest = json.loads(result['reviewed-components.json'])
    manifest['instance_state'] = []
    manifest['open_contracts'] = [
        'complete native scheduler/RNG and long-session clock behavior',
        'ground-query surface flag and nearby-aircraft geometry acceptance',
        'complete maneuver buffet and sound event mapping',
        'contrail and wing-induced vapor triggers, assets and lifecycle',
        'complete LAY fields/celestial/cloud rendering and retail comparison',
    ]
    result['reviewed-components.json'] = json.dumps(manifest, indent=2) + '\n'
    return result
