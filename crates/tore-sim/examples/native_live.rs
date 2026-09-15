//! Explicit user-owned tables/PT runtime probe; no renderer or retail execution.
use std::{env, fs::File, io::Read, sync::Arc};
use tore_sim::{
    attitude::Basis,
    flight::{PilotInput, State},
    native::Tables,
    research::Surface,
};
fn read(path: &str, limit: u64) -> std::io::Result<Vec<u8>> {
    let mut b = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return Err(std::io::Error::other("input too large"));
    }
    Ok(b)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("native_live SINE ATAN PT [PT]".into());
    }
    let tables = Arc::new(Tables::parse(
        &read(&args[0], 642)?,
        &read(&args[1], 1028)?,
    )?);
    for path in &args[2..] {
        let a = tore_formats::aircraft::Aircraft::parse(&read(path, 1024 * 1024)?)?;
        for case in [
            "level",
            "pull",
            "push",
            "roll-left",
            "roll-right",
            "rudder-left",
            "rudder-right",
            "stall",
            "spin-left",
            "spin-right",
            "vertical-up",
            "vertical-down",
            "wind",
            "devices",
        ] {
            let mut s = State::new(&a, [0., 15000., 0.])?;
            let spin = case.starts_with("spin");
            let sign = if case.ends_with("left") { -1. } else { 1. };
            s.speed = if case == "stall" {
                100.
            } else if spin {
                180.
            } else {
                500.
            };
            if spin {
                s.bank = (sign * 5f64).to_radians();
                s.throttle = 0.;
            }
            s.pitch = match case {
                "vertical-up" => 89f64.to_radians(),
                "vertical-down" => -89f64.to_radians(),
                _ => 0.,
            };
            let wind = if case == "wind" {
                [20., 0., 0.]
            } else {
                [0.; 3]
            };
            s.velocity = std::array::from_fn(|j| {
                Basis::new(s.yaw, s.pitch, s.bank).forward[j] * s.speed + wind[j]
            });
            s.gear_down = case == "devices";
            s.flaps_down = case == "devices";
            s.brake_out = case == "devices";
            s.enable_native(tables.clone(), 1)?;
            let start = s.clone();
            let mut saw_departure = false;
            let mut saw_spin = false;
            let mut recovered_spin = false;
            let surface = |_, _| Surface {
                wind,
                ..Surface::terrain(0.)
            };
            for tick in 0..1200 {
                let p = if spin {
                    PilotInput {
                        pitch: if tick < 600 { 1. } else { -1. },
                        yaw: if tick < 600 { sign } else { -sign },
                        ..Default::default()
                    }
                } else if tick < 600 {
                    PilotInput {
                        pitch: match case {
                            "pull" | "vertical-up" => 1.,
                            "push" | "vertical-down" => -1.,
                            _ => 0.,
                        },
                        roll: match case {
                            "roll-left" => -1.,
                            "roll-right" => 1.,
                            _ => 0.,
                        },
                        yaw: match case {
                            "rudder-left" => -1.,
                            "rudder-right" => 1.,
                            _ => 0.,
                        },
                        ..Default::default()
                    }
                } else {
                    PilotInput::default()
                };
                let mut replay = s.clone();
                s.step_surface(&p, surface);
                replay.step_surface(&p, surface);
                assert_eq!(s, replay, "replay {case}/{tick}");
                if let Some(error) = s.native_fault() {
                    return Err(format!("{} {case}: {error}", a.name).into());
                }
                saw_spin |= s.maneuver.departure
                    == Some(tore_formats::flight_model::departure::DepartureMode::Spinning);
                recovered_spin |= s
                    .native
                    .as_ref()
                    .unwrap()
                    .events
                    .unwrap()
                    .departure
                    .recovered_spin;
                saw_departure |= s.maneuver.departure.is_some_and(|d| {
                    d != tore_formats::flight_model::departure::DepartureMode::Normal
                });
                assert!(
                    s.position
                        .iter()
                        .chain(s.velocity.iter())
                        .all(|v| v.is_finite())
                );
            }
            if spin {
                assert!(
                    saw_spin && recovered_spin,
                    "{} {case}: spin={saw_spin} recovery={recovered_spin}",
                    a.name
                );
            }
            assert_eq!(s.native.as_ref().unwrap().elapsed, 2560);
            assert_eq!(s.ticks, 1200);
            // Restart deterministically returns to the same first update.
            let mut first = start.clone();
            let mut restarted = start;
            first.step_surface(&PilotInput::default(), surface);
            restarted.enable_native(tables.clone(), 1)?;
            restarted.step_surface(&PilotInput::default(), surface);
            assert_eq!(first, restarted);
            println!(
                "PASS {} {case}: pitch={:.2} bank={:.2} speed={:.2} departure={saw_departure}",
                a.name,
                s.pitch.to_degrees(),
                s.bank.to_degrees(),
                s.speed
            );
        }
    }
    Ok(())
}
