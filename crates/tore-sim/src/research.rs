//! Working hybrid dynamics: recovered departure/contact contracts with explicitly
//! fitted continuous force coupling. This is not a native integer-tick emulator.
use crate::{
    attitude::{Basis, dot},
    flight::{DT, State},
};
use tore_formats::{
    Result,
    flight_model::{
        clock_rng::{FixedClock, NativeRng},
        departure::{self, DepartureMode, StallState},
        ground::{LandingSeverity, landing_severity},
    },
};
#[derive(Clone, Copy, Debug)]
pub struct Surface {
    pub height: f64,
    pub water: bool,
    pub landable: bool,
    /// World wind in feet/second; uniform during each fixed step.
    pub wind: [f64; 3],
}
impl Surface {
    pub fn terrain(height: f64) -> Self {
        Self {
            height,
            wind: [0.; 3],
            water: false,
            landable: false,
        }
    }
    pub fn runway(height: f64) -> Self {
        Self {
            height,
            wind: [0.; 3],
            water: false,
            landable: true,
        }
    }
}
/// Fitted flow-dependent spin damping, independent of pilot commands.
fn forward_stability(speed: f64, stall: f64, airflow_forward: f64) -> f64 {
    fn smooth(value: f64) -> f64 {
        let t = value.clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
    let alignment = smooth(
        (airflow_forward - 45_f64.to_radians().cos())
            / (25_f64.to_radians().cos() - 45_f64.to_radians().cos()),
    );
    let pressure = smooth((speed / stall.max(1.) - 1.1) / 0.4);
    alignment * pressure
}

/// Fitted continuous drive severity; braking authority is not multiplied by this.
fn departure_drive(speed: f64, stall: f64, pitch: f64, entry: i16) -> f64 {
    fn smooth(value: f64) -> f64 {
        let t = value.clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
    if entry == 2 {
        return 0.;
    }
    smooth((1. - speed / stall.max(1.)) / 0.25)
        * smooth(pitch / 0.5)
        * if entry == 1 { 0.5 } else { 1. }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Research {
    pub departure: StallState,
    pub spinning: i8,
    /// Signed uncommanded yaw velocity, rad/s. See docs/spec/spin-transitions.md.
    pub spin_rate: f64,
    pub on_ground: bool,
    /// Applied this tick, before the native stall timer is advanced.
    pub severity_f8: i32,
    pub stall_active: bool,
    pub clock: FixedClock,
    pub rng: NativeRng,
    pub elapsed: i32,
}
impl Research {
    pub fn new(seed: i32) -> Result<Self> {
        Ok(Self {
            departure: StallState::default(),
            spinning: 0,
            spin_rate: 0.,
            on_ground: false,
            severity_f8: 0,
            stall_active: false,
            clock: FixedClock::default(),
            rng: NativeRng::seeded(seed)?,
            elapsed: 0,
        })
    }
    /// Source stall warning rules with fitted soft entry and spin dynamics.
    #[allow(clippy::too_many_arguments)] // Explicit independent native inputs.
    pub fn advance(
        &mut self,
        c: &crate::models::config::Configuration,
        speed: f64,
        stall: f64,
        pitch: f64,
        rudder: f64,
        _throttle: f64,
        bank: f64,
        roll_rate: f64,
        airflow_forward: f64,
    ) {
        let ticks = self.clock.advance(false);
        self.elapsed = self.elapsed.wrapping_add(ticks as i32);
        self.severity_f8 = 0;
        self.stall_active = false;
        if self.on_ground {
            self.departure = StallState::default();
            self.spinning = 0;
            self.spin_rate = 0.;
            return;
        }
        // Warning eligibility/direction are source-derived; torque onset is fitted.
        let drive = departure_drive(speed, stall, pitch, c.native.departure.spin_entry);
        if self.spinning == 0
            && drive > 0.
            && matches!(
                self.departure.mode,
                DepartureMode::Warning | DepartureMode::Stalled
            )
        {
            let rate = (roll_rate.to_degrees() * 256.) as i32;
            let bank = (bank.to_degrees() * 65536. / 360.) as i16;
            let random = rate == 0 && bank == 0 && self.rng.chance(50).expect("bounded generator");
            let direction = departure::spin_direction(rate, bank, random);
            if rudder * f64::from(direction) > 0. {
                self.spinning = direction;
                self.departure.mode = DepartureMode::Spinning;
            }
        }
        let q = (speed / stall.max(1.)).powi(2).clamp(0., 4.);
        let stability = forward_stability(speed, stall, airflow_forward);
        let damping = q * (0.35 + 2. * stability);
        if self.spinning != 0 {
            let direction = f64::from(self.spinning);
            let rate = (self.spin_rate * direction).max(0.);
            let torque = 1.5
                * Self::maximum_spin_rate(c)
                * rudder
                * direction
                * q.min(1.)
                * self.surface_effectiveness(c, airflow_forward)
                * if rudder * direction > 0. { drive } else { 1. };
            let acceleration = torque + (0.2 * q * (1. - stability) - damping) * rate;
            self.spin_rate =
                direction * (rate + DT * acceleration).clamp(0., Self::maximum_spin_rate(c));
            if self.spin_rate.abs() <= c.tuning.rudder_rate * q.min(1.)
                && airflow_forward >= 25_f64.to_radians().cos()
                && rudder * direction <= 0.
            {
                self.spinning = 0;
                self.departure = StallState {
                    mode: if speed < stall {
                        DepartureMode::Stalled
                    } else {
                        DepartureMode::Normal
                    },
                    elapsed: 0,
                };
            }
        } else {
            self.spin_rate *= (-damping * DT).exp();
            if self.departure.mode == DepartureMode::Stalled {
                self.stall_active = true;
                self.severity_f8 = departure::stall_severity(
                    &c.native.departure,
                    self.departure.elapsed,
                    speed as i32,
                    (stall as i32).max(1),
                )
                .expect("positive stall speed")
                .clamp(0, 256);
            }
            // Fitted clean-envelope gate; difficulty/VTOL/current-G native setup
            // is not reproduced by this continuous adapter.
            self.departure
                .advance(
                    &c.native.departure,
                    speed < stall,
                    speed < stall,
                    c.native.extended_warning,
                    false,
                    ticks,
                )
                .expect("positive fixed time");
        }
    }
    pub fn maximum_spin_rate(c: &crate::models::config::Configuration) -> f64 {
        (c.native.departure.spin_yaw[1] as f64)
            .to_radians()
            .max(0.01)
    }
    pub fn spin_blend(&self, c: &crate::models::config::Configuration) -> f64 {
        (self.spin_rate.abs() / Self::maximum_spin_rate(c)).clamp(0., 1.)
    }
    pub fn surface_effectiveness(
        &self,
        c: &crate::models::config::Configuration,
        forward: f64,
    ) -> f64 {
        let f = self.spin_blend(c);
        (1. - f) + f * (0.25 + 0.75 * forward.max(0.).powi(2)) / (1. + 2. * f * f)
    }
    pub fn contact(
        &mut self,
        s: &mut State,
        surface: Surface,
        c: &crate::models::config::Configuration,
    ) {
        let floor = surface.height + c.equipment.ground_clearance_ft; // Fitted wheel/CG clearance; not recovered geometry.
        if s.position[1] > floor + 0.05 {
            self.on_ground = false;
            return;
        }
        let basis = Basis::new(s.yaw, s.pitch, s.bank);
        let forward = dot(s.velocity, basis.forward);
        let side = dot(s.velocity, basis.right);
        let severity = landing_severity(
            c.native.landing,
            (s.bank.to_degrees() * 256.) as i32,
            (s.pitch.to_degrees() * 256.) as i32,
            (forward * 256.) as i32,
            (side * 256.) as i32,
            s.velocity[1] as i16,
        );
        if !self.on_ground
            && (surface.water
                || !surface.landable
                || s.gear < 0.99
                || severity != LandingSeverity::WithinLimits)
        {
            s.crashed = true;
            s.engine = false;
            s.burner = false;
            s.velocity = [0.; 3];
            s.speed = 0.;
        } else {
            self.on_ground = true;
            s.position[1] = floor;
            s.velocity[1] = s.velocity[1].max(0.);
            // Fitted tire contact: lateral scrub, rolling resistance and wheel brakes.
            let tuning = c.tuning;
            let scrub = (tuning.tire_scrub_rate * DT).min(1.);
            for i in [0, 2] {
                s.velocity[i] -= basis.right[i] * side * scrub;
            }
            let v = (s.velocity[0] * s.velocity[0] + s.velocity[2] * s.velocity[2]).sqrt();
            let decel = if s.brake_out {
                tuning.brake_deceleration
            } else {
                tuning.rolling_deceleration
            };
            let factor = (1. - decel * DT / v.max(0.01)).max(0.);
            for i in [0, 2] {
                s.velocity[i] *= factor;
            }
            s.bank = 0.;
            s.pitch = s.pitch.clamp(0., 20f64.to_radians());
            if s.elevator <= 0. {
                s.pitch *= 1. - (2. * DT);
            }
            let air = std::array::from_fn(|i| s.velocity[i] - surface.wind[i]);
            s.speed = dot(air, air).sqrt();
        }
        s.position[1] = s.position[1].max(floor);
        s.vertical_speed = s.velocity[1];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{flight::integration_tests::profile, models::FlightModel};
    fn config() -> crate::models::config::Configuration {
        crate::models::AircraftModel::for_aircraft(&profile())
            .unwrap()
            .configuration()
            .clone()
    }
    fn spinning(rate: f64) -> Research {
        let mut r = Research::new(1).unwrap();
        r.spinning = if rate < 0. { -1 } else { 1 };
        r.spin_rate = rate;
        r.departure.mode = DepartureMode::Spinning;
        r
    }
    #[test]
    fn soft_entry_scales_with_depth_and_has_no_old_rudder_step() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        for entry in [0, 1] {
            c.native.departure.spin_entry = entry;
            let mut rates = Vec::new();
            for rudder in [0.001, 0.46775, 0.46875, 0.46975, 0.9365, 0.9375, 0.9385, 1.] {
                let mut r = Research::new(1).unwrap();
                r.departure.mode = DepartureMode::Warning;
                r.advance(&c, 190., 200., 1., rudder, 0., -0.1, 0., 1.);
                assert_eq!(r.spinning, 1);
                rates.push(r.spin_rate);
            }
            assert!(rates.windows(2).all(|p| p[1] > p[0]));
            assert!((rates[3] - rates[1]) < rates[7] * 0.003);
            assert!((rates[6] - rates[4]) < rates[7] * 0.003);
        }
        c.native.departure.spin_entry = 0;
        for (speed, expected_acceleration) in [(200., 0.), (190., 25.3422), (180., 76.9824)] {
            let mut r = Research::new(1).unwrap();
            r.departure.mode = DepartureMode::Warning;
            r.advance(&c, speed, 200., 1., 1., 0., -0.1, 0., 1.);
            assert!((r.spin_rate.to_degrees() / DT - expected_acceleration).abs() < 1e-8);
        }
        assert_eq!(departure_drive(100., 200., 1., 2), 0.);
        assert_eq!(departure_drive(190., 200., 0., 0), 0.);
        assert!(departure_drive(190., 200., 0.001, 0) > 0.);
        assert!(departure_drive(190., 200., 0.1, 0) < departure_drive(190., 200., 0.5, 0));
    }

    #[test]
    fn rudder_accelerates_or_arrests_rotation_proportionally_below_stall() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        for direction in [-1., 1.] {
            let mut rates = Vec::new();
            for rudder in [-1., -0.781, -0.780, -0.001, 0., 0.001, 0.780, 0.781, 1.] {
                let mut r = spinning(direction);
                for _ in 0..12 {
                    r.advance(&c, 180., 200., 1., rudder * direction, 0., 0., 0., 0.5);
                }
                rates.push(r.spin_rate.abs());
            }
            assert!(rates.windows(2).all(|p| p[0] < p[1]));
            assert!(rates[0] < 1. && rates[8] > 1.);
        }
    }
    #[test]
    fn catching_early_is_faster_and_arrest_does_not_clear_a_stall() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        c.tuning.rudder_rate = 0.12;
        for direction in [-1., 1.] {
            let mut catches = Vec::new();
            for wrong_ticks in [5, 120] {
                let mut r = spinning(0.01 * direction);
                for _ in 0..wrong_ticks {
                    r.advance(&c, 180., 200., 1., direction, 0., 0., 0., 1.);
                }
                let mut ticks = 0;
                while r.spinning != 0 && ticks < 1200 {
                    r.advance(&c, 180., 200., -1., -direction, 0., 0., 0., 1.);
                    ticks += 1;
                }
                assert_eq!(r.spinning, 0);
                assert_eq!(r.departure.mode, DepartureMode::Stalled);
                catches.push(ticks);
            }
            assert!(catches[0] < catches[1]);
        }
    }
    #[test]
    fn recovery_threshold_preserves_residual_velocity_and_checks_alignment() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        c.tuning.rudder_rate = 0.12;
        for direction in [-1., 1.] {
            for (rate, angle, clears) in [(0.1, 25_f64, true), (0.1, 25.1, false), (0.5, 0., false)]
            {
                let mut r = spinning(rate * direction);
                r.advance(&c, 500., 200., 0., 0., 0., 0., 0., angle.to_radians().cos());
                assert_eq!(r.spinning == 0, clears);
                if clears {
                    assert_eq!(r.departure.mode, DepartureMode::Normal);
                    assert!(r.spin_rate.abs() > 0.);
                    let previous = r.spin_rate.abs();
                    r.advance(&c, 500., 200., 0., 0., 0., 0., 0., 1.);
                    assert!(r.spin_rate.abs() < previous && r.spin_rate.abs() > 0.);
                }
            }
        }
    }
    #[test]
    fn rotation_reduces_surface_response_without_eliminating_it() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        let slow = spinning(0.1);
        let fast = spinning(3.0);
        for forward in [-1., 0., 0.5, 1.] {
            assert!(fast.surface_effectiveness(&c, forward) > 0.);
            assert!(
                fast.surface_effectiveness(&c, forward) < slow.surface_effectiveness(&c, forward)
            );
        }
    }

    #[test]
    fn severity_uses_preincrement_timer_and_ground_clears_departure() {
        let mut c = config();
        c.native.departure.severity = 256;
        let mut r = Research::new(1).unwrap();
        r.departure = StallState {
            mode: DepartureMode::Stalled,
            elapsed: 1024,
        };
        r.advance(&c, 100., 200., 0., 0., 0., 0.1, 0., 1.);
        assert!(r.stall_active && r.severity_f8 > 0);
        assert!(r.departure.elapsed > 1024);
        r.on_ground = true;
        r.advance(&c, 100., 200., 1., 1., 1., 0., 0., 1.);
        assert_eq!(r.departure, StallState::default());
        assert!(!r.stall_active);
        assert_eq!(r.severity_f8, 0);
    }
}
