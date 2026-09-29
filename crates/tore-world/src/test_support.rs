//! Synthetic fixtures that tests in this crate and in the app share: a
//! four-cell terrain, an aircraft profile, a combat state, and the AI wings'
//! launch rows and spawned targets. No retail data. Compiled for this crate's
//! own tests and, through the `test-support` feature, for the app's.
use crate::terrain::Terrain;
use std::collections::BTreeMap;
use tore_formats::{
    aircraft::{Aircraft, AircraftId, Envelope, Token},
    theater::{Environment, Theater},
};
use tore_sim::{
    ai::{
        experience::EnemySkillOverride,
        launch::{self, WingId, WingLaunch, WingSelection, resolve_wings},
    },
    attitude::{Basis, Vector},
    combat::{
        live,
        missiles::{TargetRole, seeker::Heat},
    },
    sensors,
};

fn base_profile() -> Aircraft {
    let fields = [
        ("weight", 10000),
        ("internalFuel", 1000),
        ("thrust", 8000),
        ("aftThrust", 12000),
        ("fuelConsumption", 2),
        ("aftFuelConsumption", 10),
        ("maxTakeoffWeight", 15000),
        ("gearDrag", 23),
        ("flapsDrag", 70),
        ("airBrakesDrag", 256),
        ("loadedElevator", 40),
        ("loadedDrag", 0),
        ("_gpullDrag", 0),
    ]
    .into_iter()
    .map(|(k, v)| {
        (
            k.into(),
            Token {
                kind: "dword".into(),
                value: v.to_string(),
                scaled: false,
            },
        )
    })
    .collect();
    Aircraft {
        id: tore_formats::aircraft::AircraftId::F18,
        name: "F/A-18D".into(),
        shape: "F18.SH".into(),
        fields,
        object: BTreeMap::new(),
        hardpoints: vec![],
        sounds: BTreeMap::new(),
        envelopes: (-2..=6)
            .map(|g| Envelope {
                g,
                points: vec![[200., 0.], [250., 50000.], [1300., 50000.], [1800., 0.]],
            })
            .collect(),
    }
}
pub fn profile() -> Aircraft {
    let mut a = base_profile();
    for prefix in ["_brv.x", "puffRot.x", "puffRot.y", "puffRot.z"] {
        for (suffix, value) in [("min", -90), ("max", 90), ("acc", 200), ("dacc", 400)] {
            a.fields.insert(
                format!("{prefix}.{suffix}"),
                Token {
                    kind: "word".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        }
    }

    for key in [
        "flapsLift",
        "turbulencePercent",
        "rudderDrag",
        "bayDrag",
        "wheelBrakesDrag",
        "stallWarningDelay",
        "stallDelay",
        "stallSeverity",
        "stallPitchDown",
        "spinEntry",
        "spinExit",
        "spinYawLow",
        "spinYawHigh",
        "spinAOALow",
        "spinAOAHigh",
        "spinBankLow",
        "spinBankHigh",
        "crashSpeedForward",
        "crashSpeedSide",
        "crashSpeedVertical",
        "crashPitch",
        "crashRoll",
        "flags",
    ] {
        a.fields.insert(
            key.into(),
            Token {
                kind: "word".into(),
                value: "0".into(),
                scaled: false,
            },
        );
    }
    for key in ["gearDrag", "flapsDrag", "airBrakesDrag"] {
        a.fields.get_mut(key).unwrap().kind = "word".into();
    }
    for axis in ["x", "y", "z"] {
        for suffix in ["min", "max", "acc", "dacc"] {
            a.fields.insert(
                format!("_bv.{axis}.{suffix}"),
                Token {
                    kind: "word".into(),
                    value: if suffix == "min" { "-100" } else { "100" }.into(),
                    scaled: false,
                },
            );
        }
    }
    for (key, value) in [
        ("_brv.x.max", 100),
        ("crashSpeedForward", 330),
        ("crashSpeedSide", 50),
        ("crashSpeedVertical", 30),
        ("crashPitch", 25),
        ("crashRoll", 10),
        ("spinExit", -2),
    ] {
        a.fields.insert(
            key.into(),
            Token {
                kind: "word".into(),
                value: value.to_string(),
                scaled: false,
            },
        );
    }
    a
}

pub fn terrain() -> Terrain {
    use tore_formats::theater::TerrainCell;
    let cells = [0, 4, 8, 12]
        .map(|elevation| TerrainCell {
            color: 100,
            class: 2,
            elevation,
        })
        .to_vec();
    Terrain {
        theater: Theater {
            name: "Synthetic".into(),
            map: "T.PIC".into(),
            tiles: [1, 1],
            cells_per_tile: 2,
            cols: 2,
            rows: 2,
            cells,
            coarse: vec![],
        },
        environment: Environment::default(),
        airport_scene: tore_sim::airport::Scene::default(),
        airfield_anchors: BTreeMap::new(),
        static_manifest: Vec::new(),
        catalog: vec![],
        layout: "TEST.MM".into(),
        condition: None,
        weather: tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(
                tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                    .unwrap(),
                12,
                0,
                0,
                None,
            )
            .unwrap(),
        ),
    }
}

pub fn combat_fixture(guided: bool) -> live::State {
    use live::{Configuration, State, Station};
    use tore_formats::weapons::*;
    let zone = Zone {
        heading: 12000,
        pitch: 12000,
        minimum_range: 0,
        maximum_range: 10000,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
    };
    let seeker = Seeker {
        flags: [0; 2],
        signature: if guided { 3 } else { 0 },
        look_down: 0,
        doppler_above: 0,
        doppler_below: 0,
        doppler_minimum_range: 0,
        all_aspect: 0,
        zones: [zone; 2],
        chaff_flare_chance: 0,
        deception_chance: 0,
    };
    let w = Weapon {
        source: "SYNTHETIC.JT".into(),
        name: "Synthetic".into(),
        hud_name: "SYN".into(),
        shape: None,
        fire_sound: None,
        native_callback: "_PROJProc".into(),
        flags: if guided { 0x240 } else { 0x844 },
        object_flags: 0,
        weight: 10,
        movement: Movement {
            minimum_speed: 10,
            corner_speed: 1000,
            maximum_speed: 2000,
            acceleration: 100,
            deceleration: 2,
            initial_speed: 1000,
            final_speed: 500,
            launch_retard: 100,
            ignite_t: 0,
            fuel_t: 10,
            remove_t: 20,
            powered_turn_rate: 10000,
            unpowered_turn_rate: 10000,
            performance_at_0: 100,
            performance_at_20: 100,
            cruise: [0; 4],
            jink: [0; 3],
        },
        burst: Burst {
            projectiles_in_pod: 1,
            actual_rounds_per_game: 2,
            game_rounds_in_burst: 1,
            game_rounds_in_carpet_burst: 1,
            game_burst_t: 1,
            reload_t: 0,
            startup_shots: 0,
            random_fire_percent: 0,
            offset_fire_percent: 0,
            offset_fire_heading: 0,
            offset_fire_pitch: 0,
            sine_pattern: [0; 4],
        },
        seeker,
        guidance: Guidance {
            track_t: 1,
            track_max_g_raw: 1,
            target_sun_chance: 0,
            max_aon: 0,
            chances: [100; 4],
            hit_modifiers: [0; 9],
        },
        damage: Damage {
            by_class: [10; 5],
            fuze_arm_t: 0,
            fuze_radius: 0,
            side_hit_fuze_failure: 0,
            collateral_radius: 0,
            collateral_percent: 0,
        },
        effects: Effects {
            object_explosion: 0,
            land_explosion: 0,
            water_explosion: 0,
            crater_size: 0,
            smoke: [0; 5],
            max_sound_distance: 0,
            frequency_adjustment: 0,
        },
    };
    State::new(
        Configuration {
            fragment_offsets: [[0.; 3]; 2],
            ecm: tore_formats::weapons::Countermeasures {
                weight: 0,
                flags: 0,
                mode_flags: 0x10,
                chaff: [0; 4],
                flare: [0; 4],
                radar_deception_chance: 30,
                radar_signature_add: 0,
                radar_noise_range: [0; 2],
                infrared_deception_chance: 0,
                infrared_signature_add: 0,
                infrared_lose_lock_time: 0,
            },
            system_damage: [0x11; 45],
            damage_capacity: 30,
            afterburner_available: true,
            hardpoint_slots: vec![Some(0)],
            radar_hardpoint: 1,
            visual_hardpoint: 3,
            ecm_hardpoint: 2,
            aircraft: AircraftId::F18,
            stations: vec![Station {
                weapon: w,
                mount: [0.; 3],
                count: 11,
                internal: !guided,
            }],
            hit_points: 20,
            target_category: 0x80,
            external_equipment_lbs: 0,
            external_fuel_lbs: [0.; 9],
            engines: 1,
            wreck_power: tore_sim::wreck::Power::default(),
            infrared_hardpoint: None,
            rwr_hardpoint: None,
            sensors: sensors::SensorProfiles {
                aircraft: AircraftId::F18,
                radar: None,
                infrared: None,
                visual: None,
                jammer: None,
                signature: sensors::SignatureProfile::default(),
            },
        },
        true,
    )
    .unwrap()
}

pub fn aircraft() -> Aircraft {
    crate::test_support::profile()
}

/// Two friendly aircraft in wing 2 and two enemy aircraft in wing 1, the
/// same shape `--ai-probe-ticks` flies.
pub fn payload(enemy_override: Option<EnemySkillOverride>) -> Vec<WingLaunch> {
    let selections = [
        (launch::Side::Friendly, 1u8, 2usize, 1i32),
        (launch::Side::Enemy, 0, 2, 3),
    ]
    .map(|(side, index, count, skill_level)| WingSelection {
        wing: WingId::new(side, index).unwrap(),
        aircraft: AircraftId::F18,
        count,
        skill_level,
    });
    resolve_wings(&selections, enemy_override).unwrap()
}

/// The four rows `Combat::reset` would have spawned: friendly pair facing
/// the enemy pair, which face back.
pub fn spawned() -> Vec<live::Target> {
    vec![
        target(1, [0., 20000., 0.], 0.),
        target(2, [1500., 20000., 0.], 0.),
        target(3, [0., 20000., 40000.], std::f64::consts::PI),
        target(4, [1500., 20000., 40000.], std::f64::consts::PI),
    ]
}

pub fn target(id: u32, position: Vector, yaw: f64) -> live::Target {
    let basis = Basis::new(yaw, 0., 0.);
    live::Target {
        aircraft: Some(AircraftId::F18),
        role: TargetRole::Aircraft,
        heat: Heat::Engine {
            on: true,
            throttle: 0.7,
            afterburner: false,
        },
        radar_emitting: false,
        id,
        position,
        velocity: basis.forward.map(|v| v * 300.),
        basis,
        configuration: sensors::Configuration::CLEAN,
        signature: sensors::SignatureProfile::default(),
        jammer: None,
        jammer_active: false,
        airborne: true,
        on_ground: false,
        radius: 28.,
        hp: 100,
        initial_hp: 100,
        fragment_offsets: [[0.; 3]; 2],
        wreck: None,
        wreck_power: tore_sim::wreck::Power::default(),
        fragment_released: false,
        localized_damage: live::LocalizedDamage::default(),
        faults: Default::default(),
        category: 0,
        side: tore_sim::combat::live::NO_SIDE,
    }
}
