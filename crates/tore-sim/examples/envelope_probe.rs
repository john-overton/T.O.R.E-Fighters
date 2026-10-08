//! Envelope and performance probe for user-extracted PT data: decoded G-row
//! speeds, simulated level top speed at full power and full-aft-stick pulls
//! near top speed. Development harness; no retail fixtures.
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    attitude::Basis,
    flight::{PilotInput, State},
    models::FlightModel,
    research::Surface,
};

const KT: f64 = 1.68781;

fn ground(_: f64, _: f64) -> Surface {
    Surface::runway(-60_000.)
}

fn level_start(
    a: &Aircraft,
    altitude: f64,
    speed: f64,
) -> Result<State, Box<dyn std::error::Error>> {
    let mut s = State::new(a, [0., altitude, 0.])?;
    s.enable_research(1)?;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = speed;
    s.velocity = Basis::new(0., 0., 0.).forward.map(|v| v * speed);
    s.throttle = 1.;
    s.burner = true;
    s.cheats.unlimited_fuel = true;
    if s.model().powered_lift().is_some() {
        s.lift_controls.conversion = 0.;
        s.lift_controls.conversion_actual = 0.;
        s.lift_controls.vector_pitch = 0.;
        s.lift_controls.vector_pitch_actual = 0.;
        s.lift_controls.collective = 1.;
        s.lift_controls.collective_actual = 1.;
    }
    Ok(s)
}

/// Holds altitude and wings level at full power until speed settles.
fn level_top_speed(
    a: &Aircraft,
    altitude: f64,
    start: f64,
) -> Result<(f64, bool), Box<dyn std::error::Error>> {
    let mut s = level_start(a, altitude, start)?;
    let mut last = s.speed;
    for second in 0..900 {
        for _ in 0..120 {
            let error = altitude - s.position[1];
            let input = PilotInput {
                pitch: (0.004 * error - 0.04 * s.velocity[1] - 0.5 * s.pitch_rate).clamp(-1., 1.),
                roll: (-2. * s.bank).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            s.step_surface(&input, ground);
            if s.crashed {
                return Ok((s.speed, false));
            }
        }
        if second > 60 && (s.speed - last).abs() < 0.01 {
            break;
        }
        last = s.speed;
    }
    Ok((s.speed, (s.position[1] - altitude).abs() < 200.))
}

/// Helicopter forward flight: fixed forward stick (attitude target), collective
/// holding altitude. Returns the settled speed and whether altitude held.
fn helicopter_speed(
    a: &Aircraft,
    altitude: f64,
    stick: f64,
) -> Result<(f64, bool), Box<dyn std::error::Error>> {
    let mut s = State::new(a, [0., altitude, 0.])?;
    s.enable_research(1)?;
    s.cheats.unlimited_fuel = true;
    let mut collective = s.lift_controls.collective;
    for _ in 0..120 * 300 {
        collective = (collective
            + (-0.002 * s.velocity[1] + 0.0002 * (altitude - s.position[1])) * 0.1)
            .clamp(0., 1.);
        let input = PilotInput {
            pitch: -stick,
            roll: (-2. * s.bank).clamp(-1., 1.),
            throttle: Some(1.),
            collective: Some(collective),
            ..Default::default()
        };
        s.step_surface(&input, ground);
        if s.crashed {
            return Ok((s.speed, false));
        }
    }
    Ok((
        s.speed,
        (s.position[1] - altitude).abs() < 200. && collective < 0.999,
    ))
}

/// Full aft stick for three seconds from level flight at `speed`.
fn pull(
    a: &Aircraft,
    altitude: f64,
    speed: f64,
) -> Result<(f64, f64, f64), Box<dyn std::error::Error>> {
    let mut s = level_start(a, altitude, speed)?;
    let (mut max_g, mut max_q, mut limit) = (f64::MIN, 0_f64, 0_f64);
    for tick in 0..360 {
        s.step_surface(
            &PilotInput {
                pitch: 1.,
                throttle: Some(1.),
                ..Default::default()
            },
            ground,
        );
        if tick == 0
            && let Some(t) = s.trace().adapter
        {
            limit = t.envelope.limits_g[1];
        }
        max_g = max_g.max(s.g);
        max_q = max_q.max(s.pitch_rate.to_degrees());
        if s.crashed {
            break;
        }
    }
    Ok((max_g, max_q, limit))
}

/// The AI control adapter asked to level out of a dive toward flat ground at
/// `altitude` and `knots`. Returns the lowest height and whether it crashed.
fn ai_recovery(
    a: &Aircraft,
    altitude: f64,
    knots: f64,
    dive_deg: f64,
) -> Result<(f64, &'static str, f64), Box<dyn std::error::Error>> {
    use tore_sim::ai::{
        ScalarSpeed, SpeedLimits,
        controller::{Completion, MotionIntent},
        motion::{Bank, CompletionAxis, Duration, MotionRequest, PitchRequest, SpeedRequest},
        steering::CommandMode,
        steering_adapter::ControlAdapter,
    };
    let mut s = State::new(a, [0., altitude, 0.])?;
    s.enable_research(1)?;
    s.yaw = 0.;
    s.pitch = -dive_deg.to_radians();
    s.bank = 0.;
    s.speed = knots * KT;
    s.velocity = Basis::new(0., s.pitch, 0.).forward.map(|v| v * s.speed);
    let speed = ScalarSpeed(knots * KT);
    let intent = MotionIntent {
        formation_flight: false,
        afterburner: false,
        id: 1,
        request: MotionRequest::new(
            0,
            PitchRequest::Explicit(0),
            Bank::Unconstrained,
            SpeedRequest::Explicit(speed),
            Duration::Timed(5),
        ),
        heading_deg: 0.,
        flight_path_pitch_deg: 0.,
        speed,
        bank: Bank::Unconstrained,
        completion: Completion::Axis(CompletionAxis::Heading),
        steering_point: None,
        mode: CommandMode::Ordinary,
    };
    let limits = SpeedLimits {
        minimum: ScalarSpeed(200.),
        maximum: ScalarSpeed(1000.),
        corner: ScalarSpeed(600.),
    };
    let mut adapter = ControlAdapter::new();
    let mut lowest = altitude;
    let mut fastest = s.speed;
    let (mut highest, mut flips, mut last) = (altitude, 0, 0_f64);
    for _ in 0..120 * 60 {
        let out = adapter.controls(
            &s,
            &intent,
            &limits,
            3.,
            30.,
            60.,
            Some(0.),
            tore_sim::flight::DT,
        )?;
        s.step_surface(&out.input, |_, _| Surface::runway(0.));
        lowest = lowest.min(s.position[1]);
        highest = highest.max(s.position[1]);
        if out.input.pitch.signum() != last.signum() && out.input.pitch.abs() > 0.5 {
            flips += 1;
            last = out.input.pitch;
        }
        fastest = fastest.max(s.speed);
        if s.crashed {
            let cause = if s.position[1] <= 50. {
                "ground"
            } else {
                "structure"
            };
            return Ok((lowest, cause, fastest / KT));
        }
    }
    if std::env::var_os("PROBE_VERBOSE").is_some() {
        println!(
            "    highest={highest:.0} stick_flips={flips} final_alt={:.0} final_kt={:.0}",
            s.position[1],
            s.speed / KT
        );
    }
    Ok((lowest, "recovered", fastest / KT))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: envelope_probe EXTRACTED.PT [SECOND.PT ...]".into());
    }
    let pulls = env::var_os("PROBE_PULLS").is_some();
    let level = env::var_os("PROBE_LEVEL").is_some();
    for path in paths {
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let a = Aircraft::parse(&bytes)?;
        let state = State::new(&a, [0., 1000., 0.])?;
        let c = state.model().configuration();
        let envelopes = &c.aerodynamics.envelopes;
        let one = envelopes.iter().find(|e| e.g == 1);
        let ceiling = one.map_or(0., |e| e.points.iter().map(|p| p[1]).fold(0., f64::max));
        println!(
            "== {} ({path}) empty={} fuel={} mil={} ab={} loadedDrag={} loadedElev={} pullDrag={} ceiling={ceiling}",
            a.name,
            c.mass.empty_lbs,
            c.mass.internal_fuel_lbs,
            c.propulsion.military_thrust_lbf,
            c.propulsion.afterburner_thrust_lbf,
            c.aerodynamics.loaded_drag_percent,
            c.aerodynamics.loaded_elevator_percent,
            c.aerodynamics.g_pull_drag_f8
        );
        if env::var_os("PROBE_POINTS").is_some() {
            for e in envelopes {
                println!("  row {:+} points {:?}", e.g, e.points);
            }
        }
        for e in envelopes.iter().filter(|e| e.g >= 1) {
            let cells: Vec<String> = [0., 10_000., 20_000., 30_000.]
                .iter()
                .map(|&alt| {
                    e.speeds(alt).map_or("     -     ".into(), |(lo, hi)| {
                        format!("{:4.0}-{:4.0}", lo / KT, hi / KT)
                    })
                })
                .collect();
            println!("  {:+}G kt SL|10k|20k|30k: {}", e.g, cells.join(" | "));
        }
        let helicopter = state
            .model()
            .powered_lift()
            .is_some_and(|l| l.kind == tore_sim::models::variety::LiftKind::Helicopter);
        if level && helicopter {
            for alt in [1_000., 5_000.] {
                let top = one.and_then(|e| e.speeds(alt)).map_or(0., |s| s.1);
                for stick in [0.25, 0.5, 0.75, 1.] {
                    let (speed, held) = helicopter_speed(&a, alt, stick)?;
                    println!(
                        "  heli alt={alt:5.0} stick={stick} env_top={:4.0}kt sim={:4.0}kt held={held}",
                        top / KT,
                        speed / KT
                    );
                }
            }
        } else if level {
            let altitudes: Vec<f64> = env::var("PROBE_ALTS").map_or_else(
                |_| vec![1_000., 10_000., 20_000., 30_000.],
                |list| list.split(',').filter_map(|a| a.parse().ok()).collect(),
            );
            for alt in altitudes {
                let Some((_, top)) = one.and_then(|e| e.speeds(alt)) else {
                    continue;
                };
                let (speed, held) = level_top_speed(&a, alt, top * 0.9)?;
                println!(
                    "  level alt={alt:5.0} env_top={:4.0}kt sim_top={:4.0}kt ratio={:.3} held={held}",
                    top / KT,
                    speed / KT,
                    speed / top
                );
            }
        }
        if env::var_os("PROBE_AI").is_some() {
            for (alt, kt, dive) in [
                (3000., 300., 10.),
                (5000., 320., 20.),
                (3000., 400., 10.),
                (5000., 400., 20.),
                (8000., 420., 30.),
                (12000., 440., 20.),
                (20000., 400., 15.),
                (20000., 440., 25.),
                (10000., 455., 0.),
                (10000., 455., 5.),
            ] {
                let (lowest, crashed, fastest) = ai_recovery(&a, alt, kt, dive)?;
                println!(
                    "  ai-dive alt={alt:5.0} kt={kt} dive={dive} lowest={lowest:6.0} fastest={fastest:4.0}kt outcome={crashed}"
                );
            }
        }
        if pulls {
            for alt in [1_000., 10_000., 20_000.] {
                let Some((_, top)) = one.and_then(|e| e.speeds(alt)) else {
                    continue;
                };
                let cells: Vec<String> = [0.5, 0.7, 0.85, 0.9, 0.95]
                    .iter()
                    .map(|&f| {
                        let (g, q, limit) = pull(&a, alt, top * f).unwrap();
                        format!("{:.2}:{g:4.2}G/{q:4.1}dps(lim {limit:4.2})", f)
                    })
                    .collect();
                println!("  pull alt={alt:5.0} {}", cells.join("  "));
            }
        }
    }
    Ok(())
}
