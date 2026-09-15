//! Joined diagnostic FA force/velocity stage (0x47c682..0x47c6b5, 0x47c860).
//! Loaded/damage/device producers and later movement/contact remain caller-owned.
use super::{
    departure::DepartureMode,
    departure_stage::EnvelopeInputs,
    forces::{self, DragDevices, DragInput, DragProfile, LiftInput, ThrustInput},
    integration::{self, AxisLimits, Forces, Velocity},
    rotation::TrigTable,
};
use crate::Result;

/// Already resolved load/configuration values, never raw PT name lookups.
#[derive(Clone, Copy, Debug)]
pub struct Setup {
    pub drag: DragProfile,
    pub loaded_drag: i32,
    pub loaded_pull_drag: i32,
    pub loaded_afterburner_thrust: i32,
    pub selected_thrust: i32,
    pub flaps_lift: i16,
    pub upper_fps: i16,
    pub limits: [AxisLimits; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub velocity: Velocity,
    pub weight: i32,
    pub fuel: i32,
    pub altitude_f8: i32,
    pub g_f8: i32,
    /// Departure mode AFTER departure/normal-control dispatch, not at tick entry.
    pub departure: DepartureMode,
    /// Attenuated scale returned by departure dispatch; not stored G.
    pub lift_scale_f8: i32,
    pub envelopes: EnvelopeInputs,
    pub devices: DragDevices,
    pub throttle_f8: i32,
    pub thrust_scale_f8: i32,
    pub thrust_vector_pa: i16,
    /// Cached native body angles [pitch, bank], not movement Euler angles.
    pub body_angles_pa: [i16; 2],
    pub rudder_slip_f8: i32,
    pub turbulence_pitch_f8: i32,
    pub turbulence_yaw_f8: i32,
    /// Result of the separate native idle-floor producer; zero when inapplicable.
    pub idle_floor: i32,
    pub ticks: i16,
}
#[derive(Clone, Copy, Debug)]
pub struct Output {
    pub velocity: Velocity,
    pub forces: Forces,
    /// Temporary native G used for drag; caller's stored G is never modified.
    pub force_g_f8: i32,
    pub lift: i32,
}

/// Native thrust→drag→lift→gravity assembly and ordered scalar integration.
/// Side/down decay does not feed any of these force producers, so the pure
/// velocity helper performs it internally before applying the assembled forces.
pub fn advance(p: Setup, t: &TrigTable, i: Input) -> Result<Output> {
    let force_g_f8 = if i.departure == DepartureMode::Stalled {
        256
    } else {
        i.g_f8
    };
    let percent = super::drag_percent(i.velocity.forward, i.altitude_f8, p.upper_fps)?;
    let thrust = forces::thrust_force(
        ThrustInput {
            fuel: i.fuel,
            throttle_f8: i.throttle_f8,
            scale_f8: i.thrust_scale_f8,
            speed_f8: i.velocity.forward,
            upper_fps: p.upper_fps,
            selected_thrust: p.selected_thrust,
        },
        t.sin_cos(i.thrust_vector_pa),
    )?;
    let drag = forces::drag_force(
        p.drag,
        i.devices,
        DragInput {
            percent,
            loaded_coefficient: p.loaded_drag,
            loaded_afterburner_thrust: p.loaded_afterburner_thrust,
            idle_floor: i.idle_floor,
            weight: i.weight,
            g_f8: force_g_f8,
            loaded_pull_drag: p.loaded_pull_drag,
            rudder_slip_f8: i.rudder_slip_f8,
            turbulence_pitch_f8: i.turbulence_pitch_f8,
            turbulence_yaw_f8: i.turbulence_yaw_f8,
        },
    )?;
    let lift = forces::lift_force(
        LiftInput {
            speed_f8: i.velocity.forward,
            first_envelope_speed: i.envelopes.minimum_lift_fps,
            stall_fps: i.envelopes.clean_stall_fps,
            lift_scale_f8: i.lift_scale_f8,
            flaps_lift: p.flaps_lift,
            drag_percent: percent,
            weight: i.weight,
        },
        i.devices,
    )?;
    let gravity = forces::gravity_force(
        i.weight,
        t.sin_cos(i.body_angles_pa[0]),
        t.sin_cos(i.body_angles_pa[1].wrapping_neg()),
    )?;
    let forces = forces::assemble(drag, thrust, lift, gravity);
    let velocity = integration::velocity_step(
        i.velocity,
        forces,
        i.weight,
        p.limits,
        i.devices.on_ground && i.devices.gear,
        i.devices.on_ground,
        i.ticks,
    )?;
    Ok(Output {
        velocity,
        forces,
        force_g_f8,
        lift,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Setup, TrigTable, Input) {
        let mut words = [0i16; 321];
        words[64..128].fill(32767);
        let t = TrigTable::parse(
            &words
                .into_iter()
                .flat_map(i16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let p = Setup {
            drag: DragProfile {
                rudder: 10,
                flaps: 20,
                gear: 30,
                airbrake: 40,
                bay: 50,
                wheel: 7,
            },
            loaded_drag: 100,
            loaded_pull_drag: 10,
            loaded_afterburner_thrust: 1000,
            selected_thrust: 1000,
            flaps_lift: 100,
            upper_fps: 1000,
            limits: [AxisLimits {
                minimum: -1000,
                maximum: 1000,
                acceleration: 30000,
                deceleration: 30000,
            }; 3],
        };
        let i = Input {
            velocity: Velocity {
                forward: 500 * 256,
                side: 0,
                down: 0,
            },
            weight: 1000,
            fuel: 100,
            altitude_f8: 0,
            g_f8: 4 * 256,
            departure: DepartureMode::Stalled,
            lift_scale_f8: 128,
            envelopes: EnvelopeInputs {
                initial_class: 1,
                bounded_g_class: 1,
                current_g_stall_fps: 600,
                clean_stall_fps: 200,
                minimum_lift_fps: 100,
            },
            devices: DragDevices::default(),
            throttle_f8: 0,
            thrust_scale_f8: 256,
            thrust_vector_pa: 0,
            body_angles_pa: [0; 2],
            rudder_slip_f8: 0,
            turbulence_pitch_f8: 0,
            turbulence_yaw_f8: 0,
            idle_floor: 0,
            ticks: 2,
        };
        (p, t, i)
    }
    #[test]
    fn stalled_g_is_temporary_and_does_not_replace_attenuated_lift() {
        let (p, t, i) = fixture();
        let stalled = advance(p, &t, i).unwrap();
        assert_eq!(stalled.force_g_f8, 256);
        assert_eq!(i.g_f8, 1024);
        assert_eq!(stalled.lift, 128000);
        assert_eq!(stalled.forces.down, 128000);
        assert_eq!(stalled.velocity.down, 32); // (128000 / (1000>>5)) *2 >>8
        for mode in [
            DepartureMode::Normal,
            DepartureMode::Warning,
            DepartureMode::Spinning,
        ] {
            let other = advance(
                p,
                &t,
                Input {
                    departure: mode,
                    ..i
                },
            )
            .unwrap();
            assert_eq!(other.force_g_f8, 1024);
            assert_eq!(other.lift, stalled.lift);
            // 42 percent at 500fps; 3 excess G *10 adds 12600 force units.
            assert_eq!(other.forces.drag - stalled.forces.drag, 12600);
            assert!(other.velocity.forward < stalled.velocity.forward);
        }
    }
    #[test]
    fn clean_lift_threshold_and_native_low_speed_floor_survive_attenuation() {
        let (p, t, mut i) = fixture();
        i.lift_scale_f8 = 0;
        i.velocity.forward = 200 * 256;
        assert_eq!(advance(p, &t, i).unwrap().lift, 160000);
        i.velocity.forward = 99 * 256;
        assert_eq!(advance(p, &t, i).unwrap().lift, 0);
        i.velocity.forward = 500 * 256;
        assert_eq!(advance(p, &t, i).unwrap().lift, 0);
    }
    #[test]
    fn force_inputs_are_shared_and_invalid_integration_cannot_mutate_them() {
        let (p, t, mut i) = fixture();
        i.devices.on_ground = true;
        i.devices.gear = true;
        assert_eq!(advance(p, &t, i).unwrap().velocity.down, 0);
        i.weight = 31;
        let before = i.velocity;
        assert!(advance(p, &t, i).is_err());
        assert_eq!(i.velocity, before);
        i.weight = 1000;
        i.ticks = -1;
        assert!(advance(p, &t, i).is_err());
    }
}
