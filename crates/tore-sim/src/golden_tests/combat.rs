//! Combat goldens: the live-fire adapter firing guns and bombs, radar,
//! supported and infrared missiles with seeker activation, decoys and
//! jamming, AI-owned shots at the player and at targets, hits, damage and the
//! debrief ledger.

use std::collections::BTreeMap;

use tore_formats::aircraft::AircraftId;
use tore_formats::weapons::{
    Burst, Countermeasures, Damage, Effects, Guidance, Movement, Seeker, Weapon, Zone,
};

use super::{Fingerprint, Probe, record_threat, run_twice, verify};
use crate::ai::DecisionRandom;
use crate::ai::threat::{DecoyOutcome, GuidingMissile, SeekerClass, decoy_missile};
use crate::attitude::Basis;
use crate::combat::FallState;
use crate::combat::ledger::Resolution;
use crate::combat::live::{
    ActorSupport, Command, Configuration, Event, Launcher, LocalizedDamage, PLAYER_OWNER,
    Projectile, State, Station, Target,
};
use crate::combat::missiles::{self, Flight, LaunchMode, Motion, TargetRole, seeker};
use crate::sensors;

// Recorded on macOS aarch64. See the module comment in golden_tests.rs before
// changing any of these.
const GUNS_AND_DAMAGE: u64 = 0x2a30_d8f8_7ca3_93b7;
const GUIDED_MISSILES: u64 = 0x594e_7f73_079b_4bb3;

#[test]
fn combat_guns_and_damage_match_recorded_fingerprint() {
    let seen = std::cell::RefCell::new(BTreeMap::new());
    let outcome = run_twice("combat/guns-and-damage", GUNS_AND_DAMAGE, |probe| {
        let (value, events) = guns_and_damage(probe);
        *seen.borrow_mut() = events;
        value
    });
    verify([outcome]);
    require(
        "combat/guns-and-damage",
        &seen.into_inner(),
        &["Fired", "Hit", "Destroyed", "Ground", "PlayerDamaged"],
    );
}

#[test]
fn combat_guided_missiles_match_recorded_fingerprint() {
    let seen = std::cell::RefCell::new(BTreeMap::new());
    let outcome = run_twice("combat/guided-missiles", GUIDED_MISSILES, |probe| {
        let (value, events) = guided_missiles(probe);
        *seen.borrow_mut() = events;
        value
    });
    verify([outcome]);
    require(
        "combat/guided-missiles",
        &seen.into_inner(),
        &[
            "Fired",
            "SeekerActivated",
            "Pitbull",
            "Hit",
            "TrackLost",
            "Defeated",
            "Spoofed",
            "PlayerDamaged",
            "SubsystemDamaged",
            "Jolt",
        ],
    );
}

/// A scenario must keep producing the outcomes its name promises.
fn require(scenario: &str, seen: &BTreeMap<&'static str, u32>, wanted: &[&str]) {
    for name in wanted {
        assert!(
            seen.get(name).is_some_and(|count| *count > 0),
            "{scenario} no longer produces {name}: {seen:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Recording

fn event_name(event: &Event) -> &'static str {
    match event {
        Event::Fired(_) => "Fired",
        Event::SeekerActivated(_) => "SeekerActivated",
        Event::Pitbull(_) => "Pitbull",
        Event::Hit(_) => "Hit",
        Event::Destroyed(_) => "Destroyed",
        Event::Airburst(_) => "Airburst",
        Event::Ground => "Ground",
        Event::TrackLost(_) => "TrackLost",
        Event::PlayerDamaged(_) => "PlayerDamaged",
        Event::SubsystemDamaged(_) => "SubsystemDamaged",
        Event::PlayerDestroyed => "PlayerDestroyed",
        Event::PilotKilled => "PilotKilled",
        Event::PlayerGroundImpact => "PlayerGroundImpact",
        Event::Defeated(_) => "Defeated",
        Event::Jolt(_) => "Jolt",
    }
}

fn record_event(fp: &mut Fingerprint, event: &Event) {
    fp.text(event_name(event));
    match event {
        Event::Fired(station) | Event::SubsystemDamaged(station) => fp.count(*station),
        Event::SeekerActivated(id)
        | Event::Pitbull(id)
        | Event::Hit(id)
        | Event::Destroyed(id)
        | Event::Airburst(id)
        | Event::TrackLost(id)
        | Event::Defeated(id) => fp.int(*id),
        Event::PlayerDamaged(amount) => fp.int(*amount),
        Event::Jolt(jolt) => {
            fp.option(jolt.target, |fp, id| fp.int(id));
            fp.vector(jolt.from);
            fp.f64(jolt.strength);
        }
        Event::Ground | Event::PlayerDestroyed | Event::PilotKilled | Event::PlayerGroundImpact => {
        }
    }
}

fn record_observation(fp: &mut Fingerprint, observation: &seeker::Observation) {
    fp.int(observation.id);
    fp.vector(observation.position);
    fp.vector(observation.velocity);
    fp.f64(observation.quality);
    fp.f64(observation.off_axis);
    fp.f64(observation.range);
}

fn record_seeker(fp: &mut Fingerprint, s: &seeker::Seeker) {
    fp.option(s.target, |fp, id| fp.int(id));
    fp.option(s.candidate, |fp, id| fp.int(id));
    fp.int(s.dwell);
    fp.int(s.missing);
    fp.bool(s.acquired);
    fp.name(&s.status);
    fp.f64(s.quality);
    fp.option(s.observation.as_ref(), record_observation);
}

fn record_flight_guidance(fp: &mut Fingerprint, flight: &Flight) {
    fp.bool(flight.unguided);
    fp.vector(flight.launch_origin);
    fp.option(flight.qualified_target, |fp, id| fp.int(id));
    fp.name(&flight.mode);
    record_seeker(fp, &flight.seeker);
    fp.bool(flight.enabled);
    fp.option(flight.last_intercept, |fp, point| fp.vector(point));
    fp.option(flight.solution, |fp, solution| {
        fp.vector(solution.point);
        fp.f64(solution.seconds);
    });
}

fn record_projectile(fp: &mut Fingerprint, p: &Projectile) {
    fp.int(p.id);
    fp.int(p.owner);
    fp.option(p.weapon.as_ref(), |fp, weapon| fp.text(&weapon.source));
    fp.option(p.guidance.as_ref(), record_flight_guidance);
    fp.option(p.motion, |fp, motion| {
        fp.vector(motion.velocity);
        fp.f64(motion.gain);
        fp.f64(motion.budget);
    });
    fp.option(p.guidance_ticks, |fp, ticks| fp.u64(ticks));
    fp.u64(p.age);
    fp.bool(p.incoming);
    fp.count(p.station);
    fp.vector(p.position);
    fp.vector(p.previous);
    fp.vector(p.direction);
    fp.int(p.speed_f8);
    fp.int(p.launched_t);
    fp.option(p.target, |fp, id| fp.int(id));
    fp.int(p.fall.velocity_f8);
    fp.option(p.gun_round, |fp, round| fp.int(round));
    fp.bool(p.tracer);
}

fn record_target(fp: &mut Fingerprint, t: &Target) {
    fp.int(t.id);
    fp.int(t.hp);
    fp.int(t.initial_hp);
    fp.vector(t.position);
    fp.vector(t.velocity);
    fp.vector(t.basis.forward);
    fp.vector(t.basis.up);
    fp.bool(t.airborne);
    fp.bool(t.on_ground);
    fp.bool(t.radar_emitting);
    fp.bool(t.jammer_active);
    fp.name(&t.heat);
    fp.option(t.wreck.as_ref(), |fp, wreck| {
        fp.name(&wreck.phase);
        fp.u64(wreck.ticks);
        fp.int(wreck.polls);
        fp.vector(wreck.angular_rates);
    });
    for amount in t.localized_damage.amounts {
        fp.int(amount);
    }
    fp.option(t.localized_damage.structural_variant, |fp, v| fp.count(v));
    fp.option(t.localized_damage.structural_section, |fp, s| fp.name(&s));
    fp.bool(t.fragment_released);
    fp.int(t.category);
}

/// Everything a tick of combat can change, as the host and the player see it.
fn record_state(fp: &mut Fingerprint, s: &mut State, launcher: Launcher) {
    for strike in s.take_strikes() {
        fp.int(strike.owner);
        fp.option(strike.victim, |fp, id| fp.int(id));
        fp.int(strike.weapon_flags);
        fp.bool(strike.destroyed);
    }
    for sound in s.take_sound_events() {
        fp.name(&sound.kind);
        fp.vector(sound.position);
        fp.bool(sound.arrived);
    }
    fp.u64(s.tick());
    fp.name(&s.release_readiness);
    fp.name(&s.launch_mode);
    record_seeker(fp, &s.mounted);
    fp.option(s.bore_observation.as_ref(), record_observation);
    fp.name(&s.weapon_rules);
    fp.count(s.ammo.len());
    for ammo in &s.ammo {
        fp.int(*ammo);
    }
    fp.count(s.selected);
    fp.bool(s.armed);
    fp.option(s.sensors.selected(), |fp, id| fp.int(id));
    fp.option(s.sensors.acquired(), |fp, id| fp.int(id));
    fp.count(s.sensors.contacts().len());
    for contact in s.sensors.contacts() {
        fp.int(contact.id);
        fp.name(&contact.channel);
        fp.vector(contact.position);
    }
    fp.count(s.projectiles.len());
    for projectile in &s.projectiles {
        record_projectile(fp, projectile);
    }
    fp.count(s.targets.len());
    for target in &s.targets {
        record_target(fp, target);
    }
    fp.count(s.effects.len());
    for effect in &s.effects {
        fp.name(&effect.kind);
        fp.vector(effect.position);
        fp.int(effect.ticks);
    }
    fp.count(s.smoke.puffs.len());
    for puff in &s.smoke.puffs {
        fp.name(&puff.kind);
        fp.vector(puff.position);
        fp.int(puff.age);
    }
    fp.count(s.debris.len());
    for piece in &s.debris {
        fp.int(piece.owner);
        fp.count(piece.variant);
        fp.vector(piece.position);
        fp.vector(piece.velocity);
    }
    for count in [s.shots, s.hits, s.kills] {
        fp.int(count);
    }
    fp.int(s.player_hp);
    fp.int(s.player_damage);
    for count in s.subsystem_counts {
        fp.int(count);
    }
    fp.option(s.last_subsystem, |fp, index| fp.count(index));
    for failed in [
        s.radar_failed,
        s.visual_failed,
        s.infrared_failed,
        s.rwr_failed,
        s.ecm_failed,
        s.target_jammer,
    ] {
        fp.bool(failed);
    }
    fp.int(s.chaff);
    fp.int(s.flares);
    fp.count(s.history.len());
    for record in &s.history {
        fp.u64(record.tick);
        fp.int(record.target);
        fp.count(record.station);
        fp.count(record.class);
        fp.int(record.nominal);
        fp.int(record.applied);
        fp.int(record.hp_after);
    }
    fp.int(s.range_category);
    let threats: Vec<_> = s.missile_threats.records().collect();
    fp.count(threats.len());
    for record in threats {
        record_threat(fp, record);
    }
    // The debrief ledger through its public reads only, so a per-shot
    // outcome list added later cannot move this fingerprint.
    let tallies: Vec<_> = s.ledger.tallies().collect();
    fp.count(tallies.len());
    for (key, tally) in tallies {
        fp.int(key.owner);
        fp.option(key.aim, |fp, id| fp.int(id));
        fp.name(&key.kind);
        for value in [
            tally.launched,
            tally.hit,
            tally.damage,
            tally.missed,
            tally.spoofed,
            tally.jammed,
        ] {
            fp.int(value);
        }
    }
    fp.count(s.ledger.kills().len());
    for kill in s.ledger.kills() {
        fp.int(kill.owner);
        fp.int(kill.victim);
        fp.int(kill.category);
        fp.bool(kill.aircraft);
    }
    let victims: Vec<u32> = std::iter::once(PLAYER_OWNER)
        .chain(s.targets.iter().map(|t| t.id))
        .collect();
    for victim in victims {
        fp.option(s.ledger.credit(victim), |fp, kill| {
            fp.int(kill.owner);
            fp.int(kill.category);
        });
    }
    // What the cockpit shows and plays.
    fp.name(&s.readiness(launcher));
    fp.bool(s.can_lock(launcher));
    fp.bool(s.guidance_available(launcher));
    fp.option(s.seeker_tone(launcher), |fp, tone| {
        fp.f64(tone.strength);
        fp.bool(tone.ground);
        fp.bool(tone.radar);
        fp.bool(tone.locked);
    });
    fp.option(s.estimated_max_range(launcher), |fp, range| fp.f64(range));
    fp.bool(s.in_estimated_range(launcher));
    fp.int(s.estimated_hit_percent(launcher));
    fp.option(s.favorable_firing_band(launcher), |fp, band| {
        fp.f64(band.minimum);
        fp.f64(band.maximum);
    });
    fp.option(s.designated(), |fp, id| fp.int(id));
    for region in s.player_damage_regions() {
        fp.f64(region);
    }
    fp.option(s.player_damage_section(), |fp, section| fp.name(&section));
    fp.f64(s.payload_lbs());
}

// ---------------------------------------------------------------------------
// Fixtures: private copies of the live-fire test fixture's synthetic values

fn zone(range: i32) -> Zone {
    Zone {
        heading: 12_000,
        pitch: 12_000,
        minimum_range: 0,
        maximum_range: range,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
    }
}

/// The live-fire fixture's synthetic weapon under a reviewed source name,
/// which selects the guidance profile. Signature 0 is unguided, 2 infrared
/// and 3 radar.
fn weapon(source: &str, signature: u8) -> Weapon {
    let guided = signature != 0;
    Weapon {
        source: source.into(),
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
            corner_speed: 1_000,
            maximum_speed: 2_000,
            acceleration: 100,
            deceleration: 2,
            initial_speed: 1_000,
            final_speed: 500,
            launch_retard: 100,
            ignite_t: 0,
            fuel_t: 10,
            remove_t: 20,
            powered_turn_rate: 10_000,
            unpowered_turn_rate: 10_000,
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
        seeker: Seeker {
            flags: [0; 2],
            signature,
            look_down: 0,
            doppler_above: 0,
            doppler_below: 0,
            doppler_minimum_range: 0,
            all_aspect: 0,
            zones: [zone(10_000); 2],
            chaff_flare_chance: 0,
            deception_chance: 0,
        },
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
    }
}

/// A longer-legged synthetic missile: 40 s of flight, 10 s of it powered.
fn missile(source: &str, signature: u8, range: i32, decoy_percent: u8) -> Weapon {
    let mut w = weapon(source, signature);
    w.movement.maximum_speed = 3_000;
    w.movement.initial_speed = 1_200;
    w.movement.final_speed = 1_500;
    w.movement.acceleration = 300;
    w.movement.fuel_t = 40;
    w.movement.remove_t = 160;
    w.movement.powered_turn_rate = 12_000;
    w.movement.unpowered_turn_rate = 8_000;
    w.seeker.zones = [zone(range); 2];
    w.seeker.chaff_flare_chance = decoy_percent;
    w.damage.by_class = [60; 5];
    w.damage.fuze_radius = 30;
    w
}

fn gun() -> Weapon {
    let mut w = weapon("M61.JT", 0);
    w.burst.actual_rounds_per_game = 2;
    w.burst.game_rounds_in_burst = 4;
    w.damage.by_class = [12; 5];
    w
}

fn bomb() -> Weapon {
    let mut w = weapon("MK82.JT", 0);
    w.flags = 0x14;
    w.movement.remove_t = 120;
    w.damage.by_class = [40; 5];
    w.damage.fuze_radius = 40;
    w
}

/// A 90/50 nm radar so the default display range selects track-while-scan,
/// plus the visual channel every aircraft has.
fn sensor_profiles() -> sensors::SensorProfiles {
    let volume = |nmi: f64| sensors::Volume {
        azimuth_rad: 1.,
        elevation_rad: 1.,
        minimum_ft: 0.,
        maximum_ft: nmi * sensors::FEET_PER_NAUTICAL_MILE,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    };
    sensors::SensorProfiles {
        aircraft: AircraftId::F18,
        radar: Some(sensors::RadarProfile {
            record: "GOLDEN.SEE".into(),
            search: volume(90.),
            track: volume(50.),
            look_down: 0.,
            preset: sensors::Preset::Advanced,
            notch: sensors::Preset::Advanced.notch(),
            resistance: sensors::Preset::Advanced.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }),
        infrared: None,
        visual: Some(sensors::profile::VisualProfile {
            record: "GOLDEN.VIS".into(),
            search: volume(10.),
            track: volume(5.),
        }),
        jammer: None,
        signature: sensors::SignatureProfile::default(),
    }
}

fn configuration(stations: Vec<(Weapon, u16, bool)>) -> Configuration {
    let slots = stations.len();
    Configuration {
        fragment_offsets: [[0.; 3]; 2],
        ecm: Countermeasures {
            weight: 0,
            flags: 0,
            mode_flags: 0x110,
            chaff: [4, 0, 0, 0],
            flare: [4, 0, 0, 0],
            radar_deception_chance: 30,
            radar_signature_add: 0,
            radar_noise_range: [0; 2],
            infrared_deception_chance: 20,
            infrared_signature_add: 0,
            infrared_lose_lock_time: 0,
        },
        system_damage: [0x11; 45],
        damage_capacity: 60,
        afterburner_available: true,
        hardpoint_slots: (0..slots).map(Some).chain([None, None, None]).collect(),
        radar_hardpoint: slots,
        visual_hardpoint: slots + 2,
        ecm_hardpoint: slots + 1,
        aircraft: AircraftId::F18,
        stations: stations
            .into_iter()
            .map(|(weapon, count, internal)| Station {
                weapon,
                mount: [0.; 3],
                count,
                internal,
            })
            .collect(),
        hit_points: 20,
        target_category: 0x80,
        external_equipment_lbs: 0,
        external_fuel_lbs: [0.; 9],
        engines: 1,
        wreck_power: crate::wreck::Power::default(),
        infrared_hardpoint: None,
        rwr_hardpoint: None,
        sensors: sensor_profiles(),
    }
}

fn aircraft_target(id: u32, position: [f64; 3], velocity: [f64; 3], hp: i32) -> Target {
    let forward = crate::attitude::unit(velocity);
    Target {
        aircraft: Some(AircraftId::Mig29),
        role: TargetRole::Aircraft,
        heat: seeker::Heat::Engine {
            on: true,
            throttle: 0.8,
            afterburner: false,
        },
        radar_emitting: true,
        id,
        position,
        velocity,
        basis: Basis::new(forward[0].atan2(forward[2]), forward[1].asin(), 0.),
        configuration: sensors::Configuration::CLEAN,
        signature: sensors::SignatureProfile::default(),
        jammer: None,
        jammer_active: false,
        airborne: true,
        on_ground: false,
        radius: 28.,
        hp,
        initial_hp: hp,
        fragment_offsets: [[0.; 3]; 2],
        wreck: None,
        wreck_power: crate::wreck::Power::symmetric(2, 20., 30.),
        fragment_released: false,
        localized_damage: LocalizedDamage::default(),
        category: 0x80,
    }
}

/// The player's aircraft as combat sees it: level, straight and fast, with
/// the host moving it between ticks.
fn launcher(position: [f64; 3], heading: f64, pitch: f64, speed: f64) -> Launcher {
    let basis = Basis::new(heading, pitch, 0.);
    Launcher {
        radar_power: true,
        position,
        basis,
        speed_fps: speed,
        velocity: basis.forward.map(|axis| axis * speed),
        bay_ready: true,
        radar: true,
        jammer: false,
        alive: true,
        controls: sensors::Controls::default(),
    }
}

fn fly(launcher: &mut Launcher) {
    for (position, velocity) in launcher.position.iter_mut().zip(launcher.velocity) {
        *position += velocity * crate::flight::DT;
    }
}

/// The host's decoy rule for one released chaff or flare, as the app applies
/// it: matching seeker class, guiding on the releaser, a percentage roll.
fn release_decoy(
    s: &mut State,
    class: SeekerClass,
    releaser: u32,
    random: &mut DecisionRandom,
    fp: &mut Fingerprint,
    seen: &mut BTreeMap<&'static str, u32>,
) {
    let config = s.configuration().clone();
    for p in &mut s.projectiles {
        let w = p.weapon(&config);
        let seeker = match w.seeker.signature {
            2 => SeekerClass::Infrared,
            3 => SeekerClass::Radar,
            _ => continue,
        };
        let guiding = p.target == Some(releaser)
            && p.guidance.as_ref().is_none_or(|flight| {
                flight.enabled && flight.seeker.acquired && flight.seeker.observation.is_some()
            });
        let outcome = decoy_missile(
            &GuidingMissile {
                seeker,
                guiding_on_releaser: guiding,
                decoy_susceptibility_percent: w.seeker.chaff_flare_chance,
            },
            class,
            100,
            random,
        )
        .expect("bounded percentages");
        fp.name(&outcome);
        if outcome == DecoyOutcome::Decoyed {
            *seen.entry("Spoofed").or_default() += 1;
            s.ledger.resolve(p.id, Resolution::Spoofed);
            p.target = None;
            p.guidance = None;
        }
    }
}

/// An AI actor's missile, created the way the app realises an AI launch.
fn owned_missile(
    id: u32,
    owner: u32,
    weapon: &Weapon,
    origin: [f64; 3],
    target: u32,
    aim: [f64; 3],
    tick: u64,
) -> Projectile {
    let profile = missiles::Profile::for_weapon(weapon).expect("reviewed missile");
    let direction = crate::attitude::unit(std::array::from_fn(|i| aim[i] - origin[i]));
    let velocity = direction.map(|axis| axis * 900.);
    Projectile {
        id,
        owner,
        weapon: Some(weapon.clone()),
        guidance: Some(Flight::from_supported_launch(
            profile,
            LaunchMode::Cued,
            seeker::Observation {
                id: target,
                position: aim,
                velocity: [0.; 3],
                quality: 1.,
                off_axis: 0.,
                range: missiles::length(missiles::sub(aim, origin)),
            },
            origin,
        )),
        motion: Some(Motion::new(&weapon.movement, velocity, origin[1])),
        guidance_ticks: Some(profile.guidance_ticks),
        age: 0,
        incoming: target == PLAYER_OWNER,
        station: 0,
        position: origin,
        previous: origin,
        direction,
        speed_f8: crate::combat::launch_speed(&weapon.movement, 900 * 256).expect("valid speeds")
            * 256,
        launched_t: (tick / 30) as u16,
        target: Some(target),
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    }
}

fn step(
    s: &mut State,
    held: bool,
    launcher: Launcher,
    fp: &mut Fingerprint,
    seen: &mut BTreeMap<&'static str, u32>,
) {
    let events = s.step(held, launcher, |_, _| 0.);
    fp.bool(held);
    fp.count(events.len());
    for event in &events {
        record_event(fp, event);
        *seen.entry(event_name(event)).or_default() += 1;
    }
    record_state(fp, s, launcher);
}

// ---------------------------------------------------------------------------
// Scenario: guns, bombs, player damage and range commands

fn guns_and_damage(probe: &Probe) -> (u64, BTreeMap<&'static str, u32>) {
    let mut fp = Fingerprint::default();
    let mut seen = BTreeMap::new();
    let mut s = State::new(
        configuration(vec![(gun(), 600, true), (bomb(), 6, false)]),
        true,
    )
    .expect("synthetic configuration");
    let mut l = launcher([0., 3_000., 0.], 0., 0., 600.);
    s.range_target(l);
    s.targets.push(aircraft_target(
        20,
        [800., 3_150., 2_000.],
        [-200., 0., 250.],
        20,
    ));
    s.targets.push(aircraft_target(
        21,
        [0., 3_050., 3_500.],
        [0., 0., 450.],
        60,
    ));
    s.add_ground_target(
        0x4000_0001,
        crate::airport::OrientedBox {
            center: [0., 0., 9_000.],
            half: [60., 20., 60.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        },
        30,
        0x100,
    )
    .expect("ground target");
    for tick in 0..1_800u64 {
        let pitch = if (640..900).contains(&tick) { -0.6 } else { 0. };
        let basis = Basis::new(0., pitch, 0.);
        l.basis = basis;
        l.velocity = basis.forward.map(|axis| axis * l.speed_fps);
        match tick {
            300 => s.command(Command::DamagePlayer, l),
            360 | 420 => s.command(Command::Incoming, l),
            480 => s.cheats.easy_aiming = true,
            600 => s.cheats.easy_aiming = false,
            620 => s.command(Command::NextWeapon, l),
            650 => s.command(Command::Incoming, l),
            900 => s.command(Command::SelectNav, l),
            920 => s.command(Command::NextSelection, l),
            950 => {
                s.command(Command::ToggleArm, l);
                s.command(Command::ToggleArm, l);
            }
            1_000 => {
                let mut a = aircraft_target(30, [5_000., 3_000., 8_000.], [0., 0., 600.], 20);
                a.velocity = [0., 0., 600.];
                let mut b = aircraft_target(31, [5_000., 3_000., 8_035.], [0., 0., -600.], 20);
                b.velocity = [0., 0., -600.];
                s.targets.extend([a, b]);
            }
            1_350 => s.command(Command::FailStation, l),
            1_400 => {
                s.command(Command::SelectNav, l);
                s.command(Command::NextSelection, l);
                s.cheats.unlimited_ammo = true;
            }
            1_550 => s.command(Command::ReplaceTarget, l),
            1_650 => s.command(Command::CycleClass, l),
            1_700 => s.command(Command::TargetDistance(2_000), l),
            1_750 => s.command(Command::ClearRange, l),
            _ => {}
        }
        let held = matches!(
            tick,
            0..180 | 240..480 | 480..600 | 700..760 | 1_300..1_500 | 1_560..1_640
        );
        step(&mut s, held, l, &mut fp, &mut seen);
        fly(&mut l);
        if tick % 150 == 149 {
            probe.part(format!("tick {tick}"), fp.value());
        }
    }
    (fp.value(), seen)
}

// ---------------------------------------------------------------------------
// Scenario: guided missiles, decoys, jamming and AI-owned shots

fn guided_missiles(probe: &Probe) -> (u64, BTreeMap<&'static str, u32>) {
    let mut fp = Fingerprint::default();
    let mut seen = BTreeMap::new();
    let active = missile("AIM120.JT", 3, 60_000, 30);
    let supported = missile("R530.JT", 3, 40_000, 30);
    let infrared = missile("AIM9M.JT", 2, 20_000, 60);
    // No reviewed profile: the compatibility pursuit path.
    let compatibility = missile("SYNTHETIC.JT", 3, 40_000, 0);
    let mut s = State::new(
        configuration(vec![
            (gun(), 200, true),
            (active.clone(), 4, false),
            (supported, 2, false),
            (compatibility, 2, false),
            (infrared.clone(), 4, false),
        ]),
        true,
    )
    .expect("synthetic configuration");
    let mut l = launcher([0., 20_000., 0.], 0., 0., 800.);
    s.targets.push(aircraft_target(
        40,
        [3_000., 20_000., 42_000.],
        [0., 0., -700.],
        60,
    ));
    s.targets.push(aircraft_target(
        41,
        [-2_500., 19_500., 12_000.],
        [300., 0., 500.],
        60,
    ));
    let mut hot = aircraft_target(42, [0., 20_300., 7_000.], [0., 0., 650.], 60);
    hot.heat = seeker::Heat::Engine {
        on: true,
        throttle: 1.,
        afterburner: true,
    };
    s.targets.push(hot);
    let mut decoys = DecisionRandom::seeded(0x5eed_dec0);
    for tick in 0..2_400u64 {
        // The host flies the targets: one weaves, one notches, one turns.
        for target in &mut s.targets {
            let t = tick as f64 / 120.;
            match target.id {
                40 if tick >= 900 => target.velocity = [700., 0., 0.],
                41 => {
                    let angle = t * 0.15;
                    target.velocity = [300. * angle.cos(), 20. * (t * 0.5).sin(), 500.];
                }
                42 => target.velocity = [120. * (t * 0.4).sin(), 0., 650.],
                _ => {}
            }
        }
        match tick {
            2 => {
                s.command(Command::NextWeapon, l);
                s.command(Command::DesignateTarget(40), l);
            }
            320 => s.command(Command::DesignateTarget(41), l),
            600 => {
                s.command(Command::NextWeapon, l);
                s.command(Command::DesignateTarget(41), l);
            }
            720 => s.command(Command::NextWeapon, l),
            760 => l.radar = false,
            880 => l.radar = true,
            1_000 => {
                s.command(Command::NextWeapon, l);
                s.command(Command::ClearDesignation, l);
            }
            1_300 => s.command(Command::ToggleTargetJammer, l),
            1_320 => {
                s.command(Command::SelectNav, l);
                s.command(Command::NextSelection, l);
                s.command(Command::NextSelection, l);
                s.command(Command::DesignateTarget(40), l);
            }
            1_500 => {
                let aim = l.position;
                s.projectiles.push(owned_missile(
                    5_000,
                    7,
                    &active,
                    [2_000., 20_000., 15_000.],
                    PLAYER_OWNER,
                    aim,
                    tick,
                ));
            }
            1_600 => {
                let aim = s
                    .targets
                    .iter()
                    .find(|t| t.id == 41)
                    .map_or([0.; 3], |t| t.position);
                s.projectiles.push(owned_missile(
                    5_001,
                    8,
                    &infrared,
                    [-6_000., 19_500., 30_000.],
                    41,
                    aim,
                    tick,
                ));
            }
            _ => {}
        }
        // Owner 7 keeps its radar on the player for its supported shot.
        s.set_actor_supports([ActorSupport {
            owner: 7,
            observation: Some(seeker::Observation {
                id: PLAYER_OWNER,
                position: l.position,
                velocity: l.velocity,
                quality: 1.,
                off_axis: 0.,
                range: 15_000.,
            }),
            supported: true,
            radar_position: [2_000., 20_000., 15_000.],
            radar_emitting: true,
        }]);
        // Flares from the hot target, chaff from the player.
        if (1_150..1_300).contains(&tick) && tick.is_multiple_of(30) {
            release_decoy(
                &mut s,
                SeekerClass::Infrared,
                42,
                &mut decoys,
                &mut fp,
                &mut seen,
            );
        }
        if (1_700..1_900).contains(&tick) && tick.is_multiple_of(30) {
            release_decoy(
                &mut s,
                SeekerClass::Radar,
                PLAYER_OWNER,
                &mut decoys,
                &mut fp,
                &mut seen,
            );
        }
        let held = matches!(tick, 200 | 450 | 700 | 740 | 1_100 | 1_140 | 1_350 | 1_380);
        step(&mut s, held, l, &mut fp, &mut seen);
        fly(&mut l);
        if tick % 200 == 199 {
            probe.part(format!("tick {tick}"), fp.value());
        }
    }
    (fp.value(), seen)
}
