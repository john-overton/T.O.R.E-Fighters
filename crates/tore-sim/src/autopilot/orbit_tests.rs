use super::super::rotor_law::angle;
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
    s.autopilot.set_orbit_point([12_000., 0., 16_500.], true);
    s.autopilot.set_orbit_point([f64::NAN, 0., 0.], true);
    s.step(&PilotInput::default(), |_, _| 0.);
    let orbit = *s.autopilot.orbit().unwrap();
    assert_eq!(orbit.point, [12_000., 0., 16_500.]);
    // A track: the centre slides 200 ft/s toward the new point at once.
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

/// The search (John, 2026-10-14: released, the autopilot flies straight):
/// the console opened with a free sight takes the aircraft's current
/// heading, altitude and speed and holds them, wings level, and says SEARCH.
#[test]
fn the_search_flies_straight_and_level() {
    let mut s = hybrid();
    let expected = s.yaw;
    s.engage_console(13_000.);
    assert_eq!(s.autopilot.orbit().unwrap().speed_fps, s.speed);
    assert_eq!(s.autopilot.mode(), Mode::Orbit);
    assert_eq!(s.autopilot.label(), "SEARCH");
    assert!(s.orbit_status().is_none());
    let status = s.console_status().unwrap();
    assert_eq!(status.phase, ConsolePhase::Search);
    assert!((status.altitude_ft - 8_000.).abs() < 1e-9);
    let mut last = s.autopilot.orbit().unwrap().bank;
    fly(&mut s, 120., &PilotInput::default(), |tick, s| {
        let bank = s.autopilot.orbit().unwrap().bank;
        assert!((bank - last).abs() <= BANK_RATE_DEG_S.to_radians() / 120. + 1e-12);
        last = bank;
        assert!(s.stall_alert(0.).is_none(), "stall at {tick}");
    });
    let speed = s.autopilot.orbit().unwrap().speed_fps;
    assert!(
        angle(s.yaw - expected).abs() < 1_f64.to_radians(),
        "heading"
    );
    assert!(
        (s.position[1] - 8_000.).abs() < 100.,
        "altitude {}",
        s.position[1]
    );
    assert!(s.bank.abs() < 2_f64.to_radians(), "bank {}", s.bank);
    // The test aircraft idles faster than the planned speed at this height:
    // the throttle sits at idle then.
    assert!(
        (s.speed - speed).abs() < 5. * KT || (s.throttle == 0. && s.speed > speed),
        "speed {} of {speed}, throttle {}",
        s.speed,
        s.throttle
    );
}

/// The console's heading, altitude and speed keys step the search's
/// commanded values; the aircraft turns onto the new heading without
/// overshooting it, climbs to the altitude and holds the speed.
#[test]
fn the_keys_step_the_search() {
    let mut s = hybrid();
    s.engage_console(13_000.);
    let start = s.console_status().unwrap();
    let message = s.console_command(ConsoleCommand::Heading(6)).unwrap();
    let wanted = (start.heading_deg / 5.).round() * 5. + 30.;
    assert_eq!(
        message,
        format!("Search heading {:03.0}", wanted.rem_euclid(360.))
    );
    assert_eq!(
        s.console_command(ConsoleCommand::Altitude(2)).unwrap(),
        "Search altitude 9000 ft"
    );
    let band = crate::ai::mission::speed_limits_of(&s);
    s.console_command(ConsoleCommand::Speed(100));
    assert!((s.autopilot.orbit().unwrap().speed_fps - band.maximum.0).abs() < 1e-9);
    s.console_command(ConsoleCommand::Speed(0));
    assert!((s.autopilot.orbit().unwrap().speed_fps - band.corner.0).abs() < 1e-9);
    s.console_command(ConsoleCommand::Speed(50));
    let half = s.autopilot.orbit().unwrap().speed_fps;
    s.console_command(ConsoleCommand::SpeedStep(2));
    let stepped = s.autopilot.orbit().unwrap().speed_fps;
    assert!((stepped / KT - ((half / KT / 5.).round() * 5. + 10.)).abs() < 1e-9);
    let status = s.console_status().unwrap();
    assert!((status.heading_deg - wanted.rem_euclid(360.)).abs() < 1e-9);
    assert_eq!(status.altitude_ft, 9_000.);
    let target = status.heading_deg.to_radians();
    let mut overshoot: f64 = 0.;
    let mut max_bank: f64 = 0.;
    fly(&mut s, 150., &PilotInput::default(), |_, s| {
        // Past the heading on the far side (the turn is to the right).
        overshoot = overshoot.max(angle(s.yaw - target));
        max_bank = max_bank.max(s.bank.abs());
        assert!(s.stall_alert(0.).is_none());
    });
    assert!(overshoot < 1_f64.to_radians(), "overshoot {overshoot}");
    assert!(
        max_bank < (SEARCH_BANK_DEG + 3.).to_radians(),
        "bank {max_bank}"
    );
    assert!(angle(s.yaw - target).abs() < 0.5_f64.to_radians());
    assert!(
        (s.position[1] - 9_000.).abs() < 100.,
        "altitude {}",
        s.position[1]
    );
    assert!(
        (s.speed - stepped).abs() < 5. * KT,
        "speed {}",
        s.speed / KT
    );
    // The bounds: altitude and speed stay in their ranges.
    s.console_command(ConsoleCommand::Altitude(i8::MAX));
    assert_eq!(
        s.console_status().unwrap().altitude_ft,
        SEARCH_ALTITUDE_FT[1]
    );
    s.console_command(ConsoleCommand::SpeedStep(i8::MIN));
    assert!(s.autopilot.orbit().unwrap().speed_fps >= band.corner.0 - 1e-9);
}

/// Pinning joins the orbit about the point; letting it go takes the current
/// heading, altitude and speed as the commanded ones (John, 2026-10-14) and
/// settles back onto that heading after the roll-out, the bank never moving
/// faster than its slew.
#[test]
fn pin_joins_the_orbit_and_release_rolls_out_to_the_search() {
    let mut s = hybrid();
    s.engage_console(13_000.);
    fly(&mut s, 5., &PilotInput::default(), |_, _| {});
    s.autopilot.set_orbit_point([12_000., 0., 16_000.], false);
    assert_eq!(s.autopilot.label(), "ORBIT");
    let status = s.console_status().unwrap();
    assert_eq!(status.phase, ConsolePhase::Join);
    let orbit = *s.autopilot.orbit().unwrap();
    // Joining from the search puts the centre on the point at once.
    assert_eq!(orbit.point, [12_000., 0., 16_000.]);
    assert_eq!(orbit.centre, orbit.point);
    let mut last = orbit.bank;
    let mut settled = false;
    fly(&mut s, 300., &PilotInput::default(), |_, s| {
        let bank = s.autopilot.orbit().unwrap().bank;
        assert!((bank - last).abs() <= BANK_RATE_DEG_S.to_radians() / 120. + 1e-12);
        last = bank;
        settled |= s.console_status().unwrap().phase == ConsolePhase::Orbit;
    });
    assert!(settled, "the orbit was joined");
    assert!(s.bank.abs() > 10_f64.to_radians(), "circling banked");
    let expected = s.yaw.rem_euclid(std::f64::consts::TAU);
    let (altitude, speed) = (s.position[1], s.speed);
    s.release_orbit();
    let status = s.console_status().unwrap();
    assert_eq!(status.phase, ConsolePhase::Search);
    assert!(status.orbit.is_none() && s.orbit_status().is_none());
    assert!((status.heading_deg.to_radians() - expected).abs() < 1e-9);
    assert_eq!(s.autopilot.orbit().unwrap().speed_fps, speed);
    let mut worst: f64 = 0.;
    fly(&mut s, 60., &PilotInput::default(), |_, s| {
        let bank = s.autopilot.orbit().unwrap().bank;
        assert!((bank - last).abs() <= BANK_RATE_DEG_S.to_radians() / 120. + 1e-12);
        last = bank;
        // Past the release heading (the orbit turns left): the roll-out
        // still turns a few degrees, then the search turns back.
        worst = worst.max(-angle(s.yaw - expected));
    });
    assert!(
        // 4 to 5 degrees on the AC-130; this test aircraft is faster and
        // banks harder in its orbit.
        worst < rollout_turn(30_f64.to_radians(), 300.).abs() + 1_f64.to_radians(),
        "turned past the release heading by {worst}"
    );
    assert!(angle(s.yaw - expected).abs() < 1_f64.to_radians());
    assert!(s.bank.abs() < 2_f64.to_radians());
    assert!((s.position[1] - altitude).abs() < 150.);
}

/// In the orbit the altitude keys move its height above the point within the
/// camera's lowest and the guns' highest, the heading keys say why they do
/// nothing, and the speed stays where the guns still reach.
#[test]
fn the_keys_in_the_orbit_keep_its_limits() {
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    let orbit = *s.autopilot.orbit().unwrap();
    let (low, high) = orbit.height_range();
    assert!(
        (orbit.height_ft - low).abs() < 1e-6,
        "the plan sits on the camera's lowest"
    );
    let message = s.console_command(ConsoleCommand::Altitude(-1)).unwrap();
    assert!(message.ends_with("the camera's lowest"), "{message}");
    s.console_command(ConsoleCommand::Altitude(2));
    let orbit = *s.autopilot.orbit().unwrap();
    assert!(
        (orbit.height_ft - (low + 1_000.).min(high)).abs() < 1e-6,
        "{orbit:?} {low} {high}"
    );
    let message = s.console_command(ConsoleCommand::Altitude(100)).unwrap();
    assert!(message.ends_with("the guns' highest"), "{message}");
    assert!((s.autopilot.orbit().unwrap().height_ft - high).abs() < 1e-6);
    let before = *s.autopilot.orbit().unwrap();
    let message = s.console_command(ConsoleCommand::Heading(3)).unwrap();
    assert!(message.starts_with("Orbiting"), "{message}");
    assert_eq!(*s.autopilot.orbit().unwrap(), before);
    s.console_command(ConsoleCommand::Speed(100));
    let orbit = *s.autopilot.orbit().unwrap();
    assert!(orbit.speed_fps <= orbit.orbit_speed_cap() + 1e-9);
    let (radius, _) = orbit.minimum();
    assert!(orbit.radius_ft >= radius - 1e-9);
    assert!(orbit.radius_ft.hypot(orbit.height_range().0) <= 0.9 * 13_000. + 1e-6);
    // With the console closed the keys do nothing.
    s.autopilot.disengage();
    assert!(s.console_command(ConsoleCommand::Altitude(1)).is_none());
}

/// The search's commanded values ride in the exact state too.
#[test]
fn the_search_survives_the_exact_state() {
    use tore_codec::{BitReader, BitWriter};
    let mut s = hybrid();
    s.engage_console(13_000.);
    s.console_command(ConsoleCommand::Heading(-4));
    s.console_command(ConsoleCommand::Altitude(3));
    fly(&mut s, 10., &PilotInput::default(), |_, _| {});
    let model = s.model().clone();
    let mut w = BitWriter::new();
    s.write_exact(&mut w, None).unwrap();
    let mut copy = State::read_exact(&mut BitReader::new(&w.finish()), None, &model).unwrap();
    assert_eq!(copy.autopilot, s.autopilot);
    fly(&mut s, 5., &PilotInput::default(), |_, _| {});
    fly(&mut copy, 5., &PilotInput::default(), |_, _| {});
    assert_eq!(copy.position, s.position);
}

/// A slewed pin (John, 2026-10-14): the sight's point moves with the slew,
/// but the orbit's centre stays put until the pin has stood still 5 s, then
/// slides to it; a track is followed at once.
#[test]
fn a_moved_pin_moves_the_centre_after_a_five_second_pause() {
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    // A slew: the pin moves a little every tick for two seconds.
    for tick in 0..240 {
        let z = 16_000. + f64::from(tick) * 5.;
        s.autopilot.set_orbit_point([12_000., 0., z], false);
        s.step(&PilotInput::default(), |_, _| 0.);
        assert_eq!(s.autopilot.orbit().unwrap().centre[2], 16_000., "{tick}");
    }
    let pin = s.autopilot.orbit().unwrap().point;
    // Standing still: nothing until five seconds have passed...
    for _ in 0..(CENTRE_PAUSE_S * 120.) as u32 - 2 {
        s.autopilot.set_orbit_point(pin, false);
        s.step(&PilotInput::default(), |_, _| 0.);
    }
    assert_eq!(s.autopilot.orbit().unwrap().centre[2], 16_000.);
    // ...then the centre slides at its speed.
    for _ in 0..4 {
        s.autopilot.set_orbit_point(pin, false);
        s.step(&PilotInput::default(), |_, _| 0.);
    }
    let centre = s.autopilot.orbit().unwrap().centre[2];
    assert!(centre > 16_000. && centre <= 16_000. + 4. * CENTRE_SPEED_FPS / 120. + 1e-9);
    // A track moves the centre on the next tick.
    let before = s.autopilot.orbit().unwrap().centre;
    s.autopilot
        .set_orbit_point([pin[0] + 3_000., 0., pin[2]], true);
    s.step(&PilotInput::default(), |_, _| 0.);
    assert!(s.autopilot.orbit().unwrap().centre[0] > before[0]);
}

/// A new point far from the old one (the next target, a new pin) is joined
/// at once rather than slid to, and the phase reads JOIN until the aircraft
/// is on the circle about the centre and the centre is on the point
/// (nr-conb saw ORBIT while a far target was still 5 nm out).
#[test]
fn a_far_new_point_is_joined_and_the_phase_waits_for_the_circle() {
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    fly(&mut s, 300., &PilotInput::default(), |_, _| {});
    assert_eq!(s.console_status().unwrap().phase, ConsolePhase::Orbit);
    // The next target, 5 nm away.
    let far = [12_000. + 30_000., 0., 16_000.];
    s.autopilot.set_orbit_point(far, true);
    let orbit = *s.autopilot.orbit().unwrap();
    assert_eq!(orbit.centre, far, "a new point is joined at once");
    let mut seen_orbit_far = false;
    fly(&mut s, 120., &PilotInput::default(), |_, s| {
        let status = s.console_status().unwrap();
        let away = (s.position[0] - far[0]).hypot(s.position[2] - far[2]);
        if status.phase == ConsolePhase::Orbit && away > 2. * status.orbit.unwrap().radius_ft {
            seen_orbit_far = true;
        }
    });
    assert!(!seen_orbit_far, "ORBIT while still far from the new point");
    // A track moving a little stays a slide: the centre is not snapped.
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    s.autopilot
        .set_orbit_point([12_000., 0., 16_000. + NEW_POINT_FT / 2.], true);
    assert_eq!(s.autopilot.orbit().unwrap().centre, [12_000., 0., 16_000.]);
    // While the centre still slides toward the point the phase is JOIN even
    // on the circle about the centre.
    let mut s = hybrid();
    s.engage_orbit([12_000., 0., 16_000.], 13_000.);
    fly(&mut s, 300., &PilotInput::default(), |_, _| {});
    let radius = s.autopilot.orbit().unwrap().radius_ft;
    let mut moved = s.autopilot.orbit().unwrap().point;
    for _ in 0..(radius / 600.) as u32 + 2 {
        moved[0] += 600.;
        s.autopilot.set_orbit_point(moved, true);
    }
    assert_eq!(s.console_status().unwrap().phase, ConsolePhase::Join);
}
