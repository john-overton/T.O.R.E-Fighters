//! Acceptance tests T1 to T6 of the VTOL overhaul design (section 10) for
//! the V-22, on a synthetic record carrying its PT flight numbers
//! ([`pt_v22`]), at sea level on a standard day with no wind unless a test
//! says otherwise. The scripted pilots are part of the tests, not of the
//! game: attitude, heading and height holds that fly the aircraft through
//! its stick, pedals, lever and nacelle demand as a player would.

use super::*;
use crate::flight::{PilotCommand, PilotInput, Switch};
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_input::StabilityLevel;

const FPM: f64 = 60.;
const LEVELS: [StabilityLevel; 3] = [
    StabilityLevel::Off,
    StabilityLevel::Damper,
    StabilityLevel::Attitude,
];

/// Every stability level, each with the Easy flight physics cheat off and
/// on: corridor protection and the rotor strike never depend on either.
fn every_setting() -> impl Iterator<Item = (StabilityLevel, bool)> {
    LEVELS
        .into_iter()
        .flat_map(|level| [(level, false), (level, true)])
}

/// A synthetic record under the V-22's identity carrying its PT's flight
/// numbers (design 8.2) as plain constants: 18,298 lb empty, 2,000 lb fuel,
/// 23,810 lb maximum, 26,280 lbf, 36 to 130 kt to 7,000 ft (the AH-64's
/// envelope; the variety fit scales it to 275 kt and 25,000 ft).
pub(crate) fn pt_v22() -> Aircraft {
    let mut a = crate::models::variety::tests::synthetic(AircraftId::V22);
    for (key, value) in [
        ("weight", 18_298),
        ("internalFuel", 2_000),
        ("maxTakeoffWeight", 23_810),
        ("thrust", 26_280),
    ] {
        a.fields.get_mut(key).unwrap().value = value.to_string();
    }
    let [slow, fast] = [36., 130.].map(|kt: f64| kt * KT);
    for e in &mut a.envelopes {
        e.points = vec![
            [slow, 0.],
            [slow + 10., 7_000.],
            [fast - 20., 7_000.],
            [fast, 0.],
        ];
    }
    a
}

/// A hybrid V-22 at `height`, heading north, gear up, at `level`, not yet
/// trimmed.
fn untrimmed(height: f64, level: StabilityLevel) -> State {
    let mut s = State::new(&pt_v22(), [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.velocity = [0.; 3];
    s.speed = 0.;
    s.gear = 0.;
    s.gear_down = false;
    s.lift_controls.aids.stability = level;
    s
}

/// A hybrid V-22 trimmed level at `airspeed_fps` and `height` with the
/// nacelles at `nacelle_degrees`, heading north, gear up, at `level`.
pub(crate) fn trimmed(
    height: f64,
    airspeed_fps: f64,
    nacelle_degrees: f64,
    level: StabilityLevel,
) -> State {
    let mut s = untrimmed(height, level);
    assert!(
        s.trim_tiltrotor(airspeed_fps, nacelle_degrees),
        "trims at {airspeed_fps} ft/s, {nacelle_degrees} degrees"
    );
    s
}

fn model(s: &State) -> Tiltrotor {
    Tiltrotor::new(
        &s.model().powered_lift().unwrap(),
        s.model().configuration(),
    )
    .unwrap()
}

fn fly(s: &mut State, ticks: usize, mut pilot: impl FnMut(&State) -> PilotInput) {
    for _ in 0..ticks {
        let input = pilot(s);
        s.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
    }
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

fn indicated(s: &State) -> f64 {
    kcas(s.speed, rotor::air_density(s.position[1]))
}

/// What the scripted pilot holds. Angles in radians, climb in ft/s.
#[derive(Clone, Copy, Debug, Default)]
struct Hold {
    pitch: Option<f64>,
    bank: Option<f64>,
    heading: Option<f64>,
    climb: Option<f64>,
    lever: Option<f64>,
    /// The nacelle demand's rate (the conversion keys) and position.
    conversion_rate: f64,
    conversion: Option<f64>,
}

/// Stick, pedals, lever and nacelle demand that fly toward `hold` from `s`.
fn steer(s: &State, hold: Hold) -> PilotInput {
    let [p, q, r] = s.lift_controls.body_rates;
    let pitch = hold
        .pitch
        .map_or(0., |target| 2.5 * wrap(target - s.pitch) - 1.2 * q);
    let roll = hold
        .bank
        .map_or(0., |target| 2.5 * wrap(target - s.bank) - 0.4 * p);
    let yaw = hold
        .heading
        .map_or(0., |target| 2. * wrap(target - s.yaw) - 1.5 * r);
    let collective = hold.lever.or_else(|| {
        hold.climb.map(|climb| {
            (s.lift_controls.collective + DT * 0.05 * (climb - s.vertical_speed)).clamp(0., 1.)
        })
    });
    PilotInput {
        pitch: pitch.clamp(-1., 1.),
        roll: roll.clamp(-1., 1.),
        yaw: yaw.clamp(-1., 1.),
        collective,
        conversion_rate: hold.conversion_rate,
        conversion: hold.conversion,
        ..Default::default()
    }
}

/// The pitch attitude that holds `height`, about `trim`, rad.
fn height_hold(s: &State, height: f64, trim: f64) -> f64 {
    (trim + 0.003 * (height - s.position[1]) - 0.004 * s.vertical_speed).clamp(-0.2, 0.25)
}

/// The instant the state is at, as the step builds it (no damage, no
/// surface).
fn instant_of(s: &State) -> Instant {
    let model = model(s);
    let c = s.model().configuration();
    Instant {
        basis: Basis::new(s.yaw, s.pitch, s.bank),
        rates: s.lift_controls.body_rates,
        air_velocity: s.velocity,
        altitude_ft: s.position[1],
        density: rotor::air_density(s.position[1]),
        rotor_speed: s.lift_controls.drive.rotor_speed,
        rotor_speed_reference: s.lift_controls.drive.rotor_speed_reference,
        rotors: s.lift_controls.rotors,
        nacelle: s.nacelle_degrees().to_radians(),
        lever: s.lift_controls.collective_actual,
        available_power: model
            .drive
            .available_power(rotor::air_density(s.position[1]), 1., 1.),
        controls: [0.; 3],
        pilot: [0.; 3],
        rudder: 0.,
        flaps: 0.,
        weight: c.mass.empty_lbs + s.fuel + s.carried_lbs(),
        hub_heights_agl_ft: [None; 2],
        seconds: 0.,
        hazards: Hazards::ALL,
        drag_factor: 1.,
        lift_scale: 1.,
        g_limits: [-1., 3.],
        rudder_slip: model.rudder_slip,
        side_force: model.side_force,
    }
}

// ---------------------------------------------------------------------
// The corridor and its protection, as functions.

fn parameters() -> TiltrotorParameters {
    model(&untrimmed(1_000., StabilityLevel::Damper)).tilt
}

#[test]
fn the_corridor_follows_its_published_points_and_fitted_edges() {
    let t = parameters();
    let at = |degrees| corridor_limits(t.corridor, degrees);
    // Hover band: no minimum, 100 KCAS maximum; then the table's points
    // with straight lines between.
    assert_eq!(at(97.5), [0., 100.]);
    assert_eq!(at(85.), [0., 100.]);
    assert_eq!(at(80.), [30., 130.]);
    assert_eq!(at(70.), [45., 145.]);
    assert_eq!(at(30.), [90., 200.]);
    assert_eq!(at(0.), [110., 280.]);
    // The published mid-points (80 KCAS at 80 degrees, 110 at 60, 130 at
    // 30) sit inside, about 50 kt from each edge.
    for (degrees, mid) in [(80., 80.), (60., 110.), (30., 130.)] {
        let [low, high] = at(degrees);
        assert!(low < mid && mid < high, "{degrees}");
        assert!((mid - low - 50.).abs() <= 10. && (high - mid - 50.).abs() <= 25.);
    }
    // The inverse: how far aft and forward the nacelles may sit at a speed.
    let range = t.nacelle_range_degrees;
    assert_eq!(aft_limit_degrees(t.corridor, range, 90.), range);
    assert!((aft_limit_degrees(t.corridor, range, 180.) - 45.).abs() < 1e-9);
    assert!((aft_limit_degrees(t.corridor, range, 140.) - (80. - 20. / 3.)).abs() < 1e-9);
    assert_eq!(aft_limit_degrees(t.corridor, range, 300.), 0.);
    assert_eq!(forward_limit_degrees(t.corridor, range, 0.), 85.);
    assert!((forward_limit_degrees(t.corridor, range, 40.) - (80. - 20. / 3.)).abs() < 1e-9);
    assert_eq!(forward_limit_degrees(t.corridor, range, 120.), 0.);
    for kcas in [0., 25., 60., 99., 150., 210., 279.] {
        let [forward, aft] = [
            forward_limit_degrees(t.corridor, range, kcas),
            aft_limit_degrees(t.corridor, range, kcas),
        ];
        assert!(forward <= aft, "{kcas}: {forward} {aft}");
        let [low, _] = at(forward);
        let [_, high] = at(aft);
        assert!(low <= kcas + 1e-9 && kcas <= high + 1e-9, "{kcas}");
    }
}

#[test]
fn protection_moves_only_the_nacelles_and_keeps_the_pilots_demand() {
    let t = parameters();
    let step = |actual, demand, kcas| protect_nacelles(&t, actual, demand, kcas, DT);
    let rate = t.nacelle_rate_degrees_per_second * DT;
    // Inside the corridor, well away from its edges, the pilot's demand
    // stands at the full 8 degrees per second.
    let free = step(60., 30., 110.);
    assert!((free.degrees - (60. - rate)).abs() < 1e-12 && free.hold_degrees.is_none());
    // Above the aft lock nothing moves aft, whatever the edge allows.
    let locked = step(10., 87., 210.);
    assert_eq!(locked.degrees, 10.);
    assert_eq!(locked.hold_degrees, Some(10.));
    // Aft toward the upper edge: slowed near it, stopped at it.
    let near = step(43., 87., 180.);
    assert!(near.degrees > 43. && near.degrees - 43. < rate);
    let at_edge = step(45., 87., 180.);
    assert_eq!(at_edge.degrees, 45.);
    // Past the upper edge: forward at the full rate, even against a pilot
    // asking for more aft.
    let over = step(80., 97.5, 140.);
    assert!((over.degrees - (80. - rate)).abs() < 1e-12);
    assert!(over.hold_degrees.unwrap() < aft_limit_degrees(t.corridor, 97.5, 140.));
    // Forward toward the lower edge: slowed near it, stopped at it.
    let edge = forward_limit_degrees(t.corridor, 97.5, 40.);
    let approaching = step(edge + 2., 0., 40.);
    assert!(approaching.degrees < edge + 2. && edge + 2. - approaching.degrees < rate);
    assert!(approaching.hold_degrees.is_some());
    assert_eq!(step(edge, 0., 40.).degrees, edge);
    // Below the lower edge the nacelles are not raised for the pilot, but
    // the pilot may raise them.
    let slow = step(20., 0., 60.);
    assert_eq!(slow.degrees, 20.);
    assert!(step(20., 90., 60.).degrees > 20.);
    // Without hydraulics nothing moves.
    assert_eq!(protect_nacelles(&t, 60., 30., 110., 0.).degrees, 60.);
}

// ---------------------------------------------------------------------
// T1: the hover set (H1, H4, H6, H9b with the V-22 targets).

/// H1: a trimmed hover at the helicopter preset holds still hands-off.
#[test]
fn t1_a_trimmed_hover_holds_still_hands_off() {
    let s = trimmed(1_000., 0., 87., StabilityLevel::Damper);
    let m = model(&s);
    let lever = s.lift_controls.collective;
    assert!((0.6..0.85).contains(&lever), "hover lever {lever}");
    // Hover power at the PT gross weight leaves a margin.
    let available = m.drive.available_power(rotor::air_density(1_000.), 1., 1.);
    assert!(s.lift_controls.drive.engine_output[0] < 0.7 * available);
    let mut hovering = s.clone();
    fly(&mut hovering, 1200, |_| PilotInput::default());
    let drift = hovering.position[0].hypot(hovering.position[2]);
    assert!(drift < 1., "drift {drift}");
    assert!(
        (hovering.position[1] - 1_000.).abs() < 1.,
        "{}",
        hovering.position[1]
    );
    assert!(!hovering.crashed);
}

fn hover_rate(level: StabilityLevel, axis: usize) -> f64 {
    let mut s = trimmed(3_000., 0., 87., level);
    fly(&mut s, 240, |_| {
        let mut stick = [0.; 3];
        stick[axis] = 1.;
        PilotInput {
            pitch: stick[0],
            roll: stick[1],
            yaw: stick[2],
            ..Default::default()
        }
    });
    s.lift_controls.body_rates[[1, 0, 2][axis]].to_degrees()
}

/// H4: full stick for 2 s from a hover. At Damper the rates are within 15
/// percent of the 8.2 targets (roll 45, pitch 30, yaw 30 deg/s); at Off
/// they reach them or more. Roll comes from differential collective.
#[test]
fn t1_full_stick_reaches_the_hover_rate_targets() {
    let targets = [30., 45., 30.];
    for (axis, target) in targets.into_iter().enumerate() {
        let damped = hover_rate(StabilityLevel::Damper, axis);
        let free = hover_rate(StabilityLevel::Off, axis);
        assert!(
            (damped / target - 1.).abs() <= 0.15,
            "axis {axis}: {damped} at Damper"
        );
        assert!(free >= target, "axis {axis}: {free} at Off");
    }
    // Right stick: more thrust on the left rotor, no lateral disk tilt.
    let s = trimmed(3_000., 0., 87., StabilityLevel::Off);
    let m = model(&s);
    let mut i = instant_of(&s);
    let level = m.loads(&i);
    i.controls = [0., 1., 0.];
    let rolled = m.loads(&i);
    let [left, right] = rolled.rotors.map(|r| r.thrust_lbf);
    assert!(
        left > right + 0.3 * level.rotors[0].thrust_lbf,
        "{left} {right}"
    );
    assert!(rolled.rotors.iter().all(|r| r.tilt_target[1].abs() < 1e-3));
    assert!(rolled.moments.applied[0] > 0.);
    // Right pedal: the left disk leans forward, the right one aft.
    i.controls = [0., 0., 1.];
    for _ in 0..60 {
        let loads = m.loads(&i);
        for k in 0..2 {
            rotor::relax(&mut i.rotors[k], &loads.rotors[k], DT);
        }
    }
    let yawed = m.loads(&i);
    assert!(yawed.rotors[0].tilt_target[0] > yawed.rotors[1].tilt_target[0]);
    assert!(yawed.moments.applied[2] > 0.);
}

/// H6: a 10 percent lever step from a hover sets a climb rate that settles
/// (time constant 3 to 6 s, as the helicopters' rotor heave damping gives),
/// and full lever climbs at 1,500 to 3,000 ft/min.
#[test]
fn t1_the_lever_sets_a_climb_rate_not_an_acceleration() {
    let hold = |lever| Hold {
        pitch: Some(0.05),
        bank: Some(0.),
        heading: Some(0.),
        lever: Some(lever),
        ..Default::default()
    };
    let mut s = trimmed(1_000., 0., 87., StabilityLevel::Damper);
    let lever = s.lift_controls.collective + 0.1;
    let mut climb = Vec::new();
    for _ in 0..20 {
        fly(&mut s, 120, |s| steer(s, hold(lever)));
        climb.push(s.vertical_speed);
    }
    let settled = climb[19];
    assert!(settled > 5., "{settled}");
    assert!((climb[15] - settled).abs() < 0.03 * settled, "{climb:?}");
    let rise = climb.iter().position(|v| *v > 0.63 * settled).unwrap() + 1;
    assert!((2..=6).contains(&rise), "{rise} s: {climb:?}");
    let mut full = trimmed(1_000., 0., 87., StabilityLevel::Damper);
    fly(&mut full, 120 * 15, |s| steer(s, hold(1.)));
    let rate = full.vertical_speed * FPM;
    assert!((1_500. ..=3_000.).contains(&rate), "{rate} ft/min");
}

/// H9b: the engines cut in a hover with the lever held. Rotor speed falls
/// below 80 percent in 1.5 to 2 s with the LOW ROTOR warning; once below 70
/// percent, lowering the lever no longer saves it.
#[test]
fn t1_an_engine_cut_in_the_hover_droops_the_rotors_past_recovery() {
    let mut s = trimmed(3_000., 0., 87., StabilityLevel::Off);
    let lever = s.lift_controls.collective;
    s.command(PilotCommand::Set(Switch::Engine, false));
    let mut below = None;
    for tick in 0..120 * 5 {
        fly(&mut s, 1, |_| PilotInput {
            collective: Some(lever),
            ..Default::default()
        });
        if below.is_none() && s.lift_controls.drive.rotor_speed < drive::LOW_ROTOR {
            below = Some((tick + 1) as f64 * DT);
        }
        if s.lift_controls.drive.rotor_speed < 0.69 {
            break;
        }
    }
    let seconds = below.expect("rotor speed fell");
    assert!((1.5..=2.).contains(&seconds), "{seconds} s");
    assert!(s.lift_controls.warnings.low_rotor > 0);
    fly(&mut s, 120 * 10, |_| PilotInput {
        collective: Some(0.),
        ..Default::default()
    });
    assert!(s.lift_controls.drive.rotor_speed < 0.7);
}

// ---------------------------------------------------------------------
// T2: the conversion.

/// What a held conversion did.
struct Conversion {
    downstops: Option<f64>,
    fast: Option<f64>,
    lowest: f64,
    highest: f64,
    outside_kt: f64,
    lowest_rotor_speed: f64,
}

/// The conversion pilot from a 1,000 ft hover: the conversion keys held
/// toward airplane mode, the lever at `lever`, the nose down while the
/// nacelles are high to start the acceleration, then the height held by
/// pitch; wings level, heading north. Records the worst excursions.
fn convert(level: StabilityLevel, lever: f64, seconds: usize) -> (State, Conversion) {
    let mut s = trimmed(1_000., 0., 87., level);
    let mut c = Conversion {
        downstops: None,
        fast: None,
        lowest: f64::MAX,
        highest: f64::MIN,
        outside_kt: 0.,
        lowest_rotor_speed: f64::MAX,
    };
    // The pilot trims out a steady pitch error, as a pilot would.
    let mut trim = 0.;
    for tick in 0..120 * seconds {
        let time = (tick + 1) as f64 * DT;
        fly(&mut s, 1, |s| {
            let early = rotor::smoothstep((s.nacelle_degrees() - 60.) / 20.);
            let pitch = height_hold(s, 1_000., 0.03) * (1. - early) - 0.15 * early;
            trim = (trim + DT * 1.5 * wrap(pitch - s.pitch)).clamp(-1., 1.);
            let mut input = steer(
                s,
                Hold {
                    pitch: Some(pitch),
                    bank: Some(0.),
                    heading: Some(0.),
                    lever: Some(lever),
                    conversion_rate: -1.,
                    ..Default::default()
                },
            );
            input.pitch = (input.pitch + trim).clamp(-1., 1.);
            input
        });
        let corridor = s.conversion_corridor().unwrap();
        let [low, high] = corridor.limits_kcas;
        c.outside_kt = c
            .outside_kt
            .max(low - corridor.kcas)
            .max(corridor.kcas - high);
        c.lowest = c.lowest.min(s.position[1]);
        c.highest = c.highest.max(s.position[1]);
        c.lowest_rotor_speed = c
            .lowest_rotor_speed
            .min(s.lift_controls.drive.rotor_speed / s.lift_controls.drive.rotor_speed_reference);
        if c.downstops.is_none() && corridor.nacelle_degrees < DOWNSTOP_DEGREES {
            c.downstops = Some(time);
        }
        if c.fast.is_none() && corridor.kcas >= 200. {
            c.fast = Some(time);
        }
    }
    (s, c)
}

/// T2: the conversion keys held from a hover with the lever at climb
/// power: on the downstops and at 200 KCAS within 30 s, the height within
/// 200 ft, never outside the corridor, the rotors at speed throughout.
#[test]
fn t2_a_held_conversion_reaches_airplane_mode_inside_the_corridor() {
    for level in LEVELS {
        let (s, c) = convert(level, 0.85, 30);
        let downstops = c.downstops.expect("on the downstops");
        let fast = c.fast.expect("200 KCAS");
        assert!(
            downstops < fast && fast <= 30.,
            "{level:?}: {downstops} {fast}"
        );
        assert!(
            c.highest - 1_000. < 200. && 1_000. - c.lowest < 200.,
            "{level:?}: {} to {}",
            c.lowest,
            c.highest
        );
        assert!(c.outside_kt < 1., "{level:?}: {} kt outside", c.outside_kt);
        assert!(
            c.lowest_rotor_speed > 0.85,
            "{level:?}: {}",
            c.lowest_rotor_speed
        );
        assert!(!s.crashed && s.stall_alert(0.).is_none(), "{level:?}");
    }
}

// ---------------------------------------------------------------------
// T3: corridor protection, at every stability level.

/// True airspeed, ft/s, of `kcas` at `height`.
fn true_airspeed(kcas: f64, height: f64) -> f64 {
    kcas * KT / (rotor::air_density(height) / rotor::sea_level_density()).sqrt()
}

/// T3 (aft): commanding the helicopter preset at 180 KCAS in airplane
/// mode, the nacelles stop at the upper edge (45 degrees at 180 KCAS),
/// slowing near it; at 210 KCAS they do not move aft at all.
#[test]
fn t3_protection_stops_aft_motion_at_the_upper_edge() {
    let corridor = parameters().corridor;
    for (level, easy) in every_setting() {
        for (kt, limit) in [(180., 45.), (210., 0.)] {
            let mut s = trimmed(3_000., true_airspeed(kt, 3_000.), 0., level);
            s.cheats.easy_physics = easy;
            let lever = s.lift_controls.collective;
            let mut fastest: f64 = 0.;
            let mut previous = 0.;
            let mut protecting = false;
            for _ in 0..120 * 10 {
                fly(&mut s, 1, |s| {
                    steer(
                        s,
                        Hold {
                            pitch: Some(height_hold(s, 3_000., 0.02)),
                            bank: Some(0.),
                            heading: Some(0.),
                            lever: Some(lever),
                            conversion: Some(87. / 97.5),
                            ..Default::default()
                        },
                    )
                });
                let now = s.nacelle_degrees();
                fastest = fastest.max((now - previous) / DT);
                previous = now;
                let aft = aft_limit_degrees(corridor, 97.5, indicated(&s));
                assert!(now <= aft + 0.5, "{level:?} {kt}: {now} past {aft}");
                protecting |= s.lift_controls.corridor_hold.is_some();
            }
            assert!(protecting && s.conversion_corridor().unwrap().protecting);
            // Held at the edge for the speed it has now (the aircraft gains
            // a little as the thrust tilts up); above the aft lock, never
            // off the downstops.
            let edge = if indicated(&s) > 200. {
                0.
            } else {
                aft_limit_degrees(corridor, 97.5, indicated(&s))
            };
            assert!(
                (s.nacelle_degrees() - edge).abs() < 1. && s.nacelle_degrees() <= limit + 1.,
                "{level:?} {kt}: {} for {edge}",
                s.nacelle_degrees()
            );
            assert!(fastest <= 8. + 1e-9, "{fastest}");
            assert_eq!(s.lift_controls.conversion, 87. / 97.5, "the demand is kept");
        }
    }
}

/// T3 (fast): at 80 degrees and accelerating to 140 KCAS (past the 130
/// KCAS edge), the nacelles are driven forward within 1 s with CONV shown,
/// and the aircraft is back inside within 5 s; the pilot's demand stays.
#[test]
fn t3_protection_drives_the_nacelles_forward_off_the_upper_edge() {
    for (level, easy) in every_setting() {
        let mut s = trimmed(3_000., true_airspeed(100., 3_000.), 80., level);
        s.cheats.easy_physics = easy;
        let lever = s.lift_controls.collective;
        // Accelerate: the airspeed jumps to 140 KCAS, as a dive would.
        let tas = true_airspeed(140., 3_000.);
        s.velocity = s.velocity.map(|v| v * tas / s.speed);
        s.speed = tas;
        let start = s.nacelle_degrees();
        let mut moved = None;
        let mut inside = None;
        for tick in 0..120 * 6 {
            fly(&mut s, 1, |s| {
                steer(
                    s,
                    Hold {
                        pitch: Some(height_hold(s, 3_000., 0.)),
                        bank: Some(0.),
                        heading: Some(0.),
                        lever: Some(lever),
                        ..Default::default()
                    },
                )
            });
            let time = (tick + 1) as f64 * DT;
            let c = s.conversion_corridor().unwrap();
            if moved.is_none() && s.nacelle_degrees() < start - 0.5 {
                assert!(c.protecting, "{level:?} CONV");
                moved = Some(time);
            }
            if inside.is_none() && c.kcas <= c.limits_kcas[1] {
                inside = Some(time);
            }
        }
        assert!(moved.unwrap() <= 1., "{level:?} {moved:?}");
        assert!(inside.unwrap() <= 5., "{level:?} {inside:?}");
        assert!((s.lift_controls.conversion * 97.5 - 80.).abs() < 1e-9);
        assert!(!s.crashed);
    }
}

/// T3 (slow): commanding 0 degrees from 87 at 40 KCAS, the nacelles stop
/// at the lower edge and follow it as the speed rises.
#[test]
fn t3_protection_holds_the_nacelles_at_the_lower_edge() {
    let corridor = parameters().corridor;
    for (level, easy) in every_setting() {
        let mut s = trimmed(3_000., true_airspeed(40., 3_000.), 87., level);
        s.cheats.easy_physics = easy;
        let lever = s.lift_controls.collective;
        s.command(PilotCommand::NeutralVector);
        let mut held = 0.;
        for tick in 0..120 * 15 {
            fly(&mut s, 1, |s| {
                // Hold the attitude for 5 s, then nose down to accelerate.
                let pitch = if tick < 600 { 0.02 } else { -0.1 };
                steer(
                    s,
                    Hold {
                        pitch: Some(pitch),
                        bank: Some(0.),
                        heading: Some(0.),
                        lever: Some(lever),
                        ..Default::default()
                    },
                )
            });
            let edge = forward_limit_degrees(corridor, 97.5, indicated(&s));
            assert!(
                s.nacelle_degrees() >= edge - 0.5,
                "{level:?}: {} past {edge}",
                s.nacelle_degrees()
            );
            if tick == 600 {
                assert!(
                    (s.nacelle_degrees() - edge).abs() < 6.,
                    "{level:?} held at {} for {edge}",
                    s.nacelle_degrees()
                );
                held = s.nacelle_degrees();
            }
        }
        assert!(
            s.nacelle_degrees() < held - 10.,
            "{level:?} followed only to {}",
            s.nacelle_degrees()
        );
        assert_eq!(s.lift_controls.conversion, 0.);
    }
}

// ---------------------------------------------------------------------
// T4: airplane mode.

/// The level top speed at sea level, gross weight, all the power, kt.
fn top_speed() -> f64 {
    let s = untrimmed(0., StabilityLevel::Damper);
    (200..330)
        .map(f64::from)
        .take_while(|kt| s.clone().trim_tiltrotor(kt * KT, 0.))
        .last()
        .unwrap_or(0.)
}

/// T4: top speed 260 to 275 kt; full-stick roll within 15 percent of the
/// fitted 45 deg/s; rotor speed 84 percent on the downstops and 100
/// percent within 4 s of leaving them.
#[test]
fn t4_airplane_mode_speed_roll_and_rotor_speed() {
    let top = top_speed();
    assert!((260. ..=275.).contains(&top), "top speed {top} kt");
    let mut s = trimmed(5_000., 250. * KT, 0., StabilityLevel::Damper);
    let lever = s.lift_controls.collective;
    let mut fastest: f64 = 0.;
    fly(&mut s, 360, |s| {
        fastest = fastest.max(s.lift_controls.body_rates[0]);
        PilotInput {
            roll: 1.,
            collective: Some(lever),
            ..Default::default()
        }
    });
    let roll = fastest.to_degrees();
    assert!((roll / 45. - 1.).abs() <= 0.15, "roll {roll} deg/s");
    // Rotor speed on the downstops, then out of them.
    let mut s = trimmed(
        3_000.,
        true_airspeed(170., 3_000.),
        0.,
        StabilityLevel::Damper,
    );
    let lever = s.lift_controls.collective;
    let hold = |s: &State, conversion| Hold {
        pitch: Some(height_hold(s, 3_000., 0.03)),
        bank: Some(0.),
        heading: Some(0.),
        lever: Some(lever),
        conversion: Some(conversion),
        ..Default::default()
    };
    fly(&mut s, 600, |s| steer(s, hold(s, 0.)));
    assert!((s.lift_controls.drive.rotor_speed - 0.84).abs() < 0.005);
    let mut left = None;
    let mut recovered = None;
    for tick in 0..120 * 8 {
        fly(&mut s, 1, |s| steer(s, hold(s, 20. / 97.5)));
        let time = (tick + 1) as f64 * DT;
        if left.is_none() && s.nacelle_degrees() >= DOWNSTOP_DEGREES {
            left = Some(time);
        }
        if recovered.is_none() && s.lift_controls.drive.rotor_speed >= 0.99 {
            recovered = Some(time);
        }
    }
    let delay = recovered.unwrap() - left.unwrap();
    assert!(delay <= 4., "{delay} s");
    assert!(!s.crashed);
}

/// The wing stalls near the published 110 kt at the PT gross weight in
/// airplane mode, and the stall warning sounds below the corridor's lower
/// edge with the nacelles under 35 degrees.
#[test]
fn the_wing_stalls_near_110_kt_and_warns_below_the_lower_edge() {
    let s = untrimmed(0., StabilityLevel::Damper);
    // Wingborne: the nose no more than 10 degrees up (lower, the proprotors
    // hold the aircraft up on their thrust, as a powerful propeller does).
    let slowest = (80..200)
        .map(f64::from)
        .find(|kt| {
            let mut s = s.clone();
            s.trim_tiltrotor(kt * KT, 0.) && s.pitch < 10_f64.to_radians()
        })
        .unwrap();
    assert!((105. ..=120.).contains(&slowest), "{slowest} kt");
    // Decelerate with the power off, holding height: the warning comes
    // below 110 KCAS on the downstops.
    let mut slowing = trimmed(
        3_000.,
        true_airspeed(150., 3_000.),
        0.,
        StabilityLevel::Damper,
    );
    let mut warned = None;
    for _ in 0..120 * 30 {
        fly(&mut slowing, 1, |s| {
            steer(
                s,
                Hold {
                    pitch: Some(height_hold(s, 3_000., 0.05)),
                    bank: Some(0.),
                    heading: Some(0.),
                    lever: Some(0.),
                    ..Default::default()
                },
            )
        });
        if warned.is_none() && slowing.stall_alert(0.).is_some() {
            warned = Some(indicated(&slowing));
        }
    }
    let warned = warned.expect("stall warning");
    assert!(warned < 112. && warned > 90., "{warned} KCAS");
    // A hovering V-22 never warns.
    let mut hover = trimmed(1_000., 0., 87., StabilityLevel::Damper);
    fly(&mut hover, 120, |_| PilotInput::default());
    assert!(hover.stall_alert(0.).is_none());
}

// ---------------------------------------------------------------------
// T5: reconversion and a vertical landing.

/// T5: from airplane mode at 200 KCAS, the pilot asks for the helicopter
/// preset, slows with the lever back and the height held, then hovers and
/// lands vertically. Protection lets the nacelles up as the speed falls;
/// the touchdown is under 5 ft/s.
#[test]
fn t5_reconversion_and_a_vertical_landing() {
    let mut s = trimmed(500., true_airspeed(200., 500.), 0., StabilityLevel::Damper);
    s.command(PilotCommand::Set(Switch::Gear, true));
    let mut touchdown = None;
    for _ in 0..120 * 150 {
        let horizontal = s.velocity[0].hypot(s.velocity[2]);
        let hovering = s.nacelle_degrees() > 80.;
        fly(&mut s, 1, |s| {
            let ground = s.velocity[2];
            // The lever holds a climb rate about the hover lever.
            let climbing = |climb: f64| (0.74 + 0.03 * (climb - s.vertical_speed)).clamp(0., 1.);
            let (pitch, lever) = if !hovering && indicated(s) > 90. {
                // Slow down holding the height, lever back.
                (height_hold(s, 500., 0.08), 0.05)
            } else if !hovering {
                (
                    height_hold(s, 500., 0.08),
                    climbing(0.05 * (500. - s.position[1])),
                )
            } else if horizontal > 5. || s.position[1] > 60. {
                // Stop over the ground, then let down to 50 ft.
                let pitch = (0.05 + 0.02 * ground).clamp(-0.1, 0.25);
                (
                    pitch,
                    climbing((0.2 * (50. - s.position[1])).clamp(-10., 5.)),
                )
            } else {
                // Settle at 3 ft/s.
                ((0.05 + 0.02 * ground).clamp(-0.1, 0.25), climbing(-3.))
            };
            steer(
                s,
                Hold {
                    pitch: Some(pitch),
                    bank: Some(0.),
                    heading: Some(0.),
                    lever: Some(lever),
                    conversion: Some(87. / 97.5),
                    ..Default::default()
                },
            )
        });
        if s.weight_on_wheels() {
            touchdown = Some(s.vertical_speed);
            break;
        }
        assert!(!s.crashed, "crashed at {:?}", s.position);
    }
    let touchdown = touchdown.expect("landed");
    assert!(touchdown.abs() < 5., "{touchdown} ft/s");
    fly(&mut s, 240, |s| PilotInput {
        collective: Some(s.lift_controls.collective * 0.5),
        conversion: Some(87. / 97.5),
        ..Default::default()
    });
    assert!(!s.crashed && s.weight_on_wheels());
    assert!((s.nacelle_degrees() - 87.).abs() < 1.);
}

// ---------------------------------------------------------------------
// T6: rotor strike.

/// A V-22 on the runway, gear down, nacelles at the helicopter preset,
/// the lever down and the rotors at speed.
fn on_the_runway() -> State {
    let mut s = untrimmed(0., StabilityLevel::Damper);
    s.start_on_runway([0., 0., 0.], 0.).unwrap();
    s.gear_down = true;
    s.gear = 1.;
    s.throttle = 1.;
    s.lift_controls.conversion = 87. / 97.5;
    s.lift_controls.conversion_actual = 87. / 97.5;
    fly(&mut s, 120, |_| PilotInput {
        collective: Some(0.),
        ..Default::default()
    });
    assert!(s.weight_on_wheels() && !s.crashed);
    s
}

/// T6: nacelles below 60 degrees on the wheels below 10 kt is a rotor
/// strike; a rolling takeoff with the nacelles at 45 degrees is not.
#[test]
fn t6_rotor_strike_below_10_kt_but_not_on_a_rolling_takeoff() {
    // Standing: the nacelles at 55 degrees strike at once, Easy flight
    // physics or not.
    for easy in [false, true] {
        let mut standing = on_the_runway();
        standing.cheats.easy_physics = easy;
        standing.lift_controls.conversion_actual = 55. / 97.5;
        fly(&mut standing, 1, |_| PilotInput::default());
        assert!(standing.crashed, "easy {easy}");
    }
    // A rolling takeoff at 45 degrees: rolling at 40 kt with the brakes
    // off and full lever, it lifts off without a strike.
    let mut rolling = on_the_runway();
    rolling.brake_out = false;
    rolling.brake = 0.;
    rolling.velocity = [0., 0., 40. * KT];
    rolling.speed = 40. * KT;
    rolling.lift_controls.conversion = 45. / 97.5;
    rolling.lift_controls.conversion_actual = 45. / 97.5;
    let mut airborne = false;
    for _ in 0..120 * 20 {
        fly(&mut rolling, 1, |s| {
            steer(
                s,
                Hold {
                    pitch: Some(0.03),
                    bank: Some(0.),
                    heading: Some(0.),
                    lever: Some(1.),
                    ..Default::default()
                },
            )
        });
        assert!(!rolling.crashed, "crashed rolling");
        airborne |= rolling.position[1] > 50.;
    }
    assert!(airborne, "never lifted off");
    assert!(rolling.nacelle_degrees() < 50.);
    // A rolling landing with the nacelles left at 45 degrees strikes once
    // it slows below 10 kt.
    let mut landing = on_the_runway();
    let speed = 60. * KT;
    landing.velocity = [0., 0., speed];
    landing.speed = speed;
    landing.lift_controls.conversion = 45. / 97.5;
    landing.lift_controls.conversion_actual = 45. / 97.5;
    let mut struck_at = None;
    for _ in 0..120 * 60 {
        let before = landing.velocity[0].hypot(landing.velocity[2]);
        fly(&mut landing, 1, |_| PilotInput {
            collective: Some(0.),
            ..Default::default()
        });
        if landing.crashed {
            struck_at = Some(before);
            break;
        }
    }
    let struck_at = struck_at.expect("rotor strike") / KT;
    assert!(struck_at < 10.5 && struck_at > 9., "{struck_at} kt");
}

// ---------------------------------------------------------------------
// Determinism, hazards and warnings.

/// Mid-conversion with protection holding the nacelles, the state restores
/// exactly from the wire's exact coding and flies on bit for bit for 1,200
/// ticks.
#[test]
fn a_tiltrotor_restores_exactly_mid_conversion() {
    let (s, _) = convert(StabilityLevel::Damper, 0.95, 6);
    assert!(
        s.lift_controls.corridor_hold.is_some(),
        "protection holding"
    );
    assert!((10. ..85.).contains(&s.nacelle_degrees()));
    let model = crate::models::AircraftModel::for_aircraft(&pt_v22()).unwrap();
    let mut writer = tore_codec::BitWriter::new();
    s.write_exact(&mut writer, None).unwrap();
    let bytes = writer.as_bytes();
    let mut restored =
        State::read_exact(&mut tore_codec::BitReader::new(bytes), None, &model).unwrap();
    assert_eq!(s, restored);
    let mut original = s.clone();
    for tick in 0..1200 {
        let input = PilotInput {
            pitch: (tick as f64 / 90.).sin() * 0.2,
            roll: (tick as f64 / 70.).cos() * 0.1,
            yaw: 0.1,
            collective: Some(0.6 + 0.2 * (tick as f64 / 200.).sin()),
            conversion_rate: if tick < 600 { -1. } else { 1. },
            ..Default::default()
        };
        original.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
        restored.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
        assert_eq!(original, restored, "tick {tick}");
    }
}

/// The rotors take the hazards slice P8 switches off (the vortex ring
/// here); retreating blade stall belongs to edgewise flight, so the
/// proprotors never meet it in airplane mode, even past Vne.
#[test]
fn the_proprotors_take_the_rotor_hazards_in_edgewise_flight_only() {
    let s = trimmed(4_000., 0., 87., StabilityLevel::Off);
    let m = model(&s);
    let mut i = instant_of(&s);
    let vh = s.lift_controls.rotors[0].induced_fps;
    i.air_velocity = [0., -vh, 0.];
    let settle = |hazards: Hazards| {
        let mut i = i;
        i.hazards = hazards;
        let mut loads = m.loads(&i);
        for _ in 0..200 {
            for k in 0..2 {
                rotor::relax(&mut i.rotors[k], &loads.rotors[k], DT);
            }
            loads = m.loads(&i);
        }
        loads
    };
    let ring = settle(Hazards::ALL);
    let easy = settle(Hazards {
        vortex_ring: false,
        ..Hazards::ALL
    });
    assert!(ring.rotors.iter().all(|r| r.vortex_ring > 0.5));
    assert!(easy.thrust_lbf > 1.2 * ring.thrust_lbf);
    let mut fast = instant_of(&trimmed(4_000., 250. * KT, 0., StabilityLevel::Off));
    fast.air_velocity = fast.basis.forward.map(|f| f * 300. * KT);
    let stalls = m.loads(&fast).rotors.map(|r| r.blade_stall);
    assert_eq!(stalls, [0.; 2], "{} degrees", fast.nacelle.to_degrees());
}

/// Gear down above 140 KCAS sets the gear speed warning; the V-22's
/// structural speed is 280 KCAS on the downstops and the corridor's
/// maximum with the nacelles up.
#[test]
fn gear_speed_warning_and_structural_speed() {
    let mut s = trimmed(
        3_000.,
        true_airspeed(160., 3_000.),
        0.,
        StabilityLevel::Damper,
    );
    let lever = s.lift_controls.collective;
    fly(&mut s, 60, |_| PilotInput {
        collective: Some(lever),
        ..Default::default()
    });
    assert_eq!(s.lift_controls.warnings.gear_speed, 0);
    s.command(PilotCommand::Set(Switch::Gear, true));
    fly(&mut s, 60, |_| PilotInput {
        collective: Some(lever),
        ..Default::default()
    });
    assert!(s.lift_controls.warnings.gear_speed > 0);
    let calibrated = |s: &State| {
        kcas(
            s.overspeed_limit_fps().unwrap(),
            rotor::air_density(s.position[1]),
        )
    };
    assert!((calibrated(&s) - 280.).abs() < 1e-6, "{}", calibrated(&s));
    let hover = trimmed(3_000., 0., 87., StabilityLevel::Damper);
    assert!((calibrated(&hover) - 100.).abs() < 1e-6);
}
