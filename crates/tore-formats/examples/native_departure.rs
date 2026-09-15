//! Native departure-stage probe; explicit scripted inputs, not a full-flight oracle.
use std::{env, fs::File, io::Read};
use tore_formats::{
    aircraft::Aircraft,
    flight_model::{
        clock_rng::NativeRng,
        departure::{DepartureMode, StallState},
        departure_stage::{EnvelopeInputs, StageInput, StageState},
        integration::MovementAngles,
        profile::FlightProfile,
        rotation::{AtanTable, TrigTable, degrees_to_pa},
    },
};
fn read(path: &str, max: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = vec![];
    File::open(path)?
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > max {
        return Err(std::io::Error::other("input exceeds diagnostic bound"));
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 3 {
        return Err(
            "usage: native_departure SINE-Q15.BIN ATAN-PA.BIN EXTRACTED.PT [SECOND.PT]".into(),
        );
    }
    let t = TrigTable::parse(&read(&args[0], 642)?)?;
    let a = AtanTable::parse(&read(&args[1], 1028)?)?;
    for path in &args[2..] {
        let aircraft = Aircraft::parse(&read(path, 1024 * 1024)?)?;
        let p = FlightProfile::from_fields(&aircraft.fields)?;
        let field = |key: &str| -> tore_formats::Result<i32> {
            aircraft
                .fields
                .get(key)
                .ok_or_else(|| std::io::Error::other("missing PT field"))?
                .number()
        };
        let structure = [
            i16::try_from(field("structure[0]")?)?,
            i16::try_from(field("structure[1]")?)?,
        ];
        // Reviewed 0x47add0 returns false immediately for this source field zero.
        if field("vtLimitDown")? != 0 {
            return Err("probe requires reviewed non-VTOL profile".into());
        }
        println!(
            "aircraft={} source_departure={:?} extended_warning={} vtLimitDown=0",
            aircraft.name, p.departure, p.extended_warning
        );
        for scenario in ["tumble-left", "tumble-right", "spin-left", "spin-right"] {
            let direction = if scenario.ends_with("left") { -1 } else { 1 };
            let spin = scenario.starts_with("spin");
            let mut s = StageState {
                departure: StallState {
                    mode: DepartureMode::Warning,
                    elapsed: if spin {
                        0
                    } else {
                        p.departure.warning_delay - 2
                    },
                },
                speed_f8: if spin { 180 * 256 } else { 100 * 256 },
                movement: MovementAngles {
                    pitch: if spin { 0 } else { 75 * 256 },
                    roll: direction * 5 * 256,
                    heading: 0,
                },
                ..Default::default()
            };
            let mut replay = s;
            let mut rng = NativeRng::seeded(1)?;
            let mut replay_rng = rng.clone();
            let (mut seen_tumble, mut seen_spin, mut recovered) = (false, false, false);
            let mut rotation_sum = 0i64;
            for tick in 0..900 {
                let controls = if spin {
                    if tick < 300 {
                        [0, 256, direction * 256]
                    } else {
                        [0, -256, -direction * 256]
                    }
                } else {
                    [0; 3]
                };
                let i = StageInput {
                    now: 1000 + tick * 2,
                    ticks: 2,
                    on_ground: false,
                    // Explicit scripted body-bank producer for this isolated stage.
                    body_bank_pa: degrees_to_pa(s.movement.roll.wrapping_neg())?,
                    global_flags: 0,
                    extended_warning: p.extended_warning,
                    vertical_thrust_support: false,
                    thrust_vector_f8: 0,
                    throttle_f8: 0,
                    controls,
                    recovery_locked: s.spin.recovery_locked,
                    envelopes: EnvelopeInputs::resolve(
                        &aircraft.envelopes,
                        structure,
                        256,
                        15000 * 256,
                        s.speed_f8,
                        false,
                        0,
                    )?,
                    lift_scale_f8: 256,
                };
                let before = s;
                let out = s.advance(&p.departure, &t, &a, &mut rng, i)?;
                let repeated = replay.advance(&p.departure, &t, &a, &mut replay_rng, i)?;
                assert_eq!(s, replay);
                assert_eq!(out, repeated);
                assert_eq!(rng, replay_rng);
                seen_tumble |= out.tumble_applied;
                seen_spin |= s.departure.mode == DepartureMode::Spinning;
                recovered |= out.recovered_spin;
                rotation_sum += (s.movement.heading - before.movement.heading).abs() as i64;
                if tick % 120 == 0
                    || before.departure.mode != s.departure.mode
                    || out.recovered_spin
                {
                    println!(
                        "scenario={scenario} now={} mode={:?} speed_f8={} movement={:?} offsets={:?} deadline={} severity={} normal_controls={} recovered={}",
                        i.now,
                        s.departure.mode,
                        s.speed_f8,
                        s.movement,
                        s.offsets_f8,
                        s.tumble.deadline,
                        out.severity_f8,
                        out.run_normal_controls,
                        out.recovered_spin
                    );
                }
            }
            if spin {
                assert!(seen_spin && recovered, "native spin entry/recovery");
            } else {
                assert!(seen_tumble && rotation_sum > 0, "native tumble composition");
            }
            println!(
                "PASS {} {scenario}: tumble={seen_tumble} spin={seen_spin} recovered={recovered}",
                aircraft.name
            );
        }
    }
    println!(
        "Diagnostic departure stage only: scripted inputs; normal controls/forces/contact and retail trajectories are not simulated."
    );
    Ok(())
}
