//! Performance probe for user-extracted PT data: level acceleration dry and
//! with afterburner, climb, sustained and instantaneous turns, and fuel burn,
//! all headless against the hybrid flight model. Development harness; no retail
//! fixtures. Written for M2K-8 (docs/baselines/variety-flight.md).
//!
//! `PROBE_PAYLOAD=pounds` adds carried store mass, `PROBE_FUEL=pounds` sets the
//! internal fuel, `PROBE_ALTS=a,b,c` picks the altitudes, `PROBE_ONLY` runs one
//! section (`accel`, `climb`, `turn`, `fuel`).
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    attitude::Basis,
    flight::{PilotInput, State},
    models::FlightModel,
    research::Surface,
    telemetry::Atmosphere,
};

const KT: f64 = 1.68781;
const TICKS: usize = 120;

fn ground(_: f64, _: f64) -> Surface {
    Surface::runway(-60_000.)
}

fn sound_kt(altitude: f64) -> f64 {
    let t = Atmosphere::standard(altitude).map_or(288.15, |a| a.temperature_k);
    (1.4 * 287.053 * t).sqrt() / 0.514_444
}

/// One level-acceleration run.
struct Acceleration {
    times: Vec<Option<f64>>,
    settled_kt: f64,
    drift_ft: f64,
    peak_ps: f64,
}

/// One banked level turn.
struct Turn {
    speed_kt: f64,
    g: f64,
    drift_ft: f64,
    rate_dps: f64,
    flew: bool,
}

struct Setup {
    payload: f64,
    fuel: Option<f64>,
    /// `PROBE_CLIMB=KEAS,MACH`: the bomber climb schedule (B2-8).
    climb: Option<(f64, f64)>,
}

fn start(
    a: &Aircraft,
    set: &Setup,
    altitude: f64,
    speed_kt: f64,
    burner: bool,
) -> Result<State, Box<dyn std::error::Error>> {
    let mut s = State::new(a, [0., altitude, 0.])?;
    s.enable_research(1)?;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = speed_kt * KT;
    s.velocity = Basis::new(0., 0., 0.).forward.map(|v| v * s.speed);
    s.throttle = 1.;
    s.burner = burner;
    s.payload_lbs = set.payload;
    if let Some(f) = set.fuel {
        s.fuel = f;
    }
    // Fuel burn would change the weight during a run; the fuel section
    // measures burn separately with the real tank.
    s.cheats.unlimited_fuel = true;
    Ok(s)
}

/// Altitude-hold stick (the envelope probe's rule).
fn hold_altitude(s: &State, altitude: f64) -> f64 {
    (0.004 * (altitude - s.position[1]) - 0.04 * s.velocity[1] - 0.5 * s.pitch_rate).clamp(-1., 1.)
}

/// Level acceleration at `altitude` from `from_kt`, burner or military power.
/// Returns (time to each milestone, settled speed in kt, largest altitude drift
/// in ft, best specific excess power in ft/s).
fn accelerate(
    a: &Aircraft,
    set: &Setup,
    altitude: f64,
    from_kt: f64,
    burner: bool,
    milestones: &[f64],
) -> Result<Acceleration, Box<dyn std::error::Error>> {
    let mut s = start(a, set, altitude, from_kt, burner)?;
    let mut times = vec![None; milestones.len()];
    let mut worst = 0_f64;
    let mut ps_best = 0_f64;
    let mut last = s.speed;
    let mut prev = s.speed;
    for second in 0..900_usize {
        for _ in 0..TICKS {
            let input = PilotInput {
                pitch: hold_altitude(&s, altitude),
                roll: (-2. * s.bank).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            s.step_surface(&input, ground);
            if s.crashed {
                return Ok(Acceleration {
                    times,
                    settled_kt: s.speed / KT,
                    drift_ft: worst,
                    peak_ps: ps_best,
                });
            }
            worst = worst.max((s.position[1] - altitude).abs());
        }
        let t = (second + 1) as f64;
        for (i, m) in milestones.iter().enumerate() {
            if times[i].is_none() && s.speed / KT >= *m {
                times[i] = Some(t);
            }
        }
        // Specific excess power over the last 5 seconds: V dV/dt / g.
        if (second + 1) % 5 == 0 {
            let dv = (s.speed - prev) / 5.;
            ps_best = ps_best.max((s.speed * dv / 32.174).max(0.));
            prev = s.speed;
        }
        if second > 60 && (s.speed - last).abs() < 0.01 {
            break;
        }
        last = s.speed;
    }
    Ok(Acceleration {
        times,
        settled_kt: s.speed / KT,
        drift_ft: worst,
        peak_ps: ps_best,
    })
}

fn cell(t: Option<f64>) -> String {
    t.map_or("   -".into(), |t| format!("{t:4.0}"))
}

fn accel_section(
    a: &Aircraft,
    set: &Setup,
    altitudes: &[f64],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("-- level acceleration (seconds from the start speed to each speed, kt true)");
    for &alt in altitudes {
        let sound = sound_kt(alt);
        let from = (0.55 * sound).max(300.);
        let milestones: Vec<f64> = (3..=24)
            .map(|k| (f64::from(k) * 0.1 * sound / 10.).round() * 10.)
            .collect();
        for burner in [false, true] {
            let run = accelerate(a, set, alt, from, burner, &milestones)?;
            let (times, settled, drift, ps) =
                (run.times, run.settled_kt, run.drift_ft, run.peak_ps);
            let row: Vec<String> = milestones
                .iter()
                .zip(&times)
                .filter(|(m, t)| **m > from && t.is_some())
                .map(|(m, t)| format!("{m:.0}:{}", cell(*t).trim()))
                .collect();
            println!(
                "  alt {alt:6.0} {} from {from:4.0} kt (Mach {:.2}) settled {settled:6.1} kt (Mach {:.2}) drift {drift:4.0} ft peak Ps {ps:5.0} ft/s | {}",
                if burner { "AB " } else { "MIL" },
                from / sound,
                settled / sound,
                row.join(" "),
            );
        }
    }
    Ok(())
}

/// Constant-speed afterburner climb from sea level: the pitch holds a true
/// airspeed schedule (Mach 0.9 above 450 kt, otherwise 450 kt), then reports
/// the time to each altitude.
fn climb_section(a: &Aircraft, set: &Setup) -> Result<(), Box<dyn std::error::Error>> {
    if let Some((keas, mach)) = set.climb {
        return bomber_climb_section(a, set, keas, mach);
    }
    println!("-- afterburner climb from sea level, speed schedule max(450 kt, Mach 0.9)");
    for schedule in [0.8_f64, 0.9] {
        let mut s = start(a, set, 0., 450., true)?;
        s.pitch = 12_f64.to_radians();
        s.velocity = Basis::new(0., s.pitch, 0.).forward.map(|v| v * s.speed);
        let marks = [
            5_000., 10_000., 20_000., 30_000., 36_000., 40_000., 45_000., 50_000., 55_000., 59_000.,
        ];
        let mut hits: Vec<Option<(f64, f64)>> = vec![None; marks.len()];
        let mut best_roc = 0_f64;
        let mut roc_at = (0., 0.);
        let mut previous = (s.position[1], 0.);
        let mut top = 0_f64;
        for second in 0..1500_usize {
            for _ in 0..TICKS {
                let sound = sound_kt(s.position[1].clamp(0., 90_000.));
                let target = (schedule * sound).max(450.);
                let speed_kt = s.speed / KT;
                let want_pitch = (15. + (speed_kt - target) * 0.15).clamp(-5., 60.);
                let error = want_pitch - s.pitch.to_degrees();
                let input = PilotInput {
                    pitch: (0.06 * error - 0.03 * s.pitch_rate.to_degrees()).clamp(-1., 1.),
                    roll: (-2. * s.bank).clamp(-1., 1.),
                    throttle: Some(1.),
                    ..Default::default()
                };
                s.step_surface(&input, ground);
                if s.crashed {
                    break;
                }
            }
            let t = (second + 1) as f64;
            top = top.max(s.position[1]);
            for (i, m) in marks.iter().enumerate() {
                if hits[i].is_none() && s.position[1] >= *m {
                    hits[i] = Some((t, s.speed / KT));
                }
            }
            if (second + 1) % 5 == 0 {
                let roc = (s.position[1] - previous.0) / 5.;
                if roc > best_roc {
                    best_roc = roc;
                    roc_at = (s.position[1], s.speed / KT);
                }
                previous = (s.position[1], t);
            }
            if s.crashed || t > previous.1 + 90. && s.position[1] < top - 2_000. {
                break;
            }
        }
        println!(
            "  schedule Mach {schedule}: best sustained rate {:.0} ft/s ({:.0} ft/min) at {:.0} ft, {:.0} kt; highest {top:.0} ft",
            best_roc,
            best_roc * 60.,
            roc_at.0,
            roc_at.1
        );
        let row: Vec<String> = marks
            .iter()
            .zip(&hits)
            .map(|(m, h)| match h {
                Some((t, v)) => format!("{m:.0}:{t:.0}s@{v:.0}kt"),
                None => format!("{m:.0}:-"),
            })
            .collect();
        println!("    {}", row.join(" "));
    }
    Ok(())
}

/// Density ratio of the standard atmosphere at `altitude` against sea level.
fn density_ratio(altitude: f64) -> f64 {
    let at = |h: f64| {
        let a = Atmosphere::standard(h).expect("standard atmosphere");
        a.static_pressure_pa / (287.053 * a.temperature_k)
    };
    at(altitude.clamp(0., 90_000.)) / at(0.)
}

/// A bomber's climb (B2-8): full thrust from sea level on a constant
/// equivalent airspeed of `keas` knots, changing to `mach` once that is the
/// slower of the two, with the pitch trimmed by a speed integrator (the fixed
/// 15 degree law of the fighter climb cannot hold a speed at a thrust to
/// weight of 0.2). Reports the time to each altitude and the best rate.
fn bomber_climb_section(
    a: &Aircraft,
    set: &Setup,
    keas: f64,
    mach: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "-- full thrust climb from sea level, {keas:.0} kt equivalent airspeed to Mach {mach:.2}"
    );
    let target_kt =
        |h: f64| (keas / density_ratio(h).sqrt()).min(mach * sound_kt(h.clamp(0., 90_000.)));
    let mut s = start(a, set, 0., target_kt(0.), true)?;
    s.pitch = 4_f64.to_radians();
    s.velocity = Basis::new(0., s.pitch, 0.).forward.map(|v| v * s.speed);
    let marks = [
        2_000., 5_000., 10_000., 20_000., 30_000., 36_000., 40_000., 45_000., 50_000.,
    ];
    let mut hits: Vec<Option<(f64, f64)>> = vec![None; marks.len()];
    let mut want = 4_f64;
    let mut last_kt = s.speed / KT;
    let mut top = 0_f64;
    let mut previous = (s.position[1], 0.);
    let mut best: Vec<(f64, f64)> = Vec::new();
    let mut at_alt = [(0_f64, 0_f64); 12];
    for second in 0..3600_usize {
        for _ in 0..TICKS {
            let target = target_kt(s.position[1]);
            let speed_kt = s.speed / KT;
            want = (want + (speed_kt - target) * 0.004 / TICKS as f64).clamp(-5., 25.);
            let accel_kt = (speed_kt - last_kt) * TICKS as f64;
            last_kt = speed_kt;
            let command = (want + 0.12 * (speed_kt - target) + 1.5 * accel_kt).clamp(-8., 30.);
            let error = command - s.pitch.to_degrees();
            let input = PilotInput {
                pitch: (0.06 * error - 0.03 * s.pitch_rate.to_degrees()).clamp(-1., 1.),
                roll: (-2. * s.bank).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            s.step_surface(&input, ground);
            if s.crashed {
                break;
            }
        }
        let t = (second + 1) as f64;
        top = top.max(s.position[1]);
        for (i, m) in marks.iter().enumerate() {
            if hits[i].is_none() && s.position[1] >= *m {
                hits[i] = Some((t, s.speed / KT));
            }
        }
        if env::var("PROBE_TRACE").is_ok() && (second + 1) % 15 == 0 {
            println!(
                "    t {t:4.0} alt {:6.0} speed {:5.1} kt target {:5.1} pitch {:5.2} want {want:5.2}",
                s.position[1],
                s.speed / KT,
                target_kt(s.position[1]),
                s.pitch.to_degrees()
            );
        }
        if (second + 1) % 10 == 0 {
            let roc = (s.position[1] - previous.0) / 10.;
            best.push((s.position[1], roc));
            let band = ((s.position[1] / 5_000.) as usize).min(11);
            if at_alt[band].1 == 0. {
                at_alt[band] = (s.speed / KT, roc);
            }
            previous = (s.position[1], t);
        }
        if s.crashed
            || (second > 120 && best.last().is_some_and(|b| b.1 < 1.) && s.position[1] < 1_000_000.)
        {
            break;
        }
    }
    println!("  highest {top:.0} ft");
    let row: Vec<String> = marks
        .iter()
        .zip(&hits)
        .map(|(m, h)| match h {
            Some((t, v)) => format!("{m:.0}:{t:.0}s@{v:.0}kt"),
            None => format!("{m:.0}:-"),
        })
        .collect();
    println!("  {}", row.join(" "));
    let rate: Vec<String> = at_alt
        .iter()
        .enumerate()
        .filter(|(_, v)| v.0 > 0.)
        .map(|(i, v)| format!("{}k:{:.0}fpm@{:.0}kt", i * 5, v.1 * 60., v.0))
        .collect();
    println!(
        "  first rate seen in each 5,000 ft band: {}",
        rate.join(" ")
    );
    Ok(())
}

/// Level turn at constant altitude: bank held, load factor fed forward as
/// 1/cos(bank), altitude held by the stick, burner on. Returns (speed after the
/// run in kt, G at the end, largest altitude drift in ft, turn rate over the last
/// ten seconds in deg/s, whether it flew without crashing).
fn level_turn(
    a: &Aircraft,
    set: &Setup,
    altitude: f64,
    speed_kt: f64,
    bank_deg: f64,
    seconds: usize,
) -> Result<Turn, Box<dyn std::error::Error>> {
    let mut s = start(a, set, altitude, speed_kt, true)?;
    let bank = bank_deg.to_radians();
    s.bank = bank;
    let feed = (1. / bank.cos() - 1.) / 6.5;
    let mut worst = 0_f64;
    let mut g_end = 1_f64;
    let mut turned = 0_f64;
    let mut last_yaw = s.yaw;
    for second in 0..seconds {
        for _ in 0..TICKS {
            let error = altitude - s.position[1];
            let input = PilotInput {
                pitch: (feed + 0.003 * error - 0.05 * s.velocity[1]).clamp(-1., 1.),
                roll: (2. * (bank - s.bank)).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            s.step_surface(&input, ground);
            if s.crashed {
                return Ok(Turn {
                    speed_kt: s.speed / KT,
                    g: g_end,
                    drift_ft: worst,
                    rate_dps: 0.,
                    flew: false,
                });
            }
            worst = worst.max((s.position[1] - altitude).abs());
            g_end = s.g;
            let mut d = s.yaw - last_yaw;
            while d > std::f64::consts::PI {
                d -= 2. * std::f64::consts::PI;
            }
            while d < -std::f64::consts::PI {
                d += 2. * std::f64::consts::PI;
            }
            if second >= seconds.saturating_sub(10) {
                turned += d;
            }
            last_yaw = s.yaw;
        }
    }
    Ok(Turn {
        speed_kt: s.speed / KT,
        g: g_end,
        drift_ft: worst,
        rate_dps: (turned / 10.).abs().to_degrees(),
        flew: true,
    })
}

/// The Mach numbers of the sustained-turn section: `PROBE_TURN_MACH=a,b,c`
/// (default 0.7, 0.9, 1.1). A bomber whose sea level top speed is under Mach
/// 0.7 needs slower ones (B2-8).
fn turn_machs() -> Vec<f64> {
    env::var("PROBE_TURN_MACH")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .filter(|v: &Vec<f64>| !v.is_empty())
        .unwrap_or_else(|| vec![0.7, 0.9, 1.1])
}

fn turn_section(
    a: &Aircraft,
    set: &Setup,
    altitudes: &[f64],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("-- sustained level turn, afterburner (bank held, speed free for 40 s)");
    for &alt in altitudes {
        let sound = sound_kt(alt);
        for mach in turn_machs() {
            let v = mach * sound;
            let mut best: Option<(f64, f64, f64, f64)> = None;
            for bank in (30..=86).step_by(2) {
                let run = level_turn(a, set, alt, v, f64::from(bank), 45)?;
                let (end, g, drift, rate, ok) =
                    (run.speed_kt, run.g, run.drift_ft, run.rate_dps, run.flew);
                // Sustained: speed within 2 percent of the start or higher, altitude
                // within 400 ft, no crash.
                if ok && end >= 0.98 * v && drift < 400. {
                    best = Some((f64::from(bank), g, end, rate));
                }
            }
            match best {
                Some((bank, g, end, rate)) => println!(
                    "  alt {alt:6.0} Mach {mach:.1} ({v:4.0} kt): sustained bank {bank:.0} deg, {g:.2} G, {rate:4.1} deg/s, speed {end:.0} kt"
                ),
                None => println!(
                    "  alt {alt:6.0} Mach {mach:.1} ({v:4.0} kt): no sustained turn above 20 deg bank"
                ),
            }
        }
    }
    println!("-- instantaneous turn (full aft stick, three seconds, afterburner)");
    for &alt in altitudes {
        let sound = sound_kt(alt);
        for mach in [0.5_f64, 0.7, 0.9, 1.1] {
            let v = mach * sound;
            let mut s = start(a, set, alt, v, true)?;
            let (mut max_g, mut max_rate) = (0_f64, 0_f64);
            let mut min_speed = s.speed;
            for _ in 0..360 {
                s.step_surface(
                    &PilotInput {
                        pitch: 1.,
                        throttle: Some(1.),
                        ..Default::default()
                    },
                    ground,
                );
                max_g = max_g.max(s.g);
                max_rate = max_rate.max(s.pitch_rate.to_degrees());
                min_speed = min_speed.min(s.speed);
                if s.crashed {
                    break;
                }
            }
            // Negative G: full forward stick for two seconds.
            let mut p = start(a, set, alt, v, true)?;
            let mut min_g = 0_f64;
            for _ in 0..240 {
                p.step_surface(
                    &PilotInput {
                        pitch: -1.,
                        throttle: Some(1.),
                        ..Default::default()
                    },
                    ground,
                );
                min_g = min_g.min(p.g);
            }
            println!(
                "  alt {alt:6.0} Mach {mach:.1} ({v:4.0} kt): peak {max_g:.2} G, {max_rate:4.1} deg/s, speed after 3 s {:.0} kt; push {min_g:.2} G",
                min_speed / KT
            );
        }
    }
    Ok(())
}

/// Speed brake (B2-8): idle descent-free deceleration in level flight over ten
/// seconds, clean and with the airbrake out, at a few speeds. The altitude
/// hold keeps the flight level, so the lost speed is the drag alone.
fn brake_section(
    a: &Aircraft,
    set: &Setup,
    altitudes: &[f64],
) -> Result<(), Box<dyn std::error::Error>> {
    println!("-- speed brake: idle throttle, level, 10 s (kt lost, mean deceleration in G)");
    for &alt in altitudes {
        let top = state_top(a, alt)?.1 / KT;
        for share in [0.5_f64, 0.7, 0.9] {
            let v = share * top;
            let mut row = Vec::new();
            for out in [false, true] {
                let mut s = start(a, set, alt, v, false)?;
                s.brake_out = out;
                s.brake = if out { 1. } else { 0. };
                s.throttle = 0.;
                for _ in 0..(10 * TICKS) {
                    s.step_surface(
                        &PilotInput {
                            pitch: hold_altitude(&s, alt),
                            roll: (-2. * s.bank).clamp(-1., 1.),
                            throttle: Some(0.),
                            ..Default::default()
                        },
                        ground,
                    );
                }
                let lost = v - s.speed / KT;
                row.push((lost, lost * KT / 10. / 32.174));
            }
            println!(
                "  alt {alt:6.0} {v:4.0} kt ({:.2} of the top speed): clean {:5.1} kt ({:.3} G), brake out {:5.1} kt ({:.3} G), brake adds {:.3} G",
                share,
                row[0].0,
                row[0].1,
                row[1].0,
                row[1].1,
                row[1].1 - row[0].1,
            );
        }
    }
    Ok(())
}

fn fuel_section(a: &Aircraft, set: &Setup) -> Result<(), Box<dyn std::error::Error>> {
    let state = State::new(a, [0., 1000., 0.])?;
    let c = state.model().configuration();
    let (mil, ab) = (
        c.propulsion.military_fuel_lbs_per_second,
        c.propulsion.afterburner_fuel_lbs_per_second,
    );
    let tank = set.fuel.unwrap_or(c.mass.internal_fuel_lbs);
    println!("-- fuel (record flows against the real fuel/thrust)");
    println!(
        "  record flows: military {mil} lb/s ({:.0} lb/h), afterburner {ab} lb/s ({:.0} lb/h); tank {tank:.0} lb: {:.1} min military, {:.1} min afterburner",
        mil * 3600.,
        ab * 3600.,
        tank / mil / 60.,
        tank / ab / 60.,
    );
    println!(
        "  implied specific fuel consumption: military {:.3} lb/lbf/h, afterburner {:.3} lb/lbf/h",
        mil * 3600. / c.propulsion.military_thrust_lbf,
        ab * 3600. / c.propulsion.afterburner_thrust_lbf,
    );
    // Burn a real tank at several throttles in level flight and report the
    // time to flameout and the distance flown.
    for (label, throttle, burner) in [
        (
            "cruise 65 percent of top speed, throttle trimmed",
            f64::NAN,
            false,
        ),
        ("military power", 1., false),
        ("afterburner", 1., true),
    ] {
        let altitude = 20_000.;
        let (_, top) = state_top(a, altitude)?;
        let mut s = State::new(a, [0., altitude, 0.])?;
        s.enable_research(1)?;
        s.cheats.unlimited_fuel = false;
        s.fuel = tank;
        s.payload_lbs = set.payload;
        let speed = if throttle.is_nan() {
            0.65 * top
        } else {
            0.6 * top
        };
        s.speed = speed;
        s.velocity = Basis::new(0., 0., 0.).forward.map(|v| v * speed);
        s.burner = burner;
        s.throttle = if throttle.is_nan() { 0.6 } else { throttle };
        let mut distance = 0_f64;
        let mut ticks = 0_u64;
        let fuel_start = s.fuel;
        // Burn for five minutes (or until empty) and extrapolate: the flow is
        // constant per throttle setting.
        while s.fuel > 0.5 && ticks < 120 * 300 {
            let input = PilotInput {
                pitch: hold_altitude(&s, altitude),
                roll: (-2. * s.bank).clamp(-1., 1.),
                throttle: if throttle.is_nan() {
                    // Hold the start speed with the throttle.
                    Some((s.throttle + (speed - s.speed) * 0.0004 / 120.).clamp(0.05, 1.))
                } else {
                    Some(throttle)
                },
                ..Default::default()
            };
            s.step_surface(&input, ground);
            distance += s.speed / 120.;
            ticks += 1;
            if s.crashed {
                break;
            }
        }
        let seconds = ticks.max(1) as f64 / 120.;
        let flow = (fuel_start - s.fuel) / seconds;
        let mean_kt = distance / seconds / KT;
        println!(
            "  {label}: {flow:.2} lb/s over {seconds:.0} s at {mean_kt:.0} kt, so {:.1} min and {:.0} nm from the {tank:.0} lb tank (extrapolated), crashed {}",
            tank / flow.max(1e-9) / 60.,
            tank / flow.max(1e-9) * mean_kt / 3600.,
            s.crashed
        );
    }
    Ok(())
}

fn state_top(a: &Aircraft, altitude: f64) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    let state = State::new(a, [0., altitude, 0.])?;
    let (stall, top) = state
        .model()
        .configuration()
        .aerodynamics
        .envelopes
        .iter()
        .find(|e| e.g == 1)
        .and_then(|e| e.speeds(altitude))
        .ok_or("no 1 G row")?;
    Ok((stall, top))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: performance_probe EXTRACTED.PT")?;
    let mut bytes = Vec::new();
    File::open(&path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let a = Aircraft::parse(&bytes)?;
    let set = Setup {
        payload: env::var("PROBE_PAYLOAD")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.),
        fuel: env::var("PROBE_FUEL").ok().and_then(|v| v.parse().ok()),
        climb: env::var("PROBE_CLIMB").ok().and_then(|v| {
            let (k, m) = v.split_once(',')?;
            Some((k.parse().ok()?, m.parse().ok()?))
        }),
    };
    let altitudes: Vec<f64> = env::var("PROBE_ALTS").map_or_else(
        |_| vec![1_000., 15_000., 30_000., 36_000.],
        |v| v.split(',').filter_map(|x| x.parse().ok()).collect(),
    );
    let only = env::var("PROBE_ONLY").ok();
    let wants = |name: &str| only.as_deref().is_none_or(|o| o == name);
    println!(
        "== {} payload {:.0} lb fuel {:.0} lb",
        a.name,
        set.payload,
        set.fuel.unwrap_or(0.)
    );
    if wants("accel") {
        accel_section(&a, &set, &altitudes)?;
    }
    if wants("climb") {
        climb_section(&a, &set)?;
    }
    if wants("turn") {
        turn_section(&a, &set, &altitudes)?;
    }
    if wants("fuel") {
        fuel_section(&a, &set)?;
    }
    if only.as_deref() == Some("brake") {
        brake_section(&a, &set, &altitudes)?;
    }
    Ok(())
}
