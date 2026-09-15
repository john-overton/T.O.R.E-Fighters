//! Reviewed G/pitch and primary roll consumers, not the entire normal branch.
use super::{g_to_turn, low_speed_limit, pull_aoa, stick_input};
use crate::{Result, invalid};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoadedAxis {
    /// G axis bounds are fixed8; roll axis bounds are whole degrees/second.
    pub minimum: i16,
    pub maximum: i16,
    pub acceleration: i16,
    pub deceleration: i16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub g_f8: i32,
    pub pitch_rate_f8: i32,
    pub roll_rate_f8: i32,
    pub aoa_f8: i32,
    pub bank_offset_f8: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub g: LoadedAxis,
    pub roll: LoadedAxis,
    pub pull_aoa_coefficient: i16,
    /// Commands AFTER departure attenuation.
    pub pitch_command: i32,
    pub roll_command: i32,
    pub speed_f8: i32,
    pub stall_fps: i32,
    pub on_ground: bool,
    pub ticks: i16,
}
/// 0x47c0a2..0x47c12b, 0x47c18c..0x47c1e5, 0x47c235..0x47c2a5,
/// and bank-offset release at 0x47c2f0. Auxiliary pitch/roll rates, high-G events,
/// damage effects and rudder remain separate; call only on normal dispatch.
pub fn advance(mut s: State, i: Input) -> Result<State> {
    if i.ticks < 0 || i.stall_fps <= 0 {
        return Err(invalid("invalid native control time/stall speed"));
    }
    for axis in [i.g, i.roll] {
        if axis.minimum > axis.maximum || axis.acceleration < 0 || axis.deceleration < 0 {
            return Err(invalid("invalid loaded native control axis"));
        }
    }
    let gmax = low_speed_limit(i.g.maximum as i32, 256, i.speed_f8, i.stall_fps)?;
    let gmin = low_speed_limit(i.g.minimum as i32, 256, i.speed_f8, i.stall_fps)?.min(0);
    s.g_f8 = stick_input(
        s.g_f8,
        gmax,
        256,
        gmin,
        i.g.acceleration as i32,
        i.g.deceleration as i32,
        i.pitch_command,
        i.ticks,
    )?;
    s.pitch_rate_f8 = g_to_turn(s.g_f8.wrapping_sub(256), i.speed_f8 >> 8)?;
    s.aoa_f8 = pull_aoa(s.aoa_f8, s.g_f8, i.pull_aoa_coefficient, i.ticks);
    s.roll_rate_f8 = if i.on_ground {
        0
    } else {
        // Source scales whole-degree bounds BEFORE shifting them to fixed8.
        let max = low_speed_limit(i.roll.maximum as i32, 0, i.speed_f8, i.stall_fps)?;
        let min = low_speed_limit(i.roll.minimum as i32, 0, i.speed_f8, i.stall_fps)?;
        stick_input(
            s.roll_rate_f8,
            max.wrapping_shl(8),
            0,
            min.wrapping_shl(8),
            i.roll.acceleration as i32,
            i.roll.deceleration as i32,
            i.roll_command,
            i.ticks,
        )?
    };
    s.bank_offset_f8 = super::match_f24(s.bank_offset_f8, 0, 90 * 256, i.ticks);
    Ok(s)
}
/// 0x47bb85..0x47bcb2: distinct low-speed movement fall while departure is NORMAL.
/// Source body words may have been updated by tumble; pitch cosine is cached at tick entry.
#[allow(clippy::too_many_arguments)]
pub fn passive_fall(
    t: &super::rotation::TrigTable,
    mut movement: super::integration::MovementAngles,
    body_pitch_pa: i16,
    body_bank_pa: i16,
    cached_pitch_cos: i16,
    speed_f8: i32,
    stall_fps: i32,
    ground: bool,
    mode: super::departure::DepartureMode,
    ticks: i16,
) -> Result<super::integration::MovementAngles> {
    if ticks < 0 {
        return Err(invalid("negative passive fall time"));
    }
    let deficit = stall_fps.wrapping_sub(speed_f8 >> 8);
    if deficit < 0 || ground || mode != super::departure::DepartureMode::Normal {
        return Ok(movement);
    }
    let percent = (super::div32(deficit.wrapping_mul(100), stall_fps.max(1))? + 50).min(100);
    let rate = |angle: i16, maximum: i32| -> Result<i32> {
        let sine = (t.sin_cos(angle / 2).sin as i32).abs();
        super::div32(
            super::div32(sine.wrapping_mul(maximum), 32767)?.wrapping_mul(percent),
            100,
        )
    };
    let pitch_target = if movement.pitch >= 0 && speed_f8 < 0 {
        90 * 256
    } else {
        -90 * 256
    };
    movement.pitch = super::match_f24(
        movement.pitch,
        pitch_target,
        rate(body_pitch_pa, 40 * 256)?,
        ticks,
    );
    let roll_rate = super::div32(
        rate(body_bank_pa, 60 * 256)?.wrapping_mul(cached_pitch_cos as i32),
        32767,
    )?;
    movement.roll = super::match_f24(
        movement.roll,
        if movement.roll < 0 {
            -180 * 256
        } else {
            180 * 256
        },
        roll_rate,
        ticks,
    );
    Ok(movement)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (State, Input) {
        (
            State {
                g_f8: 256,
                pitch_rate_f8: 0,
                roll_rate_f8: 0,
                aoa_f8: 0,
                bank_offset_f8: 256,
            },
            Input {
                g: LoadedAxis {
                    minimum: -4 * 256,
                    maximum: 9 * 256,
                    acceleration: 100,
                    deceleration: 100,
                },
                roll: LoadedAxis {
                    minimum: -121,
                    maximum: 121,
                    acceleration: 1000,
                    deceleration: 1000,
                },
                pull_aoa_coefficient: 9,
                pitch_command: 256,
                roll_command: 256,
                speed_f8: 100 * 256,
                stall_fps: 200,
                on_ground: false,
                ticks: 256,
            },
        )
    }
    #[test]
    fn passive_fall_is_normal_only_and_reverses_pitch_target_for_backward_travel() {
        let bytes: Vec<_> = [32767i16; 321]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect();
        let t = super::super::rotation::TrigTable::parse(&bytes).unwrap();
        let m = super::super::integration::MovementAngles {
            pitch: 75 * 256,
            roll: 5 * 256,
            heading: 0,
        };
        use super::super::departure::DepartureMode;
        let fall = |speed, ground, mode| {
            passive_fall(&t, m, 10000, 1000, 32767, speed, 200, ground, mode, 256).unwrap()
        };
        assert_eq!(fall(0, false, DepartureMode::Normal).pitch, 35 * 256);
        assert_eq!(fall(0, false, DepartureMode::Normal).roll, 65 * 256);
        assert_eq!(fall(-1, false, DepartureMode::Normal).pitch, 90 * 256);
        assert_eq!(fall(0, false, DepartureMode::Warning), m);
        assert_eq!(fall(0, true, DepartureMode::Normal), m);
    }
    #[test]
    fn low_speed_bounds_and_neutral_release_have_native_units() {
        let (s, i) = fixture();
        let out = advance(s, i).unwrap();
        assert_eq!(out.g_f8, 3 * 256);
        assert_eq!(out.pitch_rate_f8, 10240); // native turn-rate clamp
        assert_eq!(out.roll_rate_f8, 30 * 256); // truncation before fixed8 conversion
        assert_eq!(out.aoa_f8, 512);
        assert_eq!(out.bank_offset_f8, 0);
        let release = advance(
            out,
            Input {
                pitch_command: 0,
                roll_command: 0,
                ..i
            },
        )
        .unwrap();
        assert_eq!(
            (release.g_f8, release.pitch_rate_f8, release.roll_rate_f8),
            (256, 0, 0)
        );
    }
    #[test]
    fn ground_and_negative_control_boundaries() {
        let (s, i) = fixture();
        assert_eq!(
            advance(
                s,
                Input {
                    on_ground: true,
                    ..i
                }
            )
            .unwrap()
            .roll_rate_f8,
            0
        );
        let out = advance(
            s,
            Input {
                pitch_command: -256,
                roll_command: -256,
                ..i
            },
        )
        .unwrap();
        assert_eq!(out.g_f8, -64);
        assert_eq!(out.roll_rate_f8, -30 * 256);
        assert!(advance(s, Input { ticks: -1, ..i }).is_err());
    }
}
