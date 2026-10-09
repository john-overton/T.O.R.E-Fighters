//! Probe of the vectoring jets (VTOL overhaul slice P4) against user-owned
//! PT records: hover margins, transition, wingborne envelope against the
//! conventional model on the same PT (the oracle), dive and zoom, rates.
//! No retail fixtures; prints a table for the baseline.
//!
//! Usage: `jet_probe AV8.PT [YAK141.PT ...] [--compare F16C.PT]`
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    flight::{DT, PilotCommand, PilotInput, State},
    models::{AircraftModel, FlightModel},
    research::Surface,
};

const KT: f64 = 1.687_81;

fn load(path: &str) -> Result<Aircraft, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("PT exceeds size limit".into());
    }
    Ok(Aircraft::parse(&bytes)?)
}

fn step(s: &mut State, input: &PilotInput) {
    s.step_surface(input, |_, _| Surface::runway(0.));
}

/// A level airborne start at `altitude` and `speed` (ft/s); `oracle` strips
/// the powered lift so the conventional law flies the same PT.
fn level(a: &Aircraft, altitude: f64, speed: f64, oracle: bool) -> State {
    let mut model = AircraftModel::for_aircraft(a).unwrap();
    if oracle && let AircraftModel::Variety(m) = &mut model {
        m.lift = None;
    }
    let mut s = State::from_model(model, [0., altitude, 0.]);
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = speed;
    s.velocity = [0., 0., speed];
    s
}

fn path_pitch(s: &State) -> f64 {
    s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]))
}

/// Stick to hold `pitch` (rad) and wings level.
fn attitude(s: &State, pitch: f64) -> [f64; 2] {
    [
        (3. * (pitch - s.pitch) - 0.6 * s.pitch_rate).clamp(-1., 1.),
        (-3. * s.bank - 0.5 * s.roll_rate).clamp(-1., 1.),
    ]
}

fn dive(a: &Aircraft, oracle: bool) -> String {
    let mut s = level(a, 10_000., 0., oracle);
    let start = tore_sim::flight::State::new(a, [0., 10_000., 0.])
        .unwrap()
        .speed;
    s.speed = start;
    s.velocity = [0., 0., start];
    s.throttle = 0.8;
    let mut ticks = 0;
    while s.pitch > -60_f64.to_radians() && ticks < 120 * 20 {
        let [_, roll] = attitude(&s, 0.);
        step(
            &mut s,
            &PilotInput {
                pitch: -1.,
                roll,
                ..Default::default()
            },
        );
        ticks += 1;
    }
    let pushed = ticks as f64 * DT;
    let mut out = Vec::new();
    for t in 1..=10 {
        for _ in 0..120 {
            let [_, roll] = attitude(&s, 0.);
            step(
                &mut s,
                &PilotInput {
                    roll,
                    ..Default::default()
                },
            );
        }
        if [1, 3, 5, 6, 10].contains(&t) {
            out.push(format!(
                "{t}s vs {:.0} nose {:.1} path {:.1} {:.0} kt",
                s.vertical_speed,
                s.pitch.to_degrees(),
                path_pitch(&s).to_degrees(),
                s.speed / KT
            ));
        }
    }
    format!(
        "dive from {:.0} kt (push {pushed:.1} s): {}",
        start / KT,
        out.join("; ")
    )
}

fn zoom(a: &Aircraft, oracle: bool) -> String {
    let mut s = level(a, 10_000., 350. * KT, oracle);
    s.throttle = 1.;
    s.burner = true;
    let mut out = Vec::new();
    for tick in 1..=120 * 12 {
        let [pitch, roll] = attitude(&s, 30_f64.to_radians());
        step(
            &mut s,
            &PilotInput {
                pitch,
                roll,
                throttle: Some(1.),
                ..Default::default()
            },
        );
        if tick % 240 == 0 {
            out.push(format!(
                "{}s vs {:.0} {:.0} kt",
                tick / 120,
                s.vertical_speed,
                s.speed / KT
            ));
        }
    }
    format!("zoom +30 from 350 kt: {}", out.join("; "))
}

/// Full power level flight, altitude held through the stick, for 240 s.
fn top_speed(a: &Aircraft, altitude: f64, oracle: bool) -> f64 {
    let mut s = level(a, altitude, 400. * KT, oracle);
    s.burner = true;
    for _ in 0..120 * 240 {
        let input = PilotInput {
            pitch: (0.002 * (altitude - s.position[1]) - 0.01 * s.vertical_speed).clamp(-0.2, 0.2),
            roll: (-3. * s.bank).clamp(-1., 1.),
            throttle: Some(1.),
            ..Default::default()
        };
        step(&mut s, &input);
    }
    s.speed / KT
}

/// Peak G in 3 s of full aft stick from level flight at `speed` kt.
fn peak_g(a: &Aircraft, speed: f64, oracle: bool) -> (f64, f64) {
    let mut s = level(a, 10_000., speed * KT, oracle);
    s.throttle = 1.;
    let mut peak: f64 = 0.;
    let mut rate: f64 = 0.;
    for _ in 0..360 {
        step(
            &mut s,
            &PilotInput {
                pitch: 1.,
                throttle: Some(1.),
                ..Default::default()
            },
        );
        if s.g > peak && std::env::var_os("PROBE_DEBUG").is_some() {
            let t = s.trace().adapter.unwrap();
            eprintln!(
                "peak {:.2} limits {:?} lift {:.2} speed {:.0} drag {:.0}",
                s.g, t.envelope.limits_g, s.maneuver.lift_g, s.speed, t.forces.drag.total_lbf
            );
        }
        peak = peak.max(s.g);
        rate += s.maneuver.body_rates_rad_per_second[1] / 360.;
    }
    (peak, rate.to_degrees())
}

/// Steady roll rate after 2 s of full stick at `speed` kt.
fn roll_rate(a: &Aircraft, speed: f64, oracle: bool) -> f64 {
    let mut s = level(a, 10_000., speed * KT, oracle);
    s.throttle = 1.;
    let mut peak: f64 = 0.;
    for _ in 0..240 {
        step(
            &mut s,
            &PilotInput {
                roll: 1.,
                throttle: Some(1.),
                ..Default::default()
            },
        );
        peak = peak.max(s.maneuver.body_rates_rad_per_second[0]);
    }
    peak.to_degrees()
}

/// Sustained level turn at `bank` degrees and full power from 400 kt, the
/// stick holding the altitude, for 90 s: the speed it settles at, kt, and
/// the turn rate there, deg/s.
fn sustained(a: &Aircraft, bank: f64, oracle: bool) -> (f64, f64) {
    let mut s = level(a, 10_000., 400. * KT, oracle);
    s.burner = true;
    let bank = bank.to_radians();
    let mut integral = 0.;
    let mut turned = 0.;
    for tick in 0..120 * 90 {
        let climb = -s.vertical_speed + 0.2 * (10_000. - s.position[1]);
        integral += climb * DT;
        let input = PilotInput {
            pitch: (0.01 * climb + 0.002 * integral).clamp(-1., 1.),
            roll: (3. * (bank - s.bank) - 0.5 * s.roll_rate).clamp(-1., 1.),
            throttle: Some(1.),
            ..Default::default()
        };
        let before = s.yaw;
        step(&mut s, &input);
        if tick >= 120 * 80 {
            let change = (s.yaw - before + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            turned += change.abs();
        }
        if s.crashed {
            return (f64::NAN, f64::NAN);
        }
    }
    (s.speed / KT, turned.to_degrees() / 10.)
}

/// Slowest speed at which level flight holds with full aft stick at most,
/// decelerating at idle (10,000 ft).
fn slow_speed(a: &Aircraft, oracle: bool) -> f64 {
    let mut s = level(a, 10_000., 250. * KT, oracle);
    s.throttle = 0.;
    s.burner = false;
    s.brake_out = true;
    s.brake = 1.;
    for _ in 0..120 * 120 {
        let input = PilotInput {
            pitch: (-0.02 * s.vertical_speed + 0.004 * (10_000. - s.position[1])).clamp(-1., 1.),
            roll: (-3. * s.bank).clamp(-1., 1.),
            throttle: Some(0.),
            ..Default::default()
        };
        step(&mut s, &input);
        if s.vertical_speed < -15. || s.position[1] < 9_900. {
            return s.speed / KT;
        }
    }
    s.speed / KT
}

/// Hover: thrust capacity at 100 ft out of ground effect, full throttle,
/// nozzles vertical, against the weight with `stores` lb.
fn hover(a: &Aircraft, stores: f64) -> String {
    let mut s = level(a, 1_000., 0., false);
    s.set_payload(stores).unwrap();
    s.throttle = 1.;
    s.lift_controls.vector_pitch = 0.9;
    s.lift_controls.vector_pitch_actual = 0.9;
    let mut climb = 0.;
    for tick in 0..120 * 8 {
        let [pitch, roll] = attitude(&s, 0.);
        step(
            &mut s,
            &PilotInput {
                pitch,
                roll,
                throttle: Some(1.),
                ..Default::default()
            },
        );
        if tick == 120 * 4 {
            climb = s.vertical_speed;
        }
    }
    let accel = (s.vertical_speed - climb) / 4.;
    format!(
        "hover +{stores:.0} lb: weight {:.0} thrust {:.0} lift-engine {:.0} vertical accel {:.2} ft/s² (margin {:+.1} %)",
        s.model().configuration().mass.empty_lbs + s.fuel + s.carried_lbs(),
        s.lift_controls.drive.engine_output[0],
        s.lift_controls.drive.engine_output[1],
        accel,
        accel / 32.174 * 100.
    )
}

/// Hover rates at full stick for 2 s on each axis.
fn hover_rates(a: &Aircraft) -> String {
    let mut out = Vec::new();
    for axis in 0..3 {
        let mut s = level(a, 1_000., 0., false);
        s.throttle = 0.95;
        s.lift_controls.vector_pitch = 0.9;
        s.lift_controls.vector_pitch_actual = 0.9;
        for _ in 0..240 {
            let mut input = PilotInput {
                throttle: Some(0.95),
                ..Default::default()
            };
            match axis {
                0 => input.pitch = 1.,
                1 => input.roll = 1.,
                _ => input.yaw = 1.,
            }
            step(&mut s, &input);
        }
        let rates = s.lift_controls.body_rates;
        out.push(format!(
            "{}: {:.0}",
            ["pitch", "roll", "yaw"][axis],
            [rates[1], rates[0], rates[2]][axis].to_degrees()
        ));
    }
    format!("hover full stick 2 s, Damper: {} deg/s", out.join(", "))
}

/// Manual transition from a 500 ft hover: nozzles to 60, then to 0 past 90
/// kt, full throttle, nose held level.
fn transition(a: &Aircraft) -> String {
    let mut s = level(a, 500., 0., false);
    s.throttle = 1.;
    s.lift_controls.vector_pitch = 0.9;
    s.lift_controls.vector_pitch_actual = 0.9;
    let mut lowest = s.position[1];
    let mut at_90 = None;
    let mut at_150 = None;
    for tick in 0..120 * 40 {
        let mut commands = Vec::new();
        if tick == 0 {
            for _ in 0..3 {
                commands.push(PilotCommand::Lift(tore_input::LiftCommand::NozzleStep {
                    down: false,
                }));
            }
        }
        if at_90.is_none() && s.speed >= 90. * KT {
            at_90 = Some(tick as f64 * DT);
            commands.push(PilotCommand::Lift(tore_input::LiftCommand::NozzlePreset(
                tore_input::NozzlePreset::Forward,
            )));
        }
        let target = (0.002 * (500. - s.position[1]) - 0.004 * s.vertical_speed).clamp(-0.2, 0.3);
        let [pitch, roll] = attitude(&s, target);
        step(
            &mut s,
            &PilotInput {
                pitch,
                roll,
                throttle: Some(1.),
                commands,
                ..Default::default()
            },
        );
        lowest = lowest.min(s.position[1]);
        if std::env::var_os("PROBE_DEBUG").is_some() && tick % 60 == 0 {
            eprintln!(
                "t {:.1} h {:.0} v {:.0} kt vs {:.0} pitch {:.1} bank {:.1} noz {:.0} thrust {:.0} crashed {}",
                tick as f64 * DT,
                s.position[1],
                s.speed / KT,
                s.vertical_speed,
                s.pitch.to_degrees(),
                s.bank.to_degrees(),
                s.lift_controls.vector_pitch_actual * 100.,
                s.lift_controls.drive.engine_output[0],
                s.crashed
            );
        }
        if at_150.is_none() && s.speed >= 150. * KT {
            at_150 = Some(tick as f64 * DT);
        }
    }
    format!(
        "transition 500 ft hover: 90 kt at {:.1} s, 150 kt at {:.1} s, lowest {:.0} ft, 40 s {:.0} kt {:.0} ft",
        at_90.unwrap_or(f64::NAN),
        at_150.unwrap_or(f64::NAN),
        lowest,
        s.speed / KT,
        s.position[1]
    )
}

fn wanted(name: &str) -> bool {
    std::env::var("PROBE_ONLY").map_or(true, |only| only.split(',').any(|o| o == name))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1).peekable();
    let mut jets = Vec::new();
    let mut compare = None;
    while let Some(arg) = args.next() {
        if arg == "--compare" {
            compare = args.next();
        } else {
            jets.push(arg);
        }
    }
    if let Some(path) = compare {
        let a = load(&path)?;
        println!("== {} (conventional)", a.name);
        println!("  {}", dive(&a, false));
        println!("  {}", zoom(&a, false));
    }
    for path in jets {
        let a = load(&path)?;
        println!("== {}", a.name);
        if wanted("hover") {
            for stores in [0., 1_000., 4_000.] {
                println!("  {}", hover(&a, stores));
            }
            println!("  {}", hover_rates(&a));
        }
        if wanted("transition") {
            println!("  {}", transition(&a));
        }
        for oracle in [false, true] {
            let label = if oracle { "oracle" } else { "jet" };
            if wanted("dive") {
                println!("  [{label}] {}", dive(&a, oracle));
                println!("  [{label}] {}", zoom(&a, oracle));
            }
            if wanted("speed") {
                println!(
                    "  [{label}] top speed 1,000 ft {:.0} kt, 10,000 ft {:.0} kt",
                    top_speed(&a, 1_000., oracle),
                    top_speed(&a, 10_000., oracle)
                );
                println!(
                    "  [{label}] slow level speed 10,000 ft {:.0} kt",
                    slow_speed(&a, oracle)
                );
            }
            if wanted("turn") {
                for speed in [300., 450.] {
                    let (g, pitch) = peak_g(&a, speed, oracle);
                    println!(
                        "  [{label}] {speed:.0} kt 10,000 ft: peak {g:.2} G, mean pitch rate {pitch:.1} deg/s, roll {:.0} deg/s",
                        roll_rate(&a, speed, oracle)
                    );
                }
                for bank in [60., 70.] {
                    let (speed, turn) = sustained(&a, bank, oracle);
                    println!(
                        "  [{label}] sustained {bank:.0} deg bank, 10,000 ft: {speed:.0} kt, {turn:.1} deg/s"
                    );
                }
            }
        }
    }
    Ok(())
}
