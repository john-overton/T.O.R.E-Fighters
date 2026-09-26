//! Flight-model goldens: every synthetic aircraft configuration flies the same
//! scripted maneuvers on the legacy and hybrid adapters, the hybrid adapter
//! also takes off and lands on a synthetic runway, and the restricted native
//! adapter flies on synthetic tables. Each maneuver is one scenario covering
//! every configuration, so a failure names the maneuver that changed.

use std::cell::Cell;

use tore_input::{PilotCommand, PilotInput, Switch};

use super::{
    Fingerprint, Outcome, Probe, fixtures, record_flight, record_input, run_twice, verify,
};
use crate::attitude::Basis;
use crate::flight::State;
use crate::models::FlightModel;
use crate::research::Surface;
use crate::turbulence::Disturbance;

// Recorded on macOS aarch64. See the module comment in golden_tests.rs before
// changing any of these.
const LEGACY: [(&str, u64); 8] = [
    ("cruise-roll-rudder", 0xdf13_3efa_e953_0ae7),
    ("burner-loop", 0x919d_ddec_36c5_6529),
    ("stall-and-spin", 0x0ab3_9175_f69d_6f73),
    ("devices-and-switches", 0x525e_f6b0_d477_8e4b),
    ("damage-and-disturbance", 0x6c94_a6dc_35eb_6eed),
    ("cheats-and-ricochet", 0x5052_42ea_2853_4ffc),
    ("ground-impact", 0x1426_67c2_df3b_e929),
    ("airborne-destruction", 0x842c_0532_6413_4cd0),
];
const HYBRID: [(&str, u64); 10] = [
    ("cruise-roll-rudder", 0x3bcc_161c_b77e_4d49),
    ("burner-loop", 0xac58_9973_3290_9cc5),
    ("stall-and-spin", 0xbb84_c433_9169_de6c),
    ("devices-and-switches", 0x9190_d80a_c587_b31f),
    ("damage-and-disturbance", 0xf99e_4502_333c_f5db),
    ("cheats-and-ricochet", 0xa506_3272_8028_1a08),
    ("ground-impact", 0xc0fb_092e_b136_6da9),
    ("airborne-destruction", 0x1428_8eb9_7ce4_b276),
    ("crosswind-takeoff", 0xd2a6_6cb9_b2e3_687a),
    ("approach-and-landing", 0xa326_b981_1879_c92e),
];
const NATIVE: [(&str, u64); 2] = [
    ("cruise-roll-rudder", 0xe239_05a8_a49b_d119),
    ("burner-loop", 0x90b2_e746_355c_5263),
];

#[test]
fn flight_legacy_adapter_matches_recorded_fingerprints() {
    check(Adapter::Legacy, &LEGACY);
}

#[test]
fn flight_hybrid_adapter_airborne_maneuvers_match_recorded_fingerprints() {
    check(Adapter::Hybrid, &HYBRID[..8]);
}

#[test]
fn flight_hybrid_adapter_runway_maneuvers_match_recorded_fingerprints() {
    check(Adapter::Hybrid, &HYBRID[8..]);
}

#[test]
fn flight_native_adapter_matches_recorded_fingerprints() {
    check(Adapter::Native, &NATIVE);
}

#[derive(Clone, Copy, PartialEq)]
enum Adapter {
    Legacy,
    Hybrid,
    Native,
}

impl Adapter {
    fn label(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Hybrid => "hybrid",
            Self::Native => "native",
        }
    }
}

/// One scripted maneuver. `script` runs before every tick: it may apply what
/// a host applies between ticks (damage, turbulence, a blast) and returns the
/// pilot input for that tick.
struct Maneuver {
    name: &'static str,
    ticks: u64,
    setup: fn(&mut State),
    script: fn(u64, &mut State) -> PilotInput,
    surface: fn(f64, f64) -> Surface,
}

const MANEUVERS: [Maneuver; 10] = [
    Maneuver {
        name: "cruise-roll-rudder",
        ticks: 1_800,
        setup: |s| airborne(s, [0., 15_000., 0.], 0.3, 450.),
        script: cruise_roll_rudder,
        surface: flat,
    },
    Maneuver {
        name: "burner-loop",
        ticks: 2_400,
        setup: |s| airborne(s, [0., 15_000., 0.], 0., 600.),
        script: burner_loop,
        surface: flat,
    },
    Maneuver {
        name: "stall-and-spin",
        ticks: 2_400,
        setup: |s| {
            airborne(s, [0., 15_000., 0.], 1.2, 180.);
            s.pitch = 0.35;
            s.velocity = Basis::new(s.yaw, s.pitch, 0.).forward.map(|v| v * s.speed);
        },
        script: stall_and_spin,
        surface: flat,
    },
    Maneuver {
        name: "devices-and-switches",
        ticks: 2_100,
        setup: |s| airborne(s, [0., 5_000., 0.], 1., 350.),
        script: devices_and_switches,
        surface: flat,
    },
    Maneuver {
        name: "damage-and-disturbance",
        ticks: 1_800,
        setup: |s| airborne(s, [0., 12_000., 0.], -0.5, 450.),
        script: damage_and_disturbance,
        surface: windy,
    },
    Maneuver {
        name: "cheats-and-ricochet",
        ticks: 1_500,
        setup: cheats_setup,
        script: cheats_and_ricochet,
        surface: flat,
    },
    Maneuver {
        name: "ground-impact",
        ticks: 900,
        setup: |s| {
            airborne(s, [0., 1_500., 0.], 2., 450.);
            s.pitch = -0.3;
            s.velocity = Basis::new(s.yaw, s.pitch, 0.).forward.map(|v| v * s.speed);
        },
        script: |_, _| PilotInput {
            pitch: -0.5,
            ..PilotInput::default()
        },
        surface: flat,
    },
    Maneuver {
        name: "airborne-destruction",
        ticks: 2_400,
        setup: |s| {
            airborne(s, [0., 6_000., 0.], 0.7, 400.);
            s.systems = crate::aircraft_systems::Systems::new(2, [0.; 9]);
            s.systems.hit(9, s.throttle);
        },
        script: airborne_destruction,
        surface: flat,
    },
    Maneuver {
        name: "crosswind-takeoff",
        ticks: 2_400,
        setup: |s| {
            s.fuel = 800.;
            s.start_on_runway([0., 0., -3_500.], 0.)
                .expect("researched adapter");
        },
        script: crosswind_takeoff,
        surface: crosswind_runway,
    },
    Maneuver {
        name: "approach-and-landing",
        ticks: 3_000,
        setup: landing_setup,
        script: approach_and_landing,
        surface: fixtures::runway_surface,
    },
];

/// What the configurations of one maneuver actually did, so a later change
/// cannot quietly turn a maneuver into one that no longer covers its name.
#[derive(Clone, Copy, Default)]
struct Tally {
    configurations: u32,
    spun: u32,
    landed: u32,
    climbed_away: u32,
    bounced: u32,
    crashed: u32,
    ejected: u32,
}

impl Tally {
    fn require(&self, adapter: Adapter, maneuver: &str) {
        let all = self.configurations;
        let failure = match (adapter, maneuver) {
            (Adapter::Hybrid, "stall-and-spin") if self.spun == 0 => Some("nothing spun"),
            (_, "approach-and-landing") if self.landed < all => Some("not every aircraft landed"),
            (_, "crosswind-takeoff") if self.climbed_away < all => {
                Some("not every aircraft climbed away")
            }
            (_, "cheats-and-ricochet") if self.bounced < all || self.crashed > 0 => {
                Some("not every aircraft bounced off the ground without crashing")
            }
            (_, "ground-impact") if self.crashed < all => Some("not every aircraft crashed"),
            (_, "airborne-destruction") if self.ejected == 0 => Some("nobody ejected"),
            _ => None,
        };
        if let Some(failure) = failure {
            panic!(
                "flight/{}/{maneuver} no longer covers its name: {failure}",
                adapter.label()
            );
        }
    }
}

fn check(adapter: Adapter, recorded: &[(&str, u64)]) {
    let mut tallies = Vec::new();
    let outcomes: Vec<Outcome> = recorded
        .iter()
        .map(|(name, value)| {
            let maneuver = MANEUVERS
                .iter()
                .find(|maneuver| maneuver.name == *name)
                .expect("known maneuver");
            let tally = Cell::new(Tally::default());
            let outcome = run_twice(
                &format!("flight/{}/{name}", adapter.label()),
                *value,
                |probe| {
                    let (value, counted) = fly(adapter, maneuver, probe);
                    tally.set(counted);
                    value
                },
            );
            tallies.push((maneuver.name, tally.get()));
            outcome
        })
        .collect();
    verify(outcomes);
    for (name, tally) in tallies {
        tally.require(adapter, name);
    }
}

/// Fly one maneuver with every configuration, one fresh aircraft each, and
/// fold every tick's input and resulting state into one fingerprint.
fn fly(adapter: Adapter, maneuver: &Maneuver, probe: &Probe) -> (u64, Tally) {
    let mut fp = Fingerprint::default();
    let mut tally = Tally::default();
    let configurations = if adapter == Adapter::Native {
        vec![("Synthetic native".to_string(), fixtures::native_aircraft())]
    } else {
        fixtures::configurations()
    };
    for (index, (label, aircraft)) in configurations.iter().enumerate() {
        let mut state = State::new(aircraft, [0., 10_000., 0.]).expect("synthetic aircraft");
        match adapter {
            Adapter::Legacy => {}
            Adapter::Hybrid => state.enable_research(index as i32 + 1).unwrap(),
            Adapter::Native => state
                .enable_native(fixtures::native_tables(), index as i32 + 1)
                .unwrap(),
        }
        (maneuver.setup)(&mut state);
        fp.text(label);
        let (mut spun, mut lowest) = (false, f64::INFINITY);
        for tick in 0..maneuver.ticks {
            let input = (maneuver.script)(tick, &mut state);
            record_input(&mut fp, &input);
            state.step_surface(&input, maneuver.surface);
            record_flight(&mut fp, &state);
            let ground = (maneuver.surface)(state.position[0], state.position[2]).height;
            fp.option(state.stall_alert(ground), |fp, mode| fp.name(&mode));
            fp.bool(state.afterburner_active());
            fp.bool(state.supported_at(ground));
            spun |= state.research.as_ref().is_some_and(|r| r.spinning != 0);
            lowest = lowest.min(state.position[1] - ground);
        }
        let clearance = state.model().configuration().equipment.ground_clearance_ft;
        tally.configurations += 1;
        tally.spun += u32::from(spun);
        tally.landed += u32::from(
            state
                .research
                .as_ref()
                .is_some_and(|r| r.landings.count > 0),
        );
        tally.climbed_away += u32::from(state.position[1] > 500. && !state.crashed);
        tally.bounced += u32::from(lowest <= clearance + 2.);
        tally.crashed += u32::from(state.crashed);
        tally.ejected += u32::from(state.escape.is_some());
        probe.part(label.clone(), fp.value());
    }
    (fp.value(), tally)
}

fn flat(_: f64, _: f64) -> Surface {
    Surface::terrain(0.)
}

fn windy(_: f64, _: f64) -> Surface {
    Surface {
        wind: [30., 0., -20.],
        ..Surface::terrain(0.)
    }
}

fn crosswind_runway(x: f64, z: f64) -> Surface {
    Surface {
        wind: [22., 0., -8.],
        ..fixtures::runway_surface(x, z)
    }
}

fn airborne(s: &mut State, position: [f64; 3], heading: f64, knots: f64) {
    s.position = position;
    s.yaw = heading;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = knots * 1.68781;
    s.velocity = Basis::new(heading, 0., 0.)
        .forward
        .map(|axis| axis * s.speed);
}

fn command(commands: &[PilotCommand]) -> PilotInput {
    PilotInput {
        commands: commands.to_vec(),
        ..PilotInput::default()
    }
}

fn cruise_roll_rudder(tick: u64, _: &mut State) -> PilotInput {
    let mut input = PilotInput::default();
    match tick {
        120..360 => input.roll = 1.,
        480..720 => {
            input.roll = -0.6;
            input.pitch = 0.5;
        }
        720..900 => input.yaw = 1.,
        900..1_080 => {
            input.yaw = -1.;
            input.throttle_rate = 1.;
        }
        1_080..1_320 => input.pitch = -0.6,
        1_320 => input.commands.push(PilotCommand::Throttle(0.3)),
        1_500..1_800 => {
            input.roll = 1.;
            input.pitch = 1.;
        }
        _ => {}
    }
    input
}

fn burner_loop(tick: u64, _: &mut State) -> PilotInput {
    let mut input = PilotInput {
        pitch: 1.,
        throttle: Some(1.),
        ..PilotInput::default()
    };
    if tick == 0 {
        input.commands.push(PilotCommand::Set(Switch::Burner, true));
    }
    input
}

/// Throttle closed and nose high until the stall, pro-spin controls, then
/// opposite rudder and forward stick to recover.
fn stall_and_spin(tick: u64, _: &mut State) -> PilotInput {
    match tick {
        0..500 => PilotInput {
            pitch: 1.,
            throttle: Some(0.),
            ..PilotInput::default()
        },
        500..1_500 => PilotInput {
            pitch: 1.,
            yaw: 1.,
            roll: 0.4,
            ..PilotInput::default()
        },
        1_500..2_000 => PilotInput {
            pitch: -1.,
            yaw: -1.,
            ..PilotInput::default()
        },
        _ => PilotInput {
            throttle: Some(1.),
            ..PilotInput::default()
        },
    }
}

fn devices_and_switches(tick: u64, s: &mut State) -> PilotInput {
    use PilotCommand::{AdjustThrottle, Set, Throttle, Toggle};
    match tick {
        1_400 => s.bay_auto_open = true,
        1_600 => s.bay_auto_open = false,
        _ => {}
    }
    let mut input = match tick {
        60 => command(&[Set(Switch::Gear, true)]),
        120 => command(&[Set(Switch::Flaps, true)]),
        180 => command(&[Toggle(Switch::Airbrake)]),
        400 => command(&[Toggle(Switch::Hook)]),
        450 => command(&[Toggle(Switch::Bay)]),
        600 => command(&[Set(Switch::Gear, false)]),
        660 => command(&[Set(Switch::Flaps, false)]),
        700 => command(&[Toggle(Switch::Airbrake)]),
        800 => command(&[Set(Switch::Engine, false)]),
        1_000 => command(&[Set(Switch::Engine, true)]),
        1_100 => command(&[Toggle(Switch::Radar)]),
        1_150 => command(&[Toggle(Switch::Jammer)]),
        1_200 => command(&[AdjustThrottle(0.2)]),
        1_250 => command(&[Toggle(Switch::Autopilot)]),
        1_600 => command(&[Toggle(Switch::Autopilot)]),
        1_700 => command(&[Set(Switch::Burner, true), Throttle(1.)]),
        1_900 => command(&[Set(Switch::Burner, false)]),
        _ => PilotInput::default(),
    };
    if (1_300..1_500).contains(&tick) {
        input.pitch = 0.3;
    }
    input
}

fn damage_and_disturbance(tick: u64, s: &mut State) -> PilotInput {
    let t = tick as f64 / 120.;
    match tick {
        100 => s.systems.hit(19, s.throttle),
        200 => s.damage_regions[3] = 0.5,
        250 => s.damage_regions[5] = 0.3,
        300 => {
            let blast = [s.position[0] + 30., s.position[1] - 40., s.position[2]];
            s.jolt_from(blast, 1.);
        }
        600 => s.systems.hit(29, s.throttle),
        700 => s.systems.hit(16, s.throttle),
        800 => s.systems.hit(14, s.throttle),
        1_000 => s.systems.hit(5, s.throttle),
        1_200 => s.systems.hit(25, s.throttle),
        1_400 => s.damage_fraction = 0.6,
        _ => {}
    }
    if (400..900).contains(&tick) {
        s.apply_turbulence(Disturbance {
            vertical_fps: 6. * (t * 1.3).sin(),
            yaw: 0.05 * (t * 0.7).sin(),
            pitch: 0.08 * (t * 1.1).sin(),
            roll: 0.1 * (t * 0.9).cos(),
            shake: true,
        });
    }
    let mut input = PilotInput {
        pitch: 0.4 * (t * 0.8).sin(),
        roll: 0.5 * (t * 0.6).cos(),
        yaw: 0.2 * (t * 0.3).sin(),
        ..PilotInput::default()
    };
    if (650..700).contains(&tick) {
        input.throttle_rate = 1.;
    }
    if tick == 710 {
        input.commands.push(PilotCommand::Set(Switch::Gear, true));
    }
    input
}

fn cheats_setup(s: &mut State) {
    airborne(s, [0., 600., 0.], 0., 450.);
    s.pitch = -0.3;
    s.velocity = Basis::new(s.yaw, s.pitch, 0.).forward.map(|v| v * s.speed);
    s.set_payload(2_000.)
        .expect("payload inside the mass limits");
    s.cheats.extra_g = true;
    s.cheats.unlimited_fuel = true;
    s.cheats.ignore_weapon_weights = true;
    s.cheats.no_crashes = true;
}

/// Dive into the ground (a bounce, not a crash), pull nine G, dive and bounce
/// again, then hold pro-spin controls with the throttle closed and spins
/// turned off.
fn cheats_and_ricochet(tick: u64, s: &mut State) -> PilotInput {
    if tick == 1_300 {
        s.cheats.no_spins = true;
    }
    match tick {
        0..360 => PilotInput {
            pitch: -0.5,
            throttle: Some(1.),
            ..PilotInput::default()
        },
        360..600 => PilotInput {
            pitch: 1.,
            ..PilotInput::default()
        },
        600..900 => PilotInput {
            pitch: -1.,
            roll: 0.3,
            ..PilotInput::default()
        },
        900..1_300 => PilotInput {
            pitch: 1.,
            yaw: 1.,
            roll: 0.4,
            throttle: Some(0.),
            ..PilotInput::default()
        },
        _ => PilotInput::default(),
    }
}

fn airborne_destruction(tick: u64, s: &mut State) -> PilotInput {
    if tick == 30 {
        s.crashed = true;
    }
    match tick {
        60 | 100 => command(&[PilotCommand::Eject]),
        _ => PilotInput {
            pitch: 1.,
            roll: 1.,
            ..PilotInput::default()
        },
    }
}

/// Brakes held, then full power with the burner, rotation at 230 ft/s, and
/// the gear and flaps raised once clear of the runway.
fn crosswind_takeoff(tick: u64, s: &mut State) -> PilotInput {
    let on_ground = s.research.as_ref().is_some_and(|r| r.on_ground);
    let mut input = PilotInput {
        throttle: Some(if tick < 240 { 0. } else { 1. }),
        roll: if on_ground {
            0.
        } else {
            (-2. * s.bank).clamp(-1., 1.)
        },
        pitch: if tick >= 240 && s.speed > 230. {
            0.45
        } else {
            0.
        },
        ..PilotInput::default()
    };
    if tick == 240 {
        input.commands.extend([
            PilotCommand::Set(Switch::Airbrake, false),
            PilotCommand::Set(Switch::Burner, true),
        ]);
    }
    if !on_ground && s.position[1] > 150. {
        input.commands.push(PilotCommand::Set(Switch::Gear, false));
    }
    if s.position[1] > 400. {
        input.commands.push(PilotCommand::Set(Switch::Flaps, false));
    }
    input
}

fn landing_setup(s: &mut State) {
    airborne(s, [0., 330., -8_000.], 0., 160.);
    s.velocity[1] = -14.;
    s.gear = 1.;
    s.gear_down = true;
    s.flaps = 1.;
    s.flaps_down = true;
    s.throttle = 0.45;
}

/// A scripted three-degree approach with centreline tracking, a flare and a
/// braked rollout.
fn approach_and_landing(_tick: u64, s: &mut State) -> PilotInput {
    if s.research.as_ref().is_some_and(|r| r.on_ground) {
        return PilotInput {
            pitch: -0.2,
            throttle: Some(0.),
            commands: vec![PilotCommand::Set(Switch::Airbrake, true)],
            ..PilotInput::default()
        };
    }
    let glide = 3_f64.to_radians().tan();
    let height = s.position[1] - 8.;
    let wanted_height = ((-3_200. - s.position[2]) * glide).max(0.);
    let wanted_sink = if height < 25. { -4. } else { -s.speed * glide };
    PilotInput {
        pitch: (0.03 * (wanted_height - height) + 0.02 * (wanted_sink - s.velocity[1]))
            .clamp(-0.5, 0.5),
        roll: (-0.002 * s.position[0] - 1.5 * s.bank).clamp(-0.5, 0.5),
        throttle: Some((0.45 + 0.01 * (250. - s.speed)).clamp(0., 1.)),
        ..PilotInput::default()
    }
}
