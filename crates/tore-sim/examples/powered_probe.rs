//! Probe of the helicopters (VTOL overhaul slices P2 and P3: the single-rotor
//! AH-64 and Mi-24 and the tandem CH-47) and the V-22 tiltrotor (slice P5)
//! against user-owned PT records: power, hover margin and ceilings, level top
//! speed, climb, autorotation, rotor droop and hover rates; for the V-22 also
//! the conversion, airplane-mode top speed and stall, and corridor
//! protection. No retail fixtures; prints a table for the baseline.
//!
//! Usage: `powered_probe AH64.PT [MI24.PT CH47.PT V22.PT ...]`
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_input::StabilityLevel;
use tore_sim::{
    flight::{
        DT, PilotCommand, PilotInput, State, Switch,
        powered::{
            helicopter::SingleRotor,
            rotor,
            tandem::Tandem,
            tiltrotor::{self, Tiltrotor},
        },
    },
    models::FlightModel,
    research::Surface,
};

const KT: f64 = 1.687_81;
const HP: f64 = 550.;

/// A level trim, as the probe reports it.
struct LevelTrim {
    engine_power: f64,
    collective: f64,
    pitch: f64,
    bank: f64,
}

/// The helicopter model the probe flies: one main rotor or the tandem pair.
enum Craft {
    Single(SingleRotor),
    Tandem(Tandem),
}

impl Craft {
    fn new(
        lift: &tore_sim::models::variety::PoweredLift,
        c: &tore_sim::models::config::Configuration,
    ) -> Option<Self> {
        SingleRotor::new(lift, c)
            .map(Self::Single)
            .or_else(|| Tandem::new(lift, c).map(Self::Tandem))
    }
    fn rated_power(&self) -> f64 {
        match self {
            Self::Single(h) => h.drive.rated_power,
            Self::Tandem(h) => h.drive.rated_power,
        }
    }
    fn max_thrust(&self) -> f64 {
        match self {
            Self::Single(h) => h.max_thrust,
            Self::Tandem(h) => h.max_thrust,
        }
    }
    fn radius_ft(&self) -> f64 {
        match self {
            Self::Single(h) => h.rotor.radius_ft,
            Self::Tandem(h) => h.rotors[0].radius_ft,
        }
    }
    fn tip_speed_fps(&self) -> f64 {
        match self {
            Self::Single(h) => h.rotor.tip_speed_fps,
            Self::Tandem(h) => h.rotors[0].tip_speed_fps,
        }
    }
    fn available_power(&self, density: f64) -> f64 {
        match self {
            Self::Single(h) => h.available_power(density, 1., 1.),
            Self::Tandem(h) => h.available_power(density, 1., 1.),
        }
    }
    /// The level trim at `kt` (cruise speeds use the cruise level).
    fn trim(&self, weight: f64, density: f64, kt: f64, level: StabilityLevel) -> Option<LevelTrim> {
        let level = if kt > 0. { self.cruise_level() } else { level };
        match self {
            Self::Single(h) => h
                .trim(weight, 0., kt * KT, density, 1., level)
                .map(|t| LevelTrim {
                    engine_power: t.engine_power,
                    collective: t.collective,
                    pitch: t.pitch,
                    bank: t.bank,
                }),
            Self::Tandem(h) => h
                .trim(weight, 0., kt * KT, density, 1., level)
                .map(|t| LevelTrim {
                    engine_power: t.engine_power,
                    collective: t.collective,
                    pitch: t.pitch,
                    bank: t.bank,
                }),
        }
    }
    /// The stability level a cruise is trimmed at: the tandem's longitudinal
    /// trim schedule belongs to Damper.
    fn cruise_level(&self) -> StabilityLevel {
        match self {
            Self::Single(_) => StabilityLevel::Off,
            Self::Tandem(_) => StabilityLevel::Damper,
        }
    }
}

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
    let trimmed = s.trim_single_rotor(airspeed) || s.trim_tandem(airspeed);
    assert!(trimmed, "{} trims", a.name);
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
        return Err("usage: powered_probe AH64.PT [MI24.PT CH47.PT V22.PT ...]".into());
    }
    for path in paths {
        let a = load(&path)?;
        if tiltrotor_probe(&a, &path)? {
            continue;
        }
        let base = trimmed(&a, 0., 0., StabilityLevel::Off);
        let c = base.model().configuration();
        let lift = base.model().powered_lift().ok_or("not powered lift")?;
        let Some(h) = Craft::new(&lift, c) else {
            println!("{}: not a helicopter", a.name);
            continue;
        };
        let gross = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
        let maximum = c.mass.max_takeoff_lbs;
        let rho0 = rotor::air_density(0.);
        let available = h.available_power(rho0);
        println!("== {} ({path})", a.name);
        println!(
            "weights: empty {:.0} lb, gross {gross:.0} lb, maximum {maximum:.0} lb; model maximum rotor thrust {:.0} lbf",
            c.mass.empty_lbs,
            h.max_thrust()
        );
        println!(
            "rotor: radius {:.1} ft, tip {:.0} ft/s; rated power {:.0} hp at sea level",
            h.radius_ft(),
            h.tip_speed_fps(),
            h.rated_power() / HP
        );
        let level = |weight: f64, altitude: f64, kt: f64| {
            h.trim(
                weight,
                rotor::air_density(altitude),
                kt,
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
                        t.engine_power <= h.available_power(rotor::air_density(*altitude))
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
        } else if a.name.contains("47") {
            // The CH-47's PT weights are 53 percent of the real aircraft's;
            // its maximum is printed above.
            &[]
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
                    h.available_power(rotor::air_density(above)) / HP
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

/// A hybrid flight of `a` at `height`, north, gear up, at `level`, not yet
/// trimmed.
fn untrimmed(a: &Aircraft, height: f64, level: StabilityLevel) -> State {
    let mut s = State::new(a, [0., height, 0.]).unwrap();
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

/// `x²(3 - 2x)` on `x` clamped to 0..1.
fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

/// True airspeed, ft/s, of `kcas` at `height`.
fn true_airspeed(kcas: f64, height: f64) -> f64 {
    kcas * KT / (rotor::air_density(height) / rotor::sea_level_density()).sqrt()
}

fn indicated(s: &State) -> f64 {
    tiltrotor::kcas(s.speed, rotor::air_density(s.position[1]))
}

/// The pitch attitude that holds `height`, about `trim`, rad.
fn height_hold(s: &State, height: f64, trim: f64) -> f64 {
    (trim + 0.003 * (height - s.position[1]) - 0.004 * s.vertical_speed).clamp(-0.2, 0.25)
}

/// Stick and pedals for a pitch attitude (with `trim` added), wings level
/// and north, the lever at `lever` and the nacelle demand.
fn tilt_pilot(
    s: &State,
    pitch: f64,
    trim: f64,
    lever: f64,
    conversion: Option<f64>,
    rate: f64,
) -> PilotInput {
    let mut input = attitude(s, pitch, lever);
    input.pitch = (input.pitch + trim).clamp(-1., 1.);
    input.conversion = conversion;
    input.conversion_rate = rate;
    input
}

/// The V-22's rows of the probe; false for any other aircraft.
fn tiltrotor_probe(a: &Aircraft, path: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let probe = untrimmed(a, 0., StabilityLevel::Damper);
    let c = probe.model().configuration();
    let Some(t) = probe
        .model()
        .powered_lift()
        .and_then(|lift| Tiltrotor::new(&lift, c))
    else {
        return Ok(false);
    };
    let gross = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
    let maximum = c.mass.max_takeoff_lbs;
    let preset = t.tilt.helicopter_nacelle_degrees;
    println!("== {} ({path})", a.name);
    println!(
        "weights: empty {:.0} lb, gross {gross:.0} lb, maximum {maximum:.0} lb; PT thrust {:.0} lbf",
        c.mass.empty_lbs, t.max_thrust
    );
    println!(
        "rotors: 2 x radius {:.2} ft, tip {:.0} ft/s at 100 percent; rated power {:.0} hp at sea level",
        t.rotors[0].radius_ft,
        t.rotors[0].tip_speed_fps,
        t.drive.rated_power / HP
    );
    // Hover margin and ceilings, out of ground effect at the preset.
    let available = |altitude: f64| {
        t.drive
            .available_power(rotor::air_density(altitude), 1., 1.)
    };
    let hover = |weight: f64, altitude: f64| {
        t.trim(
            rotor::Hazards::ALL,
            weight,
            0.,
            0.,
            preset.to_radians(),
            altitude,
            1.,
            StabilityLevel::Off,
            available(altitude),
        )
    };
    let at_gross = hover(gross, 0.).ok_or("no hover trim")?;
    println!(
        "hover OGE at gross, sea level, nacelles {preset:.0}: {:.0} hp, margin {:.0} percent, lever {:.0} percent, pitch {:+.1} deg",
        at_gross.engine_power / HP,
        100. * (available(0.) / at_gross.engine_power - 1.),
        100. * at_gross.lever,
        at_gross.pitch.to_degrees()
    );
    let ceiling = |weight: f64| {
        (0..30_000)
            .step_by(100)
            .map(f64::from)
            .take_while(|altitude| hover(weight, *altitude).is_some())
            .last()
            .unwrap_or(0.)
    };
    println!(
        "hover ceiling OGE: {:.0} ft at gross, {:.0} ft at maximum",
        ceiling(gross),
        ceiling(maximum)
    );
    // Hover rates and the engine cut.
    for level in [StabilityLevel::Off, StabilityLevel::Damper] {
        let rates: Vec<_> = (0..3)
            .map(|axis| {
                let mut s = untrimmed(a, 3_000., level);
                assert!(s.trim_tiltrotor(0., preset));
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
    let mut cut = untrimmed(a, 3_000., StabilityLevel::Off);
    assert!(cut.trim_tiltrotor(0., preset));
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
        "engine cut in the hover, lever held: rotor below 80 percent after {}",
        droop.map_or("never".into(), |t| format!("{t:.2} s"))
    );
    // The conversion: keys held from a 1,000 ft hover, lever at 85 percent.
    let mut s = untrimmed(a, 1_000., StabilityLevel::Damper);
    assert!(s.trim_tiltrotor(0., preset));
    let (mut downstops, mut fast) = (None, None);
    let (mut lowest, mut highest, mut outside, mut slowest_rotor) =
        (f64::MAX, f64::MIN, 0_f64, f64::MAX);
    let mut trim = 0.;
    for tick in 0..120 * 40 {
        fly(&mut s, 1, |s| {
            let early = smoothstep((s.nacelle_degrees() - 60.) / 20.);
            let pitch = height_hold(s, 1_000., 0.03) * (1. - early) - 0.15 * early;
            trim = (trim + DT * 1.5 * wrap(pitch - s.pitch)).clamp(-1., 1.);
            tilt_pilot(s, pitch, trim, 0.85, None, -1.)
        });
        let corridor = s.conversion_corridor().unwrap();
        let time = (tick + 1) as f64 * DT;
        lowest = lowest.min(s.position[1]);
        highest = highest.max(s.position[1]);
        outside = outside
            .max(corridor.limits_kcas[0] - corridor.kcas)
            .max(corridor.kcas - corridor.limits_kcas[1]);
        slowest_rotor = slowest_rotor
            .min(s.lift_controls.drive.rotor_speed / s.lift_controls.drive.rotor_speed_reference);
        if downstops.is_none() && corridor.nacelle_degrees < 0.5 {
            downstops = Some(time);
        }
        if fast.is_none() && corridor.kcas >= 200. {
            fast = Some(time);
        }
    }
    let seconds = |t: Option<f64>| t.map_or("never".into(), |t| format!("{t:.1} s"));
    println!(
        "conversion from a 1,000 ft hover, keys held, lever 85 percent, Damper: downstops {}, 200 KCAS {}, height {:+.0} to {:+.0} ft, worst {:.1} kt outside the corridor, rotor speed at least {:.0} percent of its reference",
        seconds(downstops),
        seconds(fast),
        lowest - 1_000.,
        highest - 1_000.,
        outside,
        100. * slowest_rotor
    );
    // Airplane mode: top speed and stall at sea level, gross weight.
    let trims = |kt: f64| {
        let mut s = probe.clone();
        s.trim_tiltrotor(kt * KT, 0.).then_some(s)
    };
    let top = (150..330)
        .map(f64::from)
        .take_while(|kt| trims(*kt).is_some())
        .last()
        .unwrap_or(0.);
    let stall = (60..200)
        .map(f64::from)
        .find(|kt| trims(*kt).is_some_and(|s| s.pitch < 10_f64.to_radians()))
        .unwrap_or(0.);
    println!(
        "airplane mode at sea level, gross: level top speed {top:.0} kt; slowest wingborne level flight (nose under 10 degrees) {stall:.0} kt"
    );
    // The stall warning, decelerating at idle power holding 3,000 ft.
    let mut slowing = untrimmed(a, 3_000., StabilityLevel::Damper);
    assert!(slowing.trim_tiltrotor(true_airspeed(150., 3_000.), 0.));
    let mut warned = None;
    let mut trim = 0.;
    for _ in 0..120 * 40 {
        fly(&mut slowing, 1, |s| {
            let pitch = height_hold(s, 3_000., 0.05);
            trim = (trim + DT * 1.5 * wrap(pitch - s.pitch)).clamp(-1., 1.);
            tilt_pilot(s, pitch, trim, 0., None, 0.)
        });
        if warned.is_none() && slowing.stall_alert(0.).is_some() {
            warned = Some(indicated(&slowing));
        }
    }
    println!(
        "stall warning decelerating on the downstops at idle: {}",
        warned.map_or("none".into(), |k| format!("{k:.0} KCAS"))
    );
    // Corridor protection.
    let mut aft = untrimmed(a, 3_000., StabilityLevel::Damper);
    assert!(aft.trim_tiltrotor(true_airspeed(180., 3_000.), 0.));
    let lever = aft.lift_controls.collective;
    fly(&mut aft, 120 * 10, |s| {
        tilt_pilot(
            s,
            height_hold(s, 3_000., 0.02),
            0.,
            lever,
            Some(preset / 97.5),
            0.,
        )
    });
    let mut locked = untrimmed(a, 3_000., StabilityLevel::Damper);
    assert!(locked.trim_tiltrotor(true_airspeed(210., 3_000.), 0.));
    let lever = locked.lift_controls.collective;
    fly(&mut locked, 120 * 5, |s| {
        tilt_pilot(
            s,
            height_hold(s, 3_000., 0.02),
            0.,
            lever,
            Some(preset / 97.5),
            0.,
        )
    });
    println!(
        "protection, helicopter preset asked in airplane mode: from 180 KCAS the nacelles stop at {:.1} deg ({:.0} KCAS); from 210 KCAS at {:.1} deg",
        aft.nacelle_degrees(),
        indicated(&aft),
        locked.nacelle_degrees()
    );
    let mut over = untrimmed(a, 3_000., StabilityLevel::Damper);
    assert!(over.trim_tiltrotor(true_airspeed(100., 3_000.), 80.));
    let lever = over.lift_controls.collective;
    let tas = true_airspeed(140., 3_000.);
    over.velocity = over.velocity.map(|v| v * tas / over.speed);
    over.speed = tas;
    let (mut moved, mut inside) = (None, None);
    for tick in 0..120 * 6 {
        fly(&mut over, 1, |s| {
            tilt_pilot(s, height_hold(s, 3_000., 0.), 0., lever, None, 0.)
        });
        let corridor = over.conversion_corridor().unwrap();
        let time = (tick + 1) as f64 * DT;
        if moved.is_none() && corridor.nacelle_degrees < 79.5 {
            moved = Some(time);
        }
        if inside.is_none() && corridor.kcas <= corridor.limits_kcas[1] {
            inside = Some(time);
        }
    }
    println!(
        "protection, 140 KCAS at 80 deg (edge 130): nacelles moving forward after {}, inside after {}",
        seconds(moved),
        seconds(inside)
    );
    let mut slow = untrimmed(a, 3_000., StabilityLevel::Damper);
    assert!(slow.trim_tiltrotor(true_airspeed(40., 3_000.), preset));
    let lever = slow.lift_controls.collective;
    fly(&mut slow, 120 * 5, |s| {
        tilt_pilot(s, 0.02, 0., lever, Some(0.), 0.)
    });
    let held = slow.conversion_corridor().unwrap();
    println!(
        "protection, nacelles forward asked at 40 KCAS: held at {:.1} deg at {:.0} KCAS (lower edge {:.1} deg)",
        held.nacelle_degrees, held.kcas, held.allowed_degrees[0]
    );
    Ok(true)
}
