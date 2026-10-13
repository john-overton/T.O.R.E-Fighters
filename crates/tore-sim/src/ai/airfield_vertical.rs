//! The vertical takeoff and landing phases of an airfield sequence (slice
//! H2 of the helicopter and VTOL AI pass; docs/spec/ai-airfield.md
//! "Vertical takeoff and landing"), from the retail engine states 0xa..0xc
//! and 0x18 (docs/formats/ai.md):
//!
//! - **Takeoff.** A helicopter or the V-22 lifts off where it stands, on its
//!   turn with the runway free; a hovering jet first taxis to the takeoff
//!   spot. It holds 5 s ([`Phase::VerticalHold`]), climbs straight up at
//!   25 ft/s ([`Phase::VerticalClimb`]) to 500 ft (a helicopter, which then
//!   flies on) or 800 ft (a jet), and a jet accelerates to its minimum
//!   speed plus 73 ft/s ([`Phase::Transition`]) before it is wingborne.
//! - **Landing.** From the last of four approach gates it comes to a hover
//!   over the landing point ([`Phase::HoverApproach`]), comes straight down
//!   ([`Phase::VerticalDescent`]), sits 5 s ([`Phase::Settle`]) and then a
//!   helicopter air taxis to its parking slot and sets down there
//!   ([`Phase::AirTaxi`], then the descent again), where it shuts down; a
//!   jet taxis in on its wheels as after a runway landing.
//!
//! The hovers are flown by the rotor adapter (helicopters, V-22) and the
//! jet-lift law (AV-8, Yak-141) through [`Control::Hover`]. Every touchdown
//! is judged by the shared touchdown rule ([`crate::touchdown`]): close to
//! the ground the descent goes on only while that rule would accept a
//! touchdown, which the mission reports in [`Situation::touchdown_safe`].
use super::{
    AIR_TAXI_AGL_FT, AIR_TAXI_FPS, AIR_TAXI_REACHED_FT, AIR_TAXI_TURN_FT, CLIMB_REACHED_FT,
    Command, Control, DESCENT_RAMP_FPS2, DESCENT_SHARE_PER_S, HOVER_CREEP_FPS,
    HOVER_DECELERATION_FPS2, HOVER_NOZZLE_DEG, JET_CLIMB_AGL_FT, JET_DESCENT_FPS, Lift,
    OVER_POINT_FPS, OVER_POINT_FT, PARKING_SLOTS, Phase, ROTOR_CLIMB_AGL_FT, SET_DOWN_FT,
    SHORT_NOZZLES_DEG, Sequence, Situation, TOUCHDOWN_CHECK_FT, TOUCHDOWN_SINK_FPS,
    TRANSITION_CLIMB_DEG, TRANSITION_HANDOVER_SHARE, TRANSITION_MARGIN_FPS, TRANSITION_TIMEOUT_S,
    VERTICAL_CLIMB_FPS, VERTICAL_CLIMB_TIMEOUT_S, VERTICAL_GATE_SPEED_FPS, VERTICAL_HOLD_S,
    bearing_deg, fly_to, horizontal, horizontal_distance, seconds,
};
use crate::ai::rotor_adapter::HoverGuidance;

/// Fitted (agent decision, slice H2): a helicopter or the V-22 flies the
/// hover approach on its forward law, slowing at
/// [`ROTOR_APPROACH_DECELERATION_FPS2`] as it closes, until it is slower
/// than [`HOVER_HANDOVER_FPS`] (35 kt, under the rotor adapter's 40 kt
/// hover entry) or within [`HOVER_HANDOVER_FT`]; then the hover law holds
/// it over the point. Handing over faster let the adapter's own, later
/// slowing overshoot: a V-22 overshot by 1,000 ft, its nacelles came down
/// and it flew round at 150 kt.
pub const ROTOR_APPROACH_DECELERATION_FPS2: f64 = 2.0;
pub const HOVER_HANDOVER_FPS: f64 = 35.0 * 1.687_81;
pub const HOVER_HANDOVER_FT: f64 = 200.0;
/// Fitted: an air-taxiing or parking helicopter counts as stopped below
/// this ground speed, ft/s.
pub const HOVER_STOPPED_FPS: f64 = 5.0;
/// Fitted: the air taxi first lifts to within this of its height.
pub const AIR_TAXI_LIFT_MARGIN_FT: f64 = 5.0;
/// Fitted: the climb and sink a hover approach or air taxi may command
/// while holding its height, ft/s.
pub const HOVER_HEIGHT_RATE_FPS: f64 = 10.0;

/// The sink a vertical descent may command at `wheel_ft` above the ground:
/// the retail 50 ft/s high up, a tenth of the height a second lower down,
/// never under 5 ft/s.
pub fn descent_rate(wheel_ft: f64) -> f64 {
    (wheel_ft * DESCENT_SHARE_PER_S).clamp(TOUCHDOWN_SINK_FPS, JET_DESCENT_FPS)
}

/// The sink a vertical descent may command `elapsed_s` after it began at
/// `wheel_ft`: [`descent_rate`], built up at [`DESCENT_RAMP_FPS2`]. Every
/// vertical type is held to [`JET_DESCENT_FPS`] (fitted; retail's
/// [`super::VERTICAL_DESCENT_FPS`] is out of the physics' reach); `_jet`
/// is kept for a type-specific rate.
pub fn descent_limit(wheel_ft: f64, elapsed_s: f64, _jet: bool) -> f64 {
    descent_rate(wheel_ft).min((elapsed_s * DESCENT_RAMP_FPS2).max(TOUCHDOWN_SINK_FPS))
}

impl Sequence {
    /// Wheel height above the ground under the aircraft.
    fn wheel_ft(s: &Situation) -> f64 {
        s.agl_ft - s.ground_clearance_ft
    }

    /// The height the vertical climb ends at.
    fn climb_height_ft(&self) -> f64 {
        if self.lift == Lift::Rotor {
            ROTOR_CLIMB_AGL_FT
        } else {
            JET_CLIMB_AGL_FT
        }
    }

    /// The hover approach's height: the fourth gate's above the runway.
    fn hover_height_ft(&self) -> f64 {
        self.gate(3)[1] - self.runway.elevation_ft
    }

    /// The parking slot a helicopter sets down on: the airport's slot, or
    /// without anchors the fitted spot by the far end.
    fn parking_point(&self, s: &Situation) -> [f64; 3] {
        match self.runway.anchors {
            Some(anchors) => {
                anchors.parking[self.slot.unwrap_or(0).min(PARKING_SLOTS - 1) as usize]
            }
            None => self.park_point(s),
        }
    }

    /// The heading a helicopter parks on.
    fn parking_heading_deg(&self) -> f64 {
        self.runway.anchors.map_or_else(
            || self.landing_course().to_degrees(),
            |a| a.parking_heading.to_degrees(),
        )
    }

    /// The vertical phases' transitions; true when the takeoff has finished.
    pub(super) fn vertical_transition(&mut self, s: &Situation) -> bool {
        let wheel = Self::wheel_ft(s);
        let ground_speed = horizontal(s.velocity);
        match self.phase {
            Phase::VerticalHold => {
                let since = *self.timer.get_or_insert(s.tick);
                if seconds(s.tick - since) >= VERTICAL_HOLD_S {
                    self.enter_vertical(Phase::VerticalClimb, s, self.hover_heading);
                }
            }
            Phase::VerticalClimb => {
                if !s.on_ground && self.liftoff_tick.is_none() {
                    self.liftoff_tick = Some(s.tick);
                }
                let since = *self.timer.get_or_insert(s.tick);
                let reached = wheel >= self.climb_height_ft() - CLIMB_REACHED_FT;
                if reached || seconds(s.tick - since) >= VERTICAL_CLIMB_TIMEOUT_S {
                    if self.lift == Lift::Rotor {
                        return self.finish_takeoff();
                    }
                    self.enter_vertical(Phase::Transition, s, self.hover_heading);
                }
            }
            Phase::Transition => {
                let since = *self.timer.get_or_insert(s.tick);
                if s.speed_fps >= s.minimum_speed_fps + TRANSITION_MARGIN_FPS
                    || seconds(s.tick - since) >= TRANSITION_TIMEOUT_S
                {
                    return self.finish_takeoff();
                }
            }
            Phase::HoverApproach => {
                if horizontal_distance(s.position, self.landing_point()) <= OVER_POINT_FT
                    && ground_speed <= OVER_POINT_FPS
                {
                    self.enter_vertical(Phase::VerticalDescent, s, self.hover_heading);
                }
            }
            Phase::VerticalDescent => {
                if s.on_ground {
                    // Down on the slot after the air taxi: parked for good;
                    // down at the landing point: the 5 s hold.
                    let parking = self.park.is_some() && self.lift == Lift::Rotor;
                    if parking {
                        self.enter(Phase::Parked, s);
                    } else {
                        self.enter_vertical(Phase::Settle, s, self.hover_heading);
                    }
                }
            }
            Phase::Settle => {
                let since = *self.timer.get_or_insert(s.tick);
                if seconds(s.tick - since) >= VERTICAL_HOLD_S {
                    if self.lift == Lift::Rotor {
                        self.park = Some(self.parking_point(s));
                        self.enter_vertical(Phase::AirTaxi, s, s.heading_deg);
                    } else {
                        // A jet taxis in on its wheels, as after a rollout.
                        self.park = Some(self.park_point(s));
                        self.enter(Phase::TaxiClear, s);
                    }
                }
            }
            Phase::AirTaxi => {
                let target = self.park.unwrap_or(self.spot);
                if horizontal_distance(s.position, target) <= AIR_TAXI_REACHED_FT
                    && ground_speed <= HOVER_STOPPED_FPS
                {
                    self.enter_vertical(Phase::VerticalDescent, s, self.parking_heading_deg());
                }
            }
            _ => {}
        }
        false
    }

    /// The takeoff-finish tail: free flight next.
    fn finish_takeoff(&mut self) -> bool {
        self.flaps_up = true;
        self.slot = None;
        true
    }

    /// The vertical phases' commands.
    pub(super) fn vertical_command(
        &mut self,
        s: &Situation,
        complete: bool,
        command: &mut Command,
    ) {
        let rotor = self.lift == Lift::Rotor;
        let wheel = Self::wheel_ft(s);
        let hover = |point: [f64; 3], height: f64, heading: f64| HoverGuidance {
            point: [point[0], point[2]],
            velocity_fps: [0.0; 2],
            height_agl_ft: height,
            heading_deg: heading,
            max_climb_fps: HOVER_HEIGHT_RATE_FPS,
            max_descent_fps: HOVER_HEIGHT_RATE_FPS,
            max_speed_fps: HOVER_CREEP_FPS,
        };
        command.brakes = false;
        command.gear_down = true;
        match self.phase {
            Phase::VerticalHold | Phase::Settle => {
                // On the wheels, brakes set: a helicopter with its engines
                // governed and the collective down, a jet at idle with its
                // nozzles down for the liftoff or aft after the landing.
                command.brakes = true;
                if rotor {
                    command.control = Control::Ground {
                        throttle: 1.0,
                        pitch: 0.0,
                        yaw: 0.0,
                    };
                    command.lift.collective = Some(0.0);
                } else {
                    command.control = Control::Ground {
                        throttle: 0.0,
                        pitch: 0.0,
                        yaw: 0.0,
                    };
                    command.lift.nozzle_deg = Some(if self.phase == Phase::VerticalHold {
                        HOVER_NOZZLE_DEG
                    } else {
                        0.0
                    });
                }
            }
            Phase::VerticalClimb => {
                command.control = Control::Hover(HoverGuidance {
                    max_climb_fps: VERTICAL_CLIMB_FPS,
                    ..hover(self.leg_origin, self.climb_height_ft(), self.hover_heading)
                });
                command.gear_down = !complete;
            }
            Phase::Transition => {
                let target = s.minimum_speed_fps + TRANSITION_MARGIN_FPS;
                let handover = TRANSITION_HANDOVER_SHARE * s.minimum_speed_fps;
                if s.speed_fps < handover && !complete {
                    // Jetborne: the hover law accelerates it along the
                    // runway heading, holding the height.
                    let course = self.hover_heading.to_radians();
                    command.control = Control::Hover(HoverGuidance {
                        velocity_fps: [course.sin() * target, course.cos() * target],
                        max_speed_fps: target,
                        ..hover(s.position, JET_CLIMB_AGL_FT, self.hover_heading)
                    });
                } else {
                    // Wingborne: full power on a gentle climb, the nozzles
                    // going aft as the speed builds.
                    command.control = Control::Air(super::AirGuidance {
                        heading_deg: self.hover_heading,
                        flight_path_pitch_deg: TRANSITION_CLIMB_DEG,
                        speed_fps: s.maximum_speed_fps,
                        wings_level: false,
                        terrain_floor: true,
                        full_power: true,
                    });
                    let share =
                        ((target - s.speed_fps) / (target - handover).max(1.0)).clamp(0.0, 1.0);
                    command.lift.nozzle_deg = Some(SHORT_NOZZLES_DEG[0] * share);
                }
                command.gear_down = !complete;
                if complete {
                    // Retail's takeoff finish: thrust vector 0.
                    command.lift.nozzle_deg = Some(0.0);
                }
            }
            Phase::HoverApproach => {
                let point = self.landing_point();
                let distance = horizontal_distance(s.position, point);
                let slowing = (2.0 * HOVER_DECELERATION_FPS2 * distance).sqrt();
                let height = self.hover_height_ft();
                let ground_speed = horizontal(s.velocity);
                if rotor && distance > HOVER_HANDOVER_FT && ground_speed > HOVER_HANDOVER_FPS {
                    // Still fast: the forward law, slowing as it closes.
                    let altitude = point[1] + height + s.ground_clearance_ft;
                    let speed = (2.0 * ROTOR_APPROACH_DECELERATION_FPS2 * distance)
                        .sqrt()
                        .min(VERTICAL_GATE_SPEED_FPS);
                    let mut guidance = fly_to(s, [point[0], altitude, point[2]], speed);
                    guidance.terrain_floor = false;
                    command.control = Control::Air(guidance);
                } else {
                    command.control = Control::Hover(HoverGuidance {
                        max_speed_fps: if rotor {
                            slowing.min(VERTICAL_GATE_SPEED_FPS)
                        } else {
                            slowing
                        }
                        .max(OVER_POINT_FPS),
                        // Arriving high (the gates are judged across the
                        // ground only), it comes down as it closes.
                        max_descent_fps: JET_DESCENT_FPS,
                        ..hover(point, height, self.hover_heading)
                    });
                }
            }
            Phase::VerticalDescent => {
                let point = self.park.filter(|_| rotor).unwrap_or(self.landing_point());
                let elapsed = seconds(s.tick - self.timer.unwrap_or(s.tick));
                let mut guidance = HoverGuidance {
                    max_descent_fps: descent_limit(wheel, elapsed, !rotor),
                    ..hover(point, SET_DOWN_FT, self.hover_heading)
                };
                if wheel < TOUCHDOWN_CHECK_FT && !s.touchdown_safe {
                    // The shared touchdown rule would refuse a touchdown
                    // now (too fast over the ground, or sinking too hard):
                    // hold the height until it would not.
                    guidance.height_agl_ft = wheel;
                }
                command.control = Control::Hover(guidance);
            }
            Phase::AirTaxi => {
                let target = self.park.unwrap_or(self.spot);
                let distance = horizontal_distance(s.position, target);
                if !self.stopping && wheel < AIR_TAXI_AGL_FT - AIR_TAXI_LIFT_MARGIN_FT {
                    // Straight up off the landing point first, on its
                    // heading: tilting toward the slot on the wheels holds
                    // a helicopter down.
                    command.control =
                        Control::Hover(hover(self.leg_origin, AIR_TAXI_AGL_FT, self.hover_heading));
                } else {
                    // Up: on toward the slot (`stopping` marks the lift
                    // done, so a dip on the way does not stop it).
                    self.stopping = true;
                    let heading = if distance > AIR_TAXI_TURN_FT {
                        bearing_deg(s.position, target)
                    } else {
                        self.parking_heading_deg()
                    };
                    command.control = Control::Hover(HoverGuidance {
                        max_speed_fps: AIR_TAXI_FPS,
                        ..hover(target, AIR_TAXI_AGL_FT, heading)
                    });
                }
            }
            _ => {}
        }
    }
}
