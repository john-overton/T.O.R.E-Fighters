//! What the cockpit reads of a powered-lift aircraft (VTOL overhaul design
//! section 6, slice P7): plain, pure functions of the flight state for the
//! HUD, the sound and the camera. Nothing here changes the flight and nothing
//! in the simulation reads it.
//!
//! - Rotorcraft (helicopters and the V-22): rotor speed NR, torque TQ and
//!   collective, as percentages.
//! - The V-22's nacelle angle against its 97.5-degree travel.
//! - The vectoring jets' lift engines.
//! - The hover display of the retail manual's thrust-vectoring section
//!   (pp. 62 and 81): the vertical velocity bars and the horizontal velocity
//!   circle, shown by the jets below their stall speed and by the rotorcraft
//!   below 40 knots.
//! - The rotor buffet (vortex ring state and retreating blade stall), which
//!   has no message in the real aircraft but shakes the view and the rotor's
//!   sound.

use super::{
    helicopter::{Instant, SingleRotor},
    rotor::{self, Hazards},
};
use crate::{
    attitude::Basis,
    flight::{DT, State},
    models::{FlightModel, variety::LiftKind},
};

const KNOTS_TO_FPS: f64 = 1.687_81;

/// The V-22 nacelles' travel, degrees: 0 on the downstops (airplane mode) to
/// 97.5 (design 4.8).
///
/// TODO(P5): the tiltrotor law owns this constant; use it from there.
pub const NACELLE_TRAVEL_DEGREES: f64 = 97.5;

/// The ground speed below which a rotorcraft shows the hover display, kt
/// (design section 6).
pub const ROTORCRAFT_HOVER_DISPLAY_KNOTS: f64 = 40.;

/// The hover display's two instruments (manual pp. 62 and 81).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverDisplay {
    /// Ground velocity along the heading, knots, forward positive.
    pub forward_knots: f64,
    /// Ground velocity across the heading, knots, right positive.
    pub right_knots: f64,
    /// Vertical speed, ft/s, climb positive.
    pub vertical_fps: f64,
}

impl State {
    /// A helicopter or the V-22.
    pub fn is_rotorcraft(&self) -> bool {
        self.model()
            .powered_lift()
            .is_some_and(|lift| lift.kind != LiftKind::VectorJet)
    }

    /// Rotor speed NR as a percentage of its governed 100, for the
    /// rotorcraft.
    pub fn rotor_speed_percent(&self) -> Option<f64> {
        self.is_rotorcraft()
            .then_some(self.lift_controls.drive.rotor_speed * 100.)
    }

    /// The collective lever's actual position, percent, for the rotorcraft.
    pub fn collective_percent(&self) -> Option<f64> {
        self.is_rotorcraft()
            .then(|| self.lift_controls.collective_actual.clamp(0., 1.) * 100.)
    }

    /// Torque TQ, percent of the rated power: the shaft power the engines
    /// deliver over the rated sea-level power and the rotor speed. Above 100
    /// is over the limit. For the aircraft that fly the old fitted law (the
    /// CH-47 and the V-22) the engines are not modelled yet, so the share of
    /// the rotors' maximum thrust stands in.
    ///
    /// TODO(P3, P5): read the tandem and tiltrotor drives' power.
    pub fn torque_percent(&self) -> Option<f64> {
        let lift = self.model().powered_lift()?;
        let rotor = lift.rotor?;
        if lift.kind == LiftKind::VectorJet {
            return None;
        }
        if let Some(heli) = SingleRotor::new(&lift, self.model().configuration()) {
            let nr = self.lift_controls.drive.rotor_speed.max(0.5);
            return Some(
                (self.lift_controls.drive.engine_output[0] / (heli.rated_power * nr)).max(0.)
                    * 100.,
            );
        }
        Some((self.lift_controls.thrust_lbf / rotor.max_thrust_lbf.max(1.)).max(0.) * 100.)
    }

    /// The V-22's nacelle angle, degrees from the downstops, and the angle
    /// the pilot's demand asks for.
    ///
    /// TODO(P5): the tiltrotor law's own nacelle state replaces the
    /// conversion axis' reading here.
    pub fn nacelle_degrees(&self) -> Option<[f64; 2]> {
        (self.model().powered_lift()?.kind == LiftKind::Tiltrotor).then(|| {
            [
                self.lift_controls.conversion_actual,
                self.lift_controls.conversion,
            ]
            .map(|axis| axis.clamp(0., 1.) * NACELLE_TRAVEL_DEGREES)
        })
    }

    /// The nozzle demand, degrees: where the nozzles are heading.
    pub fn nozzle_demand_degrees(&self) -> f64 {
        let range = self
            .model()
            .powered_lift()
            .and_then(|lift| lift.jet)
            .map_or(super::jet::DEFAULT_NOZZLE_RANGE_DEGREES, |jet| {
                jet.nozzle_range_degrees
            });
        self.lift_controls.vector_pitch.clamp(0., 1.) * range
    }

    /// The vectoring jet's lift engines are running (the Yak-141's).
    pub fn lift_engines_running(&self) -> bool {
        self.lift_controls.drive.lift_engine_spool > 0.
            && self
                .model()
                .powered_lift()
                .is_some_and(|lift| lift.jet.is_some_and(|jet| jet.lift_engines.is_some()))
    }

    /// The hover display's readings, when the aircraft shows it: the jets
    /// below their stall speed, the helicopters and the V-22 below 40 knots
    /// of ground speed, airborne or not. None for any other aircraft.
    pub fn hover_display(&self) -> Option<HoverDisplay> {
        let lift = self.model().powered_lift()?;
        if self.crashed {
            return None;
        }
        let ground_knots = self.velocity[0].hypot(self.velocity[2]) / KNOTS_TO_FPS;
        let shown = if lift.kind == LiftKind::VectorJet {
            let (stall, _) = self
                .model()
                .configuration()
                .aerodynamics
                .envelopes
                .iter()
                .find(|e| e.g == 1)?
                .speeds(self.position[1])?;
            self.speed < stall * (1. - 0.25 * self.flaps)
        } else {
            ground_knots < ROTORCRAFT_HOVER_DISPLAY_KNOTS
        };
        shown.then(|| {
            let basis = Basis::new(self.yaw, 0., 0.);
            let along = |axis: [f64; 3]| {
                (self.velocity[0] * axis[0] + self.velocity[2] * axis[2]) / KNOTS_TO_FPS
            };
            HoverDisplay {
                forward_knots: along(basis.forward),
                right_knots: along(basis.right),
                vertical_fps: self.velocity[1],
            }
        })
    }

    /// How hard the main rotor buffets, 0 to 1: the deeper of the vortex
    /// ring state and retreating blade stall. The real aircraft give no
    /// message for either, only vibration, so this shakes the view and
    /// changes the rotor's sound. Always computed with every hazard on, so
    /// the Easy flight physics cheat (which removes their effects) keeps the
    /// cue.
    pub fn rotor_buffet(&self) -> f64 {
        let Some(lift) = self.model().powered_lift() else {
            return 0.;
        };
        let Some(heli) = SingleRotor::new(&lift, self.model().configuration()) else {
            return 0.;
        };
        if self.crashed || self.lift_controls.drive.rotor_speed <= 0. {
            return 0.;
        }
        let instant = Instant {
            basis: Basis::new(self.yaw, self.pitch, self.bank),
            air_velocity: self.velocity,
            density: rotor::air_density(self.position[1]),
            rotor_speed: self.lift_controls.drive.rotor_speed,
            rotor: self.lift_controls.rotors[0],
            engine_power: self.lift_controls.drive.engine_output[0],
            collective: self.lift_controls.collective_actual,
            controls: [0.; 3],
            hub_height_agl_ft: None,
            seconds: self.ticks as f64 * DT,
            hazards: Hazards::ALL,
            drag_factor: 1.,
            lift_factor: 1.,
        };
        let main = heli.loads(&instant).main;
        main.vortex_ring.max(main.blade_stall).clamp(0., 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{helicopter::tests::pt_aircraft, jet::tests::fixture};
    use super::*;
    use tore_formats::aircraft::AircraftId;

    fn state(id: AircraftId) -> State {
        let aircraft = match id {
            AircraftId::Ah64 | AircraftId::Mi24 => pt_aircraft(id),
            _ => fixture(id),
        };
        let mut s = State::new(&aircraft, [0., 3_000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s
    }

    #[test]
    fn the_rotor_readouts_exist_for_rotorcraft_only() {
        let mut heli = state(AircraftId::Ah64);
        assert!(heli.start_airborne([0.; 3]));
        assert!((heli.rotor_speed_percent().unwrap() - 100.).abs() < 1e-9);
        let torque = heli.torque_percent().unwrap();
        assert!((20. ..100.).contains(&torque), "torque {torque}");
        assert_eq!(heli.collective_percent().unwrap().round(), {
            (heli.lift_controls.collective_actual * 100.).round()
        });
        let jet = state(AircraftId::Av8);
        assert_eq!(jet.rotor_speed_percent(), None);
        assert_eq!(jet.torque_percent(), None);
        assert_eq!(jet.collective_percent(), None);
        assert_eq!(jet.nacelle_degrees(), None);
    }

    #[test]
    fn the_hover_display_follows_the_manual_and_the_design_thresholds() {
        // A helicopter flies the display below 40 kt, not above.
        let mut slow = state(AircraftId::Ah64);
        slow.start_airborne([0.; 3]);
        slow.velocity = [0., 0., 30. * KNOTS_TO_FPS];
        assert!(slow.hover_display().is_some());
        slow.velocity = [0., 0., 50. * KNOTS_TO_FPS];
        assert!(slow.hover_display().is_none());
        // Readings are along and across the heading, knots and ft/s.
        let mut drift = state(AircraftId::Ah64);
        drift.yaw = std::f64::consts::FRAC_PI_2;
        drift.velocity = [10. * KNOTS_TO_FPS, 5., 0.];
        let h = drift.hover_display().unwrap();
        assert!((h.forward_knots - 10.).abs() < 1e-9 && h.right_knots.abs() < 1e-9);
        assert_eq!(h.vertical_fps, 5.);
        drift.yaw = 0.;
        let h = drift.hover_display().unwrap();
        assert!((h.right_knots - 10.).abs() < 1e-9 && h.forward_knots.abs() < 1e-9);
        // A jet shows it below its stall speed, with the nozzles down or not.
        let mut jet = state(AircraftId::Av8);
        jet.start_airborne([0.; 3]);
        assert!(jet.hover_display().is_none(), "wingborne");
        assert!(jet.trim_hover());
        assert!(jet.hover_display().is_some(), "hovering");
        // Any other aircraft has none.
        let plain = State::new(&crate::flight::integration_tests::profile(), [0.; 3]).unwrap();
        assert!(plain.hover_display().is_none());
    }

    #[test]
    fn the_lift_engines_and_the_nozzle_demand_are_readable() {
        let mut yak = state(AircraftId::Yak141);
        assert!(!yak.lift_engines_running());
        assert!(yak.trim_hover());
        assert!(yak.lift_engines_running());
        assert!((yak.nozzle_degrees() - 90.).abs() < 1e-6);
        yak.lift_controls.vector_pitch = 0.6;
        assert!((yak.nozzle_demand_degrees() - 60.).abs() < 1e-6);
        assert!(!state(AircraftId::Av8).lift_engines_running());
    }

    #[test]
    fn the_rotor_buffet_rises_in_vortex_ring_and_in_blade_stall_not_in_a_hover() {
        let mut s = state(AircraftId::Ah64);
        assert!(s.start_airborne([0.; 3]));
        assert!(s.rotor_buffet() < 0.05, "trimmed forward flight");
        assert!(s.trim_hover());
        assert!(s.rotor_buffet() < 0.05, "trimmed hover");
        // Settling through the disk at one hover induced velocity with
        // nothing across it: deep in the vortex ring state.
        let vh = s.lift_controls.rotors[0].induced_fps;
        let mut ring = s.clone();
        ring.velocity = [0., -vh, 0.];
        assert!(
            ring.rotor_buffet() > 0.5,
            "vortex ring {}",
            ring.rotor_buffet()
        );
        // Far past the never-exceed speed: retreating blade stall.
        let mut fast = s.clone();
        fast.velocity = [0., 0., 260. * KNOTS_TO_FPS];
        assert!(
            fast.rotor_buffet() > 0.2,
            "blade stall {}",
            fast.rotor_buffet()
        );
        // None of the other aircraft buffet.
        assert_eq!(state(AircraftId::Av8).rotor_buffet(), 0.);
    }

    #[test]
    fn nothing_here_changes_the_flight() {
        let mut s = state(AircraftId::Mi24);
        s.start_airborne([0.; 3]);
        let before = s.clone();
        let _ = (
            s.rotor_buffet(),
            s.hover_display(),
            s.torque_percent(),
            s.rotor_speed_percent(),
        );
        assert_eq!(s, before);
    }
}
