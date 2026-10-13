//! Scripted flights of the AI's rotorcraft and jet-lift control (slice H1),
//! over flat ground at sea level: the measurements the acceptance tests
//! assert on the synthetic PT fixtures and the `--rotor-probe` command
//! prints for the imported aircraft (docs/testing/lane-flight.md "Rotor
//! probe"). Each flight is a pure function of its aircraft, so the same
//! record gives the same numbers on every machine.
//!
//! The aircraft fly clean, on the Damper stability level, with unlimited
//! fuel so the longer flights end in the state they test.
use super::ScalarSpeed;
use super::controller::{Completion, MotionIntent};
use super::jet_lift::JetLift;
use super::motion::{Bank, CompletionAxis, Duration, MotionRequest, PitchRequest, SpeedRequest};
use super::rotor_adapter::{HoverGuidance, Regime, RotorAdapter, RotorRequest};
use super::steering::CommandMode;
use crate::flight::State;
use crate::models::FlightModel;
use crate::models::variety::{LiftKind, RotorLayout};
use crate::research::Surface;
use tore_formats::aircraft::Aircraft;

/// ft/s per knot.
pub const KT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
/// Ticks per second.
const HZ: usize = 120;
/// The steady wind of the windy hold: 15 kt from the west, across the
/// northbound heading.
pub const WIND_15_KT: [f64; 3] = [15. * KT, 0., 0.];

/// Flat ground at sea level in a steady `wind`.
pub fn ground(wind: [f64; 3]) -> impl Fn(f64, f64) -> Surface {
    move |_, _| Surface {
        wind,
        ..Surface::runway(0.)
    }
}

fn clearance(s: &State) -> f64 {
    s.model().configuration().equipment.ground_clearance_ft
}

/// Wheel height over the flat ground, ft.
pub fn wheel_height(s: &State) -> f64 {
    s.position[1] - clearance(s)
}

fn distance(point: [f64; 2], s: &State) -> f64 {
    (point[0] - s.position[0]).hypot(point[1] - s.position[2])
}

fn ground_speed(s: &State) -> f64 {
    s.velocity[0].hypot(s.velocity[2])
}

fn wrap_deg(degrees: f64) -> f64 {
    (degrees + 180.).rem_euclid(360.) - 180.
}

fn lift_kind(s: &State) -> Option<LiftKind> {
    s.model().powered_lift().map(|lift| lift.kind)
}

/// A hybrid flight of `aircraft` heading north at `altitude`, not trimmed.
fn flight(aircraft: &Aircraft, altitude: f64) -> Option<State> {
    let mut s = State::new(aircraft, [0., altitude, 0.]).ok()?;
    s.enable_research(1).ok()?;
    s.cheats.unlimited_fuel = true;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = 0.;
    s.velocity = [0.; 3];
    Some(s)
}

/// `aircraft` in a trimmed hover at wheel height `height` heading north,
/// drifting with the `wind`; none if it cannot hover.
pub fn hovering(aircraft: &Aircraft, height: f64, wind: [f64; 3]) -> Option<State> {
    let probe = flight(aircraft, 0.)?;
    let mut s = flight(aircraft, height + clearance(&probe))?;
    if lift_kind(&s) == Some(LiftKind::VectorJet) {
        s.gear_down = true;
        s.gear = 1.;
        if !s.trim_hover() {
            return None;
        }
    } else {
        s.gear = 0.;
        s.gear_down = s.model().fixed_gear();
        return moving(aircraft, height + clearance(&s), 0., wind);
    }
    for (v, w) in s.velocity.iter_mut().zip(wind) {
        *v += w;
    }
    Some(s)
}

/// A rotorcraft trimmed level at `knots` of ground speed heading north at
/// `altitude` in the `wind`; the V-22 at its 87 degree helicopter preset.
pub fn moving(aircraft: &Aircraft, altitude: f64, knots: f64, wind: [f64; 3]) -> Option<State> {
    let mut s = flight(aircraft, altitude)?;
    s.gear = 0.;
    s.gear_down = s.model().fixed_gear();
    let air = knots * KT - wind[2];
    let lift = s.model().powered_lift()?;
    let tandem = lift
        .rotor
        .is_some_and(|r| matches!(r.layout, RotorLayout::Tandem { .. }));
    let trimmed = match lift.kind {
        LiftKind::Tiltrotor => s.trim_tiltrotor(air, 87.),
        LiftKind::Helicopter if tandem => s.trim_tandem(air),
        LiftKind::Helicopter => s.trim_single_rotor(air),
        LiftKind::VectorJet => false,
    };
    if !trimmed {
        return None;
    }
    for (v, w) in s.velocity.iter_mut().zip(wind) {
        *v += w;
    }
    s.speed = crate::attitude::dot(s.velocity, s.velocity).sqrt();
    Some(s)
}

/// A steer request of the fighter families.
pub fn steer(heading_deg: f64, path_deg: f64, speed_fps: f64) -> MotionIntent {
    let duration = Duration::Timed(1);
    MotionIntent {
        id: 0,
        request: MotionRequest::new(
            heading_deg.round() as i32,
            PitchRequest::Explicit(path_deg.round() as i32),
            Bank::Unconstrained,
            SpeedRequest::Explicit(ScalarSpeed(speed_fps)),
            duration,
        ),
        heading_deg,
        flight_path_pitch_deg: path_deg,
        speed: ScalarSpeed(speed_fps),
        bank: Bank::Unconstrained,
        completion: Completion::Axis(CompletionAxis::Heading),
        steering_point: None,
        mode: CommandMode::Ordinary,
        formation_flight: false,
        afterburner: false,
    }
}

/// Hold the point under `s` at wheel height `height`, at the hover hold's
/// 500 ft/min and a 30 ft/s closing speed.
pub fn hold_here(s: &State, height: f64) -> HoverGuidance {
    HoverGuidance {
        point: [s.position[0], s.position[2]],
        velocity_fps: [0.; 2],
        height_agl_ft: height,
        heading_deg: s.yaw.to_degrees(),
        max_climb_fps: 500. / 60.,
        max_descent_fps: 500. / 60.,
        max_speed_fps: 30.,
    }
}

/// One tick of `adapter` on `s` over the flat ground in `wind`.
pub fn tick(s: &mut State, adapter: &mut RotorAdapter, request: RotorRequest<'_>, wind: [f64; 3]) {
    let (input, _) = adapter.controls(s, request, 0., wind, 60.);
    s.step_surface(&input, ground(wind));
}

fn ticks(seconds: f64) -> usize {
    (seconds * HZ as f64) as usize
}

/// A hover hold's result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hold {
    /// Largest distance from the point and height error, ft, over a minute
    /// after 30 s to settle.
    pub off_ft: f64,
    pub height_ft: f64,
    pub hovering: bool,
    pub crashed: bool,
}

/// Holds the point it starts over at 200 ft in `wind`.
pub fn hold(aircraft: &Aircraft, wind: [f64; 3]) -> Option<Hold> {
    let mut s = hovering(aircraft, 200., wind)?;
    let mut adapter = RotorAdapter::new();
    let guidance = hold_here(&s, 200.);
    let mut out = Hold::default();
    for t in 0..ticks(90.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), wind);
        if t >= ticks(30.) {
            out.off_ft = out.off_ft.max(distance(guidance.point, &s));
            out.height_ft = out
                .height_ft
                .max((wheel_height(&s) - guidance.height_agl_ft).abs());
        }
    }
    out.hovering = adapter.regime() == Regime::Hover;
    out.crashed = s.crashed;
    Some(out)
}

/// A moved point's result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Move {
    /// Farthest travel along the move past its 500 ft, ft.
    pub overshoot_ft: f64,
    /// Seconds until within 10 ft of the new point.
    pub arrived_s: Option<f64>,
    /// Distance from the point and height error after 90 s, ft.
    pub off_ft: f64,
    pub height_ft: f64,
    pub crashed: bool,
}

/// Moves the held point 500 ft diagonally ahead and right at 200 ft.
pub fn move_point(aircraft: &Aircraft) -> Option<Move> {
    let mut s = hovering(aircraft, 200., [0.; 3])?;
    let mut adapter = RotorAdapter::new();
    let mut guidance = hold_here(&s, 200.);
    for _ in 0..ticks(20.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
    }
    let start = [s.position[0], s.position[2]];
    let direction = [0.6, 0.8];
    guidance.point = [
        start[0] + 500. * direction[0],
        start[1] + 500. * direction[1],
    ];
    let mut out = Move::default();
    let mut progress = f64::MIN;
    for t in 0..ticks(90.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
        progress = progress.max(
            (s.position[0] - start[0]) * direction[0] + (s.position[2] - start[1]) * direction[1],
        );
        if out.arrived_s.is_none() && distance(guidance.point, &s) < 10. {
            out.arrived_s = Some(t as f64 / HZ as f64);
        }
    }
    out.overshoot_ft = progress - 500.;
    out.off_ft = distance(guidance.point, &s);
    out.height_ft = (wheel_height(&s) - 200.).abs();
    out.crashed = s.crashed;
    Some(out)
}

/// A vertical climb's result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Climb {
    /// Fastest climb, ft/s.
    pub fastest_fps: f64,
    /// Highest and final wheel height, ft.
    pub top_ft: f64,
    pub end_ft: f64,
    /// Largest distance from the point, ft.
    pub drift_ft: f64,
    pub crashed: bool,
}

/// Climbs vertically from 20 ft to 500 ft at retail's 25 ft/s.
pub fn vertical_climb(aircraft: &Aircraft) -> Option<Climb> {
    let mut s = hovering(aircraft, 20., [0.; 3])?;
    let mut adapter = RotorAdapter::new();
    let mut guidance = hold_here(&s, 20.);
    for _ in 0..ticks(10.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
    }
    guidance.height_agl_ft = 500.;
    guidance.max_climb_fps = 25.;
    let mut out = Climb::default();
    for _ in 0..ticks(60.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
        out.fastest_fps = out.fastest_fps.max(s.vertical_speed);
        out.top_ft = out.top_ft.max(wheel_height(&s));
        out.drift_ft = out.drift_ft.max(distance(guidance.point, &s));
    }
    out.end_ft = wheel_height(&s);
    out.crashed = s.crashed;
    Some(out)
}

/// A forward flight's result over the last 30 s of a minute.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Forward {
    /// Mean climb, ft/min.
    pub climb_fpm: f64,
    /// Largest speed and heading errors, kt and degrees.
    pub speed_off_kt: f64,
    pub heading_off_deg: f64,
    pub crashed: bool,
}

/// Flies a steer request of `knots` and `fpm` on heading 10 from level
/// flight at `knots` heading north at 3,000 ft.
pub fn forward(aircraft: &Aircraft, knots: f64, fpm: f64) -> Option<Forward> {
    let mut s = moving(aircraft, 3_000., knots, [0.; 3])?;
    let mut adapter = RotorAdapter::new();
    let speed = knots * KT;
    let intent = steer(10., (fpm / 60. / speed).asin().to_degrees(), speed);
    let mut out = Forward::default();
    let mut climb = 0.;
    for t in 0..ticks(60.) {
        let request = RotorRequest::Steer {
            intent: &intent,
            terrain_pitch_floor_deg: None,
        };
        tick(&mut s, &mut adapter, request, [0.; 3]);
        if t >= ticks(30.) {
            climb += s.vertical_speed * 60.;
            out.speed_off_kt = out.speed_off_kt.max((s.speed / KT - knots).abs());
            out.heading_off_deg = out
                .heading_off_deg
                .max(wrap_deg(10. - s.yaw.to_degrees()).abs());
        }
    }
    out.climb_fpm = climb / ticks(30.) as f64;
    out.crashed = s.crashed;
    Some(out)
}

/// Slowing to a hover: the result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slow {
    /// Most height lost on the way, ft.
    pub lost_ft: f64,
    /// Distance from the point and ground speed after 150 s, ft and kt.
    pub off_ft: f64,
    pub ground_speed_kt: f64,
    pub hovering: bool,
    pub crashed: bool,
}

/// From 120 kt at 1,000 ft, a hover over a point 3,000 ft ahead.
pub fn slow_to_hover(aircraft: &Aircraft) -> Option<Slow> {
    let mut s = moving(aircraft, 1_000., 120., [0.; 3])?;
    let mut adapter = RotorAdapter::new();
    let start = wheel_height(&s);
    let guidance = HoverGuidance {
        point: [s.position[0], s.position[2] + 3_000.],
        ..hold_here(&s, start)
    };
    let mut lowest = start;
    for _ in 0..ticks(150.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
        lowest = lowest.min(wheel_height(&s));
    }
    Some(Slow {
        lost_ft: start - lowest,
        off_ft: distance(guidance.point, &s),
        ground_speed_kt: ground_speed(&s) / KT,
        hovering: adapter.regime() == Regime::Hover,
        crashed: s.crashed,
    })
}

/// The V-22's conversion both ways: the result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Conversion {
    /// Nacelles and true airspeed after two minutes asking for 220 kt.
    pub out_nacelle_deg: f64,
    pub out_kt: f64,
    /// Seconds from the hover until the nacelles reached the downstops.
    pub converted_s: Option<f64>,
    /// Nacelles, distance from the point and ground speed after four
    /// minutes asking for a hover 15,000 ft ahead.
    pub back_nacelle_deg: f64,
    pub back_off_ft: f64,
    pub back_ground_speed_kt: f64,
    pub hovering: bool,
    /// Largest height change through both, ft.
    pub height_ft: f64,
    /// Ticks on which the corridor protection overrode the nacelle demand.
    pub corridor_ticks: usize,
    pub crashed: bool,
}

/// From a hover at 1,000 ft to airplane mode at 220 kt and back to a hover.
pub fn convert(aircraft: &Aircraft) -> Option<Conversion> {
    let mut s = hovering(aircraft, 1_000., [0.; 3])?;
    if lift_kind(&s) != Some(LiftKind::Tiltrotor) {
        return None;
    }
    let mut adapter = RotorAdapter::new();
    let start = wheel_height(&s);
    let intent = steer(0., 0., 220. * KT);
    let mut out = Conversion::default();
    let note = |s: &State, out: &mut Conversion| {
        // The protection sent the nacelles somewhere other than the
        // demand (not merely slowed them near a stop).
        out.corridor_ticks += usize::from(
            s.lift_controls
                .corridor_hold
                .is_some_and(|hold| (hold - s.lift_controls.conversion).abs() > 1e-6),
        );
        out.height_ft = out.height_ft.max((wheel_height(s) - start).abs());
    };
    for t in 0..ticks(120.) {
        let request = RotorRequest::Steer {
            intent: &intent,
            terrain_pitch_floor_deg: None,
        };
        tick(&mut s, &mut adapter, request, [0.; 3]);
        note(&s, &mut out);
        if out.converted_s.is_none() && s.nacelle_degrees() < 0.5 {
            out.converted_s = Some(t as f64 / HZ as f64);
        }
    }
    out.out_nacelle_deg = s.nacelle_degrees();
    out.out_kt = s.speed / KT;
    let guidance = HoverGuidance {
        point: [s.position[0], s.position[2] + 15_000.],
        ..hold_here(&s, start)
    };
    for _ in 0..ticks(240.) {
        tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), [0.; 3]);
        note(&s, &mut out);
    }
    out.back_nacelle_deg = s.nacelle_degrees();
    out.back_off_ft = distance(guidance.point, &s);
    out.back_ground_speed_kt = ground_speed(&s) / KT;
    out.hovering = adapter.regime() == Regime::Hover;
    out.crashed = s.crashed;
    Some(out)
}

/// A jet's hover and vertical landing: the result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JetHover {
    /// Largest distance from the point and height error, ft, over a minute
    /// after 30 s to settle at 200 ft.
    pub off_ft: f64,
    pub height_ft: f64,
    /// Vertical speed on the tick before touchdown, ft/s, descending at
    /// 5 ft/s.
    pub touchdown_fps: Option<f64>,
    /// Distance from the point at touchdown, ft.
    pub landing_off_ft: f64,
    pub crashed: bool,
}

/// An AV-8 or Yak-141 holds a hover at 200 ft, then lands vertically.
pub fn jet_hover(aircraft: &Aircraft) -> Option<JetHover> {
    let mut s = hovering(aircraft, 200., [0.; 3])?;
    if lift_kind(&s) != Some(LiftKind::VectorJet) {
        return None;
    }
    let mut law = JetLift::new();
    let guidance = hold_here(&s, 200.);
    let mut out = JetHover::default();
    for t in 0..ticks(90.) {
        let (input, _) = law.controls(&s, &guidance, 0.);
        s.step_surface(&input, ground([0.; 3]));
        if t >= ticks(30.) {
            out.off_ft = out.off_ft.max(distance(guidance.point, &s));
            out.height_ft = out.height_ft.max((wheel_height(&s) - 200.).abs());
        }
    }
    let down = HoverGuidance {
        height_agl_ft: -10.,
        max_descent_fps: 5.,
        ..guidance
    };
    for _ in 0..ticks(90.) {
        let (input, _) = law.controls(&s, &down, 0.);
        let sink = s.vertical_speed;
        s.step_surface(&input, ground([0.; 3]));
        if s.weight_on_wheels() || s.crashed {
            out.touchdown_fps = Some(sink);
            break;
        }
    }
    out.landing_off_ft = distance(guidance.point, &s);
    out.crashed = s.crashed;
    Some(out)
}

/// The probe's lines for `aircraft`, one per flight; none for an aircraft
/// that does not hover.
pub fn report(aircraft: &Aircraft) -> Vec<String> {
    let name = aircraft.id.pt();
    let mut lines = Vec::new();
    let Some(s) = flight(aircraft, 1_000.) else {
        return lines;
    };
    let fate = |crashed: bool| if crashed { "CRASHED" } else { "flew" };
    match lift_kind(&s) {
        Some(LiftKind::Helicopter | LiftKind::Tiltrotor) => {
            for (label, wind) in [("calm", [0.; 3]), ("wind15", WIND_15_KT)] {
                if let Some(h) = hold(aircraft, wind) {
                    lines.push(format!(
                        "ROTOR {name} hold {label}: off={:.1}ft height={:.1}ft hover={} {}",
                        h.off_ft,
                        h.height_ft,
                        h.hovering,
                        fate(h.crashed)
                    ));
                }
            }
            if let Some(m) = move_point(aircraft) {
                lines.push(format!(
                    "ROTOR {name} move500: overshoot={:.1}ft arrived={} off={:.1}ft height={:.1}ft {}",
                    m.overshoot_ft,
                    m.arrived_s.map_or("never".into(), |t| format!("{t:.1}s")),
                    m.off_ft,
                    m.height_ft,
                    fate(m.crashed)
                ));
            }
            if let Some(c) = vertical_climb(aircraft) {
                lines.push(format!(
                    "ROTOR {name} climb500: fastest={:.1}ft/s top={:.1}ft end={:.1}ft drift={:.1}ft {}",
                    c.fastest_fps,
                    c.top_ft,
                    c.end_ft,
                    c.drift_ft,
                    fate(c.crashed)
                ));
            }
            for knots in [40., 80., 120.] {
                for fpm in [1_000., -1_000.] {
                    if let Some(f) = forward(aircraft, knots, fpm) {
                        lines.push(format!(
                            "ROTOR {name} forward {knots:.0}kt {fpm:+.0}fpm: climb={:.0}fpm speed_off={:.1}kt heading_off={:.1}deg {}",
                            f.climb_fpm,
                            f.speed_off_kt,
                            f.heading_off_deg,
                            fate(f.crashed)
                        ));
                    }
                }
            }
            if let Some(w) = slow_to_hover(aircraft) {
                lines.push(format!(
                    "ROTOR {name} slow120: lost={:.1}ft off={:.1}ft ground_speed={:.1}kt hover={} {}",
                    w.lost_ft,
                    w.off_ft,
                    w.ground_speed_kt,
                    w.hovering,
                    fate(w.crashed)
                ));
            }
            if let Some(c) = convert(aircraft) {
                lines.push(format!(
                    "ROTOR {name} convert: out={:.1}deg {:.0}kt converted={} back={:.1}deg off={:.1}ft ground_speed={:.1}kt hover={} height={:.0}ft corridor_ticks={} {}",
                    c.out_nacelle_deg,
                    c.out_kt,
                    c.converted_s.map_or("never".into(), |t| format!("{t:.1}s")),
                    c.back_nacelle_deg,
                    c.back_off_ft,
                    c.back_ground_speed_kt,
                    c.hovering,
                    c.height_ft,
                    c.corridor_ticks,
                    fate(c.crashed)
                ));
            }
        }
        Some(LiftKind::VectorJet) => match jet_hover(aircraft) {
            Some(j) => lines.push(format!(
                "ROTOR {name} jet hover: off={:.1}ft height={:.1}ft touchdown={} landing_off={:.1}ft {}",
                j.off_ft,
                j.height_ft,
                j.touchdown_fps
                    .map_or("none".into(), |v| format!("{v:.1}ft/s")),
                j.landing_off_ft,
                fate(j.crashed)
            )),
            None => lines.push(format!("ROTOR {name} jet hover: cannot hover clean")),
        },
        None => {}
    }
    lines
}
