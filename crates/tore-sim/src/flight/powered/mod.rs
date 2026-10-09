//! The powered-lift aircraft: the AV-8 and Yak-141 vectoring jets, the V-22
//! tiltrotor and the AH-64, Mi-24 and CH-47 helicopters. Only the hybrid
//! adapter calls this solver.
//!
//! The VTOL overhaul replaced the fitted attitude-hold law of the variety
//! import with a rigid body and physical rotors, nozzles and wings, one slice
//! at a time (its design is in docs/FLIGHT-MODEL.md). Every aircraft here has
//! its own force law; [`State::step_powered`] only dispatches:
//!
//! - [`body`]: the rigid body (P1). Force laws build a
//!   [`body::Moments`] from their rotors, puffers and surfaces, take the
//!   inertia from [`body::Inertia::from_weight`] with the aircraft's
//!   [`crate::models::variety::BodyParameters`], and call
//!   [`State::advance_body`]; they integrate velocity and position after it.
//! - [`state`]: [`LiftState`], everything a powered-lift aircraft adds to
//!   the exact flight state, and the powered-lift commands (P1).
//! - The conventional step's envelope, drag and contact pieces are in
//!   `flight/airframe.rs` (`State::envelope_limits`, `State::airframe_drag`,
//!   `State::advance_position`, `State::finish_contact`) for the wings and
//!   the contact of every kind.
//! - [`sas`]: the trim-set latch and trim, applied to the stick before any
//!   force law runs, and the stability levels' feedback, which the force
//!   laws call through [`State::augment`] with their own air data (P6).
//! - [`rotor`]: one lifting rotor, reusable by every rotorcraft (P2);
//!   [`fuselage`]: the helicopters' fuselage and fixed surfaces (P2);
//!   [`helicopter`]: the single-rotor AH-64 and Mi-24 force law, its drive
//!   and its trim (P2); [`tandem`]: the CH-47's two rotors (P3).
//! - [`drive`]: the rotor speed, governor and engines shared by every
//!   rotorcraft, for any number of rotors on one interconnected drive.
//! - [`jet`] and [`aero`]: the vectoring jets and their wing (P4).
//! - [`trim`]: the airborne and ground starts (P7).
//! - [`tiltrotor`]: the V-22's nacelle rotors, mixer and corridor
//!   protection (P5).
//!
//! The parameters are in [`crate::models::variety::PoweredLift`].
pub mod aero;
pub mod body;
pub mod drive;
pub mod fuselage;
pub mod helicopter;
pub mod jet;
pub mod readout;
pub mod rotor;
pub mod sas;
pub mod state;
pub mod tandem;
pub mod tiltrotor;
pub mod trim;

#[cfg(test)]
mod easy_physics_tests;
#[cfg(test)]
mod hover_hold_tests;

use super::{DT, FlightAxis, PilotInput, State, airframe, trace};
#[cfg(test)]
use crate::models::FlightModel;
use crate::models::variety::{LiftKind, PoweredLift};
pub use state::{Drive, LiftState, PilotAids, Rotor, TrimLatch, Warnings};

impl State {
    pub fn flight_axis_available(&self, axis: FlightAxis) -> bool {
        self.model().powered_lift().is_some_and(|lift| match axis {
            FlightAxis::VectorPitch => lift.kind == LiftKind::VectorJet,
            // Neither vectoring jet vectors sideways (manual, thrust
            // vectoring table; VTOL overhaul slice P4).
            FlightAxis::VectorYaw => false,
            FlightAxis::Conversion => lift.kind == LiftKind::Tiltrotor,
            FlightAxis::Collective => lift.kind != LiftKind::VectorJet,
        })
    }
    pub(super) fn command_lift_axis(&mut self, axis: FlightAxis, value: f64, adjust: bool) {
        if !value.is_finite()
            || self.research.is_none()
            || self.native.is_some()
            || !self.flight_axis_available(axis)
        {
            return;
        }
        let current = self.lift_controls.axis_mut(axis);
        let minimum = if axis == FlightAxis::VectorYaw {
            -1.
        } else {
            0.
        };
        *current = (if adjust { *current + value } else { value }).clamp(minimum, 1.);
    }
    pub(super) fn update_lift_demands(&mut self, input: &PilotInput) {
        // A held conversion key moves the V-22's nacelle demand at the
        // nacelles' own rate (slice P5).
        let conversion_speed = self
            .model()
            .powered_lift()
            .and_then(|lift| lift.tiltrotor)
            .map_or(0.25, |t| {
                t.nacelle_rate_degrees_per_second / t.nacelle_range_degrees
            });
        for (axis, position, rate, speed) in [
            (
                FlightAxis::VectorPitch,
                input.vector_pitch,
                input.vector_pitch_rate,
                0.25,
            ),
            (
                FlightAxis::VectorYaw,
                input.vector_yaw,
                input.vector_yaw_rate,
                1.,
            ),
            (
                FlightAxis::Conversion,
                input.conversion,
                input.conversion_rate,
                conversion_speed,
            ),
            (
                FlightAxis::Collective,
                input.collective,
                input.collective_rate,
                0.35,
            ),
        ] {
            if let Some(position) = position {
                self.command_lift_axis(axis, position, false);
            }
            self.command_lift_axis(axis, rate * speed * DT, true);
        }
    }
    #[allow(clippy::too_many_arguments)] // Shared tick data already calculated by the adapter.
    pub(super) fn step_powered(
        &mut self,
        lift: PoweredLift,
        c: &crate::models::config::Configuration,
        stick: [f64; 3],
        initial_surface: crate::research::Surface,
        runway_wind_fraction: f64,
        t: trace::AdapterTrace,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
        afterburner: bool,
        fuel_rate: f64,
    ) {
        // The V-22 flies its proprotors, nacelles and wing (P5).
        if let Some(model) = tiltrotor::Tiltrotor::new(&lift, c) {
            self.step_tiltrotor(
                lift,
                model,
                c,
                stick,
                initial_surface,
                runway_wind_fraction,
                t,
                ground,
                fuel_rate,
            );
            return;
        }
        // The single-rotor helicopters fly their rotor physics (P2).
        if let Some(heli) = helicopter::SingleRotor::new(&lift, c) {
            self.step_single_rotor(
                lift,
                heli,
                c,
                stick,
                initial_surface,
                runway_wind_fraction,
                t,
                ground,
                fuel_rate,
            );
            return;
        }
        // The tandem CH-47 flies its two rotors (P3).
        if let Some(model) = tandem::Tandem::new(&lift, c) {
            self.step_tandem(
                lift,
                model,
                c,
                stick,
                initial_surface,
                runway_wind_fraction,
                t,
                ground,
                fuel_rate,
            );
            return;
        }
        if let Some(jet) = lift.jet {
            return self.step_jet(
                lift,
                jet,
                c,
                stick,
                initial_surface,
                runway_wind_fraction,
                t,
                ground,
                afterburner,
                fuel_rate,
            );
        }
        // Every powered-lift aircraft has its own force law above.
        unreachable!("{:?} has no force law", lift.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::aircraft::AircraftId;
    const IDS: [AircraftId; 6] = [
        AircraftId::Av8,
        AircraftId::Yak141,
        AircraftId::V22,
        AircraftId::Ah64,
        AircraftId::Mi24,
        AircraftId::Ch47,
    ];
    /// The synthetic record of `id`; the single-rotor helicopters and the
    /// V-22 carry their PT's flight numbers, since their rotors size their
    /// power.
    fn fixture(id: AircraftId) -> tore_formats::aircraft::Aircraft {
        if id == AircraftId::Ch47 {
            tandem::tests::pt_aircraft()
        } else if matches!(id, AircraftId::Ah64 | AircraftId::Mi24) {
            helicopter::tests::pt_aircraft(id)
        } else if id == AircraftId::V22 {
            tiltrotor::acceptance::pt_v22()
        } else {
            crate::models::variety::tests::synthetic(id)
        }
    }
    /// The aircraft whose hover the generic control test below flies: the V-22
    /// and the single-rotor helicopters.
    const HOVER_CRAFT: [AircraftId; 3] = [AircraftId::V22, AircraftId::Ah64, AircraftId::Mi24];
    fn hover(id: AircraftId, height: f64) -> State {
        let aircraft = fixture(id);
        let mut s = State::new(&aircraft, [0., height, 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.speed = 0.;
        s.velocity = [0.; 3];
        let lift = s.model().powered_lift().unwrap();
        let c = s.model().configuration();
        if helicopter::SingleRotor::new(&lift, c).is_some() {
            assert!(s.trim_single_rotor(0.), "{id:?}");
            return s;
        }
        if tandem::Tandem::new(&lift, c).is_some() {
            assert!(s.trim_tandem(0.), "{id:?}");
            return s;
        }
        if let Some(tilt) = lift.tiltrotor {
            assert!(s.trim_tiltrotor(0., tilt.helicopter_nacelle_degrees));
            return s;
        }
        assert!(lift.jet.is_some(), "{id:?} has a force law of its own");
        jet::trim_hover(&mut s);
        s
    }
    fn run(s: &mut State, input: &PilotInput, ticks: usize) {
        for _ in 0..ticks {
            s.step_surface(input, |_, _| crate::research::Surface::runway(0.));
        }
    }
    #[test]
    fn afterburner_jets_are_held_to_their_top_speed_by_drag() {
        // Drag at the top speed equals the afterburner thrust, as in the
        // fixed-wing adapter, so full burner cannot exceed the envelope.
        let mut aircraft = crate::models::variety::tests::synthetic(AircraftId::Yak141);
        aircraft.fields.get_mut("thrust").unwrap().value = "20000".into();
        aircraft.fields.get_mut("aftThrust").unwrap().value = "34000".into();
        let mut s = State::new(&aircraft, [0., 10_000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        let top = 1_800. - 500. * 10_000. / 50_000.;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.speed = top;
        s.velocity = [0., 0., top];
        s.throttle = 1.;
        s.burner = true;
        run(&mut s, &PilotInput::default(), 1);
        let c = s.model().configuration();
        let t = s.trace().adapter.unwrap();
        let loading = (s.fuel + s.carried_lbs()) / c.mass.empty_lbs;
        let expected = 34_000.
            * t.power.lapse
            * (s.speed / top).powi(2)
            * (1. + loading * c.aerodynamics.loaded_drag_percent / 100.);
        assert!(
            (t.forces.drag.total_lbf / expected - 1.).abs() < 0.01,
            "{} vs {expected}",
            t.forces.drag.total_lbf
        );
    }
    #[test]
    fn all_six_hover_climb_descend_and_lose_support_with_engine_off() {
        for id in IDS {
            // Out of ground effect, where a trimmed rotor hovers steadily.
            let original = hover(id, 1_000.);
            let mut steady = original.clone();
            run(&mut steady, &Default::default(), 1200);
            assert!(
                (steady.position[1] - 1_000.).abs() < 0.1,
                "{id:?} {}",
                steady.position[1]
            );
            assert!(!steady.crashed && steady.stall_alert(0.).is_none());
            let mut climb = original.clone();
            let mut descend = original.clone();
            if climb.model().powered_lift().unwrap().kind == LiftKind::VectorJet {
                climb.throttle *= 1.1;
                descend.throttle *= 0.9;
            } else {
                climb.lift_controls.collective *= 1.1;
                descend.lift_controls.collective *= 0.9;
            }
            run(&mut climb, &Default::default(), 600);
            run(&mut descend, &Default::default(), 600);
            assert!(
                climb.position[1] > 1_010.,
                "{id:?} climb {}",
                climb.position[1]
            );
            assert!(
                descend.position[1] < 990.,
                "{id:?} descend {}",
                descend.position[1]
            );
            let mut off = original.clone();
            off.command(super::super::PilotCommand::Set(
                super::super::Switch::Engine,
                false,
            ));
            // A rotor's stored energy holds it up a moment longer.
            run(&mut off, &Default::default(), 240);
            assert!(off.vertical_speed < -10., "{id:?} {}", off.vertical_speed);
        }
    }
    #[test]
    fn cyclic_and_yaw_control_translate_and_turn_each_hover_aircraft() {
        for id in HOVER_CRAFT {
            let mut s = hover(id, 500.);
            run(
                &mut s,
                &PilotInput {
                    pitch: -0.4,
                    roll: 0.3,
                    yaw: 0.5,
                    ..Default::default()
                },
                360,
            );
            assert!(s.pitch.abs() > 0.03 && s.bank.abs() > 0.03, "{id:?}");
            assert!(s.velocity[0].hypot(s.velocity[2]) > 3., "{id:?}");
            assert!(s.yaw > 0.1, "{id:?}");
            run(&mut s, &Default::default(), 360);
            assert!(!s.crashed);
        }
    }
    #[test]
    fn actuator_travel_rates_and_neutral_are_tick_owned_and_bounded() {
        let mut jet = hover(AircraftId::Av8, 500.);
        jet.command(super::super::PilotCommand::NeutralVector);
        run(
            &mut jet,
            &PilotInput {
                vector_yaw_rate: 1.,
                ..Default::default()
            },
            120,
        );
        // The nozzles travel 100 degrees a second from vertical to aft, and
        // the jets have no vector yaw to move (slice P4).
        assert_eq!(jet.lift_controls.vector_pitch, 0.);
        assert_eq!(jet.lift_controls.vector_pitch_actual, 0.);
        assert_eq!(jet.lift_controls.vector_yaw, 0.);
        assert_eq!(jet.lift_controls.vector_yaw_actual, 0.);
        // A held conversion key moves the V-22's nacelle demand at the
        // nacelles' 8 degrees a second; in a hover the corridor stops the
        // nacelles at its edge, 85 degrees at rest (slice P5).
        let mut tilt = hover(AircraftId::V22, 500.);
        run(
            &mut tilt,
            &PilotInput {
                conversion_rate: -1.,
                ..Default::default()
            },
            120,
        );
        assert!((tilt.lift_controls.conversion * 97.5 - 79.).abs() < 1e-9);
        assert!(
            (tilt.nacelle_degrees() - 85.).abs() < 1.,
            "{}",
            tilt.nacelle_degrees()
        );
        assert!(tilt.position.iter().all(|v| v.is_finite()));
        let mut rotor = hover(AircraftId::Ah64, 100.);
        let before = rotor.lift_controls;
        rotor.command(super::super::PilotCommand::SetAxis(
            FlightAxis::VectorPitch,
            1.,
        ));
        assert_eq!(rotor.lift_controls, before);
    }
    #[test]
    fn tiltrotor_neutral_returns_conversion_forward_and_preserves_lift_levers() {
        let mut s = hover(AircraftId::V22, 5000.);
        let collective = s.lift_controls.collective;
        let throttle = s.throttle;
        let start = s.lift_controls.conversion_actual;
        s.command(super::super::PilotCommand::NeutralVector);
        assert_eq!(s.lift_controls.conversion, 0.);
        assert_eq!(s.lift_controls.conversion_actual, start);
        assert_eq!(s.lift_controls.collective, collective);
        assert_eq!(s.throttle, throttle);
        // In a hover the corridor holds the nacelles at its edge, 85
        // degrees at rest (slice P5).
        run(&mut s, &Default::default(), 120);
        assert!(
            (s.nacelle_degrees() - 85.).abs() < 1.,
            "{}",
            s.nacelle_degrees()
        );
        assert!(s.lift_controls.corridor_hold.is_some());
    }

    #[test]
    fn fixed_gear_rotorcraft_start_down_and_ignore_retraction_but_hind_does_not() {
        use crate::flight::{PilotCommand, Switch};
        for id in [AircraftId::Ah64, AircraftId::Ch47] {
            let mut s = hover(id, 100.);
            assert!(s.gear_down && s.gear == 1.);
            s.command(PilotCommand::Toggle(Switch::Gear));
            s.command(PilotCommand::Set(Switch::Gear, false));
            run(&mut s, &Default::default(), 120);
            assert!(s.gear_down && s.gear == 1. && !s.crashed);
            // Old snapshots or direct inspection demands cannot create a
            // retractable mechanism before the next authoritative contact step.
            s.gear = 0.;
            s.gear_down = false;
            let clearance = s.model().configuration().equipment.ground_clearance_ft;
            s.position[1] = clearance + 0.001;
            s.velocity[1] = -2.;
            run(&mut s, &Default::default(), 1);
            assert!(s.weight_on_wheels() && !s.crashed && s.gear == 1.);
        }
        let mut hind = hover(AircraftId::Mi24, 100.);
        assert!(!hind.model().fixed_gear() && !hind.gear_down && hind.gear == 0.);
        hind.command(PilotCommand::Toggle(Switch::Gear));
        run(&mut hind, &Default::default(), 360);
        assert!(hind.gear_down && hind.gear > 0.99);
    }

    #[test]
    fn every_aircraft_accepts_a_gentle_vertical_landing_and_can_depart_again() {
        for id in IDS {
            let mut s = hover(id, 10.);
            if !s.model().fixed_gear() {
                s.gear = 1.;
                s.gear_down = true;
            }
            s.velocity[1] = -2.;
            let clearance = s.model().configuration().equipment.ground_clearance_ft;
            s.position[1] = clearance + 0.001;
            run(&mut s, &Default::default(), 1);
            assert!(s.weight_on_wheels() && !s.crashed, "{id:?}");
            // The jets' engines spool up (slower from low power) before they
            // lift off.
            let ticks = if s.model().powered_lift().unwrap().kind == LiftKind::VectorJet {
                s.throttle *= 1.2;
                600
            } else {
                s.lift_controls.collective *= 1.2;
                360
            };
            run(&mut s, &Default::default(), ticks);
            assert!(
                !s.weight_on_wheels() && s.position[1] > clearance + 5.,
                "{id:?} {}",
                s.position[1]
            );
        }
    }
    #[test]
    fn powered_state_restoration_and_input_replay_continue_exactly() {
        for id in IDS {
            let mut s = hover(id, 500.);
            // Every field the VTOL overhaul added, away from its default.
            s.lift_controls = state::tests::busy(s.lift_controls);
            run(
                &mut s,
                &PilotInput {
                    collective_rate: 0.1,
                    vector_yaw_rate: -0.2,
                    pitch: -0.1,
                    roll: 0.05,
                    ..Default::default()
                },
                120,
            );
            let model = crate::models::AircraftModel::for_aircraft(&fixture(id)).unwrap();
            let mut writer = tore_codec::BitWriter::new();
            s.write_exact(&mut writer, None).unwrap();
            let bytes = writer.as_bytes();
            let mut reader = tore_codec::BitReader::new(bytes);
            let mut restored = State::read_exact(&mut reader, None, &model).unwrap();
            assert_eq!(s, restored, "{id:?}");
            for tick in 0..1200 {
                let input = PilotInput {
                    pitch: (tick as f64 / 120.).sin() * 0.1,
                    yaw: 0.2,
                    collective_rate: if tick < 120 { -0.1 } else { 0. },
                    ..Default::default()
                };
                run(&mut s, &input, 1);
                run(&mut restored, &input, 1);
                assert_eq!(s, restored, "{id:?} tick {tick}");
            }
        }
    }
    #[test]
    fn airborne_start_uses_final_mass_and_altitude_without_later_retrim() {
        // The V-22 trims into wingborne forward flight on its own physics
        // (slices P5 and P7; the helicopters: helicopter::tests and
        // tandem::tests).
        let mut s = hover(AircraftId::V22, 5000.);
        s.position[1] = 3000.;
        s.fuel = 500.;
        s.set_payload(1500.).unwrap();
        assert!(s.start_airborne([0.; 3]));
        assert!(s.speed > 100., "forward flight, not a hover");
        run(&mut s, &Default::default(), 1200);
        assert!((s.position[1] - 3000.).abs() < 10., "{}", s.position[1]);
        let collective = s.lift_controls.collective;
        s.set_payload(2500.).unwrap();
        s.start_airborne([0.; 3]);
        assert_eq!(s.lift_controls.collective, collective);
        let height = s.position[1];
        run(&mut s, &Default::default(), 600);
        // A heavier aircraft is not retrimmed: it sinks relative to where it
        // was (the wing carries most of a wingborne V-22's weight).
        assert!(s.position[1] < height - 0.5, "{} {height}", s.position[1]);
    }

    #[test]
    fn loaded_rotorcraft_and_reduced_power_have_no_automatic_hover_support() {
        let mut damaged = hover(AircraftId::Ah64, 500.);
        damaged.throttle *= 0.5;
        // The rotor's speed sags first, then the aircraft.
        run(&mut damaged, &Default::default(), 1200);
        assert!(
            damaged.position[1] < 450. && damaged.vertical_speed < -10.,
            "{} {}",
            damaged.position[1],
            damaged.vertical_speed
        );
    }
    #[test]
    fn legacy_powered_aircraft_ignore_new_demands_and_native_stays_restricted() {
        let aircraft = crate::models::variety::tests::synthetic(AircraftId::Av8);
        let mut legacy = State::new(&aircraft, [0., 5000., 0.]).unwrap();
        let before = legacy.lift_controls;
        legacy.command(super::super::PilotCommand::SetAxis(
            FlightAxis::VectorPitch,
            1.,
        ));
        run(
            &mut legacy,
            &PilotInput {
                vector_pitch_rate: 1.,
                ..Default::default()
            },
            120,
        );
        assert_eq!(legacy.lift_controls, before);
        assert_eq!(legacy.trace().path, trace::Path::Legacy);
    }
}
