//! The AI's control of the helicopters and the V-22 (slice H1 of the
//! helicopter and VTOL AI pass, docs/spec/ai.md "Rotorcraft control"). The
//! fixed-wing [`super::steering_adapter`] flies a load factor through the
//! stick and a speed through the throttle; on a rotorcraft the stick tilts
//! the rotor disk, the throttle is a governed engine's power lever and lift
//! comes from the collective, which that adapter never moved. This adapter
//! flies the same steer requests (heading, flight path, speed, bank) and
//! the hover requests the airfield sequences use, through the player
//! autopilot's own laws ([`crate::autopilot::rotor_law`]) with targets it
//! commands:
//!
//! - **Engines** governed at full: the throttle is held at 1.
//! - **Forward flight** ([`Regime::Forward`]): climb on the collective,
//!   speed on the cyclic, coordinated pedals, a bank toward the heading.
//! - **Pointing** ([`Regime::Pointing`]): a steep flight path (a pop-up's
//!   pull, a rocket run's dive) is flown on the pitch, as a pilot points
//!   the nose, and the speed on the collective. The rockets leave along the
//!   nose, so the nose must follow the path the run asks for.
//! - **Hover** ([`Regime::Hover`]): hover hold's cascade to a commanded
//!   point, height and heading, with commanded climb, sink and closing speed
//!   limits, and an optional slow-flight velocity on top. Asked to hover
//!   while faster than 40 kt, it first slows on the forward law toward the
//!   point.
//! - **The V-22** turns its nacelles on a schedule against indicated
//!   airspeed inside the conversion corridor ([`nacelle_schedule`]); with
//!   the nacelles below 75 degrees it flies the climb on the pitch and the
//!   speed on its power lever (the collective), as the player's A modes do.
//!
//! Its memory (the laws' integrators and the regime) is the actor's own and
//! is checkpointed with it. Every number here is fitted (agent decisions,
//! 2026-10-12) unless its line says otherwise.
use super::controller::MotionIntent;
use super::motion::Bank;
use crate::autopilot::rotor_law::{self, ForwardTarget, HoverTarget};
use crate::autopilot::{HOVER_NACELLE_DEGREES, HOVER_SPEED_KT, Loops};
use crate::flight::{PilotInput, State};
use crate::models::FlightModel;
use crate::models::variety::LiftKind;

/// ft/s per knot.
const KT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
const GRAVITY: f64 = 32.174;

/// The climb and sink a steer request may command in forward flight, ft/s
/// (fitted: 3,000 ft/min, three times the player A modes', so the
/// helicopter defence's 10 degree path is flown at 150 kt).
pub const FORWARD_CLIMB_FPS: f64 = 3_000. / 60.;
/// The coupled law is asked for at most this speed change at a time, ft/s
/// (about 6 kt: a gentle nose-down, not the pitch limit at once).
pub const SPEED_STEP_FPS: f64 = 10.;
/// Sinking this much faster than the descent asked for (or than level when
/// a climb is asked), ft/s, starts slowing
/// it and easing the bank; at SINK_FULL_RELIEF_FPS it slows at the full
/// step.
pub const SINK_TOLERANCE_FPS: f64 = 2.;
pub const SINK_FULL_RELIEF_FPS: f64 = 5.;
/// Height before speed applies below this height over the ground, ft.
pub const RELIEF_AGL_FT: f64 = 1_000.;
/// A collective demand at or above this is out of travel.
pub const COLLECTIVE_SATURATED: f64 = 0.99;
/// A rotor speed below this share of 100 percent is out of power.
pub const ROTOR_DROOP: f64 = 0.95;
/// The share of the bank eased at the full step.
pub const BANK_RELIEF_SHARE: f64 = 0.5;
/// Wingborne, the climb error is handed to the law this many times over
/// on top of the climb.
pub const WINGBORNE_CLIMB_BOOST: f64 = 4.;
/// The B44 terrain floor's minimum height for a helicopter on this
/// adapter, ft (the fighters' is 300): below the helicopter defence's and
/// the stand-off attack's 200 ft masking height. The V-22 keeps the
/// fighters' 300 ft (slice HF1): it never masks, and at 250 kt in
/// aeroplane mode it needs the fighters' margin over rising ground.
pub const MINIMUM_ALTITUDE_FT: f64 = 100.;
/// Wingborne (the V-22's nacelles below the coupled law's 72 degrees), the
/// pitch attitude the law may command for the path, degrees (fitted, slice
/// HF1): enough for the terrain floor's 15 degree paths at cruise.
pub const WINGBORNE_PITCH_DEG: f64 = 25.;
/// The speed the adapter holds may stand this far over the maximum while
/// it settles, kt: the helicopter defence's "no descent past the maximum"
/// allows for it.
pub const SPEED_HOLD_MARGIN_KT: f64 = 10.;
/// The pitch attitude the forward law may command, degrees (the player A
/// modes' 15).
pub const FORWARD_PITCH_DEG: f64 = 15.;
/// The steepest bank the adapter flies, degrees, or the aircraft's own
/// maximum bank if less.
pub const MAXIMUM_BANK_DEG: f64 = 60.;
/// A heading error is closed over this many seconds by the bank (the
/// fixed-wing adapter's one second).
pub const BANK_LEAD_SECONDS: f64 = 1.;
/// A requested flight path steeper than this either way, that also asks for
/// more climb or sink than [`FORWARD_CLIMB_FPS`], is flown pointing the
/// nose, degrees; the forward law takes back below the lower figure.
pub const POINTING_ENTER_DEG: f64 = 12.;
pub const POINTING_LEAVE_DEG: f64 = 8.;
/// Pointing: the pitch attitude it may command, degrees.
pub const POINTING_PITCH_DEG: f64 = 30.;
/// The speed loop on the collective (pointing, and the V-22 with its
/// nacelles down): collective per ft/s of speed error, and its integrator
/// per foot of error.
pub const SPEED_COLLECTIVE_GAIN: f64 = 0.01;
pub const SPEED_COLLECTIVE_INTEGRAL: f64 = 0.002;
/// The least collective the speed loop leaves a helicopter, so pointing
/// never unloads the rotor.
pub const POINTING_MINIMUM_COLLECTIVE: f64 = 0.2;
/// The least collective a helicopter's AI flies in forward flight (fitted,
/// slice CH47): the rotor is never unloaded. At the stop (a long descent
/// asked at 120 knots) the rotor has no control power left, the nose swings
/// away from the track, the speed along the nose reads near zero and the
/// speed loop pitches the nose down after it into a dive. A tenth of the
/// travel still flies the CH-47 down at 1,000 ft/min at 120 knots.
pub const FORWARD_MINIMUM_COLLECTIVE: f64 = 0.1;
/// The hover cascade takes over below this ground speed, kt (hover hold's
/// 40 kt engage band).
pub const HOVER_ENTRY_KT: f64 = HOVER_SPEED_KT;
/// The hover cascade's pitch and bank limit, degrees (hover hold's 15 is
/// slow to move a point hundreds of feet).
pub const HOVER_TILT_DEG: f64 = 20.;
/// Slowing toward a hover point on the forward law: the deceleration
/// planned, ft/s².
pub const APPROACH_DECELERATION_FPS2: f64 = 4.;

/// The V-22's nacelle schedule: (KCAS, nacelle degrees), the nacelles
/// coming down as the speed builds and up as it falls. The nacelles stay at
/// 75 degrees or more, where the collective flies the climb, until the
/// wing's published 110 kt stall speed; each point lies inside the
/// conversion corridor (lift.rs `V22_CORRIDOR`) with at least 20 kt to
/// either edge; the helicopter preset is the published 87 degrees.
pub const NACELLE_SCHEDULE: [(f64, f64); 8] = [
    (40., 87.),
    (80., 80.),
    (110., 75.),
    (125., 60.),
    (135., 45.),
    (145., 30.),
    (155., 15.),
    (165., 0.),
];

/// The V-22 leaves the coupled law (climb on the collective) when its
/// nacelles fall below this, degrees; it enters at hover hold's 75.
pub const COUPLED_LEAVE_NACELLE_DEG: f64 = 72.;

/// The V-22 keeps this far inside its corridor's speed limits at the
/// nacelles' present angle, and moves its nacelles no further aft within
/// this of the aft lock speed, kt.
pub const CORRIDOR_MARGIN_KT: f64 = 10.;

/// Overspeed brakes the V-22 by scheduling the nacelles as if this many
/// knots slower per knot too fast.
pub const NACELLE_BRAKE_SHARE: f64 = 2.;

/// A hover or slow-flight request (the airfield sequences' vertical
/// takeoff, air taxi, approach and landing; slice H2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoverGuidance {
    /// The ground point to hold, world X/Z ft.
    pub point: [f64; 2],
    /// A ground velocity flown on top, world X/Z ft/s (air taxi); zero to
    /// hold the point.
    pub velocity_fps: [f64; 2],
    /// Wheel height above the ground under the aircraft, ft. A height
    /// below zero with a small sink limit sets the aircraft down at that
    /// sink.
    pub height_agl_ft: f64,
    /// The heading held, degrees.
    pub heading_deg: f64,
    /// The climb and sink the height loop may command, ft/s, both positive
    /// (retail's vertical climb is 25 ft/s, H2).
    pub max_climb_fps: f64,
    pub max_descent_fps: f64,
    /// The ground speed it may close on the point at, ft/s.
    pub max_speed_fps: f64,
}

/// What the adapter flies this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotorRequest<'a> {
    /// A steer request of the fighter families: heading, flight path,
    /// speed (Maximum resolved), bank. `terrain_pitch_floor_deg` is the
    /// B44 floor on the flight path.
    Steer {
        intent: &'a MotionIntent,
        terrain_pitch_floor_deg: Option<f64>,
    },
    Hover(HoverGuidance),
}

/// Which law the adapter flies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Regime {
    #[default]
    Forward,
    Pointing,
    Hover,
}

/// What one tick commanded, for the trace and the drivers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorCommand {
    pub regime: Regime,
    /// Heading, flight path and bank commanded, degrees.
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
    pub bank_deg: f64,
    /// Ground speed along the heading commanded, ft/s.
    pub speed_fps: f64,
    /// Climb commanded, ft/s.
    pub climb_fps: f64,
    /// The V-22's nacelle demand, degrees.
    pub nacelle_deg: Option<f64>,
}

/// One AI rotorcraft's control memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RotorAdapter {
    pub(super) loops: Loops,
    pub(super) regime: Regime,
}

/// Whether the AI flies `s` through this adapter: the helicopters and the
/// V-22 on the hybrid flight model.
pub fn flies(s: &State) -> bool {
    s.research.is_some()
        && s.native.is_none()
        && s.model()
            .powered_lift()
            .is_some_and(|lift| matches!(lift.kind, LiftKind::Helicopter | LiftKind::Tiltrotor))
}

/// The B44 terrain floor's minimum height for `s` on this adapter, ft:
/// [`MINIMUM_ALTITUDE_FT`] for a helicopter, the fighters' `fighters` for
/// the V-22.
pub fn minimum_altitude_ft(s: &State, fighters: f64) -> f64 {
    if s.model()
        .powered_lift()
        .is_some_and(|lift| lift.kind == LiftKind::Tiltrotor)
    {
        fighters
    } else {
        MINIMUM_ALTITUDE_FT
    }
}

/// The scheduled nacelle angle at `kcas`, degrees.
pub fn nacelle_schedule(kcas: f64) -> f64 {
    let first = NACELLE_SCHEDULE[0];
    if kcas <= first.0 {
        return first.1;
    }
    for pair in NACELLE_SCHEDULE.windows(2) {
        let [(k0, n0), (k1, n1)] = [pair[0], pair[1]];
        if kcas <= k1 {
            return n0 + (n1 - n0) * (kcas - k0) / (k1 - k0);
        }
    }
    NACELLE_SCHEDULE[NACELLE_SCHEDULE.len() - 1].1
}

fn wrap_deg(degrees: f64) -> f64 {
    (degrees + 180.).rem_euclid(360.) - 180.
}

impl RotorAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn regime(&self) -> Regime {
        self.regime
    }

    /// Forget the laws' memory: the next request starts from the aircraft
    /// as it is (slice H2: a liftoff from the ground, after the collective
    /// was held down on the wheels).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Change law: the attitude integrators (the stick trim) are kept, the
    /// outer loops start again from the aircraft's state.
    fn enter(&mut self, regime: Regime) {
        if regime != self.regime {
            self.loops = Loops {
                stick: self.loops.stick,
                ..Loops::default()
            };
            self.regime = regime;
        }
    }

    /// The controls for this tick: `ground` is the terrain height under the
    /// aircraft, `wind` the world wind there, `maximum_bank_deg` the
    /// aircraft's own maximum bank.
    pub fn controls(
        &mut self,
        s: &State,
        request: RotorRequest<'_>,
        ground: f64,
        wind: [f64; 3],
        maximum_bank_deg: f64,
    ) -> (PilotInput, RotorCommand) {
        let tiltrotor = s
            .model()
            .powered_lift()
            .is_some_and(|lift| lift.kind == LiftKind::Tiltrotor);
        // The V-22 flies the collective from 75 degrees of nacelle up, and
        // keeps doing so down to 72, so it does not change law each tick
        // while its nacelles stand near 75.
        let nacelles_up = !tiltrotor
            || s.nacelle_degrees()
                >= if self.loops.coupled {
                    COUPLED_LEAVE_NACELLE_DEG
                } else {
                    HOVER_NACELLE_DEGREES - 1e-9
                };
        let ground_speed = s.velocity[0].hypot(s.velocity[2]);
        let bank_limit = maximum_bank_deg.clamp(0., MAXIMUM_BANK_DEG);
        let mut input = PilotInput {
            throttle: Some(1.),
            ..PilotInput::default()
        };
        let command = match request {
            RotorRequest::Steer {
                intent,
                terrain_pitch_floor_deg,
            } => {
                let path = intent
                    .flight_path_pitch_deg
                    .max(terrain_pitch_floor_deg.unwrap_or(-90.))
                    .clamp(-90., 90.);
                // Steep, and more climb or sink than the collective flies.
                let vertical = s.speed.max(HOVER_SPEED_KT * KT) * path.to_radians().sin().abs();
                let pointing = !tiltrotor
                    && if self.regime == Regime::Pointing {
                        path.abs() > POINTING_LEAVE_DEG
                    } else {
                        path.abs() > POINTING_ENTER_DEG && vertical > FORWARD_CLIMB_FPS
                    };
                self.enter(if pointing {
                    Regime::Pointing
                } else {
                    Regime::Forward
                });
                let heading_error = wrap_deg(intent.heading_deg - s.yaw.to_degrees());
                let bank = match intent.bank {
                    Bank::Explicit(deg) => f64::from(deg),
                    Bank::Unconstrained => {
                        let rate = heading_error.to_radians() / BANK_LEAD_SECONDS;
                        (ground_speed.max(HOVER_SPEED_KT * KT) * rate / GRAVITY)
                            .atan()
                            .to_degrees()
                    }
                }
                .clamp(-bank_limit, bank_limit);
                let (sin, cos) = s.yaw.sin_cos();
                let wind_along = wind[0] * sin + wind[2] * cos;
                let speed = intent.speed.0 + wind_along;
                self.forward(
                    s,
                    &mut input,
                    wind,
                    intent.heading_deg,
                    path,
                    bank,
                    speed,
                    pointing,
                    nacelles_up,
                    tiltrotor,
                    s.position[1] - ground,
                )
            }
            RotorRequest::Hover(guidance) => {
                let hovering = self.regime == Regime::Hover
                    || (ground_speed < HOVER_ENTRY_KT * KT && nacelles_up);
                if hovering {
                    self.enter(Regime::Hover);
                    self.hover(s, &mut input, ground, &guidance, tiltrotor)
                } else {
                    // Too fast to hover: slow on the forward law toward the
                    // point, holding the height asked for.
                    self.enter(Regime::Forward);
                    let offset = [
                        guidance.point[0] - s.position[0],
                        guidance.point[1] - s.position[2],
                    ];
                    let (sin, cos) = s.yaw.sin_cos();
                    let ahead = offset[0] * sin + offset[1] * cos;
                    let heading = if ahead > 0. {
                        offset[0].atan2(offset[1]).to_degrees()
                    } else {
                        s.yaw.to_degrees()
                    };
                    let speed = (2. * APPROACH_DECELERATION_FPS2 * ahead.max(0.)).sqrt();
                    let clearance = s.model().configuration().equipment.ground_clearance_ft;
                    let height = s.position[1] - ground - clearance;
                    let climb = (rotor_law::HEIGHT_GAIN * (guidance.height_agl_ft - height))
                        .clamp(-guidance.max_descent_fps, guidance.max_climb_fps);
                    let path = climb
                        .atan2(ground_speed.max(HOVER_SPEED_KT * KT))
                        .to_degrees();
                    let rate = wrap_deg(heading - s.yaw.to_degrees()).to_radians();
                    let bank = (ground_speed.max(HOVER_SPEED_KT * KT) * rate / GRAVITY)
                        .atan()
                        .to_degrees()
                        .clamp(-bank_limit, bank_limit);
                    self.forward(
                        s,
                        &mut input,
                        wind,
                        heading,
                        path,
                        bank,
                        speed,
                        false,
                        nacelles_up,
                        tiltrotor,
                        s.position[1] - ground,
                    )
                }
            }
        };
        (input.bounded(), command)
    }

    #[allow(clippy::too_many_arguments)]
    fn forward(
        &mut self,
        s: &State,
        input: &mut PilotInput,
        wind: [f64; 3],
        heading_deg: f64,
        path_deg: f64,
        bank_deg: f64,
        speed: f64,
        pointing: bool,
        nacelles_up: bool,
        tiltrotor: bool,
        agl: f64,
    ) -> RotorCommand {
        let coupled = !pointing && nacelles_up;
        let requested = speed;
        // The V-22 paces its speed to the corridor at the nacelles' present
        // angle, so the conversion never runs into its protection.
        let corridor = if tiltrotor {
            s.conversion_corridor()
        } else {
            None
        };
        let calibration = match corridor {
            Some(c) if s.speed > 1. => c.kcas / (s.speed / KT),
            _ => 1.,
        };
        let speed = corridor.map_or(speed, |c| {
            let [low, high] = c.limits_kcas;
            let fastest = (high - CORRIDOR_MARGIN_KT) * KT / calibration;
            let slowest = if low > 0. {
                (low + CORRIDOR_MARGIN_KT) * KT / calibration
            } else {
                0.
            };
            speed.min(fastest).max(slowest.min(fastest))
        });
        let airspeed = s.speed.max(HOVER_SPEED_KT * KT);
        // Wingborne (the V-22 with its nacelles down) the path is flown on
        // the pitch as an aeroplane's: its climb is not held to the
        // collective's FORWARD_CLIMB_FPS, which at 240 kt is a 7 degree
        // path, flatter than the terrain floor asks over rising ground
        // (slice HF1: the V-22 flew into a hillside climbing at 50 ft/s).
        let wingborne = !coupled && !pointing;
        let climb = if pointing || wingborne {
            airspeed * path_deg.to_radians().sin()
        } else {
            (airspeed * path_deg.to_radians().sin()).clamp(-FORWARD_CLIMB_FPS, FORWARD_CLIMB_FPS)
        };
        // Height before speed: the cyclic is asked for at most
        // SPEED_STEP_FPS of speed change at a time; sinking below the climb
        // asked for, it slows instead and eases the bank, as a pilot out of
        // collective does, so a hard turn at full speed near the ground does
        // not trade height for speed.
        let along = rotor_law::speed_along(s);
        // Out of collective or out of power (the rotor drooping), any sink
        // below the climb counts: the speed asked is more than the aircraft
        // can hold level.
        let tolerance = if s.lift_controls.collective >= COLLECTIVE_SATURATED
            || s.lift_controls.drive.rotor_speed < ROTOR_DROOP
        {
            0.
        } else {
            SINK_TOLERANCE_FPS
        };
        // Pointing flies the path, not a height: no relief there (a
        // pop-up's pull asks for far more climb than it has). Nor high up,
        // where a few feet do not matter and the strike's timing does.
        let relief = if pointing || agl > RELIEF_AGL_FT {
            0.
        } else {
            // Against level or the descent asked, not a climb: a climb
            // that comes slower than asked loses no height.
            ((climb.min(0.) - s.vertical_speed - tolerance)
                / (SINK_FULL_RELIEF_FPS - SINK_TOLERANCE_FPS))
                .clamp(0., 1.)
        };
        let held = if coupled {
            let step = (speed - along).clamp(-SPEED_STEP_FPS, SPEED_STEP_FPS);
            if relief > 0. {
                along + step.min(0.) - SPEED_STEP_FPS * relief
            } else {
                along + step
            }
        } else {
            speed
        };
        let bank_deg = bank_deg * (1. - BANK_RELIEF_SHARE * relief);
        // Wingborne (the V-22 with its nacelles down) the law flies the
        // climb on the pitch by the flight path error, which a few feet a
        // second of sink barely moves at 280 ft/s; the AI asks for the
        // climb plus a multiple of its error, as the player's A modes'
        // altitude term would.
        let law_climb = if wingborne {
            climb + WINGBORNE_CLIMB_BOOST * (climb - s.vertical_speed)
        } else {
            climb
        };
        let target = ForwardTarget {
            bank: bank_deg.to_radians(),
            climb: law_climb,
            speed: held,
            pitch_limit: if pointing {
                POINTING_PITCH_DEG
            } else if wingborne {
                WINGBORNE_PITCH_DEG
            } else {
                FORWARD_PITCH_DEG
            }
            .to_radians(),
            coupled,
        };
        if !coupled && (!self.loops.started || self.loops.coupled) {
            // The speed loop on the collective starts from the lever.
            self.loops.collective = s.lift_controls.collective;
        }
        rotor_law::forward(s, &target, wind, &mut self.loops, input);
        if coupled && !tiltrotor {
            // The climb loop wound the collective down to the stop to meet
            // a descent: hold it at the floor, and its integrator with it,
            // so the descent is flown as slowly as the floor allows and the
            // climb loop is not buried when the descent ends.
            self.loops.collective = self.loops.collective.max(FORWARD_MINIMUM_COLLECTIVE);
            input.collective = input.collective.map(|c| c.max(FORWARD_MINIMUM_COLLECTIVE));
        }
        if !coupled {
            // The speed on the collective (the V-22's power lever).
            let error = speed - rotor_law::speed_along(s);
            let floor = if pointing {
                POINTING_MINIMUM_COLLECTIVE
            } else {
                0.
            };
            let loops = &mut self.loops;
            loops.collective = (loops.collective
                + SPEED_COLLECTIVE_INTEGRAL * error * crate::flight::DT)
                .clamp(floor, 1.);
            input.collective =
                Some((loops.collective + SPEED_COLLECTIVE_GAIN * error).clamp(floor, 1.));
            input.collective_rate = 0.;
        }
        let nacelle_deg = tiltrotor.then(|| {
            let kcas = corridor.map_or(0., |c| c.kcas);
            // The speed wanted, true, as indicated at this height.
            let wanted_kcas = (requested / KT * calibration).max(HOVER_ENTRY_KT);
            // Too fast: the nacelles come up to brake, as a pilot slows a
            // tiltrotor; never faster than the corridor protection allows.
            let over = (kcas - wanted_kcas).max(0.);
            let scheduled = nacelle_schedule(kcas.min(wanted_kcas) - NACELLE_BRAKE_SHARE * over);
            // Inside the angles the corridor allows at this airspeed, so
            // its protection never has to hold the nacelles.
            let locked = s
                .model()
                .powered_lift()
                .and_then(|lift| lift.tiltrotor)
                .is_some_and(|t| kcas > t.aft_lock_kcas - CORRIDOR_MARGIN_KT);
            corridor.map_or(scheduled, |c| {
                let [forward, aft] = c.allowed_degrees;
                let aft = if locked {
                    // No aft motion near the aft lock speed.
                    aft.min(c.nacelle_degrees)
                } else {
                    aft
                };
                scheduled.clamp(forward.min(aft), aft.max(forward))
            })
        });
        self.set_nacelles(s, input, nacelle_deg);
        RotorCommand {
            regime: self.regime,
            heading_deg: heading_deg.rem_euclid(360.),
            flight_path_pitch_deg: path_deg,
            bank_deg,
            speed_fps: speed,
            climb_fps: climb,
            nacelle_deg,
        }
    }

    fn hover(
        &mut self,
        s: &State,
        input: &mut PilotInput,
        ground: f64,
        guidance: &HoverGuidance,
        tiltrotor: bool,
    ) -> RotorCommand {
        let moving = guidance.velocity_fps != [0.; 2];
        let target = HoverTarget {
            point: Some(guidance.point),
            velocity: moving.then_some(guidance.velocity_fps),
            height: guidance.height_agl_ft,
            heading: guidance.heading_deg.to_radians(),
            climb_limit: guidance.max_climb_fps.max(0.),
            descent_limit: guidance.max_descent_fps.max(0.),
            close_speed: guidance.max_speed_fps.max(0.),
            tilt_limit: HOVER_TILT_DEG.to_radians(),
        };
        rotor_law::hover(s, ground, &target, &mut self.loops, input);
        let nacelle_deg = tiltrotor.then_some(NACELLE_SCHEDULE[0].1);
        self.set_nacelles(s, input, nacelle_deg);
        RotorCommand {
            regime: self.regime,
            heading_deg: guidance.heading_deg.rem_euclid(360.),
            flight_path_pitch_deg: 0.,
            bank_deg: 0.,
            speed_fps: guidance.velocity_fps[0].hypot(guidance.velocity_fps[1]),
            climb_fps: s.vertical_speed,
            nacelle_deg,
        }
    }

    fn set_nacelles(&self, s: &State, input: &mut PilotInput, nacelle_deg: Option<f64>) {
        if let (Some(degrees), Some(tilt)) = (
            nacelle_deg,
            s.model().powered_lift().and_then(|lift| lift.tiltrotor),
        ) {
            input.conversion = Some(degrees / tilt.nacelle_range_degrees);
            input.conversion_rate = 0.;
        }
    }
}
