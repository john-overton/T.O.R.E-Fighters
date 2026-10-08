//! Stability augmentation and trim on the powered-lift aircraft (VTOL
//! overhaul design 5.2 and 5.3, slice P6).
//!
//! Two pieces, each run once a tick:
//!
//! 1. [`State::trimmed_stick`], the trim-set latch and the cyclic trim. The
//!    hybrid adapter calls it before the powered-lift force law, so every
//!    law sees the pilot's command: the stick (masked while the latch holds
//!    it) plus the trim. Only the helicopters and the V-22 carry a trim; on
//!    every other aircraft the stick passes through untouched.
//! 2. [`State::augment`], the stability level's limited-authority feedback.
//!    The force laws (slices P2 to P5) call it with the air data they have
//!    already computed and turn the [`Augmented::controls`] it returns into
//!    rotor, tail rotor and puffer moments through their own mixers. The
//!    jets' wing surfaces take [`Augmented::pilot`] instead: their moment law
//!    is already a G and roll-rate command (design 4.3), so the damper acts
//!    on the puffers only.
//!
//! The law itself, [`augment`], is a pure function of the aircraft, its
//! aids and what it senses, so it is tested against a stub moment model
//! without any rotor or wing physics.
//!
//! **Levels** (design 5.2; John chose Damper as the default, 2026-10-08):
//!
//! - *Off*: nothing is added. The heading reference follows the aircraft.
//! - *Damper*: rate damping on every axis, reaching its authority (20
//!   percent of travel) at a quarter of the aircraft's full-stick hover rate;
//!   above 40 kt the yaw damper damps the departure from a coordinated turn
//!   and steers out sideslip; the force law's torque estimate is fed to the
//!   pedals, outside the authority clamp so a collective step does not yaw
//!   the nose and never past the pedals' travel; the CH-47's longitudinal cyclic trim follows airspeed. On the
//!   vectoring jets the damper is the puffers' rate limit instead: the valve
//!   demand is the pilot's less the rate as a share of the PT `puffRot`
//!   maximum, so full stick settles at that rate and a released stick stops
//!   the rotation. In roll and yaw a tight loop on the departure from the
//!   commanded rate (the stick's travel times the PT rate; the damper's 20
//!   percent, reached 5 degrees per second off it) holds off the low-speed
//!   roll-off, and the rate limit takes the remaining 80 percent of travel,
//!   so full stick still settles at the PT rate (slice P4). Release the
//!   stick and the rotation stops where it is.
//! - *Attitude*: the damper plus attitude retention about the trimmed
//!   attitude, one full travel per 30 degrees of pitch and 45 of bank (the
//!   design's full-stick attitudes), with 35 percent authority, and heading
//!   hold below 40 kt with the pedals centred (the heading where they were
//!   centred; the law keeps no other memory). A stick within the authority
//!   settles at an attitude 30 (45) degrees per full travel away from the
//!   trim attitude; more stick keeps rotating, as the physics allows.
//!   Release it and the aircraft returns to the trim attitude.
//!
//! Damage that empties the hydraulics drops every level to Off
//! ([`State::stability_in_effect`]); the chosen level is kept. The V-22's
//! corridor protection is part of its mixer, not a level.
//!
//! Every gain below is **fitted** (agent decisions, 2026-10-08): chosen for a
//! well-damped response against the stub model and the 8.2 hover rates,
//! and open to the fits of P2 to P5.

use super::super::State;
use super::state::{PilotAids, TrimLatch};
use crate::models::variety::{LiftKind, PoweredLift, RotorLayout};
use std::f64::consts::{PI, TAU};
use tore_input::StabilityLevel;

/// Damper authority, share of full travel per axis (design 5.2).
pub const DAMPER_AUTHORITY: f64 = 0.2;
/// Attitude-level authority, share of full travel per axis (design 5.2).
pub const ATTITUDE_AUTHORITY: f64 = 0.35;
/// The damper reaches its authority at this share of the aircraft's
/// full-stick hover rate (fitted).
const DAMPER_SATURATION: f64 = 0.25;
/// The Easy flight physics cheat's attitude retention on the rotorcraft at
/// Damper and Off (design 4.12, John 2026-10-08): the Attitude level's hold
/// at this share of its gain, limited to this share of travel on pitch and
/// roll, about the trim attitude. Weak on purpose: it settles the phugoid of
/// a trimmed forward flight and nothing else (no speed, height, position or
/// heading hold). Fitted.
pub const EASY_RETENTION_GAIN: f64 = 0.6;
pub const EASY_RETENTION_AUTHORITY: f64 = 0.1;
/// Attitude per full travel at the Attitude level, [pitch, bank], degrees
/// (design 5.2: 30 and 45 at full stick).
const ATTITUDE_PER_TRAVEL_DEGREES: [f64; 2] = [30., 45.];
/// Heading error per full pedal at the Attitude level's heading hold,
/// degrees (fitted).
const HEADING_PER_TRAVEL_DEGREES: f64 = 30.;
/// Sideslip per full pedal of the yaw damper's turn coordination, degrees
/// (fitted).
const SIDESLIP_PER_TRAVEL_DEGREES: f64 = 30.;
/// Turn coordination and heading hold change over around 40 kt (design
/// 5.2), blended across this band so nothing steps (fitted).
const COORDINATION_BAND_KT: [f64; 2] = [35., 45.];
/// Pedals within this share of travel of their trim count as centred for
/// heading hold (fitted, the trim latch's own margin).
const PEDALS_CENTRED: f64 = 0.05;
/// The trim-set latch frees the stick within this share of travel of
/// centre (design 5.3).
pub const LATCH_CENTRE: f64 = 0.05;
/// The Attitude level's reference moves this far per full travel of trim
/// adjustment: 5 degrees per second for the 10 percent a second the trim
/// keys move the trim (design 5.3; [`tore_input::trim_keys`]).
pub const ATTITUDE_TRIM_DEGREES_PER_TRAVEL: f64 = 50.;
/// The vectoring jets' roll and yaw rate loop reaches the damper's
/// authority at this rate, deg/s (fitted in slice P4 so a jetborne jet
/// with 10 degrees of sideslip at 40 kt stays within 10 degrees of bank
/// for 3 s at Damper, design test J11).
const JET_RATE_LOOP_SATURATION: f64 = 5.;
/// The CH-47's longitudinal cyclic trim schedule: none below the first
/// airspeed, all of it at the second, kt (fitted; P3 maps the share onto
/// its disk tilt).
const TANDEM_SCHEDULE_KT: [f64; 2] = [40., 140.];
/// Standard gravity, ft/s².
const GRAVITY: f64 = 32.174;
/// ft/s per knot.
const FPS_PER_KT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;

/// What a force law has measured this tick for the stability law.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sensed {
    /// True airspeed, ft/s.
    pub airspeed_fps: f64,
    /// Sideslip, rad: positive when the aircraft moves to its right through
    /// the air (`asin(v / V)`, design 4.1).
    pub sideslip_rad: f64,
    /// The pedal travel that would cancel the main rotor's uncompensated
    /// torque this tick (single main rotor only; zero elsewhere). Damper and
    /// Attitude feed it forward to the pedals outside their authority, within
    /// the pedals' travel.
    pub torque_pedal: f64,
}

/// The aircraft's attitude and body rates as the law reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// [pitch, bank, heading], rad, as the flight state holds them.
    pub attitude: [f64; 3],
    /// Body rates [roll, pitch, yaw], rad/s, in the axes of
    /// [`super::body`].
    pub rates: [f64; 3],
}

/// The stability law's output for one tick, in stick order [pitch, roll,
/// yaw] and travel -1..1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Augmented {
    /// The level in effect (Off without hydraulics).
    pub level: StabilityLevel,
    /// The pilot's command: stick plus trim.
    pub pilot: [f64; 3],
    /// What the stability law adds.
    pub augmentation: [f64; 3],
    /// The pilot's command plus the augmentation, within full travel: what
    /// the rotor and puffer mixers fly.
    pub controls: [f64; 3],
    /// The CH-47's scheduled longitudinal cyclic trim, 0..1 of the tilt its
    /// mixer gives it; zero on every other aircraft and at Off.
    pub longitudinal_trim: f64,
}

/// `angle` wrapped into -pi..pi.
fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// 0 below `band[0]`, 1 above `band[1]`, a smooth step between.
fn blend(value: f64, band: [f64; 2]) -> f64 {
    let t = ((value - band[0]) / (band[1] - band[0])).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// The attitude the Attitude level holds when it takes over at `attitude`
/// ([pitch, bank, heading]): the current one, within the full-stick
/// attitudes.
pub fn reference_of(attitude: [f64; 3]) -> [f64; 3] {
    let [pitch, bank] = ATTITUDE_PER_TRAVEL_DEGREES.map(f64::to_radians);
    [
        attitude[0].clamp(-pitch, pitch),
        attitude[1].clamp(-bank, bank),
        attitude[2],
    ]
}

/// The attitude hold's pitch and bank terms, in travel per axis: the error
/// from the reference over the full-stick attitudes (the Attitude level's
/// law, and the Easy flight physics retention's at reduced gain).
fn attitude_hold(aids: &PilotAids, pitch: f64, bank: f64) -> [f64; 2] {
    let [pitch_span, bank_span] = ATTITUDE_PER_TRAVEL_DEGREES.map(f64::to_radians);
    [
        (aids.attitude_reference[0] - pitch) / pitch_span,
        wrap(aids.attitude_reference[1] - bank) / bank_span,
    ]
}

/// Moves the Attitude level's reference for a trim adjustment of `amount`
/// travel on the pitch (0) or roll (1) axis, within the full-stick
/// attitudes.
pub fn adjust_reference(aids: &mut PilotAids, axis: usize, amount: f64) {
    let limit = ATTITUDE_PER_TRAVEL_DEGREES[axis].to_radians();
    let step = amount * ATTITUDE_TRIM_DEGREES_PER_TRAVEL.to_radians();
    aids.attitude_reference[axis] = (aids.attitude_reference[axis] + step).clamp(-limit, limit);
}

/// The stability law (module documentation). `level` is the level in effect;
/// `pilot` the pilot's command, stick plus trim, [pitch, roll, yaw]. Moves
/// the heading reference while heading hold is not holding.
pub fn augment(
    lift: &PoweredLift,
    level: StabilityLevel,
    aids: &mut PilotAids,
    pilot: [f64; 3],
    body: Body,
    sensed: Sensed,
) -> Augmented {
    let [pitch, bank, heading] = body.attitude;
    // Stick order: pitch, roll, yaw.
    let [roll_rate, pitch_rate, yaw_rate] = body.rates;
    let targets = lift.targets.hover_rates_degrees_per_second;
    let full_rates = [targets[1], targets[0], targets[2]].map(|r| r.to_radians().max(1e-3));
    let knots = sensed.airspeed_fps / FPS_PER_KT;
    let coordinated = blend(knots, COORDINATION_BAND_KT);
    let turn_rate = GRAVITY * bank.sin() * pitch.cos() / sensed.airspeed_fps.max(1.);
    let rates = [pitch_rate, roll_rate, yaw_rate - coordinated * turn_rate];
    let pedals = pilot[2] - aids.trim[2];
    let holding = level == StabilityLevel::Attitude
        && knots < COORDINATION_BAND_KT[1]
        && pedals.abs() <= PEDALS_CENTRED;
    if !holding {
        aids.attitude_reference[2] = heading;
    }
    let mut augmentation = [0.; 3];
    if level != StabilityLevel::Off {
        let jet = lift.kind == LiftKind::VectorJet;
        let mut hold = [0.; 3];
        if level == StabilityLevel::Attitude {
            [hold[0], hold[1]] = attitude_hold(aids, pitch, bank);
            if holding {
                hold[2] = (1. - coordinated) * wrap(aids.attitude_reference[2] - heading)
                    / HEADING_PER_TRAVEL_DEGREES.to_radians();
            }
        }
        let authority = if level == StabilityLevel::Attitude {
            ATTITUDE_AUTHORITY
        } else {
            DAMPER_AUTHORITY
        };
        // The jets' hold is limited on its own (their rate limit is not);
        // the rotorcraft limit the hold and the damping together.
        let jet_hold = hold.map(|h| h.clamp(-authority, authority));
        let damping: [f64; 3] = std::array::from_fn(|axis| {
            if jet && axis == 0 {
                -rates[axis] / full_rates[axis]
            } else if jet {
                // Roll and yaw: a tight loop within the damper's authority
                // on the departure from the rate the pilot and the attitude
                // hold ask for (their travel times the PT rate), on top of
                // the puffers' rate limit, which is scaled so the two
                // together still hold full stick at the PT rate (slice P4).
                let demand = (pilot[axis] + jet_hold[axis]) * full_rates[axis];
                (-(rates[axis] - demand) / JET_RATE_LOOP_SATURATION.to_radians())
                    .clamp(-DAMPER_AUTHORITY, DAMPER_AUTHORITY)
                    - (1. - DAMPER_AUTHORITY) * rates[axis] / full_rates[axis]
            } else {
                -rates[axis] * DAMPER_AUTHORITY / (DAMPER_SATURATION * full_rates[axis])
            }
        });
        let mut feedback = damping;
        if !jet {
            feedback[2] +=
                coordinated * sensed.sideslip_rad / SIDESLIP_PER_TRAVEL_DEGREES.to_radians();
        }
        augmentation = std::array::from_fn(|axis| {
            if jet {
                // The puffers' rate limit is not bounded by the authority:
                // it is what keeps full stick at the PT rate.
                feedback[axis] + jet_hold[axis]
            } else {
                (feedback[axis] + hold[axis]).clamp(-authority, authority)
            }
        });
        if !jet {
            // Torque compensation is a mixer, not a feedback: it moves the
            // pedals' neutral with the drive torque, outside the authority
            // clamp, so a collective step does not yaw the nose (design 5.2,
            // H12). It never takes the pedals past their physical travel.
            let pedals = pilot[2] + augmentation[2];
            let fed = (pedals + sensed.torque_pedal).clamp(-1., 1.);
            augmentation[2] += fed - pedals.clamp(-1., 1.);
        }
    }
    let longitudinal_trim = match lift.rotor.map(|rotor| rotor.layout) {
        Some(RotorLayout::Tandem { .. }) if level != StabilityLevel::Off => {
            ((knots - TANDEM_SCHEDULE_KT[0]) / (TANDEM_SCHEDULE_KT[1] - TANDEM_SCHEDULE_KT[0]))
                .clamp(0., 1.)
        }
        _ => 0.,
    };
    Augmented {
        level,
        pilot,
        augmentation,
        controls: std::array::from_fn(|axis| (pilot[axis] + augmentation[axis]).clamp(-1., 1.)),
        longitudinal_trim,
    }
}

impl State {
    /// The stability level in effect: the chosen one, or Off once the
    /// hydraulics are gone. None on aircraft without powered lift.
    pub fn stability_in_effect(&self) -> Option<StabilityLevel> {
        self.model().powered_lift()?;
        Some(if self.systems.fluids.hydraulic > 0. {
            self.lift_controls.aids.stability
        } else {
            StabilityLevel::Off
        })
    }

    /// The pilot's command this tick on a powered-lift aircraft, [pitch,
    /// roll, yaw]: `stick` through the trim-set latch, plus the trim. The
    /// helicopters and the V-22 only; every other aircraft gets `stick` back
    /// unchanged, as does a rotorcraft with no trim and a free latch.
    ///
    /// The latch (design 5.3): Trim set makes the stick plus trim the new
    /// trim (and, at the Attitude level, the current attitude the new
    /// reference), then ignores the stick until it is back within 5 percent
    /// of centre on every axis, so a spring-centred stick emulates force
    /// trim.
    pub(crate) fn trimmed_stick(&mut self, stick: [f64; 3]) -> [f64; 3] {
        if !self
            .model()
            .powered_lift()
            .is_some_and(|lift| lift.kind != LiftKind::VectorJet)
        {
            return stick;
        }
        let attitude = [self.pitch, self.bank, self.yaw];
        let retains = self.cheats.easy_physics;
        let aids = &mut self.lift_controls.aids;
        let centred = stick.iter().all(|v| v.abs() <= LATCH_CENTRE);
        let stick = match aids.trim_latch {
            TrimLatch::Free => stick,
            TrimLatch::Capture => {
                aids.trim = std::array::from_fn(|i| (aids.trim[i] + stick[i]).clamp(-1., 1.));
                if aids.stability == StabilityLevel::Attitude || retains {
                    aids.attitude_reference = reference_of(attitude);
                }
                aids.trim_latch = if centred {
                    TrimLatch::Free
                } else {
                    TrimLatch::Masked
                };
                [0.; 3]
            }
            TrimLatch::Masked if centred => {
                aids.trim_latch = TrimLatch::Free;
                stick
            }
            TrimLatch::Masked => [0.; 3],
        };
        if aids.trim == [0.; 3] {
            return stick;
        }
        std::array::from_fn(|i| (stick[i] + aids.trim[i]).clamp(-1., 1.))
    }

    /// The stability law for this tick (module documentation), for the
    /// powered-lift force laws: `pilot` is the command they were given (the
    /// output of [`State::trimmed_stick`]), `sensed` their air data.
    pub fn augment(&mut self, lift: &PoweredLift, pilot: [f64; 3], sensed: Sensed) -> Augmented {
        let level = self.stability_in_effect().unwrap_or(StabilityLevel::Off);
        let body = Body {
            attitude: [self.pitch, self.bank, self.yaw],
            rates: self.lift_controls.body_rates,
        };
        let mut augmented = augment(
            lift,
            level,
            &mut self.lift_controls.aids,
            pilot,
            body,
            sensed,
        );
        if self.easy_retention(lift, level) {
            let weight = self.lift_controls.hover_fraction(lift.kind);
            let hold = attitude_hold(&self.lift_controls.aids, body.attitude[0], body.attitude[1]);
            for (axis, hold) in hold.into_iter().enumerate() {
                let extra = (EASY_RETENTION_GAIN * weight * hold)
                    .clamp(-EASY_RETENTION_AUTHORITY, EASY_RETENTION_AUTHORITY);
                augmented.augmentation[axis] += extra;
                augmented.controls[axis] =
                    (pilot[axis] + augmented.augmentation[axis]).clamp(-1., 1.);
            }
        }
        augmented
    }

    /// Whether the Easy flight physics attitude retention acts: the cheat on,
    /// a helicopter or the V-22 (in proportion to its helicopter mode) at
    /// Damper or Off with hydraulics. The Attitude level has its own, full
    /// one; the jets none.
    pub fn easy_retention(&self, lift: &PoweredLift, level: StabilityLevel) -> bool {
        self.cheats.easy_physics
            && lift.kind != LiftKind::VectorJet
            && level != StabilityLevel::Attitude
            && self.systems.fluids.hydraulic > 0.
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::DT;
    use super::super::body::{self, Inertia, Moments};
    use super::*;
    use crate::attitude::Basis;
    use crate::models::variety::{LiftKind, PoweredLift};
    use tore_formats::aircraft::AircraftId;

    fn lift(id: AircraftId) -> PoweredLift {
        let aircraft = crate::models::variety::tests::synthetic(id);
        let state = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        state.model().powered_lift().unwrap()
    }

    /// A stub moment model: each axis turns at `power` rad/s² per unit of
    /// control travel against a natural rate damping `damping` per second,
    /// about the aircraft's own inertia. No rotor, wing or puffer physics.
    struct Stub {
        lift: PoweredLift,
        aids: PilotAids,
        basis: Basis,
        rates: [f64; 3],
        inertia: Inertia,
        /// Angular acceleration per unit travel, [roll, pitch, yaw].
        power: [f64; 3],
        /// Natural rate damping, 1/s, [roll, pitch, yaw].
        damping: [f64; 3],
        level: StabilityLevel,
        airspeed_fps: f64,
    }

    impl Stub {
        fn new(id: AircraftId, level: StabilityLevel) -> Self {
            let lift = lift(id);
            let inertia = Inertia::from_weight(20_000., lift.body.radii_of_gyration_ft);
            let targets = lift
                .targets
                .hover_rates_degrees_per_second
                .map(f64::to_radians);
            // Rotor-like: natural damping 2/s, sized so full travel with
            // no augmentation settles 25 percent above the 8.2 target
            // (rotors); puffer-like for the jets: no natural damping and
            // the PT onset (60 / 20 / 20 deg/s²).
            let (power, damping) = if lift.kind == LiftKind::VectorJet {
                ([60., 20., 20.].map(f64::to_radians), [0.; 3])
            } else {
                (targets.map(|t| 2. * 1.25 * t), [2.; 3])
            };
            Self {
                lift,
                aids: PilotAids {
                    stability: level,
                    ..Default::default()
                },
                basis: Basis::new(0., 0., 0.),
                rates: [0.; 3],
                inertia,
                power,
                damping,
                level,
                airspeed_fps: 0.,
            }
        }
        fn attitude(&self) -> [f64; 3] {
            let [yaw, pitch, bank] = self.basis.angles();
            [pitch, bank, yaw]
        }
        /// One tick with the pilot's command [pitch, roll, yaw]; returns the
        /// law's output.
        fn step(&mut self, pilot: [f64; 3]) -> Augmented {
            let attitude = self.attitude();
            let out = augment(
                &self.lift,
                self.level,
                &mut self.aids,
                pilot,
                Body {
                    attitude,
                    rates: self.rates,
                },
                Sensed {
                    airspeed_fps: self.airspeed_fps,
                    ..Default::default()
                },
            );
            let [pitch, roll, yaw] = out.controls;
            let controls = [roll, pitch, yaw];
            let moments = Moments {
                applied: std::array::from_fn(|i| self.inertia.0[i] * self.power[i] * controls[i]),
                damping: std::array::from_fn(|i| self.inertia.0[i] * self.damping[i]),
            };
            (self.basis, self.rates) =
                body::advance(self.basis, self.rates, self.inertia, moments, DT);
            out
        }
        fn run(&mut self, pilot: [f64; 3], seconds: f64) {
            for _ in 0..(seconds * 120.).round() as usize {
                self.step(pilot);
            }
        }
    }

    const ROTORCRAFT: [AircraftId; 4] = [
        AircraftId::Ah64,
        AircraftId::Mi24,
        AircraftId::Ch47,
        AircraftId::V22,
    ];

    #[test]
    fn off_adds_nothing() {
        for id in ROTORCRAFT.into_iter().chain([AircraftId::Av8]) {
            let mut stub = Stub::new(id, StabilityLevel::Off);
            stub.run([0., 1., 0.], 1.);
            let out = stub.step([0.3, -0.2, 0.1]);
            assert_eq!(out.augmentation, [0.; 3], "{id:?}");
            assert_eq!(out.controls, [0.3, -0.2, 0.1], "{id:?}");
            assert_eq!(out.longitudinal_trim, 0., "{id:?}");
        }
        // Puffers have no damping of their own: at Off a hovering Harrier
        // keeps rolling after the stick is released.
        let mut jet = Stub::new(AircraftId::Av8, StabilityLevel::Off);
        jet.run([0., 1., 0.], 0.5);
        let rate = jet.rates[0];
        jet.run([0.; 3], 3.);
        assert!((jet.rates[0] - rate).abs() < 1e-9 * rate.abs().max(1.));
    }

    #[test]
    fn damper_stops_the_rotation_and_leaves_the_attitude_where_it_was() {
        for id in ROTORCRAFT
            .into_iter()
            .chain([AircraftId::Av8, AircraftId::Yak141])
        {
            let mut stub = Stub::new(id, StabilityLevel::Damper);
            stub.run([0., 1., 0.], 0.6);
            assert!(stub.rates[0] > 10_f64.to_radians(), "{id:?} rolls");
            // The puffer limit's time constant is the PT rate over its
            // onset (0.8 s in roll), the rotors' a fraction of that.
            stub.run([0.; 3], 5.);
            assert!(
                stub.rates.iter().all(|r| r.abs() < 0.5_f64.to_radians()),
                "{id:?} rotation stopped: {:?}",
                stub.rates
            );
            let bank = stub.attitude()[1];
            assert!(bank > 10_f64.to_radians(), "{id:?} bank stays: {bank}");
            // And stays there: no return to level.
            stub.run([0.; 3], 3.);
            assert!(
                (stub.attitude()[1] - bank).abs() < 1_f64.to_radians(),
                "{id:?}"
            );
        }
    }

    #[test]
    fn full_stick_always_wins_and_the_damper_keeps_to_its_authority() {
        for id in ROTORCRAFT {
            let mut off = Stub::new(id, StabilityLevel::Off);
            let mut damper = Stub::new(id, StabilityLevel::Damper);
            off.run([1., 0., 0.], 3.);
            damper.run([1., 0., 0.], 3.);
            let out = damper.step([1., 0., 0.]);
            assert!(
                out.augmentation
                    .iter()
                    .all(|a| a.abs() <= DAMPER_AUTHORITY + 1e-12)
            );
            // Full stick still turns the aircraft at no less than the
            // travel the damper leaves it.
            assert!(
                damper.rates[1] >= (1. - DAMPER_AUTHORITY) * off.rates[1] - 1e-9,
                "{id:?}: {} against {}",
                damper.rates[1],
                off.rates[1]
            );
            // The damper holds no attitude: displaced and still, it adds
            // nothing.
            let mut still = Stub::new(id, StabilityLevel::Damper);
            still.basis = Basis::new(0.5, -0.2, 0.4);
            assert_eq!(still.step([0.; 3]).augmentation, [0.; 3], "{id:?}");
        }
    }

    #[test]
    fn the_puffer_damper_holds_full_stick_at_the_pt_rate() {
        // J2's control-law part: full stick settles at the PT puffRot
        // maximum (50 roll, 20 pitch and yaw), from the PT onset.
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            for (axis, stick) in [(0, [0., 1., 0.]), (1, [1., 0., 0.]), (2, [0., 0., 1.])] {
                let mut stub = Stub::new(id, StabilityLevel::Damper);
                stub.run(stick, 8.);
                let target = stub.lift.targets.hover_rates_degrees_per_second[axis];
                let rate = stub.rates[axis].to_degrees();
                assert!(
                    (rate - target).abs() < 0.05 * target,
                    "{id:?} axis {axis}: {rate} against {target}"
                );
            }
        }
    }

    #[test]
    fn attitude_returns_to_the_trim_attitude_and_holds_small_stick() {
        for id in ROTORCRAFT.into_iter().chain([AircraftId::Av8]) {
            let mut stub = Stub::new(id, StabilityLevel::Attitude);
            stub.aids.attitude_reference = [3_f64.to_radians(), 0., 0.];
            stub.run([0., 1., 0.], 0.5);
            stub.run([0.; 3], 10.);
            let [pitch, bank, _] = stub.attitude();
            assert!(bank.abs() < 1_f64.to_radians(), "{id:?} bank {bank}");
            assert!(
                (pitch - 3_f64.to_radians()).abs() < 1_f64.to_radians(),
                "{id:?}"
            );
            // A stick within the authority settles at 45 degrees of bank
            // per full travel.
            stub.run([0., 0.2, 0.], 15.);
            let bank = stub.attitude()[1].to_degrees();
            assert!((bank - 9.).abs() < 1., "{id:?} bank {bank}");
            assert!(stub.rates[0].abs() < 0.5_f64.to_radians(), "{id:?}");
        }
    }

    #[test]
    fn attitude_imposes_no_limit_on_full_stick() {
        for id in ROTORCRAFT {
            let mut stub = Stub::new(id, StabilityLevel::Attitude);
            stub.run([0., 1., 0.], 4.);
            let out = stub.step([0., 1., 0.]);
            assert!(out.augmentation[1].abs() <= ATTITUDE_AUTHORITY + 1e-12);
            assert!(
                stub.attitude()[1].abs() > 45_f64.to_radians() || stub.rates[0] > 0.,
                "{id:?} still rolling past 45 degrees"
            );
            assert!(stub.rates[0] > 5_f64.to_radians(), "{id:?}");
        }
    }

    #[test]
    fn heading_hold_below_40_knots_with_the_pedals_centred() {
        let mut stub = Stub::new(AircraftId::Ah64, StabilityLevel::Attitude);
        let start = stub.attitude()[2];
        // A pedal turn moves the reference with the nose; centred, the
        // damper stops the turn and the hold returns to the heading where
        // the pedals were centred.
        stub.run([0., 0., 0.5], 1.);
        let released = stub.attitude()[2];
        stub.run([0.; 3], 6.);
        let held = stub.attitude()[2];
        assert!(wrap(held - start) > 10_f64.to_radians());
        let reference = stub.aids.attitude_reference[2];
        assert!(wrap(reference - released).abs() < 0.5_f64.to_radians());
        assert!(wrap(held - reference).abs() < 0.5_f64.to_radians());
        // A disturbance is pulled back.
        stub.rates[2] = 0.;
        stub.basis = stub.basis.rotated(stub.basis.up.map(|v| v * 0.1));
        stub.run([0.; 3], 6.);
        assert!(wrap(stub.attitude()[2] - held).abs() < 1_f64.to_radians());
        // Above 40 kt the heading reference just follows the nose.
        let mut fast = Stub::new(AircraftId::Ah64, StabilityLevel::Attitude);
        fast.airspeed_fps = 60. * FPS_PER_KT;
        fast.basis = fast.basis.rotated(fast.basis.up.map(|v| v * 0.1));
        let out = fast.step([0.; 3]);
        assert_eq!(out.augmentation[2], 0.);
        assert_eq!(fast.aids.attitude_reference[2], fast.attitude()[2]);
    }

    #[test]
    fn the_damper_feeds_torque_to_the_pedals_and_coordinates_turns() {
        let lift = lift(AircraftId::Ah64);
        let mut aids = PilotAids::default();
        let still = Body::default();
        let level = StabilityLevel::Damper;
        let fed = augment(
            &lift,
            level,
            &mut aids,
            [0.; 3],
            still,
            Sensed {
                torque_pedal: 0.15,
                ..Default::default()
            },
        );
        assert!((fed.augmentation[2] - 0.15).abs() < 1e-12);
        let capped = augment(
            &lift,
            level,
            &mut aids,
            [0.; 3],
            still,
            Sensed {
                torque_pedal: 0.6,
                ..Default::default()
            },
        );
        // The feed-forward is outside the authority clamp (a mixer, not a
        // feedback), and still within the pedals' travel.
        assert!((capped.augmentation[2] - 0.6).abs() < 1e-12);
        let beyond = augment(
            &lift,
            level,
            &mut aids,
            [0., 0., 0.9],
            still,
            Sensed {
                torque_pedal: 0.6,
                ..Default::default()
            },
        );
        assert_eq!(beyond.controls[2], 1.);
        // The damping alone still keeps to the authority.
        let spun = augment(
            &lift,
            level,
            &mut aids,
            [0.; 3],
            Body {
                rates: [0., 0., 3.],
                ..Default::default()
            },
            Sensed::default(),
        );
        assert_eq!(spun.augmentation[2], -DAMPER_AUTHORITY);
        // At 100 kt in a 30-degree banked coordinated turn the yaw damper
        // leaves the turn rate alone; below 35 kt it damps it.
        let bank = 30_f64.to_radians();
        let speed = 100. * FPS_PER_KT;
        let turn = GRAVITY * bank.sin() / speed;
        let turning = Body {
            attitude: [0., bank, 0.],
            rates: [0., 0., turn],
        };
        let sensed = Sensed {
            airspeed_fps: speed,
            ..Default::default()
        };
        let fast = augment(&lift, level, &mut aids, [0.; 3], turning, sensed);
        assert!(fast.augmentation[2].abs() < 1e-12);
        let slow = augment(&lift, level, &mut aids, [0.; 3], turning, Sensed::default());
        assert!(slow.augmentation[2] < -0.01);
        // Sideslip at speed is steered out with the pedals: moving right
        // through the air, the nose goes right.
        let slipping = augment(
            &lift,
            level,
            &mut aids,
            [0.; 3],
            Body::default(),
            Sensed {
                airspeed_fps: speed,
                sideslip_rad: 3_f64.to_radians(),
                ..Default::default()
            },
        );
        assert!(slipping.augmentation[2] > 0.05);
    }

    #[test]
    fn the_ch47_schedules_its_longitudinal_trim_with_airspeed() {
        let lift = lift(AircraftId::Ch47);
        let mut aids = PilotAids::default();
        let at = |aids: &mut PilotAids, level, kt: f64| {
            augment(
                &lift,
                level,
                aids,
                [0.; 3],
                Body::default(),
                Sensed {
                    airspeed_fps: kt * FPS_PER_KT,
                    ..Default::default()
                },
            )
            .longitudinal_trim
        };
        assert_eq!(at(&mut aids, StabilityLevel::Damper, 20.), 0.);
        assert!((at(&mut aids, StabilityLevel::Damper, 90.) - 0.5).abs() < 1e-9);
        assert_eq!(at(&mut aids, StabilityLevel::Attitude, 160.), 1.);
        assert_eq!(at(&mut aids, StabilityLevel::Off, 160.), 0.);
        let ah64 = super::tests::lift(AircraftId::Ah64);
        let other = augment(
            &ah64,
            StabilityLevel::Damper,
            &mut aids,
            [0.; 3],
            Body::default(),
            Sensed {
                airspeed_fps: 150. * FPS_PER_KT,
                ..Default::default()
            },
        );
        assert_eq!(other.longitudinal_trim, 0.);
    }

    fn hybrid(id: AircraftId) -> State {
        let aircraft = crate::models::variety::tests::synthetic(id);
        let mut s = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s
    }

    #[test]
    fn trim_set_latches_the_stick_until_it_returns_to_centre() {
        use crate::flight::PilotCommand::Lift;
        use tore_input::LiftCommand;
        let mut s = hybrid(AircraftId::Ah64);
        // No trim and a free latch: the stick passes through exactly.
        let stick = [0.3, -0.25, 0.125];
        assert_eq!(s.trimmed_stick(stick), stick);
        s.command(Lift(LiftCommand::TrimSet));
        assert_eq!(s.trimmed_stick(stick), stick);
        assert_eq!(s.lift_controls.aids.trim, stick);
        assert_eq!(s.lift_controls.aids.trim_latch, TrimLatch::Masked);
        // Still held, or moved, the stick is ignored: the trim flies.
        assert_eq!(s.trimmed_stick([0.6, 0., 0.]), stick);
        assert_eq!(s.trimmed_stick([0.04, -0.06, 0.]), stick);
        // Back within 5 percent on every axis it adds to the trim again.
        let back = s.trimmed_stick([0.04, -0.03, 0.]);
        assert_eq!(s.lift_controls.aids.trim_latch, TrimLatch::Free);
        for (got, want) in back.into_iter().zip([0.34, -0.28, 0.125]) {
            assert!((got - want).abs() < 1e-12, "{back:?}");
        }
        // Within full travel.
        assert_eq!(s.trimmed_stick([1., -1., 0.]), [1., -1., 0.125]);
        // Trim set with the stick centred changes nothing and frees at once.
        s.command(Lift(LiftCommand::TrimSet));
        s.trimmed_stick([0.; 3]);
        assert_eq!(s.lift_controls.aids.trim, stick);
        assert_eq!(s.lift_controls.aids.trim_latch, TrimLatch::Free);
        // The jets and conventional aircraft carry no trim.
        for id in [AircraftId::Av8, AircraftId::F16C] {
            let mut other = hybrid(id);
            other.lift_controls.aids.trim = [0.5; 3];
            assert_eq!(other.trimmed_stick(stick), stick, "{id:?}");
        }
    }

    #[test]
    fn attitude_level_trim_moves_the_reference_and_trim_set_captures_it() {
        use crate::flight::PilotCommand::Lift;
        use tore_input::{LiftCommand, TrimAxis};
        // The trim keys' tick is the simulation's.
        assert_eq!(tore_input::trim_keys::TICK_SECONDS, DT);
        let mut s = hybrid(AircraftId::Ah64);
        s.pitch = -0.05;
        s.bank = 0.1;
        s.command(Lift(LiftCommand::SetStability(StabilityLevel::Attitude)));
        assert_eq!(s.lift_controls.aids.attitude_reference, [-0.05, 0.1, s.yaw]);
        // Ctrl+Up for a second at Attitude: 5 degrees nose down, no trim.
        for _ in 0..120 {
            s.command(Lift(LiftCommand::TrimAdjust(
                TrimAxis::Pitch,
                -tore_input::trim_keys::PER_TICK,
            )));
        }
        let reference = s.lift_controls.aids.attitude_reference;
        assert!((reference[0] - (-0.05 - 5_f64.to_radians())).abs() < 1e-9);
        assert_eq!(s.lift_controls.aids.trim, [0.; 3]);
        // The reference stays within the full-stick attitudes.
        for _ in 0..1_000 {
            s.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Roll, 0.01)));
        }
        assert!((s.lift_controls.aids.attitude_reference[1] - 45_f64.to_radians()).abs() < 1e-12);
        // Pedal trim stays a trim.
        s.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Pedal, 0.1)));
        assert_eq!(s.lift_controls.aids.trim, [0., 0., 0.1]);
        // Trim set captures the attitude flown.
        s.pitch = 0.2;
        s.bank = -0.3;
        s.command(Lift(LiftCommand::TrimSet));
        s.trimmed_stick([0.; 3]);
        assert_eq!(s.lift_controls.aids.attitude_reference[..2], [0.2, -0.3]);
        // Trim centre levels the reference and clears the trim.
        s.command(Lift(LiftCommand::TrimCentre));
        assert_eq!(s.lift_controls.aids.trim, [0.; 3]);
        assert_eq!(s.lift_controls.aids.attitude_reference[..2], [0., 0.]);
        // At Damper the trim keys move the trim, not the reference.
        s.command(Lift(LiftCommand::SetStability(StabilityLevel::Damper)));
        s.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Pitch, -0.02)));
        assert_eq!(s.lift_controls.aids.trim, [-0.02, 0., 0.]);
        assert_eq!(s.lift_controls.aids.attitude_reference[..2], [0., 0.]);
    }

    #[test]
    fn no_hydraulics_drops_the_level_to_off() {
        let mut s = hybrid(AircraftId::Ah64);
        assert_eq!(s.stability_in_effect(), Some(StabilityLevel::Damper));
        s.systems.fluids.hydraulic = 0.;
        assert_eq!(s.stability_in_effect(), Some(StabilityLevel::Off));
        assert_eq!(s.lift_controls.aids.stability, StabilityLevel::Damper);
        let lift = s.model().powered_lift().unwrap();
        s.lift_controls.body_rates = [0.3, 0., 0.];
        assert_eq!(
            s.augment(&lift, [0.; 3], Sensed::default()).augmentation,
            [0.; 3]
        );
        assert_eq!(hybrid(AircraftId::F16C).stability_in_effect(), None);
    }
}
