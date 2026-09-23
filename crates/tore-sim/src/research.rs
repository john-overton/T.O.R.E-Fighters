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
        ground::{LandingLimits, LandingSeverity, landing_severity},
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
    /// Graded touchdowns, for the debrief's landing grade.
    pub landings: Landings,
}

/// Touchdowns after real flight, each scored 100 (good) or 50 (fair). A
/// touchdown outside the aircraft's landing limits is a crash, not a grade.
/// Fitted: see docs/spec/debrief.md#landing-grade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Landings {
    pub count: u32,
    pub score: u32,
    airborne_ticks: u32,
}
impl Landings {
    /// Airborne time that separates a landing from a bounce or a spawn.
    const FLIGHT_TICKS: u32 = 5 * 120;
    /// Average score as a whole percentage, if any landing was graded.
    pub fn grade(&self) -> Option<u32> {
        (self.count > 0).then(|| self.score / self.count)
    }
    fn airborne(&mut self) {
        self.airborne_ticks = self.airborne_ticks.saturating_add(1);
    }
    fn touchdown(&mut self, limits: LandingLimits, descent_fps: f64, bank_degrees: f64) {
        if self.airborne_ticks >= Self::FLIGHT_TICKS {
            let gentle = descent_fps <= f64::from(limits.descent_fps) / 2.
                && bank_degrees.abs() <= f64::from(limits.roll_degrees) / 2.;
            self.count += 1;
            self.score += if gentle { 100 } else { 50 };
        }
        self.airborne_ticks = 0;
    }
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
            landings: Landings::default(),
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
        spins_allowed: bool,
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
        if !spins_allowed && self.spinning != 0 {
            // No spins turned on mid-spin: the rotation damps out as a stall.
            self.spinning = 0;
            self.departure = StallState {
                mode: DepartureMode::Stalled,
                elapsed: 0,
            };
        }
        if spins_allowed
            && self.spinning == 0
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
        wheel_load_fraction: f64,
        runway_wind_fraction: f64,
        previous_position: [f64; 3],
    ) {
        let floor = surface.height + c.equipment.ground_clearance_ft; // Fitted wheel/CG clearance; not recovered geometry.
        let wheel_load = wheel_load_fraction.clamp(0., 1.);
        // A surface drop removes support regardless of aerodynamic load. This
        // clearance is only a geometry tolerance, not the liftoff criterion.
        if self.on_ground && s.position[1] > floor + 0.05 {
            self.on_ground = false;
            return;
        }
        // Fitted wheel-unloading hysteresis. Once released, contact state stays
        // authoritative until the aircraft descends back to the support plane.
        if self.on_ground && s.position[1] >= floor && wheel_load <= 0.02 && s.velocity[1] > 0.1 {
            self.on_ground = false;
            return;
        }
        if !self.on_ground && s.position[1] > floor {
            self.landings.airborne();
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
            && s.cheats.no_crashes
        {
            s.ricochet(floor);
            return;
        } else if !self.on_ground
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
            if !self.on_ground {
                self.landings
                    .touchdown(c.native.landing, -s.velocity[1], s.bank.to_degrees());
            }
            self.on_ground = true;
            s.position[1] = floor;
            s.velocity[1] = s.velocity[1].max(0.);
            // Fitted tire contact: lateral scrub, rolling resistance and wheel brakes.
            let tuning = c.tuning;
            let v = (s.velocity[0] * s.velocity[0] + s.velocity[2] * s.velocity[2]).sqrt();
            let wind_grip = 1. - 0.5 * runway_wind_fraction.clamp(0., 1.);
            let scrub = (tuning.tire_scrub_rate * wheel_load * wind_grip * DT).min(1.);
            for i in [0, 2] {
                s.velocity[i] -= basis.right[i] * side * scrub;
            }
            let decel = if s.brake_out {
                tuning.brake_deceleration
            } else {
                tuning.rolling_deceleration
            } * wheel_load;
            let factor = (1. - decel * DT / v.max(0.01)).max(0.);
            for i in [0, 2] {
                s.velocity[i] *= factor;
                // Position was integrated before contact. Reapply this tick's
                // supported movement using the post-tire velocity.
                s.position[i] = previous_position[i] + s.velocity[i] * DT;
            }
            if s.brake_out && v <= decel * DT {
                // Static wheel brakes hold a parked aircraft against forces
                // accumulated during this tick.
                for i in [0, 2] {
                    s.position[i] = previous_position[i];
                    s.velocity[i] = 0.;
                }
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
    #[test]
    fn landings_after_real_flight_are_graded_and_bounces_are_not() {
        let limits = LandingLimits {
            forward_fps: 300,
            side_fps: 10,
            descent_fps: 20,
            pitch_degrees: 15,
            roll_degrees: 10,
        };
        let mut landings = Landings::default();
        // Spawning on the runway is not a landing.
        landings.touchdown(limits, 0., 0.);
        assert_eq!(landings.grade(), None);
        let fly = |l: &mut Landings, ticks| (0..ticks).for_each(|_| l.airborne());
        fly(&mut landings, Landings::FLIGHT_TICKS);
        landings.touchdown(limits, 8., 2.);
        assert_eq!(landings.grade(), Some(100));
        // A short hop after touchdown is a bounce, not a second landing.
        fly(&mut landings, 60);
        landings.touchdown(limits, 15., 0.);
        assert_eq!(landings.count, 1);
        fly(&mut landings, Landings::FLIGHT_TICKS);
        landings.touchdown(limits, 15., 0.);
        assert_eq!(landings.grade(), Some(75));
    }
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
    fn contact_state(c: &crate::models::config::Configuration) -> State {
        let mut s = State::new(&profile(), [0., c.equipment.ground_clearance_ft, 0.]).unwrap();
        s.gear = 1.;
        s.gear_down = true;
        s.velocity = [0.; 3];
        s.speed = 0.;
        s
    }

    #[test]
    fn unloaded_wheels_release_gently_and_do_not_reattach_above_floor() {
        let c = config();
        let mut s = contact_state(&c);
        let mut r = Research::new(1).unwrap();
        r.on_ground = true;
        s.velocity[1] = 0.11;
        s.position[1] += s.velocity[1] * DT;
        let previous = [
            s.position[0],
            s.position[1] - s.velocity[1] * DT,
            s.position[2],
        ];
        r.contact(&mut s, Surface::runway(0.), &c, 0.02, 0., previous);
        assert!(!r.on_ground);
        let released_height = s.position[1];

        let previous = s.position;
        r.contact(&mut s, Surface::runway(0.), &c, 1., 0., previous);
        assert!(!r.on_ground);
        assert_eq!(s.position[1], released_height);
    }

    #[test]
    fn surface_drop_releases_contact_without_snapping_down() {
        let c = config();
        let mut s = contact_state(&c);
        let integrated_height = s.position[1];
        let previous = s.position;
        let mut r = Research::new(1).unwrap();
        r.on_ground = true;
        r.contact(&mut s, Surface::runway(-10.), &c, 1., 0., previous);
        assert!(!r.on_ground);
        assert_eq!(s.position[1], integrated_height);
        assert_eq!(s.velocity[1], 0.);
    }

    #[test]
    fn rising_surface_resolves_before_wheel_unload_release() {
        let c = config();
        let mut s = contact_state(&c);
        s.velocity[1] = 0.11;
        let previous = s.position;
        let mut r = Research::new(1).unwrap();
        r.on_ground = true;
        r.contact(&mut s, Surface::runway(1.), &c, 0., 0., previous);
        assert!(r.on_ground);
        assert_eq!(s.position[1], 1. + c.equipment.ground_clearance_ft);
        assert!(s.position[1] >= 1. + c.equipment.ground_clearance_ft);
    }

    #[test]
    fn wheel_load_scales_rolling_resistance() {
        let c = config();
        let mut full = contact_state(&c);
        full.yaw = 0.;
        full.velocity = [0., 0., 100.];
        full.position[2] = 100. * DT;
        let mut light = full.clone();
        let mut full_contact = Research::new(1).unwrap();
        full_contact.on_ground = true;
        let mut light_contact = full_contact.clone();
        full_contact.contact(&mut full, Surface::runway(0.), &c, 1., 0., [0.; 3]);
        light_contact.contact(&mut light, Surface::runway(0.), &c, 0.5, 0., [0.; 3]);
        assert!(light.speed > full.speed);
        assert!((light.speed - full.speed - c.tuning.rolling_deceleration * 0.5 * DT).abs() < 1e-9);
        assert_eq!(full.position[2], full.velocity[2] * DT);
        assert_eq!(light.position[2], light.velocity[2] * DT);
    }

    #[test]
    fn runway_wind_reduces_only_lateral_tire_grip() {
        let c = config();
        let mut calm = contact_state(&c);
        calm.yaw = 0.;
        calm.velocity = [10., 0., 100.];
        calm.position = [10. * DT, calm.position[1], 100. * DT];
        let mut wind = calm.clone();
        let mut calm_contact = Research::new(1).unwrap();
        calm_contact.on_ground = true;
        let mut wind_contact = calm_contact.clone();
        calm_contact.contact(&mut calm, Surface::runway(0.), &c, 1., 0., [0.; 3]);
        wind_contact.contact(&mut wind, Surface::runway(0.), &c, 1., 1., [0.; 3]);
        assert!(wind.velocity[0].abs() > calm.velocity[0].abs());
        assert_eq!(wind.velocity[2], calm.velocity[2]);
    }

    #[test]
    fn brakes_hold_against_sub_tick_creep() {
        let c = config();
        let mut s = contact_state(&c);
        s.brake_out = true;
        s.velocity = [0.01, 0., 0.02];
        let previous = [12., s.position[1], 34.];
        s.position = [
            previous[0] + s.velocity[0] * DT,
            previous[1],
            previous[2] + s.velocity[2] * DT,
        ];
        let mut r = Research::new(1).unwrap();
        r.on_ground = true;
        r.contact(&mut s, Surface::runway(0.), &c, 1., 0., previous);
        assert_eq!([s.position[0], s.position[2]], [12., 34.]);
        assert_eq!([s.velocity[0], s.velocity[2]], [0., 0.]);
    }

    #[test]
    fn no_crashes_bounces_every_unsafe_touchdown_but_lands_a_safe_one() {
        let c = config();
        for (surface, gear, vertical_speed) in [
            (Surface::runway(0.), 0., -30.),
            (
                Surface {
                    water: true,
                    ..Surface::runway(0.)
                },
                1.,
                -30.,
            ),
            (Surface::runway(0.), 1., -100.),
        ] {
            let mut s = contact_state(&c);
            s.cheats.no_crashes = true;
            s.gear = gear;
            s.velocity = [400., vertical_speed, 0.];
            let mut r = Research::new(1).unwrap();
            let previous = s.position;
            r.contact(&mut s, surface, &c, 1., 0., previous);
            assert!(!s.crashed && !r.on_ground);
            assert!(s.velocity[1] >= 20.);
            assert_eq!(s.velocity[0], 400.);
        }
        let mut s = contact_state(&c);
        s.cheats.no_crashes = true;
        let mut r = Research::new(1).unwrap();
        let previous = s.position;
        r.contact(&mut s, Surface::runway(0.), &c, 1., 0., previous);
        assert!(r.on_ground && !s.crashed);
    }
    #[test]
    fn touchdown_keeps_safety_classification() {
        let c = config();
        for (surface, gear, vertical_speed, crashes) in [
            (Surface::runway(0.), 1., 0., false),
            (Surface::runway(0.), 0., 0., true),
            (
                Surface {
                    water: true,
                    ..Surface::runway(0.)
                },
                1.,
                0.,
                true,
            ),
            (Surface::runway(0.), 1., -100., true),
        ] {
            let mut s = contact_state(&c);
            s.gear = gear;
            s.velocity[1] = vertical_speed;
            let mut r = Research::new(1).unwrap();
            let previous = s.position;
            r.contact(&mut s, surface, &c, 1., 0., previous);
            assert_eq!(s.crashed, crashes);
        }
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
                r.advance(&c, 190., 200., 1., rudder, 0., -0.1, 0., 1., true);
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
            r.advance(&c, speed, 200., 1., 1., 0., -0.1, 0., 1., true);
            assert!((r.spin_rate.to_degrees() / DT - expected_acceleration).abs() < 1e-8);
        }
        assert_eq!(departure_drive(100., 200., 1., 2), 0.);
        assert_eq!(departure_drive(190., 200., 0., 0), 0.);
        assert!(departure_drive(190., 200., 0.001, 0) > 0.);
        assert!(departure_drive(190., 200., 0.1, 0) < departure_drive(190., 200., 0.5, 0));
    }

    #[test]
    fn no_spins_blocks_entry_and_ends_a_spin_as_a_stall() {
        let mut c = config();
        c.native.departure.spin_yaw = [120, 180];
        let mut r = Research::new(1).unwrap();
        r.departure.mode = DepartureMode::Warning;
        r.advance(&c, 190., 200., 1., 1., 0., -0.1, 0., 1., false);
        assert_eq!(r.spinning, 0);
        let mut r = spinning(1.);
        r.advance(&c, 180., 200., 1., 1., 0., 0., 0., 1., false);
        assert_eq!(r.spinning, 0);
        assert!(r.spin_rate < 1., "the rotation damps out");
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
                    r.advance(
                        &c,
                        180.,
                        200.,
                        1.,
                        rudder * direction,
                        0.,
                        0.,
                        0.,
                        0.5,
                        true,
                    );
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
                    r.advance(&c, 180., 200., 1., direction, 0., 0., 0., 1., true);
                }
                let mut ticks = 0;
                while r.spinning != 0 && ticks < 1200 {
                    r.advance(&c, 180., 200., -1., -direction, 0., 0., 0., 1., true);
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
                r.advance(
                    &c,
                    500.,
                    200.,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                    angle.to_radians().cos(),
                    true,
                );
                assert_eq!(r.spinning == 0, clears);
                if clears {
                    assert_eq!(r.departure.mode, DepartureMode::Normal);
                    assert!(r.spin_rate.abs() > 0.);
                    let previous = r.spin_rate.abs();
                    r.advance(&c, 500., 200., 0., 0., 0., 0., 0., 1., true);
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
        r.advance(&c, 100., 200., 0., 0., 0., 0.1, 0., 1., true);
        assert!(r.stall_active && r.severity_f8 > 0);
        assert!(r.departure.elapsed > 1024);
        r.on_ground = true;
        r.advance(&c, 100., 200., 1., 1., 1., 0., 0., 1., true);
        assert_eq!(r.departure, StallState::default());
        assert!(!r.stall_active);
        assert_eq!(r.severity_f8, 0);
    }
}
