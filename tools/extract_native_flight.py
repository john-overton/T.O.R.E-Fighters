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


# Manually reviewed FA address boundaries, including helpers hidden inside SMS spans.
# These are static research slices, not executable modules or a complete call graph.
REVIEWED_REGIONS = (
    ('object_remove_caller', 0x4627b0, 0x4628aa, 'ground'),
    ('object_death_notify', 0x471400, 0x47144b, 'ground'),
    ('object_remove_notify', 0x46fdb0, 0x46fdf3, 'ground'),
    ('event_recipient_invalidate', 0x4189e0, 0x418a0d, 'ground'),
    ('object_record_expire', 0x44baa0, 0x44bae1, 'ground'),
    ('object_unscheduled_events', 0x462c91, 0x462d39, 'ground'),
    ('object_local_effect_events', 0x462d40, 0x462e22, 'ground'),
    ('object_score_entry_gate', 0x486580, 0x4865b8, 'ground'),
    ('object_death_statistics', 0x485820, 0x485a39, 'ground'),
    ('object_damage_attribution_prefix', 0x4c1870, 0x4c18fc, 'ground'),
    ('object_schedule_service', 0x462a50, 0x462b64, 'ground'),
    ('object_schedule_merge', 0x462b70, 0x462c91, 'ground'),
    ('clock_frame_begin', 0x486a90, 0x486a9b, 'clock'),
    ('clock_scale_set', 0x486c60, 0x486c7c, 'clock'),
    ('clock_peer_pause', 0x46ff70, 0x46ffbc, 'clock'),
    ('object_hit_dispatch', 0x463ec0, 0x463f24, 'ground'),
    ('object_collision_hit', 0x473b40, 0x473bdd, 'ground'),
    ('object_base_selector', 0x473be0, 0x473c0a, 'ground'),
    ('object_npc_selector', 0x473db0, 0x473dd8, 'ground'),
    ('object_death_mark', 0x473c10, 0x473d9a, 'ground'),
    ('object_crater_zero_gate', 0x443d00, 0x443d21, 'ground'),
    ('event_queue_reset', 0x418070, 0x418098, 'ground'),
    ('event_enqueue_recipients', 0x41818a, 0x4182e8, 'ground'),
    ('event_enqueue_observers', 0x418382, 0x418433, 'ground'),
    ('event_enqueue_fallback', 0x4184b6, 0x41859f, 'ground'),
    ('speech_state_reset', 0x48d2b0, 0x48d2e3, 'clock'),
    ('speech_event_observer', 0x48d350, 0x48d3b3, 'ground'),
    ('speech_default_callback', 0x48d3c0, 0x48d402, 'ground'),
    ('speech_buffer_append', 0x48d420, 0x48d46c, 'ground'),
    ('speech_emit', 0x48d470, 0x48d5dc, 'ground'),
    ('speech_handle_reset', 0x48d600, 0x48d60a, 'ground'),
    ('speech_sample_sequence', 0x48d610, 0x48d6d8, 'ground'),
    ('mission_interceptor_select', 0x480aa0, 0x480ac7, 'ground'),
    ('mission_interceptor_name', 0x481f30, 0x481f85, 'ground'),
    ('object_movement_heading_common', 0x4374ac, 0x4376d6, 'ground'),
    ('object_movement_pitch_hold', 0x4376d6, 0x4376de, 'ground'),
    ('object_movement_pitch_ground', 0x4376f5, 0x43777b, 'ground'),
    ('object_movement_pitch_common', 0x4377af, 0x437a84, 'ground'),
    ('object_movement_bank_hold', 0x437a84, 0x437a90, 'ground'),
    ('object_movement_bank_common', 0x437c17, 0x437daa, 'ground'),
    ('object_movement_speed_select', 0x437ecb, 0x437eff, 'ground'),
    ('object_angle_approach', 0x411950, 0x41199b, 'ground'),
    ('signed_word_magnitude', 0x4c6614, 0x4c661d, 'ground'),
    ('object_command_initialize', 0x463a20, 0x463ae6, 'ground'),
    ('object_command_condition', 0x463af0, 0x463b74, 'ground'),
    ('object_command_deadline', 0x463b90, 0x463bbc, 'clock'),
    ('object_command_reset', 0x463e50, 0x463e95, 'ground'),
    ('object_script_entry_gate', 0x463d40, 0x463d63, 'ground'),
    ('object_command_predicate', 0x4382d0, 0x438454, 'ground'),
    ('object_default_event', 0x473a40, 0x473b37, 'ground'),
    ('object_movement_heading_hold', 0x436eca, 0x436edb, 'ground'),
    ('object_movement_finish', 0x43805e, 0x438227, 'ground'),
    ('object_movement_roll_rate', 0x478090, 0x4780cc, 'ground'),
    ('object_movement_turn_rate', 0x4780d0, 0x478143, 'ground'),
    ('object_movement_speed_reference', 0x477d10, 0x477d29, 'ground'),
    ('object_service_snapshot', 0x4631b0, 0x4631e5, 'ground'),
    ('object_movement_query_prefix', 0x436b30, 0x436c70, 'ground'),
    ('object_event_service', 0x4631f0, 0x463721, 'ground'),
    ('object_event_mask_gate', 0x463980, 0x4639b3, 'ground'),
    ('object_event_dispatch', 0x4639c0, 0x463a12, 'ground'),
    ('event_queue_consume', 0x4185a0, 0x4186d5, 'ground'),
    ('event_enqueue_rng_prefix', 0x4180a0, 0x41818a, 'clock'),
    ('event_enqueue_payload', 0x4182e8, 0x418382, 'ground'),
    ('event_enqueue_local_schedule', 0x418433, 0x4184b6, 'ground'),
    ('object_current_push', 0x4629e0, 0x462a17, 'ground'),
    ('object_current_pop', 0x462a20, 0x462a4c, 'ground'),
    ('speech_delay_scale', 0x48d5e0, 0x48d5f3, 'clock'),
    ('speech_event_submit', 0x48e950, 0x48ea0f, 'ground'),
    ('clock_initialize', 0x486a10, 0x486a81, 'clock'),
    ('plane_callback_selector', 0x49fb10, 0x49fb4c, 'ground'),
    ('airport_takeoff_entry_gate', 0x4badb0, 0x4bae24, 'ground'),
    ('airport_landing_entry_gate', 0x4bc270, 0x4bc310, 'ground'),
    ('airport_slot_lookup', 0x4bd3d0, 0x4bd419, 'ground'),
    ('airport_slot_reserve', 0x4bd420, 0x4bd48f, 'ground'),
    ('airport_slot_release', 0x4bd490, 0x4bd50a, 'ground'),
    ('object_enter_state', 0x464300, 0x46441c, 'ground'),
    ('plane_airport_refresh_tail', 0x452594, 0x452628, 'ground'),
    ('airport_comment_selection', 0x48f6a0, 0x48f7a6, 'ground'),
    ('airport_comment_finish', 0x490041, 0x4900a0, 'ground'),
    ('speech_buffer_reset', 0x48d410, 0x48d41d, 'ground'),
    ('airport_lookup', 0x4bd2d0, 0x4bd310, 'ground'),
    ('airport_service_reset', 0x4bd310, 0x4bd3ca, 'ground'),
    ('airport_predicate_point', 0x4bab20, 0x4bab7c, 'ground'),
    ('airport_predicate_axis', 0x4bab80, 0x4babfb, 'ground'),
    ('airport_predicate_approach', 0x4bac00, 0x4bac6a, 'ground'),
    ('service_actor_list_reset', 0x49d510, 0x49d51a, 'ground'),
    ('service_actor_list_remove', 0x49d520, 0x49d57a, 'ground'),
    ('service_actor_list_register', 0x49fa50, 0x49fa9f, 'ground'),
    ('object_service_dispatch', 0x462e70, 0x4631a9, 'ground'),
    ('object_service_priority', 0x464550, 0x464637, 'ground'),
    ('rng_word_bound', 0x4562f0, 0x4562fb, 'clock'),
    ('object_schedule_reset', 0x462600, 0x462619, 'ground'),
    ('object_schedule_remove', 0x462620, 0x4626ae, 'ground'),
    ('object_schedule_insert', 0x4626b0, 0x4627a6, 'ground'),
    ('mission_object_type_alias', 0x4824b1, 0x4825c4, 'ground'),
    ('mission_object_nationality', 0x482695, 0x48271d, 'ground'),
    ('mission_nationality_remap', 0x483d50, 0x483dd5, 'ground'),
    ('mission_object_flags_speed', 0x4827b9, 0x482842, 'ground'),
    ('mission_object_name', 0x483bbd, 0x483c2a, 'ground'),
    ('mission_object_post_create', 0x482df7, 0x482eea, 'ground'),
    ('object_type_setup', 0x4a6eb0, 0x4a71dc, 'ground'),
    ('object_shape_resolve', 0x4a71e0, 0x4a71fc, 'ground'),
    ('object_creation_finish', 0x4a7806, 0x4a7859, 'ground'),
    ('object_creation_store', 0x4a7a06, 0x4a7a1a, 'ground'),
    ('object_release_last_allocation', 0x491490, 0x4914b4, 'ground'),
    ('object_airport_attachment_gate', 0x4beb90, 0x4bec5c, 'ground'),
    ('collision_object_unregister', 0x42e5c0, 0x42e679, 'ground'),
    ('airport_reset', 0x4ba7e0, 0x4ba7fa, 'ground'),
    ('airport_delete', 0x4ba870, 0x4ba8de, 'ground'),
    ('resource_setup_notification', 0x4a6df0, 0x4a6e17, 'ground'),
    ('resource_extension', 0x4a6860, 0x4a686d, 'ground'),
    ('symbol_call_by_name', 0x46a570, 0x46a63a, 'ground'),
    ('strip_callback_selector', 0x4be640, 0x4be675, 'ground'),
    ('strip_add', 0x4be2a0, 0x4be636, 'ground'),
    ('shape_contact_box_lookup', 0x42e100, 0x42e134, 'ground'),
    ('airport_register', 0x4ba800, 0x4ba867, 'ground'),
    ('airport_point_transforms', 0x4bd950, 0x4bdb29, 'ground'),
    ('object_offset_transform', 0x411d10, 0x411dda, 'ground'),
    ('mission_object_begin', 0x482443, 0x4824b1, 'ground'),
    ('mission_object_position_angles', 0x4825c4, 0x482695, 'ground'),
    ('mission_object_create', 0x482dcf, 0x482df7, 'ground'),
    ('object_initial_ground', 0x4a73b0, 0x4a762c, 'ground'),
    ('object_static_kind', 0x4a762c, 0x4a7638, 'ground'),
    ('object_add_callback', 0x4a77e4, 0x4a7810, 'ground'),
    ('object_callback_dispatch', 0x463f60, 0x463f94, 'ground'),
    ('object_callback_resolver', 0x463f30, 0x463f5c, 'ground'),
    ('object_current_load', 0x4628b0, 0x462930, 'ground'),
    ('object_current_store', 0x462980, 0x4629ba, 'ground'),
    ('collision_object_register', 0x42e540, 0x42e5bf, 'ground'),
    ('candidate_direction_angles', 0x411a40, 0x411aec, 'ground'),
    ('direction_word_reduction', 0x4c6c30, 0x4c6d5f, 'ground'),
    ('terrain_traversal', 0x42bdc0, 0x42bfb9, 'ground'),
    ('terrain_cell', 0x42bfc0, 0x42c1a0, 'ground'),
    ('terrain_plane', 0x42c1a0, 0x42c413, 'ground'),
    ('horizontal_plane', 0x42dda0, 0x42de5d, 'ground'),
    ('shape_contact_record', 0x42e0c0, 0x42e0f4, 'ground'),
    ('terrain_normal', 0x4a8d30, 0x4a8e4a, 'ground'),
    ('terrain_cell_lookup', 0x4c6040, 0x4c60e8, 'ground'),
    ('integer_square_root', 0x4d65c4, 0x4d663d, 'ground'),
    ('ground_entry_queries', 0x47af20, 0x47af70, 'ground'),
    ('collision_dispatch_cache', 0x42b800, 0x42bd2e, 'ground'),
    ('ground_slope_projection', 0x42bd30, 0x42bdb1, 'ground'),
    ('collision_candidate_commit', 0x42de60, 0x42df80, 'ground'),
    ('landing_object_preference', 0x4747c0, 0x4747f6, 'ground'),
    ('control_disturbance', 0x47bcb2, 0x47c0a2, 'response'),
    ('control_disturbance_select', 0x47af70, 0x47b01e, 'response'),
    ('environment_disabled_gate', 0x477590, 0x4775b5, 'integration'),
    ('environment_disabled_reset', 0x477ce4, 0x477d07, 'integration'),
    ('loaded_g_envelopes', 0x452167, 0x452482, 'loading'),
    ('normal_passive_fall', 0x47bb85, 0x47bcb2, 'response'),
    ('auxiliary_rate_scale', 0x47b0e7, 0x47b182, 'response'),
    ('rudder_ground_and_air', 0x47c301, 0x47c682, 'response'),
    ('idle_pitch_floor', 0x47ac20, 0x47ac56, 'integration'),
    ('departure_envelope_row', 0x49d200, 0x49d229, 'departure'),
    ('departure_envelope_class', 0x49d230, 0x49d2cb, 'departure'),
    ('departure_stall_speed', 0x49d1d0, 0x49d1fa, 'departure'),
    ('vertical_thrust_support', 0x47add0, 0x47aeb0, 'departure'),
    ('ground_control_inhibition', 0x47b201, 0x47b250, 'departure'),
    ('tumble_warning_start', 0x47b554, 0x47b5fb, 'departure'),
    ('tumble_extended_start', 0x47b681, 0x47b72f, 'departure'),
    ('tumble_movement', 0x47ba8c, 0x47bb85, 'departure'),
    ('stalled_movement_fall', 0x47b2e2, 0x47b36f, 'departure'),
    ('rng_byte', 0x4562e0, 0x4562ea, 'clock'),
    ('sine_cosine_wrapper', 0x4d5c98, 0x4d5cbe, 'rotation'),
    ('flight_response_setup', 0x47b020, 0x47b250, 'response'),
    ('flight_response_controls', 0x47ba8c, 0x47c682, 'response'),
    ('flight_response_finish', 0x47c682, 0x47c860, 'response'),
    ('maneuver_sound_inputs', 0x434550, 0x434620, 'response'),
    ('maneuver_sound_caller', 0x434d63, 0x434dc2, 'response'),
    ('rng_chance', 0x4561a0, 0x4561b8, 'clock'),
    ('rng_reseed', 0x4561c0, 0x4561d0, 'clock'),
    ('object_service_age', 0x462930, 0x46295f, 'clock'),
    ('object_due_dispatch', 0x462a7c, 0x462acc, 'clock'),
    ('ground_query', 0x4abab0, 0x4abbe2, 'ground'),
    ('nearest_surface', 0x4ba8e0, 0x4baa06, 'ground'),
    ('approximate_distance', 0x4c66cc, 0x4c670e, 'ground'),
    ('touchdown_event_gate', 0x412a60, 0x412a8f, 'ground'),
    ('touchdown_event_state', 0x499240, 0x49927e, 'ground'),
    ('contact_predicate', 0x411910, 0x411942, 'ground'),
    ('contact_latch', 0x49fd40, 0x49fd61, 'ground'),
    ('contact_approach', 0x4119a0, 0x4119e8, 'ground'),
    ('word_vector_transform', 0x4cf328, 0x4cf40e, 'rotation'),
    ('equipment_resolution', 0x452770, 0x4527e1, 'loading'),
    ('loaded_control_limits', 0x477ed0, 0x478089, 'loading'),
    ('movement_display_conversion', 0x451820, 0x451891, 'rotation'),
    ('cockpit_offset', 0x417f00, 0x418065, 'rotation'),
    ('cockpit_composition', 0x476cba, 0x476d67, 'rotation'),
    ('matrix_builder', 0x4d5e58, 0x4d60d6, 'rotation'),
    ('matrix_compose', 0x4cf2d0, 0x4cf325, 'rotation'),
    ('atan_interpolation', 0x4ccb88, 0x4ccc3e, 'rotation'),
    ('shuffled_rng', 0x4561d0, 0x4562ce, 'clock'),
    ('frame_clock', 0x486aa0, 0x486bea, 'clock'),
    ('counter_clock', 0x486bf0, 0x486c53, 'clock'),
    ('weight_update', 0x4516b0, 0x451815, 'loading'),
    ('pull_drag_loading', 0x4784a0, 0x4784ea, 'loading'),
    ('thrust_selection', 0x478190, 0x4781c6, 'loading'),
    ('position_wind_step', 0x476ed2, 0x476f98, 'movement'),
    ('world_velocity_builder', 0x476fb0, 0x47700a, 'movement'),
    ('trig_interpolation', 0x4cd588, 0x4cd5c6, 'rotation'),
    ('body_rate_transform', 0x477010, 0x477139, 'rotation'),
    ('rotate_xz', 0x4c6654, 0x4c66cb, 'rotation'),
    ('angle_conversions', 0x4c6620, 0x4c6652, 'rotation'),
    ('vector_thrust', 0x47a860, 0x47a961, 'integration'),
    ('departure_dispatch', 0x47b250, 0x47ba8c, 'departure'),
    ('stall_predicate', 0x47cc70, 0x47cca4, 'departure'),
    ('spin_entry', 0x47ccb0, 0x47cd64, 'departure'),
    ('spin_direction', 0x47cd70, 0x47cdad, 'departure'),
    ('spin_recovery', 0x47cdb0, 0x47ce61, 'departure'),
    ('spin_interpolation', 0x47ce70, 0x47cea5, 'departure'),
    ('landing_limits', 0x477140, 0x47723b, 'ground'),
    ('ground_contact', 0x477240, 0x4774e4, 'ground'),
    ('landing_surface', 0x4774f0, 0x477581, 'ground'),
    ('loaded_speed_limits', 0x452482, 0x4524d6, 'profile'),
    ('drag_force', 0x47a970, 0x47ab54, 'integration'),
    ('live_velocity', 0x47c860, 0x47c97b, 'integration'),
    ('lift_force', 0x47c980, 0x47ca61, 'integration'),
    ('gravity_force', 0x47ca70, 0x47cb74, 'integration'),
    ('transverse_decay', 0x47cb80, 0x47cbdb, 'integration'),
    ('axis_integration', 0x47cbe0, 0x47cc67, 'integration'),
    ('service_multiply', 0x4c65ec, 0x4c65f8, 'clock'),
    ('movement_gravity_turn', 0x476bb0, 0x476cba, 'movement'),
    ('movement_low_speed_travel', 0x476d67, 0x476ed2, 'movement'),
    ('movement_angle_step', 0x476ae0, 0x476bb0, 'movement'),
)

# Partial reviewed instance layout; deliberately independent of guessed reference structs.
REVIEWED_STATE = (
    ('tumble_start', 0x50d046, 4), ('tumble_deadline', 0x50d04a, 4),
    ('tumble_progress', 0x50d04e, 2), ('tumble_previous', 0x50d050, 2),
    ('tumble_direction', 0x50d052, 2),
    ('speed_f8', 0x50ceb4, 4), ('side_velocity_f8', 0x50cff7, 4),
    ('down_velocity_f8', 0x50cffb, 4), ('roll_rate_f8', 0x50cfff, 4),
    ('pitch_rate_f8', 0x50d003, 4), ('yaw_rate_f8', 0x50d007, 4),
    ('movement_roll_f8', 0x50d00f, 4), ('movement_pitch_f8', 0x50d013, 4),
    ('movement_heading_f8', 0x50d017, 4), ('g_f8', 0x50d01b, 4),
    ('bank_offset_f8', 0x50d023, 4), ('aoa_offset_f8', 0x50d027, 4),
    ('slip_offset_f8', 0x50d02b, 4), ('low_speed_pitch_f8', 0x50d02f, 4),
    ('spin_intensity_f8', 0x50d03f, 4), ('spin_direction', 0x50d043, 1),
    ('spin_recovery_ticks', 0x50d044, 2), ('departure_mode', 0x50d08c, 1),
    ('departure_ticks', 0x50d08d, 2), ('touchdown_hold_ticks', 0x50d091, 2),
    ('vertical_speed_word', 0x50d0aa, 2),
)


def reviewed_regions(exe, rows, instructions, regions=REVIEWED_REGIONS, decode_region=None):
    """Explicit bounded slices. Caller must gate on BOTH reviewed source hashes."""
    addresses = [a for a, _ in instructions]
    artifacts, manifest = {}, []
    incoming = {}
    for address, line in instructions:
        match = re.search(r'\b(call|j[a-z]+)\s+0x([0-9a-fA-F]+)', line)
        if match:
            incoming.setdefault(int(match[2], 16), []).append(
                {'at': address, 'kind': match[1]})
    for name, start, end, component in regions:
        section = next((r for r in rows if r['executable'] and
                        r['va'] <= start < end <= r['va']+r['size']), None)
        if section is None:
            raise ValueError(f'reviewed region outside executable: {name}')
        raw = section['raw'] + start-section['va']
        lines = (decode_region(start, end) if decode_region else
                 instructions[bisect.bisect_left(addresses, start):bisect.bisect_left(addresses, end)])
        decoded_addresses = [a for a, _ in lines]
        if (not lines or decoded_addresses[0] != start or
                decoded_addresses != sorted(set(decoded_addresses)) or
                any(not start <= a < end for a in decoded_addresses)):
            raise ValueError(f'invalid aligned reviewed disassembly: {name}')
        edges = []
        for address, line in lines:
            match = re.search(r'\b(call|j[a-z]+)\s+0x([0-9a-fA-F]+)', line)
            if match:
                target = int(match[2], 16)
                edges.append({'at': address, 'kind': match[1], 'target': target,
                              'outside_region': not start <= target < end})
        manifest.append({'name': name, 'component': component, 'va': start, 'end': end,
                         'sha256': hashlib.sha256(exe[raw:raw+end-start]).hexdigest(),
                         'edges': edges, 'entry_references': incoming.get(start, [])})
        artifacts[f'reviewed/{start:08x}-{name}.txt'] = '\n'.join(l for _, l in lines)+'\n'
    artifacts['reviewed-components.json'] = json.dumps({
        'schema_version': 2, 'complete_model': False,
        'method': ('manual static boundaries; independently aligned regions; '
                   'global linear entry-reference inventory; no native execution' if decode_region else
                   'manual static boundaries; direct edges only, no native execution'),
        'regions': manifest,
        'instance_state': [{'name': n, 'va': va, 'cp_offset': va-0x50ce80, 'width': w}
                           for n, va, w in REVIEWED_STATE],
        'open_contracts': ['timer scheduling', 'loaded force assembly', 'body/world rotations',
                           'terrain and carrier contacts', 'damage', 'RNG and turbulence'],
    }, indent=2)+'\n'
    return artifacts


def static_table(exe, rows, va, count, width=2):
    """Read bounded inert word/dword data from one file-backed non-code section."""
    if not 0 < count <= 4096 or width not in (2, 4):
        raise ValueError('native table count outside bound')
    section = next((r for r in rows if not r['executable'] and
                    r['va'] <= va and va+count*width <= r['va']+r['size']), None)
    if section is None:
        raise ValueError('native table outside file-backed data section')
    raw = section['raw']+va-section['va']
    if raw < 0 or raw+count*width > len(exe):
        raise ValueError('truncated native table')
    return exe[raw:raw+count*width]


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


def field_addresses(repo, base, layouts=('OBJECT', 'NPC', 'PLANE')):
    """Derive packed PT offsets from the same Rust schema used by extraction."""
    schema = (repo/'crates/tore-formats/src/aircraft_schema.rs').read_text()
    result, offset = [], 0
    sizes = {'byte': 1, 'word': 2, 'dword': 4, 'ptr': 4, 'symbol': 4}
    for section in layouts:
        block = schema.split('pub const '+section+':', 1)[1].split('];', 1)[0]
        fields = re.findall(r'\("(\w+)", "([^"]+)"\)', block)
        if not fields:
            raise ValueError('unrecognized aircraft schema')
        for kind, name in fields:
            result.append({'section': section, 'field': name, 'offset': offset,
                           'va': base+offset, 'width': sizes[kind]})
            offset += sizes[kind]
    return result


def extract(source, output, *, overwrite=False, preview=False, domain='flight'):
    if domain not in ('flight', 'weapons', 'menus', 'weather'):
        raise ValueError('unknown native research domain')
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
    if domain == 'weapons':
        from native_weapons import KEYWORDS
        selected = [s for s in names if executable(s['va']) and any(t in s['name'].lower() for t in KEYWORDS)]
    if domain == 'menus':
        from native_menus import KEYWORDS
        selected = [s for s in names if executable(s['va']) and any(t in s['name'].lower() for t in KEYWORDS)]
    if domain == 'weather':
        from native_weather import KEYWORDS
        selected = [s for s in names if executable(s['va']) and any(t in s['name'].lower() for t in KEYWORDS)]
    report = {'schema_version': 1, 'method': 'static disassembly only; no retail execution',
              'exe_sha256': hashlib.sha256(exe).hexdigest(), 'sms_sha256': hashlib.sha256(sms).hexdigest(),
              'symbol_count': len(names), 'sections': rows, 'selected_symbol_count': len(selected)}
    report['reviewed_fa_build'] = (report['exe_sha256'] == REVIEWED_FA and report['sms_sha256'] == REVIEWED_SMS)
    report['domain'] = domain
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
    if report['reviewed_fa_build'] and domain == 'weapons':
        from native_weapons import artifacts as weapon_artifacts
        artifacts.update(weapon_artifacts(exe, rows, instructions, repo))
    if report['reviewed_fa_build'] and domain == 'weather':
        from native_weather import artifacts as weather_artifacts
        artifacts.update(weather_artifacts(exe, rows, instructions))
    if report['reviewed_fa_build'] and domain == 'menus':
        from native_menus import artifacts as menu_artifacts
        from native_menu_tables import ALIGNED_REGIONS, read_va
        artifacts.update(menu_artifacts(exe, rows, instructions))
        aligned = []
        for name, start, end in ALIGNED_REGIONS:
            code_bytes = read_va(exe, rows, start, end-start, executable=True)
            output_text = subprocess.run(
                [objdump, '-d', '--x86-asm-syntax=intel',
                 f'--start-address={start:#x}', f'--stop-address={end:#x}', str(files['FA.EXE'])],
                capture_output=True, text=True, check=True, timeout=120).stdout
            if len(output_text) > 1024*1024 or not re.search(rf'\b{start:x}:', output_text):
                raise ValueError('invalid aligned menu disassembly')
            artifacts[f'aligned/{start:08x}-{name}.txt'] = output_text
            aligned.append({'name': name, 'va': start, 'end': end,
                            'sha256': hashlib.sha256(code_bytes).hexdigest()})
        artifacts['aligned-regions.json'] = json.dumps(aligned, indent=2)+'\n'
    if report['reviewed_fa_build'] and domain == 'flight':
        cpt = next(s['va'] for s in names if s['name']=='_cpt')
        fields = field_addresses(repo,cpt)
        references = {}
        for address, line in instructions:
            for literal in set(re.findall(r'\b0x([0-9a-fA-F]+)\b', line)):
                references.setdefault(int(literal, 16), []).append(address)
        for field in fields:
            field['direct_references'] = references.get(field['va'], [])
        artifacts['pt-field-references.json'] = json.dumps(fields,indent=2)
        def decode_region(start, end):
            output = subprocess.run(
                [objdump, '-d', '--x86-asm-syntax=intel',
                 f'--start-address={start:#x}', f'--stop-address={end:#x}', str(files['FA.EXE'])],
                capture_output=True, text=True, check=True, timeout=120).stdout
            if len(output) > 1024*1024:
                raise ValueError('aligned flight disassembly exceeds bound')
            return [(int(match[1], 16), line) for line in output.splitlines()
                    if (match := re.match(r'\s*([0-9a-fA-F]+):\s', line))]

        artifacts.update(reviewed_regions(exe, rows, instructions, decode_region=decode_region))
        table = static_table(exe, rows, 0x515a48, 321)
        artifacts['tables/sine-q15.bin'] = table
        artifacts['tables/atan-pa.bin'] = static_table(exe, rows, 0x515644, 514)
        root_table = static_table(exe, rows, 0x51d624, 1024, 4)
        artifacts['tables/sqrt-seed.bin'] = root_table
        strip_template = static_table(exe, rows, 0x50ccc8, 0x134 // 2)
        artifacts['tables/strip-template.bin'] = strip_template
        artifacts['tables/inventory.json'] = json.dumps({
            'schema_version': 1, 'source_exe_sha256': report['exe_sha256'],
            'tables': [{'path': 'sine-q15.bin', 'va': 0x515a48, 'count': 321,
                        'format': 'little-endian signed 16-bit',
                        'sha256': hashlib.sha256(table).hexdigest(),
                        'consumer': '0x4cd588; sine[index] and cosine[index+64]'},
                       {'path': 'atan-pa.bin', 'va': 0x515644, 'count': 514,
                        'format': 'little-endian unsigned 16-bit',
                        'sha256': hashlib.sha256(static_table(exe, rows, 0x515644, 514)).hexdigest(),
                        'consumer': '0x4ccb88; octant interpolation'},
                       {'path': 'sqrt-seed.bin', 'va': 0x51d624, 'count': 1024,
                        'format': 'little-endian unsigned 32-bit',
                        'sha256': hashlib.sha256(root_table).hexdigest(),
                        'consumer': '0x4d65c4; seed plus one integer Newton step'},
                       {'path': 'strip-template.bin', 'va': 0x50ccc8, 'size': 0x134,
                        'format': 'packed inert airport template; pointer words are diagnostic only',
                        'sha256': hashlib.sha256(strip_template).hexdigest(),
                        'consumer': '0x4be2a0 mutates; 0x4ba800 copies; unknown fields not runtime accepted'}],
        }, indent=2)+'\n'
    artifacts['inventory.json'] = json.dumps(report,indent=2)+'\n'
    artifacts = {name: content.encode() if isinstance(content, str) else content
                 for name, content in artifacts.items()}
    # Preflight every output before changing anything. Never overwrite differing
    # research output unless explicitly requested; filenames never contain symbols.
    for name, content in artifacts.items():
        path = output/name
        if not path.resolve().is_relative_to(output):
            raise ValueError('output symlink escapes research directory')
        if path.exists() and path.read_bytes()!=content and not overwrite:
            raise ValueError(f'differing output {path}; choose a new --out or --overwrite')
    for name, content in artifacts.items():
        path=output/name;path.parent.mkdir(parents=True,exist_ok=True)
        path.write_bytes(content)
    print(f"Native {domain} research: {len(selected)} symbol spans, {len(names)} symbols; {output}")


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    parser.add_argument('--overwrite',action='store_true')
    parser.add_argument('--dry-run',action='store_true')
    parser.add_argument('--domain', choices=('flight', 'weather'), default='flight')
    args=parser.parse_args()
    try:
        extract(args.source,args.out,overwrite=args.overwrite,preview=args.dry_run,domain=args.domain)
    except (ValueError,OSError,subprocess.SubprocessError) as error:
        parser.exit(1,f'{error}\n')

if __name__=='__main__':
    main()
