//! Acceptance tests A1 to A4 of the VTOL overhaul design (section 10) for
//! hover hold (slice P9), and the heading and waypoint autopilot modes on
//! the six powered-lift aircraft, on the synthetic records carrying the PT
//! numbers the other powered-lift tests use. Sea level, standard day, at the
//! default Damper level unless a test says otherwise.

use super::jet::tests::fixture;
use super::*;
use crate::autopilot::{Mode, NavigationTarget};
use crate::flight::trace::Release;
use crate::flight::{HOVER_HOLD_ENGAGED, HOVER_HOLD_OFF, PilotCommand, PilotInput, Switch};
use crate::models::AircraftModel;
use crate::research::Surface;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_input::{FlightAxis, LiftCommand, StabilityLevel, TrimAxis};

const KT: f64 = 1.687_81;
/// The aircraft hover hold flies.
const HOVERING: [AircraftId; 4] = [
    AircraftId::Ah64,
    AircraftId::Mi24,
    AircraftId::Ch47,
    AircraftId::V22,
];
/// Every powered-lift aircraft.
const POWERED: [AircraftId; 6] = [
    AircraftId::Ah64,
    AircraftId::Mi24,
    AircraftId::Ch47,
    AircraftId::V22,
    AircraftId::Av8,
    AircraftId::Yak141,
];

fn aircraft(id: AircraftId) -> Aircraft {
    match id {
        AircraftId::Ah64 | AircraftId::Mi24 => helicopter::tests::pt_aircraft(id),
        AircraftId::Ch47 => tandem::tests::pt_aircraft(),
        AircraftId::V22 => tiltrotor::acceptance::pt_v22(),
        _ => fixture(id),
    }
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// A hybrid flight of `id` at `height`, heading `heading` (rad), at `level`,
/// with the Easy flight physics cheat `easy`, not yet trimmed.
fn flight(id: AircraftId, height: f64, heading: f64, level: StabilityLevel, easy: bool) -> State {
    let mut s = State::new(&aircraft(id), [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.yaw = heading;
    s.pitch = 0.;
    s.bank = 0.;
    s.speed = 0.;
    s.velocity = [0.; 3];
    s.gear = 0.;
    s.gear_down = s.model().fixed_gear();
    s.lift_controls.aids.stability = level;
    s
}

/// A rotorcraft trimmed level at `knots` of ground speed along `heading`
/// (rad) in `wind` (ft/s, world axes): the wind along the heading comes off
/// the airspeed and the wind across it is a drift. The V-22 flies at its
/// 87-degree helicopter preset.
fn moving(
    id: AircraftId,
    height: f64,
    knots: f64,
    heading: f64,
    level: StabilityLevel,
    easy: bool,
    wind: [f64; 3],
) -> State {
    let mut s = flight(id, height, heading, level, easy);
    let air = knots * KT - (wind[0] * heading.sin() + wind[2] * heading.cos());
    let trimmed = match id {
        AircraftId::Ah64 | AircraftId::Mi24 => s.trim_single_rotor(air),
        AircraftId::Ch47 => s.trim_tandem(air),
        AircraftId::V22 => s.trim_tiltrotor(air, 87.),
        _ => unreachable!("{id:?} does not hover hold"),
    };
    assert!(trimmed, "{id:?} trims at {air} ft/s");
    for (v, w) in s.velocity.iter_mut().zip(wind) {
        *v += w;
    }
    s.speed = crate::attitude::dot(s.velocity, s.velocity).sqrt();
    s
}

/// Flat ground at `height` in a steady `wind`.
fn ground(height: f64, wind: [f64; 3]) -> impl Fn(f64, f64) -> Surface {
    move |_, _| Surface {
        wind,
        ..Surface::runway(height)
    }
}

/// Flat ground at sea level in a steady `wind`.
fn steady(wind: [f64; 3]) -> impl Fn(f64, f64) -> Surface {
    ground(0., wind)
}

const CALM: [f64; 3] = [0.; 3];

fn press(switch: Switch) -> PilotInput {
    PilotInput {
        commands: vec![PilotCommand::Toggle(switch)],
        ..Default::default()
    }
}

fn command(command: PilotCommand) -> PilotInput {
    PilotInput {
        commands: vec![command],
        ..Default::default()
    }
}

fn trim(axis: TrimAxis, amount: f64) -> PilotInput {
    command(PilotCommand::Lift(LiftCommand::TrimAdjust(axis, amount)))
}

fn messages(s: &State, text: &str) -> usize {
    s.systems.messages.iter().filter(|m| *m == text).count()
}

/// What one hover hold run measured.
#[derive(Clone, Copy, Debug, Default)]
struct Hold {
    /// Seconds from engagement until the drift fell below 1 kt.
    captured_s: Option<f64>,
    /// Largest distance from the held point, ft, in the 60 s after.
    position_ft: f64,
    /// Largest height change from engagement, ft, in the same minute.
    height_ft: f64,
    /// Largest heading change from engagement, degrees, in the same minute.
    heading_deg: f64,
    /// Still engaged at the end.
    engaged: bool,
}

/// Engages hover hold on `s` and flies `seconds` hands off in the wind
/// `wind(tick)` gives.
fn hold(mut s: State, wind: impl Fn(u64) -> [f64; 3], seconds: f64) -> (State, Hold) {
    let heading = s.yaw;
    let height = s.position[1];
    let start = s.ticks;
    s.step_surface(&press(Switch::HoverHold), steady(wind(s.ticks)));
    let mut out = Hold::default();
    let mut captured = None;
    while ((s.ticks - start) as f64) < seconds * 120. {
        s.step_surface(&PilotInput::default(), steady(wind(s.ticks)));
        if s.autopilot.mode() != Mode::Hover {
            break;
        }
        let Some(point) = s.autopilot.hover_point() else {
            continue;
        };
        let since = s.ticks - *captured.get_or_insert(s.ticks);
        if since <= 60 * 120 {
            let off = (point[0] - s.position[0]).hypot(point[1] - s.position[2]);
            out.position_ft = out.position_ft.max(off);
            out.height_ft = out.height_ft.max((s.position[1] - height).abs());
            out.heading_deg = out
                .heading_deg
                .max(wrap(s.yaw - heading).to_degrees().abs());
        }
    }
    out.captured_s = captured.map(|tick| (tick - start) as f64 / 120.);
    out.engaged = s.autopilot.mode() == Mode::Hover;
    (s, out)
}

/// A gusting wind along `mean`: 5 kt gusts every 7 s and a 3 kt swing every
/// 3 s on top of 15 kt, in proportion for other means.
fn gusting(mean: [f64; 3]) -> impl Fn(u64) -> [f64; 3] {
    move |tick| {
        let t = tick as f64 / 120.;
        let tau = std::f64::consts::TAU;
        let scale = 1. + (5. * (t * tau / 7.).sin() + 3. * (t * tau / 3.).sin()) / 15.;
        mean.map(|w| w * scale)
    }
}

/// A1: engaged at 30 kt in a 15 kt steady wind, hover hold brakes the drift
/// below 1 kt within 20 s, then holds the point within 20 ft and the height
/// within 10 ft for 60 s, announced and labelled. AH-64 (the design's
/// case), Mi-24, CH-47 and V-22.
#[test]
fn a1_hover_hold_brakes_the_drift_and_holds_the_point_in_a_wind() {
    let wind = [15. * KT, 0., 0.];
    for id in HOVERING {
        let s = moving(id, 500., 30., 0., StabilityLevel::Damper, false, wind);
        assert!(s.velocity[0].hypot(s.velocity[2]) > 30. * KT, "{id:?}");
        let mut engaged = s.clone();
        engaged.step_surface(&press(Switch::HoverHold), steady(wind));
        assert_eq!(engaged.autopilot.mode(), Mode::Hover, "{id:?}");
        assert_eq!(engaged.autopilot.label(), "HOVER");
        assert_eq!(messages(&engaged, HOVER_HOLD_ENGAGED), 1, "{id:?}");
        let (s, hold) = hold(s, |_| wind, 85.);
        let captured = hold.captured_s.expect("the drift is braked");
        assert!(hold.engaged && !s.crashed, "{id:?} {hold:?}");
        assert!(captured < 20., "{id:?} drift below 1 kt after {captured} s");
        assert!(hold.position_ft < 20., "{id:?} {hold:?}");
        assert!(hold.height_ft < 10., "{id:?} {hold:?}");
        assert_eq!(messages(&s, HOVER_HOLD_OFF), 0, "{id:?}");
    }
}

/// A1 across the board: every stability level, with and without the Easy
/// flight physics cheat, in calm air, a steady 15 kt wind from four sides
/// and a gusting one, engaged at 30 kt (20 kt where a crosswind would put the
/// drift over 40) on a south-westerly heading. The same tolerances hold
/// everywhere, and the heading stays within 5 degrees.
#[test]
fn a1b_hover_hold_holds_at_every_level_with_the_cheat_and_in_gusts() {
    let heading = 225_f64.to_radians();
    let w = 15. * KT;
    let winds: [[f64; 3]; 5] = [CALM, [w, 0., 0.], [0., 0., w], [-w, 0., 0.], [0., 0., -w]];
    for id in HOVERING {
        for level in StabilityLevel::ALL {
            for easy in [false, true] {
                for (n, wind) in winds.into_iter().enumerate() {
                    let along = wind[0] * heading.sin() + wind[2] * heading.cos();
                    let across = (wind[0].hypot(wind[2]).powi(2) - along * along)
                        .max(0.)
                        .sqrt();
                    let knots = if (30. * KT).hypot(across) < 39. * KT {
                        30.
                    } else {
                        20.
                    };
                    let s = moving(id, 500., knots, heading, level, easy, wind);
                    let gust = n == 1;
                    let (s, hold) = if gust {
                        hold(s, gusting(wind), 85.)
                    } else {
                        hold(s, |_| wind, 85.)
                    };
                    let case = format!("{id:?} {level:?} easy={easy} wind {n} gust={gust}");
                    let captured = hold.captured_s.unwrap_or(f64::INFINITY);
                    assert!(hold.engaged && !s.crashed, "{case}: {hold:?}");
                    assert!(captured < 20., "{case}: {hold:?}");
                    assert!(hold.position_ft < 20., "{case}: {hold:?}");
                    assert!(hold.height_ft < 10., "{case}: {hold:?}");
                    assert!(hold.heading_deg < 5., "{case}: {hold:?}");
                }
            }
        }
    }
}

fn clearance(id: AircraftId) -> f64 {
    let s = State::new(&aircraft(id), [0., 1_000., 0.]).unwrap();
    s.model().configuration().equipment.ground_clearance_ft
}

/// Hover hold holds the wheels at least 10 ft above the ground: engaged
/// lower, it climbs to 10 ft and stays there, in ground effect.
#[test]
fn hover_hold_holds_at_least_ten_feet_above_the_ground() {
    for id in HOVERING {
        let clearance = clearance(id);
        let mut s = flight(id, clearance + 5., 0., StabilityLevel::Damper, false);
        assert!(s.trim_hover(), "{id:?}");
        s.step_surface(&press(Switch::HoverHold), steady(CALM));
        assert_eq!(s.autopilot.mode(), Mode::Hover, "{id:?}");
        for _ in 0..120 * 30 {
            s.step_surface(&PilotInput::default(), steady(CALM));
        }
        let wheels = s.position[1] - clearance;
        assert!((wheels - 10.).abs() < 1., "{id:?} wheels at {wheels} ft");
        assert_eq!(s.autopilot.mode(), Mode::Hover, "{id:?}");
    }
}

/// A2: hover hold is refused, with its message, above 40 kt, on the ground,
/// on the vectoring jets and a fixed-wing aircraft, on the legacy adapter,
/// and on the V-22 with its nacelles below 75 degrees; and without engine
/// power or hydraulics.
#[test]
fn a2_hover_hold_refusals_say_why() {
    let refused = |mut s: State, message: &str| {
        let before = s.autopilot.mode();
        s.step_surface(&press(Switch::HoverHold), steady(CALM));
        assert_eq!(s.autopilot.mode(), before, "{message}");
        assert_eq!(
            messages(&s, message),
            1,
            "{message}: {:?}",
            s.systems.messages
        );
        assert_eq!(messages(&s, HOVER_HOLD_ENGAGED), 0, "{message}");
    };
    for id in HOVERING {
        refused(
            moving(id, 500., 45., 0., StabilityLevel::Damper, false, CALM),
            "Hover hold needs less than 40 knots",
        );
        let mut parked = flight(id, 0., 0., StabilityLevel::Damper, false);
        parked.start_on_runway([0.; 3], 0.).unwrap();
        refused(parked, "Hover hold is not available on the ground");
        let mut cut = moving(id, 500., 0., 0., StabilityLevel::Damper, false, CALM);
        cut.engine = false;
        refused(cut, "Hover hold needs engine power");
        let mut dry = moving(id, 500., 0., 0., StabilityLevel::Damper, false, CALM);
        dry.systems.fluids.hydraulic = 0.;
        refused(dry, "Hover hold needs hydraulic power");
    }
    for id in [AircraftId::Av8, AircraftId::Yak141] {
        let mut s = flight(id, 500., 0., StabilityLevel::Damper, false);
        assert!(s.trim_hover(), "{id:?}");
        refused(s, "Hover hold is not available on this aircraft");
    }
    let mut fixed =
        State::new(&crate::flight::integration_tests::profile(), [0., 500., 0.]).unwrap();
    fixed.enable_research(1).unwrap();
    refused(fixed, "Hover hold is not available on this aircraft");
    let mut v22 = flight(AircraftId::V22, 500., 0., StabilityLevel::Damper, false);
    assert!(v22.trim_tiltrotor(140. * KT, 60.));
    refused(v22, "Hover hold needs the nacelles at 75 degrees or more");
    // On the legacy adapter the rotorcraft have no rotor physics to hold.
    let legacy = State::new(&aircraft(AircraftId::Ah64), [0., 500., 0.]).unwrap();
    refused(legacy, "Hover hold is not available on this aircraft");
}

/// An `id` holding a hover in calm air, 30 s after engagement.
fn holding(id: AircraftId) -> State {
    let mut s = moving(id, 500., 0., 0., StabilityLevel::Damper, false, CALM);
    s.step_surface(&press(Switch::HoverHold), steady(CALM));
    for _ in 0..120 * 30 {
        s.step_surface(&PilotInput::default(), steady(CALM));
    }
    assert_eq!(s.autopilot.mode(), Mode::Hover);
    assert!(s.autopilot.hover_point().is_some());
    s.systems.messages.clear();
    s
}

/// One tick of `input` over flat ground at `height` turns hover hold off
/// with its message, for `why`.
fn cancels(name: &str, mut s: State, input: PilotInput, height: f64, why: Release) {
    s.step_surface(&input, ground(height, CALM));
    assert_eq!(s.autopilot.mode(), Mode::Off, "{name}");
    assert_eq!(messages(&s, HOVER_HOLD_OFF), 1, "{name}");
    assert_eq!(
        s.trace().autopilot.and_then(|a| a.released),
        Some(why),
        "{name}"
    );
}

/// A3: each of the stick or pedal past 0.15, a collective or throttle key,
/// the collective keys' rate, a collective or throttle lever moving more
/// than 2 percent, ground contact, an engine failure and the loss of the
/// hydraulics turns hover hold off on that tick with its message, as does
/// Ctrl+Alt+A again; the cyclic and pedal trim keys do not, nor does a
/// lever held still.
#[test]
fn a3_hover_hold_cancels_on_that_tick() {
    let s = holding(AircraftId::Ah64);
    let stick = |pitch, roll, yaw| PilotInput {
        pitch,
        roll,
        yaw,
        ..Default::default()
    };
    cancels(
        "pitch",
        s.clone(),
        stick(0.16, 0., 0.),
        0.,
        Release::PilotOverride,
    );
    cancels(
        "roll",
        s.clone(),
        stick(0., -0.16, 0.),
        0.,
        Release::PilotOverride,
    );
    cancels(
        "pedal",
        s.clone(),
        stick(0., 0., 0.16),
        0.,
        Release::PilotOverride,
    );
    for (name, key) in [
        (
            "collective step",
            PilotCommand::AdjustAxis(FlightAxis::Collective, 0.05),
        ),
        (
            "collective set",
            PilotCommand::SetAxis(FlightAxis::Collective, 0.6),
        ),
        ("throttle key", PilotCommand::Throttle(0.8)),
        ("throttle step", PilotCommand::AdjustThrottle(-0.05)),
    ] {
        cancels(name, s.clone(), command(key), 0., Release::PilotCollective);
    }
    for (name, input) in [
        (
            "collective keys",
            PilotInput {
                collective_rate: 1.,
                ..Default::default()
            },
        ),
        (
            "throttle keys",
            PilotInput {
                throttle_rate: -1.,
                ..Default::default()
            },
        ),
    ] {
        cancels(name, s.clone(), input, 0., Release::PilotCollective);
    }
    // A lever held still keeps the hold; moving it 2 percent cancels.
    for throttle in [false, true] {
        let lever = |value| PilotInput {
            collective: (!throttle).then_some(value),
            throttle: throttle.then_some(value),
            ..Default::default()
        };
        let mut still = s.clone();
        for _ in 0..240 {
            still.step_surface(&lever(0.6), steady(CALM));
        }
        still.step_surface(&lever(0.619), steady(CALM));
        assert_eq!(still.autopilot.mode(), Mode::Hover, "throttle={throttle}");
        cancels("lever", still, lever(0.621), 0., Release::PilotCollective);
    }
    // The ground rises under it.
    cancels(
        "ground",
        s.clone(),
        PilotInput::default(),
        s.position[1] - 1.,
        Release::Ground,
    );
    let mut cut = s.clone();
    cut.engine = false;
    cancels(
        "engine",
        cut,
        PilotInput::default(),
        0.,
        Release::EngineFailure,
    );
    // Without hydraulics the autopilot is unavailable altogether.
    let mut dry = s.clone();
    dry.systems.fluids.hydraulic = 0.;
    dry.step_surface(&PilotInput::default(), steady(CALM));
    assert_eq!(dry.autopilot.mode(), Mode::Off, "hydraulics");
    assert_eq!(messages(&dry, HOVER_HOLD_OFF), 1, "hydraulics");
    // Ctrl+Alt+A again turns it off.
    let mut again = s.clone();
    again.step_surface(&press(Switch::HoverHold), steady(CALM));
    assert_eq!(again.autopilot.mode(), Mode::Off, "switch");
    assert_eq!(messages(&again, HOVER_HOLD_OFF), 1, "switch");
    // The trim keys and the stick at 0.15 do not.
    let mut kept = s.clone();
    for input in [
        trim(TrimAxis::Pitch, -0.02),
        trim(TrimAxis::Roll, 0.02),
        trim(TrimAxis::Pedal, 0.02),
        command(PilotCommand::Lift(LiftCommand::TrimCentre)),
        stick(0.15, -0.15, 0.15),
    ] {
        kept.step_surface(&input, steady(CALM));
        assert_eq!(kept.autopilot.mode(), Mode::Hover, "{input:?}");
    }
    assert_eq!(messages(&kept, HOVER_HOLD_OFF), 0);
}

/// The cyclic trim keys nudge the held point 10 ft a tap along and across
/// the heading, and do not move the trim; the aircraft follows the point.
#[test]
fn trim_keys_nudge_the_held_point() {
    let mut s = holding(AircraftId::Ah64);
    let point = s.autopilot.hover_point().unwrap();
    let trim_before = s.lift_controls.aids.trim;
    let (sin, cos) = s.yaw.sin_cos();
    // Ctrl+Up three times: 30 ft forward. Ctrl+Right twice: 20 ft right.
    for _ in 0..3 {
        s.step_surface(&trim(TrimAxis::Pitch, -0.02), steady(CALM));
    }
    for _ in 0..2 {
        s.step_surface(&trim(TrimAxis::Roll, 0.02), steady(CALM));
    }
    let moved = s.autopilot.hover_point().unwrap();
    let delta = [moved[0] - point[0], moved[1] - point[1]];
    let forward = delta[0] * sin + delta[1] * cos;
    let right = delta[0] * cos - delta[1] * sin;
    assert!(
        (forward - 30.).abs() < 0.1 && (right - 20.).abs() < 0.1,
        "{delta:?}"
    );
    assert_eq!(s.lift_controls.aids.trim, trim_before);
    for _ in 0..120 * 30 {
        s.step_surface(&PilotInput::default(), steady(CALM));
    }
    let off = (moved[0] - s.position[0]).hypot(moved[1] - s.position[2]);
    assert!(off < 3., "{off} ft from the nudged point");
}

/// A4: hover hold flies only through the controls. The inputs the flight
/// model received while it held, replayed by hand on a copy without the
/// autopilot, give a bit-identical flight, gusts and all.
#[test]
fn a4_hover_hold_flies_only_through_the_controls() {
    let mean = [10. * KT, 0., -6. * KT];
    let wind = gusting(mean);
    for id in HOVERING {
        let start = moving(id, 400., 25., 0.6, StabilityLevel::Damper, false, mean);
        let mut held = start.clone();
        let mut by_hand = start;
        let mut tape = Vec::new();
        for tick in 0..1_800 {
            let input = if tick == 0 {
                press(Switch::HoverHold)
            } else {
                PilotInput::default()
            };
            held.step_surface(&input, steady(wind(held.ticks)));
            let flown = held.trace().autopilot.expect("engaged");
            assert_eq!(flown.mode, Mode::Hover, "{id:?} tick {tick}");
            tape.push(PilotInput {
                pitch: flown.commanded[0],
                roll: flown.commanded[1],
                yaw: flown.commanded[2],
                collective: flown.collective,
                ..Default::default()
            });
        }
        assert!(held.autopilot.hover_point().is_some(), "{id:?} holds");
        for input in &tape {
            by_hand.step_surface(input, steady(wind(by_hand.ticks)));
            assert_eq!(by_hand.autopilot.mode(), Mode::Off);
        }
        assert_eq!(held.position, by_hand.position, "{id:?}");
        assert_eq!(held.velocity, by_hand.velocity, "{id:?}");
        assert_eq!(
            [held.yaw, held.pitch, held.bank],
            [by_hand.yaw, by_hand.pitch, by_hand.bank],
            "{id:?}"
        );
        assert_eq!(held.lift_controls, by_hand.lift_controls, "{id:?}");
        assert_eq!(held.fuel, by_hand.fuel, "{id:?}");
    }
}

/// A hold restores exactly mid-hold, from the wire's exact coding (alone
/// and against a baseline) and from a checkpoint, then flies on bit for bit
/// for 1,200 ticks, still holding.
#[test]
fn hover_hold_restores_exactly_mid_hold() {
    use crate::checkpoint::{Loader, Models, Saver, load_flight, save_flight};
    let wind = [8. * KT, 0., 8. * KT];
    for id in HOVERING {
        let model = AircraftModel::for_aircraft(&aircraft(id)).unwrap();
        let mut s = moving(id, 300., 30., 1., StabilityLevel::Damper, false, wind);
        s.step_surface(&press(Switch::HoverHold), steady(wind));
        for _ in 0..240 {
            s.step_surface(&PilotInput::default(), steady(wind));
        }
        let baseline = s.clone();
        // Mid-hold: still braking the drift, then holding the point.
        for ticks in [200, 2_400] {
            let mut original = baseline.clone();
            for _ in 0..ticks {
                original.step_surface(&PilotInput::default(), steady(wind));
            }
            assert_eq!(original.autopilot.mode(), Mode::Hover, "{id:?}");
            assert_eq!(
                original.autopilot.hover_point().is_some(),
                ticks > 1_000,
                "{id:?} after {ticks}"
            );
            let mut copies = Vec::new();
            for base in [None, Some(&baseline)] {
                let mut w = tore_codec::BitWriter::new();
                original.write_exact(&mut w, base).unwrap();
                let bytes = w.as_bytes().to_vec();
                let r = &mut tore_codec::BitReader::new(&bytes);
                copies.push(State::read_exact(r, base, &model).unwrap());
            }
            let mut models = Models::default();
            models.insert(id, model.clone()).unwrap();
            let mut saver = Saver::with_models(models.clone());
            save_flight(&mut saver, &original, id).unwrap();
            let body = saver.finish_section();
            let mut loader = Loader::new(&body, &[], &models);
            copies.push(load_flight(&mut loader).unwrap().1);
            loader.finish().unwrap();
            for copy in &copies {
                assert_eq!(*copy, original, "{id:?} after {ticks}");
            }
            for tick in 0..1_200 {
                original.step_surface(&PilotInput::default(), steady(wind));
                for copy in &mut copies {
                    copy.step_surface(&PilotInput::default(), steady(wind));
                    assert_eq!(*copy, original, "{id:?} after {ticks}, tick {tick}");
                }
            }
            assert_eq!(original.autopilot.mode(), Mode::Hover, "{id:?}");
        }
    }
}

/// Hover hold and the rotorcraft A modes meet at 40 kt: below it A and
/// Ctrl+A are refused (and cannot replace hover hold), above it Ctrl+Alt+A
/// is refused; Ctrl+Alt+A replaces an A mode once under 40 kt.
#[test]
fn hover_hold_and_the_a_modes_meet_at_forty_knots() {
    let mut fast = moving(
        AircraftId::Ah64,
        1_000.,
        41.,
        0.,
        StabilityLevel::Damper,
        false,
        CALM,
    );
    fast.step_surface(&press(Switch::Autopilot), steady(CALM));
    assert_eq!(fast.autopilot.mode(), Mode::Heading);
    fast.step_surface(&press(Switch::HoverHold), steady(CALM));
    assert_eq!(fast.autopilot.mode(), Mode::Heading, "41 kt is too fast");
    let mut slow = moving(
        AircraftId::Ah64,
        1_000.,
        39.,
        0.,
        StabilityLevel::Damper,
        false,
        CALM,
    );
    for switch in [Switch::Autopilot, Switch::WaypointAutopilot] {
        let mut s = slow.clone();
        s.step_surface(&press(switch), steady(CALM));
        assert_eq!(s.autopilot.mode(), Mode::Off);
        assert_eq!(
            messages(&s, "Autopilot needs 40 knots; Ctrl+Alt+A holds a hover"),
            1
        );
    }
    slow.step_surface(&press(Switch::HoverHold), steady(CALM));
    assert_eq!(slow.autopilot.mode(), Mode::Hover);
    slow.step_surface(&press(Switch::Autopilot), steady(CALM));
    assert_eq!(
        slow.autopilot.mode(),
        Mode::Hover,
        "A is refused below 40 kt"
    );
    let mut s = fast;
    s.velocity = s.velocity.map(|v| v * 30. / 41.);
    s.systems.messages.clear();
    s.step_surface(&press(Switch::HoverHold), steady(CALM));
    assert_eq!(s.autopilot.mode(), Mode::Hover);
    assert_eq!(messages(&s, HOVER_HOLD_ENGAGED), 1);
}

/// The vectoring jets fly the fixed-wing heading and waypoint law at flying
/// speed, refuse it below their 1 G stall speed and let go below 85 percent
/// of it.
#[test]
fn the_jets_need_flying_speed_for_the_a_modes() {
    for id in [AircraftId::Av8, AircraftId::Yak141] {
        let mut hovering = flight(id, 3_000., 0., StabilityLevel::Damper, false);
        assert!(hovering.trim_hover());
        hovering.step_surface(&press(Switch::Autopilot), steady(CALM));
        assert_eq!(hovering.autopilot.mode(), Mode::Off, "{id:?}");
        assert_eq!(messages(&hovering, "Autopilot needs flying speed"), 1);
        let mut s = flight(id, 3_000., 0., StabilityLevel::Damper, false);
        assert!(s.start_airborne([0.; 3]));
        s.step_surface(&press(Switch::Autopilot), steady(CALM));
        assert_eq!(s.autopilot.mode(), Mode::Heading, "{id:?}");
        let stall = s
            .model()
            .configuration()
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .and_then(|e| e.speeds(s.position[1]))
            .unwrap()
            .0;
        let scale = 0.8 * stall / s.speed;
        s.velocity = s.velocity.map(|v| v * scale);
        s.speed *= scale;
        s.step_surface(&PilotInput::default(), steady(CALM));
        assert_eq!(s.autopilot.mode(), Mode::Off, "{id:?}");
        assert_eq!(
            s.trace().autopilot.unwrap().released,
            Some(Release::TooSlow)
        );
        assert_eq!(messages(&s, "Autopilot off: too slow"), 1);
    }
}

/// The heading and waypoint modes on the six powered-lift aircraft, from
/// their airborne starts (trimmed forward flight) at every stability level
/// and with the Easy flight physics cheat: upset by 20 degrees of bank and
/// 100 ft low, the heading mode is within 30 ft of its altitude from 30 s on
/// and on its heading within 3 degrees after 120 s; the waypoint mode turns
/// about 45 degrees onto a waypoint, heading for it within 3 degrees after
/// 120 s, and holds its
/// altitude within 50 ft throughout (the fixed-wing acceptance of
/// docs/spec/autopilot.md, with tighter altitude numbers). The speed stays
/// within 10 kt.
#[test]
fn the_a_modes_fly_the_six_powered_lift_aircraft() {
    for id in POWERED {
        for level in StabilityLevel::ALL {
            for easy in [false, true] {
                for nav in [false, true] {
                    let mut s = flight(id, 3_000., 0., level, easy);
                    assert!(s.start_airborne([0.; 3]), "{id:?}");
                    let speed = s.speed;
                    let switch = if nav {
                        Switch::WaypointAutopilot
                    } else {
                        Switch::Autopilot
                    };
                    s.step_surface(&press(switch), steady(CALM));
                    if nav {
                        s.autopilot.set_navigation_target(Some(NavigationTarget {
                            number: 1,
                            position: [100_000., 100_000.],
                        }));
                    } else {
                        s.bank = 20_f64.to_radians();
                        s.position[1] -= 100.;
                    }
                    let mut worst: f64 = 0.;
                    for tick in 0..120 * 120 {
                        s.step_surface(&PilotInput::default(), steady(CALM));
                        if nav || tick > 120 * 30 {
                            worst = worst.max((s.position[1] - 3_000.).abs());
                        }
                    }
                    let case = format!("{id:?} {level:?} easy={easy} nav={nav}");
                    let bearing = if nav {
                        (100_000. - s.position[0]).atan2(100_000. - s.position[2])
                    } else {
                        0.
                    };
                    let error = wrap(bearing - s.yaw).to_degrees();
                    assert!(!s.crashed && s.autopilot.mode() != Mode::Off, "{case}");
                    assert!(error.abs() < 3., "{case}: heading error {error}");
                    let limit = if nav { 50. } else { 30. };
                    assert!(worst < limit, "{case}: {worst} ft");
                    assert!((s.speed - speed).abs() < 10. * KT, "{case}: speed");
                }
            }
        }
    }
}

/// On the helicopters the A modes hold the speed on the cyclic and the
/// height on the collective; the pitch trim keys move the held speed 2 kt a
/// tap, and a collective input or an engine failure lets go as hover hold
/// does. The V-22 on its downstops leaves its power lever to the pilot.
#[test]
fn the_rotorcraft_a_modes_hold_speed_on_the_cyclic_and_height_on_the_collective() {
    let mut s = moving(
        AircraftId::Ah64,
        2_000.,
        80.,
        0.,
        StabilityLevel::Damper,
        false,
        CALM,
    );
    s.step_surface(&press(Switch::Autopilot), steady(CALM));
    assert_eq!(s.autopilot.mode(), Mode::Heading);
    // Ctrl+Up five times: 10 kt faster.
    for _ in 0..5 {
        s.step_surface(&trim(TrimAxis::Pitch, -0.02), steady(CALM));
    }
    for _ in 0..120 * 60 {
        s.step_surface(&PilotInput::default(), steady(CALM));
    }
    let knots = s.velocity[2] / KT;
    assert!((knots - 90.).abs() < 2., "{knots} kt");
    assert!((s.position[1] - 2_000.).abs() < 10., "{}", s.position[1]);
    assert!(s.trace().autopilot.unwrap().collective.is_some());
    let mut keyed = s.clone();
    keyed.step_surface(
        &PilotInput {
            collective_rate: -1.,
            ..Default::default()
        },
        steady(CALM),
    );
    assert_eq!(keyed.autopilot.mode(), Mode::Off);
    assert_eq!(
        keyed.trace().autopilot.unwrap().released,
        Some(Release::PilotCollective)
    );
    let mut cut = s.clone();
    cut.engine = false;
    cut.step_surface(&PilotInput::default(), steady(CALM));
    assert_eq!(cut.autopilot.mode(), Mode::Off);
    // The V-22 on its downstops: the lever is the pilot's.
    let mut v22 = flight(AircraftId::V22, 3_000., 0., StabilityLevel::Damper, false);
    assert!(v22.start_airborne([0.; 3]));
    v22.step_surface(&press(Switch::Autopilot), steady(CALM));
    let lever = v22.lift_controls.collective;
    v22.step_surface(
        &PilotInput {
            collective_rate: 1.,
            ..Default::default()
        },
        steady(CALM),
    );
    assert_eq!(v22.autopilot.mode(), Mode::Heading);
    assert!(v22.lift_controls.collective > lever);
}
