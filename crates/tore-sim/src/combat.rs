//! FA combat components with explicit caller state and native integer domains.
//! Diagnostic until complete dispatch/contact/guidance contracts are accepted.
//! No renderer, resource lookup, native execution, wall clock or hidden randomness.
use std::io::{Error, ErrorKind};
pub mod loading;
pub mod systems;
use tore_formats::{
    Result,
    weapons::{Movement, Zone},
};

fn invalid(message: &str) -> Error {
    Error::new(ErrorKind::InvalidData, message)
}

/// FA PROJSpeed 0x4c1120. Returns integer feet/second, not Q8 speed.
pub fn launch_speed(m: &Movement, launcher_speed_f8: i32) -> Result<i32> {
    if m.minimum_speed > m.maximum_speed {
        return Err(invalid("inverted projectile speed limits"));
    }
    let inherited = (launcher_speed_f8 >> 8).wrapping_mul(i32::from(m.launch_retard)) / 100;
    Ok(inherited
        .max(i32::from(m.initial_speed))
        .clamp(i32::from(m.minimum_speed), i32::from(m.maximum_speed)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnginePhase {
    BeforeIgnition,
    Powered,
    Coast,
}

/// FA 0x4c1170: signed wrapping age compared with unsigned source words.
/// fuelT is a launch-relative cutoff here, not added to igniteT by this consumer.
pub fn engine_phase(m: &Movement, current_t: u16, launched_t: u16) -> EnginePhase {
    let age = i32::from(current_t.wrapping_sub(launched_t) as i16);
    if age < i32::from(m.ignite_t) {
        EnginePhase::BeforeIgnition
    } else if age < i32::from(m.fuel_t) {
        EnginePhase::Powered
    } else {
        EnginePhase::Coast
    }
}

/// FA 0x4c1215 and 0x4c1227 use unsigned age and signed Q8 height respectively.
pub fn removal_due(m: &Movement, current_t: u16, launched_t: u16, height_f8: i32) -> bool {
    current_t.wrapping_sub(launched_t) >= m.remove_t || height_f8 > 0x0186_a000
}

/// FA 0x477da0. Altitude is quantized in 256-ft bands, then percentages are
/// interpolated with TWO integer divisions. 78/156 bands are 19,968/39,936 ft.
pub fn altitude_performance(base: i16, at_0: u8, at_20: u8, altitude_f8: i32) -> i32 {
    let band = i32::from((altitude_f8 >> 16) as i16);
    let percent = if band >= 156 {
        100
    } else if band >= 78 {
        let fraction = ((band - 78) * 100 / 78) as i16;
        i32::from(at_20) + (100 - i32::from(at_20)) * i32::from(fraction) / 100
    } else {
        let fraction = (band * 100 / 78) as i16;
        i32::from(at_0) + (i32::from(at_20) - i32::from(at_0)) * i32::from(fraction) / 100
    };
    i32::from(base) * i32::from(percent as i16) / 100
}

/// FA PROJMoveProc's 0x40 speed-command branch; generic velocity integration
/// remains a separate caller stage. No invented missile drag law is applied.
pub fn commanded_speed(m: &Movement, phase: EnginePhase, speed_f8: i32, altitude_f8: i32) -> i32 {
    match phase {
        EnginePhase::BeforeIgnition => speed_f8 >> 8,
        EnginePhase::Powered => altitude_performance(
            m.maximum_speed,
            m.performance_at_0,
            m.performance_at_20,
            altitude_f8,
        ),
        EnginePhase::Coast => i32::from(m.final_speed),
    }
}

/// Generic object speed approach, FA 0x438070..0x4380b0. The braking flag
/// belongs to the caller's command; it is not inferred from a weapon class.
pub fn axial_speed(
    m: &Movement,
    current_f8: i32,
    target_fps: i16,
    braking: bool,
    service_ticks: i16,
) -> Result<i32> {
    if service_ticks < 0 || m.acceleration < 0 || m.deceleration < 0 {
        return Err(invalid("invalid axial service input"));
    }
    let target = i32::from(target_fps) << 8;
    let rate = if target > current_f8 {
        m.acceleration.checked_mul(256)
    } else {
        m.deceleration
            .checked_mul(256)
            .map(|r| if braking { r.max(0x9600) } else { r })
    }
    .ok_or_else(|| invalid("axial acceleration exceeds fixed8 domain"))?;
    Ok(tore_formats::flight_model::match_f24(
        current_f8,
        target,
        rate,
        service_ticks,
    ))
}

/// FA 0x4120c0 positive-distance helper. Native X/Y(height)/Z coordinates and
/// binary angles; the caller supplies the extracted native trigonometry table.
pub fn advance_position(
    position_f8: [i32; 3],
    heading: i16,
    pitch: i16,
    distance_f8: i32,
    trig: &tore_formats::flight_model::rotation::TrigTable,
) -> [i32; 3] {
    use tore_formats::flight_model::rotation::rotate_xz;
    if distance_f8 <= 0 {
        return position_f8;
    }
    let mut delta = [0, 0, distance_f8];
    if pitch != 0 {
        delta = rotate_xz(delta, trig.sin_cos(pitch));
        delta[1] = delta[0];
        delta[0] = 0;
    }
    if heading != 0 {
        delta = rotate_xz(delta, trig.sin_cos(heading));
    }
    std::array::from_fn(|i| position_f8[i].wrapping_add(delta[i]))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerTrigger {
    pub next_t: u16,
    pub was_held: bool,
}
impl PlayerTrigger {
    /// FA 0x416f3b..0x416f74, for an already resolved selected weapon.
    /// Bay, target and firing gates follow in the caller, even if they reject it.
    pub fn poll(&mut self, held: bool, flags: u32, burst_t: u8, current_t: u16) -> bool {
        let due = held
            && if flags & 0x800 != 0 {
                self.next_t <= current_t
            } else {
                !self.was_held
            };
        self.was_held = held;
        if due {
            self.next_t = current_t.wrapping_add(u16::from(burst_t));
        }
        due
    }
    /// Authored host interruption: clear held input, retain the native deadline.
    pub fn release(&mut self) {
        self.was_held = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FallState {
    pub velocity_f8: u16,
}
impl FallState {
    /// FA 0x4c14cf..0x4c1569. This subtracts fall from position independently
    /// of generic axial motion; unsigned word wrap/clamp is intentional.
    pub fn advance(
        &mut self,
        gravity_flag: bool,
        phase: EnginePhase,
        service_ticks: i16,
        height_f8: i32,
    ) -> Result<i32> {
        if service_ticks < 0 {
            return Err(invalid("negative combat service time"));
        }
        if !gravity_flag || phase == EnginePhase::Powered {
            self.velocity_f8 = 0;
            return Ok(height_f8);
        }
        let ticks = i32::from(service_ticks);
        let delta = ticks.wrapping_shl(13) / 256;
        self.velocity_f8 = self.velocity_f8.wrapping_add(delta as u16).min(0x5000);
        let displacement = i32::from(self.velocity_f8 as i16).wrapping_mul(ticks) / 256;
        Ok(height_f8.wrapping_sub(displacement))
    }
}

/// FA HARDUnload 0x452814..0x452856, after pointer resolution. Zero request
/// clears; 0x7fff means unlimited. A partial last debit SUCCEEDS. High flag stays.
/// Caller must perform HARDSetFlags/weight updates after a successful operation.
pub fn unload(count_word: &mut u16, requested: u16) -> bool {
    let count = *count_word & 0x7fff;
    let (remaining, accepted) = if requested == 0 {
        (0, true)
    } else if count == 0x7fff {
        (count, true)
    } else if count == 0 {
        (0, false)
    } else {
        (count.saturating_sub(requested), true)
    };
    *count_word = (*count_word & 0x8000) | remaining;
    accepted
}

/// FA PROJRadarIsOn 0x4c2eb0. Input flags come from the resolved target instance,
/// not a guess based on weapon name. The AI call may extend its emitting deadline.
pub fn radar_is_on(
    human: bool,
    instance_radar_flag: bool,
    until_t: &mut u16,
    current_t: u16,
    requested_t: i16,
) -> bool {
    if human {
        return *until_t > current_t;
    }
    if !instance_radar_flag {
        return false;
    }
    if i32::from(current_t) + i32::from(requested_t) > i32::from(*until_t) {
        *until_t = current_t.wrapping_add(requested_t as u16);
    }
    true
}

/// Final FA FOV decision (0x4c2ad1), after native coordinate/angle production.
/// Inputs are normalized nonnegative native angles; do not feed radians/degrees.
pub fn angular_gate(zone: &Zone, heading_abs: i16, pitch_abs: i16) -> Result<bool> {
    if heading_abs < 0 || pitch_abs < 0 || zone.heading < 0 || zone.pitch < 0 {
        return Err(invalid("FOV requires normalized nonnegative source angles"));
    }
    if zone.heading == 0x7fff && zone.pitch == 0x7fff {
        return Ok(true);
    }
    Ok(if zone.pitch <= 0x3ffc {
        heading_abs <= zone.heading && pitch_abs <= zone.pitch
    } else {
        heading_abs <= zone.heading.max(0x3ffc)
            || i32::from(pitch_abs) >= 0x7ff8 - i32::from(zone.pitch)
    })
}

/// The range/relative-altitude subcase of PROJInFOV. Its native geometry and
/// predicted-range producer must be supplied explicitly; this is not a detector.
pub fn range_gate(
    zone: &Zone,
    distance_f8: i32,
    predicted_distance_f8: i32,
    relative_height_f8: i32,
) -> Result<bool> {
    let scale = |v: i32| {
        v.checked_mul(256)
            .ok_or_else(|| invalid("zone distance exceeds native fixed8 domain"))
    };
    if zone.minimum_range < 0 || zone.minimum_range > zone.maximum_range {
        return Err(invalid("invalid range gate"));
    }
    let min = scale(zone.minimum_range)?;
    let max = scale(zone.maximum_range)?;
    let high =
        zone.maximum_altitude == i32::MAX || relative_height_f8 <= scale(zone.maximum_altitude)?;
    let low =
        zone.minimum_altitude == i32::MIN || relative_height_f8 >= scale(zone.minimum_altitude)?;
    Ok(high && low && distance_f8 >= min && distance_f8 <= max && predicted_distance_f8 <= max)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn movement() -> Movement {
        Movement {
            minimum_speed: 10,
            corner_speed: 1000,
            maximum_speed: 1000,
            acceleration: 5,
            deceleration: 2,
            initial_speed: 200,
            final_speed: 80,
            launch_retard: 50,
            ignite_t: 4,
            fuel_t: 12,
            remove_t: 20,
            powered_turn_rate: 0,
            unpowered_turn_rate: 0,
            performance_at_0: 50,
            performance_at_20: 75,
            cruise: [0; 4],
            jink: [0; 3],
        }
    }
    #[test]
    fn speed_is_scalar_clamped_and_does_not_add_muzzle_velocity() {
        let m = movement();
        assert_eq!(launch_speed(&m, 600 * 256).unwrap(), 300);
        assert_eq!(launch_speed(&m, 100 * 256).unwrap(), 200);
        assert_eq!(launch_speed(&m, 3000 * 256).unwrap(), 1000);
    }
    #[test]
    fn ignition_burnout_expiry_and_counter_wrap_are_distinct() {
        let m = movement();
        let launch = 65530;
        assert_eq!(engine_phase(&m, 65533, launch), EnginePhase::BeforeIgnition);
        assert_eq!(engine_phase(&m, 65534, launch), EnginePhase::Powered);
        assert_eq!(engine_phase(&m, 6, launch), EnginePhase::Coast);
        assert!(!removal_due(&m, 13, launch, 0));
        assert!(removal_due(&m, 14, launch, 0));
        assert!(removal_due(&m, launch, launch, 0x186a001));
    }
    #[test]
    fn performance_uses_quantized_bands_and_two_rounding_stages() {
        let m = movement();
        assert_eq!(commanded_speed(&m, EnginePhase::Powered, 0, 0), 500);
        assert_eq!(commanded_speed(&m, EnginePhase::Powered, 0, 78 << 16), 750);
        assert_eq!(
            commanded_speed(&m, EnginePhase::Powered, 0, 156 << 16),
            1000
        );
        assert_eq!(altitude_performance(1000, 50, 75, 1 << 16), 500);
        assert_eq!(altitude_performance(1000, 50, 75, (1 << 16) + 65535), 500);
    }
    #[test]
    fn fall_accumulates_caps_and_resets_when_powered() {
        let mut f = FallState::default();
        let mut h = 1000 * 256;
        h = f.advance(true, EnginePhase::Coast, 256, h).unwrap();
        assert_eq!(h, 968 * 256);
        h = f.advance(true, EnginePhase::Coast, 256, h).unwrap();
        assert_eq!(h, 904 * 256);
        h = f.advance(true, EnginePhase::Coast, 256, h).unwrap();
        assert_eq!(h, 824 * 256);
        assert_eq!(f.advance(true, EnginePhase::Powered, 256, h).unwrap(), h);
        assert_eq!(f.velocity_f8, 0);
        assert!(f.advance(true, EnginePhase::Coast, -1, h).is_err());
    }
    #[test]
    fn ammo_partial_last_round_unlimited_and_flag_preservation() {
        let mut count = 0x8001;
        assert!(unload(&mut count, 2));
        assert_eq!(count, 0x8000);
        assert!(!unload(&mut count, 2));
        count = 0xffff;
        assert!(unload(&mut count, 2));
        assert_eq!(count, 0xffff);
        assert!(unload(&mut count, 0));
        assert_eq!(count, 0x8000);
    }
    #[test]
    fn radar_deadline_uses_native_human_ai_branches() {
        let mut until = 10;
        assert!(!radar_is_on(true, true, &mut until, 10, 5));
        assert_eq!(until, 10);
        assert!(!radar_is_on(false, false, &mut until, 10, 5));
        assert!(radar_is_on(false, true, &mut until, 10, 5));
        assert_eq!(until, 15);
        assert!(radar_is_on(true, false, &mut until, 14, 0));
    }
    #[test]
    fn trigger_repeats_only_for_native_repeat_flag_and_retains_deadline() {
        let mut t = PlayerTrigger::default();
        assert!(t.poll(true, 0, 1, 0));
        assert!(!t.poll(true, 0, 1, 1));
        assert!(!t.poll(false, 0, 1, 2));
        assert!(t.poll(true, 0, 1, 2));
        assert!(!t.poll(true, 0x800, 1, 2));
        assert!(t.poll(true, 0x800, 1, 3));
        t.release();
        assert_eq!(t.next_t, 4);
        assert!(!t.poll(true, 0x800, 1, 3));
    }
    #[test]
    fn axial_approach_uses_native_rates_and_cannot_overshoot_command() {
        let m = movement();
        assert_eq!(
            axial_speed(&m, 100 * 256, 200, false, 256).unwrap(),
            105 * 256
        );
        assert_eq!(
            axial_speed(&m, 100 * 256, 99, false, 256).unwrap(),
            99 * 256
        );
        assert_eq!(axial_speed(&m, 200 * 256, 0, true, 256).unwrap(), 50 * 256);
    }
    #[test]
    fn range_and_angle_boundaries_do_not_imply_detection() {
        let mut z = Zone {
            heading: 100,
            pitch: 100,
            minimum_range: 10,
            maximum_range: 100,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        assert!(range_gate(&z, 10 * 256, 100 * 256, 0).unwrap());
        assert!(!range_gate(&z, 10 * 256 - 1, 100 * 256, 0).unwrap());
        assert!(!range_gate(&z, 20 * 256, 100 * 256 + 1, 0).unwrap());
        assert!(angular_gate(&z, 100, 100).unwrap());
        assert!(!angular_gate(&z, 101, 100).unwrap());
        z.pitch = 0x6000;
        assert!(angular_gate(&z, 0x3ffc, 0).unwrap());
        assert!(!angular_gate(&z, 0x4000, 0x1ff7).unwrap());
        assert!(angular_gate(&z, 0x4000, 0x1ff8).unwrap());
    }
}

pub mod live;
