//! FA 0x477140 landing-limit classifier, separate from terrain/carrier callbacks.
#[derive(Clone, Copy, Debug)]
pub struct LandingLimits {
    pub forward_fps: i16,
    pub side_fps: i16,
    pub descent_fps: i16,
    pub pitch_degrees: i16,
    pub roll_degrees: i16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LandingSeverity {
    WithinLimits = 0,
    Code5 = 5,
    Code6 = 6,
}
/// FA truncates angles/speed with SAR then narrows ABS operands to signed words.
/// Descent input is the native signed vertical-speed WORD (negative descending).
/// Result is only one input to contact handling, not a final crash decision.
pub fn landing_severity(
    limits: LandingLimits,
    movement_roll_f8: i32,
    movement_pitch_f8: i32,
    forward_f8: i32,
    side_f8: i32,
    vertical_fps: i16,
) -> LandingSeverity {
    let abs_word = |value: i32| (value as i16 as i32).abs();
    let excess = [
        (
            abs_word(movement_roll_f8 >> 8) - limits.roll_degrees as i32,
            20,
        ),
        (
            abs_word(movement_pitch_f8 >> 8) - limits.pitch_degrees as i32,
            20,
        ),
        ((forward_f8 >> 8) - limits.forward_fps as i32, 50),
        (abs_word(side_f8 >> 8) - limits.side_fps as i32, 10),
        (-(limits.descent_fps as i32 + vertical_fps as i32), 20),
    ];
    if excess.iter().any(|(e, margin)| e > margin) {
        LandingSeverity::Code6
    } else if excess.iter().any(|(e, _)| *e > 0) {
        LandingSeverity::Code5
    } else {
        LandingSeverity::WithinLimits
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landing_boundaries_use_native_strict_comparisons() {
        let l = LandingLimits {
            forward_fps: 300,
            side_fps: 10,
            descent_fps: 20,
            pitch_degrees: 15,
            roll_degrees: 10,
        };
        let check = |speed, vertical| landing_severity(l, 0, 0, speed * 256, 0, vertical);
        assert_eq!(check(300, -20), LandingSeverity::WithinLimits);
        assert_eq!(check(350, -40), LandingSeverity::Code5);
        assert_eq!(check(351, -20), LandingSeverity::Code6);
        assert_eq!(check(300, -41), LandingSeverity::Code6);
        assert_eq!(
            landing_severity(l, -31 * 256, 0, 0, 0, 0),
            LandingSeverity::Code6
        );
        assert_eq!(
            super::super::integration::ground_pitch(10 * 256, 0, 100, 200, 256),
            0
        );
        assert_eq!(
            super::super::integration::ground_pitch(10 * 256, 0, 156, 200, 256),
            10 * 256
        );
    }
}

/// FA 0x4774f0: query results are caller supplied, not guessed from a terrain pixel.
#[derive(Clone, Copy, Debug)]
pub struct ContactSurface {
    pub difficulty_bypass: bool,
    pub water: bool,
    pub gear_down: bool,
    pub type_surface_bypass: bool,
    /// None means query failure; otherwise the signed output of 0x4ba8e0.
    pub surface_query: Option<i32>,
}
pub fn contact_code(surface: ContactSurface, severity: LandingSeverity) -> u8 {
    if surface.difficulty_bypass {
        return 0;
    }
    if surface.water {
        return 7;
    }
    if severity != LandingSeverity::WithinLimits {
        return severity as u8;
    }
    if surface.gear_down
        && (surface.type_surface_bypass || surface.surface_query.is_some_and(|v| v <= 0x465000))
    {
        0
    } else {
        5
    }
}
/// FA height retention preceding the *second* OnTheGround query (0x477258..2e9).
/// Keep the ground PA word separate: this branch compares PA+364 against F8.
#[derive(Clone, Copy, Debug)]
pub struct ContactRetention {
    pub previous_ground: bool,
    pub previous_height: i32,
    pub height: i32,
    pub ground_pitch_pa: i16,
    pub minimum_lift_fps: i32,
    pub vertical_support: bool,
}
pub fn retain_height(
    input: ContactRetention,
    y: i32,
    hold: i16,
    forward_f8: i32,
    effective_pitch_f8: i32,
    ticks: i16,
) -> (i32, i16) {
    let abrupt = input
        .previous_height
        .wrapping_sub(input.height)
        .wrapping_abs()
        > 0x1900;
    let holding = hold > 0 && !abrupt;
    let slow_ground = input.previous_ground
        && !abrupt
        && (forward_f8 >> 8) < input.minimum_lift_fps
        && !input.vertical_support
        && input.ground_pitch_pa as i32 + 364 >= effective_pitch_f8;
    (
        if holding || slow_ground {
            input.height
        } else {
            y
        },
        if holding {
            hold.wrapping_sub(ticks)
        } else {
            hold
        },
    )
}
#[cfg(test)]
mod contact_tests {
    use super::*;
    #[test]
    fn surface_precedence_and_hold_threshold() {
        let mut s = ContactSurface {
            difficulty_bypass: false,
            water: false,
            gear_down: true,
            type_surface_bypass: false,
            surface_query: Some(0x465000),
        };
        assert_eq!(contact_code(s, LandingSeverity::WithinLimits), 0);
        s.surface_query = Some(0x465001);
        assert_eq!(contact_code(s, LandingSeverity::WithinLimits), 5);
        s.water = true;
        assert_eq!(contact_code(s, LandingSeverity::Code6), 7);
        s.difficulty_bypass = true;
        assert_eq!(contact_code(s, LandingSeverity::Code6), 0);
        let mut r = ContactRetention {
            previous_ground: false,
            previous_height: 0,
            height: 0x1900,
            ground_pitch_pa: 0,
            minimum_lift_fps: 100,
            vertical_support: false,
        };
        assert_eq!(retain_height(r, 42, 1, 0, 0, 5), (0x1900, -4));
        r.height += 1;
        assert_eq!(retain_height(r, 42, 1, 0, 0, 5), (42, 1));
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ContactState {
    pub y_f8: i32,
    pub pitch_f8: i32,
    pub roll_f8: i32,
    pub roll_rate_f8: i32,
    pub yaw_rate_f8: i32,
    pub pitch_down_rate_f8: i32,
    pub hold_ticks: i16,
    pub side_f8: i32,
    pub down_f8: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct ContactInput {
    /// Result of 0x411910 after height retention.
    pub touching: bool,
    pub previous_ground: bool,
    /// State written by GetGround, independently of the second touching query.
    pub ground: bool,
    pub water: bool,
    pub cp_0xe3_nonzero: bool,
    pub classified_code: u8,
    pub ground_height_f8: i32,
    pub ground_pitch_f8: i32,
    pub ground_roll_pa: i16,
    pub low_speed_pitch_f8: i32,
    pub forward_fps: i32,
    pub stall_fps: i32,
    pub ticks: i16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactEvents {
    pub code: u8,
    /// Invoke 0x412a60 with (4, 0x40) at the successful touchdown boundary.
    pub touchdown: bool,
    /// Apply contact_flags (0x49fd40) when this is true.
    pub contact_callback: bool,
}
/// FA 0x4772e9..0x4774e4 after queries. Terrain query and touchdown event dispatch remain caller responsibilities.
pub fn settle_contact(s: &mut ContactState, i: ContactInput) -> crate::Result<ContactEvents> {
    use super::integration::{ground_pitch, service_delta};
    use super::rotation::pa_to_degrees;
    let mut events = ContactEvents {
        code: 0,
        touchdown: false,
        contact_callback: false,
    };
    if !i.touching {
        return Ok(events);
    }
    events.code = if i.cp_0xe3_nonzero {
        i.classified_code
    } else {
        6
    };
    if events.code == 0 && !i.previous_ground {
        events.touchdown = true;
        s.roll_rate_f8 = 0;
        s.yaw_rate_f8 = 0;
        let effective = s.pitch_f8.wrapping_add(i.low_speed_pitch_f8);
        if s.pitch_f8 > i.ground_pitch_f8 && effective < i.ground_pitch_f8 {
            let rate = i
                .ground_pitch_f8
                .wrapping_sub(effective)
                .wrapping_mul(6)
                .clamp(30 * 256, 120 * 256);
            s.pitch_down_rate_f8 = s.pitch_down_rate_f8.max(rate);
        }
        s.hold_ticks = 128;
    }
    s.y_f8 = s.y_f8.max(i.ground_height_f8);
    let approach = |value: i32, target: i32, rate: i32| {
        let step = rate.wrapping_mul(i.ticks as i32) >> 8;
        if value < target {
            value.wrapping_add(step).min(target)
        } else {
            value.wrapping_sub(step).max(target)
        }
    };
    if s.pitch_down_rate_f8 > 0 {
        s.pitch_f8 = s
            .pitch_f8
            .wrapping_sub(service_delta(s.pitch_down_rate_f8, i.ticks));
        if s.pitch_f8 <= i.ground_pitch_f8 {
            s.pitch_down_rate_f8 = 0;
            s.pitch_f8 = i.ground_pitch_f8;
        }
        s.pitch_down_rate_f8 = approach(s.pitch_down_rate_f8, 0, 60 * 256);
    }
    if i.ground {
        s.pitch_f8 = ground_pitch(
            s.pitch_f8,
            i.ground_pitch_f8,
            i.forward_fps,
            i.stall_fps,
            i.ticks,
        );
        s.roll_f8 = approach(
            s.roll_f8,
            pa_to_degrees(i.ground_roll_pa.wrapping_neg())?,
            180 * 256,
        );
        s.side_f8 = 0;
        s.down_f8 = s.down_f8.min(0);
    }
    if i.water && matches!(events.code, 5 | 6) {
        events.code = 7;
    }
    events.contact_callback = true;
    Ok(events)
}

/// FA 0x411910: one fixed8 foot of contact tolerance, inclusive.
pub fn touching_ground(y_f8: i32, queried_height_f8: i32) -> bool {
    queried_height_f8.wrapping_add(256) >= y_f8
}
/// FA 0x49fd40: sets the landing latch on transition, preserves it while grounded,
/// clears it when airborne. This helper is not a carrier dynamics callback.
pub fn contact_flags(flags: u32, previous_ground: bool, ground: bool) -> u32 {
    if !ground {
        flags & !0x04000000
    } else if !previous_ground {
        flags | 0x04000000
    } else {
        flags
    }
}
#[cfg(test)]
mod settle_tests {
    use super::*;
    #[test]
    fn touchdown_settles_but_airborne_query_does_not() {
        let mut s = ContactState {
            y_f8: -1,
            pitch_f8: 10 * 256,
            roll_f8: 30 * 256,
            roll_rate_f8: 100,
            yaw_rate_f8: 100,
            pitch_down_rate_f8: 0,
            hold_ticks: 0,
            side_f8: 40,
            down_f8: 50,
        };
        let i = ContactInput {
            touching: true,
            previous_ground: false,
            ground: true,
            water: false,
            cp_0xe3_nonzero: true,
            classified_code: 0,
            ground_height_f8: 0,
            ground_pitch_f8: 0,
            ground_roll_pa: 0,
            low_speed_pitch_f8: -20 * 256,
            forward_fps: 100,
            stall_fps: 200,
            ticks: 2,
        };
        let e = settle_contact(&mut s, i).unwrap();
        assert_eq!(
            e,
            ContactEvents {
                code: 0,
                touchdown: true,
                contact_callback: true
            }
        );
        assert_eq!(
            (
                s.y_f8,
                s.hold_ticks,
                s.side_f8,
                s.down_f8,
                s.roll_rate_f8,
                s.yaw_rate_f8
            ),
            (0, 128, 0, 0, 0, 0)
        );
        assert!(s.pitch_f8 < 10 * 256 && s.pitch_down_rate_f8 > 0 && s.roll_f8 < 30 * 256);
        assert!(
            !settle_contact(
                &mut s,
                ContactInput {
                    touching: false,
                    ..i
                }
            )
            .unwrap()
            .contact_callback
        );
        assert!(touching_ground(256, 0));
        assert!(!touching_ground(257, 0));
        assert_eq!(contact_flags(1, false, true), 0x04000001);
        assert_eq!(contact_flags(1, true, true), 1);
        assert_eq!(contact_flags(0x04000001, true, false), 1);
    }
}
