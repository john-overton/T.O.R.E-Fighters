//! Native rudder/steering and auxiliary-rate consumers from FMFlight.
use super::{
    div32, g_to_turn,
    integration::{service_delta, wrap_angle},
    low_speed_limit, match_f24,
    normal_control::LoadedAxis,
    stick_input,
};
use crate::{Result, invalid};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub rudder: LoadedAxis,
    pub slip: i16,
    pub bank: i16,
    pub nominal_max_g: i16,
    pub puff: [LoadedAxis; 3],
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub yaw_rate_f8: i32,
    pub slip_f8: i32,
    pub normalized_rudder_f8: i32,
    pub auxiliary_rates_f8: [i32; 3],
    pub movement_roll_f8: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub speed_f8: i32,
    pub stall_fps: i32,
    pub loaded_max_g_f8: i16,
    pub ground_yaw: LoadedAxis,
    pub on_ground: bool,
    /// Native player flag enables the damage-percent reduction; None bypasses it.
    pub rudder_damage: Option<u8>,
    /// Effective primary roll/rudder commands, after departure/ground inhibition.
    pub roll_command: i32,
    pub rudder_command: i32,
    /// Original captured roll/pitch/rudder commands for the puff consumers.
    pub original_commands: [i32; 3],
    pub throttle_f8: i32,
    pub vector_f8: i32,
    pub ticks: i16,
}
/// 0x47b0e7..0x47b182. Preserve signed truncation and the vector MIN(-90) branch.
pub fn auxiliary_scale(
    throttle_f8: i32,
    vector_f8: i32,
    speed_f8: i32,
    ground: bool,
) -> Result<i32> {
    if ground {
        return Ok(0);
    }
    let throttle = ((throttle_f8 & !255) / 50).min(256);
    let vector = (vector_f8 >> 8).min(-90);
    let scale = div32(vector.wrapping_mul(throttle), -90)?;
    let speed = div32((220i32 * 256).wrapping_sub(speed_f8 & !255), 220)?.clamp(0, 256);
    div32(scale.wrapping_mul(speed), 256)
}
fn axis(current: i32, p: LoadedAxis, command: i32, ticks: i16) -> Result<i32> {
    stick_input(
        current,
        (p.maximum as i32) << 8,
        0,
        (p.minimum as i32) << 8,
        p.acceleration as i32,
        p.deceleration as i32,
        command,
        ticks,
    )
}
/// 0x47c130..0x47c18c, 0x47c235..0x47c682. Primary G/roll consumers run separately.
pub fn advance(mut s: State, p: Profile, i: Input) -> Result<State> {
    if i.ticks < 0 || i.stall_fps <= 0 {
        return Err(invalid("invalid control-tail time/stall speed"));
    }
    let scale = auxiliary_scale(i.throttle_f8, i.vector_f8, i.speed_f8, i.on_ground)?;
    for j in 0..3 {
        s.auxiliary_rates_f8[j] = axis(
            s.auxiliary_rates_f8[j],
            p.puff[j],
            div32(scale.wrapping_mul(i.original_commands[j]), 256)?,
            i.ticks,
        )?;
    }
    if i.on_ground {
        s.auxiliary_rates_f8 = [0; 3];
        let command = if i.rudder_command.wrapping_abs() >= i.roll_command.wrapping_abs() {
            i.rudder_command
        } else {
            i.roll_command
        };
        let percent = super::mul_div(i.speed_f8.wrapping_sub(73 * 256), 100 * 256, 73 * 256)?
            .clamp(0, 100 * 256);
        let bound = |v: i16| -> Result<i32> {
            let v = v as i32;
            Ok(v.wrapping_add(super::mul_div(v / 4 - v, percent, 100 * 256)?))
        };
        s.yaw_rate_f8 = stick_input(
            s.yaw_rate_f8,
            bound(i.ground_yaw.maximum)?.wrapping_shl(8),
            0,
            bound(i.ground_yaw.minimum)?.wrapping_shl(8),
            i.ground_yaw.acceleration as i32,
            i.ground_yaw.deceleration as i32,
            command,
            i.ticks,
        )?;
        s.normalized_rudder_f8 = 0;
        s.slip_f8 = match_f24(s.slip_f8, 0, 90 * 256, i.ticks);
        return Ok(s);
    }
    let turn = g_to_turn(256, i.speed_f8 >> 8)?;
    let raw_max = (p.rudder.maximum as i32).wrapping_mul(turn);
    let raw_min = (p.rudder.minimum as i32).wrapping_mul(turn);
    let ratio = div32(i.loaded_max_g_f8 as i32, p.nominal_max_g as i32)?.min(256);
    let limit = (i.loaded_max_g_f8 as i32 / 2).max(256);
    let max = div32(
        low_speed_limit(raw_max, 0, i.speed_f8, i.stall_fps)?.wrapping_mul(ratio),
        256,
    )?
    .min(limit);
    let min = div32(
        low_speed_limit(raw_min, 0, i.speed_f8, i.stall_fps)?.wrapping_mul(ratio),
        256,
    )?
    .max(-limit);
    let response = |value: i16| -> Result<i32> {
        let base = div32((value as i32).wrapping_mul(turn), 256)?;
        div32(base.wrapping_mul(max), raw_max)
    };
    let percent = 100 - i.rudder_damage.map_or(0, i32::from);
    let damage = |v: i32| div32(v.wrapping_mul(percent), 100);
    s.yaw_rate_f8 = stick_input(
        s.yaw_rate_f8,
        damage(max)?,
        0,
        damage(min)?,
        damage(response(p.rudder.acceleration)?)?,
        damage(response(p.rudder.deceleration)?)?,
        i.rudder_command,
        i.ticks,
    )?;
    let yaw_percent = div32(s.yaw_rate_f8.wrapping_mul(100), raw_max)?.clamp(-100, 100);
    let divisor = (raw_max / 4).max(max);
    s.normalized_rudder_f8 = if divisor == 0 {
        0
    } else {
        div32(s.yaw_rate_f8.wrapping_shl(8), divisor)?.clamp(-256, 256)
    };
    s.slip_f8 = div32(
        (p.slip as i32).wrapping_mul(yaw_percent).wrapping_shl(8),
        100,
    )?;
    s.movement_roll_f8 = wrap_angle(s.movement_roll_f8.wrapping_add(service_delta(
        (p.bank as i32).wrapping_mul(yaw_percent),
        i.ticks,
    )));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Profile, Input) {
        let axis = LoadedAxis {
            minimum: -2,
            maximum: 2,
            acceleration: 256,
            deceleration: 256,
        };
        (
            Profile {
                rudder: axis,
                slip: 20,
                bank: 10,
                nominal_max_g: 9,
                puff: [axis; 3],
            },
            Input {
                speed_f8: 1000 * 256,
                stall_fps: 200,
                loaded_max_g_f8: 9 * 256,
                ground_yaw: LoadedAxis {
                    minimum: -40,
                    maximum: 40,
                    ..axis
                },
                on_ground: false,
                rudder_damage: None,
                roll_command: 0,
                rudder_command: 256,
                original_commands: [0; 3],
                throttle_f8: 0,
                vector_f8: 0,
                ticks: 256,
            },
        )
    }
    #[test]
    fn airborne_symmetry_damage_and_release() {
        let (p, i) = fixture();
        let right = advance(State::default(), p, i).unwrap();
        let left = advance(
            State::default(),
            p,
            Input {
                rudder_command: -256,
                ..i
            },
        )
        .unwrap();
        assert_eq!(right.yaw_rate_f8, 1152);
        assert_eq!(left.yaw_rate_f8, -right.yaw_rate_f8);
        assert_eq!(right.slip_f8, 18 * 256);
        assert_eq!(left.slip_f8, -right.slip_f8);
        assert_eq!(right.movement_roll_f8, 900);
        let damaged = advance(
            State::default(),
            p,
            Input {
                rudder_damage: Some(50),
                ..i
            },
        )
        .unwrap();
        assert_eq!(damaged.yaw_rate_f8, 576);
        let release = advance(
            right,
            p,
            Input {
                rudder_command: 0,
                ..i
            },
        )
        .unwrap();
        assert_eq!(
            (
                release.yaw_rate_f8,
                release.slip_f8,
                release.normalized_rudder_f8
            ),
            (0, 0, 0)
        );
    }
    #[test]
    fn ground_selects_stronger_command_rudder_tie_and_quarters_limits() {
        let (p, i) = fixture();
        for (speed, expected) in [(73, 40), (146, 10)] {
            let out = advance(
                State::default(),
                p,
                Input {
                    on_ground: true,
                    speed_f8: speed * 256,
                    roll_command: -256,
                    rudder_command: 256,
                    ..i
                },
            )
            .unwrap();
            assert_eq!(out.yaw_rate_f8, expected * 256);
            assert_eq!(out.normalized_rudder_f8, 0);
            assert_eq!(out.auxiliary_rates_f8, [0; 3]);
        }
    }
    #[test]
    fn auxiliary_scale_preserves_source_vector_branch() {
        assert_eq!(auxiliary_scale(50 * 256, 0, 110 * 256, false).unwrap(), 128);
        assert_eq!(
            auxiliary_scale(50 * 256, -100 * 256, 110 * 256, false).unwrap(),
            142
        );
        assert_eq!(auxiliary_scale(50 * 256, 0, 220 * 256, false).unwrap(), 0);
        assert_eq!(auxiliary_scale(50 * 256, 0, 0, true).unwrap(), 0);
        let (p, i) = fixture();
        assert!(
            advance(
                State::default(),
                Profile {
                    nominal_max_g: 0,
                    ..p
                },
                i
            )
            .is_err()
        );
    }
}
