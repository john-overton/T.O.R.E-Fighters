//! Probe of the single-rotor helicopters (VTOL overhaul slice P2) against
//! user-owned PT records: power, hover margin and ceilings, level top speed,
//! climb, autorotation, rotor droop and hover rates. No retail fixtures;
//! prints a table for the baseline.
//!
//! Usage: `powered_probe AH64.PT [MI24.PT ...]`
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_input::StabilityLevel;
use tore_sim::{
    flight::{
        DT, PilotCommand, PilotInput, State, Switch,
        powered::{helicopter::SingleRotor, rotor},
    },
    models::FlightModel,
    research::Surface,
};

const KT: f64 = 1.687_81;
const HP: f64 = 550.;

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

fn fly(s: &mut State, ticks: usize, mut pilot: impl FnMut(&State) -> PilotInput) {
    for _ in 0..ticks {
        let input = pilot(s);
        s.step_surface(&input, |_, _| Surface::runway(0.));
    }
}

/// Trimmed level at `height` and `airspeed` (ft/s), heading north.
fn trimmed(a: &Aircraft, height: f64, airspeed: f64, level: StabilityLevel) -> State {
    let mut s = State::new(a, [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.velocity = [0.; 3];
    s.speed = 0.;
    s.lift_controls.aids.stability = level;
    assert!(s.trim_single_rotor(airspeed), "{} trims", a.name);
    s
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// Stick and pedals for a pitch attitude, wings level and north.
fn attitude(s: &State, pitch: f64, collective: f64) -> PilotInput {
    let [p, q, r] = s.lift_controls.body_rates;
    PilotInput {
        pitch: (2.5 * (pitch - s.pitch) - 1.2 * q).clamp(-1., 1.),
        roll: (-2.5 * s.bank - 0.4 * p).clamp(-1., 1.),
        yaw: (-2. * wrap(s.yaw) - 1.5 * r).clamp(-1., 1.),
        collective: Some(collective),
        ..Default::default()
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: powered_probe AH64.PT [MI24.PT ...]".into());
    }
    for path in paths {
        let a = load(&path)?;
        let base = trimmed(&a, 0., 0., StabilityLevel::Off);
        let c = base.model().configuration();
        let lift = base.model().powered_lift().ok_or("not powered lift")?;
        let Some(h) = SingleRotor::new(&lift, c) else {
            println!("{}: not a single-rotor helicopter", a.name);
            continue;
        };
        let gross = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
        let maximum = c.mass.max_takeoff_lbs;
        let rho0 = rotor::air_density(0.);
        let available = h.available_power(rho0, 1., 1.);
        println!("== {} ({path})", a.name);
        println!(
            "weights: empty {:.0} lb, gross {gross:.0} lb, maximum {maximum:.0} lb; PT thrust {:.0} lbf",
            c.mass.empty_lbs, h.max_thrust
        );
        println!(
            "rotor: radius {:.1} ft, tip {:.0} ft/s; rated power {:.0} hp at sea level",
            h.rotor.radius_ft,
            h.rotor.tip_speed_fps,
            h.drive.rated_power / HP
        );
        let level = |weight: f64, altitude: f64, kt: f64| {
            h.trim(
                weight,
                0.,
                kt * KT,
                rotor::air_density(altitude),
                1.,
                StabilityLevel::Off,
            )
        };
        let hover = level(gross, 0., 0.).ok_or("no hover trim")?;
        println!(
            "hover OGE at gross, sea level: {:.0} hp, margin {:.0} percent, collective {:.0} percent, pitch {:+.1}, bank {:+.1} deg",
            hover.engine_power / HP,
            100. * (available / hover.engine_power - 1.),
            100. * hover.collective,
            hover.pitch.to_degrees(),
            hover.bank.to_degrees()
        );
        let ceiling = |weight: f64| {
            (0..30_000)
                .step_by(100)
                .map(f64::from)
                .take_while(|altitude| {
                    level(weight, *altitude, 0.).is_some_and(|t| {
                        t.engine_power <= h.available_power(rotor::air_density(*altitude), 1., 1.)
                    })
                })
                .last()
                .unwrap_or(0.)
        };
        println!(
            "hover ceiling OGE: {:.0} ft at gross, {:.0} ft at maximum",
            ceiling(gross),
            ceiling(maximum)
        );
        let profile = |weight: f64| {
            let mut top = 0.;
            let mut minimum = (f64::MAX, 0.);
            for kt in 0..260 {
                let kt = f64::from(kt);
                match level(weight, 0., kt) {
                    Some(t) if t.engine_power <= available => {
                        top = kt;
                        if t.engine_power < minimum.0 {
                            minimum = (t.engine_power, kt);
                        }
                    }
                    _ => break,
                }
            }
            (top, minimum)
        };
        let (top, minimum) = profile(gross);
        println!(
            "level top speed at sea level, gross: {top:.0} kt; least power {:.0} hp at {:.0} kt",
            minimum.0 / HP,
            minimum.1
        );
        println!(
            "best climb (excess power over weight): {:.0} ft/min",
            (available - minimum.0) / gross * 60.
        );
        // Published reference weights: the AH-64A maximum takeoff weight
        // (17,650 lb, Aerospaceweb), the Mi-24D and V normal and maximum
        // takeoff weights (24,250 and 26,455 lb, Aerospaceweb).
        let published: &[f64] = if a.name.contains("AH-64") {
            &[17_650.]
        } else {
            &[24_250., 26_455.]
        };
        for weight in published {
            let (top, minimum) = profile(*weight);
            let hover = level(*weight, 0., 0.).map(|t| t.engine_power);
            let above = ceiling(*weight) + 100.;
            let why = match level(*weight, above, 0.) {
                None => "no trim within the controls' travel".to_string(),
                Some(t) => format!(
                    "needs {:.0} hp, has {:.0} hp",
                    t.engine_power / HP,
                    h.available_power(rotor::air_density(above), 1., 1.) / HP
                ),
            };
            println!("  100 ft above it: {why}");
            println!(
                "at {weight:.0} lb: hover OGE ceiling {:.0} ft, hover margin {:.0} percent at sea level, top speed {top:.0} kt, best climb {:.0} ft/min",
                ceiling(*weight),
                hover.map_or(f64::NAN, |p| 100. * (available / p - 1.)),
                (available - minimum.0) / weight * 60.
            );
        }
        // Full collective from a hover, level attitude.
        let mut climb = trimmed(&a, 1_000., 0., StabilityLevel::Damper);
        fly(&mut climb, 120 * 20, |s| attitude(s, 0., 1.));
        println!(
            "vertical climb at full collective: {:.0} ft/min, rotor {:.0} percent",
            climb.vertical_speed * 60.,
            100. * climb.lift_controls.drive.rotor_speed
        );
        // Engine cut in a hover, collective held.
        let mut cut = trimmed(&a, 3_000., 0., StabilityLevel::Off);
        let lever = cut.lift_controls.collective;
        cut.command(PilotCommand::Set(Switch::Engine, false));
        let mut droop = None;
        for tick in 0..120 * 5 {
            fly(&mut cut, 1, |_| PilotInput {
                collective: Some(lever),
                ..Default::default()
            });
            if droop.is_none() && cut.lift_controls.drive.rotor_speed < 0.8 {
                droop = Some((tick + 1) as f64 * DT);
            }
        }
        println!(
            "engine cut in the hover, collective held: rotor below 80 percent after {}",
            droop.map_or("never".into(), |t| format!("{t:.2} s"))
        );
        // Autorotation from 80 kt, collective managing the rotor speed,
        // airspeed held near 75 kt.
        let mut auto = trimmed(&a, 3_000., 80. * KT, StabilityLevel::Damper);
        let mut pitch = auto.pitch;
        auto.command(PilotCommand::Set(Switch::Engine, false));
        let mut lowest = f64::MAX;
        for tick in 0..120 * 30 {
            let nr = auto.lift_controls.drive.rotor_speed;
            lowest = lowest.min(nr);
            let lever = if tick > 120 {
                (4. * (nr - 1.)).clamp(0., 0.6)
            } else {
                0.
            };
            pitch = (pitch + DT * 0.002 * (auto.speed - 75. * KT)).clamp(-0.3, 0.3);
            let target = (pitch + 0.004 * (auto.speed - 75. * KT)).clamp(-0.3, 0.3);
            fly(&mut auto, 1, |s| attitude(s, target, lever));
        }
        println!(
            "autorotation: {:.0} ft/min at {:.0} kt, rotor {:.0} percent (lowest {:.0})",
            -auto.vertical_speed * 60.,
            auto.speed / KT,
            100. * auto.lift_controls.drive.rotor_speed,
            100. * lowest
        );
        // Full-stick hover rates after 2 s, Off and Damper.
        for level in [StabilityLevel::Off, StabilityLevel::Damper] {
            let rates: Vec<_> = (0..3)
                .map(|axis| {
                    let mut s = trimmed(&a, 3_000., 0., level);
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
                })
                .collect();
            println!(
                "full stick 2 s from a hover at {level:?}: pitch {:.0}, roll {:.0}, yaw {:.0} deg/s",
                rates[0], rates[1], rates[2]
            );
        }
        // Torque: +30 percent collective with the pedals fixed.
        for level in [StabilityLevel::Off, StabilityLevel::Damper] {
            let mut s = trimmed(&a, 3_000., 0., level);
            let lever = s.lift_controls.collective + 0.3;
            let mut fastest: f64 = 0.;
            fly(&mut s, 240, |s| {
                fastest = if s.lift_controls.body_rates[2].abs() > fastest.abs() {
                    s.lift_controls.body_rates[2]
                } else {
                    fastest
                };
                PilotInput {
                    collective: Some(lever),
                    ..Default::default()
                }
            });
            println!(
                "collective +30 percent, pedals fixed, at {level:?}: yaw {:+.1} deg/s (+ is nose right)",
                fastest.to_degrees()
            );
        }
    }
    Ok(())
}
