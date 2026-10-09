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
fn fa_throttle_presets_steps_and_countermeasures_parse_as_ui_actions() {
    for action in [
        "throttle-preset=0",
        "throttle-preset=0.75",
        "throttle-preset=burner",
        "throttle-step=-0.05",
        "throttle-step=0.05",
        "chaff",
        "flare",
        "waypoint-next",
        "waypoint-previous",
    ] {
        assert_eq!(Action::parse(action), Ok(Action::Ui(action.into())));
    }
    for action in [
        "throttle-preset=1.5",
        "throttle-preset=full",
        "throttle-step=0",
        "throttle-step=2",
    ] {
        assert!(Action::parse(action).is_err(), "{action}");
    }
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
    for control in ["+a", "a+", "a+a", "a+b+a", "a+b+c+d", "hat=x+b", "t>+b"] {
        assert!(Profile::parse(&format!("tore-input 1\nbind pad {control} fire hold")).is_err());
    }
    assert!(Profile::parse("tore-input 1\nbind pad modifier+b fire hold").is_ok());
    assert!(Profile::parse("tore-input 1\nbind pad lb+rb+b fire hold").is_ok());
    assert!(Profile::parse("tore-input 1\nbind keyboard a+b gear press").is_err());
}

#[test]
fn stacked_modifiers_pick_the_most_specific_layer() {
    let mut r = resolver(
        "bind pad a gear press\nbind pad lb+a flaps press\nbind pad lb+rb+a hook press\nbind pad rb+a airbrake press",
    );
    for c in ["a", "lb", "rb"] {
        event(&mut r, "pad", c, 0., true);
    }
    let press = |r: &mut Resolver| {
        event(r, "pad", "a", 1., false);
        event(r, "pad", "a", 0., false);
        r.drain()
            .into_iter()
            .map(|(_, a)| crate::profile_text::action_name(&a))
            .collect::<Vec<_>>()
    };
    assert_eq!(press(&mut r), ["gear"]);
    event(&mut r, "pad", "lb", 1., false);
    assert_eq!(press(&mut r), ["flaps"]);
    event(&mut r, "pad", "rb", 1., false);
    assert_eq!(press(&mut r), ["hook"]);
    event(&mut r, "pad", "lb", 0., false);
    assert_eq!(press(&mut r), ["airbrake"]);
    event(&mut r, "pad", "rb", 0., false);
    assert_eq!(press(&mut r), ["gear"]);
}
#[test]
fn dpad_directions_are_independent_dedicated_modifiers() {
    let mut r = resolver(
        "modifier pad hat=-1\nmodifier pad hat=1\nbind pad hat instrument-previous position=-1\nbind pad hat instrument-next position=1\nbind pad hat=-1+a flaps press\nbind pad hat=1+a hook press\nbind pad a gear press",
    );
    for c in ["a", "hat"] {
        event(&mut r, "pad", c, 0., true);
    }
    event(&mut r, "pad", "hat", -1., false);
    // A dedicated modifier no longer performs its own hat action.
    assert!(r.drain().is_empty());
    event(&mut r, "pad", "a", 1., false);
    event(&mut r, "pad", "a", 0., false);
    assert_eq!(
        r.drain(),
        vec![(
            "pad".into(),
            Action::Pilot(PilotCommand::Toggle(Switch::Flaps))
        )]
    );
    event(&mut r, "pad", "hat", 1., false);
    event(&mut r, "pad", "a", 1., false);
    assert_eq!(
        r.drain(),
        vec![(
            "pad".into(),
            Action::Pilot(PilotCommand::Toggle(Switch::Hook))
        )]
    );
    event(&mut r, "pad", "a", 0., false);
    event(&mut r, "pad", "hat", 0., false);
    event(&mut r, "pad", "a", 1., false);
    assert_eq!(
        r.drain(),
        vec![(
            "pad".into(),
            Action::Pilot(PilotCommand::Toggle(Switch::Gear))
        )]
    );
}
#[test]
fn analog_threshold_acts_as_a_button_and_trigger_scale_is_sensitivity() {
    let mut r =
        resolver("bind pad rt>0 fire hold\nbind pad lt yaw trigger-negative -1 0 1 0 1 0.5 10");
    event(&mut r, "pad", "rt", -1., true);
    event(&mut r, "pad", "lt", -1., true);
    event(&mut r, "pad", "rt", -0.5, false);
    assert!(!r.held("fire"));
    event(&mut r, "pad", "rt", 0.4, false);
    assert!(r.held("fire"));
    event(&mut r, "pad", "rt", -1., false);
    assert!(!r.held("fire"));
    event(&mut r, "pad", "lt", 1., false);
    assert_eq!(frame(&mut r).yaw, -0.5);
}
#[test]
fn head_axes_report_absolute_angles_only_when_bound() {
    let mut r = resolver("bind stick x roll axis");
    assert_eq!(r.head(), None);
    let mut r = resolver("bind track yaw head-yaw axis -1 0 1 0 1 1 10");
    event(&mut r, "track", "yaw", 0., true);
    event(&mut r, "track", "yaw", 0.5, false);
    let head = r.head().unwrap();
    assert!((head[0] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(head[1], 0.);
}
#[test]
fn star_bindings_never_match_keyboard_or_mouse() {
    let mut r = resolver("bind * wheel:up zoom-in press");
    event(&mut r, "mouse", "wheel:up", 0., true);
    event(&mut r, "mouse", "wheel:up", 1., false);
    assert!(r.drain().is_empty());
}

#[test]
fn lift_levers_retain_zero_and_release_on_disconnect() {
    let mut r = resolver("bind stick lift collective unit\nbind stick turn vector-yaw axis");
    event(&mut r, "stick", "lift", -1., true);
    event(&mut r, "stick", "turn", 0., true);
    assert_eq!(frame(&mut r).collective, Some(0.));
    assert_eq!(frame(&mut r).vector_yaw, Some(0.));
    event(&mut r, "stick", "lift", 1., false);
    assert_eq!(frame(&mut r).collective, Some(1.));
    assert!(r.disconnect("stick"));
    assert_eq!(frame(&mut r).collective, None);
}
#[test]
fn lift_profile_commands_roundtrip_and_reject_invalid_positions() {
    let p = Profile::parse("tore-input 1\nbind stick c collective unit\nbind stick r collective-rate positive\nbind stick n neutral-vector press\nbind stick z vector-yaw=-0.5 press\nbind stick s conversion-step=-0.25 press").unwrap();
    assert_eq!(
        Profile::parse(&p.to_text().unwrap())
            .unwrap()
            .to_text()
            .unwrap(),
        p.to_text().unwrap()
    );
    for action in [
        "collective=-0.1",
        "conversion=1.1",
        "vector-yaw=NaN",
        "vector-pitch-step=2",
    ] {
        assert!(Action::parse(action).is_err());
    }
}

#[test]
fn digital_lift_override_holds_until_lever_movement_even_after_focus_loss() {
    let mut r = resolver(
        "bind stick lever vector-pitch unit\nbind stick yaw vector-yaw axis -1 0 1 0 1 1 10",
    );
    event(&mut r, "stick", "lever", -0.2, true);
    assert_eq!(frame(&mut r).vector_pitch, Some(0.4));
    r.override_lift_axes(&[Axis::VectorPitch]);
    assert_eq!(frame(&mut r).vector_pitch, None);
    event(&mut r, "stick", "lever", -0.19, false);
    assert_eq!(frame(&mut r).vector_pitch, None);
    r.context(false, false);
    r.context(false, true);
    assert_eq!(frame(&mut r).vector_pitch, None);
    event(&mut r, "stick", "lever", -0.15, false);
    assert!((frame(&mut r).vector_pitch.unwrap() - 0.425).abs() < 1e-12);
    event(&mut r, "stick", "yaw", 0., true);
    event(&mut r, "stick", "yaw", 0.4, false);
    assert_eq!(frame(&mut r).vector_yaw, Some(0.4));
    r.override_lift_axes(&[Axis::VectorYaw]);
    event(&mut r, "stick", "yaw", 0.41, false);
    assert_eq!(frame(&mut r).vector_yaw, None);
    event(&mut r, "stick", "yaw", 0.45, false);
    assert_eq!(frame(&mut r).vector_yaw, Some(0.45));
}

#[test]
fn vtol_overhaul_actions_parse_name_and_roundtrip_through_profiles() {
    let lift = |c| Action::Pilot(PilotCommand::Lift(c));
    for (name, action) in [
        (
            "nozzle-step-up",
            lift(LiftCommand::NozzleStep { down: false }),
        ),
        (
            "nozzle-step-down",
            lift(LiftCommand::NozzleStep { down: true }),
        ),
        (
            "nozzle-preset-forward",
            lift(LiftCommand::NozzlePreset(NozzlePreset::Forward)),
        ),
        (
            "nozzle-preset-vertical",
            lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)),
        ),
        ("stability-level", lift(LiftCommand::CycleStability)),
        (
            "stability-level=off",
            lift(LiftCommand::SetStability(StabilityLevel::Off)),
        ),
        (
            "stability-level=damper",
            lift(LiftCommand::SetStability(StabilityLevel::Damper)),
        ),
        (
            "stability-level=attitude",
            lift(LiftCommand::SetStability(StabilityLevel::Attitude)),
        ),
        ("trim-set", lift(LiftCommand::TrimSet)),
        ("trim-centre", lift(LiftCommand::TrimCentre)),
        (
            "hover-hold",
            Action::Pilot(PilotCommand::Toggle(Switch::HoverHold)),
        ),
        ("trim-pitch-rate", Action::Axis(Axis::TrimPitchRate)),
        ("trim-roll-rate", Action::Axis(Axis::TrimRollRate)),
        ("trim-pedal-rate", Action::Axis(Axis::TrimPedalRate)),
    ] {
        assert_eq!(Action::parse(name), Ok(action.clone()), "{name}");
        assert_eq!(profile_text::action_name(&action), name);
    }
    let p = Profile::parse(
        "tore-input 1\nbind keyboard Ctrl-Alt-a hover-hold press\nbind keyboard Ctrl-Shift-a stability-level press\nbind keyboard Shift-z nozzle-preset-forward press\nbind stick hat trim-pitch-rate negative\nbind stick t trim-roll-rate axis\nbind stick b trim-set press\nbind stick b2 stability-level=attitude press",
    )
    .unwrap();
    let text = p.to_text().unwrap();
    assert_eq!(Profile::parse(&text).unwrap().to_text().unwrap(), text);
    for bad in ["stability-level=full", "nozzle-step", "trim-yaw-rate"] {
        assert!(Action::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn held_trim_taps_two_percent_then_moves_ten_percent_a_second() {
    let mut r = resolver(
        "bind keyboard Ctrl-ArrowUp trim-pitch-rate negative\nbind keyboard Ctrl-ArrowRight trim-roll-rate positive",
    );
    let trims = |input: PilotInput| -> Vec<(TrimAxis, f64)> {
        input
            .commands
            .into_iter()
            .filter_map(|c| match c {
                PilotCommand::Lift(LiftCommand::TrimAdjust(axis, v)) => Some((axis, v)),
                _ => None,
            })
            .collect()
    };
    event(&mut r, "keyboard", "Ctrl-ArrowUp", 0., true);
    event(&mut r, "keyboard", "Ctrl-ArrowRight", 0., true);
    assert!(trims(frame(&mut r)).is_empty());
    // A tap: one press for a few ticks is exactly 2 percent.
    event(&mut r, "keyboard", "Ctrl-ArrowUp", 1., false);
    assert_eq!(trims(frame(&mut r)), vec![(TrimAxis::Pitch, -0.02)]);
    for _ in 0..10 {
        assert!(trims(frame(&mut r)).is_empty());
    }
    event(&mut r, "keyboard", "Ctrl-ArrowUp", 0., false);
    assert!(trims(frame(&mut r)).is_empty());
    // Held for one second: the tap, then the rate after 0.2 s.
    event(&mut r, "keyboard", "Ctrl-ArrowRight", 1., false);
    let total: f64 = (0..120)
        .flat_map(|_| trims(frame(&mut r)))
        .map(|(axis, v)| {
            assert_eq!(axis, TrimAxis::Roll);
            v
        })
        .sum();
    let steps = (120 - trim_keys::DELAY_TICKS).div_ceil(trim_keys::STEP_TICKS);
    let expected = trim_keys::TAP + f64::from(steps) * trim_keys::STEP;
    assert!((total - expected).abs() < 1e-12, "{total}");
    assert!((total - 0.1).abs() < 1e-9, "{total}");
    // Losing focus forgets the hold: the next press is a fresh tap.
    r.context(false, false);
    r.context(false, true);
    event(&mut r, "keyboard", "Ctrl-ArrowRight", 0., true);
    event(&mut r, "keyboard", "Ctrl-ArrowRight", 1., false);
    assert_eq!(trims(frame(&mut r)), vec![(TrimAxis::Roll, 0.02)]);
}

#[test]
fn gunsight_names_parse_name_and_roundtrip_through_profiles() {
    for (name, action) in [
        ("sight-x", Action::Axis(Axis::SightX)),
        ("sight-y", Action::Axis(Axis::SightY)),
        ("sight-designate", Action::Ui("sight-designate".into())),
        ("sight-pin", Action::Ui("sight-pin".into())),
        ("sight-zoom-in", Action::Ui("sight-zoom-in".into())),
        ("sight-zoom-out", Action::Ui("sight-zoom-out".into())),
        ("sight-left", Action::Ui("sight-left".into())),
        ("sight-right", Action::Ui("sight-right".into())),
        ("sight-up", Action::Ui("sight-up".into())),
        ("sight-down", Action::Ui("sight-down".into())),
    ] {
        assert_eq!(Action::parse(name), Ok(action.clone()), "{name}");
        assert_eq!(profile_text::action_name(&action), name);
    }
    let p = Profile::parse(
        "tore-input 1\nmodifier pad button:314\nbind keyboard Alt-ArrowLeft sight-left hold\nbind keyboard Shift-\\ sight-pin press\nbind keyboard \\ sight-designate press\nbind pad axis:16=-1 sight-left hold\nbind pad button:314+axis:3 sight-x axis -1 0 1 0.1 1 1 10\nbind pad button:314+axis:4 sight-y axis -1 0 1 0.1 1 -1 10\nbind pad button:314+button:304 sight-designate tap\nbind pad button:314+button:304 sight-pin long\nbind stick hat sight-x negative",
    )
    .unwrap();
    let text = p.to_text().unwrap();
    assert_eq!(Profile::parse(&text).unwrap().to_text().unwrap(), text);
    assert!(text.contains("button:314+button:304 sight-designate tap"));
    assert!(text.contains("button:314+button:304 sight-pin long"));
    assert!(text.contains("button:314+axis:4 sight-y axis -1 0 1 0.1 1 -1 10"));
    // The axes take analog and hold modes only, the holds hold, and the
    // timed modes belong to commands.
    for bad in [
        "bind stick b sight-x press",
        "bind stick b sight-x tap",
        "bind stick a roll long",
        "bind stick b sight-pin hold",
    ] {
        assert!(
            Profile::parse(&format!("tore-input 1\n{bad}")).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_tap_fires_on_release_and_a_long_press_fires_at_half_a_second() {
    let mut r = resolver(
        "modifier pad sel\nbind pad sel+a sight-designate tap\nbind pad sel+a sight-pin long\n",
    );
    for control in ["sel", "a"] {
        event(&mut r, "pad", control, 0., true);
    }
    event(&mut r, "pad", "sel", 1., false);
    // A short press designates when the button comes up, not before.
    event(&mut r, "pad", "a", 1., false);
    for _ in 0..LONG_PRESS_TICKS - 1 {
        frame(&mut r);
    }
    assert!(r.drain().is_empty());
    event(&mut r, "pad", "a", 0., false);
    assert_eq!(
        r.drain(),
        vec![("pad".into(), Action::Ui("sight-designate".into()))]
    );
    // Held to the long press: the pin fires on that tick, and the release
    // afterwards designates nothing.
    event(&mut r, "pad", "a", 1., false);
    for _ in 0..LONG_PRESS_TICKS - 1 {
        frame(&mut r);
    }
    assert!(r.drain().is_empty());
    frame(&mut r);
    assert_eq!(
        r.drain(),
        vec![("pad".into(), Action::Ui("sight-pin".into()))]
    );
    for _ in 0..LONG_PRESS_TICKS {
        frame(&mut r);
    }
    event(&mut r, "pad", "a", 0., false);
    assert!(r.drain().is_empty(), "the long press fires once");
}

#[test]
fn timed_presses_forget_an_interrupted_hold_and_need_a_fresh_press() {
    let mut r = resolver("bind pad a sight-pin long\nbind pad b sight-designate tap\n");
    for control in ["a", "b"] {
        event(&mut r, "pad", control, 0., true);
    }
    event(&mut r, "pad", "a", 1., false);
    event(&mut r, "pad", "b", 1., false);
    for _ in 0..10 {
        frame(&mut r);
    }
    // A pause drops both: neither the long press nor the tap completes, and
    // a control still held when play resumes needs releasing first.
    r.context(true, true);
    r.context(false, true);
    for _ in 0..LONG_PRESS_TICKS {
        frame(&mut r);
    }
    event(&mut r, "pad", "b", 0., false);
    assert!(r.drain().is_empty());
    // A press that began in a menu (a baseline) is not a press.
    event(&mut r, "pad", "a", 1., true);
    for _ in 0..LONG_PRESS_TICKS {
        frame(&mut r);
    }
    assert!(r.drain().is_empty());
}

#[test]
fn sight_axes_read_the_stick_and_the_holds_read_buttons_and_keys() {
    let mut r = resolver(
        "modifier pad sel\nbind pad sel+x sight-x axis -1 0 1 0.1 1 1 10\nbind pad sel+y sight-y axis -1 0 1 0.1 1 -1 10\nbind pad axis:16=1 sight-right hold\nbind keyboard Alt-ArrowUp sight-up hold\n",
    );
    for (device, control) in [
        ("pad", "sel"),
        ("pad", "x"),
        ("pad", "y"),
        ("pad", "axis:16"),
        ("keyboard", "Alt-ArrowUp"),
    ] {
        event(&mut r, device, control, 0., true);
    }
    assert_eq!(r.sight_axes(), [0.; 2]);
    event(&mut r, "pad", "sel", 1., false);
    event(&mut r, "pad", "x", 0.5, false);
    event(&mut r, "pad", "y", -1., false);
    let [x, y] = r.sight_axes();
    assert!(x > 0. && x < 1., "{x}");
    assert_eq!(
        y, 1.,
        "the stick pushed up reads negative, sight-y inverts it"
    );
    // Without the Select modifier the stick is not the sight.
    event(&mut r, "pad", "sel", 0., false);
    assert_eq!(r.sight_axes(), [0.; 2]);
    // Holds do not appear in the axes; they are read by name.
    event(&mut r, "keyboard", "Alt-ArrowUp", 1., false);
    event(&mut r, "pad", "axis:16", 1., false);
    assert_eq!(r.sight_axes(), [0.; 2]);
    assert!(r.held("sight-up") && r.held("sight-right"));
    assert!(!r.held("sight-left") && !r.held("sight-down"));
    r.context(true, true);
    assert!(!r.held("sight-up"), "a pause releases the holds");
    assert_eq!(r.sight_axes(), [0.; 2]);
}
