//! Each seat command does what the between-frames handler it replaced did:
//! giving it before a tick leaves combat, the flight and the airport service
//! exactly as calling the old path before that tick did.

use super::{tick_tests::mission, *};
use crate::seats::SeatCommand;
use tore_input::{PilotCommand, Switch};
use tore_sim::combat::live::Command as Live;

/// An input for the world's next tick with `commands` and nothing else.
fn input(world: &World, commands: Vec<SeatCommand>) -> SeatInput {
    SeatInput {
        tick: world.tick(),
        commands,
        ..Default::default()
    }
}

/// The mission with a synthetic radar reaching 90 nmi and a 10 nmi visual
/// channel, so the player can designate a contact.
fn sighted() -> World {
    use tore_sim::sensors;
    let mut world = mission();
    let volume = |nmi: f64| sensors::Volume {
        azimuth_rad: 1.,
        elevation_rad: 1.,
        minimum_ft: 0.,
        maximum_ft: nmi * sensors::FEET_PER_NAUTICAL_MILE,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    };
    let mut config = world.combat.state.own().configuration().clone();
    config.sensors.radar = Some(sensors::RadarProfile {
        record: "SYNTHETIC.SEE".into(),
        search: volume(90.),
        track: volume(50.),
        look_down: 0.,
        preset: sensors::Preset::Advanced,
        notch: sensors::Preset::Advanced.notch(),
        resistance: sensors::Preset::Advanced.resistance(),
        band: 0,
        source_flags: [0; 2],
        source_doppler: [0; 3],
    });
    config.sensors.visual = Some(sensors::profile::VisualProfile {
        record: "SYNTHETIC.VIS".into(),
        search: volume(10.),
        track: volume(5.),
    });
    let old = std::mem::replace(
        &mut world.combat.state,
        tore_sim::combat::live::State::new(config, true).unwrap(),
    );
    world.combat.state.targets = old.targets.clone();
    // Every aircraft is a contact worth designating, and there is
    // something to release.
    world.combat.state.own_mut().chaff = 5;
    world.combat.state.own_mut().flares = 5;
    world.combat.apply_startup_weapons();
    world.order_call = OrderCall::Spoken;
    world
}

/// The sighted mission flown for 300 ticks with the radar on, so the scope
/// holds contacts to designate.
fn warmed() -> World {
    let mut world = sighted();
    let mut out = TickOutput::default();
    world.combat.start_tape();
    for tick in 0..300 {
        let mut first = input(&world, Vec::new());
        if tick == 0 {
            first
                .pilot
                .commands
                .push(PilotCommand::Set(Switch::Radar, true));
        }
        world.step(&[first], &mut out).unwrap();
    }
    world.combat.take_tape();
    let _ = world.combat.take_notes();
    world
}

/// Everything a command can touch, as text: combat's selection, designation,
/// countermeasures, hit points, payload and trigger, the player's flight, the
/// airport service and the records the mission recording and the combat tape
/// keep of the commands.
fn state(world: &mut World) -> String {
    let own = &world.cockpits[0];
    let notes = world.combat.take_notes();
    let tape: Vec<String> = world
        .combat
        .take_tape()
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    let held = {
        let trigger = world.combat.own_trigger();
        (trigger.input.held, trigger.controller.held)
    };
    let combat = &world.combat.state;
    [
        format!(
            "{:?}",
            (
                combat.tick(),
                combat.own().selected,
                combat.own().armed,
                combat.own_view().designated(),
                combat.own_view().view_target().map(|target| target.id),
                combat.own().chaff,
                combat.own().flares,
                combat.own().ammo.clone(),
                combat.own().hp,
                combat.own().payload_lbs(),
                combat.projectiles.len(),
            )
        ),
        format!(
            "{:?}",
            combat
                .targets
                .iter()
                .map(|target| (target.id, target.hp, target.position))
                .collect::<Vec<_>>()
        ),
        format!(
            "{:?}",
            (
                held,
                (own.flight.position, own.flight.velocity, own.flight.speed),
                (own.flight.fuel, own.flight.crashed),
                (own.airport_nav_mode, own.airport_service.selected()),
                (own.flight.sensors, own.flight.cheats, combat.cheats),
                world.comms.radio_silence(crate::seats::SeatId(0)),
                world
                    .comms
                    .channel_free(crate::seats::SeatId(0), combat.tick() as f64 / 120.),
                world.ai_wings.as_ref().map(|wings| {
                    wings
                        .mission()
                        .actors()
                        .iter()
                        .map(|actor| (actor.id(), actor.controller().ordered_formation()))
                        .collect::<Vec<_>>()
                }),
                world
                    .roster
                    .seat(crate::seats::SeatId(0))
                    .map(|seat| seat.wing_recipient),
            )
        ),
        format!("{:?}", world.comms.journal()),
        format!("{notes:?}"),
        format!("{tape:?}"),
    ]
    .join("\n")
}

/// Two identical worlds; `old` does to the first what the between-frames
/// handler did, and the second is given `commands`. Each then flies one tick.
/// Returns both, after checking they agree, and the second tick's output.
fn same_as_old(
    commands: Vec<SeatCommand>,
    old: impl FnOnce(&mut World),
) -> (World, World, TickOutput) {
    let (mut before, mut after) = (warmed(), warmed());
    old(&mut before);
    let mut out = TickOutput::default();
    let plain = input(&before, Vec::new());
    before.step(&[plain], &mut out).unwrap();
    let commanded = input(&after, commands);
    after.step(&[commanded], &mut out).unwrap();
    assert_eq!(state(&mut before), state(&mut after));
    (before, after, out)
}

fn launcher(world: &World) -> tore_sim::combat::live::Launcher {
    combat::launcher(&world.cockpits[0].flight)
}

/// The old handler for a plain combat command.
fn old_combat(world: &mut World, command: Live) {
    let launcher = launcher(world);
    world.combat.command(command, launcher);
}

/// The old handler for a key, button or menu combat command.
fn old_manual(world: &mut World, command: Live) {
    if !world.combat.range
        && !matches!(
            command,
            Live::ToggleArm | Live::ClearDesignation | Live::ToggleSeekerMode
        )
    {
        return;
    }
    world.combat.cancel();
    let launcher = launcher(world);
    world.combat.command(command, launcher);
    if world.combat.range {
        world
            .combat
            .refresh_render(&world.cockpits[0].flight, world.ai_wings.as_ref());
    }
    let flight = &mut world.cockpits[0].flight;
    flight
        .set_payload(
            (world.combat.state.own().payload_lbs() - flight.systems.used_external_lbs()).max(0.),
        )
        .unwrap();
}

#[test]
fn designation_commands_match_the_old_path() {
    for command in [
        Live::Designate,
        Live::DesignatePrevious,
        Live::DesignateVisual,
    ] {
        let (_, after, _) = same_as_old(vec![SeatCommand::Combat(command)], |world| {
            old_combat(world, command)
        });
        if command == Live::Designate {
            assert!(
                after.combat.state.own_view().designated().is_some(),
                "{command:?}"
            );
        }
    }
}

#[test]
fn designation_by_identity_from_a_scope_click_matches_the_old_path() {
    let id = warmed().combat.state.own().sensors.contacts()[0].id;
    let (_, after, _) = same_as_old(
        vec![SeatCommand::Combat(Live::DesignateTarget(id))],
        |world| old_combat(world, Live::DesignateTarget(id)),
    );
    assert_eq!(after.combat.state.own_view().designated(), Some(id));
}

#[test]
fn the_weapon_display_buttons_match_the_old_path() {
    for command in [Live::ToggleSeekerMode, Live::ClearDesignation] {
        same_as_old(vec![SeatCommand::Combat(command)], |world| {
            old_combat(world, command)
        });
    }
}

#[test]
fn arming_seeker_mode_and_jettison_match_the_old_path() {
    for command in [
        Live::ToggleArm,
        Live::ToggleSeekerMode,
        Live::ClearDesignation,
    ] {
        let (_, after, _) = same_as_old(vec![SeatCommand::Manual(command)], |world| {
            old_manual(world, command)
        });
        if command == Live::ToggleArm {
            assert_ne!(
                after.combat.state.own().armed,
                warmed().combat.state.own().armed
            );
        }
    }
}

#[test]
fn a_range_command_outside_live_fire_is_refused_with_a_message() {
    let (_, after, out) = same_as_old(vec![SeatCommand::Manual(Live::CycleClass)], |_| {});
    assert!(!after.combat.range);
    assert_eq!(out.commanded, 1);
    assert!(matches!(
        &out.cues[0],
        Cue::Message(text) if text == "Manual range command requires --live-fire"
    ));
}

#[test]
fn range_commands_match_the_old_path_in_live_fire() {
    let range = |world: &mut World| world.combat.range = true;
    for command in [
        Live::CycleClass,
        Live::FailStation,
        Live::DamagePlayer,
        Live::Jettison,
    ] {
        let (mut before, mut after) = (warmed(), warmed());
        range(&mut before);
        range(&mut after);
        old_manual(&mut before, command);
        let mut out = TickOutput::default();
        let plain = input(&before, Vec::new());
        before.step(&[plain], &mut out).unwrap();
        let commanded = input(&after, vec![SeatCommand::Manual(command)]);
        after.step(&[commanded], &mut out).unwrap();
        assert_eq!(state(&mut before), state(&mut after), "{command:?}");
    }
}

#[test]
fn a_range_reset_matches_the_old_path_and_is_refused_outside_live_fire() {
    let (_, _, out) = same_as_old(vec![SeatCommand::RangeReset], |_| {});
    assert!(matches!(
        &out.cues[0],
        Cue::Message(text) if text == "Target reset is available only with --live-fire"
    ));
    let (mut before, mut after) = (warmed(), warmed());
    before.combat.range = true;
    after.combat.range = true;
    before.combat.cancel();
    let launcher = launcher(&before);
    before.combat.command(Live::ReplaceTarget, launcher);
    before
        .combat
        .refresh_render(&before.cockpits[0].flight, before.ai_wings.as_ref());
    let mut out = TickOutput::default();
    let plain = input(&before, Vec::new());
    before.step(&[plain], &mut out).unwrap();
    let commanded = input(&after, vec![SeatCommand::RangeReset]);
    after.step(&[commanded], &mut out).unwrap();
    assert_eq!(state(&mut before), state(&mut after));
}

/// The old handler for chaff and flares, without its pause check, which
/// stayed with the app.
fn old_countermeasure(world: &mut World, chaff: bool) -> Option<String> {
    let launcher = launcher(world);
    if !launcher.alive
        || world.cockpits[0].flight.escape.is_some()
        || world.combat.state.own().hp <= 0
    {
        return None;
    }
    let count = |state: &tore_sim::combat::live::State| {
        if chaff {
            state.own().chaff
        } else {
            state.own().flares
        }
    };
    let before = count(&world.combat.state);
    world.combat.command(
        if chaff {
            Live::ReleaseChaff
        } else {
            Live::ReleaseFlare
        },
        launcher,
    );
    let after = count(&world.combat.state);
    Some(match (chaff, before) {
        (true, 0) => "Out of chaff".to_string(),
        (false, 0) => "Out of flares".to_string(),
        (true, _) => format!("Chaff launched, {after} left"),
        (false, _) => format!("Flare launched, {after} left"),
    })
}

#[test]
fn chaff_and_flares_match_the_old_path_and_say_how_many_are_left() {
    for chaff in [true, false] {
        let mut old_message = None;
        let (before, after, out) = same_as_old(
            vec![if chaff {
                SeatCommand::ReleaseChaff
            } else {
                SeatCommand::ReleaseFlare
            }],
            |world| old_message = old_countermeasure(world, chaff),
        );
        let old_message = old_message.expect("a live aircraft releases");
        assert_eq!(out.commanded, 1);
        assert!(matches!(&out.cues[0], Cue::Message(text) if *text == old_message));
        let left = |world: &World| {
            let state = &world.combat.state;
            if chaff {
                state.own().chaff
            } else {
                state.own().flares
            }
        };
        assert_eq!(left(&before), left(&after));
        assert!(left(&after) < left(&warmed()));
    }
}

#[test]
fn countermeasures_are_refused_for_a_destroyed_ejected_or_hitless_aircraft() {
    let refusals: [fn(&mut World); 3] = [
        |world| world.cockpits[0].flight.crashed = true,
        |world| {
            world.combat.state.own_mut().hp = 0;
        },
        |world| {
            let flight = &mut world.cockpits[0].flight;
            flight.escape = Some(crate::combat::fixtures::pilots().remove(0));
        },
    ];
    for refuse in refusals {
        for command in [SeatCommand::ReleaseChaff, SeatCommand::ReleaseFlare] {
            let mut world = warmed();
            refuse(&mut world);
            let (chaff, flares) = (
                world.combat.state.own().chaff,
                world.combat.state.own().flares,
            );
            let mut out = TickOutput::default();
            let commanded = input(&world, vec![command]);
            world.step(&[commanded], &mut out).unwrap();
            assert_eq!(
                (
                    world.combat.state.own().chaff,
                    world.combat.state.own().flares
                ),
                (chaff, flares)
            );
            assert_eq!(out.commanded, 0, "no message for a refused release");
            assert!(world.combat.take_notes().is_empty());
        }
    }
}

#[test]
fn releasing_the_trigger_and_the_space_key_match_the_old_path() {
    let held = |world: &mut World| world.combat.own_trigger().input.space(true, false, false);
    let (mut before, mut after) = (warmed(), warmed());
    held(&mut before);
    held(&mut after);
    before.combat.cancel();
    let mut out = TickOutput::default();
    let plain = input(&before, Vec::new());
    before.step(&[plain], &mut out).unwrap();
    let commanded = input(&after, vec![SeatCommand::ReleaseTrigger]);
    after.step(&[commanded], &mut out).unwrap();
    assert_eq!(state(&mut before), state(&mut after));
    assert!(!after.combat.own_trigger().input.held);

    // Press, then a press the game was paused for: the second only lets go.
    for (down, repeat, blocked) in [
        (true, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let (mut before, mut after) = (warmed(), warmed());
        before
            .combat
            .own_trigger()
            .input
            .space(down, repeat, blocked);
        let mut out = TickOutput::default();
        let plain = input(&before, Vec::new());
        before.step(&[plain], &mut out).unwrap();
        let commanded = input(
            &after,
            vec![SeatCommand::TriggerKey {
                down,
                repeat,
                blocked,
            }],
        );
        after.step(&[commanded], &mut out).unwrap();
        assert_eq!(state(&mut before), state(&mut after));
    }
}

#[test]
fn weapon_cycling_matches_the_old_path() {
    for forward in [true, false] {
        let (_, _, out) = same_as_old(vec![SeatCommand::CycleWeapon { forward }], |world| {
            world.cycle_cockpit_weapon(0, forward)
        });
        assert!(matches!(out.cues[0], Cue::WeaponCycled));
    }
}

#[test]
fn commands_apply_in_the_order_given() {
    let (_, after, out) = same_as_old(
        vec![
            SeatCommand::Manual(Live::ToggleArm),
            SeatCommand::Combat(Live::Designate),
            SeatCommand::CycleWeapon { forward: true },
            SeatCommand::Manual(Live::ToggleArm),
        ],
        |world| {
            old_manual(world, Live::ToggleArm);
            old_combat(world, Live::Designate);
            world.cycle_cockpit_weapon(0, true);
            old_manual(world, Live::ToggleArm);
        },
    );
    assert_eq!(out.commanded, 1);
    assert!(after.combat.state.own_view().designated().is_some());
}

#[test]
fn the_step_reports_the_state_after_the_commands_and_before_the_tick() {
    let mut world = warmed();
    let armed = world.combat.state.own().armed;
    let tick = world.tick();
    let mut out = TickOutput::default();
    let commanded = input(&world, vec![SeatCommand::Manual(Live::ToggleArm)]);
    let mut seen = None;
    world
        .step_observed(&[commanded], &mut out, |world, out| {
            seen = Some((world.combat.state.own().armed, world.tick(), out.cues.len()));
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, Some((!armed, tick, 0)));
    assert_eq!(world.tick(), tick + 1);
}

/// Two identical worlds; `old` does to the first what the frame loop did
/// before the tick, and the second is given `mission` commands and `commands`.
/// Both are given `sensors` as the tick's scope controls.
fn same_as_old_with(
    mission: Vec<MissionCommand>,
    sensors: tore_sim::sensors::Controls,
    commands: Vec<SeatCommand>,
    old: impl FnOnce(&mut World),
) -> (World, World, TickOutput) {
    let (mut before, mut after) = (warmed(), warmed());
    old(&mut before);
    let mut out = TickOutput::default();
    let mut plain = input(&before, Vec::new());
    plain.sensors = sensors;
    before.step(&[plain], &mut out).unwrap();
    let mut commanded = input(&after, commands);
    commanded.sensors = sensors;
    after
        .step_with(&mission, &[commanded], &mut out, |_, _| Ok(()))
        .unwrap();
    assert_eq!(state(&mut before), state(&mut after));
    (before, after, out)
}

#[test]
fn the_scope_controls_reach_the_flight_before_the_commands() {
    use tore_sim::sensors::{Channel, Controls};
    let controls = Controls {
        channel: Channel::Infrared,
        range_index: 2,
        history: true,
    };
    let (_, after, _) = same_as_old_with(
        Vec::new(),
        controls,
        vec![SeatCommand::Combat(Live::DesignateVisual)],
        |world| {
            world.cockpits[0].flight.sensors = controls;
            old_combat(world, Live::DesignateVisual);
        },
    );
    assert_eq!(after.cockpits[0].flight.sensors, controls);
}

#[test]
fn a_settings_change_reaches_every_part_of_the_mission_before_the_commands() {
    let cheats = tore_sim::cheats::Cheats {
        unlimited_ammo: true,
        no_crashes: true,
        guns_only: true,
        enemy_ai: Some(tore_sim::ai::Experience::Novice),
        ..Default::default()
    };
    let (_, after, _) = same_as_old_with(
        vec![MissionCommand::Settings(Settings { cheats })],
        Default::default(),
        vec![SeatCommand::Manual(Live::ToggleArm)],
        |world| {
            world.cockpits[0].flight.cheats = cheats;
            world.combat.state.cheats = cheats;
            let wings = world.ai_wings.as_mut().unwrap();
            wings.set_enemy_skill(cheats.enemy_ai);
            wings.set_guns_only(cheats.guns_only);
            old_manual(world, Live::ToggleArm);
        },
    );
    assert_eq!(after.cockpits[0].flight.cheats, cheats);
    assert_eq!(after.combat.state.cheats, cheats);
    // The commands see the new settings: the observer runs after them.
    let mut world = warmed();
    let mut out = TickOutput::default();
    let mut seen = None;
    let commanded = input(&world, vec![SeatCommand::Manual(Live::ToggleArm)]);
    world
        .step_with(
            &[MissionCommand::Settings(Settings { cheats })],
            &[commanded],
            &mut out,
            |world, _| {
                seen = Some(world.combat.state.cheats);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(seen, Some(cheats));
}

#[test]
fn radio_silence_toggles_and_says_so() {
    let (_, after, out) = same_as_old_with(
        Vec::new(),
        Default::default(),
        vec![SeatCommand::RadioSilence],
        |world| {
            world.comms.toggle_silence(crate::seats::SeatId(0));
        },
    );
    assert!(after.comms.radio_silence(crate::seats::SeatId(0)));
    assert!(matches!(&out.cues[0], Cue::Message(text) if text == "Radio silence"));
}

use tore_sim::ai::wing::{Formation, PlayerOrder};

/// The old handler for a wing order, minus the app's audio and HUD.
fn old_order(world: &mut World, order: PlayerOrder, recipient: Option<u8>) -> String {
    let now = world.combat.state.tick() as f64 / 120.;
    let selected = world.combat.state.own_view().designated();
    let report = world
        .ai_wings
        .as_mut()
        .unwrap()
        .command_at(crate::ai_wings::PLAYER_ID, order, selected, recipient, None)
        .unwrap();
    if !report.radio.is_empty() {
        world.comms.spoken(crate::seats::SeatId(0), now);
    }
    report.message
}

#[test]
fn wing_orders_match_the_old_path_and_the_order_call_comes_back() {
    for order in [
        PlayerOrder::AttackOnContact,
        PlayerOrder::EngageMyTarget,
        PlayerOrder::Disengage,
        PlayerOrder::Formation(Formation::ALL[1]),
    ] {
        let mut message = String::new();
        let (_, _, out) = same_as_old(vec![SeatCommand::WingOrder(order)], |world| {
            message = old_order(world, order, None)
        });
        assert!(
            matches!(&out.orders[..], [OrderReply { order: o, outcome: OrderOutcome::Given { message: m } }]
                if *o == order && *m == message),
            "{order:?}: {:?} against {message:?}",
            out.orders
        );
        assert!(matches!(out.cues[0], Cue::OrderVoice(_)));
        assert!(matches!(&out.cues[1], Cue::Message(text) if *text == message));
    }
}

#[test]
fn the_wing_recipient_is_the_seats_and_addresses_its_orders() {
    let (_, after, _) = same_as_old(
        vec![
            SeatCommand::WingRecipient(Some(2)),
            SeatCommand::WingOrder(PlayerOrder::Disengage),
        ],
        |world| {
            world
                .roster
                .set_wing_recipient(crate::seats::SeatId(0), Some(2));
            old_order(world, PlayerOrder::Disengage, Some(2));
        },
    );
    assert_eq!(
        after
            .roster
            .seat(crate::seats::SeatId(0))
            .unwrap()
            .wing_recipient,
        Some(2)
    );
}

#[test]
fn a_formation_cycle_orders_the_formation_after_the_wings_current_one() {
    let (_, _, out) = same_as_old(vec![SeatCommand::WingFormationCycle], |world| {
        let next = world
            .ai_wings
            .as_ref()
            .unwrap()
            .next_formation(crate::ai_wings::PLAYER_ID, None);
        old_order(world, PlayerOrder::Formation(next), None);
    });
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            order: PlayerOrder::Formation(_),
            outcome: OrderOutcome::Given { .. }
        }]
    ));
}

#[test]
fn a_wing_order_without_an_ai_wing_is_refused_and_journaled() {
    let (mut world, mut out) = (warmed(), TickOutput::default());
    world.ai_wings = None;
    let commanded = input(
        &world,
        vec![
            SeatCommand::WingOrder(PlayerOrder::Disengage),
            SeatCommand::WingFormationCycle,
        ],
    );
    world.step(&[commanded], &mut out).unwrap();
    assert!(matches!(
        &out.orders[..],
        [OrderReply {
            outcome: OrderOutcome::Refused { .. },
            ..
        }]
    ));
    assert!(
        matches!(&out.cues[0], Cue::Message(text) if text == "Wing order unavailable: no AI wing")
    );
    assert!(
        matches!(&out.cues[1], Cue::Message(text) if text == "Wing order unavailable: no AI wing")
    );
}

#[test]
fn land_at_selected_reports_a_refused_site_to_the_pilot_and_the_journal() {
    let (mut world, mut out) = (warmed(), TickOutput::default());
    let site = crate::ai_wings::AiWings::landing_site(
        &world.terrain.airport_scene,
        &world.terrain.airfield_anchors,
        &world.cockpits[0].airport_service,
    );
    let commanded = input(
        &world,
        vec![SeatCommand::WingOrder(PlayerOrder::LandAtSelected)],
    );
    world.step(&[commanded], &mut out).unwrap();
    match (site, &out.orders[..]) {
        (
            Err(message),
            [
                OrderReply {
                    outcome: OrderOutcome::Refused { message: given },
                    ..
                },
            ],
        ) => {
            assert_eq!(&message, given);
            assert!(matches!(&out.cues[0], Cue::Message(text) if *text == message));
            assert!(!format!("{:?}", world.comms.journal()).is_empty());
        }
        (Ok(_), [OrderReply { outcome, .. }]) => {
            assert!(!matches!(outcome, OrderOutcome::Refused { .. }));
        }
        (site, orders) => panic!("{site:?} gave {orders:?}"),
    }
}
