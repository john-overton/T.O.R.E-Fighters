//! Source-backed acceptance for the variety flight adapters. No retail fixtures.
use std::{env, fs::File, io::Read};
use tore_formats::aircraft::Aircraft;
use tore_sim::{
    flight::{DT, FlightAxis, PilotCommand, PilotInput, State},
    models::{FlightModel, variety::LiftKind},
    research::Surface,
};

fn run(state: &mut State, input: &PilotInput, ticks: usize) {
    for _ in 0..ticks {
        state.step_surface(input, |_, _| Surface::runway(0.));
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: variety_flight EXTRACTED.PT [SECOND.PT ...]".into());
    }
    for path in paths {
        let mut bytes = Vec::new();
        File::open(path)?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 1024 * 1024 {
            return Err("PT exceeds size limit".into());
        }
        let aircraft = Aircraft::parse(&bytes)?;
        let mut state = State::new(&aircraft, [0., 1000., 0.])?;
        state.enable_research(1)?;
        let powered = state.model().powered_lift();
        if let Some(lift) = powered {
            state.yaw = 0.;
            state.pitch = 0.;
            state.bank = 0.;
            state.velocity = [0.; 3];
            state.speed = 0.;
            state.cheats.unlimited_fuel = true;
            let config = state.model().configuration();
            assert!(
                config.joined_native().is_err(),
                "powered lift remains outside restricted native research"
            );
            let weight = config.mass.empty_lbs + state.fuel;
            let lapse = (-1000. / config.tuning.thrust_lapse_feet).exp();
            // The jets hover on their nozzles' vertical efficiency and lift
            // engines (VTOL overhaul slice P4); the others on the old law.
            let jet = lift.jet;
            let capacity = jet.map_or(
                config.propulsion.military_thrust_lbf * lift.efficiency + lift.additional_lift_lbf,
                |jet| {
                    config.propulsion.military_thrust_lbf * jet.vertical_efficiency
                        + jet.lift_engines.map_or(0., |e| e.thrust_lbf)
                },
            );
            let fraction = weight / (capacity * lapse);
            let military = config.propulsion.military_thrust_lbf;
            assert!(fraction < 1., "{} cannot hover at this mass", aircraft.name);
            if let Some(jet) = jet {
                let vertical = 90. / jet.nozzle_range_degrees;
                state.throttle = fraction;
                state.lift_controls.vector_pitch = vertical;
                state.lift_controls.vector_pitch_actual = vertical;
                state.lift_controls.drive.engine_output = [
                    military * fraction * lapse,
                    jet.lift_engines.map_or(0., |e| e.thrust_lbf) * fraction * lapse,
                ];
                state.lift_controls.drive.lift_engine_spool = f64::from(jet.lift_engines.is_some());
            } else {
                state.throttle = 1.;
                state.lift_controls.collective = fraction;
                state.lift_controls.collective_actual = fraction;
            }
            state.lift_controls.thrust_lbf = weight;
            let mut copy = state.clone();
            run(&mut state, &Default::default(), 1200);
            run(&mut copy, &Default::default(), 1200);
            assert_eq!(state, copy);
            assert!((state.position[1] - 1000.).abs() < 5. && !state.crashed);
            let clearance = state.model().configuration().equipment.ground_clearance_ft;
            state.position[1] = clearance + 0.01;
            state.gear = 1.;
            state.gear_down = true;
            state.velocity[1] = -2.;
            run(&mut state, &Default::default(), 1);
            assert!(
                state.weight_on_wheels() && !state.crashed,
                "{} landing",
                aircraft.name
            );
            if lift.kind == LiftKind::VectorJet {
                // A fully fuelled Yak-141 hovers with 2.5 percent to spare,
                // less than its suck-down on the ground: it lifts off
                // vertically only lighter, as the real aircraft did.
                state.fuel *= 0.5;
                state.command(PilotCommand::AdjustThrottle(0.15));
            } else {
                state.command(PilotCommand::AdjustAxis(FlightAxis::Collective, 0.15));
            }
            run(&mut state, &Default::default(), 600);
            assert!(
                !state.weight_on_wheels() && state.position[1] > clearance + 10.,
                "{} departure",
                aircraft.name
            );
            run(
                &mut state,
                &PilotInput {
                    pitch: -0.2,
                    roll: 0.1,
                    yaw: 0.25,
                    ..Default::default()
                },
                240,
            );
            assert!(state.velocity[0].hypot(state.velocity[2]) > 1. && !state.crashed);
            // Convert away from hover using held commands, rather than teleporting actual pose.
            if lift.kind != LiftKind::Helicopter {
                state.position[1] = 5000.;
                state.pitch = 0.;
                state.bank = 0.;
                run(
                    &mut state,
                    &PilotInput {
                        conversion_rate: -1.,
                        vector_pitch_rate: -1.,
                        throttle: Some(1.),
                        ..Default::default()
                    },
                    480,
                );
                assert!(state.position.iter().all(|v| v.is_finite()) && !state.crashed);
                assert!(state.lift_controls.hover_fraction(lift.kind) < DT);
                assert!(
                    state.velocity[0].hypot(state.velocity[2]) > 10.,
                    "{} transition",
                    aircraft.name
                );
            }
            println!(
                "aircraft={} hover=pass vertical_landing=pass departure=pass controls=pass transition={} ticks={}",
                aircraft.name,
                if lift.kind == LiftKind::Helicopter {
                    "n/a"
                } else {
                    "pass"
                },
                state.ticks
            );
        } else {
            let mut copy = state.clone();
            let input = PilotInput {
                throttle: Some(1.),
                yaw: 0.02,
                ..Default::default()
            };
            run(&mut state, &input, 1200);
            run(&mut copy, &input, 1200);
            assert_eq!(state, copy);
            assert!(
                !state.crashed && state.position.iter().all(|v| v.is_finite()),
                "{} conventional flight",
                aircraft.name
            );
            assert!(state.fuel < copy.model().configuration().mass.internal_fuel_lbs);
            let clearance = state.model().configuration().equipment.ground_clearance_ft;
            let mut landing = state.clone();
            landing.position = [0., clearance + 0.001, 0.];
            landing.yaw = 0.;
            landing.pitch = 0.;
            landing.bank = 0.;
            landing.gear = 1.;
            landing.gear_down = true;
            landing.throttle = 0.;
            landing.engine = false;
            landing.velocity = [0., -2., 100.];
            landing.speed = 100.;
            run(&mut landing, &Default::default(), 1);
            assert!(
                landing.weight_on_wheels() && !landing.crashed,
                "{} gentle touchdown",
                aircraft.name
            );
            assert_eq!(landing.position[1], clearance);
            println!(
                "aircraft={} conventional=pass deterministic=pass landing=pass ticks={}",
                aircraft.name, state.ticks
            );
        }
    }
    Ok(())
}
