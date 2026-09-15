"""Reviewed FA clock/weather/turbulence/vapor research slices; no retail execution."""
import json

KEYWORDS = ('_time', '_wr', 'turbulence', 'cloud', 'graphicaddsmoke', 'streamer', 'sample')

# Half-open, instruction-aligned regions in the reviewed EXE/SMS pair only.
REGIONS = (
    ('time_init', 0x486a10, 0x486a81, 'clock'),
    ('time_update', 0x486aa0, 0x486bea, 'clock'),
    ('counter_clock', 0x486bf0, 0x486c53, 'clock'),
    ('weather_view_update', 0x4b3480, 0x4b3746, 'weather'),
    ('weather_time_selection', 0x4b3750, 0x4b3817, 'weather'),
    ('weather_interpolation', 0x4b3820, 0x4b3b60, 'weather'),
    ('layer_query', 0x4b3190, 0x4b31e3, 'weather'),
    ('layer_altitude_blend', 0x4b3be0, 0x4b3ca5, 'weather'),
    ('layer_altitude_haze', 0x4b3cb0, 0x4b3d84, 'weather'),
    ('palette_tint_smoothing', 0x4b3f28, 0x4b3f74, 'weather'),
    ('palette_tint_ranges', 0x4b3ff6, 0x4b4041, 'weather'),
    ('palette_tint_channels', 0x4c8f10, 0x4c8f79, 'weather'),
    ('shade_nearest_header', 0x4b3ad0, 0x4b3b60, 'weather'),
    ('shade_distance_level', 0x4b3410, 0x4b3476, 'weather'),
    ('shade_cross_layer_ray', 0x4b31f0, 0x4b3410, 'weather'),
    ('palette_worker_timer', 0x486e80, 0x486ef6, 'weather'),
    ('deck_wildcard_choice', 0x4b4680, 0x4b46c8, 'weather'),
    ('horizon_dispatch', 0x4aacf0, 0x4ab446, 'weather'),
    ('horizon_solid', 0x4c924c, 0x4c942b, 'weather'),
    ('horizon_gouraud', 0x4c942c, 0x4c95c8, 'weather'),
    ('deck_horizon_edges', 0x447ed7, 0x4483c7, 'weather'),
    ('deck_horizon_polygon', 0x448585, 0x448929, 'weather'),
    ('deck_distance_helpers', 0x447970, 0x447a40, 'weather'),
    ('shape_weather_fog_toggle', 0x4d426c, 0x4d42c5, 'weather'),
    ('weather_fog_toggle_flag', 0x4b352b, 0x4b3547, 'weather'),
    ('indexed_effect_remap', 0x4cc44c, 0x4cc4ac, 'weather'),
    ('sun_whitening_channels', 0x4c8e6c, 0x4c8ec6, 'weather'),
    ('sun_whitening_target', 0x4b4170, 0x4b41e3, 'weather'),
    ('sun_view_alignment', 0x4cd8b0, 0x4cd8f0, 'weather'),
    ('sun_lens_flare', 0x4b4990, 0x4b4b2a, 'weather'),
    ('celestial_shape_load', 0x4aaca0, 0x4aacdd, 'weather'),
    ('celestial_shape_dispatch', 0x4ab0af, 0x4ab309, 'weather'),
    ('weather_shape_fills', 0x4d2fc8, 0x4d300a, 'weather'),
    ('weather_shape_circles_points', 0x4d17f8, 0x4d1974, 'weather'),
    ('weather_shape_uv', 0x4d4a30, 0x4d4aca, 'weather'),
    ('weather_shape_billboard', 0x4d5644, 0x4d59a1, 'weather'),
    ('cloud_initialize', 0x4a7f40, 0x4a7f64, 'weather'),
    ('cloud_repeat_grid', 0x4a8090, 0x4a8125, 'weather'),
    ('cloud_periodic_placement', 0x4a8130, 0x4a83de, 'weather'),
    ('cloud_dispatch', 0x4a8b90, 0x4a8c2e, 'weather'),
    ('cloud_view_sectors', 0x4a9660, 0x4a97af, 'weather'),
    ('cloud_queue', 0x4a8c30, 0x4a8cbd, 'weather'),
    ('cloud_queue_consumer', 0x4a7c26, 0x4a7cc1, 'weather'),
    ('shape_range_and_frustum', 0x4d028c, 0x4d0796, 'weather'),
    ('cloud_generated_altitude', 0x42a8df, 0x42a91c, 'weather'),
    ('fog_callback', 0x4b4320, 0x4b4370, 'weather'),
    ('weather_effects', 0x4b4720, 0x4b4785, 'weather'),
    ('weather_visibility', 0x4b4b30, 0x4b4ba4, 'weather'),
    ('physical_turbulence', 0x477590, 0x477d07, 'turbulence'),
    ('flight_turbulence_call', 0x47c7b6, 0x47c7fa, 'turbulence'),
    ('sound_turbulence_level', 0x434550, 0x434620, 'audio'),
    ('sound_turbulence_dispatch', 0x434d63, 0x434dc2, 'audio'),
    ('streamer_def_lookup', 0x49fd70, 0x49fd8b, 'vapor'),
    ('streamer_draw', 0x49fd90, 0x4a0007, 'vapor'),
    ('streamers_init', 0x4a0010, 0x4a0101, 'vapor'),
    ('streamer_attachment', 0x4a0110, 0x4a0249, 'vapor'),
    ('streamers_update', 0x4a0250, 0x4a02cf, 'vapor'),
    ('streamer_shape_opcodes', 0x4d47a4, 0x4d4871, 'vapor'),
    ('swing_wing_publish', 0x4ab7c1, 0x4ab7e6, 'vapor'),
    ('sample_init', 0x4124e0, 0x412542, 'vapor'),
    ('sample_update', 0x412570, 0x4125b7, 'vapor'),
    ('sample_get', 0x4125c0, 0x412771, 'vapor'),
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
        'engine contrail and broader wing-induced vapor triggers and assets',
        'complete LAY fields/celestial/cloud rendering and retail comparison',
    ]
    result['reviewed-components.json'] = json.dumps(manifest, indent=2) + '\n'
    return result
