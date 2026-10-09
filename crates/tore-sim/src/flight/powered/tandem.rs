//! The tandem-rotor CH-47 (VTOL overhaul design 4.6, slice P3): two
//! counter-rotating [`super::rotor`] rotors fore and aft of the centre of
//! gravity on one cross-shafted [`super::drive`], flown on the
//! [`super::body`] rigid body with the helicopters' [`super::fuselage`]. No
//! tail rotor, no attitude limits and no velocity damping.
//!
//! - **Pitch** is differential collective: aft stick adds blade pitch to the
//!   front rotor and takes it from the rear, a moment of `dT * l`, `l` half the
//!   hub spacing. **Roll** tilts both disks the same way. **Yaw** tilts them
//!   in opposite directions (pedals right: front right, rear left), a moment
//!   `2 T sin(delta) l`. The collective is common.
//! - **Torque** cancels: the two rotors turn opposite ways, so the airframe
//!   feels only the drive torque times the share of power the front rotor
//!   takes over the rear (`P_front - P_rear`), which is where the rear
//!   rotor's interference leaves a small residual yaw.
//! - **Interference**: the rear rotor flies in the front rotor's wake. Its
//!   air velocity along the front disk normal gains `k vi_front`, `k` fading
//!   from its hover value to nothing by 40 kt (design 4.6), so the rear rotor
//!   needs more collective and more power in a hover and the pilot's
//!   differential collective trims it.
//! - **Rate damping** comes from the rotors themselves: each hub moves with
//!   the body's rotation (`omega x r`), so pitch rate feeds a heave change on
//!   each rotor with the lever arm `l`, which is why a tandem's pitch is
//!   deliberate and well damped.
//! - **Longitudinal trim**: the stability law's CH-47 schedule
//!   ([`super::sas::Augmented::longitudinal_trim`], 0 at 40 kt to 1 at 140 kt,
//!   Damper and Attitude only) tilts both disks forward by up to
//!   `trim_tilt_degrees` of the layout, which keeps the fuselage level as
//!   speed rises.
//! - **Engines and governor** are [`super::drive`]'s with the two rotors'
//!   power summed on the one rotor speed.
//!
//! Every fitted constant is an agent decision of 2026-10-09, tuned to the
//! design's section 10 acceptance numbers for the CH-47.

use super::{
    DT, State, airframe,
    body::{GRAVITY, Inertia, Moments, body_axes},
    drive::{self, DriveModel, DriveStep},
    fuselage::{self, AirframeInput, AirframeLoads},
    helicopter::solve,
    rotor::{self, DiskFrame, Hazards, RotorInput, RotorModel, RotorOutput},
    sas,
    state::{PilotAids, Rotor},
    trace,
};
use crate::{
    attitude::{Basis, Vector, cross, dot},
    models::{
        FlightModel,
        config::Configuration,
        variety::{PoweredLift, RotorLayout, RotorcraftAirframe},
    },
};

/// The rear rotor's interference fades out by this airspeed, ft/s (design
/// 4.6: 40 kt).
const INTERFERENCE_FADE_FPS: f64 = 40. * 1.687_81;
/// Collective lever travel per second (as the single-rotor helicopters').
const COLLECTIVE_RATE: f64 = 2.;
/// Dynamic rollover: bank on the wheels beyond this with the thrust leaning
/// the same way, carrying more than this share of the weight (fitted, as
/// the single-rotor helicopters').
const ROLLOVER_BANK_DEGREES: f64 = 15.;
const ROLLOVER_THRUST_SHARE: f64 = 0.1;
/// The wheels hold yaw while they carry more than this share of the weight.
const WHEEL_YAW_HOLD: f64 = 0.05;
/// The fin and rear pylon sit this far behind the rear hub, ft (fitted).
const PYLON_BEHIND_REAR_HUB_FT: f64 = 10.;
/// Trim: Newton iterations, inner equilibrium passes and the finite
/// difference step.
const TRIM_ITERATIONS: usize = 16;
const TRIM_EQUILIBRIUM_PASSES: usize = 80;
const TRIM_STEP: f64 = 1e-6;
/// Fast trims are found in speed steps this big, ft/s (about 20 kt).
const TRIM_CONTINUATION_FPS: f64 = 20. * 1.687_81;

/// The CH-47's model, derived from its parameters and its configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tandem {
    pub lift: PoweredLift,
    /// The front and rear rotors: the same, turning opposite ways (the front
    /// counter-clockwise seen from above).
    pub rotors: [RotorModel; 2],
    /// Each hub's distance from the centre of gravity along the fuselage,
    /// ft (half the hub spacing).
    pub arm_ft: f64,
    /// The rear rotor's hover inflow from the front rotor's wake, as a share
    /// of the front rotor's induced velocity.
    pub interference: f64,
    /// Lever travel added to one rotor and taken from the other by a full
    /// pitch stick.
    pub pitch_lever: f64,
    /// Share of the lateral cyclic range a full pedal gives each rotor (in
    /// opposite directions).
    pub pedal_cyclic: f64,
    /// Share of the longitudinal cyclic range the full trim schedule gives
    /// both rotors, forward.
    pub trim_cyclic: f64,
    pub airframe: RotorcraftAirframe,
    /// Empty plus full internal fuel, lb.
    pub reference_weight: f64,
    /// Maximum static thrust of both rotors at sea level, lbf.
    pub max_thrust: f64,
    pub drive: DriveModel,
}

/// One instant of the CH-47, enough to find its loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instant {
    pub basis: Basis,
    /// Air velocity of the centre of gravity, world axes, ft/s.
    pub air_velocity: Vector,
    /// Body rates [roll, pitch, yaw], rad/s.
    pub body_rates: [f64; 3],
    pub density: f64,
    pub rotor_speed: f64,
    /// Front and rear rotor states.
    pub rotors: [Rotor; 2],
    /// Shaft power the engines deliver, ft·lbf/s.
    pub engine_power: f64,
    /// Collective lever 0..1, and stick and pedal [pitch (aft positive),
    /// roll (right), pedal (nose right)].
    pub collective: f64,
    pub controls: [f64; 3],
    /// The scheduled longitudinal cyclic trim, 0..1.
    pub longitudinal_trim: f64,
    /// Hub heights above the surface, front and rear, ft.
    pub hub_height_agl_ft: [Option<f64>; 2],
    pub seconds: f64,
    pub hazards: Hazards,
    /// Fuselage drag growth (stores, damage) and lift factor.
    pub drag_factor: f64,
    pub lift_factor: f64,
}

/// Everything acting on the CH-47 at one instant, except gravity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loads {
    /// World axes, lbf.
    pub force: Vector,
    pub moments: Moments,
    pub rotors: [RotorOutput; 2],
    pub airframe: AirframeLoads,
    /// Both rotors' power, ft·lbf/s.
    pub power: f64,
    /// Both rotors' thrust, lbf.
    pub thrust_lbf: f64,
    /// The drive torque's residual yaw moment on the airframe, ft·lbf.
    pub torque_yaw: f64,
}

impl Tandem {
    /// The model of a tandem-rotor helicopter, or none for any other
    /// aircraft.
    pub fn new(lift: &PoweredLift, c: &Configuration) -> Option<Self> {
        let p = lift.rotor?;
        let RotorLayout::Tandem {
            hub_spacing_ft,
            interference,
            pitch_collective_degrees,
            pedal_cyclic_share,
            trim_tilt_degrees,
        } = p.layout
        else {
            return None;
        };
        let reference_weight = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
        // Each rotor carries half the aircraft.
        let front = RotorModel::new(&p, 1., reference_weight / 2.);
        let rear = RotorModel::new(&p, -1., reference_weight / 2.);
        let rho0 = rotor::sea_level_density();
        let range = front.blade_pitch(1.) - front.blade_pitch(0.);
        let hover = 2. * front.hover_power(reference_weight / 2., rho0);
        let rated = p.rated_power_hp.map_or_else(
            || 2. * front.hover_power(p.max_thrust_lbf / 2., rho0),
            |hp| hp * 550.,
        );
        let cyclic = front.cyclic_range();
        Some(Self {
            lift: *lift,
            rotors: [front, rear],
            arm_ft: hub_spacing_ft / 2.,
            interference,
            pitch_lever: pitch_collective_degrees.to_radians() / range,
            pedal_cyclic: pedal_cyclic_share,
            trim_cyclic: trim_tilt_degrees.to_radians() / cyclic[0],
            airframe: p.airframe,
            reference_weight,
            max_thrust: p.max_thrust_lbf,
            drive: DriveModel::new(rated, hover, p.energy_seconds),
        })
    }

    /// The interference gain at an airspeed, ft/s.
    fn wake_gain(&self, airspeed: f64) -> f64 {
        self.interference * (1. - rotor::smoothstep(airspeed / INTERFERENCE_FADE_FPS))
    }

    /// The rotors' input at `i`, with the front rotor's wake in the rear
    /// rotor's air when `wake` is its normal scaled by the gain times the
    /// front induced velocity.
    fn rotor_input(&self, i: &Instant, which: usize, hub_velocity: Vector) -> RotorInput {
        let sign = if which == 0 { 1. } else { -1. };
        let lever = |delta: f64| (i.collective + delta).clamp(0., 1.);
        // Aft stick: more front blade pitch, less rear.
        let collective = lever(sign * i.controls[0] * self.pitch_lever);
        let lateral = (i.controls[1] + sign * i.controls[2] * self.pedal_cyclic).clamp(-1., 1.);
        RotorInput {
            frame: DiskFrame {
                shaft: i.basis.up,
                forward: i.basis.forward,
                right: i.basis.right,
            },
            air_velocity: hub_velocity,
            density: i.density,
            rotor_speed: i.rotor_speed,
            collective,
            cyclic: [self.trim_cyclic * i.longitudinal_trim, lateral],
            hub_height_agl_ft: i.hub_height_agl_ft[which],
            seconds: i.seconds,
            hazards: i.hazards,
        }
    }

    /// Where each hub is from the centre of gravity, world axes, ft.
    fn hubs(&self, basis: &Basis) -> [Vector; 2] {
        let h = self.rotors[0].hub_height_ft;
        let at = |along: f64| -> Vector {
            std::array::from_fn(|k| along * basis.forward[k] + h * basis.up[k])
        };
        [at(self.arm_ft), at(-self.arm_ft)]
    }

    /// Everything acting on the CH-47 at `i`, except gravity.
    pub fn loads(&self, i: &Instant) -> Loads {
        let basis = i.basis;
        let hubs = self.hubs(&basis);
        let axes = body_axes(&basis);
        let omega: Vector =
            std::array::from_fn(|k| (0..3).map(|axis| axes[axis][k] * i.body_rates[axis]).sum());
        let hub_velocity = |r: Vector| -> Vector {
            let spin = cross(omega, r);
            std::array::from_fn(|k| i.air_velocity[k] + spin[k])
        };
        let airspeed = dot(i.air_velocity, i.air_velocity).sqrt();
        let front_in = self.rotor_input(i, 0, hub_velocity(hubs[0]));
        let front = self.rotors[0].evaluate(&i.rotors[0], &front_in);
        // The rear rotor flies in the front rotor's wake: relative to the air
        // it sits in, it climbs at k vi along the front disk's normal.
        let wake = self.wake_gain(airspeed) * i.rotors[0].induced_fps.max(0.);
        let rear_velocity: Vector =
            std::array::from_fn(|k| hub_velocity(hubs[1])[k] + wake * front.normal[k]);
        let rear_in = self.rotor_input(i, 1, rear_velocity);
        let rear = self.rotors[1].evaluate(&i.rotors[1], &rear_in);
        let outputs = [front, rear];
        let mut force: Vector = [0.; 3];
        let mut rotor_moment: Vector = [0.; 3];
        let mut thrust_lbf = 0.;
        for (out, hub) in outputs.iter().zip(hubs) {
            let thrust: Vector = out.normal.map(|n| n * out.thrust_lbf);
            let lever = cross(hub, thrust);
            for k in 0..3 {
                force[k] += thrust[k];
                rotor_moment[k] += lever[k] + out.hub_moment[k];
            }
            thrust_lbf += out.thrust_lbf;
        }
        let power = front.power + rear.power;
        // The drive torque reacts on the airframe through each rotor in
        // proportion to the power it takes; the rotors turn opposite ways, so
        // the two cancel but for the difference.
        let torque_yaw = if i.hazards.torque && power > 1. {
            let torque = i.engine_power / (i.rotor_speed.max(0.05) * self.rotors[0].omega);
            torque * (self.rotors[0].turn * front.power + self.rotors[1].turn * rear.power) / power
        } else {
            0.
        };
        let airframe = fuselage::airframe_loads(
            &self.airframe,
            &AirframeInput {
                basis,
                air_velocity: i.air_velocity,
                density: i.density,
                tail_arm_ft: self.arm_ft + PYLON_BEHIND_REAR_HUB_FT,
                drag_factor: i.drag_factor,
                turn: 0.,
                reference_torque: 0.,
                lift_factor: i.lift_factor,
            },
        );
        let mut applied = fuselage::body_moments(&basis, rotor_moment);
        applied[2] += torque_yaw;
        for (a, b) in applied.iter_mut().zip(airframe.moments) {
            *a += b;
        }
        // The disks lag the body by the rate lag; each radian of lag is the
        // hub stiffness plus the thrust's lever arm. Rotor heave does the
        // rest of the pitch damping through the hubs' own velocity.
        let h = self.rotors[0].hub_height_ft;
        let rotor_damping: f64 = outputs
            .iter()
            .map(|o| (o.tilt_stiffness + o.thrust_lbf.max(0.) * h.abs()) * o.rate_lag_seconds)
            .sum();
        let damping = [
            rotor_damping + airframe.damping[0],
            rotor_damping + airframe.damping[1],
            airframe.damping[2],
        ];
        for (f, a) in force.iter_mut().zip(airframe.force) {
            *f += a;
        }
        Loads {
            force,
            moments: Moments { applied, damping },
            rotors: outputs,
            airframe,
            power,
            thrust_lbf,
            torque_yaw,
        }
    }

    /// What the stability law senses at `i`: airspeed, sideslip and the pedal
    /// that would cancel the residual drive torque.
    pub fn sensed(&self, i: &Instant, loads: &Loads) -> sas::Sensed {
        let airspeed_fps = dot(i.air_velocity, i.air_velocity).sqrt();
        let sideslip_rad = if airspeed_fps > 1e-9 {
            (dot(i.air_velocity, i.basis.right) / airspeed_fps)
                .clamp(-1., 1.)
                .asin()
        } else {
            0.
        };
        // Yaw moment of a full pedal: both rotors' side thrusts at their arms.
        let lateral = self.rotors[0].cyclic_range()[1];
        let per_pedal =
            loads.thrust_lbf.max(0.) * (self.pedal_cyclic * lateral).sin() * self.arm_ft;
        let torque_pedal = if i.hazards.torque && per_pedal > 1. {
            // Positive pedal yaws the nose right.
            (-loads.torque_yaw / per_pedal).clamp(-1., 1.)
        } else {
            0.
        };
        sas::Sensed {
            airspeed_fps,
            sideslip_rad,
            torque_pedal,
        }
    }

    /// Engine power the helicopter can have now, ft·lbf/s.
    pub fn available_power(&self, density: f64, damage: f64, throttle: f64) -> f64 {
        self.drive.available_power(density, damage, throttle)
    }

    /// The trimmed state of flight at `airspeed_fps` along `heading`, level,
    /// at `density`, out of ground effect, with the stability law at `level`
    /// in the loop: [collective, pitch stick, roll stick, pedal, pitch,
    /// bank], or none when the solution needs more than full travel of any
    /// control. Fast trims are found by continuation from a hover in steps of
    /// 20 kt, each started from the last, so the Newton iteration stays on
    /// the branch it knows.
    pub fn trim(
        &self,
        weight: f64,
        heading: f64,
        airspeed_fps: f64,
        density: f64,
        drag_factor: f64,
        level: tore_input::StabilityLevel,
    ) -> Option<Trim> {
        self.trim_with(
            Hazards::ALL,
            weight,
            heading,
            airspeed_fps,
            density,
            drag_factor,
            level,
        )
    }

    /// [`Tandem::trim`] under the given `hazards` (the Easy flight physics
    /// cheat's none leaves no torque residual to trim out).
    #[allow(clippy::too_many_arguments)] // The trim's inputs, plus the hazards in force.
    pub fn trim_with(
        &self,
        hazards: Hazards,
        weight: f64,
        heading: f64,
        airspeed_fps: f64,
        density: f64,
        drag_factor: f64,
        level: tore_input::StabilityLevel,
    ) -> Option<Trim> {
        let steps = (airspeed_fps / TRIM_CONTINUATION_FPS).ceil().max(1.) as usize;
        let mut guess = None;
        let mut result = None;
        for step in 1..=steps {
            let speed = airspeed_fps * step as f64 / steps as f64;
            result = self.trim_from(
                hazards,
                weight,
                heading,
                speed,
                density,
                drag_factor,
                level,
                guess,
            );
            guess = Some(result?);
        }
        result
    }

    /// The steady state of the rotors, engine power and stability law with
    /// the unknowns `x` ([collective, pitch stick, roll stick, pedal, pitch,
    /// bank]) held, started from the rotor states `start`, and its loads.
    fn equilibrium(&self, c: &TrimConditions, x: &[f64; 6], start: [Rotor; 2]) -> (Instant, Loads) {
        let basis = Basis::new(c.heading, x[4], x[5]);
        let mut instant = Instant {
            basis,
            air_velocity: Basis::new(c.heading, 0., 0.)
                .forward
                .map(|f| f * c.airspeed_fps),
            body_rates: [0.; 3],
            density: c.density,
            rotor_speed: 1.,
            rotors: start,
            engine_power: 0.,
            collective: x[0],
            controls: [x[1], x[2], x[3]],
            longitudinal_trim: 0.,
            hub_height_agl_ft: [None; 2],
            seconds: 0.,
            hazards: c.hazards,
            drag_factor: c.drag_factor,
            lift_factor: 1.,
        };
        let pilot = [x[1], x[2], x[3]];
        let mut aids = PilotAids {
            stability: c.level,
            trim: pilot,
            attitude_reference: [x[4], x[5], c.heading],
            ..Default::default()
        };
        let body = sas::Body {
            attitude: [x[4], x[5], c.heading],
            rates: [0.; 3],
        };
        let mut loads = self.loads(&instant);
        for _ in 0..TRIM_EQUILIBRIUM_PASSES {
            // The stability law first, so the rotors never see a pass with the
            // pilot's controls alone (an instant of no longitudinal trim
            // would tilt the disks aft and stall them at speed).
            instant.engine_power = loads.power;
            instant.controls = pilot;
            let sensed = self.sensed(&instant, &loads);
            let augmented = sas::augment(&self.lift, c.level, &mut aids, pilot, body, sensed);
            instant.controls = augmented.controls;
            instant.longitudinal_trim = augmented.longitudinal_trim;
            loads = self.loads(&instant);
            for (state, out) in instant.rotors.iter_mut().zip(&loads.rotors) {
                state.induced_fps += 0.5 * (out.induced_target_fps - state.induced_fps);
                state.tilt = out.tilt_target;
            }
        }
        // One last look at the settled state.
        loads = self.loads(&instant);
        (instant, loads)
    }

    /// One Newton solve of [`Self::trim`], from the `guess` trim if given.
    #[allow(clippy::too_many_arguments)] // The trim's own conditions.
    fn trim_from(
        &self,
        hazards: Hazards,
        weight: f64,
        heading: f64,
        airspeed_fps: f64,
        density: f64,
        drag_factor: f64,
        level: tore_input::StabilityLevel,
        guess: Option<Trim>,
    ) -> Option<Trim> {
        let rotor = &self.rotors[0];
        let induced = (weight / 2. / (2. * density * rotor.area_ft2)).sqrt();
        let pitch_guess = rotor.blade_pitch(0.)
            + 3. * (weight
                / 2.
                / (density * rotor.area_ft2 * rotor.tip_speed_fps.powi(2))
                / (rotor.solidity * rotor::LIFT_SLOPE / 2.)
                + induced / rotor.tip_speed_fps / 2.);
        let mut x = guess.map_or(
            [rotor.lever_for_pitch(pitch_guess), 0., 0., 0., 0., 0.],
            |g| {
                [
                    g.collective,
                    g.controls[0],
                    g.controls[1],
                    g.controls[2],
                    g.pitch,
                    g.bank,
                ]
            },
        );
        // The equilibrium is found from the guess's rotor states (or the
        // hover's), so a continuation stays on the branch it is on.
        let start = guess.map_or(
            [Rotor {
                induced_fps: induced,
                tilt: [0.; 2],
            }; 2],
            |g| g.rotors,
        );
        let conditions = TrimConditions {
            hazards,
            heading,
            airspeed_fps,
            density,
            drag_factor,
            level,
        };
        let evaluate = |x: &[f64; 6]| self.equilibrium(&conditions, x, start);
        let residual = |loads: &Loads| -> [f64; 6] {
            let f = loads.force;
            let m = loads.moments.applied;
            let scale = weight * 10.;
            [
                f[0] / weight,
                f[1] / weight - 1.,
                f[2] / weight,
                m[0] / scale,
                m[1] / scale,
                m[2] / scale,
            ]
        };
        for _ in 0..TRIM_ITERATIONS {
            let (_, loads) = evaluate(&x);
            let r = residual(&loads);
            let mut jacobian = [[0.; 6]; 6];
            for j in 0..6 {
                let mut stepped = x;
                stepped[j] += TRIM_STEP;
                let (_, loads) = evaluate(&stepped);
                let rs = residual(&loads);
                for i in 0..6 {
                    jacobian[i][j] = (rs[i] - r[i]) / TRIM_STEP;
                }
            }
            let step = solve(jacobian, r.map(|v| -v))?;
            for (value, delta) in x.iter_mut().zip(step) {
                *value += delta.clamp(-0.5, 0.5);
            }
        }
        let (instant, loads) = evaluate(&x);
        let worst = residual(&loads).iter().fold(0_f64, |a, b| a.max(b.abs()));
        let within = x[0] >= 0. && x[0] <= 1. && x[1..4].iter().all(|v| v.abs() <= 1.);
        (worst < 1e-6 && within && x.iter().all(|v| v.is_finite())).then_some(Trim {
            collective: x[0],
            controls: [x[1], x[2], x[3]],
            pitch: x[4],
            bank: x[5],
            rotors: instant.rotors,
            engine_power: loads.power,
        })
    }
}

/// The conditions of a trim.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TrimConditions {
    hazards: Hazards,
    heading: f64,
    airspeed_fps: f64,
    density: f64,
    drag_factor: f64,
    level: tore_input::StabilityLevel,
}

/// A trimmed state of flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    pub collective: f64,
    /// Stick and pedal [pitch, roll, pedal].
    pub controls: [f64; 3],
    pub pitch: f64,
    pub bank: f64,
    pub rotors: [Rotor; 2],
    pub engine_power: f64,
}

impl State {
    /// Fuselage drag growth from carried stores and regional damage.
    fn tandem_drag_factor(&self, c: &Configuration, damage_percent: f64) -> f64 {
        (1. + self.carried_lbs() / c.mass.empty_lbs * c.aerodynamics.loaded_drag_percent / 100.)
            * (1. + damage_percent / 100.)
    }

    /// Trims the CH-47 in level flight at `airspeed_fps` (0 for a hover) at
    /// its current weight and altitude, out of ground effect: attitude,
    /// collective, stick and pedal trim, the rotors' inflow and tilt, rotor
    /// speed and engine power, with the body at rest. Sets the velocity along
    /// the heading for a forward trim. Returns false, and changes nothing,
    /// when no trim within the controls' travel exists.
    pub fn trim_tandem(&mut self, airspeed_fps: f64) -> bool {
        let Some(lift) = self.model().powered_lift() else {
            return false;
        };
        let c = self.model().configuration();
        let Some(model) = Tandem::new(&lift, c) else {
            return false;
        };
        let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        let drag_factor = self.tandem_drag_factor(c, 0.);
        let density = rotor::air_density(self.position[1]);
        let level = self
            .stability_in_effect()
            .unwrap_or(tore_input::StabilityLevel::Off);
        let hazards = self.rotor_hazards();
        let Some(trim) = model.trim_with(
            hazards,
            weight,
            self.yaw,
            airspeed_fps,
            density,
            drag_factor,
            level,
        ) else {
            return false;
        };
        self.pitch = trim.pitch;
        self.bank = trim.bank;
        self.roll_rate = 0.;
        self.pitch_rate = 0.;
        self.lift_controls.body_rates = [0.; 3];
        self.lift_controls.collective = trim.collective;
        self.lift_controls.collective_actual = trim.collective;
        self.lift_controls.aids.trim = trim.controls;
        self.lift_controls.aids.attitude_reference = [trim.pitch, trim.bank, self.yaw];
        self.lift_controls.rotors = trim.rotors;
        self.lift_controls.drive.rotor_speed = 1.;
        self.lift_controls.drive.rotor_speed_reference = 1.;
        self.lift_controls.drive.engine_output[0] = trim.engine_power;
        self.lift_controls.thrust_lbf = weight;
        self.throttle = 1.;
        if airspeed_fps > 0. {
            let forward = Basis::new(self.yaw, 0., 0.).forward;
            self.velocity = forward.map(|f| f * airspeed_fps);
            self.speed = airspeed_fps;
        }
        true
    }

    /// One tick of the CH-47. The velocity is air-relative on entry (the
    /// adapter took the wind out); the shared tail of the powered step (wind
    /// back, position, contact) follows.
    #[allow(clippy::too_many_arguments)] // Shared tick data already calculated by the adapter.
    pub(super) fn step_tandem(
        &mut self,
        lift: PoweredLift,
        model: Tandem,
        c: &Configuration,
        stick: [f64; 3],
        initial_surface: crate::research::Surface,
        runway_wind_fraction: f64,
        mut t: trace::AdapterTrace,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
        fuel_rate: f64,
    ) {
        let hydraulics = self.systems.fluids.hydraulic > 0.;
        self.lift_controls.advance(hydraulics);
        if hydraulics {
            let lever = &mut self.lift_controls;
            let step = COLLECTIVE_RATE * DT;
            lever.collective_actual = (lever.collective_actual
                + (lever.collective - lever.collective_actual).clamp(-step, step))
            .clamp(0., 1.);
        }
        let hazards = self.rotor_hazards();
        let carried = self.carried_lbs();
        let weight = c.mass.empty_lbs + self.fuel + carried;
        let mass = weight / GRAVITY;
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let density = rotor::air_density(self.position[1]);
        let effects = t.regional.effects;
        let wheel_contact = self.research.as_ref().is_some_and(|r| r.on_ground);
        let hubs = model.hubs(&basis);
        let height = |hub: Vector| Some(self.position[1] + hub[1] - initial_surface.height);
        let mut instant = Instant {
            basis,
            air_velocity: self.velocity,
            body_rates: self.lift_controls.body_rates,
            density,
            rotor_speed: self.lift_controls.drive.rotor_speed,
            rotors: self.lift_controls.rotors,
            engine_power: self.lift_controls.drive.engine_output[0],
            collective: self.lift_controls.collective_actual,
            controls: stick,
            longitudinal_trim: 0.,
            hub_height_agl_ft: [height(hubs[0]), height(hubs[1])],
            seconds: self.ticks as f64 * DT,
            hazards,
            drag_factor: self.tandem_drag_factor(c, effects.drag_percent),
            lift_factor: effects.lift,
        };
        // The stability level's feedback, from this tick's air data and the
        // torque residual at the pilot's own controls.
        let provisional = model.loads(&instant);
        let sensed = model.sensed(&instant, &provisional);
        let augmented = self.augment(&lift, stick, sensed);
        let controls = augmented.controls;
        instant.controls = controls;
        instant.longitudinal_trim = augmented.longitudinal_trim;
        let loads = model.loads(&instant);
        for (state, out) in self.lift_controls.rotors.iter_mut().zip(&loads.rotors) {
            rotor::relax(state, out, DT);
        }
        // Rotor speed and engines.
        let available = if self.engine {
            model.available_power(density, self.systems.power_available(), self.throttle)
        } else {
            0.
        };
        let next = model.drive.advance(
            &mut self.lift_controls.drive,
            DriveStep {
                load_power: loads.power,
                available_power: available,
                rotor_stall_hazard: hazards.rotor_stall,
                wheel_contact,
                seconds: DT,
            },
        );
        self.lift_controls.thrust_lbf = loads.thrust_lbf;
        let warnings = &mut self.lift_controls.warnings;
        drive::update_warnings(warnings, next);
        warnings.blade_stall = if loads.rotors.iter().any(|o| o.blade_stall > 0.05) {
            warnings.blade_stall.saturating_add(1)
        } else {
            0
        };
        // The rigid body, then the velocity, from the start-of-tick loads.
        let inertia = Inertia::from_weight(weight, lift.body.radii_of_gyration_ft);
        self.advance_body(inertia, loads.moments);
        for (velocity, force) in self.velocity.iter_mut().zip(loads.force) {
            *velocity += force / mass * DT;
        }
        self.velocity[1] -= GRAVITY * DT;
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        let support = loads.force[1] / weight;
        let wheel_load = (1. - support).clamp(0., 1.);
        self.g = dot(loads.force, basis.up) / weight;
        self.lift_g = loads.thrust_lbf / weight;
        let rates = self.lift_controls.body_rates;
        self.maneuver = crate::telemetry::Maneuver {
            tick: self.ticks,
            commanded_g: 1.,
            lift_g: self.lift_g,
            achieved_g: self.g,
            body_rates_rad_per_second: rates,
            rudder_command: stick[2],
            rudder_deflection: self.rudder,
            effective_rudder: controls[2],
            ..Default::default()
        };
        let envelope = c.aerodynamics.envelopes.iter().find(|e| e.g == 1).unwrap();
        let ceiling = envelope.points.iter().map(|p| p[1]).fold(0., f64::max);
        let (clean_stall, top_speed) = envelope
            .speeds(self.position[1].min(ceiling))
            .unwrap_or((200., 600.));
        t.power = trace::PowerTrace {
            engine: self.engine,
            fuel_starved: self.fuel + self.systems.external_lbs() <= 0.,
            afterburner: false,
            burner_blocked: self.burner_block(),
            throttle: self.throttle,
            afterburner_throttle: c.equipment.afterburner_throttle,
            fuel_flow_lbs_per_second: if self.engine { fuel_rate } else { 0. },
            unlimited_fuel: self.cheats.unlimited_fuel,
            rated_thrust_lbf: model.max_thrust,
            lapse: available / model.drive.rated_power.max(1.),
            power_available: self.systems.power_available(),
            thrust_lbf: loads.thrust_lbf,
        };
        t.forces = trace::ForceTrace {
            weight_lbs: weight,
            carried_lbs: carried,
            payload_lbs: self.payload_lbs,
            ignore_weapon_weights: self.cheats.ignore_weapon_weights,
            drag: trace::DragTrace {
                total_lbf: loads.airframe.drag_lbf,
                airframe_lbf: loads.airframe.drag_lbf,
                ..Default::default()
            },
            achieved_g: self.g,
            support_g: support,
            wheel_load,
            ..Default::default()
        };
        t.envelope.clean_stall_fps = clean_stall;
        t.envelope.stall_fps = clean_stall;
        t.envelope.top_speed_fps = top_speed;
        t.envelope.limits_g = [-1., 1.];
        t.lift.commanded_g = 1.;
        self.trace.0.adapter = Some(t);
        for (velocity, wind) in self.velocity.iter_mut().zip(initial_surface.wind) {
            *velocity += wind;
        }
        self.vertical_speed = self.velocity[1];
        // Dynamic rollover: on the wheels, banked past the limit with the
        // thrust leaning the same way (design 4.9).
        let lean =
            0.5 * (self.lift_controls.rotors[0].tilt[1] + self.lift_controls.rotors[1].tilt[1]);
        let rolled = wheel_contact
            && hazards.dynamic_rollover
            && self.bank.abs() > ROLLOVER_BANK_DEGREES.to_radians()
            && loads.thrust_lbf > ROLLOVER_THRUST_SHARE * weight
            && self.bank.signum() * lean > -0.01;
        let bank_before_contact = self.bank;
        let previous_position = self.advance_position();
        let surface = ground(self.position[0], self.position[2]);
        let mut research = self.research.take().expect("hybrid powered lift");
        research.departure = Default::default();
        research.spinning = 0;
        research.spin_rate = 0.;
        research.severity_f8 = 0;
        research.stall_active = false;
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
        if rolled && !self.crashed {
            self.crashed = true;
            self.engine = false;
            self.burner = false;
            self.velocity = [0.; 3];
            self.speed = 0.;
            self.vertical_speed = 0.;
            return;
        }
        if self.weight_on_wheels() && !self.crashed {
            // The contact holds pitch and the loaded wheels hold yaw. A roll
            // the weight cannot hold about the wheels banks the aircraft.
            let rates = &mut self.lift_controls.body_rates;
            rates[1] = 0.;
            if wheel_load > WHEEL_YAW_HOLD {
                rates[2] = 0.;
            }
            let lifted = (weight - loads.force[1]).max(0.) * model.airframe.half_track_ft;
            let pivot = hazards.dynamic_rollover && loads.moments.applied[0].abs() > lifted;
            if pivot {
                self.bank = bank_before_contact;
            } else {
                rates[0] = 0.;
            }
            self.roll_rate = rates[0];
            self.pitch_rate = rates[1];
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::flight::PilotInput;
    use tore_formats::aircraft::{Aircraft, AircraftId};

    /// A synthetic record under the CH-47's identity carrying the PT's own
    /// flight numbers (design 8.2), as plain constants: 18,078 lb empty,
    /// 3,307 lb fuel, 28,660 lb maximum, 135,795 lbf (implausible, so the
    /// model fits its own), 53 to 178 kt to 7,000 ft.
    pub(crate) fn pt_aircraft() -> Aircraft {
        let mut a = crate::models::variety::tests::synthetic(AircraftId::Ch47);
        for (key, value) in [
            ("weight", 18_078),
            ("internalFuel", 3_307),
            ("maxTakeoffWeight", 28_660),
            ("thrust", 135_795),
        ] {
            a.fields.get_mut(key).unwrap().value = value.to_string();
        }
        let [slow, fast] = [53., 178.].map(|kt: f64| kt * 1.687_81);
        for e in &mut a.envelopes {
            e.points = vec![
                [slow, 0.],
                [slow + 10., 7_000.],
                [fast - 20., 7_000.],
                [fast, 0.],
            ];
        }
        a
    }

    /// A hybrid flight trimmed level at `airspeed_fps` and `height`, heading
    /// north, stability augmentation at `level`.
    pub(crate) fn trimmed_at(
        height: f64,
        airspeed_fps: f64,
        level: tore_input::StabilityLevel,
    ) -> State {
        let mut s = State::new(&pt_aircraft(), [0., height, 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.velocity = [0.; 3];
        s.speed = 0.;
        s.lift_controls.aids.stability = level;
        assert!(s.trim_tandem(airspeed_fps), "CH-47 trims");
        s
    }

    pub(crate) fn model(s: &State) -> Tandem {
        Tandem::new(
            &s.model().powered_lift().unwrap(),
            s.model().configuration(),
        )
        .unwrap()
    }

    /// Flies `ticks` ticks over flat ground at sea level with the inputs
    /// `pilot` gives each tick.
    pub(crate) fn fly(s: &mut State, ticks: usize, mut pilot: impl FnMut(&State) -> PilotInput) {
        for _ in 0..ticks {
            let input = pilot(s);
            s.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
        }
    }
}

#[cfg(test)]
#[path = "tandem_tests.rs"]
mod acceptance;
