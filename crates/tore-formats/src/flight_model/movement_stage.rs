//! Diagnostic composition of FA MovePlane (0x476ae0), with explicit contact queries.
use super::{
    departure::DepartureMode,
    ground::{self, ContactEvents, ContactInput, ContactState},
    integration::{self, MovementAngles, Velocity},
    rotation::{self, AtanTable, TrigTable},
};
use crate::{Result, invalid};

/// FA 0x476bb0..0x476cba, after the rate transform and vertical chart crossing.
/// The speed is caller-cached 0x5451f0, not necessarily the updated forward speed.
pub fn gravity_turn(
    t: &TrigTable,
    mut a: MovementAngles,
    speed_fps: i32,
    mode: DepartureMode,
    grounded: bool,
    ticks: i16,
) -> Result<MovementAngles> {
    if ticks < 0 {
        return Err(invalid("negative movement elapsed time"));
    }
    if mode == DepartureMode::Spinning || grounded || a.pitch <= -90 * 256 {
        return Ok(a);
    }
    let rate = super::g_to_turn(256, speed_fps)?;
    let pitch = t.sin_cos(rotation::degrees_to_pa(a.pitch)?);
    let correction = super::div32((pitch.cos as i32).wrapping_mul(rate), 32767)?;
    a.pitch = a
        .pitch
        .wrapping_sub(integration::service_delta(correction, ticks));
    let roll = t.sin_cos(rotation::degrees_to_pa(a.roll)?);
    a.pitch = a.pitch.wrapping_add(integration::service_delta(
        super::div32((roll.cos as i32).wrapping_mul(correction), 32767)?,
        ticks,
    ));
    a.heading = integration::wrap_angle(a.heading.wrapping_add(integration::service_delta(
        super::div32((roll.sin as i32).wrapping_mul(correction), 32767)?,
        ticks,
    )));
    a.pitch = a.pitch.max(-90 * 256);
    Ok(a)
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub movement: MovementAngles,
    pub position_f8: [i32; 3],
    pub velocity: Velocity,
    /// Already combined native body rates, including the temporary additive rates.
    pub body_rates_f8: [i32; 3],
    pub cached_speed_fps: i32,
    pub departure: DepartureMode,
    pub on_ground: bool,
    /// Canonical departure state order: bank, AoA, slip.
    pub offsets_f8: [i32; 3],
    /// Native compositor order: heading, pitch.
    pub turbulence_f8: [i32; 2],
    pub previous_heading_pa: i16,
    /// Query with flaps explicitly cleared, unlike departure envelope queries.
    pub clean_stall_fps: i32,
    pub low_speed_span: i16,
    pub low_speed_pitch: i16,
    pub wind_fps: i32,
    pub wind_heading_pa: i16,
    pub ticks: i16,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Output {
    pub movement: MovementAngles,
    pub position_f8: [i32; 3],
    pub velocity: Velocity,
    pub body_angles_pa: [i16; 3],
    pub heading_chart_toggle: bool,
    pub vertical_speed_fps: i16,
    pub low_speed_pitch_f8: i32,
}
/// Up to the 0x477240 contact call; does not invent terrain/carrier producers.
pub fn advance(t: &TrigTable, atan: &AtanTable, i: Input) -> Result<Output> {
    if i.ticks < 0 {
        return Err(invalid("negative movement elapsed time"));
    }
    let rates = rotation::body_rates(
        t,
        i.body_rates_f8,
        rotation::degrees_to_pa(i.movement.roll)?,
        rotation::degrees_to_pa(i.movement.pitch)?,
    )?;
    let movement = gravity_turn(
        t,
        integration::movement_angles(i.movement, rates, i.ticks),
        i.cached_speed_fps,
        i.departure,
        i.on_ground,
        i.ticks,
    )?;
    // cockpit_angles accepts heading-offset/AoA/bank, not departure's bank/AoA/slip.
    let (body_angles_pa, heading_chart_toggle) = rotation::cockpit_angles(
        t,
        atan,
        movement,
        [i.offsets_f8[2], i.offsets_f8[1], i.offsets_f8[0]],
        i.turbulence_f8,
        i.previous_heading_pa,
    )?;
    let low_speed_pitch_f8 = super::low_speed_pitch(
        i.velocity.forward,
        i.clean_stall_fps,
        i.low_speed_span,
        i.low_speed_pitch,
        movement.pitch,
        i.on_ground,
    )?;
    let effective = if i.velocity.forward < 0 {
        movement.pitch.wrapping_sub(low_speed_pitch_f8)
    } else {
        movement.pitch.wrapping_add(low_speed_pitch_f8)
    }
    .clamp(-90 * 256, 90 * 256);
    let world = rotation::world_velocity(t, i.velocity, movement, effective)?;
    let position = integration::position_step(
        i.position_f8,
        world,
        i.wind_fps,
        t.sin_cos(i.wind_heading_pa),
        i.on_ground,
        i.ticks,
    )?;
    Ok(Output {
        movement,
        position_f8: position.position_f8,
        velocity: i.velocity,
        body_angles_pa,
        heading_chart_toggle,
        vertical_speed_fps: position.vertical_speed_fps,
        low_speed_pitch_f8,
    })
}
impl Output {
    /// Apply reviewed post-query settling at the newly integrated position.
    /// ContactInput must come from queries at this output position. Query ground,
    /// touching, retained height and classification remain distinct explicit inputs.
    /// Display angles/vertical speed intentionally retain their pre-contact values.
    pub fn settle(
        &mut self,
        contact: &mut ContactState,
        mut i: ContactInput,
    ) -> Result<ContactEvents> {
        if i.ticks < 0 {
            return Err(invalid("negative contact elapsed time"));
        }
        let mut state = *contact;
        state.y_f8 = self.position_f8[1];
        state.pitch_f8 = self.movement.pitch;
        state.roll_f8 = self.movement.roll;
        state.side_f8 = self.velocity.side;
        state.down_f8 = self.velocity.down;
        i.low_speed_pitch_f8 = self.low_speed_pitch_f8;
        i.forward_fps = self.velocity.forward >> 8;
        let events = ground::settle_contact(&mut state, i)?;
        self.position_f8[1] = state.y_f8;
        self.movement.pitch = state.pitch_f8;
        self.movement.roll = state.roll_f8;
        self.velocity.side = state.side_f8;
        self.velocity.down = state.down_f8;
        *contact = state;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> (TrigTable, AtanTable) {
        // Synthetic mathematical fixture, never copied from retail tables.
        let bytes: Vec<_> = (0..321)
            .flat_map(|n| {
                ((n as f64 * std::f64::consts::TAU / 256.)
                    .sin()
                    .mul_add(32767., 0.)
                    .round() as i16)
                    .to_le_bytes()
            })
            .collect();
        (
            TrigTable::parse(&bytes).unwrap(),
            AtanTable::parse(&[0; 1028]).unwrap(),
        )
    }
    fn input() -> Input {
        Input {
            movement: MovementAngles::default(),
            position_f8: [0; 3],
            velocity: Velocity {
                forward: 500 * 256,
                side: 0,
                down: 0,
            },
            body_rates_f8: [0; 3],
            cached_speed_fps: 500,
            departure: DepartureMode::Normal,
            on_ground: false,
            offsets_f8: [0; 3],
            turbulence_f8: [0; 2],
            previous_heading_pa: 0,
            clean_stall_fps: 200,
            low_speed_span: 100,
            low_speed_pitch: 20,
            wind_fps: 0,
            wind_heading_pa: 0,
            ticks: 2,
        }
    }
    #[test]
    fn gravity_turn_preserves_level_and_has_distinct_spin_ground_gates() {
        let (t, _) = tables();
        let flat = MovementAngles::default();
        assert_eq!(
            gravity_turn(&t, flat, 500, DepartureMode::Normal, false, 256).unwrap(),
            flat
        );
        let inverted = MovementAngles {
            roll: 180 * 256,
            ..flat
        };
        let turned = gravity_turn(&t, inverted, 500, DepartureMode::Normal, false, 256).unwrap();
        assert!(turned.pitch < -2500);
        assert_eq!(
            gravity_turn(&t, inverted, 500, DepartureMode::Spinning, false, 256).unwrap(),
            inverted
        );
        assert_eq!(
            gravity_turn(&t, inverted, 500, DepartureMode::Normal, true, 256).unwrap(),
            inverted
        );
    }
    #[test]
    fn body_offsets_do_not_steer_velocity_and_vertical_crossing_remains_available() {
        let (t, a) = tables();
        let i = input();
        let plain = advance(&t, &a, i).unwrap();
        let bank = advance(
            &t,
            &a,
            Input {
                offsets_f8: [5 * 256, 0, 0],
                ..i
            },
        )
        .unwrap();
        assert_eq!(plain.position_f8, bank.position_f8);
        assert_eq!(plain.movement, bank.movement);
        assert_eq!(
            bank.body_angles_pa[2],
            rotation::degrees_to_pa(5 * 256).unwrap()
        );
        let crossed = advance(
            &t,
            &a,
            Input {
                movement: MovementAngles {
                    pitch: 89 * 256,
                    ..Default::default()
                },
                body_rates_f8: [0, 4 * 256, 0],
                ticks: 256,
                departure: DepartureMode::Spinning,
                ..i
            },
        )
        .unwrap();
        assert!(crossed.movement.pitch < 89 * 256);
        assert_eq!(crossed.movement.roll, 180 * 256);
        assert_eq!(crossed.movement.heading, 180 * 256);
    }
    #[test]
    fn repeated_loops_cross_both_vertical_attitudes_in_both_directions() {
        let (t, a) = tables();
        for direction in [-1, 1] {
            let mut i = Input {
                body_rates_f8: [0, direction * 4 * 256, 0],
                departure: DepartureMode::Spinning,
                ticks: 256,
                ..input()
            };
            let mut crossings = 0;
            let mut high = false;
            let mut low = false;
            for _ in 0..200 {
                let out = advance(&t, &a, i).unwrap();
                crossings += usize::from(out.movement.roll != i.movement.roll);
                high |= out.movement.pitch > 80 * 256;
                low |= out.movement.pitch < -80 * 256;
                i.movement = out.movement;
                i.position_f8 = out.position_f8;
            }
            assert!(crossings >= 4 && high && low);
        }
    }
    #[test]
    fn contact_uses_integrated_position_and_leaves_precontact_display_snapshot() {
        let (t, a) = tables();
        let mut out = advance(&t, &a, input()).unwrap();
        let before_display = out.body_angles_pa;
        let mut contact = ContactState {
            y_f8: -999,
            pitch_f8: 999,
            roll_f8: 999,
            roll_rate_f8: 20,
            yaw_rate_f8: 30,
            pitch_down_rate_f8: 0,
            hold_ticks: 0,
            side_f8: 0,
            down_f8: 0,
        };
        let query = ContactInput {
            touching: true,
            previous_ground: false,
            ground: true,
            water: false,
            cp_0xe3_nonzero: true,
            classified_code: 0,
            ground_height_f8: 256,
            ground_pitch_f8: 0,
            ground_roll_pa: 0,
            low_speed_pitch_f8: 999,
            forward_fps: -999,
            stall_fps: 200,
            ticks: 2,
        };
        let event = out.settle(&mut contact, query).unwrap();
        assert!(event.touchdown && event.contact_callback);
        assert_eq!(out.position_f8[1], 256);
        assert_eq!(contact.hold_ticks, 128);
        assert_eq!((contact.roll_rate_f8, contact.yaw_rate_f8), (0, 0));
        assert_eq!(out.body_angles_pa, before_display);
        let before = (out, contact);
        assert!(
            out.settle(&mut contact, ContactInput { ticks: -1, ..query })
                .is_err()
        );
        assert_eq!((out, contact), before);
    }
}
