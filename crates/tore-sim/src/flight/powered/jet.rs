//! The vectoring jets, the AV-8 and the Yak-141 (VTOL overhaul design 4.7,
//! slice P4): one continuous physics from the hover to wingborne flight,
//! with no mode switch. Each tick the nozzles, the puffer jets, the intakes
//! and the angle-of-attack wing ([`super::aero`]) all act, each with its
//! own physical authority, on the rigid body ([`super::body`]).
//!
//! - **Nozzles** turn 0 (aft) to the PT's braking stop (`vtLimitDown`, 100
//!   degrees: 10 degrees forward of vertical) at the PT rate (`vtSpeed`,
//!   100 degrees per second). The demand moves by the nozzle keys and the
//!   nozzle lever (slice P6); the nozzles follow it here.
//! - **Main thrust** acts through the centre of gravity along the nozzles,
//!   `cos θ forward + sin θ up`, times an efficiency that falls from 1 with
//!   the nozzles aft to the aircraft's vertical efficiency with them down
//!   (turning losses, bleed, reingestion). The engine spools up and down
//!   with separate lags, slower from idle.
//! - **Puffer jets** at nose, tail and wing tips give pure moments. A full
//!   valve gives the PT `puffRot` onset acceleration on its axis times the
//!   bleed authority `clamp((θ - 10°) / 10°, 0, 1) x sqrt(thrust / maximum)`,
//!   so they live only with the nozzles down and the engine turning. Valve
//!   demand costs up to 8 percent of main thrust. They add no damping of
//!   their own: at the Off stability level a hovering jet keeps rotating
//!   until the pilot stops it.
//! - **Intake momentum drag**: the air the engine swallows, `mdot = thrust
//!   / V_jet`, is turned at the intakes ahead of the centre of gravity. Its
//!   sideways and vertical parts push the intakes downwind (the axial part
//!   is in the net thrust and the airframe drag), which yaws the nose away
//!   from the airflow; with the jet-induced dihedral that rolls the aircraft
//!   away from a sideslip, that is the Harrier's low-speed roll-off, the
//!   physics behind the manual's warnings about sideways stick and rudder
//!   near stall speed. No crash is scripted.
//! - **Lift engines** (Yak-141): straight up the body, started when the main
//!   nozzle passes 30 degrees with the engine running and below 200 kt,
//!   spooled in over 2 s, throttled with the main engine, stopped below 20
//!   degrees or above 200 kt; takeoff and landing only. They burn their own
//!   fuel. The afterburner stays blocked above 20 percent nozzle travel.
//! - **Suck-down**: within one span of the ground the jets lose up to 6
//!   percent of their lift at wheel height.
//! - **No vector yaw**: neither aircraft vectors sideways; low-speed yaw is
//!   the tail puffer through the pedals.
//! - **On the wheels** a rolling moment that beats the wheels' righting arm
//!   tips the aircraft, and past 15 degrees of bank it is a crash (dynamic
//!   rollover): the manual's "don't move the stick sideways" on a vertical
//!   takeoff.
//!
//! The stability levels (slice P6, [`super::sas`]) act on the puffer valves
//! once a tick through [`State::augment`]; the wing's surfaces fly the
//! pilot's own command, since its law already commands G and roll rate.
//! At Damper, the default, the puffers' rate limit holds full stick at the
//! PT `puffRot` maximum rate. The hazards a player can
//! remove with the Easy flight physics cheat are [`Hazards`], read through
//! [`State::jet_hazards`], the hook slice P8 fills.
//!
//! Every constant here is `fitted` (agent decisions, 2026-10-08, slice P4)
//! unless its line says otherwise; the per-aircraft values are in
//! [`crate::models::variety::JetParameters`].

use super::super::{DT, State, airframe, trace};
use super::{
    aero::{self, AirData, RollLaw, WingInputs},
    body::{GRAVITY, Inertia, Moments},
};
use crate::{
    attitude::{Basis, Vector, dot, unit},
    models::{
        Conditions, FlightModel,
        config::Configuration,
        variety::{JetParameters, PoweredLift},
    },
};

const KNOTS_TO_FPS: f64 = 1.687_81;
/// Nozzle angles between which the puffer valves come alive, degrees
/// (design 4.7).
const PUFFER_START_DEGREES: f64 = 10.;
const PUFFER_FULL_DEGREES: f64 = 20.;
/// Main thrust bled at full valve demand (design 4.7).
const PUFFER_BLEED: f64 = 0.08;
/// Jet lift lost at wheel height to suck-down and reingestion (design 4.7).
const SUCK_DOWN: f64 = 0.06;
/// Spooling up from idle takes up to this many times longer than from
/// hover power.
const IDLE_SPOOL_FACTOR: f64 = 2.;
/// Bank on the wheels past which the aircraft rolls over (design 4.9).
const ROLLOVER_DEGREES: f64 = 15.;
/// The wing carries the aircraft, for the stall warning and departure,
/// with the nozzles below this angle and above the stall speed (design 4.2).
const WINGBORNE_NOZZLE_DEGREES: f64 = 45.;
/// The nose wheel's arm against a nose-up moment on the ground, ft: the
/// aircraft rotates for a rolling takeoff near its stall speed, not on
/// its own at taxi speed.
const NOSE_GEAR_ARM_FT: f64 = 6.;

/// The physical hazards of the vectoring jets that the Easy flight physics
/// cheat removes (design 4.12). All on by default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hazards {
    /// The intake momentum drag's yawing moment and the jet-induced
    /// dihedral: the low-speed roll-off.
    pub roll_off: bool,
    /// At the Off stability level the puffers have no damping. Without it
    /// the stability law runs at Damper when the pilot chose Off (slice P8
    /// passes the level into [`super::sas::augment`]).
    pub undamped_puffers: bool,
    /// Tipping over on the wheels.
    pub dynamic_rollover: bool,
}

impl Default for Hazards {
    fn default() -> Self {
        Self {
            roll_off: true,
            undamped_puffers: true,
            dynamic_rollover: true,
        }
    }
}

/// Main-thrust efficiency at nozzle angle `nozzle` (rad): 1 aft, the
/// vertical efficiency from 90 degrees on.
pub fn nozzle_efficiency(jet: &JetParameters, nozzle: f64) -> f64 {
    1. - (1. - jet.vertical_efficiency) * nozzle.clamp(0., std::f64::consts::FRAC_PI_2).sin()
}

/// The puffers' authority, 0..1: the valves open with the nozzles down and
/// blow engine bleed, so they weaken with the thrust.
pub fn puffer_authority(nozzle_degrees: f64, thrust_lbf: f64, maximum_lbf: f64) -> f64 {
    puffer_gate(nozzle_degrees) * (thrust_lbf / maximum_lbf.max(1.)).clamp(0., 1.).sqrt()
}

fn puffer_gate(nozzle_degrees: f64) -> f64 {
    ((nozzle_degrees - PUFFER_START_DEGREES) / (PUFFER_FULL_DEGREES - PUFFER_START_DEGREES))
        .clamp(0., 1.)
}

/// Thrust lost to suck-down at `height_ft` above the wheels' contact plane
/// with the nozzles at `nozzle` (rad).
pub fn suck_down(jet: &JetParameters, height_ft: f64, nozzle: f64) -> f64 {
    SUCK_DOWN * (1. - height_ft / jet.span_ft).clamp(0., 1.) * nozzle.sin().max(0.)
}

/// The intake momentum drag: its force (world axes, lbf) and its moments
/// with the jet-induced dihedral (body axes [roll, pitch, yaw], ft·lbf).
/// With `roll_off` off the yawing moment and the dihedral are gone.
pub fn intake(
    jet: &JetParameters,
    air: &AirData,
    basis: &Basis,
    thrust_lbf: f64,
    nozzle: f64,
    roll_off: bool,
) -> (Vector, [f64; 3]) {
    let flow = thrust_lbf.max(0.) / jet.jet_velocity_fps;
    let [_, side, normal] = air.body;
    let force = std::array::from_fn(|i| -flow * (side * basis.right[i] + normal * basis.up[i]));
    let pitch = -flow * jet.intake_arm_ft * normal;
    if !roll_off {
        return (force, [0., pitch, 0.]);
    }
    let yaw = -flow * jet.intake_arm_ft * side;
    let roll = -jet.jet_dihedral
        * jet.span_ft
        * flow
        * air.body[0].abs()
        * air.beta.sin()
        * air.beta.sin().abs()
        * nozzle.sin().max(0.);
    (force, [roll, pitch, yaw])
}

/// The nozzle travel of a vectoring jet whose PT record has none, degrees
/// (both jets' `vtLimitDown`).
pub const DEFAULT_NOZZLE_RANGE_DEGREES: f64 = 100.;

impl State {
    /// The nozzles' actual angle, degrees from aft (0) through vertical (90)
    /// to the braking stop (100 on both jets): what the nozzle animation
    /// draws and the HUD shows. Zero on aircraft without nozzles.
    pub fn nozzle_degrees(&self) -> f64 {
        let range = self
            .model()
            .powered_lift()
            .and_then(|lift| lift.jet)
            .map_or(DEFAULT_NOZZLE_RANGE_DEGREES, |jet| jet.nozzle_range_degrees);
        self.lift_controls.vector_pitch_actual.clamp(0., 1.) * range
    }

    /// The vectoring jets' hazards now: all on, until slice P8 turns them
    /// off with the Easy flight physics cheat.
    pub fn jet_hazards(&self) -> Hazards {
        Hazards::default()
    }

    /// One tick of a vectoring jet on the hybrid adapter. `stick` is the
    /// pilot's [pitch, roll, yaw] after damage; the wind has been taken out
    /// of the velocity and is put back here.
    #[allow(clippy::too_many_arguments)] // Shared tick data already calculated by the adapter.
    pub(super) fn step_jet(
        &mut self,
        lift: PoweredLift,
        jet: JetParameters,
        c: &Configuration,
        stick: [f64; 3],
        initial_surface: crate::research::Surface,
        runway_wind_fraction: f64,
        mut t: trace::AdapterTrace,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
        afterburner: bool,
        fuel_rate: f64,
    ) {
        let hazards = self.jet_hazards();
        // Nozzles at the PT rate; no vector yaw.
        let range = jet.nozzle_range_degrees;
        self.lift_controls.vector_yaw = 0.;
        if self.systems.fluids.hydraulic > 0. {
            let step = jet.nozzle_rate_degrees_per_second / range * DT;
            let n = &mut self.lift_controls;
            n.vector_pitch_actual += (n.vector_pitch - n.vector_pitch_actual).clamp(-step, step);
            n.vector_yaw_actual -= n.vector_yaw_actual.clamp(-DT, DT);
        }
        let nozzle_degrees = self.lift_controls.vector_pitch_actual * range;
        let nozzle = nozzle_degrees.to_radians();
        let carried = self.carried_lbs();
        let weight = c.mass.empty_lbs + self.fuel + carried;
        let mass = weight / GRAVITY;
        let inertia = Inertia::from_weight(weight, lift.body.radii_of_gyration_ft);
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let air = AirData::new(&basis, self.velocity, self.position[1]);
        let limits = self.envelope_limits(c);
        let wheel_contact = self.research.as_ref().is_some_and(|r| r.on_ground);
        let lapse = self
            .model()
            .response(Conditions {
                altitude_msl_ft: self.position[1],
                tas_fps: self.speed,
                load_factor: self.g,
            })
            .thrust_lapse;
        let power = self.systems.power_available();
        let military = c.propulsion.military_thrust_lbf;
        // Main engine spool.
        let rated = if self.engine {
            if afterburner {
                c.propulsion.afterburner_thrust_lbf
            } else {
                military * self.throttle
            }
        } else {
            0.
        };
        let target = rated * lapse * power;
        let output = self.lift_controls.drive.engine_output[0];
        let spooled = (output / (military * lapse).max(1.)).clamp(0., 1.);
        let lag = if target > output {
            jet.spool_up_seconds * (1. + IDLE_SPOOL_FACTOR * (1. - spooled).powi(2))
        } else {
            jet.spool_down_seconds
        };
        let main = output + (target - output) * (DT / lag).min(1.);
        self.lift_controls.drive.engine_output[0] = main;
        // Lift engines: takeoff and landing only.
        let mut lift_thrust = 0.;
        if let Some(engines) = jet.lift_engines {
            let drive = &mut self.lift_controls.drive;
            let slow = air.speed < engines.stop_speed_kt * KNOTS_TO_FPS;
            let start = self.engine && slow && nozzle_degrees >= engines.start_nozzle_degrees;
            let stop = !self.engine || !slow || nozzle_degrees < engines.stop_nozzle_degrees;
            let spool_target = if start {
                1.
            } else if stop {
                0.
            } else {
                f64::from(drive.lift_engine_spool > 0.)
            };
            let step = DT / engines.spool_seconds;
            drive.lift_engine_spool += (spool_target - drive.lift_engine_spool).clamp(-step, step);
            let target = drive.lift_engine_spool * engines.thrust_lbf * self.throttle * lapse;
            let current = drive.engine_output[1];
            let lag = if target > current {
                jet.spool_up_seconds
            } else {
                jet.spool_down_seconds
            };
            drive.engine_output[1] = current + (target - current) * (DT / lag).min(1.);
            lift_thrust = drive.engine_output[1];
            if lift_thrust > 0. {
                self.consume_fuel(
                    engines.fuel_lbs_per_second * lift_thrust / engines.thrust_lbf * DT,
                );
            }
        } else {
            self.lift_controls.drive.engine_output[1] = 0.;
            self.lift_controls.drive.lift_engine_spool = 0.;
        }
        // Puffers, through the stability augmentation.
        let profile = c
            .controls
            .expect("variety aircraft carry a handling profile");
        let authority = puffer_authority(nozzle_degrees, main, military);
        let axes = [
            profile.auxiliary[1],
            profile.auxiliary[0],
            profile.auxiliary[2],
        ];
        let puffer_acceleration =
            axes.map(|axis| f64::from(axis.acceleration).to_radians() * authority);
        let rates = self.lift_controls.body_rates;
        let augmented = self.augment(
            &lift,
            stick,
            super::sas::Sensed {
                airspeed_fps: air.speed,
                sideslip_rad: air.beta,
                torque_pedal: 0.,
            },
        );
        let demands = augmented.controls;
        let valve = demands.iter().fold(0_f64, |m, d| m.max(d.abs()));
        let bleed = PUFFER_BLEED * puffer_gate(nozzle_degrees) * valve;
        let puffers = [
            inertia.0[0] * puffer_acceleration[1] * demands[1],
            inertia.0[1] * puffer_acceleration[0] * demands[0],
            inertia.0[2] * puffer_acceleration[2] * demands[2],
        ];
        // Thrust with its losses.
        let height = self.position[1] - initial_surface.height - c.equipment.ground_clearance_ft;
        let suck = suck_down(&jet, height, nozzle);
        let main_force = main * nozzle_efficiency(&jet, nozzle) * (1. - bleed) * (1. - suck);
        let (sin, cos) = nozzle.sin_cos();
        let lift_force = lift_thrust * (1. - SUCK_DOWN * (1. - height / jet.span_ft).clamp(0., 1.));
        let thrust: Vector = std::array::from_fn(|i| {
            main_force * (cos * basis.forward[i] + sin * basis.up[i]) + lift_force * basis.up[i]
        });
        let (intake_force, intake_moments) =
            intake(&jet, &air, &basis, main, nozzle, hazards.roll_off);
        // The wing.
        let regional = t.regional.effects;
        let lift_scale = regional.lift * if self.systems.has(25) { 0.5 } else { 1. };
        let capacity = aero::lift_capacity(
            &c.aerodynamics.envelopes,
            self.position[1],
            weight,
            self.flaps,
            &air,
        );
        let tuning = self.model().tuning();
        let wing = aero::wing(
            &air,
            &WingInputs {
                shape: aero::JET_WING,
                roll: RollLaw::from_axis(profile.roll),
                basis,
                rates,
                inertia,
                weight_lbs: weight,
                capacity,
                lift_scale,
                limits: limits.limits,
                controls: augmented.pilot,
                rudder: self.rudder * regional.authority[2] + regional.yaw_bias,
                other_force: std::array::from_fn(|i| thrust[i] + intake_force[i]),
                rudder_slip: tuning.rudder_rate / tuning.alignment_rate,
                side_force: tuning.sideslip_force,
            },
        );
        // Airframe drag, the conventional terms with the wing's own G.
        let slip_fraction = if air.speed > 1e-9 {
            air.body[1] / air.speed
        } else {
            0.
        };
        let (drag, drag_trace) = self.airframe_drag(
            c,
            airframe::DragInputs {
                weight,
                top_speed: limits.top_speed,
                reference_thrust: military.max(c.propulsion.afterburner_thrust_lbf),
                lapse,
                loading: limits.loading,
                load_factor: wing.lift_g,
                slip_drag: weight * tuning.sideslip_drag * slip_fraction.powi(2) * wing.authority,
                drag_percent: self.device_drag_percent(limits.top_speed),
                wheel_contact,
                damage_percent: regional.drag_percent,
            },
        );
        // The conventional terms hold a fixed share of the weight for the gear,
        // the flaps and the G pull at any airspeed, which only makes sense in
        // wingborne flight: below the stall speed they fade with dynamic
        // pressure like every other aerodynamic force.
        let drag = drag * wing.authority;
        let direction = unit(self.velocity);
        let force: Vector = std::array::from_fn(|i| {
            thrust[i] + intake_force[i] + wing.force[i] - direction[i] * drag
        });
        let support = force[1] / weight;
        let wheel_load = (1. - support).clamp(0., 1.);
        // Moments, then the body.
        let mut applied: [f64; 3] =
            std::array::from_fn(|i| wing.moments.applied[i] + puffers[i] + intake_moments[i]);
        let mut tipping = false;
        if wheel_contact {
            // The nose wheel holds the nose down until the wing can lift it.
            if self.pitch <= 1e-9 && applied[1] > 0. {
                applied[1] = (applied[1] - wheel_load * weight * NOSE_GEAR_ARM_FT).max(0.);
            }
            // The wheels hold the wings level against a rolling moment up to
            // their righting arm; past it the aircraft tips.
            let righting = wheel_load * weight * jet.half_track_ft;
            if hazards.dynamic_rollover && (self.bank.abs() > 1e-6 || applied[0].abs() > righting) {
                tipping = true;
                // Right wing down is both positive roll and positive bank.
                let lean = if self.bank.abs() > 1e-6 {
                    self.bank.signum()
                } else {
                    applied[0].signum()
                };
                applied[0] -= lean * righting;
            } else {
                applied[0] = 0.;
                self.lift_controls.body_rates[0] = 0.;
            }
        }
        let moments = Moments {
            applied,
            damping: wing.moments.damping,
        };
        let bank_before = self.bank;
        self.advance_body(inertia, moments);
        for (i, velocity) in self.velocity.iter_mut().enumerate() {
            *velocity += (force[i] / mass - if i == 1 { GRAVITY } else { 0. }) * DT;
        }
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            t.forces.speed_capped_from_fps = Some(self.speed);
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        self.g = dot(force, basis.up) / weight;
        self.lift_g = wing.lift_g;
        let body_rates = self.lift_controls.body_rates;
        self.maneuver = crate::telemetry::Maneuver {
            tick: self.ticks,
            commanded_g: wing.commanded_g,
            lift_g: wing.lift_g,
            achieved_g: self.g,
            body_rates_rad_per_second: body_rates,
            rudder_command: stick[2],
            rudder_deflection: self.rudder,
            effective_rudder: augmented.pilot[2],
            ..Default::default()
        };
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
            power_available: power,
            thrust_lbf: main + lift_thrust,
        };
        self.lift_controls.thrust_lbf = main + lift_thrust;
        t.forces = trace::ForceTrace {
            weight_lbs: weight,
            carried_lbs: carried,
            payload_lbs: self.payload_lbs,
            ignore_weapon_weights: self.cheats.ignore_weapon_weights,
            drag: trace::DragTrace {
                total_lbf: drag,
                ..drag_trace
            },
            achieved_g: self.g,
            support_g: support,
            wheel_load,
            speed_capped_from_fps: t.forces.speed_capped_from_fps,
        };
        t.envelope = limits.trace(stick[0], wing.commanded_g);
        t.envelope.authority = wing.authority;
        t.lift.commanded_g = wing.commanded_g;
        t.lift.target_g = wing.commanded_g;
        t.lift.lagged_g = wing.lift_g;
        self.trace.0.adapter = Some(t);
        for (velocity, wind) in self.velocity.iter_mut().zip(initial_surface.wind) {
            *velocity += wind;
        }
        self.vertical_speed = self.velocity[1];
        let previous_position = self.advance_position();
        let surface = ground(self.position[0], self.position[2]);
        let mut research = self.research.take().expect("hybrid powered lift");
        if nozzle_degrees < WINGBORNE_NOZZLE_DEGREES && air.speed > limits.stall {
            let departure = research.advance(
                c,
                self.speed,
                limits.stall,
                self.pitch,
                stick[2],
                self.throttle,
                self.bank,
                self.roll_rate,
                dot(direction, basis.forward),
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
        let tipped_bank = self.bank;
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
        if self.weight_on_wheels() && !self.crashed {
            self.settle_on_wheels(tipping, bank_before, tipped_bank, wheel_load);
        }
    }

    /// The body rates and bank of a jet on its wheels after the contact:
    /// the wheels stop pitch into the ground and steer the yaw, and a jet
    /// that is tipping keeps its bank until it rolls back onto its wheels
    /// or over (a crash past 15 degrees).
    fn settle_on_wheels(&mut self, tipping: bool, bank_before: f64, tipped: f64, wheel_load: f64) {
        let steering = (self.gear >= 0.99).then(|| {
            let basis = Basis::new(self.yaw, self.pitch, self.bank);
            dot(self.velocity, basis.forward) / 18. * self.nosewheel_angle().sin()
        });
        let rates = &mut self.lift_controls.body_rates;
        if (self.pitch <= 0. && rates[1] < 0.)
            || (self.pitch >= 20_f64.to_radians() && rates[1] > 0.)
        {
            rates[1] = 0.;
        }
        if let Some(steering) = steering {
            rates[2] += (steering - rates[2]) * wheel_load;
        }
        if tipping {
            let landed = bank_before.abs() > 1e-6 && tipped * bank_before <= 0.;
            if landed {
                rates[0] = 0.;
            } else {
                self.bank = tipped;
            }
            if self.bank.abs() > ROLLOVER_DEGREES.to_radians() {
                self.crashed = true;
                self.engine = false;
                self.burner = false;
                self.velocity = [0.; 3];
                self.speed = 0.;
                self.systems.notify("Rolled over on the ground");
            }
        } else {
            rates[0] = 0.;
        }
        self.roll_rate = self.lift_controls.body_rates[0];
        self.pitch_rate = self.lift_controls.body_rates[1];
    }
}

/// Sets a vectoring jet at rest in the air into a hover with the nozzles
/// vertical and its engines spooled to just hold its weight, for the tests
/// (the trimmed starts are slice P7's).
#[cfg(test)]
pub(crate) fn trim_hover(s: &mut State) {
    let lift = s.model().powered_lift().unwrap();
    let jet = lift.jet.unwrap();
    let c = s.model().configuration();
    let weight = c.mass.empty_lbs + s.fuel + s.carried_lbs();
    let lapse = (-s.position[1].max(0.) / c.tuning.thrust_lapse_feet).exp();
    let height = s.position[1] - c.equipment.ground_clearance_ft;
    let vertical = 90. / jet.nozzle_range_degrees;
    let nozzle = std::f64::consts::FRAC_PI_2;
    let lift_engines = jet.lift_engines.map_or(0., |e| e.thrust_lbf);
    let military = c.propulsion.military_thrust_lbf;
    let throttle = weight
        / ((military * nozzle_efficiency(&jet, nozzle) + lift_engines)
            * lapse
            * (1. - suck_down(&jet, height, nozzle)));
    s.throttle = throttle;
    s.lift_controls.vector_pitch = vertical;
    s.lift_controls.vector_pitch_actual = vertical;
    s.lift_controls.drive.engine_output =
        [military * throttle * lapse, lift_engines * throttle * lapse];
    s.lift_controls.drive.lift_engine_spool = f64::from(jet.lift_engines.is_some());
}

#[cfg(test)]
mod tests {
    //! The jet acceptance tests of the VTOL overhaul (design section 10,
    //! J1 to J13, slice P4's parts) on synthetic aircraft carrying the AV-8
    //! and Yak-141 PT numbers of design section 8 as plain constants. The
    //! speed envelopes are synthetic polygons with the PTs' stall and top
    //! speeds, and rows that, like the retail ones, are not a speed-squared
    //! family. The key parts of J3, J8, J9 and J12 are slice P6's; here the
    //! commands those keys send drive the physics.
    use super::super::super::{PilotCommand, PilotInput, Switch};
    use super::*;
    use crate::{models::AircraftModel, research::Surface};
    use tore_formats::aircraft::{Aircraft, AircraftId, Envelope, Token};
    use tore_input::{LiftCommand, NozzlePreset, StabilityLevel};

    const KT: f64 = KNOTS_TO_FPS;

    fn set(a: &mut Aircraft, key: &str, value: i64) {
        a.fields.insert(
            key.into(),
            Token {
                kind: "dword".into(),
                value: value.to_string(),
                scaled: false,
            },
        );
    }

    /// Speed envelope rows -3 to 7 G: the 1 G row from `stall` to `top`
    /// ft/s at sea level up to 50,000 ft; each higher row starts 45 percent
    /// of the stall speed further right (not a speed-squared family) and
    /// reaches less high.
    fn envelopes(stall: f64, top: f64) -> Vec<Envelope> {
        (-3..=7)
            .map(|g: i32| {
                let k = f64::from(g.abs().max(1) - 1);
                let slow = if g == 0 {
                    0.7 * stall
                } else {
                    stall * (1. + 0.45 * k)
                };
                let fast = top * (1. - 0.04 * k);
                let ceiling = 50_000. * (1. - k / 8.);
                Envelope {
                    g,
                    points: vec![
                        [slow, 0.],
                        [slow * 2.5, ceiling],
                        [fast * 0.75, ceiling],
                        [fast, 0.],
                    ],
                }
            })
            .collect()
    }

    /// An aircraft with `id`'s identity and its PT figures (design 8.2).
    fn fixture(id: AircraftId) -> Aircraft {
        let mut a = crate::models::variety::tests::synthetic(id);
        let (empty, fuel, thrust, afterburner, maximum, elevator, drag, pull, stall, top) = match id
        {
            AircraftId::Av8 => (13_968, 7_759, 33_800, 0, 31_000, 43, 45, 33, 170., 980.),
            AircraftId::Yak141 => (
                25_685, 9_700, 19_840, 34_170, 42_990, 30, 20, 51, 110., 1_140.,
            ),
            _ => unreachable!("vectoring jets only"),
        };
        for (key, value) in [
            ("weight", empty),
            ("internalFuel", fuel),
            ("thrust", thrust),
            ("aftThrust", afterburner),
            ("aftFuelConsumption", if afterburner > 0 { 17 } else { 0 }),
            ("maxTakeoffWeight", maximum),
            ("loadedElevator", elevator),
            ("loadedDrag", drag),
            ("_gpullDrag", pull),
        ] {
            set(&mut a, key, value);
        }
        for (axis, [max, acc, dacc]) in [
            ("_brv.x", [225, 286, 571]),
            ("puffRot.x", [50, 60, 20]),
            ("puffRot.y", [20, 20, 8]),
            ("puffRot.z", [20, 20, 8]),
        ] {
            for (suffix, value) in [("min", -max), ("max", max), ("acc", acc), ("dacc", dacc)] {
                set(&mut a, &format!("{axis}.{suffix}"), i64::from(value));
            }
        }
        a.envelopes = envelopes(stall, top);
        a
    }

    fn state(a: &Aircraft, position: [f64; 3]) -> State {
        let mut s = State::new(a, position).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s
    }

    /// Level flight at `altitude` and `speed` ft/s; `oracle` flies the same
    /// PT on the conventional law (the powered lift taken away).
    fn level(id: AircraftId, altitude: f64, speed: f64, oracle: bool) -> State {
        let mut model = AircraftModel::for_aircraft(&fixture(id)).unwrap();
        if oracle && let AircraftModel::Variety(m) = &mut model {
            m.lift = None;
        }
        let mut s = State::from_model(model, [0., altitude, 0.]);
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.speed = speed;
        s.velocity = [0., 0., speed];
        s
    }

    fn hover(id: AircraftId, height: f64) -> State {
        let mut s = state(&fixture(id), [0., height, 0.]);
        s.speed = 0.;
        s.velocity = [0.; 3];
        s.gear_down = true;
        s.gear = 1.;
        trim_hover(&mut s);
        s
    }

    /// Full power with the nose held level for `ticks`.
    fn hold_level(s: &mut State, throttle: f64, ticks: usize) {
        for _ in 0..ticks {
            let [pitch, roll] = attitude(s, 0.);
            step(
                s,
                &PilotInput {
                    pitch,
                    roll,
                    throttle: Some(throttle),
                    ..Default::default()
                },
            );
        }
    }

    fn step(s: &mut State, input: &PilotInput) {
        s.step_surface(input, |_, _| Surface::runway(0.));
    }

    fn run(s: &mut State, input: &PilotInput, ticks: usize) {
        for _ in 0..ticks {
            step(s, input);
        }
    }

    fn path_pitch(s: &State) -> f64 {
        s.velocity[1].atan2(s.velocity[0].hypot(s.velocity[2]))
    }

    /// A pilot's stick holding `pitch` and wings level.
    fn attitude(s: &State, pitch: f64) -> [f64; 2] {
        [
            (3. * (pitch - s.pitch) - 0.6 * s.pitch_rate).clamp(-1., 1.),
            (-3. * s.bank - 0.5 * s.roll_rate).clamp(-1., 1.),
        ]
    }

    fn lift(command: LiftCommand) -> PilotCommand {
        PilotCommand::Lift(command)
    }

    /// `s` at stability level `level` (slice P6; Damper is the default).
    fn at(mut s: State, level: StabilityLevel) -> State {
        s.command(lift(LiftCommand::SetStability(level)));
        s
    }

    #[test]
    fn j1_an_unloaded_av8_takes_off_vertically_and_climbs() {
        let a = fixture(AircraftId::Av8);
        let mut s = state(&a, [0., 0., 0.]);
        s.start_on_runway([0., 0., 0.], 0.).unwrap();
        s.command(PilotCommand::Set(Switch::Flaps, false));
        s.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        let mut lifted = None;
        for tick in 0..120 * 12 {
            let [pitch, roll] = attitude(&s, 0.);
            step(
                &mut s,
                &PilotInput {
                    pitch,
                    roll,
                    throttle: Some(1.),
                    ..Default::default()
                },
            );
            if lifted.is_none() && !s.weight_on_wheels() {
                lifted = Some(tick);
            }
        }
        assert!(
            !s.crashed && lifted.is_some_and(|tick| tick < 120 * 4),
            "{lifted:?}"
        );
        // 500 ft/min.
        assert!(s.vertical_speed > 500. / 60., "{}", s.vertical_speed);
        assert!(s.position[1] > 50., "{}", s.position[1]);
    }

    #[test]
    fn j1b_at_combat_weight_the_av8_cannot_hover() {
        let mut s = hover(AircraftId::Av8, 300.);
        s.set_payload(4_000.).unwrap();
        s.lift_controls.drive.engine_output[0] = 33_800.;
        hold_level(&mut s, 1., 120 * 10);
        assert!(
            s.position[1] < 300. && s.vertical_speed < -3.,
            "{} {}",
            s.position[1],
            s.vertical_speed
        );
        // Clean, the same full power climbs.
        let mut clean = hover(AircraftId::Av8, 300.);
        hold_level(&mut clean, 1., 120 * 10);
        assert!(clean.vertical_speed > 10., "{}", clean.vertical_speed);
    }

    /// Highest hover rates [pitch, roll, yaw], deg/s, in 2 s of full stick
    /// on each axis at `level`.
    fn hover_rates(id: AircraftId, level: StabilityLevel) -> [f64; 3] {
        std::array::from_fn(|axis| {
            let mut s = at(hover(id, 1_000.), level);
            let mut stick = [0.; 3];
            stick[axis] = 1.;
            let mut peak: f64 = 0.;
            for _ in 0..240 {
                let [pitch, roll, yaw] = stick;
                let throttle = s.throttle;
                step(
                    &mut s,
                    &PilotInput {
                        pitch,
                        roll,
                        yaw,
                        throttle: Some(throttle),
                        ..Default::default()
                    },
                );
                let [p, q, r] = s.lift_controls.body_rates;
                peak = peak.max([q, p, r][axis].to_degrees());
            }
            peak
        })
    }

    #[test]
    fn j2_hover_rates_come_from_the_puffers_and_only_with_the_nozzles_down() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let [pitch, roll, yaw] = hover_rates(id, StabilityLevel::Damper);
            assert!((40. ..=55.).contains(&roll), "{id:?} roll {roll}");
            assert!((15. ..=22.).contains(&pitch), "{id:?} pitch {pitch}");
            assert!((15. ..=22.).contains(&yaw), "{id:?} yaw {yaw}");
            // Off: the puffers keep accelerating the aircraft.
            let off = hover_rates(id, StabilityLevel::Off);
            assert!(
                off[0] > 22. && off[1] > 55. && off[2] > 22.,
                "{id:?} {off:?}"
            );
        }
        // Nozzles aft: no puffer authority, so at rest in the air a full
        // stick turns nothing.
        assert_eq!(puffer_authority(0., 30_000., 33_800.), 0.);
        assert_eq!(puffer_authority(10., 30_000., 33_800.), 0.);
        assert!((puffer_authority(20., 33_800., 33_800.) - 1.).abs() < 1e-12);
        let mut s = hover(AircraftId::Av8, 1_000.);
        s.lift_controls.vector_pitch = 0.;
        s.lift_controls.vector_pitch_actual = 0.;
        run(
            &mut s,
            &PilotInput {
                pitch: 1.,
                roll: 1.,
                yaw: 1.,
                ..Default::default()
            },
            12,
        );
        assert!(
            s.lift_controls.body_rates.iter().all(|r| r.abs() < 1e-3),
            "{:?}",
            s.lift_controls.body_rates
        );
    }

    #[test]
    fn j3_the_manual_transition_reaches_150_kt_holding_height() {
        let mut s = hover(AircraftId::Av8, 500.);
        let mut lowest = s.position[1];
        let mut forward = false;
        let mut reached = None;
        for tick in 0..120 * 25 {
            let mut commands = Vec::new();
            if tick == 0 {
                commands = vec![lift(LiftCommand::NozzleStep { down: false }); 3];
            }
            if !forward && s.speed >= 90. * KT {
                forward = true;
                commands.push(lift(LiftCommand::NozzlePreset(NozzlePreset::Forward)));
            }
            let target =
                (0.002 * (500. - s.position[1]) - 0.004 * s.vertical_speed).clamp(-0.2, 0.3);
            let [pitch, roll] = attitude(&s, target);
            step(
                &mut s,
                &PilotInput {
                    pitch,
                    roll,
                    throttle: Some(1.),
                    commands,
                    ..Default::default()
                },
            );
            lowest = lowest.min(s.position[1]);
            if reached.is_none() && s.speed >= 150. * KT {
                reached = Some(tick as f64 * DT);
            }
        }
        assert!(reached.is_some_and(|t| t < 25.), "{reached:?}");
        assert!(500. - lowest < 100., "{lowest}");
        assert!(!s.crashed);
    }

    fn peak_g(id: AircraftId, speed: f64, oracle: bool) -> (f64, f64) {
        let mut s = level(id, 10_000., speed * KT, oracle);
        let mut peak: f64 = 0.;
        for _ in 0..360 {
            step(
                &mut s,
                &PilotInput {
                    pitch: 1.,
                    throttle: Some(1.),
                    ..Default::default()
                },
            );
            peak = peak.max(s.g);
        }
        // How fast the pull turns the flight path, deg/s.
        let rate = path_pitch(&s).to_degrees() / 3.;
        (peak, rate)
    }

    /// Settled speed (ft/s) and turn rate (deg/s) in a full-power level turn
    /// at `bank` degrees, the stick holding the altitude.
    fn sustained(id: AircraftId, bank: f64, oracle: bool) -> (f64, f64) {
        let mut s = level(id, 10_000., 400. * KT, oracle);
        s.burner = true;
        let bank = bank.to_radians();
        let mut integral = 0.;
        let mut turned = 0.;
        for tick in 0..120 * 90 {
            let climb = -s.vertical_speed + 0.2 * (10_000. - s.position[1]);
            integral += climb * DT;
            let input = PilotInput {
                pitch: (0.01 * climb + 0.002 * integral).clamp(-1., 1.),
                roll: (3. * (bank - s.bank) - 0.5 * s.roll_rate).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            let before = s.yaw;
            step(&mut s, &input);
            if tick >= 120 * 80 {
                turned += ((s.yaw - before + std::f64::consts::PI)
                    .rem_euclid(std::f64::consts::TAU)
                    - std::f64::consts::PI)
                    .abs();
            }
        }
        assert!(!s.crashed);
        (s.speed, turned.to_degrees() / 10.)
    }

    /// Full-power level speed after 240 s at `altitude`, ft/s.
    fn top_speed(id: AircraftId, altitude: f64, oracle: bool) -> f64 {
        let mut s = level(id, altitude, 400. * KT, oracle);
        s.burner = true;
        for _ in 0..120 * 240 {
            let input = PilotInput {
                pitch: (0.002 * (altitude - s.position[1]) - 0.01 * s.vertical_speed)
                    .clamp(-0.2, 0.2),
                roll: (-3. * s.bank).clamp(-1., 1.),
                throttle: Some(1.),
                ..Default::default()
            };
            step(&mut s, &input);
        }
        s.speed
    }

    /// The speed at which, slowing at idle with the airbrake out and the
    /// stick holding the altitude, the aircraft starts to sink, ft/s.
    fn stall_speed(id: AircraftId, oracle: bool) -> f64 {
        let mut s = level(id, 10_000., 250. * KT, oracle);
        s.throttle = 0.;
        s.burner = false;
        s.brake_out = true;
        s.brake = 1.;
        for _ in 0..120 * 120 {
            let input = PilotInput {
                pitch: (-0.02 * s.vertical_speed + 0.004 * (10_000. - s.position[1]))
                    .clamp(-1., 1.),
                roll: (-3. * s.bank).clamp(-1., 1.),
                throttle: Some(0.),
                ..Default::default()
            };
            step(&mut s, &input);
            if s.vertical_speed < -15. || s.position[1] < 9_900. {
                break;
            }
        }
        s.speed
    }

    fn within(value: f64, oracle: f64, share: f64) -> bool {
        (value / oracle - 1.).abs() <= share
    }

    #[test]
    fn j4_wingborne_flight_matches_the_conventional_oracle_on_the_same_pt() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let pair = |f: &dyn Fn(bool) -> f64| (f(false), f(true));
            let (jet, oracle) = pair(&|o| stall_speed(id, o));
            assert!(within(jet, oracle, 0.1), "{id:?} stall {jet} {oracle}");
            for speed in [300., 450.] {
                let (jet, oracle) = (peak_g(id, speed, false), peak_g(id, speed, true));
                assert!(
                    within(jet.0, oracle.0, 0.1),
                    "{id:?} {speed} kt G {jet:?} {oracle:?}"
                );
            }
            for bank in [60., 70.] {
                let (jet, oracle) = (sustained(id, bank, false), sustained(id, bank, true));
                assert!(
                    within(jet.0, oracle.0, 0.1),
                    "{id:?} {bank} speed {jet:?} {oracle:?}"
                );
                assert!(
                    within(jet.1, oracle.1, 0.1),
                    "{id:?} {bank} turn {jet:?} {oracle:?}"
                );
            }
            let (jet, oracle) = pair(&|o| top_speed(id, 10_000., o));
            assert!(within(jet, oracle, 0.1), "{id:?} top {jet} {oracle}");
        }
    }

    /// Pushed from level at 350 kt to 60 degrees nose down, then hands off:
    /// [vertical speed, nose minus flight path] each second for 6 s.
    fn dive(id: AircraftId, oracle: bool) -> Vec<[f64; 2]> {
        let mut s = level(id, 15_000., 350. * KT, oracle);
        s.throttle = 0.8;
        while s.pitch > -60_f64.to_radians() {
            let [_, roll] = attitude(&s, 0.);
            step(
                &mut s,
                &PilotInput {
                    pitch: -1.,
                    roll,
                    ..Default::default()
                },
            );
            assert!(s.ticks < 120 * 15);
        }
        (0..6)
            .map(|_| {
                for _ in 0..120 {
                    let [_, roll] = attitude(&s, 0.);
                    step(
                        &mut s,
                        &PilotInput {
                            roll,
                            ..Default::default()
                        },
                    );
                }
                [s.vertical_speed, (s.pitch - path_pitch(&s)).to_degrees()]
            })
            .collect()
    }

    #[test]
    fn j5_the_jets_dive_like_jets() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let jet = dive(id, false);
            // Before slice P4 the AV-8 settled near -116 ft/s with its nose
            // 60 degrees down (design appendix A).
            assert!(jet[4][1].abs() < 5., "{id:?} {jet:?}");
            assert!(jet[5][0] < -450., "{id:?} {jet:?}");
            // The conventional law on the same PT dives the same way.
            let oracle = dive(id, true);
            assert!(
                within(jet[5][0], oracle[5][0], 0.2),
                "{id:?} {jet:?} {oracle:?}"
            );
        }
    }

    #[test]
    fn j6_the_jets_zoom_like_jets_and_trade_speed_for_height() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            for throttle in [1., 0.] {
                let mut s = level(id, 10_000., 350. * KT, false);
                s.burner = true;
                let energy = |s: &State| s.position[1] + s.speed * s.speed / (2. * GRAVITY);
                let start = energy(&s);
                let mut climb: f64 = 0.;
                for _ in 0..120 * 8 {
                    let [pitch, roll] = attitude(&s, 30_f64.to_radians());
                    step(
                        &mut s,
                        &PilotInput {
                            pitch,
                            roll,
                            throttle: Some(throttle),
                            ..Default::default()
                        },
                    );
                    climb = climb.max(s.vertical_speed);
                }
                if throttle == 1. {
                    assert!(climb > 250., "{id:?} {climb}");
                } else {
                    assert!(s.speed < 300. * KT, "{id:?} {}", s.speed / KT);
                    assert!(energy(&s) < start, "{id:?} energy");
                }
            }
        }
    }

    #[test]
    fn j7_wingborne_rates_match_the_pt_and_the_oracle() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let mut s = level(id, 10_000., 400. * KT, false);
            let mut peak: f64 = 0.;
            for _ in 0..240 {
                step(
                    &mut s,
                    &PilotInput {
                        roll: 1.,
                        throttle: Some(1.),
                        ..Default::default()
                    },
                );
                peak = peak.max(s.lift_controls.body_rates[0].to_degrees());
            }
            assert!(within(peak, 225., 0.1), "{id:?} roll {peak}");
            for speed in [300., 450.] {
                let (jet, oracle) = (peak_g(id, speed, false).1, peak_g(id, speed, true).1);
                assert!(
                    within(jet, oracle, 0.15),
                    "{id:?} {speed} pitch {jet} {oracle}"
                );
            }
        }
    }

    #[test]
    fn j8_the_braking_stop_decelerates_the_jet() {
        let mut s = level(AircraftId::Av8, 2_000., 200. * KT, false);
        s.throttle = 1.;
        s.lift_controls.drive.engine_output[0] = 33_000.;
        let mut start = s.speed;
        for tick in 0..240 {
            if tick == 120 {
                start = s.speed;
            }
            let commands = if tick == 0 {
                vec![lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)); 2]
            } else {
                Vec::new()
            };
            let [pitch, roll] = attitude(&s, 0.);
            step(
                &mut s,
                &PilotInput {
                    pitch,
                    roll,
                    throttle: Some(1.),
                    commands,
                    ..Default::default()
                },
            );
        }
        assert!((s.lift_controls.vector_pitch_actual - 1.).abs() < 1e-12);
        // Over the second after the nozzles reach the stop.
        let deceleration = (start - s.speed) / GRAVITY;
        assert!(deceleration > 0.3, "{deceleration}");
    }

    #[test]
    fn j9_a_short_takeoff_at_maximum_weight_gets_airborne() {
        let a = fixture(AircraftId::Av8);
        let mut s = state(&a, [0., 0., 0.]);
        s.start_on_runway([0., 0., 0.], 0.).unwrap();
        let payload = 31_000. - 13_968. - s.fuel;
        s.set_payload(payload).unwrap();
        s.command(PilotCommand::Set(Switch::Airbrake, false));
        let mut stepped = false;
        let mut airborne = None;
        for tick in 0..120 * 40 {
            let mut commands = Vec::new();
            if tick == 0 {
                commands.push(PilotCommand::Set(Switch::Airbrake, false));
            }
            if !stepped && s.speed >= 85. * KT {
                stepped = true;
                commands = vec![lift(LiftCommand::NozzleStep { down: true }); 4];
            }
            let pitch = if stepped {
                attitude(&s, 10_f64.to_radians())[0]
            } else {
                0.
            };
            s.brake_out = false;
            step(
                &mut s,
                &PilotInput {
                    pitch,
                    throttle: Some(1.),
                    commands,
                    ..Default::default()
                },
            );
            if airborne.is_none() && !s.weight_on_wheels() && s.position[1] > 20. {
                airborne = Some(tick);
            }
        }
        assert!(stepped && airborne.is_some() && !s.crashed, "{airborne:?}");
        assert!(s.position[1] > 100., "{}", s.position[1]);
    }

    #[test]
    fn j10_a_vertical_landing_touches_down_gently() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let mut s = hover(id, 100.);
            let hold = s.throttle;
            let mut touchdown = None;
            for _ in 0..120 * 60 {
                let throttle = (hold + 0.02 * (-4. - s.vertical_speed)).clamp(0., 1.);
                let [pitch, roll] = attitude(&s, 0.);
                step(
                    &mut s,
                    &PilotInput {
                        pitch,
                        roll,
                        throttle: Some(throttle),
                        ..Default::default()
                    },
                );
                if s.weight_on_wheels() {
                    touchdown = Some(s.trace().contact);
                    break;
                }
            }
            assert!(touchdown.is_some() && !s.crashed, "{id:?} {touchdown:?}");
        }
    }

    /// Bank after 3 s hands off, jetborne at 40 kt with 10 degrees of
    /// sideslip, at the Damper level or Off.
    /// J11's Damper half needs a tighter rate loop than slice P6's puffer
    /// damping alone gives (17.9 degrees of bank with it, against the
    /// design's 10): a 20-percent-authority loop that saturates at 5
    /// degrees per second, flown here on the stick on top of the Damper
    /// level. See the P4 notes in the design; P6's `sas.rs` should take it.
    const TIGHT: bool = true;
    fn roll_off(damped: bool, hazards: bool) -> f64 {
        let level = if damped {
            StabilityLevel::Damper
        } else {
            StabilityLevel::Off
        };
        let mut s = at(hover(AircraftId::Av8, 1_000.), level);
        let speed = 40. * KT;
        let slip = 10_f64.to_radians();
        // Heading straight ahead, sliding right.
        s.velocity = [speed * slip.sin(), 0., speed * slip.cos()];
        s.speed = speed;
        let throttle = s.throttle;
        if !hazards {
            // The Easy flight physics cheat's switch (slice P8): checked
            // here by the parts it removes.
            let air = AirData::new(&Basis::new(0., 0., 0.), s.velocity, 1_000.);
            let jet = s.model().powered_lift().unwrap().jet.unwrap();
            let (_, moments) = intake(&jet, &air, &Basis::new(0., 0., 0.), 30_000., 1.5, false);
            return moments[0].abs() + moments[2].abs();
        }
        for _ in 0..360 {
            let [p, _, r] = s.lift_controls.body_rates;
            let tight = |rate: f64| (-rate / 5_f64.to_radians()).clamp(-0.2, 0.2);
            let (roll, yaw) = if damped && TIGHT {
                (tight(p), tight(r))
            } else {
                (0., 0.)
            };
            step(
                &mut s,
                &PilotInput {
                    roll,
                    yaw,
                    throttle: Some(throttle),
                    ..Default::default()
                },
            );
        }
        s.bank.to_degrees().abs()
    }

    #[test]
    fn j11_sideslip_at_low_speed_rolls_the_jet_off() {
        let off = roll_off(false, true);
        assert!(off > 30., "Off {off}");
        let damped = roll_off(true, true);
        assert!(damped < 10., "Damper {damped}");
        assert_eq!(roll_off(false, false), 0.);
    }

    #[test]
    fn j12_the_nozzles_travel_at_the_pt_rate_to_the_braking_stop() {
        let mut s = level(AircraftId::Av8, 5_000., 300. * KT, false);
        s.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        run(&mut s, &PilotInput::default(), 108);
        assert!((s.lift_controls.vector_pitch_actual - 0.9).abs() < 1e-9);
        s.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        run(&mut s, &PilotInput::default(), 12);
        assert!((s.lift_controls.vector_pitch_actual - 1.).abs() < 1e-9);
        // No vector yaw on either jet.
        assert!(!s.flight_axis_available(super::super::super::FlightAxis::VectorYaw));
        s.command(PilotCommand::SetAxis(
            super::super::super::FlightAxis::VectorYaw,
            1.,
        ));
        run(
            &mut s,
            &PilotInput {
                vector_yaw_rate: 1.,
                ..Default::default()
            },
            60,
        );
        assert_eq!(s.lift_controls.vector_yaw_actual, 0.);
    }

    #[test]
    fn j13_a_clean_yak_just_hovers_and_any_store_takes_the_hover_away() {
        let a = fixture(AircraftId::Yak141);
        let s = state(&a, [0., 0., 0.]);
        let c = s.model().configuration();
        let jet = s.model().powered_lift().unwrap().jet.unwrap();
        let weight = c.mass.empty_lbs + s.fuel;
        let lift_engines = jet.lift_engines.unwrap().thrust_lbf;
        let vertical = c.propulsion.military_thrust_lbf * jet.vertical_efficiency + lift_engines;
        let margin = vertical / weight - 1.;
        assert!((0.02..=0.06).contains(&margin), "{margin}");
        assert!(vertical < weight + 1_000.);
        // Flown: full dry power out of ground effect climbs clean and sinks
        // with a 1,000 lb store.
        for (stores, climbs) in [(0., true), (1_000., false)] {
            let mut s = hover(AircraftId::Yak141, 300.);
            s.set_payload(stores).unwrap();
            hold_level(&mut s, 1., 120 * 8);
            assert_eq!(
                s.vertical_speed > 0.,
                climbs,
                "{stores} {}",
                s.vertical_speed
            );
        }
        // The afterburner stays dark with the nozzles down.
        let mut s = hover(AircraftId::Yak141, 3_000.);
        s.command(PilotCommand::Set(Switch::Burner, true));
        run(
            &mut s,
            &PilotInput {
                throttle: Some(1.),
                ..Default::default()
            },
            12,
        );
        assert!(!s.afterburner_active());
    }

    #[test]
    fn the_yak_lift_engines_run_for_takeoff_and_landing_only() {
        let mut s = hover(AircraftId::Yak141, 3_000.);
        assert_eq!(s.lift_controls.drive.lift_engine_spool, 1.);
        let fuel = s.fuel;
        s.cheats.unlimited_fuel = false;
        run(&mut s, &PilotInput::default(), 120);
        assert!(s.fuel < fuel);
        // Nozzles aft: below 20 degrees they spool down over 2 s.
        s.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Forward)));
        run(&mut s, &PilotInput::default(), 120 * 3);
        assert_eq!(s.lift_controls.drive.lift_engine_spool, 0.);
        run(&mut s, &PilotInput::default(), 120 * 3);
        assert!(s.lift_controls.drive.engine_output[1] < 100.);
        // Fast, they do not start whatever the nozzles do.
        let mut fast = level(AircraftId::Yak141, 3_000., 250. * KT, false);
        fast.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        run(&mut fast, &PilotInput::default(), 120);
        assert_eq!(fast.lift_controls.drive.lift_engine_spool, 0.);
        // The AV-8 has none.
        assert_eq!(
            hover(AircraftId::Av8, 300.)
                .lift_controls
                .drive
                .lift_engine_spool,
            0.
        );
    }

    #[test]
    fn suck_down_takes_lift_near_the_ground_only() {
        let jet = AircraftModel::for_aircraft(&fixture(AircraftId::Av8))
            .unwrap()
            .powered_lift()
            .unwrap()
            .jet
            .unwrap();
        let vertical = std::f64::consts::FRAC_PI_2;
        assert!((suck_down(&jet, 0., vertical) - 0.06).abs() < 1e-12);
        assert!(suck_down(&jet, jet.span_ft / 2., vertical) > 0.02);
        assert_eq!(suck_down(&jet, jet.span_ft, vertical), 0.);
        assert_eq!(suck_down(&jet, 0., 0.), 0.);
        assert!((nozzle_efficiency(&jet, 0.) - 1.).abs() < 1e-12);
        assert!((nozzle_efficiency(&jet, vertical) - jet.vertical_efficiency).abs() < 1e-12);
        assert_eq!(nozzle_efficiency(&jet, 1.74), jet.vertical_efficiency);
    }

    #[test]
    fn sideways_stick_on_the_wheels_at_liftoff_power_rolls_the_jet_over() {
        let a = fixture(AircraftId::Av8);
        // Heavy on its wheels at idle, full roll does nothing.
        let mut parked = state(&a, [0., 0., 0.]);
        parked.start_on_runway([0., 0., 0.], 0.).unwrap();
        parked.command(lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        run(
            &mut parked,
            &PilotInput {
                roll: 1.,
                ..Default::default()
            },
            240,
        );
        assert!(!parked.crashed && parked.bank == 0. && parked.weight_on_wheels());
        // Light on its wheels with the thrust nearly carrying it, full roll
        // tips it past 15 degrees: the manual's warning.
        let mut light = hover(AircraftId::Av8, 0.);
        light.position[1] = light.model().configuration().equipment.ground_clearance_ft + 0.001;
        light.velocity = [0., -1., 0.];
        run(&mut light, &PilotInput::default(), 1);
        assert!(light.weight_on_wheels());
        light.throttle *= 1.06;
        let throttle = light.throttle;
        run(
            &mut light,
            &PilotInput {
                roll: 1.,
                throttle: Some(throttle),
                ..Default::default()
            },
            240,
        );
        assert!(light.crashed, "bank {}", light.bank.to_degrees());
    }

    #[test]
    fn a_snapshot_mid_transition_continues_bit_for_bit() {
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let mut s = hover(id, 800.);
            let input = |tick: usize| PilotInput {
                pitch: (tick as f64 / 90.).sin() * 0.3,
                roll: (tick as f64 / 70.).cos() * 0.2,
                yaw: 0.1,
                throttle: Some(1.),
                commands: if tick == 0 {
                    vec![lift(LiftCommand::NozzleStep { down: false }); 4]
                } else {
                    Vec::new()
                },
                ..Default::default()
            };
            for tick in 0..240 {
                step(&mut s, &input(tick));
            }
            // Mid-transition: nozzles partly down, both the wing and the
            // puffers working, the lift engines (Yak) still running.
            assert!(
                s.lift_controls.vector_pitch_actual > 0.4 && s.speed > 10.,
                "{id:?} {} {}",
                s.lift_controls.vector_pitch_actual,
                s.speed
            );
            let model = AircraftModel::for_aircraft(&fixture(id)).unwrap();
            let mut writer = tore_codec::BitWriter::new();
            s.write_exact(&mut writer, None).unwrap();
            let bytes = writer.as_bytes();
            let mut restored =
                State::read_exact(&mut tore_codec::BitReader::new(bytes), None, &model).unwrap();
            assert_eq!(s, restored, "{id:?}");
            for tick in 240..1_440 {
                step(&mut s, &input(tick));
                step(&mut restored, &input(tick));
                assert_eq!(s, restored, "{id:?} tick {tick}");
            }
        }
    }

    #[test]
    fn hazard_switches_remove_the_roll_off_moments_only() {
        let jet = AircraftModel::for_aircraft(&fixture(AircraftId::Av8))
            .unwrap()
            .powered_lift()
            .unwrap()
            .jet
            .unwrap();
        let basis = Basis::new(0., 0., 0.);
        let air = AirData::new(&basis, [10., -5., 60.], 0.);
        let (force, on) = intake(&jet, &air, &basis, 30_000., 1.4, true);
        let (same, off) = intake(&jet, &air, &basis, 30_000., 1.4, false);
        assert_eq!(force, same);
        assert_eq!(on[1], off[1]);
        // Sliding right: the intakes yaw the nose left, away from the air,
        // and the jet-induced dihedral rolls left.
        assert!(on[2] < 0. && on[0] < 0.);
        assert_eq!((off[0], off[2]), (0., 0.));
        assert_eq!(
            Hazards::default(),
            Hazards {
                roll_off: true,
                undamped_puffers: true,
                dynamic_rollover: true,
            }
        );
    }
}
