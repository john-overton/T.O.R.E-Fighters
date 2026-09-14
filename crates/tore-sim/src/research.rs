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
        departure::{self, DepartureMode, SpinInput, StallState},
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
#[derive(Clone, Debug, PartialEq)]
pub struct Research {
    pub departure: StallState,
    pub spinning: i8,
    pub recovery_ticks: i32,
    pub spin_intensity_f8: i32,
    pub on_ground: bool,
    pub clock: FixedClock,
    pub rng: NativeRng,
    pub elapsed: i32,
}
impl Research {
    pub fn new(seed: i32) -> Result<Self> {
        Ok(Self {
            departure: StallState::default(),
            spinning: 0,
            recovery_ticks: 0,
            spin_intensity_f8: 0,
            on_ground: false,
            clock: FixedClock::default(),
            rng: NativeRng::seeded(seed)?,
            elapsed: 0,
        })
    }
    /// Source stall-state/entry/recovery predicates; continuous spin forces fitted.
    #[allow(clippy::too_many_arguments)] // Explicit independent native inputs.
    pub fn advance(
        &mut self,
        c: &crate::models::config::Configuration,
        speed: f64,
        stall: f64,
        pitch: f64,
        rudder: f64,
        throttle: f64,
        bank: f64,
        roll_rate: f64,
    ) {
        let ticks = self.clock.advance(false);
        self.elapsed = self.elapsed.wrapping_add(ticks as i32);
        let input = SpinInput {
            pitch_stick: (pitch * 256.) as i32,
            rudder: (rudder * 256.) as i32,
            throttle_f8: (throttle * 25600.) as i32,
            speed_f8: (speed * 256.) as i32,
            clean_stall_fps: stall as i32,
            thrust_vector_f8: 0,
            inhibited: self.on_ground,
        };
        if self.spinning != 0 {
            let command = input.rudder * self.spinning as i32;
            if command.abs() >= 200 {
                self.spin_intensity_f8 = tore_formats::flight_model::match_f24(
                    self.spin_intensity_f8,
                    if command > 0 { 25600 } else { 0 },
                    25 * 256,
                    ticks,
                );
            }
            if departure::spin_recovery(&c.native.departure, input, self.spinning, false) {
                self.recovery_ticks += ticks as i32;
            } else {
                self.recovery_ticks = 0;
            }
            let delay = if c.native.departure.spin_exit == -2 {
                256
            } else {
                768
            };
            if self.recovery_ticks >= delay {
                self.spinning = 0;
                self.departure = StallState::default();
                self.recovery_ticks = 0;
            }
        } else {
            // All inputs are bounded and ticks is positive by construction.
            self.departure
                .advance(
                    &c.native.departure,
                    speed < stall,
                    speed < stall && !self.on_ground,
                    c.native.extended_warning,
                    self.on_ground,
                    ticks,
                )
                .expect("positive fixed time");
            if matches!(
                self.departure.mode,
                DepartureMode::Warning | DepartureMode::Stalled
            ) {
                // Source tie draw occurs only for exactly zero bank and roll rate.
                let random = bank == 0.
                    && roll_rate == 0.
                    && self.rng.chance(50).expect("bounded generator");
                let direction = departure::spin_direction(
                    (roll_rate.to_degrees() * 256.) as i32,
                    (bank.to_degrees() * 65536. / 360.) as i16,
                    random,
                );
                if departure::spin_entry(&c.native.departure, self.departure.mode, input, direction)
                {
                    self.spinning = direction;
                    self.departure.mode = DepartureMode::Spinning;
                }
            }
        }
    }
    pub fn spin_yaw_rate(&self, c: &crate::models::config::Configuration) -> f64 {
        let low = c.native.departure.spin_yaw[0] as f64;
        let high = c.native.departure.spin_yaw[1] as f64;
        (low + (high - low) * self.spin_intensity_f8 as f64 / 25600.).to_radians()
            * self.spinning as f64
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
