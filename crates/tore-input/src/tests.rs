use crate::*;
fn resolver(lines: &str) -> Resolver {
    Resolver::new(Profile::parse(&format!("tore-input 1\n{lines}")).unwrap())
}
fn event(r: &mut Resolver, d: &str, c: &str, v: f64, baseline: bool) {
    r.event(Event {
        device: d.into(),
        control: c.into(),
        value: v,
        baseline,
    });
}
fn frame(r: &mut Resolver) -> PilotInput {
    r.frame(0.7).0
}
#[test]
fn short_taps_are_ordered_and_repeats_do_not_toggle() {
    let mut r = resolver("bind stick b gear press");
    event(&mut r, "stick", "b", 0., true);
    event(&mut r, "stick", "b", 1., false);
    event(&mut r, "stick", "b", 1., false);
    event(&mut r, "stick", "b", 0., false);
    event(&mut r, "stick", "b", 1., false);
    event(&mut r, "stick", "b", 0., false);
    assert_eq!(r.drain().len(), 2);
    assert!(r.drain().is_empty());
}
#[test]
fn held_buttons_aggregate_and_disconnect_only_removes_that_source() {
    let mut r = resolver("bind * b airbrake hold\nbind * r roll positive");
    for d in ["one", "two"] {
        for c in ["b", "r"] {
            event(&mut r, d, c, 0., true);
            event(&mut r, d, c, 1., false);
        }
    }
    assert_eq!(frame(&mut r).roll, 1.);
    assert_eq!(
        frame(&mut r).commands,
        vec![PilotCommand::Set(Switch::Airbrake, true)]
    );
    assert!(r.disconnect("one"));
    assert_eq!(frame(&mut r).roll, 1.);
    assert_eq!(
        frame(&mut r).commands,
        vec![PilotCommand::Set(Switch::Airbrake, true)]
    );
    r.disconnect("two");
    assert_eq!(frame(&mut r).roll, 0.);
    assert_eq!(
        frame(&mut r).commands,
        vec![PilotCommand::Set(Switch::Airbrake, false)]
    );
}
#[test]
fn maintained_switch_baseline_and_disconnect_never_toggle() {
    let mut r = resolver("bind box gear gear switch");
    event(&mut r, "box", "gear", 1., true);
    assert!(r.drain().is_empty());
    event(&mut r, "box", "gear", 0., false);
    assert_eq!(
        r.drain(),
        vec![(
            "box".into(),
            Action::Pilot(PilotCommand::Set(Switch::Gear, false))
        )]
    );
    r.disconnect("box");
    assert!(r.drain().is_empty());
    assert!(frame(&mut r).commands.is_empty());
    event(&mut r, "box", "gear", 1., true);
    assert!(r.drain().is_empty());
}
#[test]
fn follow_position_is_idempotent_and_relinquishes_on_disconnect() {
    let mut r = resolver("bind box gear gear follow");
    event(&mut r, "box", "gear", 1., true);
    r.drain();
    assert_eq!(
        frame(&mut r).commands,
        vec![PilotCommand::Set(Switch::Gear, true)]
    );
    r.context(true, true);
    assert!(frame(&mut r).commands.is_empty());
    r.context(false, true);
    assert_eq!(
        frame(&mut r).commands,
        vec![PilotCommand::Set(Switch::Gear, true)]
    );
    r.disconnect("box");
    assert!(frame(&mut r).commands.is_empty());
}
#[test]
fn attach_and_pause_require_release_before_new_press() {
    let mut r = resolver("bind pad b gear press\nbind pad pause pause press");
    event(&mut r, "pad", "b", 1., true);
    event(&mut r, "pad", "b", 1., false);
    assert!(r.drain().is_empty());
    event(&mut r, "pad", "b", 0., false);
    event(&mut r, "pad", "b", 1., false);
    assert_eq!(r.drain().len(), 1);
    r.context(true, true);
    event(&mut r, "pad", "b", 0., false);
    event(&mut r, "pad", "b", 1., false);
    event(&mut r, "pad", "pause", 0., true);
    event(&mut r, "pad", "pause", 1., false);
    assert_eq!(r.drain().len(), 1);
    r.context(false, true);
    event(&mut r, "pad", "b", 1., false);
    assert!(r.drain().is_empty());
    event(&mut r, "pad", "b", 0., false);
    event(&mut r, "pad", "b", 1., false);
    assert_eq!(r.drain().len(), 1);
    r.context(true, false);
    event(&mut r, "pad", "pause", 0., false);
    event(&mut r, "pad", "pause", 1., false);
    assert!(r.drain().is_empty());
}
#[test]
fn centered_axes_rearm_and_keyboard_priority_does_not_sum_noise() {
    let mut r = resolver("bind stick x roll axis\nbind keyboard x roll axis -1 0 1 0 1 1 100");
    event(&mut r, "stick", "x", 0.8, true);
    assert_eq!(frame(&mut r).roll, 0.);
    event(&mut r, "stick", "x", 0., false);
    event(&mut r, "stick", "x", 0.8, false);
    assert!(frame(&mut r).roll > 0.7);
    event(&mut r, "keyboard", "x", 0., true);
    event(&mut r, "keyboard", "x", -1., false);
    assert_eq!(frame(&mut r).roll, -1.);
    event(&mut r, "stick", "x", 0.79, false);
    assert_eq!(frame(&mut r).roll, -1.);
    event(&mut r, "keyboard", "x", 0., false);
    assert!(frame(&mut r).roll > 0.7);
}
#[test]
fn equal_priority_axis_owner_is_stable_until_neutral() {
    let mut r = resolver("bind * x roll axis");
    for d in ["a", "b"] {
        event(&mut r, d, "x", 0., true);
    }
    event(&mut r, "b", "x", 0.5, false);
    assert!(frame(&mut r).roll > 0.);
    event(&mut r, "a", "x", -0.9, false);
    assert!(frame(&mut r).roll > 0.);
    event(&mut r, "b", "x", 0., false);
    assert!(frame(&mut r).roll < 0.);
}
#[test]
fn throttle_pickup_crosses_target_and_rearms_after_keyboard_override() {
    let mut r = resolver("bind hotas t throttle unit");
    event(&mut r, "hotas", "t", -1., true);
    assert_eq!(frame(&mut r).throttle, None);
    event(&mut r, "hotas", "t", 0.6, false);
    assert_eq!(frame(&mut r).throttle, Some(0.8));
    r.override_throttle();
    assert_eq!(r.frame(0.2).0.throttle, None);
    event(&mut r, "hotas", "t", -0.8, false);
    assert_eq!(r.frame(0.2).0.throttle, Some(0.09999999999999998));
    assert!(r.disconnect("hotas"));
    assert_eq!(frame(&mut r).throttle, None);
}
#[test]
fn encoder_steps_and_three_position_selector_are_not_button_repeats() {
    let mut r = resolver("bind box knob throttle-rate delta\nbind box selector page-5 position=2");
    event(&mut r, "box", "knob", 0., true);
    event(&mut r, "box", "knob", 2., false);
    event(&mut r, "box", "knob", 2., false);
    let actions = r.drain();
    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0].1,
        Action::Pilot(PilotCommand::AdjustThrottle(0.02))
    );
    event(&mut r, "box", "selector", 2., true);
    assert!(r.drain().is_empty());
    event(&mut r, "box", "selector", 0., false);
    event(&mut r, "box", "selector", 2., false);
    assert_eq!(r.drain().len(), 1);
}
#[test]
fn profile_aliases_are_exact_and_limits_reject_bad_input() {
    let mut r = resolver("alias left serial-123\nbind left b gear press");
    event(&mut r, "serial-123", "b", 0., true);
    event(&mut r, "serial-123", "b", 1., false);
    assert_eq!(r.drain().len(), 1);
    for text in [
        "",
        "tore-input 2",
        "tore-input 1\nbind a b nonsense press",
        "tore-input 1\nbind a b pitch press",
        "tore-input 1\nbind a b gear axis",
        "tore-input 1\nbind a b pitch axis -1 0 1 1 1 1 1",
        "tore-input 1\nbind a b pitch axis -1 0 1 0 NaN 1 1",
        "tore-input 1\nalias a x\nalias a y",
    ] {
        assert!(Profile::parse(text).is_err(), "{text}");
    }
    assert!(Profile::parse(&"x".repeat(256 * 1024 + 1)).is_err());
}
#[test]
fn calibration_bounds_noise_endpoints_inversion_and_invalid_samples() {
    let c = Calibration {
        curve: 2.,
        scale: -1.,
        ..Default::default()
    };
    assert!(c.validate());
    assert_eq!(c.apply(0.04, false), 0.);
    assert_eq!(c.apply(-20., false), 1.);
    assert_eq!(c.apply(20., false), -1.);
    assert_eq!(c.apply(f64::NAN, false), 0.);
    assert_eq!(c.apply(-1., true), 1.);
    let mut r = resolver("bind a x pitch axis");
    event(&mut r, "a", "x", 0., true);
    event(&mut r, "a", "x", f64::INFINITY, false);
    assert_eq!(frame(&mut r).pitch, 0.);
}
#[test]
fn event_overflow_drops_commands_and_requests_recovery() {
    let mut r = resolver("bind box b gear press");
    event(&mut r, "box", "b", 0., true);
    for _ in 0..1025 {
        event(&mut r, "box", "b", 1., false);
        event(&mut r, "box", "b", 0., false);
    }
    assert!(r.take_overflow());
    assert!(r.drain().is_empty());
    assert!(!r.take_overflow());
}
#[test]
fn hundreds_of_button_assignments_are_not_limited_to_gamepad_masks() {
    let mut lines = String::new();
    for i in 0..300 {
        lines.push_str(&format!("bind box button:{i} gear press\n"));
    }
    let mut r = resolver(&lines);
    event(&mut r, "box", "button:299", 0., true);
    event(&mut r, "box", "button:299", 1., false);
    assert_eq!(r.drain().len(), 1);
}
#[test]
fn neutral_controls_work_on_first_movement_after_menu_closes() {
    let mut r = resolver(
        "bind pad b gear press\nbind pad x roll axis\nbind pad trigger yaw trigger-positive",
    );
    r.context(true, true);
    for (c, v) in [("b", 0.), ("x", 0.), ("trigger", -1.)] {
        event(&mut r, "pad", c, v, true);
    }
    r.context(false, true);
    event(&mut r, "pad", "b", 1., false);
    assert_eq!(r.drain().len(), 1);
    event(&mut r, "pad", "x", 0.6, false);
    assert!(frame(&mut r).roll > 0.5);
    event(&mut r, "pad", "trigger", 1., false);
    assert_eq!(frame(&mut r).yaw, 1.);
}
#[test]
fn paired_triggers_cancel_and_respect_keyboard_priority() {
    let mut r = resolver(
        "bind pad left yaw trigger-negative\nbind pad right yaw trigger-positive\nbind keyboard yaw yaw axis -1 0 1 0 1 1 100",
    );
    for c in ["left", "right"] {
        event(&mut r, "pad", c, -1., true);
    }
    event(&mut r, "keyboard", "yaw", 0., true);
    event(&mut r, "pad", "left", 1., false);
    assert_eq!(frame(&mut r).yaw, -1.);
    event(&mut r, "pad", "right", 1., false);
    assert_eq!(frame(&mut r).yaw, 0.);
    event(&mut r, "pad", "left", -1., false);
    assert_eq!(frame(&mut r).yaw, 1.);
    event(&mut r, "keyboard", "yaw", -1., false);
    assert_eq!(frame(&mut r).yaw, -1.);
}
#[test]
fn invalid_live_sample_releases_previous_deflection_and_requests_pause() {
    let mut r = resolver("bind stick x roll axis");
    event(&mut r, "stick", "x", 0., true);
    event(&mut r, "stick", "x", 1., false);
    assert_eq!(frame(&mut r).roll, 1.);
    event(&mut r, "stick", "x", f64::NAN, false);
    assert_eq!(frame(&mut r).roll, 0.);
    assert!(r.take_overflow());
}

#[test]
fn combat_chords_suppress_flight_actions_and_require_release_after_interruptions() {
    let mut r = resolver(
        "bind pad rb throttle-rate positive\nbind pad select+rb fire hold\nbind pad a gear press\nbind pad select+a designate press\nbind pad select+hat damage-class position=-1\nbind pad select+hat fail-station position=1",
    );
    for c in ["select", "rb", "a", "hat"] {
        event(&mut r, "pad", c, 0., true);
    }
    event(&mut r, "pad", "select", 1., false);
    event(&mut r, "pad", "rb", 1., false);
    assert!(r.held("fire"));
    assert_eq!(frame(&mut r).throttle_rate, 0.);
    event(&mut r, "pad", "a", 1., false);
    assert_eq!(
        r.drain(),
        vec![("pad".into(), Action::Ui("designate".into()))]
    );
    event(&mut r, "pad", "hat", -1., false);
    assert_eq!(r.drain().len(), 1);
    r.context(true, true);
    assert!(!r.held("fire"));
    r.context(false, true);
    assert!(!r.held("fire"));
    event(&mut r, "pad", "rb", 1., false);
    assert!(!r.held("fire"));
    event(&mut r, "pad", "rb", 0., false);
    event(&mut r, "pad", "rb", 1., false);
    assert!(r.held("fire"));
    event(&mut r, "pad", "select", 0., false);
    assert!(!r.held("fire"));
    assert_eq!(frame(&mut r).throttle_rate, 0.);
    event(&mut r, "pad", "rb", 0., false);
    event(&mut r, "pad", "rb", 1., false);
    assert_eq!(frame(&mut r).throttle_rate, 1.);
    event(&mut r, "pad", "select", 1., false);
    assert!(!r.held("fire"));
    event(&mut r, "pad", "rb", 0., false);
    event(&mut r, "pad", "rb", 1., false);
    assert!(r.held("fire"));
    r.disconnect("pad");
    assert!(!r.held("fire"));
    assert_eq!(frame(&mut r).throttle_rate, 0.);
}

#[test]
fn fire_profiles_cannot_silently_select_edge_or_axis_modes() {
    for mode in ["press", "release", "switch", "axis", "delta"] {
        assert!(Profile::parse(&format!("tore-input 1\nbind pad b fire {mode}")).is_err());
    }
    for control in ["+a", "a+", "a+a", "a+b+c"] {
        assert!(Profile::parse(&format!("tore-input 1\nbind pad {control} fire hold")).is_err());
    }
    assert!(Profile::parse("tore-input 1\nbind pad modifier+b fire hold").is_ok());
}
