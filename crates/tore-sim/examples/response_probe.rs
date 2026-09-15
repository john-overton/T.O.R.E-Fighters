//! Fixed-input response ledger; optional TORE_RESPONSE_TRACE directory holds per-tick state.
use std::{
    env, fs,
    io::{BufWriter, Write},
};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    attitude::{Basis, dot, unit},
    flight::{PilotInput, State},
    models::FlightModel,
    research::Surface,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: response_probe EXTRACTED.PT [SECOND.PT ...]".into());
    }
    for path in paths {
        let bytes = fs::read(path)?;
        if bytes.len() > 1024 * 1024 {
            return Err("PT exceeds size bound".into());
        }
        let aircraft = Aircraft::parse(&bytes)?;
        for hybrid in [false, true] {
            let mut loop_state = State::new(&aircraft, [0., 15000., 0.])?;
            if hybrid {
                loop_state.enable_research(1)?;
            }
            loop_state.throttle = 1.;
            loop_state.burner = true;
            let nose = Basis::new(loop_state.yaw, 0., 0.).forward;
            let (mut up, mut down, mut inverted, mut complete) = (false, false, false, false);
            for _ in 0..10800 {
                loop_state.step(
                    &PilotInput {
                        pitch: 1.,
                        ..Default::default()
                    },
                    |_, _| 0.,
                );
                let b = Basis::new(loop_state.yaw, loop_state.pitch, loop_state.bank);
                up |= b.forward[1] > 0.99;
                down |= b.forward[1] < -0.99;
                inverted |= b.up[1] < -0.9;
                complete |= inverted && b.up[1] > 0.9 && dot(b.forward, nose) > 0.98;
                if complete || loop_state.crashed {
                    break;
                }
            }
            assert!(
                up && down && complete && !loop_state.crashed,
                "full loop both vertical attitudes"
            );
            println!(
                "{} hybrid={hybrid} loop ticks={} both_verticals=true",
                aircraft.name, loop_state.ticks
            );
            for scenario in [
                "level",
                "pull",
                "push",
                "turn",
                "pull-low",
                "pull-high",
                "push-low",
                "push-high",
                "devices",
                "payload",
                "roll-left",
                "roll-right",
                "rudder-left",
                "rudder-right",
                "stall",
                "spin-left",
                "spin-right",
            ] {
                let mut s = State::new(&aircraft, [0., 15000., 0.])?;
                if hybrid {
                    s.enable_research(1)?;
                }
                let mut input = PilotInput::default();
                match scenario {
                    "pull" | "turn" => input.pitch = 1.,
                    "push" => input.pitch = -1.,
                    "roll-left" => input.roll = -1.,
                    "roll-right" => input.roll = 1.,
                    "rudder-left" => input.yaw = -1.,
                    "rudder-right" => input.yaw = 1.,
                    _ => {}
                }
                if scenario.ends_with("low") || scenario.ends_with("high") {
                    s.speed = if scenario.ends_with("low") {
                        300.
                    } else {
                        1500.
                    };
                    s.position[1] = if scenario.ends_with("low") {
                        5000.
                    } else {
                        30000.
                    };
                    s.velocity = Basis::new(s.yaw, 0., 0.).forward.map(|v| v * s.speed);
                    input.pitch = if scenario.starts_with("pull") {
                        1.
                    } else {
                        -1.
                    };
                }
                if scenario == "devices" {
                    s.gear_down = true;
                    s.flaps_down = true;
                    s.brake_out = true;
                    input.pitch = 1.;
                }
                if scenario == "payload" {
                    let c = s.model().configuration();
                    s.set_payload((c.mass.max_takeoff_lbs - c.mass.empty_lbs - s.fuel) * 0.5)?;
                    input.pitch = 1.;
                }
                if scenario == "turn" {
                    s.bank = 0.7;
                }
                if scenario == "stall" || scenario.starts_with("spin") {
                    s.speed = 180.;
                    s.engine = false;
                    s.velocity = Basis::new(s.yaw, 0., 0.).forward.map(|v| v * s.speed);
                    if scenario.starts_with("spin") {
                        input.pitch = 1.;
                        input.yaw = if scenario.ends_with("left") { -1. } else { 1. };
                        s.bank = -input.yaw * 0.01;
                    }
                }
                let mut trace = if let Ok(dir) = env::var("TORE_RESPONSE_TRACE") {
                    fs::create_dir_all(&dir)?;
                    Some(BufWriter::new(fs::File::create(format!(
                        "{dir}/{}-{hybrid}-{scenario}.txt",
                        aircraft.shape
                    ))?))
                } else {
                    None
                };
                let surface = Surface::terrain(0.);
                if let Some(f) = &mut trace {
                    writeln!(
                        f,
                        "surface={surface:?} atmosphere=adapter-lapse initial={s:?}"
                    )?;
                }
                let mut replay = s.clone();
                let (mut min_g, mut max_g, mut max_roll) = (1f64, 1f64, 0f64);
                let mut modes = [false; 5];
                let mut spun = false;
                for tick in 0..3600 {
                    if tick == 1200 {
                        input = if scenario.starts_with("spin") {
                            PilotInput {
                                pitch: -1.,
                                yaw: if scenario.ends_with("left") { 1. } else { -1. },
                                ..Default::default()
                            }
                        } else {
                            PilotInput::default()
                        };
                    }
                    s.step_surface(&input, |_, _| surface);
                    replay.step_surface(&input, |_, _| surface);
                    assert_eq!(s, replay);
                    assert!(
                        s.position
                            .iter()
                            .chain(s.velocity.iter())
                            .chain([&s.g, &s.roll_rate, &s.pitch_rate])
                            .all(|v| v.is_finite())
                    );
                    min_g = min_g.min(s.g);
                    max_g = max_g.max(s.g);
                    max_roll = max_roll.max(s.roll_rate.abs());
                    if let Some(r) = &s.research {
                        modes[r.departure.mode as usize] = true;
                        spun |= r.spinning != 0;
                    }
                    if let Some(f) = &mut trace {
                        let b = Basis::new(s.yaw, s.pitch, s.bank);
                        let v = unit(s.velocity);
                        let aoa = (-dot(v, b.up)).atan2(dot(v, b.forward)).to_degrees();
                        let slip = dot(v, b.right).clamp(-1., 1.).asin().to_degrees();
                        writeln!(
                            f,
                            "tick={} input={input:?} position={:?} velocity={:?} attitude={:?} aoa_deg={aoa} slip_deg={slip} fuel={} devices={:?} response={:?} departure={:?}",
                            s.ticks,
                            s.position,
                            s.velocity,
                            [s.yaw, s.pitch, s.bank],
                            s.fuel,
                            [s.gear, s.flaps, s.brake],
                            s.maneuver,
                            s.research
                        )?;
                    }
                    if s.crashed {
                        break;
                    }
                }
                if hybrid && scenario.starts_with("spin") {
                    assert!(
                        spun && s.research.as_ref().unwrap().spinning == 0,
                        "spin/recovery {scenario}"
                    );
                }
                if hybrid && scenario == "stall" {
                    assert!(modes[1] && modes[2], "warning/stall sequence");
                }
                println!(
                    "{} hybrid={hybrid} {scenario}: G={min_g:.3}..{max_g:.3} roll_max={max_roll:.4} release_roll={:.5} speed={:.2} modes={modes:?} spun={spun} spin_now={} crashed={}",
                    aircraft.name,
                    s.roll_rate,
                    s.speed,
                    s.research.as_ref().map_or(0, |r| r.spinning),
                    s.crashed
                );
            }
        }
    }
    Ok(())
}
