//! The fuel a rotorcraft burns (slice HF2, John's requests of 2026-10-13).
//!
//! A helicopter or the V-22 burns by the shaft power its engines deliver,
//! not by a throttle. The PT record's `fuelConsumption` (1 lb/s for the
//! AH-64, V-22 and CH-47, 3 lb/s for the Mi-24, whole pounds per second) is
//! a placeholder for them and is not read: it is the same whole number a jet
//! carries, it does not move with the rotor's load, and the Mi-24's 3 lb/s
//! empties its 3,307 lb in 18 minutes where the real aircraft flies about two
//! hours. See [aircraft records](../../../../../docs/formats/aircraft.md#rotorcraft-fuel-records).
//!
//! The flow is the turboshaft line, **fitted**:
//!
//! `flow = full_flow x (IDLE_SHARE + (1 - IDLE_SHARE) x power / rated_power)`
//!
//! with `full_flow = rated shaft hp x SFC / 3600` lb/s. The engines' power is
//! the drive's own lagged output, so a hover burns what the rotor draws to
//! hover, a cruise less, a flare more, and an engine on the ground with the
//! collective down only the idle share.
//!
//! The AI and the crew voice plan with one steady figure instead: a planning
//! figure that follows the moment's power would move the bingo line with
//! every climb and descent. Since slice HF3 (John, 2026-10-13: "helicopter
//! bingo planned at cruise flow") that figure is the flow at the AI's cruise
//! home, [`crate::ai::route::ROTORCRAFT_CRUISE_FRACTION`] of the envelope's
//! top speed, level at sea level, worked out once when the model is built
//! ([`cruise_power`]) and scaled with the weight. Slice HF2 planned at the
//! hover flow, which sent helicopters home early.

use super::{
    helicopter::SingleRotor,
    rotor::{self, Hazards},
    tandem::Tandem,
    tiltrotor::Tiltrotor,
};
use crate::{
    flight::State,
    models::{
        FlightModel,
        config::Configuration,
        variety::{LiftKind, PoweredLift},
    },
};

/// Specific fuel consumption at rated power, pounds an hour per shaft
/// horsepower delivered to the rotors (**fitted**): 0.55 for turboshafts of
/// this class (T700, TV3-117 and T55 run 0.45 to 0.60) over the 8 percent of
/// the engines' power the transmission and installation take before the
/// rotors, the loss the rated powers in `models/variety/lift.rs` already
/// assume.
pub const SPECIFIC_FUEL_CONSUMPTION_LB_PER_HP_HOUR: f64 = 0.6;

/// The fuel flow with no load on the rotor (flight idle), as a share of the
/// flow at rated power (**fitted**: a turboshaft burns about a fifth to a
/// quarter of its full flow at idle).
pub const IDLE_SHARE: f64 = 0.22;

/// Horsepower in ft·lbf/s.
const HORSEPOWER: f64 = 550.;

/// One rotorcraft's fuel numbers, from its drive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorFuel {
    /// Rated shaft power, ft·lbf/s.
    pub rated_power: f64,
    /// Hover power at the reference weight, ft·lbf/s.
    pub hover_power: f64,
    /// Empty plus full internal fuel, lb.
    pub reference_weight: f64,
    /// Level cruise power at the reference weight, ft·lbf/s
    /// ([`cruise_power`]); none when no level trim was found there.
    pub cruise_power: Option<f64>,
}

impl RotorFuel {
    /// Pounds a second at rated power.
    pub fn full_flow(&self) -> f64 {
        self.rated_power / HORSEPOWER * SPECIFIC_FUEL_CONSUMPTION_LB_PER_HP_HOUR / 3600.
    }

    /// Pounds a second with the engines delivering `power` (ft·lbf/s).
    pub fn flow(&self, power: f64) -> f64 {
        let share = (power / self.rated_power.max(1.)).clamp(0., 1.2);
        self.full_flow() * (IDLE_SHARE + (1. - IDLE_SHARE) * share)
    }

    /// Pounds a second hovering at `weight` lb (sea level, out of ground
    /// effect): the induced power grows with the weight to the 1.5.
    pub fn hover_flow(&self, weight: f64) -> f64 {
        let ratio = (weight / self.reference_weight.max(1.)).max(0.);
        self.flow(self.hover_power * ratio.powf(1.5))
    }

    /// Pounds a second the AI plans with at `weight` lb (slice HF3): the
    /// level cruise flow, its power in proportion to the weight (**fitted**:
    /// at cruise most of the power is profile and parasite drag, which the
    /// weight barely moves, so this errs high for a heavy aircraft and low
    /// for a light one by a few percent), or the hover flow when the model
    /// has no cruise trim.
    pub fn planning_flow(&self, weight: f64) -> f64 {
        match self.cruise_power {
            Some(power) => {
                let ratio = (weight / self.reference_weight.max(1.)).max(0.);
                self.flow(power * ratio)
            }
            None => self.hover_flow(weight),
        }
    }
}

/// The power a helicopter or the V-22 draws flying level at the AI's cruise
/// home ([`crate::ai::route::ROTORCRAFT_CRUISE_FRACTION`] of its envelope's
/// top speed at sea level, the V-22 with its nacelles on the downstops) at
/// empty plus full internal fuel, sea level, ft·lbf/s; none for any other
/// aircraft or when the trim finds no level flight there. Sea level, where a
/// rotorcraft's cruise costs most, keeps the planning on the safe side.
pub fn cruise_power(lift: &PoweredLift, c: &Configuration) -> Option<f64> {
    lift.rotor?;
    if lift.kind == LiftKind::VectorJet {
        return None;
    }
    let top = c
        .aerodynamics
        .envelopes
        .iter()
        .filter_map(|e| e.speeds(0.))
        .map(|(_, high)| high)
        .fold(f64::NEG_INFINITY, f64::max);
    if !top.is_finite() || top <= 0. {
        return None;
    }
    let speed = top * crate::ai::route::ROTORCRAFT_CRUISE_FRACTION;
    let weight = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
    let density = rotor::sea_level_density();
    let level = tore_input::StabilityLevel::Off;
    let power = if let Some(heli) = SingleRotor::new(lift, c) {
        heli.trim_with(Hazards::ALL, weight, 0., speed, density, 1., level)?
            .engine_power
    } else if let Some(model) = Tandem::new(lift, c) {
        model
            .trim_with(Hazards::ALL, weight, 0., speed, density, 1., level)?
            .engine_power
    } else {
        let model = Tiltrotor::new(lift, c)?;
        let available = model.drive.available_power(density, 1., 1.);
        model
            .trim(
                Hazards::ALL,
                weight,
                0.,
                speed,
                0.,
                0.,
                1.,
                level,
                available,
            )?
            .engine_power
    };
    (power.is_finite() && power > 0.).then_some(power)
}

impl State {
    /// The fuel numbers of a helicopter or the V-22, or none for any other
    /// aircraft.
    pub fn rotor_fuel(&self) -> Option<RotorFuel> {
        let lift = self.model().powered_lift()?;
        lift.rotor?;
        if lift.kind == LiftKind::VectorJet {
            return None;
        }
        let c = self.model().configuration();
        let (drive, reference_weight) = if let Some(heli) = SingleRotor::new(&lift, c) {
            (heli.drive, heli.reference_weight)
        } else if let Some(model) = Tandem::new(&lift, c) {
            (model.drive, model.reference_weight)
        } else {
            let model = Tiltrotor::new(&lift, c)?;
            (model.drive, model.reference_weight)
        };
        Some(RotorFuel {
            rated_power: drive.rated_power,
            hover_power: drive.hover_power,
            reference_weight,
            cruise_power: self.model().rotor_cruise_power(),
        })
    }

    /// The pounds a second a rotorcraft burns this moment: the turboshaft
    /// line at the power the engines deliver. Zero with the engines off.
    pub fn rotorcraft_fuel_flow(&self) -> Option<f64> {
        let fuel = self.rotor_fuel()?;
        Some(if self.engine {
            fuel.flow(self.lift_controls.drive.engine_output[0])
        } else {
            0.
        })
    }

    /// The flow the AI and the crew voice plan endurance with, pounds a
    /// second: the throttle times the military flow for an aeroplane (the
    /// B48 host rule, floored at 10 percent throttle), the level cruise flow
    /// at the aircraft's weight now for a rotorcraft
    /// ([`RotorFuel::planning_flow`], **fitted**, steady).
    pub fn endurance_flow_lbs_per_second(&self) -> f64 {
        let flow = match self.rotor_fuel() {
            Some(fuel) => fuel.planning_flow(
                self.model().configuration().mass.empty_lbs + self.fuel + self.carried_lbs(),
            ),
            None => {
                self.model()
                    .configuration()
                    .propulsion
                    .military_fuel_lbs_per_second
                    * self.throttle.max(0.1)
            }
        };
        flow.max(crate::ai::mission::MINIMUM_FUEL_FLOW_LBS_PER_S)
    }

    /// The flow a full internal tank is planned with: as
    /// [`Self::endurance_flow_lbs_per_second`], at the weight with full
    /// internal fuel and nothing carried.
    pub fn full_tank_endurance_flow_lbs_per_second(&self) -> f64 {
        let c = self.model().configuration();
        let flow = match self.rotor_fuel() {
            Some(fuel) => fuel.planning_flow(c.mass.empty_lbs + c.mass.internal_fuel_lbs),
            None => c.propulsion.military_fuel_lbs_per_second * self.throttle.max(0.1),
        };
        flow.max(crate::ai::mission::MINIMUM_FUEL_FLOW_LBS_PER_S)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FUEL: RotorFuel = RotorFuel {
        rated_power: 1_000_000.,
        hover_power: 600_000.,
        reference_weight: 20_000.,
        cruise_power: Some(400_000.),
    };

    #[test]
    fn the_flow_follows_the_power_the_rotor_draws() {
        let rated = FUEL.full_flow();
        assert!((FUEL.flow(FUEL.rated_power) - rated).abs() < 1e-12);
        assert!((FUEL.flow(0.) - rated * IDLE_SHARE).abs() < 1e-12);
        assert!(FUEL.flow(0.) < FUEL.flow(400_000.));
        assert!(FUEL.flow(400_000.) < FUEL.flow(600_000.));
        assert!(FUEL.flow(600_000.) < rated);
    }

    #[test]
    fn planning_takes_the_cruise_flow_scaled_by_weight_or_else_the_hover() {
        assert!((FUEL.planning_flow(20_000.) - FUEL.flow(400_000.)).abs() < 1e-12);
        assert!((FUEL.planning_flow(10_000.) - FUEL.flow(200_000.)).abs() < 1e-12);
        assert!(FUEL.planning_flow(20_000.) < FUEL.hover_flow(20_000.));
        let hover_only = RotorFuel {
            cruise_power: None,
            ..FUEL
        };
        assert_eq!(
            hover_only.planning_flow(18_000.),
            hover_only.hover_flow(18_000.)
        );
    }

    #[test]
    fn a_heavier_hover_burns_more() {
        assert!(FUEL.hover_flow(24_000.) > FUEL.hover_flow(20_000.));
        assert!((FUEL.hover_flow(20_000.) - FUEL.flow(FUEL.hover_power)).abs() < 1e-12);
    }
}
