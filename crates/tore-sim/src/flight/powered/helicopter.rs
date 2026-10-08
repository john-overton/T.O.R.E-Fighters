//! The single-rotor helicopters, AH-64 and Mi-24 (VTOL overhaul design 4.5,
//! slice P2): one [`super::rotor`] main rotor, an anti-torque tail rotor,
//! the [`super::fuselage`] surfaces, governed engines and the rotor speed,
//! flown on the [`super::body`] rigid body. There are no attitude limits
//! and no velocity damping: the helicopter flies where its disk points, as
//! fast as its power allows.
//!
//! - **Controls**: the pilot's stick and pedals plus trim (slice P6's
//!   `State::trimmed_stick`, before the law), through the stability level
//!   ([`super::sas`], fed this law's airspeed, sideslip and the pedal that
//!   would cancel the drive torque), into the cyclic and the tail rotor.
//! - **Tail rotor**: thrust `Nr² (rho/rho0) (T_ref - pedal T_range)`
//!   sideways at the tail arm, where `T_ref` balances the hover torque at the
//!   reference weight. Its yaw damping is its own heave damping times the arm
//!   squared; its power is momentum power with the translational gain.
//! - **Torque**: the drive's torque `(P_engine - P_tail) / (Nr Omega)` reacts
//!   on the airframe about the shaft, so collective swings the nose and an
//!   autorotating rotor leaves none.
//! - **Engines and governor**: the engines aim at the power the rotors need
//!   plus `K (Nr_ref - Nr) P_rated`, within what they have
//!   (`P_rated x (rho/rho0)^0.8 x damage x throttle`), with a lag. The rotor
//!   speed moves with the difference between engine and rotor power over its
//!   energy constant.
//! - **Rated power** is the power that hovers at the PT maximum thrust at
//!   sea level (design 8.1): about 3,600 hp for the AH-64's PT.
//! - **Ground**: wheels hold yaw and pitch; a roll about the wheels that the
//!   weight cannot hold banks the aircraft, and 15 degrees of bank with the
//!   thrust leaning the same way is dynamic rollover (design 4.9).
//!
//! The CH-47 (P3) and V-22 (P5) still fly the old powered law; their rotors
//! will reuse [`super::rotor::RotorModel`] and this file's drive and trim
//! pieces where they fit.

use super::{
    DT, State, airframe,
    body::{GRAVITY, Inertia, Moments},
    fuselage::{self, AirframeInput, AirframeLoads},
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
        variety::{
            PoweredLift, RotorLayout, RotorRotation, RotorcraftAirframe, TailRotorParameters,
        },
    },
};

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
pub const EASY_ROTOR_FLOOR: f64 = 0.85;
/// Rotor speed limits of the integration.
const ROTOR_SPEED_LIMIT: f64 = 1.5;
/// Dynamic rollover: bank on the wheels beyond this with the thrust leaning
/// the same way, carrying more than this share of the weight (fitted).
const ROLLOVER_BANK_DEGREES: f64 = 15.;
const ROLLOVER_THRUST_SHARE: f64 = 0.1;
/// The wheels hold yaw while they carry more than this share of the weight.
const WHEEL_YAW_HOLD: f64 = 0.05;
/// Collective lever travel per second (fitted: full travel in half a
/// second, a hand's speed).
const COLLECTIVE_RATE: f64 = 2.;
/// Tail rotor height above the centre of gravity, as a share of the hub's
/// (fitted).
const TAIL_ROTOR_HEIGHT_SHARE: f64 = 0.35;
/// Trim: Newton iterations, inner equilibrium passes and the finite
/// difference step.
const TRIM_ITERATIONS: usize = 16;
const TRIM_EQUILIBRIUM_PASSES: usize = 80;
const TRIM_STEP: f64 = 1e-6;

/// One single-rotor helicopter's model, derived from its parameters and its
/// configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SingleRotor {
    pub lift: PoweredLift,
    pub rotor: RotorModel,
    pub tail: TailRotorParameters,
    /// Tail rotor arm behind the centre of gravity, ft.
    pub tail_arm_ft: f64,
    pub airframe: RotorcraftAirframe,
    /// Empty plus full internal fuel, lb.
    pub reference_weight: f64,
    /// Main rotor torque in a sea-level hover at the reference weight,
    /// ft·lbf.
    pub reference_torque: f64,
    /// Tail rotor thrust that balances it, lbf.
    pub tail_reference_thrust: f64,
    /// Rated power at sea level, ft·lbf/s.
    pub rated_power: f64,
    /// Rotor energy constant `J Omega0²`, ft·lbf.
    pub rotor_energy: f64,
    /// Maximum static thrust at sea level, lbf.
    pub max_thrust: f64,
}

/// The tail rotor's loads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TailLoads {
    /// Force along the body's right axis, lbf.
    pub force_right_lbf: f64,
    /// Power it absorbs, ft·lbf/s.
    pub power: f64,
    /// Its yaw damping, ft·lbf per rad/s.
    pub yaw_damping: f64,
}

/// One instant of a helicopter, enough to find its loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instant {
    pub basis: Basis,
    /// Air velocity, world axes, ft/s.
    pub air_velocity: Vector,
    pub density: f64,
    pub rotor_speed: f64,
    pub rotor: Rotor,
    /// Shaft power the engines deliver, ft·lbf/s.
    pub engine_power: f64,
    /// Collective lever 0..1, and stick and pedal [pitch (aft positive),
    /// roll (right), pedal (nose right)].
    pub collective: f64,
    pub controls: [f64; 3],
    /// Hub height above the surface, ft; none out of ground effect.
    pub hub_height_agl_ft: Option<f64>,
    pub seconds: f64,
    pub hazards: Hazards,
    /// Fuselage drag growth (stores, damage) and stub wing lift loss.
    pub drag_factor: f64,
    pub lift_factor: f64,
}

/// Everything acting on the helicopter at one instant, except gravity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loads {
    /// World axes, lbf.
    pub force: Vector,
    pub moments: Moments,
    pub main: RotorOutput,
    pub tail: TailLoads,
    pub airframe: AirframeLoads,
    /// Main plus tail rotor power, ft·lbf/s.
    pub power: f64,
    /// The drive torque's yaw moment on the airframe, ft·lbf.
    pub torque_yaw: f64,
}

impl SingleRotor {
    /// The model of a single-rotor helicopter, or none for any other
    /// aircraft.
    pub fn new(lift: &PoweredLift, c: &Configuration) -> Option<Self> {
        let p = lift.rotor?;
        let RotorLayout::Single {
            rotation,
            tail_rotor_arm_ft,
        } = p.layout
        else {
            return None;
        };
        let tail = p.tail_rotor?;
        let turn = match rotation {
            RotorRotation::CounterClockwise => 1.,
            RotorRotation::Clockwise => -1.,
        };
        let reference_weight = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
        let rotor = RotorModel::new(&p, turn, reference_weight);
        let rho0 = rotor::sea_level_density();
        let mut model = Self {
            lift: *lift,
            rotor,
            tail,
            tail_arm_ft: tail_rotor_arm_ft,
            airframe: p.airframe,
            reference_weight,
            reference_torque: 0.,
            tail_reference_thrust: 0.,
            rated_power: 0.,
            rotor_energy: 0.,
            max_thrust: p.max_thrust_lbf,
        };
        let main_hover = rotor.hover_power(reference_weight, rho0);
        model.reference_torque = main_hover / rotor.omega;
        model.tail_reference_thrust = model.reference_torque / tail_rotor_arm_ft;
        let hover = main_hover + model.tail_power(model.tail_reference_thrust, rho0, 1., 0.);
        let main_rated = rotor.hover_power(p.max_thrust_lbf, rho0);
        model.rated_power = main_rated
            + model.tail_power(main_rated / rotor.omega / tail_rotor_arm_ft, rho0, 1., 0.);
        // With the engines cut and the collective held, the power the rotor
        // needs falls with the cube of its speed, so its speed falls from 1
        // to 0.8 in `energy_seconds` when J Omega0² = 4 x that x the hover
        // power.
        model.rotor_energy = 4. * p.energy_seconds * hover;
        Some(model)
    }

    fn tail_area(&self) -> f64 {
        std::f64::consts::PI * self.tail.radius_ft * self.tail.radius_ft
    }

    /// Tail rotor power at a thrust, ft·lbf/s: momentum power, less with
    /// airspeed, plus profile power.
    fn tail_power(&self, thrust: f64, density: f64, rotor_speed: f64, airspeed: f64) -> f64 {
        let area = self.tail_area();
        let hover = thrust.abs() / (2. * density * area);
        let induced = hover / (airspeed * airspeed + hover).sqrt().max(1e-9);
        let tip = rotor_speed * self.tail.tip_speed_fps;
        thrust.abs() * rotor::INDUCED_POWER_FACTOR * induced
            + density * area * tip * tip * tip * self.tail.solidity * self.rotor.profile_drag() / 8.
    }

    /// The tail rotor's loads at an instant with drive torque `torque`.
    fn tail_loads(&self, i: &Instant, torque: f64, airspeed: f64) -> TailLoads {
        let rho = i.density;
        let nr = i.rotor_speed.max(0.);
        let scale = nr * nr * rho / rotor::sea_level_density();
        let range = self.tail.pedal_authority * self.tail_reference_thrust;
        // Easy flight physics: the anti-torque thrust is whatever cancels
        // the torque, so collective never yaws the aircraft.
        let anti = if i.hazards.torque {
            scale * self.tail_reference_thrust
        } else {
            torque / self.tail_arm_ft
        };
        let commanded = self.rotor.turn * anti - scale * range * i.controls[2].clamp(-1., 1.);
        let area = self.tail_area();
        let tip = nr * self.tail.tip_speed_fps;
        let hover = commanded.abs() / (2. * rho * area);
        let induced = hover / (airspeed * airspeed + hover).sqrt().max(1e-9);
        // Blade element heave damping in series with the momentum one: the
        // tail rotor's thrust falls as the tail moves along it. Sideways
        // motion so meets a drag and turns the nose into the airflow; the
        // yaw rate's share is the yaw damping, applied implicitly.
        let blade = rho * area * tip * self.tail.solidity * rotor::LIFT_SLOPE / 4.;
        let heave = blade / (1. + blade / (4. * rho * area * induced.max(1.)));
        let force_right_lbf = commanded - heave * dot(i.air_velocity, i.basis.right);
        TailLoads {
            force_right_lbf,
            power: self.tail_power(force_right_lbf, rho, nr, airspeed),
            yaw_damping: heave * self.tail_arm_ft * self.tail_arm_ft,
        }
    }

    /// The main rotor's input at an instant.
    fn rotor_input(&self, i: &Instant) -> RotorInput {
        RotorInput {
            frame: DiskFrame {
                shaft: i.basis.up,
                forward: i.basis.forward,
                right: i.basis.right,
            },
            air_velocity: i.air_velocity,
            density: i.density,
            rotor_speed: i.rotor_speed,
            collective: i.collective,
            // Aft stick (positive pitch) tilts the disk aft.
            cyclic: [-i.controls[0], i.controls[1]],
            hub_height_agl_ft: i.hub_height_agl_ft,
            seconds: i.seconds,
            hazards: i.hazards,
        }
    }

    /// The drive's torque on the main rotor at `i`, ft·lbf, and the tail
    /// rotor's loads before any easy-physics balancing.
    fn drive_torque(&self, i: &Instant) -> (f64, TailLoads) {
        let airspeed = dot(i.air_velocity, i.air_velocity).sqrt();
        let nr = i.rotor_speed.max(0.05);
        let provisional = self.tail_loads(i, 0., airspeed);
        (
            (i.engine_power - provisional.power) / (nr * self.rotor.omega),
            provisional,
        )
    }

    /// What the stability law senses at `i`: airspeed, sideslip and the
    /// pedal that would cancel the drive torque (none when the Easy flight
    /// physics balance it already).
    pub fn sensed(&self, i: &Instant) -> sas::Sensed {
        let airspeed_fps = dot(i.air_velocity, i.air_velocity).sqrt();
        let sideslip_rad = if airspeed_fps > 1e-9 {
            (dot(i.air_velocity, i.basis.right) / airspeed_fps)
                .clamp(-1., 1.)
                .asin()
        } else {
            0.
        };
        let torque_pedal = if i.hazards.torque {
            self.torque_pedal(self.drive_torque(i).0, i.rotor_speed, i.density)
        } else {
            0.
        };
        sas::Sensed {
            airspeed_fps,
            sideslip_rad,
            torque_pedal,
        }
    }

    /// Everything acting on the helicopter at `i`, except gravity.
    pub fn loads(&self, i: &Instant) -> Loads {
        let basis = i.basis;
        let main = self.rotor.evaluate(&i.rotor, &self.rotor_input(i));
        let airspeed = dot(i.air_velocity, i.air_velocity).sqrt();
        // The tail rotor's share first, to know the main rotor's torque.
        let (torque, provisional) = self.drive_torque(i);
        let tail = if i.hazards.torque {
            provisional
        } else {
            self.tail_loads(i, torque, airspeed)
        };
        let airframe = fuselage::airframe_loads(
            &self.airframe,
            &AirframeInput {
                basis,
                air_velocity: i.air_velocity,
                density: i.density,
                tail_arm_ft: self.tail_arm_ft,
                drag_factor: i.drag_factor,
                turn: self.rotor.turn,
                reference_torque: self.reference_torque,
                lift_factor: i.lift_factor,
            },
        );
        let h = self.rotor.hub_height_ft;
        let thrust: Vector = main.normal.map(|n| n * main.thrust_lbf);
        let hub: Vector = basis.up.map(|u| u * h);
        // The thrust about the centre of gravity, plus the hub moment.
        let lever = cross(hub, thrust);
        let rotor_moment: Vector = std::array::from_fn(|k| lever[k] + main.hub_moment[k]);
        let mut applied = fuselage::body_moments(&basis, rotor_moment);
        // The tail rotor a third of the way up to the hub, the tail arm behind.
        let side = tail.force_right_lbf;
        applied[0] += TAIL_ROTOR_HEIGHT_SHARE * h * side;
        applied[2] -= self.tail_arm_ft * side;
        let torque_yaw = self.rotor.turn * torque;
        applied[2] += torque_yaw;
        for (a, b) in applied.iter_mut().zip(airframe.moments) {
            *a += b;
        }
        // The rotor's rate damping: the disk lags the body by the rate lag,
        // and each radian of lag is the hub stiffness plus the thrust's
        // lever arm.
        let rotor_damping =
            (main.tilt_stiffness + main.thrust_lbf.max(0.) * h.abs()) * main.rate_lag_seconds;
        let damping = [
            rotor_damping + airframe.damping[0],
            rotor_damping + airframe.damping[1],
            tail.yaw_damping + airframe.damping[2],
        ];
        let force = std::array::from_fn(|k| thrust[k] + side * basis.right[k] + airframe.force[k]);
        Loads {
            force,
            moments: Moments { applied, damping },
            main,
            tail,
            airframe,
            power: main.power + tail.power,
            torque_yaw,
        }
    }

    /// The pedal that makes the tail rotor balance a drive `torque`
    /// (ft·lbf) at a rotor speed and density in still air: what a
    /// collective-to-pedal feed-forward (slice P6's Damper) aims at.
    pub fn torque_pedal(&self, torque: f64, rotor_speed: f64, density: f64) -> f64 {
        let scale = rotor_speed * rotor_speed * density / rotor::sea_level_density();
        let range = scale * self.tail.pedal_authority * self.tail_reference_thrust;
        (scale * self.tail_reference_thrust - torque / self.tail_arm_ft) * self.rotor.turn
            / range.max(1e-9)
    }

    /// Engine power the helicopter can have now, ft·lbf/s.
    pub fn available_power(&self, density: f64, damage: f64, throttle: f64) -> f64 {
        self.rated_power
            * (density / rotor::sea_level_density()).powf(ENGINE_LAPSE_EXPONENT)
            * damage
            * throttle.clamp(0., 1.)
    }

    /// The trimmed state of flight at `airspeed_fps` along `heading`, level,
    /// at `density`, out of ground effect, with the stability law at `level`
    /// in the loop (at rest it adds only its torque feed-forward and
    /// sideslip terms): [collective, pitch stick, roll stick, pedal, pitch,
    /// bank] with the rotor's equilibrium, or none when the solution needs
    /// more than full travel of any control.
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

    /// [`SingleRotor::trim`] under the given `hazards`: with the Easy flight
    /// physics cheat's none, the trim needs no anti-torque pedal.
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
        let air_velocity = Basis::new(heading, 0., 0.)
            .forward
            .map(|f| f * airspeed_fps);
        let induced = (weight / (2. * density * self.rotor.area_ft2)).sqrt();
        let pitch_guess = self.rotor.blade_pitch(0.)
            + 3. * (weight
                / (density * self.rotor.area_ft2 * self.rotor.tip_speed_fps.powi(2))
                / (self.rotor.solidity * rotor::LIFT_SLOPE / 2.)
                + induced / self.rotor.tip_speed_fps / 2.);
        let mut x = [self.rotor.lever_for_pitch(pitch_guess), 0., 0., 0., 0., 0.];
        let evaluate = |x: &[f64; 6]| -> (Instant, Loads) {
            let basis = Basis::new(heading, x[4], x[5]);
            let mut instant = Instant {
                basis,
                air_velocity,
                density,
                rotor_speed: 1.,
                rotor: Rotor {
                    induced_fps: induced,
                    tilt: [0.; 2],
                },
                engine_power: 0.,
                collective: x[0],
                controls: [x[1], x[2], x[3]],
                hub_height_agl_ft: None,
                seconds: 0.,
                hazards,
                drag_factor,
                lift_factor: 1.,
            };
            let pilot = [x[1], x[2], x[3]];
            let mut aids = PilotAids {
                stability: level,
                trim: pilot,
                attitude_reference: [x[4], x[5], heading],
                ..Default::default()
            };
            let body = sas::Body {
                attitude: [x[4], x[5], heading],
                rates: [0.; 3],
            };
            let mut loads = self.loads(&instant);
            for _ in 0..TRIM_EQUILIBRIUM_PASSES {
                instant.rotor.induced_fps +=
                    0.5 * (loads.main.induced_target_fps - instant.rotor.induced_fps);
                instant.rotor.tilt = loads.main.tilt_target;
                instant.engine_power = loads.power;
                instant.controls = pilot;
                let sensed = self.sensed(&instant);
                instant.controls =
                    sas::augment(&self.lift, level, &mut aids, pilot, body, sensed).controls;
                loads = self.loads(&instant);
            }
            (instant, loads)
        };
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
            rotor: instant.rotor,
            engine_power: loads.power,
        })
    }
}

/// A trimmed state of flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    pub collective: f64,
    /// Stick and pedal [pitch, roll, pedal].
    pub controls: [f64; 3],
    pub pitch: f64,
    pub bank: f64,
    pub rotor: Rotor,
    pub engine_power: f64,
}

/// Solves `a x = b` by Gaussian elimination with partial pivoting, or none
/// when `a` is singular.
fn solve(mut a: [[f64; 6]; 6], mut b: [f64; 6]) -> Option<[f64; 6]> {
    for col in 0..6 {
        let pivot = (col..6).fold(col, |best, row| {
            if a[row][col].abs() > a[best][col].abs() {
                row
            } else {
                best
            }
        });
        if a[pivot][col].abs() < 1e-14 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..6 {
            let factor = a[row][col] / a[col][col];
            let pivot_row = a[col];
            for (value, pivot) in a[row].iter_mut().zip(pivot_row).skip(col) {
                *value -= factor * pivot;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = [0.; 6];
    for row in (0..6).rev() {
        let sum: f64 = (row + 1..6).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

impl State {
    /// The rotor hazards in force: all of them, or none with the Easy flight
    /// physics cheat. Every rotorcraft step builds its rotors' hazards here.
    pub(super) fn rotor_hazards(&self) -> Hazards {
        if self.cheats.easy_physics {
            Hazards::NONE
        } else {
            Hazards::ALL
        }
    }

    /// Fuselage drag growth from carried stores and regional damage.
    fn rotorcraft_drag_factor(&self, c: &Configuration, damage_percent: f64) -> f64 {
        (1. + self.carried_lbs() / c.mass.empty_lbs * c.aerodynamics.loaded_drag_percent / 100.)
            * (1. + damage_percent / 100.)
    }

    /// Trims a single-rotor helicopter in level flight at `airspeed_fps`
    /// (0 for a hover) at its current weight and altitude, out of ground
    /// effect: attitude, collective, the stick and pedal trim, the rotor's
    /// inflow and tilt, rotor speed and engine power, with the body at rest.
    /// Sets the velocity along the heading for a forward trim; a hover keeps
    /// the velocity it has (the wind it drifts with). Returns false, and
    /// changes nothing, when no trim within the controls' travel exists.
    pub fn trim_single_rotor(&mut self, airspeed_fps: f64) -> bool {
        let Some(lift) = self.model().powered_lift() else {
            return false;
        };
        let c = self.model().configuration();
        let Some(heli) = SingleRotor::new(&lift, c) else {
            return false;
        };
        let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        let drag_factor = self.rotorcraft_drag_factor(c, 0.);
        let density = rotor::air_density(self.position[1]);
        let level = self
            .stability_in_effect()
            .unwrap_or(tore_input::StabilityLevel::Off);
        let hazards = self.rotor_hazards();
        let Some(trim) = heli.trim_with(
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
        self.lift_controls.rotors[0] = trim.rotor;
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

    /// One tick of a single-rotor helicopter. The velocity is air-relative
    /// on entry (the adapter took the wind out); the shared tail of the
    /// powered step (wind back, position, contact) follows.
    #[allow(clippy::too_many_arguments)] // Shared tick data already calculated by the adapter.
    pub(super) fn step_single_rotor(
        &mut self,
        lift: PoweredLift,
        heli: SingleRotor,
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
        // The collective lever moves at hand speed, not the old law's
        // actuator rate.
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
        let h = heli.rotor.hub_height_ft;
        let mut instant = Instant {
            basis,
            air_velocity: self.velocity,
            density,
            rotor_speed: self.lift_controls.drive.rotor_speed,
            rotor: self.lift_controls.rotors[0],
            engine_power: self.lift_controls.drive.engine_output[0],
            collective: self.lift_controls.collective_actual,
            controls: stick,
            hub_height_agl_ft: Some(self.position[1] + h * basis.up[1] - initial_surface.height),
            seconds: self.ticks as f64 * DT,
            hazards,
            drag_factor: self.rotorcraft_drag_factor(c, effects.drag_percent),
            lift_factor: effects.lift,
        };
        // The stability level's feedback, from this tick's air data.
        let sensed = heli.sensed(&instant);
        let controls = self.augment(&lift, stick, sensed).controls;
        instant.controls = controls;
        let loads = heli.loads(&instant);
        rotor::relax(&mut self.lift_controls.rotors[0], &loads.main, DT);
        // Rotor speed and engines.
        let drive = &mut self.lift_controls.drive;
        let nr = drive.rotor_speed;
        let available = if self.engine {
            heli.available_power(density, self.systems.power_available(), self.throttle)
        } else {
            0.
        };
        let engine = drive.engine_output[0];
        let mut next = (nr + DT * (engine - loads.power) / (heli.rotor_energy * nr.max(0.05)))
            .clamp(0., ROTOR_SPEED_LIMIT);
        if !hazards.rotor_stall && !wheel_contact {
            next = next.max(EASY_ROTOR_FLOOR);
        }
        drive.rotor_speed = next;
        let demand = (loads.power
            + GOVERNOR_GAIN * (drive.rotor_speed_reference - nr) * heli.rated_power)
            .clamp(0., available);
        drive.engine_output[0] = if available > 0. {
            engine + (demand - engine) * (DT / ENGINE_SECONDS).min(1.)
        } else {
            0.
        };
        self.lift_controls.thrust_lbf = loads.main.thrust_lbf;
        let warnings = &mut self.lift_controls.warnings;
        let count = |ticks: u32, on: bool| if on { ticks.saturating_add(1) } else { 0 };
        warnings.low_rotor = count(warnings.low_rotor, next < LOW_ROTOR);
        warnings.rotor_overspeed = count(warnings.rotor_overspeed, next > ROTOR_OVERSPEED);
        warnings.blade_stall = count(warnings.blade_stall, loads.main.blade_stall > 0.05);
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
        self.lift_g = loads.main.thrust_lbf / weight;
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
            rated_thrust_lbf: heli.max_thrust,
            lapse: available / heli.rated_power.max(1.),
            power_available: self.systems.power_available(),
            thrust_lbf: loads.main.thrust_lbf,
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
        let rolled = wheel_contact
            && hazards.dynamic_rollover
            && self.bank.abs() > ROLLOVER_BANK_DEGREES.to_radians()
            && loads.main.thrust_lbf > ROLLOVER_THRUST_SHARE * weight
            && self.bank.signum() * self.lift_controls.rotors[0].tilt[1] > -0.01;
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
            let lifted = (weight - loads.force[1]).max(0.) * heli.airframe.half_track_ft;
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

    /// A synthetic record under the helicopter's identity carrying the PT's
    /// own flight numbers (design 8.2), as plain constants: AH-64 18,298 lb
    /// empty, 2,000 lb fuel, 23,810 lb maximum, 26,280 lbf, 36 to 130 kt to
    /// 7,000 ft; Mi-24 18,078 / 3,307 / 28,660 lb, 35,691 lbf, 53 to 178 kt.
    pub(crate) fn pt_aircraft(id: AircraftId) -> Aircraft {
        let mut a = crate::models::variety::tests::synthetic(id);
        let (empty, fuel, maximum, thrust, slow, fast) = match id {
            AircraftId::Ah64 => (18_298, 2_000, 23_810, 26_280, 36., 130.),
            AircraftId::Mi24 => (18_078, 3_307, 28_660, 35_691, 53., 178.),
            _ => panic!("not a single-rotor helicopter"),
        };
        for (key, value) in [
            ("weight", empty),
            ("internalFuel", fuel),
            ("maxTakeoffWeight", maximum),
            ("thrust", thrust),
        ] {
            a.fields.get_mut(key).unwrap().value = value.to_string();
        }
        let [slow, fast] = [slow, fast].map(|kt: f64| kt * 1.687_81);
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

    /// A hybrid flight of `id` trimmed level at `airspeed_fps` and
    /// `height`, heading north, stability augmentation off.
    pub(crate) fn trimmed(id: AircraftId, height: f64, airspeed_fps: f64) -> State {
        let mut s = State::new(&pt_aircraft(id), [0., height, 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.velocity = [0.; 3];
        s.speed = 0.;
        s.lift_controls.aids.stability = tore_input::StabilityLevel::Off;
        assert!(s.trim_single_rotor(airspeed_fps), "{id:?} trims");
        s
    }

    pub(crate) fn heli(s: &State) -> SingleRotor {
        SingleRotor::new(
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
#[path = "helicopter_tests.rs"]
mod acceptance;
