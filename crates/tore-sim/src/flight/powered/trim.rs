//! The starts of the powered-lift aircraft (VTOL overhaul decision 8, slice
//! P7). They replace the old hover start, `initialize_airborne_hover`.
//!
//! - **Airborne starts** begin in trimmed forward flight, not a hover, at the
//!   fixed-wing rule's speed (65 percent of the level top speed at the start
//!   altitude, between 130 percent of the 1-G stall speed and 95 percent of
//!   top speed; [`crate::flight::State::retune_airborne_start_speed`]). The
//!   host calls [`State::start_airborne`] once, after the aircraft's final
//!   mass, altitude and attitude are set, on every spawn path: single
//!   player's restart, an AI actor put on the hybrid model (which is what a
//!   multiplayer seat is until a human takes it) and a revival.
//!   - A single-rotor helicopter gets the trim of
//!     [`State::trim_single_rotor`]: collective, cyclic and pedals, the
//!     rotor governed at 100 percent, the body at rest.
//!   - A vectoring jet gets its nozzles at 0, its lift engines off and a
//!     wingborne trim: the throttle that balances its drag and the pitch
//!     that makes its wing carry its weight, both found by probing the
//!     real force law one tick at a time (fixed iteration count, no clock,
//!     so the result is deterministic).
//! - **Ground starts** match the fixed-wing ones (stationary, engine idling,
//!   gear and flaps down, brakes on): [`State::start_on_ground`] is called
//!   by the runway start. A rotorcraft has its rotor at the governed speed
//!   with the collective down and its engines at 100 percent (the throttle
//!   keys drive the collective, so the engine throttle is set here and
//!   nowhere else); a jet has its nozzles at 0 and its engine at idle. No
//!   cold start.
//!
//! The CH-47 and the V-22 still fly the old fitted law (slices P3 and P5
//! replace it), so they keep their hover start; their hooks are marked
//! `TODO(P3)` and `TODO(P5)`.

use super::{
    helicopter::{Instant, SingleRotor},
    rotor::{self, Hazards},
    state::Rotor,
};
use crate::{
    attitude::{Basis, dot, unit},
    flight::{DT, PilotInput, State, variety_start_speed},
    models::{Conditions, FlightModel, config::Configuration, variety::PoweredLift},
    research::Surface,
};

/// Whether an aircraft starts airborne in trimmed forward flight now: the
/// single-rotor helicopters and the vectoring jets.
///
/// TODO(P3): the CH-47 joins this when its tandem law lands.
/// TODO(P5): the V-22 joins this when its tiltrotor law lands (airplane
/// mode at about 180 KCAS with the nacelles on the downstops).
pub fn starts_in_forward_flight(lift: &PoweredLift, c: &Configuration) -> bool {
    lift.jet.is_some() || SingleRotor::new(lift, c).is_some()
}

/// The V-22's nacelle angle on the ground, degrees (design section 7: the
/// helicopter preset).
///
/// TODO(P5): the ground start sets the nacelle demand to this share of the
/// 97.5-degree travel once the tiltrotor law reads it.
pub const V22_GROUND_NACELLE_DEGREES: f64 = 87.;

/// Newton iterations, finite-difference steps and the residual (in G) a jet
/// trim accepts.
const JET_TRIM_ITERATIONS: usize = 10;
const JET_THROTTLE_STEP: f64 = 0.01;
const JET_PITCH_STEP: f64 = 0.002;
const JET_TRIM_STEP: f64 = 0.002;
const JET_TRIM_TOLERANCE_G: f64 = 2e-5;
/// The pitch a jet trim starts from, rad.
const JET_PITCH_GUESS: f64 = 0.06;
/// The pitch rate residual's weight against the G ones.
const PITCH_RATE_SCALE: f64 = 10.;
/// How far below the jet the probe's ground is, ft: out of ground effect.
const PROBE_GROUND_DEPTH_FT: f64 = 100_000.;

impl State {
    /// Puts an airborne start that has not flown yet into trimmed forward
    /// flight at the airspeed it holds (the start rule's, see the module
    /// documentation), after its final mass, altitude and heading are set.
    /// `wind` is the wind the velocity is ground-relative to. Returns true
    /// when the aircraft was trimmed; false when it is not a powered-lift
    /// aircraft that trims yet (the V-22 and the CH-47 get a hover on the
    /// old law's collective), is not on the hybrid model, has flown, is on
    /// the ground, or no trim exists within the controls' travel (a helicopter
    /// then tries a hover, then falls back to full collective).
    pub fn start_airborne(&mut self, wind: [f64; 3]) -> bool {
        if self.ticks != 0
            || self.native.is_some()
            || self.crashed
            || self
                .research
                .as_ref()
                .is_none_or(|research| research.on_ground)
        {
            return false;
        }
        let Some(lift) = self.model().powered_lift() else {
            return false;
        };
        let c = self.model().configuration();
        if SingleRotor::new(&lift, c).is_some() {
            return self.start_single_rotor(wind);
        }
        if lift.jet.is_some() {
            return self.start_jet(wind);
        }
        // TODO(P3), TODO(P5): the CH-47 and the V-22 trim on their own laws.
        // Until then the old law's hover, as `initialize_airborne_hover` did.
        self.hover_on_the_old_law(&lift);
        false
    }

    /// The airspeed an airborne start flies at: the speed it holds, or the
    /// variety rule's at its altitude.
    fn start_airspeed(&self) -> Option<f64> {
        if self.speed > 0. {
            Some(self.speed)
        } else {
            variety_start_speed(self.model(), self.position[1])
        }
    }

    fn start_single_rotor(&mut self, wind: [f64; 3]) -> bool {
        let airspeed = self.start_airspeed().unwrap_or(0.);
        if !self.trim_single_rotor(airspeed) {
            // No trim at that speed: try the hover, then the full collective.
            if !self.trim_single_rotor(0.) {
                self.throttle = 1.;
                self.lift_controls.collective = 1.;
                self.lift_controls.collective_actual = 1.;
            }
            self.velocity = self.velocity.map(|_| 0.);
            self.speed = 0.;
        }
        self.add_wind_to_start(wind);
        self.speed > 0.
    }

    /// The velocity of a trim is air-relative; the start's is ground-relative.
    fn add_wind_to_start(&mut self, wind: [f64; 3]) {
        for (v, w) in self.velocity.iter_mut().zip(wind) {
            *v += w;
        }
        self.vertical_speed = self.velocity[1];
    }

    /// A vectoring jet in wingborne level flight: nozzles at 0, lift engines
    /// off, the throttle that balances drag and the pitch that makes the
    /// wing carry the weight.
    fn start_jet(&mut self, wind: [f64; 3]) -> bool {
        let Some(airspeed) = self.start_airspeed() else {
            return false;
        };
        let controls = &mut self.lift_controls;
        controls.vector_pitch = 0.;
        controls.vector_pitch_actual = 0.;
        controls.vector_yaw = 0.;
        controls.vector_yaw_actual = 0.;
        controls.body_rates = [0.; 3];
        controls.drive.lift_engine_spool = 0.;
        controls.drive.engine_output[1] = 0.;
        self.roll_rate = 0.;
        self.pitch_rate = 0.;
        self.bank = 0.;
        let forward = Basis::new(self.yaw, 0., 0.).forward;
        self.velocity = forward.map(|f| f * airspeed);
        self.speed = airspeed;
        self.vertical_speed = 0.;
        self.pitch = JET_PITCH_GUESS;
        self.throttle = self.throttle.clamp(0.05, 1.);
        // Newton on [throttle, pitch, pitch trim] for no acceleration along
        // the path or across it and no pitching, from one-tick probes of the
        // real force law. The wing's neutral-stick command is a G the thrust
        // and the intakes' moment do not quite leave in balance, so the jet
        // also gets a small pitch trim, as any aircraft does.
        let mut x = [self.throttle, self.pitch, 0.];
        let mut residual = self.jet_residual(x);
        for _ in 0..JET_TRIM_ITERATIONS {
            if residual.iter().all(|r| r.abs() < JET_TRIM_TOLERANCE_G) {
                break;
            }
            let mut jacobian = [[0.; 3]; 3];
            for (j, step) in [JET_THROTTLE_STEP, JET_PITCH_STEP, JET_TRIM_STEP]
                .into_iter()
                .enumerate()
            {
                let mut moved = x;
                moved[j] += step;
                let r = self.jet_residual(moved);
                for i in 0..3 {
                    jacobian[i][j] = (r[i] - residual[i]) / step;
                }
            }
            let Some(delta) = solve3(jacobian, residual.map(|r| -r)) else {
                break;
            };
            x[0] = (x[0] + delta[0].clamp(-0.3, 0.3)).clamp(0., 1.);
            x[1] = (x[1] + delta[1].clamp(-0.05, 0.05)).clamp(-0.3, 0.6);
            x[2] = (x[2] + delta[2].clamp(-0.1, 0.1)).clamp(-1., 1.);
            residual = self.jet_residual(x);
        }
        self.throttle = x[0];
        self.pitch = x[1];
        self.lift_controls.aids.trim = [x[2], 0., 0.];
        self.set_jet_engine(x[0]);
        self.add_wind_to_start(wind);
        true
    }

    /// The engine at the output the spool holds for `throttle` now.
    fn set_jet_engine(&mut self, throttle: f64) {
        let military = self.model().configuration().propulsion.military_thrust_lbf;
        let lapse = self
            .model()
            .response(Conditions {
                altitude_msl_ft: self.position[1],
                tas_fps: self.speed,
                load_factor: self.g,
            })
            .thrust_lapse;
        let output = if self.engine {
            military * throttle * lapse * self.systems.power_available()
        } else {
            0.
        };
        self.lift_controls.drive.engine_output[0] = output;
        self.lift_controls.thrust_lbf = output;
    }

    /// The acceleration, in G, along the flight path and across it (up
    /// positive, gravity included) and the pitch rate (rad/s, scaled) of
    /// this start at `[throttle, pitch, pitch trim]`, after one tick of the
    /// real force law.
    fn jet_residual(&self, [throttle, pitch, trim]: [f64; 3]) -> [f64; 3] {
        let mut probe = self.clone();
        probe.throttle = throttle;
        probe.pitch = pitch;
        probe.lift_controls.aids.trim = [trim, 0., 0.];
        probe.set_jet_engine(throttle);
        let before = probe.velocity;
        let along = unit(before);
        let floor = probe.position[1] - PROBE_GROUND_DEPTH_FT;
        probe.step_surface(&PilotInput::default(), |_, _| Surface::runway(floor));
        let dv: [f64; 3] = std::array::from_fn(|i| probe.velocity[i] - before[i]);
        let g = super::body::GRAVITY * DT;
        // The pitch rate one tick of the moment leaves, scaled to G-like units.
        [
            dot(dv, along) / g,
            dv[1] / g,
            probe.lift_controls.body_rates[1] * PITCH_RATE_SCALE,
        ]
    }

    /// The old law's hover for the aircraft that have no trim of their own
    /// yet: full engine, the collective that holds the weight.
    fn hover_on_the_old_law(&mut self, lift: &PoweredLift) {
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

    /// The jets and helicopters in a hover, at rest in the air: a jet with
    /// its nozzles vertical and its engines spooled to just hold its weight,
    /// a helicopter in the trim of [`State::trim_single_rotor`]. For tools
    /// and tests that want the hover display; no start uses it. False for
    /// the aircraft with no hover trim yet.
    pub fn trim_hover(&mut self) -> bool {
        let Some(lift) = self.model().powered_lift() else {
            return false;
        };
        if let Some(jet) = lift.jet {
            let c = self.model().configuration();
            let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
            let lapse = (-self.position[1].max(0.) / c.tuning.thrust_lapse_feet).exp();
            let height = self.position[1] - c.equipment.ground_clearance_ft;
            let vertical = 90. / jet.nozzle_range_degrees;
            let nozzle = std::f64::consts::FRAC_PI_2;
            let lift_engines = jet.lift_engines.map_or(0., |e| e.thrust_lbf);
            let military = c.propulsion.military_thrust_lbf;
            let throttle = weight
                / ((military * super::jet::nozzle_efficiency(&jet, nozzle) + lift_engines)
                    * lapse
                    * (1. - super::jet::suck_down(&jet, height, nozzle)));
            self.throttle = throttle;
            self.velocity = [0.; 3];
            self.speed = 0.;
            self.pitch = 0.;
            self.bank = 0.;
            self.lift_controls.vector_pitch = vertical;
            self.lift_controls.vector_pitch_actual = vertical;
            self.lift_controls.drive.engine_output =
                [military * throttle * lapse, lift_engines * throttle * lapse];
            self.lift_controls.drive.lift_engine_spool = f64::from(jet.lift_engines.is_some());
            return true;
        }
        let c = self.model().configuration();
        if SingleRotor::new(&lift, c).is_none() {
            return false;
        }
        // At rest in the air: a hover keeps the velocity it has.
        let (velocity, speed) = (self.velocity, self.speed);
        self.velocity = [0.; 3];
        self.speed = 0.;
        if self.trim_single_rotor(0.) {
            return true;
        }
        (self.velocity, self.speed) = (velocity, speed);
        false
    }

    /// Sets a powered-lift aircraft up for a ground start, stationary on the
    /// wheels (the runway start calls it; the fixed-wing parts of the start
    /// are the same for every aircraft). Engines running at idle, no cold
    /// start; the rest is in the module documentation. Does nothing for an
    /// aircraft with no powered lift.
    pub(crate) fn start_on_ground(&mut self) {
        let Some(lift) = self.model().powered_lift() else {
            return;
        };
        let c = self.model().configuration();
        let heli = SingleRotor::new(&lift, c);
        let density = rotor::air_density(self.position[1]);
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let controls = &mut self.lift_controls;
        controls.body_rates = [0.; 3];
        controls.corridor_hold = None;
        if lift.jet.is_some() {
            // Nozzles aft, lift engines off, the engine at idle.
            controls.vector_pitch = 0.;
            controls.vector_pitch_actual = 0.;
            controls.vector_yaw = 0.;
            controls.vector_yaw_actual = 0.;
            controls.drive.engine_output = [0.; 2];
            controls.drive.lift_engine_spool = 0.;
            controls.thrust_lbf = 0.;
            return;
        }
        // A rotorcraft: the rotor at its governed speed with the collective
        // down. The throttle keys drive the collective, so the engines'
        // own throttle is set here.
        controls.collective = 0.;
        controls.collective_actual = 0.;
        controls.thrust_lbf = 0.;
        controls.drive.rotor_speed = 1.;
        controls.drive.rotor_speed_reference = 1.;
        controls.drive.lift_engine_spool = 0.;
        controls.rotors = [Rotor::default(); 2];
        if let Some(heli) = heli {
            // The power the flat-pitch rotor needs at full speed: the
            // governor holds it from the first tick.
            let instant = Instant {
                basis,
                air_velocity: [0.; 3],
                density,
                rotor_speed: 1.,
                rotor: Rotor::default(),
                engine_power: 0.,
                collective: 0.,
                controls: [0.; 3],
                hub_height_agl_ft: None,
                seconds: 0.,
                hazards: Hazards::ALL,
                drag_factor: 1.,
                lift_factor: 1.,
            };
            controls.drive.engine_output[0] = heli.loads(&instant).power.max(0.);
        }
        // TODO(P3): the CH-47's tandem drive idles the same way.
        // TODO(P5): the V-22's nacelles go to V22_GROUND_NACELLE_DEGREES of
        // the 97.5-degree travel, `conversion` and `conversion_actual`.
        self.throttle = 1.;
    }
}

/// Solves a 3 by 3 system by Cramer's rule, or none when singular.
fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    if d.abs() < 1e-14 {
        return None;
    }
    Some(std::array::from_fn(|k| {
        let mut m = a;
        for row in 0..3 {
            m[row][k] = b[row];
        }
        det(m) / d
    }))
}

#[cfg(test)]
mod tests {
    //! Slice P7's acceptance S1 (airborne starts) and S2 (ground starts) on
    //! synthetic aircraft with the PT numbers of design section 8.
    use super::super::{helicopter::tests::pt_aircraft, jet::tests::fixture};
    use super::*;
    use tore_formats::aircraft::{Aircraft, AircraftId};
    use tore_input::StabilityLevel;

    const KT: f64 = 1.687_81;
    const FLAT: fn(f64, f64) -> Surface = |_, _| Surface::runway(0.);

    fn aircraft(id: AircraftId) -> Aircraft {
        match id {
            AircraftId::Ah64 | AircraftId::Mi24 => pt_aircraft(id),
            _ => fixture(id),
        }
    }

    /// A start of `id` at `altitude`, heading north on the hybrid model.
    fn start(id: AircraftId, altitude: f64) -> State {
        let mut s = State::new(&aircraft(id), [0., altitude, 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.yaw = 0.;
        s
    }

    const FINISHED: [AircraftId; 4] = [
        AircraftId::Ah64,
        AircraftId::Mi24,
        AircraftId::Av8,
        AircraftId::Yak141,
    ];

    /// The speed rule of the fixed-wing variety aircraft at `altitude`.
    fn rule(s: &State) -> f64 {
        variety_start_speed(s.model(), s.position[1]).unwrap()
    }

    #[test]
    fn s1_airborne_starts_fly_hands_off_in_trimmed_forward_flight() {
        for id in FINISHED {
            for altitude in [3_000., 6_000.] {
                for level in [StabilityLevel::Damper, StabilityLevel::Off] {
                    let mut s = start(id, altitude);
                    s.lift_controls.aids.stability = level;
                    let speed = rule(&s);
                    assert!(speed > 60. * KT, "{id:?} starts at {speed} ft/s");
                    assert!(s.start_airborne([0.; 3]), "{id:?} trims");
                    assert!((s.speed - speed).abs() < 1e-9, "{id:?} keeps the rule");
                    assert_eq!(s.lift_controls.vector_pitch_actual, 0., "{id:?} nozzles");
                    let (height, airspeed) = (s.position[1], s.speed);
                    let (mut low, mut high) = (height, height);
                    let mut worst_speed = 0_f64;
                    for _ in 0..1200 {
                        s.step_surface(&PilotInput::default(), FLAT);
                        low = low.min(s.position[1]);
                        high = high.max(s.position[1]);
                        worst_speed = worst_speed.max((s.speed - airspeed).abs());
                    }
                    assert!(
                        high - low < 10. && (s.position[1] - height).abs() < 10.,
                        "{id:?} {level:?} at {altitude}: height {low:.1} to {high:.1}"
                    );
                    assert!(
                        worst_speed < 2. * KT,
                        "{id:?} {level:?} at {altitude}: speed moved {:.2} kt",
                        worst_speed / KT
                    );
                    assert!(!s.crashed);
                }
            }
        }
    }

    #[test]
    fn s1_a_start_in_a_wind_is_ground_relative_and_flies_the_same() {
        let wind = [12., 0., -8.];
        for id in FINISHED {
            let mut s = start(id, 5_000.);
            let air = rule(&s);
            s.start_airborne(wind);
            let along = Basis::new(s.yaw, 0., 0.).forward;
            for i in 0..3 {
                assert!(
                    (s.velocity[i] - (along[i] * air + wind[i])).abs() < 1e-6,
                    "{id:?} axis {i}"
                );
            }
            assert!((s.speed - air).abs() < 1e-9, "{id:?} airspeed");
        }
    }

    #[test]
    fn s1_the_start_uses_the_final_mass_and_altitude() {
        for id in FINISHED {
            let mut light = start(id, 5_000.);
            let mut loaded = start(id, 5_000.);
            loaded.fuel = 800.;
            loaded.set_payload(2_500.).unwrap();
            light.start_airborne([0.; 3]);
            loaded.start_airborne([0.; 3]);
            assert_ne!(
                light.lift_controls, loaded.lift_controls,
                "{id:?} trims for its weight"
            );
            let height = loaded.position[1];
            for _ in 0..1200 {
                loaded.step_surface(&PilotInput::default(), FLAT);
            }
            assert!(
                (loaded.position[1] - height).abs() < 10.,
                "{id:?} drifted {:.1} ft",
                loaded.position[1] - height
            );
        }
    }

    #[test]
    fn an_aircraft_that_has_flown_or_stands_on_the_ground_is_not_retrimmed() {
        let mut s = start(AircraftId::Ah64, 5_000.);
        s.start_airborne([0.; 3]);
        let trimmed = s.clone();
        s.step_surface(&PilotInput::default(), FLAT);
        let flown = s.clone();
        assert!(!s.start_airborne([0.; 3]));
        assert_eq!(s, flown);
        let mut ground = start(AircraftId::Ah64, 0.);
        ground.start_on_runway([0.; 3], 0.).unwrap();
        let parked = ground.clone();
        assert!(!ground.start_airborne([0.; 3]));
        assert_eq!(ground, parked);
        assert_ne!(trimmed, flown);
    }

    #[test]
    fn the_aircraft_without_a_trim_yet_keep_the_old_hover() {
        for id in [AircraftId::Ch47, AircraftId::V22] {
            let mut s = State::new(
                &crate::models::variety::tests::synthetic(id),
                [0., 5_000., 0.],
            )
            .unwrap();
            s.enable_research(1).unwrap();
            assert_eq!(s.speed, 0., "{id:?} starts at rest");
            assert!(!s.start_airborne([0.; 3]));
            assert_eq!(s.throttle, 1.);
            assert!(s.lift_controls.collective > 0.);
        }
    }

    #[test]
    fn s2_ground_starts_match_the_fixed_wing_ones() {
        for id in FINISHED {
            let mut s = start(id, 0.);
            s.start_on_runway([0.; 3], 0.).unwrap();
            assert!(s.engine && s.fuel > 0., "{id:?} engine running");
            assert!(s.gear_down && s.flaps_down && s.brake_out, "{id:?} devices");
            assert_eq!((s.speed, s.velocity), (0., [0.; 3]), "{id:?} stationary");
            assert!(s.lift_controls.vector_pitch_actual == 0.);
            assert_eq!(s.nozzle_degrees(), 0.);
            assert_eq!(s.lift_controls.drive.lift_engine_spool, 0.);
            let rotorcraft = s.model().powered_lift().unwrap().jet.is_none();
            if rotorcraft {
                assert_eq!(s.lift_controls.collective_actual, 0., "{id:?} collective");
                assert_eq!(s.lift_controls.drive.rotor_speed, 1., "{id:?} rotor");
                assert_eq!(s.throttle, 1., "{id:?} engine at 100 percent");
            } else {
                assert_eq!(s.throttle, 0., "{id:?} idling");
            }
            // Ten seconds on the ground, hands off: still parked, the rotor
            // still at its governed speed, the engines running.
            for _ in 0..1200 {
                s.step_surface(&PilotInput::default(), FLAT);
            }
            assert!(!s.crashed, "{id:?}");
            assert!(s.speed < 1., "{id:?} still parked at {} ft/s", s.speed);
            assert!((s.position[1]).abs() < 30., "{id:?} on the ground");
            if rotorcraft {
                let nr = s.lift_controls.drive.rotor_speed;
                assert!((0.97..1.03).contains(&nr), "{id:?} rotor speed {nr}");
                assert!(s.lift_controls.warnings.low_rotor == 0);
            }
        }
    }

    #[test]
    fn the_ground_start_idles_a_rotor_without_sagging_it() {
        for id in [AircraftId::Ah64, AircraftId::Mi24] {
            let mut s = start(id, 0.);
            s.start_on_runway([0.; 3], 0.).unwrap();
            let mut low = 1_f64;
            let mut high = 1_f64;
            for _ in 0..600 {
                s.step_surface(&PilotInput::default(), FLAT);
                low = low.min(s.lift_controls.drive.rotor_speed);
                high = high.max(s.lift_controls.drive.rotor_speed);
            }
            assert!(low > 0.985 && high < 1.015, "{id:?} {low} {high}");
        }
    }

    #[test]
    fn the_v22_ground_nacelle_hook_is_the_helicopter_preset() {
        assert_eq!(V22_GROUND_NACELLE_DEGREES, 87.);
    }
}
