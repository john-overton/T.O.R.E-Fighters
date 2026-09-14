//! Reviewed scalar velocity stage, FA 0x47c860..0x47cc64.
//! Forces and loaded limits are caller inputs; this is not MovePlane or FMFlight.
use super::{div32, match_f24};
use crate::{Result, invalid};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisLimits {
    pub minimum: i16,
    pub maximum: i16,
    pub acceleration: i16,
    pub deceleration: i16,
}
impl AxisLimits {
    fn validate(self) -> Result<()> {
        if self.minimum > self.maximum || self.acceleration < 0 || self.deceleration < 0 {
            return Err(invalid("invalid native velocity limits"));
        }
        Ok(())
    }
}
/// FA 0x4c65ec uses a WIDE product; MatchF24 uses a wrapping 32-bit product.
pub fn service_delta(value: i32, ticks: i16) -> i32 {
    ((value as i64 * ticks as i64) >> 8) as i32
}
/// FA 0x47cbe0. Drag cannot cross zero; subsequent forces can.
pub fn axis_step(
    value: i32,
    acceleration_f8: i32,
    limits: AxisLimits,
    drag: bool,
    ticks: i16,
) -> Result<i32> {
    limits.validate()?;
    if ticks < 0 {
        return Err(invalid("negative native elapsed time"));
    }
    let acceleration = acceleration_f8.clamp(
        -(limits.deceleration as i32) << 8,
        (limits.acceleration as i32) << 8,
    );
    let delta = service_delta(acceleration, ticks);
    let result = if drag {
        if value < 0 {
            value.wrapping_add(delta).min(0)
        } else {
            value.wrapping_sub(delta).max(0)
        }
    } else {
        value.wrapping_add(delta)
    };
    Ok(result.clamp((limits.minimum as i32) << 8, (limits.maximum as i32) << 8))
}
/// FA 0x47cb80. Always damp transverse components before adding this tick's forces.
pub fn transverse_decay(value: i32, ticks: i16) -> i32 {
    let rate = (value.wrapping_abs() / 2).clamp(256, 16384);
    let delta = service_delta(rate, ticks);
    if value < 0 {
        value.wrapping_add(delta).min(0)
    } else {
        value.wrapping_sub(delta).max(0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Velocity {
    pub forward: i32,
    pub side: i32,
    /// Native positive-down body component; not world vertical speed.
    pub down: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Forces {
    pub drag: i32,
    pub forward: i32,
    pub side: i32,
    pub down: i32,
}
/// FA 0x47c860: decay, drag, forward force, side force, down force, ground clamp.
/// Force assembly (engine, devices, gravity/lift), loaded limits and contacts are upstream.
pub fn velocity_step(
    mut v: Velocity,
    f: Forces,
    weight: i32,
    mut limits: [AxisLimits; 3],
    grounded_with_gear: bool,
    on_ground: bool,
    ticks: i16,
) -> Result<Velocity> {
    if weight < 32 || ticks < 0 {
        return Err(invalid("invalid native weight/time"));
    }
    for l in limits {
        l.validate()?;
    }
    let mass = weight >> 5;
    v.side = transverse_decay(v.side, ticks);
    v.down = transverse_decay(v.down, ticks);
    v.forward = axis_step(v.forward, div32(f.drag, mass)?, limits[0], true, ticks)?;
    // Native temporarily quarters forward acceleration AFTER the drag pass.
    if grounded_with_gear {
        limits[0].acceleration = (limits[0].acceleration as i32 * 25 / 100) as i16;
    }
    v.forward = axis_step(v.forward, div32(f.forward, mass)?, limits[0], false, ticks)?;
    v.side = axis_step(v.side, div32(f.side, mass)?, limits[1], false, ticks)?;
    v.down = axis_step(v.down, div32(f.down, mass)?, limits[2], false, ticks)?;
    if on_ground {
        v.down = v.down.min(0);
    }
    Ok(v)
}
/// FA 0x477437..0x477479: pitch settling rate, depending on speed below clean stall.
pub fn ground_pitch(
    pitch_f8: i32,
    slope_f8: i32,
    speed_fps: i32,
    stall_fps: i32,
    ticks: i16,
) -> i32 {
    let difference = speed_fps.wrapping_sub(stall_fps);
    let rate = if pitch_f8 < slope_f8 {
        90
    } else if difference < -73 {
        45
    } else if difference < -44 {
        (-44 - difference) * 45 / 29
    } else {
        0
    };
    match_f24(pitch_f8, slope_f8, rate << 8, ticks)
}

#[cfg(test)]
mod tests {
    use super::*;
    const L: AxisLimits = AxisLimits {
        minimum: -1000,
        maximum: 1000,
        acceleration: 100,
        deceleration: 200,
    };
    #[test]
    fn wide_product_signed_rounding_and_drag_zero() {
        assert_eq!(service_delta(1_000_000, 32767), 127_996_093);
        assert_eq!(service_delta(-1, 1), -1);
        assert_eq!(axis_step(-10, 100 * 256, L, true, 256).unwrap(), 0);
        assert_eq!(axis_step(10, 100 * 256, L, true, 256).unwrap(), 0);
        assert_eq!(axis_step(10, -256, L, false, 256).unwrap(), -246);
        assert_eq!(transverse_decay(-300, 256), -44);
    }
    #[test]
    fn ordering_limits_and_ground_acceleration() {
        let v = Velocity {
            forward: 10 * 256,
            side: 0,
            down: 0,
        };
        let f = Forces {
            drag: 20 * 256,
            forward: 100 * 256,
            side: 0,
            down: 256,
        };
        let air = velocity_step(v, f, 32, [L; 3], false, false, 256).unwrap();
        assert_eq!(air.forward, 100 * 256); // net-force integration would incorrectly give 90.
        assert_eq!(air.down, 256);
        let ground = velocity_step(v, f, 32, [L; 3], true, true, 256).unwrap();
        assert_eq!(ground.forward, 25 * 256);
        assert_eq!(ground.down, 0);
        assert!(velocity_step(v, f, 31, [L; 3], false, false, 1).is_err());
        assert!(axis_step(0, 0, L, false, -1).is_err());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MovementAngles {
    pub roll: i32,
    pub pitch: i32,
    pub heading: i32,
}
/// FA 0x4768f0: one turn only; +/-180 degrees remain distinct endpoints.
pub fn wrap_angle(angle: i32) -> i32 {
    if angle.wrapping_sub(180 * 256) > 0 {
        angle.wrapping_sub(360 * 256)
    } else if angle.wrapping_add(180 * 256) < 0 {
        angle.wrapping_add(360 * 256)
    } else {
        angle
    }
}
/// FA 0x476b0d..0x476bb0, after 0x477010 transforms body rates.
/// Gravity, AoA/display rotations, wind and world position are separate later stages.
pub fn movement_angles(
    mut a: MovementAngles,
    transformed_rates: [i32; 3],
    ticks: i16,
) -> MovementAngles {
    a.roll = wrap_angle(
        a.roll
            .wrapping_add(service_delta(transformed_rates[0], ticks)),
    );
    a.pitch = a
        .pitch
        .wrapping_add(service_delta(transformed_rates[1], ticks));
    let crossed = a.pitch > 90 * 256 || a.pitch < -90 * 256;
    if crossed {
        a.pitch = if a.pitch > 90 * 256 {
            a.pitch.wrapping_sub(180 * 256)
        } else {
            a.pitch.wrapping_add(180 * 256)
        }
        .wrapping_neg();
        a.roll = wrap_angle(a.roll.wrapping_add(180 * 256));
        a.heading = wrap_angle(a.heading.wrapping_add(180 * 256));
    }
    a.heading = wrap_angle(
        a.heading
            .wrapping_add(service_delta(transformed_rates[2], ticks)),
    );
    a
}
#[cfg(test)]
mod movement_tests {
    use super::*;
    #[test]
    fn vertical_crossings_change_chart_without_pitch_clamp() {
        let a = MovementAngles {
            roll: 0,
            pitch: 89 * 256,
            heading: 0,
        };
        assert_eq!(
            movement_angles(a, [0, 2 * 256, 0], 256),
            MovementAngles {
                roll: 180 * 256,
                pitch: 89 * 256,
                heading: 180 * 256
            }
        );
        let a = MovementAngles {
            pitch: -89 * 256,
            ..a
        };
        assert_eq!(
            movement_angles(a, [0, -2 * 256, 0], 256),
            MovementAngles {
                roll: 180 * 256,
                pitch: -89 * 256,
                heading: 180 * 256
            }
        );
        assert_eq!(wrap_angle(180 * 256), 180 * 256);
        assert_eq!(wrap_angle(181 * 256), -179 * 256);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionStep {
    pub position_f8: [i32; 3],
    pub vertical_speed_fps: i16,
}
/// FA 0x476ed2..0x476f98, given the preceding body-to-world velocity result.
/// Wind is integrated separately then rotated, and omitted while on the ground.
/// Surface correction/collision follows this stage and is not included here.
pub fn position_step(
    mut position: [i32; 3],
    world_velocity: [i32; 3],
    wind_fps: i32,
    wind_trig: super::rotation::SinCos,
    on_ground: bool,
    ticks: i16,
) -> Result<PositionStep> {
    if ticks < 0 {
        return Err(invalid("negative position elapsed time"));
    }
    for (p, v) in position.iter_mut().zip(world_velocity) {
        *p = p.wrapping_add(service_delta(v, ticks));
    }
    if !on_ground {
        let wind = super::rotation::rotate_xz(
            [0, 0, service_delta(wind_fps.wrapping_shl(8), ticks)],
            wind_trig,
        );
        position[0] = position[0].wrapping_add(wind[0]);
        position[2] = position[2].wrapping_add(wind[2]);
    }
    Ok(PositionStep {
        position_f8: position,
        vertical_speed_fps: (world_velocity[1] >> 8) as i16,
    })
}
#[cfg(test)]
mod position_tests {
    use super::*;
    #[test]
    fn wind_is_separate_and_vertical_word_is_world_velocity() {
        let t = super::super::rotation::SinCos { sin: 0, cos: 32767 };
        let air = position_step([0; 3], [256, -257, 512], 10, t, false, 256).unwrap();
        assert_eq!(air.position_f8, [256, -257, 3071]);
        assert_eq!(air.vertical_speed_fps, -2);
        let ground = position_step([0; 3], [256, -257, 512], 10, t, true, 256).unwrap();
        assert_eq!(ground.position_f8, [256, -257, 512]);
    }
}
