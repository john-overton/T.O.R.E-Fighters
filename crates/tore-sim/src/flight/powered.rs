//! Fitted continuous powered lift. Only the hybrid adapter calls this solver.
use super::{DT, FlightAxis, PilotInput, State, trace};
use crate::{
    attitude::{Basis, dot, unit},
    models::{
        FlightModel,
        variety::{LiftKind, PoweredLift},
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub vector_pitch: f64,
    pub vector_yaw: f64,
    pub conversion: f64,
    pub collective: f64,
    pub vector_pitch_actual: f64,
    pub vector_yaw_actual: f64,
    pub conversion_actual: f64,
    pub collective_actual: f64,
    /// Lagged force in lbf, coded exactly along with actuator positions.
    pub thrust_lbf: f64,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            vector_pitch: 0.,
            vector_yaw: 0.,
            conversion: 1.,
            collective: 0.,
            vector_pitch_actual: 0.,
            vector_yaw_actual: 0.,
            conversion_actual: 1.,
            collective_actual: 0.,
            thrust_lbf: 0.,
        }
    }
}
impl Controls {
    pub fn reset_ground(&mut self) {
        self.collective = 0.;
        self.collective_actual = 0.;
        self.thrust_lbf = 0.;
    }
    pub fn hover_fraction(&self, kind: LiftKind) -> f64 {
        match kind {
            LiftKind::VectorJet => self.vector_pitch_actual,
            LiftKind::Tiltrotor => self.conversion_actual,
            LiftKind::Helicopter => 1.,
        }
    }
    fn axis_mut(&mut self, axis: FlightAxis) -> &mut f64 {
        match axis {
            FlightAxis::VectorPitch => &mut self.vector_pitch,
            FlightAxis::VectorYaw => &mut self.vector_yaw,
            FlightAxis::Conversion => &mut self.conversion,
            FlightAxis::Collective => &mut self.collective,
        }
    }
    fn advance(&mut self, hydraulics: bool) {
        if !hydraulics {
            return;
        }
        for (actual, target, rate) in [
            (&mut self.vector_pitch_actual, self.vector_pitch, 0.25),
            (&mut self.vector_yaw_actual, self.vector_yaw, 1.),
            (&mut self.conversion_actual, self.conversion, 0.25),
            (&mut self.collective_actual, self.collective, 0.7),
        ] {
            *actual += (target - *actual).clamp(-rate * DT, rate * DT);
        }
    }
}
crate::flight::exact::exact_struct!(Controls {
    vector_pitch,
    vector_yaw,
    conversion,
    collective,
    vector_pitch_actual,
    vector_yaw_actual,
    conversion_actual,
    collective_actual,
    thrust_lbf,
});
impl State {
    /// Initialize a human airborne start after its final mass and altitude are set.
    /// This never trims an aircraft that has stepped or is supported by wheels.
    pub fn initialize_airborne_hover(&mut self) {
        if self.ticks != 0
            || self.native.is_some()
            || self
                .research
                .as_ref()
                .is_none_or(|research| research.on_ground)
        {
            return;
        }
        let Some(lift) = self
            .model()
            .powered_lift()
            .filter(|lift| lift.kind != LiftKind::VectorJet)
        else {
            return;
        };
        let c = self.model().configuration();
        let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        let capacity = c.propulsion.military_thrust_lbf
            * lift.efficiency
            * (-self.position[1].max(0.) / c.tuning.thrust_lapse_feet).exp();
        let collective = (weight / capacity).clamp(0., 1.);
        self.throttle = 1.;
        self.lift_controls.conversion = 1.;
        self.lift_controls.conversion_actual = 1.;
        self.lift_controls.collective = collective;
        self.lift_controls.collective_actual = collective;
        self.lift_controls.thrust_lbf = capacity * collective;
    }

    pub fn flight_axis_available(&self, axis: FlightAxis) -> bool {
        self.model().powered_lift().is_some_and(|lift| match axis {
            FlightAxis::VectorPitch | FlightAxis::VectorYaw => lift.kind == LiftKind::VectorJet,
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
                0.25,
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
        let rotation = std::array::from_fn(|i| {
            -basis_before.right[i] * self.pitch_rate * DT
                - basis_before.forward[i] * self.roll_rate * DT
                + basis_before.up[i] * yaw_rate * DT
        });
        let basis = basis_before.rotated(rotation);
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
            / (1. + loading * c.aerodynamics.loaded_elevator_percent / 100.);
        let requested_g = (1. + stick[0] * (max_g.max(1.) - 1.)).clamp(-1., max_g.max(1.));
        let wing_g = requested_g * wing_authority * t.regional.effects.lift;
        let drag = c.propulsion.military_thrust_lbf
            * lift.efficiency
            * lapse
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
                lift.horizontal_damping * hover
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
        let previous_position = self.position;
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
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
        research.contact(
            self,
            surface,
            c,
            wheel_load,
            runway_wind_fraction,
            previous_position,
        );
        self.research = Some(research);
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
    fn hover(id: AircraftId, height: f64) -> State {
        let aircraft = crate::models::variety::tests::synthetic(id);
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
        let weight = c.mass.empty_lbs + s.fuel;
        let lapse = (-height / c.tuning.thrust_lapse_feet).exp();
        let fraction = weight
            / ((c.propulsion.military_thrust_lbf * lift.efficiency + lift.additional_lift_lbf)
                * lapse);
        if lift.kind == LiftKind::VectorJet {
            s.throttle = fraction;
            s.lift_controls.vector_pitch = 1.;
            s.lift_controls.vector_pitch_actual = 1.;
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
    fn all_six_hover_climb_descend_and_lose_support_with_engine_off() {
        for id in IDS {
            let original = hover(id, 100.);
            let mut steady = original.clone();
            run(&mut steady, &Default::default(), 1200);
            assert!(
                (steady.position[1] - 100.).abs() < 0.01,
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
                climb.position[1] > 110.,
                "{id:?} climb {}",
                climb.position[1]
            );
            assert!(
                descend.position[1] < 90.,
                "{id:?} descend {}",
                descend.position[1]
            );
            let mut off = original.clone();
            off.command(super::super::PilotCommand::Set(
                super::super::Switch::Engine,
                false,
            ));
            run(&mut off, &Default::default(), 120);
            assert!(off.vertical_speed < -10., "{id:?} {}", off.vertical_speed);
        }
    }
    #[test]
    fn cyclic_and_yaw_control_translate_and_turn_each_hover_aircraft() {
        for id in IDS {
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
            assert!(s.pitch.abs() < 0.01 && s.bank.abs() < 0.01, "{id:?}");
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
        assert_eq!(jet.lift_controls.vector_pitch, 0.);
        assert!((jet.lift_controls.vector_pitch_actual - 0.75).abs() < 1e-12);
        assert!((jet.lift_controls.vector_yaw - 1.).abs() < 1e-12);
        assert!(jet.lift_controls.vector_yaw_actual > 0.99);
        let mut tilt = hover(AircraftId::V22, 500.);
        run(
            &mut tilt,
            &PilotInput {
                conversion_rate: -1.,
                ..Default::default()
            },
            120,
        );
        assert!((tilt.lift_controls.conversion - 0.75).abs() < 1e-12);
        assert!((tilt.lift_controls.conversion_actual - 0.75).abs() < 1e-12);
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
        s.command(super::super::PilotCommand::NeutralVector);
        assert_eq!(s.lift_controls.conversion, 0.);
        assert_eq!(s.lift_controls.conversion_actual, 1.);
        assert_eq!(s.lift_controls.collective, collective);
        assert_eq!(s.throttle, throttle);
        run(&mut s, &Default::default(), 120);
        assert!((s.lift_controls.conversion_actual - 0.75).abs() < 1e-12);
    }

    #[test]
    fn every_aircraft_accepts_a_gentle_vertical_landing_and_can_depart_again() {
        for id in IDS {
            let mut s = hover(id, 10.);
            s.gear = 1.;
            s.gear_down = true;
            s.velocity[1] = -2.;
            let clearance = s.model().configuration().equipment.ground_clearance_ft;
            s.position[1] = clearance + 0.001;
            run(&mut s, &Default::default(), 1);
            assert!(s.weight_on_wheels() && !s.crashed, "{id:?}");
            if s.model().powered_lift().unwrap().kind == LiftKind::VectorJet {
                s.throttle *= 1.2;
            } else {
                s.lift_controls.collective *= 1.2;
            }
            run(&mut s, &Default::default(), 360);
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
            let model = crate::models::AircraftModel::for_aircraft(
                &crate::models::variety::tests::synthetic(id),
            )
            .unwrap();
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
        let mut s = hover(AircraftId::Ah64, 5000.);
        s.position[1] = 15000.;
        s.fuel = 500.;
        s.set_payload(1500.).unwrap();
        s.initialize_airborne_hover();
        let weight = s.model().configuration().mass.empty_lbs + s.fuel + s.carried_lbs();
        assert!((s.lift_controls.thrust_lbf - weight).abs() < 1e-8);
        run(&mut s, &Default::default(), 1200);
        assert!((s.position[1] - 15000.).abs() < 0.01);
        let collective = s.lift_controls.collective;
        s.set_payload(2500.).unwrap();
        s.initialize_airborne_hover();
        assert_eq!(s.lift_controls.collective, collective);
        run(&mut s, &Default::default(), 600);
        assert!(s.position[1] < 14990.);
        let mut overloaded = hover(AircraftId::Ch47, 5000.);
        overloaded.position[1] = 15000.;
        overloaded.fuel = 500.;
        overloaded.set_payload(1500.).unwrap();
        overloaded.initialize_airborne_hover();
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
        run(&mut damaged, &Default::default(), 600);
        assert!(damaged.position[1] < 450. && damaged.vertical_speed < -10.);
    }
    #[test]
    fn conversion_gains_forward_motion_and_recovers_vertical_velocity() {
        for id in [AircraftId::Av8, AircraftId::Yak141, AircraftId::V22] {
            let mut s = hover(id, 5000.);
            run(
                &mut s,
                &PilotInput {
                    throttle: Some(1.),
                    conversion_rate: -1.,
                    vector_pitch_rate: -1.,
                    ..Default::default()
                },
                480,
            );
            assert!(
                s.lift_controls
                    .hover_fraction(s.model().powered_lift().unwrap().kind)
                    < DT
            );
            run(
                &mut s,
                &PilotInput {
                    throttle: Some(1.),
                    ..Default::default()
                },
                4800,
            );
            assert!(
                !s.crashed && s.position[1] > 1000.,
                "{id:?} {}",
                s.position[1]
            );
            assert!(s.velocity[0].hypot(s.velocity[2]) > 200., "{id:?}");
            assert!(s.vertical_speed.abs() < 5., "{id:?} {}", s.vertical_speed);
        }
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
