//! FA 0x47a970 drag assembly. Loaded coefficients are supplied by setup/loadout code.
use super::clean_drag;
use crate::Result;
#[derive(Clone, Copy, Debug)]
pub struct DragProfile {
    pub rudder: i16,
    pub flaps: i16,
    pub gear: i16,
    pub airbrake: i16,
    pub bay: i16,
    pub wheel: i16,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct DragDevices {
    pub gear: bool,
    pub flaps: bool,
    pub brake: bool,
    pub bay: bool,
    pub on_ground: bool,
}
#[derive(Clone, Copy, Debug)]
pub struct DragInput {
    pub percent: i32,
    pub loaded_coefficient: i32,
    pub loaded_afterburner_thrust: i32,
    /// Caller derives the airborne idle floor through 0x47ac20; zero otherwise.
    pub idle_floor: i32,
    pub weight: i32,
    pub g_f8: i32,
    pub loaded_pull_drag: i32,
    pub rudder_slip_f8: i32,
    pub turbulence_pitch_f8: i32,
    pub turbulence_yaw_f8: i32,
}
/// Preserve native order/truncation, especially wheel-coefficient truncation BEFORE weight.
pub fn drag_force(p: DragProfile, d: DragDevices, i: DragInput) -> Result<i32> {
    let mut force = clean_drag(
        i.percent,
        i.loaded_coefficient,
        i.loaded_afterburner_thrust,
        i.idle_floor,
    )?;
    let excess_g = i.g_f8.wrapping_abs().wrapping_sub(256);
    let mut extra = if excess_g > 0 {
        i.loaded_pull_drag.wrapping_mul(excess_g) / 256
    } else {
        0
    };
    if !d.on_ground {
        extra = extra
            .wrapping_add((p.rudder as i32).wrapping_mul(i.rudder_slip_f8.wrapping_abs()) / 256);
        extra = extra.wrapping_add(
            (i.turbulence_pitch_f8 >> 8)
                .wrapping_abs()
                .wrapping_mul(p.flaps as i32)
                .wrapping_mul(6)
                / 20,
        );
        extra = extra.wrapping_add(
            (i.turbulence_yaw_f8 >> 8)
                .wrapping_abs()
                .wrapping_mul(p.flaps as i32)
                .wrapping_mul(4)
                / 40,
        );
        if d.gear {
            extra = extra.wrapping_add(p.gear as i32);
        }
    }
    for (enabled, coefficient) in [(d.flaps, p.flaps), (d.brake, p.airbrake), (d.bay, p.bay)] {
        if enabled {
            extra = extra.wrapping_add(coefficient as i32);
        }
    }
    if extra != 0 {
        force = force.wrapping_add((i.weight.wrapping_mul(i.percent) / 100).wrapping_mul(extra));
    }
    if d.on_ground {
        let ground = if d.gear {
            ((p.wheel as i32).wrapping_mul(if d.brake { 50 } else { 20 }) / 100)
                .wrapping_mul(i.weight)
        } else {
            i.weight.wrapping_mul(384)
        };
        force = force.wrapping_add(ground);
    }
    Ok(force)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn devices_ground_and_integer_order() {
        let p = DragProfile {
            rudder: 10,
            flaps: 20,
            gear: 30,
            airbrake: 40,
            bay: 50,
            wheel: 7,
        };
        let i = DragInput {
            percent: 50,
            loaded_coefficient: 100,
            loaded_afterburner_thrust: 1000,
            idle_floor: 0,
            weight: 10000,
            g_f8: 256,
            loaded_pull_drag: 10,
            rudder_slip_f8: 0,
            turbulence_pitch_f8: 0,
            turbulence_yaw_f8: 0,
        };
        assert_eq!(drag_force(p, DragDevices::default(), i).unwrap(), 25000);
        assert_eq!(
            drag_force(
                p,
                DragDevices {
                    gear: true,
                    ..Default::default()
                },
                i
            )
            .unwrap(),
            175000
        );
        assert_eq!(
            drag_force(
                p,
                DragDevices {
                    gear: true,
                    on_ground: true,
                    ..Default::default()
                },
                i
            )
            .unwrap(),
            35000
        );
        // 7*50/100 = 3, rather than multiplying weight first to get 35000.
        assert_eq!(
            drag_force(
                p,
                DragDevices {
                    gear: true,
                    on_ground: true,
                    brake: true,
                    ..Default::default()
                },
                i
            )
            .unwrap(),
            255000
        );
        assert_eq!(
            drag_force(
                p,
                DragDevices {
                    on_ground: true,
                    ..Default::default()
                },
                i
            )
            .unwrap(),
            3865000
        );
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LiftInput {
    pub speed_f8: i32,
    /// First 1G envelope point speed (0x49d1b0), NOT altitude-adjusted stall speed.
    pub first_envelope_speed: i32,
    pub stall_fps: i32,
    pub lift_scale_f8: i32,
    pub flaps_lift: i16,
    pub drag_percent: i32,
    pub weight: i32,
}
/// FA 0x47c980: positive returned magnitude is subtracted from native down force.
pub fn lift_force(i: LiftInput, d: DragDevices) -> Result<i32> {
    if i.stall_fps <= 0 {
        return Err(crate::invalid("nonpositive native lift stall speed"));
    }
    let speed = i.speed_f8 >> 8;
    if speed < i.first_envelope_speed {
        return Ok(0);
    }
    let mut multiplier = 256;
    if d.flaps {
        // Gear-up branch leaves ECX at the raw PT coefficient, even though EAX
        // computed the speed-scaled value. Preserve this reviewed distinction.
        let scaled = i.drag_percent.wrapping_mul(i.flaps_lift as i32) / 100;
        let bonus = if !d.gear {
            i.flaps_lift as i32
        } else if d.on_ground {
            (scaled as i16 >> 1) as i32
        } else {
            scaled as i16 as i32
        };
        multiplier += bonus;
    }
    let mut scale = i.lift_scale_f8.wrapping_mul(multiplier) / 256;
    let top = i.stall_fps.wrapping_add(i.stall_fps.min(146));
    if top.wrapping_sub(speed) > 0 {
        scale = super::div32(speed.wrapping_mul(scale), top)?.max(160);
    }
    Ok(i.weight.wrapping_mul(scale))
}
/// FA 0x47ca70: gravity projection uses truncation by 32767, not >>15.
/// Trig arguments are pitch and NEGATIVE body roll, as passed by the native caller.
pub fn gravity_force(
    weight: i32,
    pitch: super::rotation::SinCos,
    negative_roll: super::rotation::SinCos,
) -> Result<[i32; 3]> {
    let w = weight.wrapping_shl(8);
    let side = super::div32(
        (negative_roll.sin as i32).wrapping_mul(pitch.cos as i32),
        32767,
    )?;
    let down = super::div32(
        (negative_roll.cos as i32).wrapping_mul(pitch.cos as i32),
        32767,
    )?;
    Ok([
        super::mul_div(w, pitch.sin as i32, 32767)?.wrapping_neg(),
        super::mul_div(w, side, 32767)?,
        super::mul_div(w, down, 32767)?,
    ])
}
#[derive(Clone, Copy, Debug)]
pub struct ThrustInput {
    pub fuel: i32,
    pub throttle_f8: i32,
    pub scale_f8: i32,
    pub speed_f8: i32,
    pub upper_fps: i16,
    pub selected_thrust: i32,
}
/// FA 0x47a860/0x47a8c0. Caller selects loaded military/AB thrust; vector trig supplied.
pub fn thrust_force(i: ThrustInput, vector: super::rotation::SinCos) -> Result<[i32; 2]> {
    if i.fuel <= 0 || i.throttle_f8 <= 0 {
        return Ok([0, 0]);
    }
    let command = super::div32((i.throttle_f8 >> 8).wrapping_mul(i.scale_f8), 100)?;
    let penalty = super::div32((i.speed_f8 & !254) >> 1, i.upper_fps as i32)?;
    let scalar = command.wrapping_sub(penalty).max(0);
    let component =
        |trig: i16| -> Result<i32> {
            Ok(super::div32((trig as i32).wrapping_mul(scalar), 32767)?
                .wrapping_mul(i.selected_thrust))
        };
    Ok([component(vector.cos)?, component(vector.sin)?])
}
/// Pure force assembly in the native order. Dynamic load/damage/VTOL setup stays upstream.
pub fn assemble(
    drag: i32,
    thrust: [i32; 2],
    lift: i32,
    gravity: [i32; 3],
) -> super::integration::Forces {
    super::integration::Forces {
        drag,
        forward: thrust[0].wrapping_add(gravity[0]),
        side: gravity[1],
        down: thrust[1].wrapping_sub(lift).wrapping_add(gravity[2]),
    }
}
#[cfg(test)]
mod force_tests {
    use super::*;
    use crate::flight_model::rotation::SinCos;
    const LEVEL: SinCos = SinCos { sin: 0, cos: 32767 };
    #[test]
    fn gravity_level_vertical_inverted_and_negative_projection() {
        assert_eq!(gravity_force(1000, LEVEL, LEVEL).unwrap(), [0, 0, 256000]);
        assert_eq!(
            gravity_force(1000, SinCos { sin: 32767, cos: 0 }, LEVEL).unwrap(),
            [-256000, 0, 0]
        );
        assert_eq!(
            gravity_force(
                1000,
                LEVEL,
                SinCos {
                    sin: 0,
                    cos: -32767
                }
            )
            .unwrap(),
            [0, 0, -256000]
        );
        let a = gravity_force(
            1000,
            LEVEL,
            SinCos {
                sin: 16000,
                cos: 28000,
            },
        )
        .unwrap();
        let b = gravity_force(
            1000,
            LEVEL,
            SinCos {
                sin: -16000,
                cos: 28000,
            },
        )
        .unwrap();
        assert_eq!(a[1], -b[1]);
        assert_eq!(a[2], b[2]);
    }
    #[test]
    fn lift_threshold_floor_and_gear_branch() {
        let mut i = LiftInput {
            speed_f8: 199 * 256,
            first_envelope_speed: 200,
            stall_fps: 200,
            lift_scale_f8: 256,
            flaps_lift: 100,
            drag_percent: 50,
            weight: 1000,
        };
        assert_eq!(lift_force(i, DragDevices::default()).unwrap(), 0);
        i.speed_f8 = 200 * 256;
        assert_eq!(lift_force(i, DragDevices::default()).unwrap(), 160000);
        i.speed_f8 = 400 * 256;
        assert_eq!(
            lift_force(
                i,
                DragDevices {
                    flaps: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            356000
        );
        assert_eq!(
            lift_force(
                i,
                DragDevices {
                    flaps: true,
                    gear: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            306000
        );
        assert_eq!(
            lift_force(
                i,
                DragDevices {
                    flaps: true,
                    gear: true,
                    on_ground: true,
                    ..Default::default()
                }
            )
            .unwrap(),
            281000
        );
    }
    #[test]
    fn power_gate_vector_and_level_force_balance() {
        let i = ThrustInput {
            fuel: 100,
            throttle_f8: 100 * 256,
            scale_f8: 256,
            speed_f8: 0,
            upper_fps: 1000,
            selected_thrust: 1000,
        };
        assert_eq!(thrust_force(i, LEVEL).unwrap(), [256000, 0]);
        assert_eq!(
            thrust_force(
                i,
                SinCos {
                    sin: -32767,
                    cos: 0
                }
            )
            .unwrap(),
            [0, -256000]
        );
        assert_eq!(
            thrust_force(ThrustInput { fuel: 0, ..i }, LEVEL).unwrap(),
            [0, 0]
        );
        let f = assemble(
            10,
            [20, 0],
            256000,
            gravity_force(1000, LEVEL, LEVEL).unwrap(),
        );
        assert_eq!((f.drag, f.forward, f.side, f.down), (10, 20, 0, 0));
    }
}
