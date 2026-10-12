//! The helicopter family of the strike tactics (docs/spec/ai-strike.md,
//! "Helicopters"; slice ST-F): the AH-64 and Mi-24 attack with retail's
//! helicopter pop-up, or, with a stand-off weapon under threat Missiles,
//! from low at the weapon's range.
//!
//! - **Pop-up** (retail shape, docs/spec/ai.md B20): toward a
//!   point 1,500 ft above the target until 10,000 ft out, pull 30 degrees
//!   over the line of sight for 6 s, then point at the target until 1,500 ft
//!   or it is no longer ahead. Retail's pull asks for minimum speed by an
//!   operand slip; T.O.R.E keeps its speed through it (John, 2026-10-11:
//!   "maintain proper speed for defense and quick exfil"), and every leg
//!   asks for the helicopter's maximum speed.
//! - **Stand-off** ([`Pass::StandOff`], fitted, agent): with an AT-2 (or any
//!   guided or seeker store) under threat Missiles, it runs in low and
//!   masked (200 ft above the ground) to 60% of the weapon's zone-1 range
//!   (about where the guided T bar stands), then unmasks: climbs 500 ft for
//!   at most 10 s while the shot comes up,
//!   fires, holds the line, and masks again turning away. It does not stop
//!   to hover: the AI's input-only adapter cannot hold a helicopter at a
//!   hover (asked for a low speed it settles nose-down into the ground), so
//!   the unmask is flown at speed, which also keeps the helicopter's speed
//!   for its defence (John, 2026-10-11).
//! - **Rockets** fly the pop-up's final like a fighter's rocket run: the cue
//!   is flown onto the pass's drawn aim point and up to eight leave at the
//!   record's cadence while it stays inside the skill circle.
//! - **The AT-2** leaves when the shared release test passes (the target in
//!   the launcher's 45 degree cone with a clear line), and the helicopter
//!   then holds its nose on the target until the missile has hit or is gone
//!   (the launcher sight rule: command guidance needs the target in the
//!   launcher's cone every tick).
//! - **Exit** (retail): 5 degrees up at maximum speed, levelling at the
//!   pass's height, to max(20,000 ft, 2R) from the target (a stand-off
//!   attack turns away and stays low), then turn back and choose again.
//!
//! Defence, the safe test and going home are the fighters'.
use super::{
    CHOICE_DISTANCE_FT, Context, Handling, Order, Pass, Phase, ROCKETS_PER_PASS, Release,
    STRAIGHT_EXIT_FT, Segment, Solution, Steer, Store, StoreKind, Strike, TURN_BACK_DEG, aim_draw,
    angle_difference, bearing_deg, depression_deg, elevation_deg, hold_pitch_deg, horizontal,
    known_position, objective_point, release_circle_ft, standing, steering_frame, turn_radius_ft,
};
use crate::ai::awareness::ground::{BriefedKind, ThreatLevel};
use crate::ai::controller::{Activity, OwnState};
use crate::ai::motion::SpeedRequest;
use crate::attitude::Vector;

/// Pop-up (retail `H.AI`): the approach point's height over the target,
/// where the pull starts, its angle over the line of sight, its length, and
/// where the run ends.
pub const POPUP_OFFSET_FT: f64 = 1_500.;
pub const POPUP_PULL_AT_FT: f64 = 10_000.;
pub const POPUP_PULL_DEG: f64 = 30.;
pub const POPUP_PULL_TICKS: u64 = 6 * 120;
pub const POPUP_EXIT_FT: f64 = 1_500.;
/// Exit (retail): the climb and the distance the turn back waits for.
pub const EXIT_CLIMB_DEG: f64 = 5.;
pub const EXIT_TURN_MIN_FT: f64 = 20_000.;
/// Pitch requests are held within this (fitted, agent: a helicopter flies
/// its pass by its flight path, not by an 80 degree fighter dive).
pub const PITCH_LIMIT_DEG: f64 = 30.;
/// The steepest dive flown at the cue, and the height above the ground at
/// which the run ends whatever the cue says (fitted, agent: the AI's
/// adapter cannot pull a helicopter out of a steep dive near the ground).
pub const DIVE_LIMIT_DEG: f64 = 20.;
pub const RUN_FLOOR_AGL_FT: f64 = 500.;
/// Flying the rocket cue: the pitch and heading gains (fitted, agent).
pub const CUE_PITCH_GAIN: f64 = 3.;
pub const CUE_TURN_GAIN: f64 = 3.;
/// A helicopter releases its rockets once the cue is within this share of
/// its skill circle of the drawn aim (fitted, agent; a fighter's is 5%): its
/// slow sweep and its adapter hold the cue to about this.
pub const GATE_SHARE: f64 = 0.25;
/// The least a helicopter's rocket gate may be, feet (fitted, agent,
/// ST-INT): with John's tighter circles (Ace 33 ft) a quarter of the circle
/// is 8 to 29 ft at skills 3 and 2, and the AH-64's cue never came that
/// close in a 150 s pass; 30 ft released at both skills.
pub const GATE_FLOOR_FT: f64 = 40.;
/// Stand-off attack (fitted, agent): the share of the weapon's zone-1
/// range it unmasks at (the guided T bar stands 40% of the reach in from
/// its far edge, docs/spec/air-to-ground.md), the masked run's height above the ground, the
/// unmask's climb and its longest wait, and how far beyond the firing range
/// the masked exit goes before turning back.
pub const STANDOFF_SHARE: f64 = 0.6;
pub const MASK_AGL_FT: f64 = crate::ai::helicopter_defense::MASK_AGL_FT;
pub const UNMASK_CLIMB_FT: f64 = 500.;
pub const UNMASK_TICKS: u64 = 10 * 120;
pub const STANDOFF_EXIT_FT: f64 = 6_000.;
/// A guided round's flight is watched for this long after release before
/// "none in flight" ends the hold (the host sees it the next step).
pub const GUIDE_SETTLE_TICKS: u64 = 60;

/// The helicopter's pass (spec, "Helicopters"): the stand-off attack for a
/// guided or seeker store under threat Missiles, else the pop-up.
pub fn choose_pass(kind: StoreKind, threat: ThreatLevel) -> Pass {
    let stand_off = matches!(
        kind,
        StoreKind::Command | StoreKind::Seeker | StoreKind::RadarAgm | StoreKind::Laser
    );
    if stand_off && threat == ThreatLevel::Missiles {
        Pass::StandOff
    } else {
        Pass::PopUp
    }
}

/// The helicopter's store: the guided weapons first (they reach farthest),
/// then rockets (fitted, agent). Guns and bombs are not chosen.
pub fn choose_store(stores: &[Store], designator: bool) -> Option<Store> {
    use StoreKind as K;
    [K::Command, K::Seeker, K::RadarAgm, K::Laser, K::Rocket]
        .iter()
        .filter(|k| **k != K::Laser || designator)
        .find_map(|kind| {
            stores
                .iter()
                .find(|s| s.kind == *kind && s.rounds > 0)
                .copied()
        })
}

/// The heading and flight-path pitch that fly the rocket cue onto the aim
/// (fitted, agent): the flight path turned by [`CUE_PITCH_GAIN`] times the
/// angle between the aim and the cue, and the heading by [`CUE_TURN_GAIN`]
/// times the cross-track angle, continuously. A helicopter's cue moves
/// slowly and does not creep across the aim in a held dive as a fighter's
/// does, so it is flown all the way in.
pub fn fly_cue(own: &OwnState, velocity: Vector, aim: Vector, impact: Vector) -> (f64, f64) {
    let turn = elevation_deg(own.position, aim) - elevation_deg(own.position, impact);
    let pitch = (steering_frame(own.position, aim).1 + CUE_PITCH_GAIN * turn)
        .clamp(-DIVE_LIMIT_DEG, PITCH_LIMIT_DEG);
    let norm = velocity[0].hypot(velocity[2]);
    let heading = if norm > 1. {
        let u = [velocity[0] / norm, velocity[2] / norm];
        let cross = (aim[0] - impact[0]) * u[1] - (aim[2] - impact[2]) * u[0];
        let reach = horizontal(own.position, aim).max(500.);
        let correction = (CUE_TURN_GAIN * cross.atan2(reach).to_degrees()).clamp(-20., 20.);
        (own.heading_deg + correction).rem_euclid(360.)
    } else {
        bearing_deg(own.position, aim)
    };
    (heading, pitch)
}

/// Whether a store guides after release and so is held to impact.
fn guided(kind: StoreKind) -> bool {
    !kind.unguided()
}

/// One step of a helicopter (see the module notes).
pub(super) fn step(s: &mut Strike, cx: &Context<'_>) -> Order {
    let own = cx.own;
    let order = Order {
        steer: None,
        release: None,
        activity: None,
    };
    // A helicopter flies no lasing hold (slice ST-L's hold is the fixed-wing
    // tactics'; `AiWings::can_lase` gives it no duty): a lase is dropped.
    if s.phase == Phase::Lase || s.lase.is_some() {
        s.lase = None;
        if s.phase == Phase::Lase {
            s.enter(Phase::Ingress, cx.tick);
        }
    }
    if cx.defending || s.threatened(cx) {
        if s.phase != Phase::Defend {
            s.enter(Phase::Defend, cx.tick);
            s.released = 0;
        }
        return mask_run(cx, order);
    }
    if s.phase == Phase::Defend {
        if !s.quiet(cx) {
            return mask_run(cx, order);
        }
        if s.safe(cx) {
            s.enter(Phase::Ingress, cx.tick);
        } else {
            s.withdrawn = true;
            exit(s, cx);
        }
    }
    let solution = s.solution(cx);
    let target_at = solution
        .map(|sol| sol.target_position)
        .or_else(|| s.target.and_then(|id| known_position(cx.picture, id)));
    let target_alive = match (s.target, solution) {
        (Some(_), Some(sol)) => sol.target_alive,
        (Some(id), None) => standing(cx.picture, id),
        (None, _) => false,
    };
    if s.phase == Phase::Attack && !target_alive {
        exit(s, cx);
    }
    match s.phase {
        Phase::Ingress | Phase::Wait => {
            let point = target_at.or_else(|| objective_point(cx.picture));
            let near = point.is_none_or(|p| horizontal(own.position, p) <= CHOICE_DISTANCE_FT);
            let waited =
                s.phase != Phase::Wait || cx.tick.saturating_sub(s.since) >= super::WAIT_TICKS;
            if near && waited {
                choose(s, cx);
            }
        }
        Phase::TurnBack => match target_at {
            Some(point)
                if angle_difference(own.heading_deg, bearing_deg(own.position, point)).abs()
                    > TURN_BACK_DEG => {}
            _ => choose(s, cx),
        },
        Phase::Exit => {
            if let Some(point) = target_at
                && horizontal(own.position, point) >= exit_distance(s, cx)
            {
                if !s.withdrawn {
                    s.enter(Phase::TurnBack, cx.tick);
                } else if s.safe(cx) {
                    s.withdrawn = false;
                    s.enter(Phase::TurnBack, cx.tick);
                } else {
                    s.enter(Phase::Home, cx.tick);
                }
            }
        }
        _ => {}
    }
    let solution = s.solution(cx);
    let target_at = solution
        .map(|sol| sol.target_position)
        .or_else(|| s.target.and_then(|id| known_position(cx.picture, id)));
    let toward = |point: Vector, speed: SpeedRequest, s: &Strike| Steer {
        heading_deg: bearing_deg(own.position, point),
        pitch_deg: hold_pitch_deg(own.position[1], cruise_altitude(s, cx, point)),
        speed,
        afterburner: false,
        bank_deg: None,
        handling: Handling::PLAIN,
    };
    match s.phase {
        Phase::Home | Phase::Defend | Phase::Regain | Phase::Lase => order,
        Phase::Ingress | Phase::Wait | Phase::TurnBack => Order {
            steer: target_at
                .or_else(|| objective_point(cx.picture))
                .map(|p| toward(p, SpeedRequest::Maximum, s)),
            activity: Some(Activity::Pursuing),
            ..order
        },
        Phase::Exit => Order {
            steer: Some(exit_steer(s, cx, target_at)),
            activity: Some(Activity::Attacking),
            ..order
        },
        Phase::Attack => match target_at {
            Some(point) => attack(s, cx, point, solution),
            None => {
                exit(s, cx);
                Order {
                    steer: Some(exit_steer(s, cx, None)),
                    ..order
                }
            }
        },
    }
}

/// While a threat is up and the controller is not flying a helicopter break
/// (a warning not yet answered, or the ten quiet seconds after one): hold the
/// course at maximum speed and settle to the masking height over the highest
/// ground ahead, which `helicopter_defense` does for the break itself. The
/// fighters' air logic would otherwise fly this time at corner speed, which
/// sinks a helicopter (slice ST-H, docs/spec/ai-strike.md, "Helicopter
/// defence").
fn mask_run(cx: &Context<'_>, order: Order) -> Order {
    if cx.defending {
        // The controller's defence flies, and overrides a steer.
        return order;
    }
    let own = cx.own;
    let ground = (own.position[1] - own.agl_ft).max(own.terrain_ahead_ft);
    let floor = ground + MASK_AGL_FT;
    Order {
        steer: Some(Steer {
            heading_deg: own.heading_deg,
            pitch_deg: hold_pitch_deg(own.position[1], floor),
            speed: SpeedRequest::Maximum,
            afterburner: false,
            bank_deg: None,
            handling: Handling::PLAIN,
        }),
        activity: Some(Activity::Defending),
        ..order
    }
}

/// The height the helicopter cruises and turns back at: the pop-up's
/// approach height over the target, or the masked height for a stand-off
/// attack.
fn cruise_altitude(s: &Strike, cx: &Context<'_>, target: Vector) -> f64 {
    let ground = cx.own.position[1] - cx.own.agl_ft;
    if s.pass == Some(Pass::StandOff) {
        ground + MASK_AGL_FT
    } else {
        (target[1] + POPUP_OFFSET_FT).max(ground + MASK_AGL_FT)
    }
}

/// The choice: target, store, pass and aim; or wait, or home.
fn choose(s: &mut Strike, cx: &Context<'_>) {
    let own = cx.own;
    let Some(target) = s.pick_target(cx) else {
        let waiting = cx
            .picture
            .briefed()
            .iter()
            .any(|b| b.standing && b.side != cx.side && b.kind != BriefedKind::Ordered);
        s.enter(
            if waiting && cx.picture.ordered().is_none() {
                Phase::Wait
            } else {
                Phase::Home
            },
            cx.tick,
        );
        s.target = None;
        return;
    };
    let target_at = known_position(cx.picture, target)
        .or_else(|| cx.inputs.solution.map(|sol| sol.target_position))
        .unwrap_or(own.position);
    let threat = cx
        .picture
        .threats()
        .level(cx.side, &[target_at], cx.warnings);
    let store = match s.forced_station {
        Some(station) => cx
            .inputs
            .stores
            .iter()
            .find(|st| st.station == station && st.rounds > 0)
            .copied(),
        None => choose_store(&cx.inputs.stores, cx.inputs.designator),
    };
    let Some(store) = store else {
        s.enter(Phase::Home, cx.tick);
        s.target = None;
        return;
    };
    if s.target != Some(target) {
        s.pass_number = 0;
    }
    s.target = Some(target);
    s.station = Some(store.station);
    s.kind = Some(store.kind);
    s.threat = threat;
    let pass = match s.forced {
        Some(pass @ (Pass::PopUp | Pass::StandOff)) => pass,
        _ => choose_pass(store.kind, threat),
    };
    s.pass = Some(pass);
    s.pass_number += 1;
    let radius_ft = release_circle_ft(cx.experience);
    let draw = aim_draw(s.seed, cx.aircraft, s.pass_number, radius_ft);
    s.aim_ft = draw.offset_ft;
    s.inside = draw.inside;
    s.jink_side = draw.side;
    s.gate_ft = (GATE_SHARE * radius_ft).max(GATE_FLOOR_FT);
    s.holding = None;
    s.released = 0;
    s.next_release = 0;
    s.hold_ft = Some(match pass {
        Pass::StandOff => own.position[1] - own.agl_ft + MASK_AGL_FT,
        _ => target_at[1] + POPUP_OFFSET_FT,
    });
    s.enter(Phase::Attack, cx.tick);
}

fn exit(s: &mut Strike, cx: &Context<'_>) {
    s.exit_heading_deg = cx.own.heading_deg;
    if s.pass == Some(Pass::StandOff)
        && let Some(target) = s.target.and_then(|id| known_position(cx.picture, id))
    {
        // A stand-off attack turns away from the target and stays masked.
        s.exit_heading_deg = bearing_deg(target, cx.own.position);
    }
    s.holding = None;
    s.enter(Phase::Exit, cx.tick);
}

fn exit_distance(s: &Strike, cx: &Context<'_>) -> f64 {
    let radius = turn_radius_ft(cx.own.speed.0, cx.own.g_limit);
    let base = EXIT_TURN_MIN_FT.max(2. * radius);
    if s.pass == Some(Pass::StandOff) {
        let standoff = s
            .station
            .and_then(|st| cx.inputs.stores.iter().find(|x| x.station == st))
            .map_or(0., |st| STANDOFF_SHARE * st.maximum_range_ft);
        base.max(standoff + STANDOFF_EXIT_FT)
    } else {
        base
    }
}

/// Retail's exit: 5 degrees up at maximum speed on the exit heading,
/// levelling at the pass's height; a stand-off attack's exit stays masked.
fn exit_steer(s: &Strike, cx: &Context<'_>, target_at: Option<Vector>) -> Steer {
    let own = cx.own;
    let level = target_at.map_or(own.position[1], |t| cruise_altitude(s, cx, t));
    let pitch = if s.pass == Some(Pass::StandOff) || own.position[1] >= level {
        hold_pitch_deg(own.position[1], level)
    } else {
        EXIT_CLIMB_DEG
    };
    Steer {
        heading_deg: s.exit_heading_deg,
        pitch_deg: pitch,
        speed: SpeedRequest::Maximum,
        afterburner: false,
        bank_deg: None,
        handling: Handling::PLAIN,
    }
}

/// The pass in flight.
fn attack(s: &mut Strike, cx: &Context<'_>, target: Vector, solution: Option<Solution>) -> Order {
    let own = cx.own;
    let pass = s.pass.unwrap_or(Pass::PopUp);
    let kind = s.kind.unwrap_or(StoreKind::Rocket);
    let mut order = Order {
        steer: None,
        release: None,
        activity: Some(Activity::Attacking),
    };
    // A moving target is led by the rocket's time of flight, as a fighter's.
    s.lead_ft = match solution {
        Some(sol) if kind.unguided() => {
            let t = sol.impact_seconds.unwrap_or(0.).clamp(0., 60.);
            [sol.target_velocity[0] * t, sol.target_velocity[2] * t]
        }
        _ => [0.; 2],
    };
    let aim = if kind.unguided() {
        [
            target[0] + s.aim_ft[0] + s.lead_ft[0],
            target[1],
            target[2] + s.aim_ft[1] + s.lead_ft[1],
        ]
    } else {
        target
    };
    let distance = horizontal(own.position, target);
    let ahead = angle_difference(own.heading_deg, bearing_deg(own.position, target)).abs() < 90.;
    let standoff = s
        .store(cx)
        .map_or(POPUP_PULL_AT_FT, |st| STANDOFF_SHARE * st.maximum_range_ft);

    // Segment changes.
    match (pass, s.segment) {
        (Pass::PopUp, Segment::Approach) if distance < POPUP_PULL_AT_FT && ahead => {
            s.segment_to(Segment::Pull, cx.tick)
        }
        (Pass::PopUp, Segment::Pull) if cx.tick.saturating_sub(s.since) >= POPUP_PULL_TICKS => {
            s.segment_to(Segment::Final, cx.tick)
        }
        // The stand-off run unmasks at its range: the "pull" is the climb.
        (Pass::StandOff, Segment::Approach) if distance <= standoff => {
            s.segment_to(Segment::Pull, cx.tick)
        }
        _ => {}
    }
    let since = cx.tick.saturating_sub(s.since);
    let guiding = s.segment == Segment::Guide;
    let ended = if guiding {
        // The launcher holds the line until its round is gone (a short
        // settle covers the step before the host sees it).
        since >= GUIDE_SETTLE_TICKS && !solution.is_some_and(|sol| sol.guided_in_flight)
    } else {
        (s.released > 0 && !s.burst_left(cx, solution, aim))
            || (pass == Pass::PopUp
                && s.segment == Segment::Final
                && (distance < POPUP_EXIT_FT || !ahead || own.agl_ft < RUN_FLOOR_AGL_FT))
            || (pass == Pass::StandOff && s.segment == Segment::Pull && since >= UNMASK_TICKS)
            || (pass == Pass::StandOff && distance < STRAIGHT_EXIT_FT)
    };
    if ended {
        exit(s, cx);
        order.steer = Some(exit_steer(s, cx, Some(target)));
        return order;
    }

    // Steering.
    let point_at = |p: Vector| {
        let (heading, pitch) = steering_frame(own.position, p);
        (heading, pitch.clamp(-PITCH_LIMIT_DEG, PITCH_LIMIT_DEG))
    };
    // Every leg at the helicopter's best speed (John, 2026-10-11: "maintain
    // proper speed for defense and quick exfil"): asked for corner, the
    // adapter slows a helicopter until it sinks.
    let corner = SpeedRequest::Maximum;
    let steer = match (pass, s.segment) {
        (Pass::PopUp, Segment::Approach) => {
            let (heading, pitch) = point_at([target[0], target[1] + POPUP_OFFSET_FT, target[2]]);
            (heading, pitch, corner)
        }
        (Pass::PopUp, Segment::Pull) => (
            bearing_deg(own.position, aim),
            (-depression_deg(own.position, target) + POPUP_PULL_DEG)
                .clamp(-PITCH_LIMIT_DEG, PITCH_LIMIT_DEG),
            corner,
        ),
        (Pass::StandOff, Segment::Approach) => (
            bearing_deg(own.position, target),
            hold_pitch_deg(own.position[1], own.position[1] - own.agl_ft + MASK_AGL_FT),
            corner,
        ),
        (Pass::StandOff, _) => {
            // Unmasked: nose on the target, up to the unmask height.
            let top = s.hold_ft.unwrap_or(own.position[1]) + UNMASK_CLIMB_FT;
            (
                bearing_deg(own.position, target),
                hold_pitch_deg(own.position[1], top),
                corner,
            )
        }
        (_, Segment::Guide) => {
            // Holding the line: the nose on the target, level.
            (
                bearing_deg(own.position, target),
                hold_pitch_deg(own.position[1], own.position[1]),
                corner,
            )
        }
        _ => match solution
            .and_then(|sol| sol.impact)
            .filter(|_| kind.unguided())
        {
            Some(impact) => {
                let (_, pitch) = fly_cue(own, cx.velocity, aim, impact);
                (bearing_deg(own.position, aim), pitch, corner)
            }
            None => {
                let (heading, pitch) = point_at(aim);
                (heading, pitch, corner)
            }
        },
    };
    order.steer = Some(Steer {
        heading_deg: steer.0,
        pitch_deg: steer.1,
        speed: steer.2,
        afterburner: false,
        bank_deg: None,
        handling: Handling::PLAIN,
    });

    // The release: in the pop-up's final (rockets also in its pull), or
    // unmasked on a stand-off run.
    let window = match pass {
        Pass::StandOff => s.segment == Segment::Pull,
        _ => matches!(s.segment, Segment::Final | Segment::Pull),
    };
    if window
        && let Some(sol) = solution
        && cx.tick >= s.next_release
    {
        let first = s.released == 0;
        let in_zone = !kind.unguided()
            || sol.impact.is_some_and(|i| {
                let range =
                    (horizontal(own.position, i).powi(2) + (own.position[1] - i[1]).powi(2)).sqrt();
                s.store(cx).is_none_or(|st| {
                    range >= st.minimum_range_ft
                        && (st.maximum_range_ft <= 0. || range <= st.maximum_range_ft)
                })
            });
        let fire = in_zone
            && match kind {
                StoreKind::Rocket => {
                    (first && sol.due)
                        || (!first
                            && s.released < ROCKETS_PER_PASS
                            && sol.impact.is_some_and(|i| {
                                horizontal(i, aim) <= release_circle_ft(cx.experience)
                            }))
                }
                _ => first && sol.due && !sol.guided_in_flight,
            };
        if fire && !sol.warning && s.rounds_left(cx) > 0 {
            order.release = Some(Release {
                station: sol.station,
                target: sol.target,
            });
            s.released += 1;
            s.next_release = cx.tick
                + s.store(cx)
                    .map_or(super::super::QUARTER_SECOND_TICKS, |st| st.cadence_ticks);
            if guided(kind) {
                s.segment_to(Segment::Guide, cx.tick);
            }
        }
    }
    order
}

#[cfg(test)]
#[path = "family_helicopter_tests.rs"]
mod tests;
