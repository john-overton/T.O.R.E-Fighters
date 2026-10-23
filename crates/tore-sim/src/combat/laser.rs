//! The laser designator of an aircraft with a targeting sight that lases
//! (Pave Spike, Pave Knife or a built-in designator), its states and the
//! laser spot it puts on the sight's point (docs/spec/air-to-ground.md, "The
//! laser" and "Buddy lasing").
//!
//! Retail has no spot, no laze key and no laser state: a laser weapon homes
//! on its designated target while its own launcher carries a working
//! designator (docs/formats/missiles.md, "Laser and designator"). The spot,
//! its states, the automatic rule and Ctrl+L are opinionated (John,
//! 2026-10-10: the laze button and the lasing displays); the limits read the
//! retail records and weather (agent design on them). Spots belong to an
//! aircraft and its side, so buddy lasing (decision 6) and a later AI round
//! can use any aircraft's spot.
use super::live::Side;
use crate::attitude::Vector;
use crate::sensors::{Weather, track::selector};
use tore_formats::weapons::Weapon;

/// What the laser is doing (the state diagram in the spec).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaserState {
    /// No designator, or the sight holds no point.
    #[default]
    Off,
    /// The sight holds a point and the laser is off.
    Ready,
    /// The spot is on the sight's tracked object or pin.
    Lasing,
    /// The laser is wanted but the line is out of range, outside the
    /// gimbal or blocked by terrain: the spot is gone until it clears.
    Masked,
}

/// Why a wanted laser is masked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mask {
    /// Beyond the designator's reach, which cloud or fog on the line halves.
    Range,
    /// Outside the sensor's gimbal.
    Gimbal,
    /// Terrain blocks the line (the AC-130's 25 ft rule).
    Terrain,
}

/// One laser spot: whose it is, its side, where it is and the object it
/// sits on (none on a pinned ground point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaserSpot {
    /// The lasing aircraft.
    pub owner: u32,
    pub side: Side,
    pub position: Vector,
    pub target: Option<u32>,
}

/// The fraction of a laser's (or a laser weapon's release) range left by
/// the weather on the line between two altitudes: the retail laser selector,
/// 50 percent in cloud or fog and 100 elsewhere (docs/formats/missiles.md,
/// "Weather and night"). Retail applies it at release only.
pub fn weather_scale(weather: &Weather, a_ft: f64, b_ft: f64) -> f64 {
    weather.percent(selector::LASER, a_ft, b_ft) / 100.
}

/// A laser weapon: signature 1 with the designator flag 0x400 (the GBU-10
/// family, GBU-27, GBU-28, AS-14, AS-30, AT-12; retail data).
pub fn is_laser_weapon(weapon: &Weapon) -> bool {
    weapon.seeker.signature == super::sight::LASER_SIGNATURE && weapon.flags & 0x400 != 0
}

/// What one step of the laser reads.
#[derive(Clone, Copy, Debug)]
pub struct LaserInputs {
    /// The aircraft carries a laser designator: its reach, feet.
    pub designator: Option<f64>,
    /// The sight's held point and the object it tracks, if it holds one.
    pub held: Option<(Vector, Option<u32>)>,
    /// The sight's eye in the world.
    pub eye: Vector,
    /// The sensor can follow the held point (inside its gimbal).
    pub in_gimbal: bool,
    /// The automatic rule: a laser weapon is selected and armed, or one of
    /// the aircraft's own laser weapons is in flight.
    pub automatic: bool,
    /// The laser weather scale on the line ([`weather_scale`]).
    pub weather: f64,
}

/// One aircraft's laser.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Laser {
    pub state: LaserState,
    /// Ctrl+L: `Some(true)` on by hand, `Some(false)` off by hand, `None` the
    /// automatic rule. A manual on ends when the sight stops holding a point;
    /// a manual off ends at Ctrl+L or a new laser weapon release.
    pub manual: Option<bool>,
    /// The automatic rule wanted the laser on at the last step.
    pub automatic: bool,
    /// The sight's point while it holds one, and the object it tracks.
    pub point: Option<Vector>,
    pub target: Option<u32>,
    /// From the eye to the point, feet (0 with no point).
    pub range_ft: f64,
    /// The laser's reach now, feet: the record's, scaled by the weather.
    pub limit_ft: f64,
    pub mask: Option<Mask>,
}
impl Laser {
    /// The laser is wanted on, by hand or by the automatic rule.
    pub fn wanted(&self) -> bool {
        self.manual.unwrap_or(self.automatic)
    }
    pub fn lasing(&self) -> bool {
        self.state == LaserState::Lasing
    }
    /// Ctrl+L: turn the laser on or off by hand. Returns whether it is wanted
    /// on afterwards.
    pub fn toggle(&mut self) -> bool {
        let on = !self.wanted();
        self.manual = Some(on);
        on
    }
    /// A laser weapon left the aircraft: a manual off gives way to the
    /// automatic rule again.
    pub fn note_release(&mut self) {
        if self.manual == Some(false) {
            self.manual = None;
        }
    }
    /// The spot while lasing, owned by `owner` of `side`.
    pub fn spot(&self, owner: u32, side: Side) -> Option<LaserSpot> {
        self.lasing()
            .then_some(self.point)
            .flatten()
            .map(|position| LaserSpot {
                owner,
                side,
                position,
                target: self.target,
            })
    }
    /// One fixed step. `clear` says whether terrain leaves the line from the
    /// eye to the point open; it is asked only when every other limit passes.
    pub fn step(&mut self, inputs: LaserInputs, clear: impl FnOnce(Vector, Vector) -> bool) {
        self.automatic = inputs.automatic;
        self.mask = None;
        let Some(reach) = inputs.designator else {
            *self = Self {
                automatic: inputs.automatic,
                ..Self::default()
            };
            return;
        };
        self.limit_ft = reach * inputs.weather;
        let Some((point, target)) = inputs.held else {
            if self.point.is_some() && self.manual == Some(true) {
                self.manual = None;
            }
            self.state = LaserState::Off;
            self.point = None;
            self.target = None;
            self.range_ft = 0.;
            return;
        };
        self.point = Some(point);
        self.target = target;
        self.range_ft = (0..3)
            .map(|i| (point[i] - inputs.eye[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if !self.wanted() {
            self.state = LaserState::Ready;
            return;
        }
        self.mask = if self.range_ft > self.limit_ft {
            Some(Mask::Range)
        } else if !inputs.in_gimbal {
            Some(Mask::Gimbal)
        } else if !clear(inputs.eye, point) {
            Some(Mask::Terrain)
        } else {
            None
        };
        self.state = if self.mask.is_some() {
            LaserState::Masked
        } else {
            LaserState::Lasing
        };
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "laser_checkpoint.rs"]
mod checkpoint;

#[cfg(test)]
#[path = "laser_tests.rs"]
mod tests;
