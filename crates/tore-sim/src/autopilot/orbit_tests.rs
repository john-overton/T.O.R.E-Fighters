use super::*;
use crate::autopilot::Mode;
use crate::flight::Switch;
use crate::flight::integration_tests::profile;

fn hybrid() -> State {
    let mut s = State::new(&profile(), [0., 8_000., 0.]).unwrap();
    s.enable_research(42).unwrap();
    s
}

/// Flies `seconds` with `input` from `s`, calling `each` after every tick.
fn fly(s: &mut State, seconds: f64, input: &PilotInput, mut each: impl FnMut(u64, &State)) {
    for tick in 0..(seconds * 120.) as u64 {
        s.step(input, |_, _| 0.);
        each(tick, s);
    }
}

#[test]
fn the_plan_is_never_below_the_minimum() {
    let mut s = hybrid();
    for reach in [0., 8_000., 13_000., 20_000.] {
        s.engage_orbit([10_000., 0., 0.], reach);
        let orbit = *s.autopilot.orbit().unwrap();
        let (radius, height) = orbit.minimum();
        assert!(
            orbit.radius_ft >= radius && orbit.height_ft >= height,
            "{reach}: {orbit:?}"
        );
        // The minimum keeps the anti-aircraft guns' reach and 35 degrees.
        let reach = if reach > 0. { reach } else { 13_000. };
        assert!(radius.hypot(height) >= THREAT_SLANT_FT.min(0.9 * reach) - 1e-6);
        assert!(((height / radius).atan().to_degrees() - 35.).abs() < 1e-9);
    }
    // No reach given: the shipped guns' 13,000 ft.
    s.engage_orbit([10_000., 0., 0.], 0.);
    assert_eq!(s.autopilot.orbit().unwrap().reach_ft, 13_000.);
}

/// From 8,000 ft and 3.3 nmi away the orbit settles: on the circle, every
/// gun and the camera bearing, never inside the minimum, the bank command
/// never faster than 5 degrees a second and no stall warning. The stick and
/// the throttle the pilot holds change nothing.
#[test]
fn it_settles_smoothly_and_ignores_the_stick() {
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    assert_eq!(s.autopilot.label(), "ORBIT");
    let pilot = PilotInput {
        pitch: 1.,
        roll: -1.,
        yaw: 1.,
        throttle: Some(1.),
        commands: vec![PilotCommand::Throttle(1.)],
        ..PilotInput::default()
    };
    let mut hands_off = s.clone();
    let mut last = s.autopilot.orbit().unwrap().bank;
    let (mut settled, mut bearing) = (0, 0);
    fly(&mut s, 400., &pilot, |tick, s| {
        let orbit = s.autopilot.orbit().expect("still engaged");
        assert!((orbit.bank - last).abs() <= BANK_RATE_DEG_S.to_radians() / 120. + 1e-12);
        last = orbit.bank;
        assert!(s.stall_alert(0.).is_none(), "stall at {tick}");
        if tick >= 300 * 120 {
            let status = s.orbit_status().unwrap();
            settled += u32::from(status.on_orbit);
            bearing += u32::from(status.all_bearing());
            assert!(status.distance_ft >= status.minimum_radius_ft);
            assert!(status.height_now_ft >= status.minimum_height_ft);
        }
    });
    assert_eq!((settled, bearing), (100 * 120, 100 * 120));
    assert!(!s.crashed);
    fly(&mut hands_off, 400., &PilotInput::default(), |_, _| {});
    assert_eq!(
        s.position, hands_off.position,
        "the stick changed the flight"
    );
}

#[test]
fn a_mode_switch_or_the_ground_ends_it_and_the_point_moves() {
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    s.autopilot.set_orbit_point([12_000., 0., 18_000.]);
    s.autopilot.set_orbit_point([f64::NAN, 0., 0.]);
    s.step(&PilotInput::default(), |_, _| 0.);
    let orbit = *s.autopilot.orbit().unwrap();
    assert_eq!(orbit.point, [12_000., 0., 18_000.]);
    // The centre slides 200 ft/s toward the new point.
    assert!((orbit.centre[2] - (16_000. + CENTRE_SPEED_FPS / 120.)).abs() < 1e-6);
    s.step(
        &PilotInput {
            commands: vec![PilotCommand::Toggle(Switch::Autopilot)],
            ..PilotInput::default()
        },
        |_, _| 0.,
    );
    assert_eq!(s.autopilot.mode(), Mode::Heading);
    assert!(s.autopilot.orbit().is_none() && s.orbit_status().is_none());
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    s.step(&PilotInput::default(), |_, _| 20_000.);
    assert_eq!(s.autopilot.mode(), Mode::Off);
    assert!(s.orbit_refusal().is_some());
}

/// The orbit rides in the exact state, so a client's prediction and a
/// checkpoint carry on the same orbit.
#[test]
fn the_orbit_survives_the_exact_state() {
    use tore_codec::{BitReader, BitWriter};
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    fly(&mut s, 20., &PilotInput::default(), |_, _| {});
    let model = s.model().clone();
    let mut w = BitWriter::new();
    s.write_exact(&mut w, None).unwrap();
    let mut copy = State::read_exact(&mut BitReader::new(&w.finish()), None, &model).unwrap();
    assert_eq!(copy.autopilot, s.autopilot);
    fly(&mut s, 5., &PilotInput::default(), |_, _| {});
    fly(&mut copy, 5., &PilotInput::default(), |_, _| {});
    assert_eq!(copy.position, s.position);
}

/// The guns and the camera on a steady AC-130 orbit: the centre at 35
/// degrees of depression abeam left, 21 degrees of bank, bears for all
/// three guns and the camera; the same point off the nose or above the
/// wing line does not.
#[test]
fn bearing_follows_the_arcs() {
    let mut s = hybrid();
    s.position = [0., 5_965., 0.];
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = -21_f64.to_radians();
    // Abeam left (west, -x) 8,519 ft.
    let centre = [-8_519., 0., 0.];
    assert_eq!(bearing(&s, centre, 13_000.), ([true; 3], true));
    // Out of reach.
    assert_eq!(bearing(&s, centre, 9_000.).0, [false; 3]);
    // Ahead: outside every heading arc.
    assert_eq!(bearing(&s, [0., 0., 8_519.], 13_000.).0, [false; 3]);
    // Banked so far the point lies above the wing line: the camera loses it.
    s.bank = -60_f64.to_radians();
    assert!(!bearing(&s, centre, 13_000.).1);
}
