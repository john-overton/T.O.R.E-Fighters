//! The rotor drive shared by every rotorcraft (VTOL overhaul design 4.4):
//! the rotor speed `Nr` and the engines that hold it. One [`DriveModel`]
//! serves any number of rotors turning on one interconnected drive with one
//! governor: the AH-64 and Mi-24 (main rotor plus tail rotor), the CH-47
//! (two rotors cross-shafted) and the V-22 (two proprotors cross-shafted
//! through the wing).
//!
//! - **Rotor speed** is one state for the whole drive. It moves with the
//!   difference between the engines' power and the power every rotor (and
//!   anything else on the drive) absorbs, over the rotor energy constant:
//!   `dNr/dt = (P_engine - P_load) / (J Omega0² Nr)`.
//! - **Governor**: the engines aim at the power the load needs plus
//!   `K (Nr_ref - Nr) P_rated`, within what they have
//!   (`P_rated x (rho/rho0)^0.8 x damage x throttle`), and follow with a
//!   lag. `Nr_ref` is [`Drive::rotor_speed_reference`]: 1, or the V-22's
//!   airplane-mode share.
//! - **Rated power** and the **rotor energy** are fixed per aircraft; the
//!   owner derives them from its rotors with [`DriveModel::new`].
//!
//! The drive owns no rotor loads: the owner sums its rotors' power into
//! [`DriveStep::load_power`] and works out torque and the tail rotor itself.

use super::{rotor, state::Drive, state::Warnings};

/// Governor gain: rated power per unit rotor speed error (fitted).
const GOVERNOR_GAIN: f64 = 5.;
/// Engine power lag, seconds (design 4.4: 0.5 to 1 s).
const ENGINE_SECONDS: f64 = 0.6;
/// Turboshaft power lapse: (rho / rho0) to this power (fitted: the AH-64
/// at its maximum weight stops hovering out of ground effect near
/// 4,000 ft, design H13).
const ENGINE_LAPSE_EXPONENT: f64 = 0.8;
/// LOW ROTOR below, ROTOR OVERSPEED above (design 4.4).
pub const LOW_ROTOR: f64 = 0.8;
pub const ROTOR_OVERSPEED: f64 = 1.1;
/// With rotor stall switched off the rotor speed stays above this in
/// flight (design 4.12).
const EASY_ROTOR_FLOOR: f64 = 0.85;
/// Rotor speed limits of the integration.
const ROTOR_SPEED_LIMIT: f64 = 1.5;

/// One aircraft's drive: rated engine power and the rotor system's energy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriveModel {
    /// Rated power at sea level, ft·lbf/s.
    pub rated_power: f64,
    /// Rotor energy constant `J Omega0²` of all the rotors on the drive,
    /// ft·lbf.
    pub rotor_energy: f64,
}

/// What the drive is asked to do this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriveStep {
    /// Power every rotor and tail rotor on the drive absorbs now, ft·lbf/s.
    pub load_power: f64,
    /// Power the engines can deliver now, ft·lbf/s (zero when they are off).
    pub available_power: f64,
    /// Rotor stall is a hazard in force (see [`super::rotor::Hazards`]). Off, the rotor speed cannot fall
    /// below 85 percent in flight.
    pub rotor_stall_hazard: bool,
    /// The wheels carry weight (the floor above does not apply).
    pub wheel_contact: bool,
    pub seconds: f64,
}

impl DriveModel {
    /// The drive of an aircraft that hovers at sea level at its reference
    /// weight on `hover_power` and whose engines are rated `rated_power`.
    /// With the engines cut and the collective held, the power the rotors
    /// need falls with the cube of their speed, so their speed falls from 1
    /// to 0.8 in `energy_seconds` when J Omega0² = 4 x that x the hover
    /// power.
    pub fn new(rated_power: f64, hover_power: f64, energy_seconds: f64) -> Self {
        Self {
            rated_power,
            rotor_energy: 4. * energy_seconds * hover_power,
        }
    }

    /// Engine power the aircraft can have now, ft·lbf/s.
    pub fn available_power(&self, density: f64, damage: f64, throttle: f64) -> f64 {
        self.rated_power
            * (density / rotor::sea_level_density()).powf(ENGINE_LAPSE_EXPONENT)
            * damage
            * throttle.clamp(0., 1.)
    }

    /// One tick: the rotor speed from the start-of-tick engine output and
    /// load, then the governor and the engine lag. Returns the new rotor
    /// speed.
    pub fn advance(&self, drive: &mut Drive, step: DriveStep) -> f64 {
        let nr = drive.rotor_speed;
        let engine = drive.engine_output[0];
        let mut next = (nr
            + step.seconds * (engine - step.load_power) / (self.rotor_energy * nr.max(0.05)))
        .clamp(0., ROTOR_SPEED_LIMIT);
        if !step.rotor_stall_hazard && !step.wheel_contact {
            next = next.max(EASY_ROTOR_FLOOR);
        }
        drive.rotor_speed = next;
        let demand = (step.load_power
            + GOVERNOR_GAIN * (drive.rotor_speed_reference - nr) * self.rated_power)
            .clamp(0., step.available_power);
        drive.engine_output[0] = if step.available_power > 0. {
            engine + (demand - engine) * (step.seconds / ENGINE_SECONDS).min(1.)
        } else {
            0.
        };
        next
    }
}

/// Counts the LOW ROTOR and ROTOR OVERSPEED warnings for a rotor speed.
pub fn update_warnings(warnings: &mut Warnings, rotor_speed: f64) {
    let count = |ticks: u32, on: bool| if on { ticks.saturating_add(1) } else { 0 };
    warnings.low_rotor = count(warnings.low_rotor, rotor_speed < LOW_ROTOR);
    warnings.rotor_overspeed = count(warnings.rotor_overspeed, rotor_speed > ROTOR_OVERSPEED);
}
