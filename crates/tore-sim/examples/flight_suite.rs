//! Repeatable aircraft-agnostic flight acceptance using user-extracted PT data.
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    attitude::{Basis, dot, unit},
    flight::{PilotInput, State},
    models::{AircraftModel, FlightModel},
    research::Surface,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: flight_suite EXTRACTED.PT [SECOND.PT ...]".into());
    }
    for path in paths {
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("PT size exceeds limit".into());
        }
        let a = Aircraft::parse(&bytes)?;
        println!(
            "aircraft={} empty={} fuel={} thrust={} afterburner={} spin_entry={} spin_exit={}",
            a.name,
            a.number("weight"),
            a.number("internalFuel"),
            a.number("thrust"),
            a.number("aftThrust"),
            a.number("spinEntry"),
            a.number("spinExit")
        );
        let mut level_result: Option<State> = None;
        let mut left_offset = 0.;
        for scenario in [
            "level",
            "loop",
            "bank-left",
            "bank-right",
            "stall",
            "spin",
            "landing",
            "gear-up",
            "taxi",
            "takeoff",
            "wind",
            "water",
            "hard-landing",
        ] {
            let mut s = State::new(&a, [0., 15000., 0.])?;
            s.enable_research(1)?;
            let mut keys = PilotInput::default();
            match scenario {
                "loop" => {
                    s.throttle = 1.;
                    s.burner = true;
                    keys.pitch = 1.;
                }
                "bank-left" => {
                    s.bank = -0.7;
                    keys.pitch = 1.;
                }
                "bank-right" => {
                    s.bank = 0.7;
                    keys.pitch = 1.;
                }
                "stall" | "spin" => {
                    s.speed = 180.;
                    s.engine = false;
                    s.velocity = Basis::new(s.yaw, 0., 0.).forward.map(|v| v * s.speed);
                    if scenario == "spin" {
                        keys.pitch = 1.;
                        keys.yaw = 1.;
                    }
                }
                "landing" | "gear-up" | "taxi" | "takeoff" | "water" | "hard-landing" => {
                    s.position[1] = AircraftModel::for_aircraft(&a)?
                        .configuration()
                        .equipment
                        .ground_clearance_ft;
                    s.yaw = 0.;
                    s.pitch = 0.;
                    s.gear = 1.;
                    s.gear_down = true;
                    s.speed = 100.;
                    s.velocity = [0., -1., 100.];
                    s.throttle = 0.;
                    if scenario == "landing" {
                        s.position[1] = 30.;
                        s.speed = 280.;
                        s.velocity = [0., -5., 280.];
                    }
                    if scenario == "hard-landing" {
                        s.velocity[1] = -150.;
                    }
                    if scenario == "gear-up" {
                        s.gear = 0.;
                        s.gear_down = false;
                    }
                    if scenario == "taxi" {
                        s.brake_out = true;
                    }
                    if scenario == "takeoff" {
                        s.throttle = 1.;
                        s.burner = true;
                        keys.pitch = 1.;
                    }
                }
                _ => {}
            }
            let mut surface = Surface::runway(0.);
            if scenario == "water" {
                surface.water = true;
            }
            if scenario == "wind" {
                surface.wind = [40., 0., 0.];
                s.velocity[0] += 40.;
            }
            let initial = s.clone();
            let mut replay = s.clone();
            let nose = Basis::new(s.yaw, s.pitch, s.bank).forward;
            let (mut vertical, mut inverted, mut looped, mut departed, mut spun) =
                (false, false, false, false, false);
            for tick in 0..120 * 90 {
                if scenario == "spin" && tick == 120 * 12 {
                    keys.pitch = 0.;
                    keys.yaw = 0.;
                    keys.pitch = -1.;
                    keys.yaw = -1.;
                }
                if scenario == "takeoff" {
                    keys.pitch = 0.;
                    keys.pitch = 0.;
                    if s.speed > 240. && s.pitch.to_degrees() < 8. {
                        keys.pitch = 1.;
                    }
                    if s.pitch.to_degrees() > 12. {
                        keys.pitch = -1.;
                    }
                    if s.position[1] > 500. {
                        s.gear_down = false;
                        replay.gear_down = false;
                    }
                }
                s.step_surface(&keys, |_, _| surface);
                replay.step_surface(&keys, |_, _| surface);
                assert_eq!(s, replay, "deterministic replay {scenario}");
                assert!(
                    s.position
                        .iter()
                        .chain(s.velocity.iter())
                        .all(|v| v.is_finite()),
                    "nonfinite {scenario}"
                );
                let b = Basis::new(s.yaw, s.pitch, s.bank);
                vertical |= b.forward[1] > 0.99;
                inverted |= b.up[1] < -0.9;
                looped |= inverted && b.up[1] > 0.9 && dot(b.forward, nose) > 0.98;
                let r = s.research.as_ref().unwrap();
                departed |= r.departure.mode
                    != tore_formats::flight_model::departure::DepartureMode::Normal;
                spun |= r.spinning != 0;
                if (scenario == "loop" && looped) || s.crashed {
                    break;
                }
            }
            println!(
                "scenario={scenario} ticks={} speed={:.2} altitude={:.2} crashed={} ground={} departure={} spin_seen={} spin_now={} loop={} vector_offset={:.4}",
                s.ticks,
                s.speed,
                s.position[1],
                s.crashed,
                s.research.as_ref().unwrap().on_ground,
                departed,
                spun,
                s.research.as_ref().unwrap().spinning,
                looped,
                1. - dot(Basis::new(s.yaw, s.pitch, s.bank).forward, unit(s.velocity))
            );
            if scenario == "level" {
                level_result = Some(s.clone());
            }
            if scenario == "bank-left" {
                left_offset = dot(s.position, Basis::new(initial.yaw, 0., 0.).right);
            }
            if scenario == "bank-right" {
                assert!(
                    (left_offset + dot(s.position, Basis::new(initial.yaw, 0., 0.).right)).abs()
                        < 1e-6,
                    "bank symmetry"
                );
            }
            if scenario == "wind" {
                let calm = level_result.as_ref().unwrap();
                assert!((s.speed - calm.speed).abs() < 1e-7);
                assert!((s.position[0] - calm.position[0] - 3600.).abs() < 1e-6);
            }
            match scenario {
                "level" => assert!(!s.crashed && s.fuel < initial.fuel),
                "loop" => assert!(vertical && looped && !s.crashed, "loop acceptance"),
                "bank-left" | "bank-right" => assert!(
                    !s.crashed
                        && dot(Basis::new(s.yaw, s.pitch, s.bank).forward, unit(s.velocity))
                            < 0.9999,
                    "banked AoA"
                ),
                "stall" => assert!(departed, "stall acceptance"),
                "spin" if a.number("spinEntry") == 2. => {
                    assert!(!spun, "source disables spin entry")
                }
                "spin" => assert!(
                    spun && s.research.as_ref().unwrap().spinning == 0,
                    "spin entry/recovery acceptance"
                ),
                "landing" | "taxi" => assert!(
                    !s.crashed && s.research.as_ref().unwrap().on_ground,
                    "ground acceptance"
                ),
                "gear-up" | "water" | "hard-landing" => assert!(s.crashed),
                "takeoff" => assert!(!s.crashed && s.position[1] > 100., "takeoff acceptance"),
                _ => {}
            }
        }
    }
    Ok(())
}
