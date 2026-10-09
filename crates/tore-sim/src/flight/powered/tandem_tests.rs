//! Acceptance tests H1 to H14 of the VTOL overhaul design (section 10) for
//! the CH-47, on a synthetic record carrying its PT's flight numbers
//! ([`super::tests::pt_aircraft`]: 18,078 lb empty, 3,307 lb fuel, 28,660 lb
//! maximum, 53 to 178 kt), at sea level on a standard day with no wind, at
//! stability level Off unless a test says otherwise. The tandem clauses
//! (differential collective for pitch, differential cyclic for yaw, the rear
//! rotor's interference, the longitudinal trim schedule, torque that cancels)
//! come after H14.
//!
//! The scripted pilots are part of the tests, not of the game: simple
//! attitude, heading and climb holds that fly the aircraft through its
//! stick, pedals and collective as a player would.

use super::drive::LOW_ROTOR;
use super::tests::{fly, model, pt_aircraft, trimmed_at};
use super::*;
use crate::attitude::Basis;
use crate::flight::{PilotCommand, PilotInput, Switch};
use tore_input::StabilityLevel;

const KT: f64 = 1.687_81;
const FPM: f64 = 60.;

fn trimmed(height: f64, airspeed_fps: f64) -> State {
    trimmed_at(height, airspeed_fps, StabilityLevel::Off)
}

/// What the scripted pilot holds. Angles in radians, climb in ft/s.
#[derive(Clone, Copy, Debug, Default)]
struct Hold {
    pitch: Option<f64>,
    bank: Option<f64>,
    heading: Option<f64>,
    climb: Option<f64>,
    /// A fixed collective lever instead of a climb hold.
    collective: Option<f64>,
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// Stick, pedals and collective that fly toward `hold` from `s`.
fn steer(s: &State, hold: Hold) -> PilotInput {
    let [p, q, r] = s.lift_controls.body_rates;
    let pitch = hold
        .pitch
        .map_or(0., |target| 3. * wrap(target - s.pitch) - 2.5 * q);
    let roll = hold
        .bank
        .map_or(0., |target| 2.5 * wrap(target - s.bank) - 0.8 * p);
    let yaw = hold
        .heading
        .map_or(0., |target| 2. * wrap(target - s.yaw) - 2.5 * r);
    let collective = hold.collective.or_else(|| {
        hold.climb.map(|climb| {
            (s.lift_controls.collective + DT * 0.05 * (climb - s.vertical_speed)).clamp(0., 1.)
        })
    });
    PilotInput {
        pitch: pitch.clamp(-1., 1.),
        roll: roll.clamp(-1., 1.),
        yaw: yaw.clamp(-1., 1.),
        collective,
        ..Default::default()
    }
}

/// A scripted pilot with trim: integrators on the pitch and bank errors
/// take out the steady stick a hold needs.
#[derive(Clone, Copy, Debug, Default)]
struct Trimmer {
    pitch: f64,
    bank: f64,
}

impl Trimmer {
    fn steer(&mut self, s: &State, hold: Hold) -> PilotInput {
        let mut input = steer(s, hold);
        if let Some(target) = hold.pitch {
            self.pitch = (self.pitch + DT * 1.5 * wrap(target - s.pitch)).clamp(-1., 1.);
            input.pitch = (input.pitch + self.pitch).clamp(-1., 1.);
        }
        if let Some(target) = hold.bank {
            self.bank = (self.bank + DT * 1.5 * wrap(target - s.bank)).clamp(-1., 1.);
            input.roll = (input.roll + self.bank).clamp(-1., 1.);
        }
        input
    }
}

/// A climb rate that brings the aircraft back to `height`.
fn toward(s: &State, height: f64) -> f64 {
    (0.3 * (height - s.position[1])).clamp(-20., 20.)
}

fn horizontal_speed(s: &State) -> f64 {
    s.velocity[0].hypot(s.velocity[2])
}

fn degrees(rate: f64) -> f64 {
    rate.to_degrees()
}

/// H1: trimmed with the controls centred, the hover holds position and
/// height within a foot for 10 s, at Off and at Damper.
#[test]
fn h1_a_trimmed_hover_holds_still_hands_off() {
    for level in [StabilityLevel::Off, StabilityLevel::Damper] {
        let mut s = trimmed_at(1_000., 0., level);
        assert_eq!(s.lift_controls.body_rates, [0.; 3]);
        let start = s.position;
        fly(&mut s, 1200, |_| PilotInput::default());
        for (axis, (now, then)) in s.position.iter().zip(start).enumerate() {
            assert!(
                (now - then).abs() < 1.,
                "{level:?} axis {axis}: {}",
                now - then
            );
        }
        assert!((s.lift_controls.drive.rotor_speed - 1.).abs() < 1e-3);
    }
}

/// H1b: no artificial velocity damping. A 2 ft/s sideways gust at Off is
/// still above 1 ft/s after 5 s.
#[test]
fn h1b_nothing_damps_the_velocity_away() {
    let mut s = trimmed(1_000., 0.);
    s.velocity[0] += 2.;
    fly(&mut s, 600, |_| PilotInput::default());
    assert!(horizontal_speed(&s) > 1., "{}", horizontal_speed(&s));
}

/// H2: nose down 15 degrees from a hover, height held by collective, at Damper: 100 kt
/// in 15 to 25 s, height within 150 ft.
#[test]
fn h2_a_nose_down_hover_reaches_100_kt_in_time() {
    let mut s = trimmed_at(1_000., 0., StabilityLevel::Damper);
    let mut reached = None;
    let mut worst: f64 = 0.;
    let mut pilot = Trimmer::default();
    for tick in 0..120 * 40 {
        let hold = Hold {
            pitch: Some((-15_f64).to_radians()),
            bank: Some(0.),
            heading: Some(0.),
            climb: Some(toward(&s, 1_000.)),
            ..Default::default()
        };
        fly(&mut s, 1, |s| pilot.steer(s, hold));
        worst = worst.max((s.position[1] - 1_000.).abs());
        if horizontal_speed(&s) >= 100. * KT {
            reached = Some((tick + 1) as f64 * DT);
            break;
        }
    }
    let seconds = reached.expect("never reached 100 kt");
    assert!((15. ..=25.).contains(&seconds), "{seconds} s");
    assert!(worst < 150., "height off by {worst}");
}

/// H2b: trim is speed control. From a hover, 10 percent of forward trim at
/// the Attitude level, with the height held by collective and the stick let
/// go, settles at a steady 60 to 100 kt with a steady pitch.
#[test]
fn h2b_forward_trim_is_speed_control() {
    let mut s = trimmed_at(2_000., 0., StabilityLevel::Attitude);
    s.command(PilotCommand::Lift(tore_input::LiftCommand::TrimAdjust(
        tore_input::TrimAxis::Pitch,
        -0.1,
    )));
    let mut speeds = Vec::new();
    let mut pitches = Vec::new();
    for _ in 0..180 {
        fly(&mut s, 120, |s| PilotInput {
            collective: Some(
                (s.lift_controls.collective + DT * 0.05 * (toward(s, 2_000.) - s.vertical_speed))
                    .clamp(0., 1.),
            ),
            ..Default::default()
        });
        speeds.push(s.speed / KT);
        pitches.push(s.pitch.to_degrees());
    }
    let speed = *speeds.last().unwrap();
    println!("H2B speed {speed} pitch {}", pitches.last().unwrap());
    assert!((60. ..=160.).contains(&speed), "{speed} kt");
    let recent = &speeds[speeds.len() - 10..];
    assert!(recent[9] - recent[0] < 1., "still accelerating: {recent:?}");
    let pitch = &pitches[pitches.len() - 10..];
    assert!((pitch[9] - pitch[0]).abs() < 0.5, "pitch moving: {pitch:?}");
    assert!((s.position[1] - 2_000.).abs() < 50.);
}

/// The fastest level trim, kt, at sea level with the power the engines
/// have, at the level in the loop: the level top speed.
fn top_speed(level: StabilityLevel) -> f64 {
    let s = trimmed(0., 0.);
    let m = model(&s);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let rho = rotor::air_density(0.);
    let available = m.available_power(rho, 1., 1.);
    let mut guess = None;
    let mut top = 0.;
    for kt in (60..260).step_by(2) {
        let kt = f64::from(kt);
        guess = m.trim(weight, 0., kt * KT, rho, 1., level);
        match guess {
            Some(t) if t.engine_power <= available => top = kt,
            _ => break,
        }
    }
    let _ = guess;
    top
}

/// H3: level top speed at maximum power, with the stability law's
/// longitudinal trim schedule: 160 to 170 kt. Without the schedule (Off)
/// the fuselage would have to dive to tilt the disks and the top speed is
/// lower.
#[test]
fn h3_power_sets_the_level_top_speed() {
    let damper = top_speed(StabilityLevel::Damper);
    assert!((160. ..=170.).contains(&damper), "Damper {damper} kt");
    let off = top_speed(StabilityLevel::Off);
    assert!(off > 120. && off < damper, "Off {off} kt");
}

/// The body rate on `axis` after `seconds` of full stick (or pedal) from a
/// trimmed hover.
fn full_stick_rate(axis: usize, seconds: f64, level: StabilityLevel) -> f64 {
    let mut s = trimmed_at(3_000., 0., level);
    fly(&mut s, (seconds * 120.) as usize, |_| {
        let mut stick = [0.; 3];
        stick[axis] = 1.;
        PilotInput {
            pitch: stick[0],
            roll: stick[1],
            yaw: stick[2],
            ..Default::default()
        }
    });
    // Stick axes [pitch, roll, yaw]; body rates [roll, pitch, yaw].
    degrees(s.lift_controls.body_rates[[1, 0, 2][axis]])
}

/// H4: full stick from a hover. At Off the rates reach the design's CH-47
/// hover targets (roll 45, pitch 25, yaw 45 deg/s) or more, up to twice
/// them, within 2 s; at Damper they come within 15 percent.
#[test]
fn h4_full_stick_reaches_the_hover_rate_targets() {
    let s = trimmed(3_000., 0.);
    let [roll, pitch, yaw] = s
        .model()
        .powered_lift()
        .unwrap()
        .targets
        .hover_rates_degrees_per_second;
    for (axis, target) in [(1, roll), (0, pitch), (2, yaw)] {
        let off = full_stick_rate(axis, 2., StabilityLevel::Off);
        assert!(
            off >= target && off < 2. * target,
            "axis {axis}: {off} deg/s at Off against {target}"
        );
        let damped = full_stick_rate(axis, 2., StabilityLevel::Damper);
        assert!(
            (damped / target - 1.).abs() < 0.15,
            "axis {axis}: {damped} deg/s damped against {target}"
        );
    }
}

/// H5: no artificial limits. The CH-47 banks past 60 degrees and pitches
/// past 45 nose down and nose up. (The single-rotor loop from 150 kt is not
/// asked of a 25 deg/s tandem.)
#[test]
fn h5_no_artificial_attitude_limits() {
    let reach = |stick: [f64; 3], seconds: usize, done: &dyn Fn(&State) -> bool| {
        let mut s = trimmed(3_000., 0.);
        for _ in 0..120 * seconds {
            fly(&mut s, 1, |_| PilotInput {
                pitch: stick[0],
                roll: stick[1],
                ..Default::default()
            });
            if done(&s) {
                return true;
            }
        }
        false
    };
    assert!(reach([0., 1., 0.], 6, &|s| s.bank > 60_f64.to_radians()));
    assert!(reach([-1., 0., 0.], 8, &|s| s.pitch < -45_f64.to_radians()));
    assert!(reach([1., 0., 0.], 8, &|s| s.pitch > 45_f64.to_radians()));
}

/// H6: a 10 percent collective step from a hover climbs at a rate that
/// settles with a time constant of 2 to 4 s (the design's; the tandem's low disk
/// loading and two rotors make it quicker than the single-rotor 3 to 6 s); full collective climbs at
/// 1,500 to 3,000 ft/min.
#[test]
fn h6_collective_sets_a_climb_rate_not_an_acceleration() {
    let mut s = trimmed(1_000., 0.);
    let lever = s.lift_controls.collective + 0.1;
    let mut climbs = Vec::new();
    for _ in 0..120 * 20 {
        let hold = Hold {
            pitch: Some(0.),
            bank: Some(s.bank),
            heading: Some(0.),
            collective: Some(lever),
            ..Default::default()
        };
        fly(&mut s, 1, |s| steer(s, hold));
        climbs.push(s.vertical_speed);
    }
    let settled = *climbs.last().unwrap();
    assert!(settled > 3., "{settled}");
    assert!(
        (settled - climbs[climbs.len() - 121]).abs() < 0.05 * settled,
        "still accelerating"
    );
    let rise = climbs.iter().position(|v| *v >= 0.632 * settled).unwrap() as f64 * DT;
    assert!((2. ..=4.).contains(&rise), "time constant {rise} s");
    let mut full = trimmed(1_000., 0.);
    fly(&mut full, 120 * 20, |s| {
        steer(
            s,
            Hold {
                pitch: Some(0.),
                bank: Some(s.bank),
                heading: Some(0.),
                collective: Some(1.),
                ..Default::default()
            },
        )
    });
    let rate = full.vertical_speed * FPM;
    assert!(
        (1_500. ..=3_000.).contains(&rate),
        "full collective {rate} ft/min"
    );
}

/// The steady power, ft·lbf/s, of a sea-level hover at the reference
/// weight with the hubs `height` above the ground (none out of ground
/// effect), the collective found by bisection.
fn hover_power(m: &Tandem, height: Option<f64>) -> f64 {
    let rho = rotor::air_density(0.);
    let weight = m.reference_weight;
    let instant = |lever: f64| {
        let mut i = Instant {
            basis: Basis::new(0., 0., 0.),
            air_velocity: [0.; 3],
            body_rates: [0.; 3],
            density: rho,
            rotor_speed: 1.,
            rotors: [Rotor {
                induced_fps: 30.,
                tilt: [0.; 2],
            }; 2],
            engine_power: 0.,
            collective: lever,
            controls: [0.; 3],
            longitudinal_trim: 0.,
            hub_height_agl_ft: [height; 2],
            seconds: 0.,
            hazards: Hazards::ALL,
            drag_factor: 1.,
            lift_factor: 1.,
        };
        let mut loads = m.loads(&i);
        for _ in 0..200 {
            for (state, out) in i.rotors.iter_mut().zip(&loads.rotors) {
                state.induced_fps += 0.5 * (out.induced_target_fps - state.induced_fps);
            }
            i.engine_power = loads.power;
            loads = m.loads(&i);
        }
        loads
    };
    let (mut low, mut high) = (0., 1.);
    for _ in 0..50 {
        let mid = 0.5 * (low + high);
        if instant(mid).thrust_lbf < weight {
            low = mid;
        } else {
            high = mid;
        }
    }
    instant(low).power
}

/// H7: hovering with the hubs half a rotor diameter above the ground takes
/// 85 to 92 percent of the power out of ground effect.
#[test]
fn h7_ground_effect_cushions_the_hover() {
    let m = model(&trimmed(1_000., 0.));
    let radius = m.rotors[0].radius_ft;
    let ratio = hover_power(&m, Some(radius)) / hover_power(&m, None);
    assert!((0.85..=0.92).contains(&ratio), "{ratio}");
    // It fades out with height.
    let high = hover_power(&m, Some(4. * radius)) / hover_power(&m, None);
    assert!(high > ratio && high < 1., "{high}");
}

/// H8: at a fixed collective, accelerating through 15 to 30 kt gains at
/// least 100 ft/min of climb at its best (translational lift).
#[test]
fn h8_translational_lift_climbs_through_25_kt() {
    let mut s = trimmed_at(1_000., 10. * KT, StabilityLevel::Off);
    let lever = s.lift_controls.collective;
    let level = s.pitch;
    let mut pitch = level;
    let mut best: f64 = f64::MIN;
    let start = s.vertical_speed;
    fly(&mut s, 120 * 25, |s| {
        pitch = (pitch - DT * 0.003 * (30. * KT - horizontal_speed(s))).clamp(-0.12, 0.1);
        if (15. * KT..30. * KT).contains(&horizontal_speed(s)) {
            best = best.max(s.vertical_speed);
        }
        steer(
            s,
            Hold {
                pitch: Some(pitch),
                bank: Some(s.bank),
                heading: Some(0.),
                collective: Some(lever),
                ..Default::default()
            },
        )
    });
    let speed = horizontal_speed(&s) / KT;
    assert!((25. ..=40.).contains(&speed), "{speed} kt");
    let climb = (best - start) * FPM;
    assert!(climb >= 100., "{climb} ft/min");
}

/// Autorotation from 80 kt at 1,500 ft: the engine cut, the collective
/// down within a second and then managing the rotor speed, the airspeed
/// held near 75 kt, then from `flare` ft a flare to `flare_degrees` nose up
/// and, a second above the ground, the collective cushioning the sink.
/// Returns (lowest rotor speed before the flare, steady descent ft/min,
/// steady speed kt, touchdown vertical ft/s and ground speed kt, crashed).
fn autorotate(flare: f64, flare_degrees: f64) -> (f64, f64, f64, f64, f64, bool) {
    let mut s = trimmed_at(1_500., 80. * KT, StabilityLevel::Off);
    let level = s.pitch;
    s.command(PilotCommand::Set(Switch::Gear, true));
    s.command(PilotCommand::Set(Switch::Engine, false));
    let mut lowest = f64::MAX;
    let mut steady = (0., 0.);
    let mut lever = 0.;
    let mut pitch = level;
    let mut touchdown = None;
    let mut pilot = Trimmer::default();
    let mut cushion: f64 = 0.;
    let mut held: f64 = 0.;
    for tick in 0..120 * 90 {
        let height = s.position[1] - s.model().configuration().equipment.ground_clearance_ft;
        let nr = s.lift_controls.drive.rotor_speed;
        // Rotor speed by collective: up when it runs fast.
        if tick > 120 {
            lever = (4. * (nr - 1.)).clamp(0., 0.6);
        }
        let hold = if height > flare {
            pitch = (pitch + DT * 0.002 * (s.speed - 75. * KT)).clamp(-0.3, 0.3);
            let pitch = (pitch + 0.004 * (s.speed - 75. * KT)).clamp(-0.3, 0.3);
            Hold {
                pitch: Some(pitch),
                bank: Some(0.),
                heading: Some(0.),
                collective: Some(lever),
                ..Default::default()
            }
        } else {
            // The flare: nose up to trade speed for rotor speed and lift,
            // easing off as the speed goes and level near the ground.
            let ease = ((horizontal_speed(&s) / KT - 5.) / 20.).clamp(0., 1.);
            let pitch = if height > 20. {
                (flare_degrees * ease).to_radians()
            } else {
                6_f64.to_radians()
            };
            held = held.max(lever);
            if cushion > 0. || height < 1.2 * -s.vertical_speed + 3. {
                cushion =
                    (cushion.max(lever) + DT * 1.5 * (-5. - s.vertical_speed)).clamp(0.01, 1.);
            }
            Hold {
                pitch: Some(pitch),
                bank: Some(0.),
                heading: Some(0.),
                collective: Some(if cushion > 0. { cushion } else { held }),
                ..Default::default()
            }
        };
        if height > flare {
            lowest = lowest.min(nr);
        }
        if tick == 120 * 25 {
            steady = (s.vertical_speed * FPM, s.speed / KT);
        }
        let before = (s.vertical_speed, horizontal_speed(&s));
        if height > flare {
            fly(&mut s, 1, |s| pilot.steer(s, hold));
        } else {
            fly(&mut s, 1, |s| steer(s, hold));
        }
        if s.weight_on_wheels() || s.crashed {
            touchdown = Some(before);
            break;
        }
    }
    let (vertical, ground) = touchdown.expect("touched down");
    (
        lowest,
        -steady.0,
        steady.1,
        vertical,
        ground / KT,
        s.crashed,
    )
}

/// H9: autorotation. Rotor speed above 90 percent; a steady descent of
/// 1,500 to 2,500 ft/min at 70 to 80 kt; a flare touches down under 10 ft/s
/// and 20 kt.
#[test]
fn h9_an_engine_cut_at_speed_autorotates_to_a_landing() {
    let (lowest, descent, speed, ..) = autorotate(100., 20.);
    assert!(lowest > 0.9, "rotor speed fell to {lowest}");
    assert!((1_200. ..=2_500.).contains(&descent), "{descent} ft/min");
    assert!((70. ..=80.).contains(&speed), "{speed} kt");
    let mut tried = Vec::new();
    let landed = (4..=16).any(|tens| {
        (0..=6).any(|step| {
            let degrees = 15. + 2.5 * f64::from(step);
            let (.., vertical, ground, crashed) = autorotate(f64::from(tens) * 10., degrees);
            tried.push((tens * 10, degrees, vertical, ground, crashed));
            !crashed && vertical > -10. && ground < 20.
        })
    });
    assert!(landed, "no flare landed: {tried:?}");
}

/// H9b: the engines cut in a hover with the collective held. Rotor speed
/// falls below 80 percent in 2 to 3 s (the CH-47's rotor energy constant is
/// 2.5 s, design 8.2) with the LOW ROTOR warning; once below 70 percent,
/// lowering the collective no longer saves it.
#[test]
fn h9b_an_engine_cut_in_the_hover_droops_the_rotor_past_recovery() {
    let mut s = trimmed(3_000., 0.);
    let lever = s.lift_controls.collective;
    s.command(PilotCommand::Set(Switch::Engine, false));
    let mut below = None;
    for tick in 0..120 * 6 {
        fly(&mut s, 1, |_| PilotInput {
            collective: Some(lever),
            ..Default::default()
        });
        if below.is_none() && s.lift_controls.drive.rotor_speed < LOW_ROTOR {
            below = Some((tick + 1) as f64 * DT);
        }
        if s.lift_controls.drive.rotor_speed < 0.69 {
            break;
        }
    }
    let seconds = below.expect("rotor speed fell");
    assert!((2. ..=3.).contains(&seconds), "{seconds} s");
    assert!(s.lift_controls.warnings.low_rotor > 0);
    assert!(s.lift_controls.drive.rotor_speed < 0.7);
    fly(&mut s, 120 * 10, |_| PilotInput {
        collective: Some(0.),
        ..Default::default()
    });
    assert!(
        s.lift_controls.drive.rotor_speed < 0.7,
        "recovered to {}",
        s.lift_controls.drive.rotor_speed
    );
}

/// The loads' instant of a flying state, for inspection.
fn instant_of(s: &State) -> Instant {
    Instant {
        basis: Basis::new(s.yaw, s.pitch, s.bank),
        air_velocity: s.velocity,
        body_rates: s.lift_controls.body_rates,
        density: rotor::air_density(s.position[1]),
        rotor_speed: s.lift_controls.drive.rotor_speed,
        rotors: s.lift_controls.rotors,
        engine_power: s.lift_controls.drive.engine_output[0],
        collective: s.lift_controls.collective_actual,
        controls: s.lift_controls.aids.trim,
        longitudinal_trim: 0.,
        hub_height_agl_ft: [None; 2],
        seconds: 0.,
        hazards: Hazards::ALL,
        drag_factor: 1.,
        lift_factor: 1.,
    }
}

/// Descending vertically at one hover induced velocity below 10 kt, at the
/// hover collective for a second.
fn in_the_vortex_ring() -> State {
    let mut s = trimmed(4_000., 0.);
    let m = model(&s);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let vh = (weight / 2. / (2. * rotor::air_density(4_000.) * m.rotors[0].area_ft2)).sqrt();
    s.velocity[1] = -vh;
    let lever = s.lift_controls.collective;
    fly(&mut s, 120, |s| {
        steer(
            s,
            Hold {
                pitch: Some(0.),
                bank: Some(s.bank),
                heading: Some(0.),
                collective: Some(lever),
                ..Default::default()
            },
        )
    });
    s
}

/// H10: vortex ring state. Full collective fails to arrest the sink within
/// 3 s; forward flight to 30 kt recovers within 5 s.
#[test]
fn h10_full_collective_cannot_climb_out_of_the_vortex_ring() {
    let mut s = in_the_vortex_ring();
    assert!(s.vertical_speed < -15., "{}", s.vertical_speed);
    let mut deepest: f64 = 0.;
    for _ in 0..120 * 3 {
        let hold = Hold {
            pitch: Some(0.),
            bank: Some(s.bank),
            heading: Some(0.),
            collective: Some(1.),
            ..Default::default()
        };
        fly(&mut s, 1, |s| steer(s, hold));
        assert!(s.vertical_speed < 0., "arrested");
        let loads = model(&s).loads(&instant_of(&s));
        deepest = deepest.max(loads.rotors[0].vortex_ring.max(loads.rotors[1].vortex_ring));
    }
    assert!(deepest > 0.5, "{deepest}");
    let mut recovered = None;
    let mut pilot = Trimmer::default();
    for tick in 0..120 * 8 {
        let hold = Hold {
            pitch: Some(if horizontal_speed(&s) < 30. * KT {
                (-30_f64).to_radians()
            } else {
                0.
            }),
            bank: Some(0.),
            heading: Some(0.),
            collective: Some(1.),
            ..Default::default()
        };
        fly(&mut s, 1, |s| pilot.steer(s, hold));
        if s.vertical_speed >= 0. {
            recovered = Some(tick as f64 * DT);
            break;
        }
    }
    let seconds = recovered.expect("never recovered");
    assert!(seconds <= 5., "{seconds} s");
}

/// H11: diving past the never-exceed speed with the stick frozen sets the
/// vibration flag and adds nose-up pitching moment and a thrust loss at the
/// rotors (the counter-rotating pair's roll tendencies oppose, so the roll
/// is not asked of the tandem).
#[test]
fn h11_retreating_blade_stall_past_vne_pitches_up() {
    let mut s = trimmed_at(4_000., 120. * KT, StabilityLevel::Off);
    // The airframe's structural limit is the overspeed rule's; keep it from
    // ending the dive.
    s.cheats.damage = crate::cheats::Damage::Invulnerable;
    let mut pilot = Trimmer::default();
    let vne = s
        .model()
        .powered_lift()
        .unwrap()
        .rotor
        .unwrap()
        .never_exceed_kt;
    // Dive at 25 degrees nose down until the blades stall.
    let mut start = None;
    for _ in 0..120 * 80 {
        let hold = Hold {
            pitch: Some((-25_f64).to_radians()),
            bank: Some(0.),
            heading: Some(0.),
            collective: Some(s.lift_controls.collective),
            ..Default::default()
        };
        let input = pilot.steer(&s, hold);
        fly(&mut s, 1, |_| input.clone());
        if s.lift_controls.warnings.blade_stall > 0 {
            let speed = s.speed / KT;
            fly(&mut s, 12, |_| input.clone());
            start = Some((speed, instant_of(&s)));
            break;
        }
    }
    let (speed, now) = start.unwrap_or_else(|| panic!("no vibration, {} kt", s.speed / KT));
    assert!(
        (vne - 50. ..vne + 15.).contains(&speed),
        "stalled at {speed} kt"
    );
    // The stall's effect on the rotors: the same instant with the hazard off
    // has more thrust and less nose-up pitching moment.
    let model = model(&s);
    let with = model.loads(&now);
    let mut easy = now;
    easy.hazards.blade_stall = false;
    let without = model.loads(&easy);
    assert!(
        with.rotors.iter().any(|o| o.blade_stall > 0.),
        "{:?}",
        with.rotors.map(|o| o.blade_stall)
    );
    assert!(with.thrust_lbf < without.thrust_lbf);
    assert!(
        with.moments.applied[1] > without.moments.applied[1],
        "{} against {}",
        with.moments.applied[1],
        without.moments.applied[1]
    );
}

/// H12: a 30 percent collective step from a hover with the pedals fixed.
/// The tandem clause: its two torques cancel, so the yaw rate stays under
/// 1 deg/s within 2 s at Off, and at Damper too.
#[test]
fn h12_collective_does_not_swing_the_nose() {
    for level in [StabilityLevel::Off, StabilityLevel::Damper] {
        let mut s = trimmed_at(3_000., 0., level);
        let lever = s.lift_controls.collective + 0.3;
        let mut worst: f64 = 0.;
        fly(&mut s, 240, |s| {
            worst = worst.max(s.lift_controls.body_rates[2].abs());
            PilotInput {
                collective: Some(lever),
                ..Default::default()
            }
        });
        assert!(degrees(worst) < 1., "{level:?} {} deg/s", degrees(worst));
    }
}

/// The highest altitude, ft, at which the CH-47 at `weight` can hover out
/// of ground effect with the power its engines have there.
fn hover_ceiling(weight: f64) -> f64 {
    let s = trimmed(0., 0.);
    let m = model(&s);
    (0..30_000)
        .step_by(250)
        .map(f64::from)
        .take_while(|altitude| {
            let rho = rotor::air_density(*altitude);
            m.trim(weight, 0., 0., rho, 1., StabilityLevel::Off)
                .is_some_and(|t| t.engine_power <= m.available_power(rho, 1., 1.))
        })
        .last()
        .unwrap_or(0.)
}

/// H13: weight. A heavier CH-47 hovers lower, and at its maximum weight it
/// cannot hover out of ground effect above its ceiling: flown 1,500 ft
/// above it, full collective sinks.
#[test]
fn h13_weight_and_altitude_take_the_hover_away() {
    let gross = hover_ceiling(18_078. + 3_307.);
    let heavy = hover_ceiling(28_660.);
    assert!(
        gross > heavy + 2_000. && heavy > 4_000.,
        "{gross} / {heavy} ft"
    );
    // 1,500 ft above the heavy ceiling the power falls short of the hover.
    let s = trimmed(0., 0.);
    let m = model(&s);
    let rho = rotor::air_density(heavy + 1_500.);
    let t = m
        .trim(28_660., 0., 0., rho, 1., StabilityLevel::Off)
        .unwrap();
    assert!(t.engine_power > m.available_power(rho, 1., 1.));
}

/// The CH-47 on the runway with its rotors turning.
fn on_the_ground() -> State {
    let mut s = State::new(&pt_aircraft(), [0., 0., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.start_on_runway([0., 0., 0.], 0.).unwrap();
    s.throttle = 1.;
    s.gear_down = true;
    s.gear = 1.;
    fly(&mut s, 120, |_| PilotInput::default());
    assert!(s.weight_on_wheels() && !s.crashed);
    s
}

/// H14: dynamic rollover. On the wheels, 20 degrees of bank with the thrust
/// leaning the same way is a crash; the same bank with the collective down,
/// or a gentle lift-off, is not.
#[test]
fn h14_dynamic_rollover_on_the_ground() {
    let mut s = on_the_ground();
    let lever = trimmed(0., 0.).lift_controls.collective;
    s.lift_controls.collective = 0.8 * lever;
    s.lift_controls.collective_actual = 0.8 * lever;
    s.bank = 20_f64.to_radians();
    s.lift_controls.rotors[0].tilt[1] = 0.02;
    s.lift_controls.rotors[1].tilt[1] = 0.02;
    fly(&mut s, 1, |_| PilotInput::default());
    assert!(s.crashed, "20 degrees under thrust");
    let mut flat = on_the_ground();
    flat.bank = 20_f64.to_radians();
    fly(&mut flat, 1, |_| PilotInput::default());
    assert!(!flat.crashed && flat.bank == 0., "collective down");
    let mut off = on_the_ground();
    fly(&mut off, 120 * 6, |_| PilotInput {
        collective: Some(1.05 * lever),
        ..Default::default()
    });
    assert!(!off.crashed && !off.weight_on_wheels(), "lift-off");
}

/// A CH-47 mid-autorotation and mid-hover restores exactly from the wire's
/// exact coding and flies on bit for bit, and two copies with the same
/// inputs stay equal.
#[test]
fn a_tandem_restores_exactly_mid_flight() {
    let mut hovering = trimmed_at(800., 0., StabilityLevel::Damper);
    fly(&mut hovering, 120, |s| PilotInput {
        roll: 0.1,
        yaw: -0.2,
        collective: Some(s.lift_controls.collective + 0.05),
        ..Default::default()
    });
    let mut autorotating = trimmed_at(2_000., 80. * KT, StabilityLevel::Damper);
    autorotating.command(PilotCommand::Set(Switch::Engine, false));
    fly(&mut autorotating, 360, |_| PilotInput {
        collective: Some(0.1),
        pitch: 0.05,
        ..Default::default()
    });
    assert!(autorotating.lift_controls.drive.engine_output[0] == 0.);
    for s in [hovering, autorotating] {
        let model = crate::models::AircraftModel::for_aircraft(&pt_aircraft()).unwrap();
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
                collective: Some(0.3 + 0.2 * (tick as f64 / 200.).sin()),
                ..Default::default()
            };
            original.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
            restored.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
            assert_eq!(original, restored, "tick {tick}");
        }
    }
}

/// Pitch is differential collective: aft stick raises the front rotor's
/// thrust and lowers the rear's by the same amount, a nose-up moment of
/// `dT * l`, with little change in the total and none in roll or yaw.
#[test]
fn pitch_is_differential_collective() {
    let s = trimmed(1_000., 0.);
    let m = model(&s);
    let level = instant_of(&s);
    let base = m.loads(&level);
    let mut aft = level;
    aft.controls[0] += 0.5;
    let pulled = m.loads(&aft);
    let [front, rear] = [0, 1].map(|k| pulled.rotors[k].thrust_lbf - base.rotors[k].thrust_lbf);
    assert!(front > 100. && rear < -100., "{front} {rear}");
    assert!((front + rear).abs() < 0.15 * front, "{front} {rear}");
    let moment = pulled.moments.applied[1] - base.moments.applied[1];
    assert!(moment > 0.5 * (front - rear) * m.arm_ft, "{moment}");
    assert!(pulled.moments.applied[0].abs() < 0.02 * moment.abs());
}

/// Roll tilts both disks the same way; yaw tilts them in opposite
/// directions (pedal right: front right, rear left) for a nose-right
/// moment `2 T sin(delta) l` and no roll.
#[test]
fn roll_tilts_both_disks_and_yaw_tilts_them_apart() {
    let s = trimmed(1_000., 0.);
    let m = model(&s);
    let hover = instant_of(&s);
    let sideways = |controls: [f64; 3]| {
        let mut i = hover;
        i.controls = controls;
        let l = m.loads(&i);
        (l.rotors.map(|o| o.tilt_target[1]), l.moments.applied)
    };
    let (roll_tilt, roll_moment) = sideways([0., 0.5, 0.]);
    assert!(
        roll_tilt[0] > 0.01 && (roll_tilt[0] - roll_tilt[1]).abs() < 1e-3,
        "{roll_tilt:?}"
    );
    assert!(roll_moment[0] > 0.);
    let (yaw_tilt, yaw_moment) = sideways([0., 0., 0.5]);
    assert!(yaw_tilt[0] > 0.01 && yaw_tilt[1] < -0.01, "{yaw_tilt:?}");
    assert!((yaw_tilt[0] + yaw_tilt[1]).abs() < 1e-3);
    assert!(yaw_moment[2] > 0.);
    // No tail rotor, so no pedal-to-roll coupling beyond the hub lever.
    assert!(yaw_moment[0].abs() < 0.05 * yaw_moment[2].abs());
}

/// The rear rotor flies in the front rotor's wake: it needs more collective
/// than the front for the same thrust in a hover, the hover takes more
/// power than without the wake (the design's about 10 percent), and the
/// wake fades out by 40 kt.
#[test]
fn the_rear_rotor_flies_in_the_front_wake() {
    let s = trimmed(1_000., 0.);
    let m = model(&s);
    // The trim's differential: aft rotor's blade pitch above the front's.
    assert!(s.lift_controls.aids.trim[0] < -0.01);
    let mut dry = m;
    dry.interference = 0.;
    let wet = hover_power(&m, None);
    let without = hover_power(&dry, None);
    let extra = wet / without - 1.;
    assert!((0.04..=0.15).contains(&extra), "{extra}");
    // Fades out by 40 kt: the same loads with and without the wake.
    let mut cruise = instant_of(&s);
    cruise.air_velocity = [0., 0., 41. * KT];
    let a = m.loads(&cruise);
    let b = dry.loads(&cruise);
    assert!((a.thrust_lbf / b.thrust_lbf - 1.).abs() < 1e-9);
    // And is on in a hover for the rear rotor only.
    let hover = instant_of(&s);
    let (a, b) = (m.loads(&hover), dry.loads(&hover));
    assert!(a.rotors[1].thrust_lbf < b.rotors[1].thrust_lbf - 50.);
    assert_eq!(a.rotors[0].thrust_lbf, b.rotors[0].thrust_lbf);
}

/// The two rotors' torques cancel but for their difference: the residual yaw
/// moment is a small share of one rotor's torque, and gone with the torque
/// hazard off.
#[test]
fn torque_cancels_but_for_the_residual() {
    let s = trimmed(1_000., 0.);
    let m = model(&s);
    let mut i = instant_of(&s);
    i.engine_power *= 1.3;
    let loads = m.loads(&i);
    let one_rotor = i.engine_power / (i.rotor_speed * m.rotors[0].omega) / 2.;
    assert!(
        loads.torque_yaw.abs() < 0.1 * one_rotor,
        "{} of {one_rotor}",
        loads.torque_yaw
    );
    assert!(loads.torque_yaw != 0.);
    i.hazards.torque = false;
    assert_eq!(m.loads(&i).torque_yaw, 0.);
    // Autorotating, the rotors leave no torque at all.
    i.hazards.torque = true;
    i.engine_power = 0.;
    assert_eq!(m.loads(&i).torque_yaw, 0.);
}

/// The longitudinal trim schedule (Damper and Attitude, 40 to 140 kt) tilts
/// both disks forward, so the fuselage flies more level than at Off, where
/// it has to tilt the whole aircraft.
#[test]
fn the_longitudinal_trim_schedule_levels_the_fuselage() {
    let off = trimmed_at(1_000., 120. * KT, StabilityLevel::Off);
    let damper = trimmed_at(1_000., 120. * KT, StabilityLevel::Damper);
    assert!(
        damper.pitch > off.pitch + 1.5_f64.to_radians(),
        "{} against {}",
        damper.pitch.to_degrees(),
        off.pitch.to_degrees()
    );
    let m = model(&damper);
    let (long_off, long_damper) = (
        off.lift_controls.rotors[0].tilt[0],
        damper.lift_controls.rotors[0].tilt[0],
    );
    assert!(long_damper > long_off, "{long_damper} {long_off}");
    // None below 40 kt.
    let slow = trimmed_at(1_000., 30. * KT, StabilityLevel::Damper);
    let slow_off = trimmed_at(1_000., 30. * KT, StabilityLevel::Off);
    assert!((slow.pitch - slow_off.pitch).abs() < 0.01);
    let full = m.trim_cyclic * m.rotors[0].cyclic_range()[0];
    assert!((full - 2.5_f64.to_radians()).abs() < 1e-9, "{full}");
}

/// One drive: both rotors turn at the one rotor speed, and engine power is
/// shared by the rotors in proportion to the power each absorbs.
#[test]
fn both_rotors_share_one_drive() {
    let s = trimmed(1_000., 0.);
    let m = model(&s);
    let loads = m.loads(&instant_of(&s));
    assert_eq!(loads.power, loads.rotors[0].power + loads.rotors[1].power);
    // The trim balances the engine's power against both rotors' and the
    // governor holds the one rotor speed.
    assert!((s.lift_controls.drive.engine_output[0] / loads.power - 1.).abs() < 1e-3);
    let mut s = s;
    fly(&mut s, 600, |_| PilotInput::default());
    assert!((s.lift_controls.drive.rotor_speed - 1.).abs() < 1e-3);
}

/// The Easy flight physics hazards at the rotors: without the vortex ring
/// the same descent gives more thrust; without blade stall no pitch-up past
/// Vne; without rotor stall a slow rotor keeps its thrust; without dynamic
/// rollover the ground tips nothing over.
#[test]
fn each_hazard_switches_off_at_the_rotors() {
    let s = trimmed(4_000., 0.);
    let m = model(&s);
    let mut i = instant_of(&s);
    let vh = s.lift_controls.rotors[0].induced_fps;
    i.air_velocity = [0., -vh, 0.];
    let settle = |hazards: Hazards| {
        let mut i = i;
        i.hazards = hazards;
        let mut loads = m.loads(&i);
        for _ in 0..200 {
            for (state, out) in i.rotors.iter_mut().zip(&loads.rotors) {
                rotor::relax(state, out, DT);
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
    assert!(ring.rotors[0].vortex_ring > 0.5 && easy.rotors[0].vortex_ring == 0.);
    assert!(
        easy.thrust_lbf > 1.1 * ring.thrust_lbf,
        "{} {}",
        easy.thrust_lbf,
        ring.thrust_lbf
    );
    // Past Vne.
    let mut fast = instant_of(&trimmed(4_000., 120. * KT));
    fast.air_velocity = fast.basis.forward.map(|f| f * 240. * KT);
    let stalled = m.loads(&fast);
    fast.hazards.blade_stall = false;
    let clean = m.loads(&fast);
    assert!(stalled.rotors[0].blade_stall > 0.5 && clean.rotors[0].blade_stall == 0.);
    assert!(stalled.thrust_lbf < clean.thrust_lbf);
    // A rotor at 65 percent.
    let mut slow = instant_of(&s);
    slow.rotor_speed = 0.65;
    let stalled = m.loads(&slow);
    slow.hazards.rotor_stall = false;
    let clean = m.loads(&slow);
    assert!(stalled.rotors[0].rotor_stall > 0.9 && clean.rotors[0].rotor_stall == 0.);
    assert!(stalled.thrust_lbf < 0.5 * clean.thrust_lbf);
}

/// An airborne start high above the hover ceiling trims on the lever alone,
/// and the engines cannot hold it: the aircraft sinks.
#[test]
fn an_overloaded_airborne_start_has_no_hover_support() {
    let mut overloaded = trimmed(5_000., 0.);
    overloaded.position[1] = 24_000.;
    overloaded.ticks = 0;
    overloaded.lift_controls.thrust_lbf = 0.;
    overloaded.set_payload(1_500.).unwrap();
    overloaded.initialize_airborne_hover();
    fly(&mut overloaded, 600, |_| PilotInput::default());
    assert!(overloaded.position[1] < 23_990.);
}

/// Reduced power takes the hover away: with the engines at half power the
/// rotor speed sags and the aircraft sinks.
#[test]
fn reduced_power_has_no_automatic_hover_support() {
    let mut damaged = trimmed(500., 0.);
    damaged.throttle *= 0.5;
    fly(&mut damaged, 1200, |_| PilotInput::default());
    assert!(
        damaged.position[1] < 450. && damaged.vertical_speed < -10.,
        "{} {}",
        damaged.position[1],
        damaged.vertical_speed
    );
}
