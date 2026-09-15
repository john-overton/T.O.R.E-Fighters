//! Native departure-stage probe; explicit scripted inputs, not a full-flight oracle.
use std::{env, fs::File, io::Read};
use tore_formats::{
    aircraft::Aircraft,
    flight_model::{
        clock_rng::NativeRng,
        departure::{DepartureMode, StallState},
        departure_stage::{EnvelopeInputs, StageInput, StageState},
        force_stage::{self, Input as ForceInput, Setup as ForceSetup},
        forces::DragDevices,
        integration::{MovementAngles, Velocity},
        movement_stage::{self, Input as MovementInput},
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
                .or_else(|| aircraft.object.get(key))
                .ok_or_else(|| std::io::Error::other(format!("missing PT field {key}")))?
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
        let clean = aircraft
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .ok_or("missing 1G envelope")?;
        let upper = i16::try_from(
            tore_formats::flight_model::envelope_limits(clean, 15000 * 256, false, structure)?
                .maximum,
        )?;
        // Explicit empty, fuel-off, undamaged setup: loading percentages are zero.
        let force_setup = ForceSetup {
            drag: p.drag,
            loaded_drag: field("coefDrag")?,
            loaded_pull_drag: field("_gpullDrag")?,
            loaded_afterburner_thrust: field("aftThrust")?,
            selected_thrust: field("thrust")?,
            flaps_lift: i16::try_from(field("flapsLift")?)?,
            upper_fps: upper,
            limits: p.loaded_velocity(upper)?,
        };
        let empty_weight = field("weight")?;
        let low_speed_span = i16::try_from(field("lowAOASpeed")?)?;
        let low_speed_pitch = i16::try_from(field("lowAOAPitch")?)?;
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
                // Evaluate a force snapshot from the departure outputs. Do not
                // feed it back without the intervening normal-control/movement contract.
                let force_input = ForceInput {
                    velocity: Velocity {
                        forward: s.speed_f8,
                        side: 0,
                        down: 0,
                    },
                    weight: empty_weight,
                    fuel: 0,
                    altitude_f8: 15000 * 256,
                    g_f8: 256,
                    departure: s.departure.mode,
                    lift_scale_f8: out.lift_scale_f8,
                    envelopes: i.envelopes,
                    devices: DragDevices::default(),
                    throttle_f8: 0,
                    thrust_scale_f8: 256,
                    thrust_vector_pa: 0,
                    body_angles_pa: [degrees_to_pa(s.movement.pitch)?, i.body_bank_pa],
                    rudder_slip_f8: s.offsets_f8[2],
                    turbulence_pitch_f8: 0,
                    turbulence_yaw_f8: 0,
                    idle_floor: 0,
                    ticks: i.ticks,
                };
                let force = force_stage::advance(force_setup, &t, force_input)?;
                let force_replay = force_stage::advance(force_setup, &t, force_input)?;
                assert_eq!(force.velocity, force_replay.velocity);
                assert_eq!(force.lift, force_replay.lift);
                assert_eq!(force.force_g_f8, 256);
                let movement_input = MovementInput {
                    movement: s.movement,
                    position_f8: [0, 15000 * 256, 0],
                    velocity: force.velocity,
                    body_rates_f8: s.body_rates_f8,
                    cached_speed_fps: s.speed_f8 >> 8,
                    departure: s.departure.mode,
                    on_ground: false,
                    offsets_f8: s.offsets_f8,
                    turbulence_f8: [0; 2],
                    previous_heading_pa: 0,
                    clean_stall_fps: i.envelopes.clean_stall_fps,
                    low_speed_span,
                    low_speed_pitch,
                    wind_fps: 0,
                    wind_heading_pa: 0,
                    ticks: i.ticks,
                };
                let movement = movement_stage::advance(&t, &a, movement_input)?;
                assert_eq!(movement, movement_stage::advance(&t, &a, movement_input)?);
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
        "Diagnostic departure stage only: scripted inputs; force/movement snapshots evaluated separately; full normal controls/contact queries and retail trajectories are not simulated."
    );
    Ok(())
}
