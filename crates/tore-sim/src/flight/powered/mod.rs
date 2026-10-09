//! The powered-lift aircraft: the AV-8 and Yak-141 vectoring jets, the V-22
//! tiltrotor and the AH-64, Mi-24 and CH-47 helicopters. Only the hybrid
//! adapter calls this solver.
//!
//! The VTOL overhaul replaces the fitted attitude-hold law below,
//! [`State::step_powered`], with a rigid body and physical rotors, nozzles
//! and wings, one slice at a time (its design moves into
//! docs/FLIGHT-MODEL.md when the project ships):
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
//!   and its trim (P2).
//! - [`drive`]: the rotor speed, governor and engines shared by every
//!   rotorcraft, for any number of rotors on one interconnected drive.
//! - [`jet`] and [`aero`]: the vectoring jets and their wing (P4).
//! - [`trim`]: the airborne and ground starts (P7), which replace the old
//!   hover start.
//! - [`tiltrotor`]: the V-22's nacelle rotors, mixer and corridor
//!   protection (P5).
//! - Still to come, in its own file: the CH-47's tandem mixer (P3). The
//!   parameters are in [`crate::models::variety::PoweredLift`].
//!
//! The CH-47 still flies the fitted law of the variety import,
//! `step_powered` below: its body rates are commanded, not integrated, and
//! it records them in the state's body rates.
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
pub mod tiltrotor;
pub mod trim;

#[cfg(test)]
mod easy_physics_tests;

use super::{DT, FlightAxis, PilotInput, State, airframe, trace};
use crate::{
    attitude::{Basis, dot, unit},
    models::{
        FlightModel,
        variety::{LiftKind, PoweredLift},
    },
};
pub use state::{Drive, LiftState, PilotAids, Rotor, TrimLatch, Warnings};

/// Airspeed, ft/s, above which the low-speed horizontal damping force stops
/// growing (fitted, agent decision 2026-10-08). Below it the damping rate is
/// the aircraft's own; well above it the force is about rate x 10 ft/s.
const LOW_SPEED_DAMPING_FPS: f64 = 10.;

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
        mut t: trace::AdapterTrace,
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
        const GRAVITY: f64 = 32.174;
        self.lift_controls
            .advance(self.systems.fluids.hydraulic > 0.);
        let hover = self.lift_controls.hover_fraction(lift.kind);
        let angle = hover * std::f64::consts::FRAC_PI_2;
        let carried = self.carried_lbs();
        let weight = c.mass.empty_lbs + self.fuel + carried;
        let envelope = c.aerodynamics.envelopes.iter().find(|e| e.g == 1).unwrap();
        let ceiling = envelope.points.iter().map(|p| p[1]).fold(0., f64::max);
        let (clean_stall, top_speed) = envelope
            .speeds(self.position[1].min(ceiling))
            .unwrap_or((200., 600.));
        let stall = clean_stall * (1. - self.flaps * 0.25);
        let basis_before = Basis::new(self.yaw, self.pitch, self.bank);
        let forward_speed = dot(self.velocity, basis_before.forward).max(0.);
        let wing_authority = if lift.kind == LiftKind::Helicopter {
            0.
        } else {
            (forward_speed / stall.max(1.)).powi(2).clamp(0., 1.)
                * super::ceiling_lift_ratio(self.position[1], ceiling)
        };
        // Low-speed attitude targets preserve cyclic control in hover. The
        // same commands gain ordinary rate control as forward airflow grows.
        let forward_controls = (1. - hover) * wing_authority;
        let available_power = if self.engine {
            (self.lift_controls.thrust_lbf / weight).clamp(0., 1.) * self.systems.power_available()
        } else {
            0.
        };
        let hover_control = available_power.clamp(0., 1.) * (1. - forward_controls);
        let target_pitch = stick[0] * lift.pitch_degrees.to_radians();
        let target_bank = stick[1] * lift.bank_degrees.to_radians();
        let max_rate = 45_f64.to_radians();
        let powered_rate = |rate: f64, axis: usize| {
            c.controls.map_or(rate, |controls| {
                rate.clamp(
                    f64::from(controls.auxiliary[axis].minimum).to_radians(),
                    f64::from(controls.auxiliary[axis].maximum).to_radians(),
                )
            })
        };
        let hover_pitch_rate = powered_rate(
            ((target_pitch - self.pitch) * 2.).clamp(-max_rate, max_rate),
            1,
        );
        let hover_roll_rate = powered_rate(
            ((target_bank - self.bank) * 2.).clamp(-max_rate, max_rate),
            0,
        );
        self.pitch_rate = hover_pitch_rate * hover_control + stick[0] * 0.25 * forward_controls;
        self.roll_rate = hover_roll_rate * hover_control
            + stick[1] * c.aerodynamics.roll_limit_rad_per_second * forward_controls;
        let turn = self.bank.sin() * GRAVITY / self.speed.max(stall.max(1.)) * forward_controls;
        let yaw_rate = powered_rate(stick[2] * lift.yaw_degrees_per_second.to_radians(), 2)
            * hover_control
            + turn;
        // Commanded rates, not integrated ones: the old law has no inertia.
        let rates = [self.roll_rate, self.pitch_rate, yaw_rate];
        self.lift_controls.body_rates = rates;
        let basis = basis_before.rotated(body::rotation(&basis_before, rates, DT));
        [self.yaw, self.pitch, self.bank] = basis.angles();
        let lapse = self
            .model()
            .response(crate::models::Conditions {
                altitude_msl_ft: self.position[1],
                tas_fps: self.speed,
                load_factor: self.g,
            })
            .thrust_lapse;
        let collective = if lift.kind == LiftKind::VectorJet {
            1.
        } else {
            self.lift_controls.collective_actual
        };
        let rated = if self.engine {
            (if afterburner {
                c.propulsion.afterburner_thrust_lbf
            } else {
                c.propulsion.military_thrust_lbf * self.throttle
            }) * lift.efficiency
                * collective
                + lift.additional_lift_lbf * self.throttle * angle.sin()
        } else {
            0.
        };
        let target_thrust = rated * lapse * self.systems.power_available();
        self.lift_controls.thrust_lbf +=
            (target_thrust - self.lift_controls.thrust_lbf) * (DT / lift.response_seconds).min(1.);
        let thrust = self.lift_controls.thrust_lbf;
        let vector_yaw = if lift.kind == LiftKind::VectorJet {
            self.lift_controls.vector_yaw_actual * 15_f64.to_radians()
        } else {
            0.
        };
        let thrust_direction: [f64; 3] = std::array::from_fn(|i| {
            basis.forward[i] * angle.cos() * vector_yaw.cos()
                + basis.up[i] * angle.sin() * vector_yaw.cos()
                + basis.right[i] * vector_yaw.sin()
        });
        let loading = (self.fuel + carried) / c.mass.empty_lbs;
        let max_g = c
            .aerodynamics
            .envelopes
            .iter()
            .filter(|e| {
                e.speeds(self.position[1])
                    .is_some_and(|(low, high)| self.speed >= low && self.speed <= high)
            })
            .map(|e| f64::from(e.g))
            .fold(1., f64::max)
            .max(
                super::fast_side_hold(&c.aerodynamics.envelopes, self.position[1], self.speed)
                    .map_or(1., |hold| hold.g),
            )
            / (1. + loading * c.aerodynamics.loaded_elevator_percent / 100.);
        let requested_g = (1. + stick[0] * (max_g.max(1.) - 1.)).clamp(-1., max_g.max(1.));
        let wing_g = requested_g * wing_authority * t.regional.effects.lift;
        // Forward drag reaches the reference force at the envelope's top speed.
        // Jets and the tiltrotor: their full rated thrust, with the afterburner
        // when they have one, as the fixed-wing adapter does. Helicopters fly
        // forward by tilting their rotor thrust, so their reference is the
        // forward force at their full attitude target while holding their
        // weight, less the saturated low-speed damping. Fitted, agent
        // decision 2026-10-08 (docs/spec/variety-flight.md).
        let reference = if lift.kind == LiftKind::Helicopter {
            weight
                * (lift.pitch_degrees.to_radians().tan()
                    - lift.horizontal_damping * LOW_SPEED_DAMPING_FPS / GRAVITY)
                    .max(0.05)
        } else {
            c.propulsion
                .military_thrust_lbf
                .max(c.propulsion.afterburner_thrust_lbf)
                * lift.efficiency
                * lapse
        };
        let drag = reference
            * (self.speed / top_speed.max(100.)).powi(2)
            * (1. + loading * c.aerodynamics.loaded_drag_percent / 100.)
            * (1. + t.regional.effects.drag_percent / 100.);
        let drag = drag.min(weight * self.speed / GRAVITY / DT);
        let direction = unit(self.velocity);
        let support = thrust_direction[1] * thrust / weight + basis.up[1] * wing_g;
        let wheel_load = (1. - support).clamp(0., 1.);
        self.g = dot(thrust_direction, basis.up) * thrust / weight + wing_g;
        self.lift_g = wing_g;
        self.maneuver = crate::telemetry::Maneuver {
            tick: self.ticks,
            commanded_g: requested_g,
            lift_g: wing_g,
            achieved_g: self.g,
            body_rates_rad_per_second: [self.roll_rate, self.pitch_rate, yaw_rate],
            rudder_command: stick[2],
            rudder_deflection: self.rudder,
            effective_rudder: stick[2],
            ..Default::default()
        };
        for (i, velocity) in self.velocity.iter_mut().enumerate() {
            let damping = if i == 1 {
                0.35 * hover + 0.5 * wing_authority
            } else {
                // Low-speed damping: its force stops growing above about
                // 10 ft/s, so forward flight is left to the drag above.
                lift.horizontal_damping * hover / (1. + self.speed / LOW_SPEED_DAMPING_FPS)
            };
            *velocity += (thrust_direction[i] * thrust / weight * GRAVITY
                + basis.up[i] * wing_g * GRAVITY
                - direction[i] * drag / weight * GRAVITY
                - *velocity * damping
                - if i == 1 { GRAVITY } else { 0. })
                * DT;
        }
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        t.power = trace::PowerTrace {
            engine: self.engine,
            fuel_starved: self.fuel + self.systems.external_lbs() <= 0.,
            afterburner,
            burner_blocked: self.burner_block(),
            throttle: self.throttle,
            afterburner_throttle: c.equipment.afterburner_throttle,
            fuel_flow_lbs_per_second: if self.engine { fuel_rate } else { 0. },
            unlimited_fuel: self.cheats.unlimited_fuel,
            rated_thrust_lbf: rated,
            lapse,
            power_available: self.systems.power_available(),
            thrust_lbf: thrust,
        };
        t.forces = trace::ForceTrace {
            weight_lbs: weight,
            carried_lbs: carried,
            payload_lbs: self.payload_lbs,
            ignore_weapon_weights: self.cheats.ignore_weapon_weights,
            drag: trace::DragTrace {
                total_lbf: drag,
                airframe_lbf: drag,
                ..Default::default()
            },
            achieved_g: self.g,
            support_g: support,
            wheel_load,
            ..Default::default()
        };
        t.envelope.clean_stall_fps = clean_stall;
        t.envelope.stall_fps = stall;
        t.envelope.top_speed_fps = top_speed;
        t.envelope.authority = wing_authority;
        t.envelope.limits_g = [-1., max_g];
        t.lift.commanded_g = requested_g;
        self.trace.0.adapter = Some(t);
        for (velocity, wind) in self.velocity.iter_mut().zip(initial_surface.wind) {
            *velocity += wind;
        }
        self.vertical_speed = self.velocity[1];
        let previous_position = self.advance_position();
        let surface = ground(self.position[0], self.position[2]);
        let mut research = self.research.take().expect("hybrid powered lift");
        if lift.kind != LiftKind::Helicopter && hover <= 0.5 {
            let departure = research.advance(
                c,
                self.speed,
                stall,
                self.pitch,
                stick[2],
                self.throttle,
                self.bank,
                self.roll_rate,
                dot(unit(self.velocity), basis.forward),
                false,
            );
            self.trace.0.adapter.as_mut().unwrap().departure = Some(departure);
        } else {
            research.departure = Default::default();
            research.spinning = 0;
            research.spin_rate = 0.;
            research.severity_f8 = 0;
            research.stall_active = false;
        }
        self.finish_contact(
            Some(research),
            c,
            surface,
            airframe::ContactInputs {
                wheel_load,
                runway_wind_fraction,
                previous_position,
                parked_in_wind: None,
            },
        );
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
        if matches!(id, AircraftId::Ah64 | AircraftId::Mi24) {
            helicopter::tests::pt_aircraft(id)
        } else if id == AircraftId::V22 {
            tiltrotor::acceptance::pt_v22()
        } else {
            crate::models::variety::tests::synthetic(id)
        }
    }
    /// The rotorcraft: the V-22 and CH-47 on the old attitude-hold law, the
    /// AH-64 and Mi-24 on their rotors since slice P2 (the jets have their
    /// own physics since slice P4, tested in `jet.rs`).
    const OLD_LAW: [AircraftId; 4] = [
        AircraftId::V22,
        AircraftId::Ah64,
        AircraftId::Mi24,
        AircraftId::Ch47,
    ];
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
        if let Some(tilt) = lift.tiltrotor {
            assert!(s.trim_tiltrotor(0., tilt.helicopter_nacelle_degrees));
            return s;
        }
        let weight = c.mass.empty_lbs + s.fuel;
        let lapse = (-height / c.tuning.thrust_lapse_feet).exp();
        let fraction = weight
            / ((c.propulsion.military_thrust_lbf * lift.efficiency + lift.additional_lift_lbf)
                * lapse);
        if lift.kind == LiftKind::VectorJet {
            jet::trim_hover(&mut s);
            return s;
        } else {
            s.throttle = 1.;
            s.lift_controls.collective = fraction;
            s.lift_controls.collective_actual = fraction;
        }
        s.lift_controls.thrust_lbf = weight;
        s
    }
    fn run(s: &mut State, input: &PilotInput, ticks: usize) {
        for _ in 0..ticks {
            s.step_surface(input, |_, _| crate::research::Surface::runway(0.));
        }
    }
    #[test]
    fn a_helicopter_reaches_nearly_its_top_speed_at_full_forward_stick() {
        // Before 2026-10-08 the low-speed damping acted at every speed and the
        // drag was scaled to full rotor thrust, so helicopters topped out near
        // 40 kt whatever their envelope said. Synthetic 166 kt envelope. The
        // CH-47 still flies this law; the AH-64 and Mi-24 fly their rotors.
        let mut aircraft = crate::models::variety::tests::synthetic(AircraftId::Ch47);
        for e in &mut aircraft.envelopes {
            e.points = vec![[60., 0.], [70., 7_000.], [260., 7_000.], [280., 0.]];
        }
        let mut s = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        let top = 280. - 20. * 1_000. / 7_000.;
        let mut collective = s.lift_controls.collective;
        let mut speeds = Vec::new();
        for stick in [0.25, 0.5, 1.] {
            for _ in 0..120 * 120 {
                collective = (collective
                    + (-0.0005 * s.velocity[1] + 0.00005 * (1_000. - s.position[1])))
                    .clamp(0., 1.);
                let input = PilotInput {
                    pitch: -stick,
                    roll: (-2. * s.bank).clamp(-1., 1.),
                    throttle: Some(1.),
                    collective: Some(collective),
                    ..Default::default()
                };
                run(&mut s, &input, 1);
            }
            assert!((s.position[1] - 1_000.).abs() < 200., "{}", s.position[1]);
            speeds.push(s.speed);
        }
        assert!(speeds.windows(2).all(|w| w[1] > w[0]), "{speeds:?}");
        assert!(
            speeds[2] > 0.9 * top && speeds[2] < 1.001 * top,
            "{} of {top}",
            speeds[2]
        );
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
        for id in OLD_LAW {
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
            // The old law holds attitude; a rotor without stability
            // augmentation keeps the attitude it was left at.
            let lift = s.model().powered_lift().unwrap();
            let c = s.model().configuration();
            if helicopter::SingleRotor::new(&lift, c).is_none()
                && tiltrotor::Tiltrotor::new(&lift, c).is_none()
            {
                assert!(s.pitch.abs() < 0.01 && s.bank.abs() < 0.01, "{id:?}");
            }
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
        // (slices P5 and P7; the single-rotor helicopters: helicopter::tests);
        // the CH-47 below on the old law.
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
        let mut overloaded = hover(AircraftId::Ch47, 5000.);
        overloaded.position[1] = 15000.;
        overloaded.fuel = 500.;
        overloaded.set_payload(1500.).unwrap();
        overloaded.start_airborne([0.; 3]);
        assert_eq!(overloaded.lift_controls.collective, 1.);
        assert!(overloaded.lift_controls.thrust_lbf < 12000.);
        run(&mut overloaded, &Default::default(), 600);
        assert!(overloaded.position[1] < 14970.);
    }

    #[test]
    fn loaded_rotorcraft_and_reduced_power_have_no_automatic_hover_support() {
        let mut loaded = hover(AircraftId::Ch47, 500.);
        loaded.set_payload(3000.).unwrap();
        loaded.lift_controls.collective = 1.;
        loaded.lift_controls.collective_actual = 1.;
        run(&mut loaded, &Default::default(), 600);
        assert!(loaded.position[1] < 470. && loaded.vertical_speed < -5.);
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
