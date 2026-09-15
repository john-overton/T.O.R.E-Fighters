//! Arithmetic translated from the hash-reviewed FA.EXE; not a complete flight tick.
//! See docs/formats/native-flight.md for addresses, domains and integration limits.
pub mod clock_rng;
pub mod departure;
pub mod departure_stage;
pub mod force_stage;
pub mod forces;
pub mod ground;
pub mod integration;
pub mod loading;
pub mod movement_stage;
pub mod normal_control;
pub mod profile;
pub mod queries;
pub mod rotation;
use crate::{Result, aircraft::Envelope, invalid};
fn divide(n: i64, d: i32) -> Result<i32> {
    if d == 0 {
        return Err(invalid("native flight division by zero"));
    }
    i32::try_from(n / d as i64).map_err(|_| invalid("native flight quotient overflow"))
}
fn mul_div(a: i32, b: i32, d: i32) -> Result<i32> {
    divide(a as i64 * b as i64, d)
}
fn div32(n: i32, d: i32) -> Result<i32> {
    divide(n as i64, d)
}
/// FA 0x4119a0: rate and state are fixed8; elapsed time is signed native 1/256s.
pub fn match_f24(current: i32, target: i32, rate: i32, ticks: i16) -> i32 {
    if current == target || rate == 0 {
        return current;
    }
    let step = rate.wrapping_mul(ticks as i32) >> 8;
    if current < target {
        current.wrapping_add(step).min(target)
    } else {
        current.wrapping_sub(step).max(target)
    }
}
/// FA 0x451e50. Engine/fuel gates and five-second tank scheduling are callers.
pub fn fuel_rate(military: i16, afterburner: i16, throttle: i32) -> i32 {
    if throttle > 100 {
        (afterburner as i32).wrapping_shl(8)
    } else {
        (military as i32).wrapping_mul(throttle).wrapping_shl(8) / 100
    }
}
/// FA 0x476aa0. Return degrees*256/second, not radians or binary angle units.
pub fn g_to_turn(g_fixed: i32, speed_fps: i32) -> Result<i32> {
    div32(
        (g_fixed as i16 as i32).wrapping_mul(2500),
        speed_fps.max(125) as i16 as i32,
    )
    .map(|v| v.clamp(-10240, 10240))
}
/// FA 0x47c18c..0x47c1e5. G in fixed8; positive sub-1G contributes no AoA.
pub fn pull_aoa(current: i32, g_fixed: i32, coefficient: i16, ticks: i16) -> i32 {
    let load = if g_fixed < 0 {
        g_fixed
    } else {
        g_fixed.wrapping_sub(256).max(0)
    };
    let target = (coefficient as i32).wrapping_mul(load) / 9;
    let rate = if target == 0 {
        current.wrapping_mul(15).wrapping_div(9).max(1024)
    } else {
        2560
    };
    match_f24(current, target, rate, ticks)
}
/// FA 0x476daf..0x476e58. Negative offset applies to movement pitch, not gear trim.
pub fn low_speed_pitch(
    speed_f8: i32,
    clean_stall_fps: i32,
    span: i16,
    pitch: i16,
    movement_pitch_f8: i32,
    ground: bool,
) -> Result<i32> {
    let excess = speed_f8
        .wrapping_sub(clean_stall_fps.wrapping_shl(8))
        .max(0);
    let range = (span as i32) << 8;
    if excess >= range {
        return Ok(0);
    }
    let mut offset = mul_div(
        -(pitch as i32).wrapping_shl(8),
        range.wrapping_sub(excess),
        range,
    )?;
    if ground {
        offset = offset.max(-20 * 256);
    }
    let factor = div32(
        (90i32 * 256)
            .wrapping_sub(movement_pitch_f8.wrapping_abs())
            .wrapping_shl(8),
        90 * 256,
    )?;
    Ok(div32(factor.wrapping_mul(offset), 256)?.min(offset.wrapping_add(25 * 256)))
}
/// FA 0x476950. All range/state values fixed8; acceleration/deceleration integer rates.
#[allow(clippy::too_many_arguments)]
pub fn stick_input(
    current: i32,
    maximum: i32,
    neutral: i32,
    minimum: i32,
    acc: i32,
    decel: i32,
    stick: i32,
    ticks: i16,
) -> Result<i32> {
    if minimum > maximum {
        return Err(invalid("native control range reversed"));
    }
    if stick == 0 {
        return Ok(match_f24(
            current,
            neutral.clamp(minimum, maximum),
            decel.wrapping_shl(8),
            ticks,
        ));
    }
    let extent = if stick > 0 {
        maximum.wrapping_sub(neutral)
    } else {
        minimum.wrapping_sub(neutral)
    };
    let offset = mul_div(extent, stick, if stick > 0 { 256 } else { -256 })?;
    let target = neutral.wrapping_add(offset).clamp(minimum, maximum);
    let mut rate = acc;
    if (current >= neutral) != (target >= neutral) {
        rate = rate.wrapping_add(decel / 2);
    }
    let signed = div32((rate as i16 as i32).wrapping_mul(stick as i16 as i32), 256)?;
    let scaled = if stick > 0 {
        signed
    } else {
        signed.wrapping_neg()
    };
    let rate = scaled.min(rate).max(rate >> 2);
    Ok(match_f24(
        current,
        target,
        if rate == 0 { 256 } else { rate.wrapping_shl(8) },
        ticks,
    ))
}
/// FA 0x476880: diminish excursions about neutral below twice clean 1G stall speed.
pub fn low_speed_limit(value: i32, neutral: i32, speed_f8: i32, stall: i32) -> Result<i32> {
    let speed = (speed_f8 >> 8).wrapping_abs();
    let top = stall.wrapping_mul(2);
    let excursion = value.wrapping_sub(neutral);
    Ok(if speed < top {
        div32(excursion.wrapping_mul(speed), top)?
    } else {
        excursion
    }
    .wrapping_add(neutral))
}
/// FA 0x412780: altitude fixed8 feet, result integer feet/second.
pub fn sound_speed(altitude_f8: i32) -> i32 {
    1115 + ((altitude_f8 >> 8).min(36000)).wrapping_mul(-148) / 36000
}
/// FA 0x47ab60: transonic drag percentage, not an aerodynamic CD.
pub fn drag_percent(speed_f8: i32, altitude_f8: i32, upper: i16) -> Result<i32> {
    if upper <= 0 {
        return Err(invalid("native upper speed must be positive"));
    }
    let base = div32(
        (speed_f8.wrapping_abs() >> 8).wrapping_mul(100),
        upper as i32,
    )?;
    let end = sound_speed(altitude_f8).min(upper as i32);
    let start = 366.min(end);
    let correction = if start == end {
        0
    } else {
        div32(
            (speed_f8 >> 8).wrapping_sub(start).wrapping_mul(75),
            end - start,
        )?
        .wrapping_sub(30)
    }
    .clamp(-30, 45);
    if correction <= 0 {
        div32((100 + correction).wrapping_mul(base), 100)
    } else {
        Ok(base.wrapping_add(div32(
            (100i32.wrapping_sub(base)).wrapping_mul(correction),
            100,
        )?))
    }
}
/// FA 0x47a8c0 at zero thrust-vector angle, caller-selected raw thrust.
pub fn scalar_thrust(
    throttle_f8: i32,
    scale_f8: i32,
    speed_f8: i32,
    upper: i16,
    selected: i32,
) -> Result<i32> {
    let command = div32((throttle_f8 >> 8).wrapping_mul(scale_f8), 100)?;
    let penalty = div32((speed_f8 & !254) >> 1, upper as i32)?;
    let scalar = command.wrapping_sub(penalty).max(0);
    Ok(div32(scalar.wrapping_mul(32767), 32767)?.wrapping_mul(selected))
}
pub fn clean_drag(
    percent: i32,
    coefficient: i32,
    selected_ab_thrust: i32,
    idle_floor: i32,
) -> Result<i32> {
    Ok(selected_ab_thrust.wrapping_mul(
        div32(percent.wrapping_mul(coefficient), 200)?
            .max(16)
            .max(idle_floor),
    ))
}
/// FA 0x49d440: signed 64-bit multiply and truncating division, wrapping differences.
pub fn interpolate(x0: i32, y0: i32, x1: i32, y1: i32, x: i32) -> Result<i32> {
    Ok(y0.wrapping_add(mul_div(
        x.wrapping_sub(x0),
        y1.wrapping_sub(y0),
        x1.wrapping_sub(x0),
    )?))
}
#[derive(Debug, PartialEq)]
pub struct Limits {
    pub minimum: i32,
    pub maximum: i32,
    pub structural: i32,
}
/// FA 0x49d2d0. Undefined/malformed source intersections become explicit errors.
pub fn envelope_limits(
    e: &Envelope,
    altitude_f8: i32,
    flaps: bool,
    structure: [i16; 2],
) -> Result<Limits> {
    if !(2..=20).contains(&e.points.len())
        || e.points
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.fract() != 0. || *x < 0. || *x > i32::MAX as f64)
    {
        return Err(invalid("envelope needs authored chains"));
    }
    let points: Vec<[i32; 2]> = e.points.iter().map(|p| p.map(|x| x as i32)).collect();
    let altitude = altitude_f8 >> 8;
    let highest = (0..points.len())
        .rev()
        .max_by_key(|&i| (points[i][1], i))
        .unwrap();
    let top = points[highest];
    let (mut minimum, maximum) = if altitude > top[1] {
        (top[0], top[0])
    } else {
        let lower = points
            .windows(2)
            .find(|p| p[0][1] <= altitude && p[1][1] >= altitude)
            .ok_or_else(|| invalid("no lower native envelope intersection"))?;
        let upper = points[highest..]
            .windows(2)
            .find(|p| p[0][1] >= altitude && p[1][1] <= altitude)
            .ok_or_else(|| invalid("no upper native envelope intersection"))?;
        (
            interpolate(lower[0][1], lower[0][0], lower[1][1], lower[1][0], altitude)?,
            interpolate(upper[0][1], upper[0][0], upper[1][1], upper[1][0], altitude)?,
        )
    };
    if altitude <= top[1] && flaps && (-1..=1).contains(&e.g) {
        minimum -= minimum >> 2;
    }
    let structural = if altitude >= 36000 {
        structure[1] as i32
    } else {
        interpolate(0, structure[0] as i32, 36000, structure[1] as i32, altitude)?
    };
    Ok(Limits {
        minimum,
        maximum,
        structural,
    })
}
pub fn envelope_class(limits: &Limits, speed_f8: i32) -> u8 {
    let speed = speed_f8 >> 8;
    if speed < limits.minimum {
        1
    } else if speed >= limits.structural {
        3
    } else if speed >= limits.maximum {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_arithmetic_and_faults() {
        assert_eq!(interpolate(0, 0, 3, -10, 1).unwrap(), -3);
        assert!(interpolate(1, 0, 1, 10, 1).is_err());
        assert!(mul_div(i32::MAX, 2, 1).is_err());
        assert_eq!(match_f24(0, 100, 2560, 2), 20);
        assert_eq!(match_f24(95, 100, 2560, 2), 100);
        assert_eq!(match_f24(5, 0, 2560, 2), 0);
    }
    #[test]
    fn load_offset_and_low_speed_are_separate() {
        assert_eq!(pull_aoa(0, 256, 20, 256), 0);
        assert_eq!(pull_aoa(0, 0, 20, 256), 0);
        assert_eq!(pull_aoa(0, 2560, 20, 256), 2560);
        assert_eq!(pull_aoa(0, -256, 20, 256), -568);
        assert_eq!(pull_aoa(100, 256, 20, 256), 0);
        assert_eq!(
            low_speed_pitch(200 * 256, 200, 70, 15, 0, false).unwrap(),
            -15 * 256
        );
        assert_eq!(
            low_speed_pitch(235 * 256, 200, 70, 15, 0, false).unwrap(),
            -1920
        );
        assert_eq!(
            low_speed_pitch(270 * 256, 200, 70, 15, 0, false).unwrap(),
            0
        );
        assert_eq!(
            low_speed_pitch(200 * 256, 200, 70, 15, 90 * 256, false).unwrap(),
            0
        );
    }
    #[test]
    fn control_authority_and_release() {
        assert_eq!(g_to_turn(256, 100).unwrap(), 5120);
        assert_eq!(g_to_turn(9 * 256, 125).unwrap(), 10240);
        assert_eq!(g_to_turn(-256, 100).unwrap(), -5120);
        assert_eq!(
            low_speed_limit(9 * 256, 256, 200 * 256, 200).unwrap(),
            5 * 256
        );
        assert_eq!(
            stick_input(0, 2560, 0, -2560, 10, 20, 256, 128).unwrap(),
            1280
        );
        assert_eq!(
            stick_input(1280, 2560, 0, -2560, 10, 20, 0, 128).unwrap(),
            0
        );
        assert!(stick_input(0, -1, 0, 1, 10, 20, 0, 1).is_err());
    }
    #[test]
    fn power_and_fuel_boundaries() {
        assert_eq!(fuel_rate(10, 30, 100), 2560);
        assert_eq!(fuel_rate(10, 30, 101), 7680);
        assert_eq!(sound_speed(0), 1115);
        assert_eq!(sound_speed(36000 * 256), 967);
        assert_eq!(sound_speed(50000 * 256), 967);
        assert_eq!(scalar_thrust(100 * 256, 256, 0, 1000, 100).unwrap(), 25600);
        assert_eq!(
            scalar_thrust(100 * 256, 256, 1000 * 256, 1000, 100).unwrap(),
            12800
        );
        assert_eq!(clean_drag(0, 100, 100, 0).unwrap(), 1600);
        assert_eq!(drag_percent(1000 * 256, 0, 1000).unwrap(), 100);
        assert!(drag_percent(100, 0, 0).is_err());
    }
    #[test]
    fn authored_envelope_chains_and_bad_data() {
        let mut e = Envelope {
            g: 1,
            points: vec![[100., 0.], [200., 10000.], [300., 10000.], [500., 0.]],
        };
        assert_eq!(
            envelope_limits(&e, 5000 * 256, false, [600, 800]).unwrap(),
            Limits {
                minimum: 150,
                maximum: 400,
                structural: 627
            }
        );
        assert_eq!(
            envelope_limits(&e, 5000 * 256, true, [600, 800])
                .unwrap()
                .minimum,
            113
        );
        assert_eq!(
            envelope_limits(&e, 10001 * 256, true, [600, 800])
                .unwrap()
                .minimum,
            300
        );
        let l = Limits {
            minimum: 100,
            maximum: 500,
            structural: 600,
        };
        for (speed, class) in [(99, 1), (100, 0), (499, 0), (500, 2), (600, 3)] {
            assert_eq!(envelope_class(&l, speed * 256), class);
        }
        e.points[0][0] = f64::NAN;
        assert!(envelope_limits(&e, 0, false, [600, 800]).is_err());
    }
}

pub mod tumble;
