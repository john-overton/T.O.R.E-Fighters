//! The imported records copied into mission state, coded by value: weapon
//! records, ownship and AI configurations (docs/formats/checkpoint.md,
//! "Shared records"). Callers code them through [`super::Saver::shared`], so
//! equal copies cost one coding.
//!
//! Every field of every type is named, so a field added to a record without
//! coding it fails to compile here. The weapon types live in `tore-formats`
//! and have public fields; `Configuration` and `Station` are `combat::live`'s.
//! A station's weapon is a shared record (a weapon carried by many stations,
//! on many aircraft, is coded once), and so is a configuration's sensor
//! profile set, which the aircraft's `Sensors` carries a second copy of.

use crate::combat::live::{Configuration, Station};
use tore_formats::weapons::{
    Burst, Countermeasures, Damage as WeaponDamage, Effects, Guidance as WeaponGuidance, Movement,
    Seeker, Weapon, Zone,
};

crate::checkpoint_struct!(Zone {
    heading,
    pitch,
    minimum_range,
    maximum_range,
    minimum_altitude,
    maximum_altitude,
});

crate::checkpoint_struct!(Seeker {
    flags,
    signature,
    look_down,
    doppler_above,
    doppler_below,
    doppler_minimum_range,
    all_aspect,
    zones,
    chaff_flare_chance,
    deception_chance,
});

crate::checkpoint_struct!(Movement {
    minimum_speed,
    corner_speed,
    maximum_speed,
    acceleration,
    deceleration,
    initial_speed,
    final_speed,
    launch_retard,
    ignite_t,
    fuel_t,
    remove_t,
    powered_turn_rate,
    unpowered_turn_rate,
    performance_at_0,
    performance_at_20,
    cruise,
    jink,
});

crate::checkpoint_struct!(Burst {
    projectiles_in_pod,
    actual_rounds_per_game,
    game_rounds_in_burst,
    game_rounds_in_carpet_burst,
    game_burst_t,
    reload_t,
    startup_shots,
    random_fire_percent,
    offset_fire_percent,
    offset_fire_heading,
    offset_fire_pitch,
    sine_pattern,
});

crate::checkpoint_struct!(WeaponGuidance {
    track_t,
    track_max_g_raw,
    target_sun_chance,
    max_aon,
    chances,
    hit_modifiers,
});

crate::checkpoint_struct!(WeaponDamage {
    by_class,
    fuze_arm_t,
    fuze_radius,
    side_hit_fuze_failure,
    collateral_radius,
    collateral_percent,
});

crate::checkpoint_struct!(Effects {
    object_explosion,
    land_explosion,
    water_explosion,
    crater_size,
    smoke,
    max_sound_distance,
    frequency_adjustment,
});

crate::checkpoint_struct!(Weapon {
    source,
    name,
    hud_name,
    shape,
    fire_sound,
    native_callback,
    flags,
    object_flags,
    weight,
    movement,
    burst,
    seeker,
    guidance,
    damage,
    effects,
});

crate::checkpoint_struct!(Countermeasures {
    weight,
    flags,
    mode_flags,
    chaff,
    flare,
    radar_deception_chance,
    radar_signature_add,
    radar_noise_range,
    infrared_deception_chance,
    infrared_signature_add,
    infrared_lose_lock_time,
});

crate::checkpoint_struct!(Station { mount, count, internal } shared { weapon });

// A configuration is imported once per aircraft type and never changes, but a
// handoff can copy it into state the fresh world does not hold, so callers
// code it as a shared record. Its sensor profile set is shared inside it, so
// the `Sensors` copy of the same profiles costs only an index.
crate::checkpoint_struct!(Configuration {
    ecm,
    system_damage,
    damage_capacity,
    fragment_offsets,
    afterburner_available,
    hardpoint_slots,
    radar_hardpoint,
    visual_hardpoint,
    infrared_hardpoint,
    rwr_hardpoint,
    ecm_hardpoint,
    aircraft,
    stations,
    hit_points,
    target_category,
    external_equipment_lbs,
    external_fuel_lbs,
    engines,
    wreck_power,
} shared { sensors });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};
    use crate::sensors::{
        Generation, InfraredProfile, JammerProfile, Preset, RadarProfile, SensorProfiles,
        SignatureProfile, Volume,
    };
    use tore_formats::aircraft::AircraftId;

    /// A weapon record with every field a different, non-default value, so a
    /// field left out of the coder or swapped with another changes the result.
    fn weapon(seed: u8) -> Weapon {
        let n = i16::from(seed);
        let zone = |k: i16| Zone {
            heading: -k,
            pitch: k * 3,
            minimum_range: i32::from(k) * 10,
            maximum_range: i32::from(k) * 1000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        Weapon {
            source: format!("W{seed}.JT"),
            name: format!("Missile {seed}"),
            hud_name: "AAM".into(),
            shape: Some("SHAPE".into()),
            fire_sound: None,
            native_callback: String::new(),
            flags: 0x8000_0001 | u32::from(seed),
            object_flags: u32::from(seed) << 9,
            weight: -5 * i32::from(seed),
            movement: Movement {
                minimum_speed: n,
                corner_speed: n + 1,
                maximum_speed: n + 2,
                acceleration: 70_000,
                deceleration: -70_000,
                initial_speed: n + 3,
                final_speed: n + 4,
                launch_retard: seed,
                ignite_t: 300 + u16::from(seed),
                fuel_t: 600,
                remove_t: 900,
                powered_turn_rate: -n,
                unpowered_turn_rate: n * 2,
                performance_at_0: 10,
                performance_at_20: 20,
                cruise: [1, 2, 3, seed],
                jink: [-1, 2, -3],
            },
            burst: Burst {
                projectiles_in_pod: n,
                actual_rounds_per_game: 3,
                game_rounds_in_burst: 4,
                game_rounds_in_carpet_burst: 5,
                game_burst_t: 6,
                reload_t: 7,
                startup_shots: 8,
                random_fire_percent: 9,
                offset_fire_percent: 10,
                offset_fire_heading: -11,
                offset_fire_pitch: 12,
                sine_pattern: [13, 14, 15, 16],
            },
            seeker: Seeker {
                flags: [1, 2],
                signature: 3,
                look_down: 4,
                doppler_above: 5,
                doppler_below: 6,
                doppler_minimum_range: 7,
                all_aspect: 8,
                zones: [zone(n), zone(n + 1)],
                chaff_flare_chance: 9,
                deception_chance: 10,
            },
            guidance: WeaponGuidance {
                track_t: 1,
                track_max_g_raw: 2,
                target_sun_chance: 3,
                max_aon: 4,
                chances: [5, 6, 7, 8],
                hit_modifiers: [1, 2, 3, 4, 5, 6, 7, 8, 9],
            },
            damage: WeaponDamage {
                by_class: [1, -2, 3, -4, 5],
                fuze_arm_t: 6,
                fuze_radius: 7,
                side_hit_fuze_failure: 8,
                collateral_radius: 9,
                collateral_percent: 10,
            },
            effects: Effects {
                object_explosion: 1,
                land_explosion: 2,
                water_explosion: 3,
                crater_size: 4,
                smoke: [5, 6, 7, 8, 9],
                max_sound_distance: -10,
                frequency_adjustment: 11,
            },
        }
    }

    fn profiles() -> SensorProfiles {
        let volume = Volume {
            azimuth_rad: 1.,
            elevation_rad: 0.5,
            minimum_ft: 0.,
            maximum_ft: 6076. * 40.,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        };
        let preset = Preset::Advanced;
        SensorProfiles {
            aircraft: AircraftId::F18,
            radar: Some(RadarProfile {
                record: "F18R.SEE".into(),
                search: volume,
                track: volume,
                look_down: 50.,
                preset,
                notch: preset.notch(),
                resistance: preset.resistance(),
                band: 0,
                source_flags: [1, 2],
                source_doppler: [3, 4, 5],
            }),
            infrared: Some(InfraredProfile {
                record: "F18I.SEE".into(),
                search: volume,
                track: volume,
            }),
            visual: None,
            jammer: Some(JammerProfile {
                record: "F18.ECM".into(),
                generation: Generation::LateColdWar,
                strength: 0.3,
                band: 0,
                radio_frequency: true,
            }),
            signature: SignatureProfile::default(),
        }
    }

    /// An ownship configuration with `count` stations that carry only
    /// `distinct` different weapons.
    fn configuration(count: usize, distinct: u8) -> Configuration {
        Configuration {
            ecm: Countermeasures {
                weight: 5,
                flags: 1,
                mode_flags: 0x110,
                chaff: [4, 1, 2, 3],
                flare: [4, 5, 6, 7],
                radar_deception_chance: 30,
                radar_signature_add: 2,
                radar_noise_range: [1, 9],
                infrared_deception_chance: 20,
                infrared_signature_add: 3,
                infrared_lose_lock_time: 4,
            },
            system_damage: std::array::from_fn(|i| i as u8),
            damage_capacity: 60,
            fragment_offsets: [[1., -0., 3.], [f64::INFINITY, 5., 6.]],
            afterburner_available: true,
            hardpoint_slots: (0..count).map(Some).chain([None, Some(0)]).collect(),
            radar_hardpoint: count,
            visual_hardpoint: count + 1,
            infrared_hardpoint: Some(2),
            rwr_hardpoint: None,
            ecm_hardpoint: count + 2,
            aircraft: AircraftId::F18,
            stations: (0..count)
                .map(|i| Station {
                    weapon: weapon((i as u8) % distinct),
                    mount: [i as f64, -1.5, 0.25],
                    count: 1 + i as u16,
                    internal: i % 2 == 0,
                })
                .collect(),
            hit_points: 20,
            target_category: 0x80,
            external_equipment_lbs: 300,
            external_fuel_lbs: [10., 20., 30., 0., 0., 0., 0., 0., 1.],
            engines: 2,
            wreck_power: crate::wreck::Power::default(),
            sensors: profiles(),
        }
    }

    #[test]
    fn a_weapon_record_round_trips_equal() {
        let models = Models::default();
        for seed in [0, 1, 200] {
            let original = weapon(seed);
            assert_eq!(round_trip(&original, &models).unwrap(), original);
        }
        let mut changed = weapon(1);
        changed.effects.smoke[4] ^= 1;
        assert_ne!(
            to_bytes(&changed, &models).unwrap(),
            to_bytes(&weapon(1), &models).unwrap()
        );
    }

    #[test]
    fn a_configuration_round_trips_and_codes_each_distinct_weapon_once() {
        let models = Models::default();
        // 30 stations carrying 3 different weapons.
        let original = configuration(30, 3);
        let copy = round_trip(&original, &models).unwrap();
        assert_eq!(format!("{copy:?}"), format!("{original:?}"));
        let coded = to_bytes(&original, &models).unwrap();
        // Three weapon records and the profile set, whatever the station count.
        assert_eq!(coded.records.len(), 4);
        let more = to_bytes(&configuration(32, 3), &models).unwrap();
        assert_eq!(more.records.len(), 4);
        // One weapon on every station: one weapon record and the profile set.
        assert_eq!(
            to_bytes(&configuration(30, 1), &models)
                .unwrap()
                .records
                .len(),
            2
        );
        // The bytes are the same however often the world codes them.
        assert_eq!(to_bytes(&copy, &models).unwrap(), coded);
    }

    #[test]
    fn many_configurations_and_their_sensors_share_records() {
        use crate::sensors::Sensors;
        let models = Models::default();
        // Thirty aircraft of one type: equal configurations, equal sensor
        // profiles in their `Sensors`, one coding of each record.
        let aircraft: Vec<(Configuration, Sensors)> = (0..30)
            .map(|_| {
                let config = configuration(8, 4);
                let sensors = Sensors::new(config.sensors.clone());
                (config, sensors)
            })
            .collect();
        let one = to_bytes(&aircraft[..1].to_vec(), &models).unwrap();
        let thirty = to_bytes(&aircraft, &models).unwrap();
        assert_eq!(thirty.records, one.records);
        // The two records of the configuration and its suite share one
        // profile record: 4 weapons and 1 profile set.
        assert_eq!(thirty.records.len(), 5);
    }

    #[test]
    fn a_configuration_nested_as_a_shared_record_keeps_the_weapons_shared() {
        // `World` codes a configuration as a shared record of its own, which
        // refers to the weapon and profile records written before it.
        let models = Models::default();
        let original = configuration(12, 3);
        let mut s = crate::checkpoint::Saver::with_models(models.clone());
        s.shared(&original).unwrap();
        s.shared(&original.stations[0].weapon).unwrap();
        let section = s.finish_section();
        let records = s.into_records();
        // Three weapons, the profile set and the configuration; the weapon
        // written again by hand is the first one's record, not a sixth.
        assert_eq!(records.len(), 5);
        let mut l = crate::checkpoint::Loader::new(&section, &records, &models);
        let copy: Configuration = l.shared().unwrap();
        let weapon: Weapon = l.shared().unwrap();
        l.finish().unwrap();
        assert_eq!(format!("{copy:?}"), format!("{original:?}"));
        assert_eq!(weapon, original.stations[0].weapon);
    }

    #[test]
    fn a_damaged_configuration_is_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&configuration(6, 2), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut damaged = coded.clone();
            damaged.body.truncate(cut);
            assert!(
                from_bytes::<Configuration>(&damaged, &models).is_err(),
                "cut {cut}"
            );
        }
        // A shared record that is not what its type tag says.
        let mut damaged = coded.clone();
        damaged.records[0][0] ^= 0xff;
        assert!(from_bytes::<Configuration>(&damaged, &models).is_err());
        for record in 0..coded.records.len() {
            let mut hostile = coded.clone();
            for byte in &mut hostile.records[record] {
                *byte = byte.wrapping_mul(31).wrapping_add(7);
            }
            let _ = from_bytes::<Configuration>(&hostile, &models);
        }
    }
}
