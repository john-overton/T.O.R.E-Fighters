//! Tests of the lobby screen: who may press which button in each state, the
//! slot clicks, the words that say why Fly cannot be pressed, the Leave
//! confirmation, Kick, chat, and the lines a change of state puts in Messages.
//! Built on the kit of synthetic pieces; no window, no session.
use super::facts::{self, FlyAs, Show, SlotClick};
use super::preview::{player, sample, slots};
use super::*;
use crate::widgets::test_kit;
use std::time::{Duration, Instant};
use tore_session::wire::messages::{LobbyPhase, StartRule};

fn screen_of(lobby: &LobbyState, hosting: bool) -> LobbyScreen {
    LobbyScreen::sample(Arc::new(test_kit::kit()), lobby.clone(), hosting)
}

/// The King's lobby with Goose's slot ready and the King holding a slot.
fn king() -> LobbyState {
    sample(1)
}

fn click(screen: &mut LobbyScreen, at: Point) -> Option<Request> {
    screen.moved(Some((f64::from(at.0), f64::from(at.1))));
    let down = screen.button(true);
    let up = screen.button(false);
    down.or(up)
}

fn button_centre(place: usize) -> Point {
    (SLOT_X[place] + 40, BUTTON_Y + 12)
}

fn slot_row(index: i32) -> Point {
    (100, 168 + 18 * index + 8)
}

#[test]
fn the_king_in_a_lobby_has_every_button_and_the_joiner_only_his_own() {
    let state = king();
    let king = facts::buttons(&Facts::of(Some(&state), None), false);
    assert!(king.mission.is_enabled(), "Mission... in the lobby");
    assert!(king.loadout.is_enabled(), "the King holds a slot");
    assert!(king.ready.is_enabled());
    assert_eq!(king.kick, Show::Disabled, "nobody is selected to kick");
    assert_eq!(king.fly, Show::Disabled, "the King is not ready yet");
    assert!(king.leave.is_enabled());
    assert!(
        facts::buttons(&Facts::of(Some(&state), None), true)
            .kick
            .is_enabled()
    );

    let joiner = facts::buttons(&Facts::of(Some(&sample(2)), None), true);
    for hidden in [joiner.mission, joiner.kick, joiner.fly] {
        assert_eq!(hidden, Show::Hidden, "the King's buttons are not offered");
    }
    assert!(joiner.loadout.is_enabled() && joiner.ready.is_enabled() && joiner.leave.is_enabled());
}

#[test]
fn a_player_with_no_slot_cannot_arm_or_ready() {
    let mut state = sample(2);
    state.players[1].slot = None;
    state.players[1].ready = false;
    state.slots = slots(&[(0, 1)]);
    let b = facts::buttons(&Facts::of(Some(&state), None), false);
    assert_eq!(b.loadout, Show::Disabled);
    assert_eq!(b.ready, Show::Disabled);
    let facts = Facts::of(Some(&state), None);
    assert_eq!(
        facts::disabled_reason(&facts, Id::Ready, false).as_deref(),
        Some("Take a slot first.")
    );
}

#[test]
fn fly_waits_for_everyone_holding_a_slot_and_says_who() {
    let mut state = king();
    let facts = Facts::of(Some(&state), None);
    assert_eq!(
        facts::fly_block(&facts).as_deref(),
        Some("Not ready: Maverick.")
    );
    // The host's own words for the same refusal.
    state.players[0].ready = true;
    let facts = Facts::of(Some(&state), None);
    assert_eq!(facts::fly_block(&facts), None);
    assert!(facts::buttons(&facts, false).fly.is_enabled());
    // Two waiting: both named, in the order they connected.
    state.players[0].ready = false;
    state.players[1].ready = false;
    let facts = Facts::of(Some(&state), None);
    assert_eq!(
        facts::fly_block(&facts).as_deref(),
        Some("Not ready: Maverick, Goose.")
    );
    // Nobody holds a slot.
    for p in &mut state.players {
        p.slot = None;
    }
    state.slots = slots(&[]);
    assert_eq!(
        facts::fly_block(&Facts::of(Some(&state), None)).as_deref(),
        Some("Nobody holds a slot.")
    );
}

#[test]
fn while_the_mission_flies_fly_ends_it_and_ready_joins_it() {
    let mut state = king();
    state.phase = LobbyPhase::Flying;
    let facts = Facts::of(Some(&state), None);
    let b = facts::buttons(&facts, false);
    assert_eq!(b.fly_as, FlyAs::EndMission);
    assert!(b.fly.is_enabled());
    assert_eq!(
        b.mission,
        Show::Disabled,
        "the mission changes only in the lobby"
    );
    assert_eq!(
        b.loadout,
        Show::Disabled,
        "loadouts are chosen in the lobby"
    );
    assert_eq!(facts::ready_label(&facts), "Join");
    assert!(b.ready.is_enabled());
    let mut screen = screen_of(&state, true);
    assert_eq!(
        click(&mut screen, button_centre(4)),
        Some(Request::EndMission)
    );
}

#[test]
fn the_blue_button_is_fly_when_it_can_be_pressed_else_ready() {
    let mut state = king();
    let screen = screen_of(&state, true);
    assert_eq!(screen.default_button(), Some(facts::DefaultButton::Ready));
    state.players[0].ready = true;
    let screen = screen_of(&state, true);
    assert_eq!(screen.default_button(), Some(facts::DefaultButton::Fly));
    // A joiner who is ready has nothing to press.
    let screen = screen_of(&sample(2), false);
    assert_eq!(screen.default_button(), None);
}

#[test]
fn clicking_a_free_slot_takes_it_and_clicking_ones_own_frees_it() {
    let mut state = sample(2);
    // Goose holds plane 1; Maverick plane 0.
    let mut screen = screen_of(&state, false);
    assert_eq!(click(&mut screen, slot_row(2)), Some(Request::Take(2)));
    // Own slot (row 1): freed.
    assert_eq!(click(&mut screen, slot_row(1)), Some(Request::LeaveSlot));
    // Another player's slot (row 0) is not clickable, and says why.
    assert_eq!(click(&mut screen, slot_row(0)), None);
    assert!(
        screen
            .message_lines()
            .last()
            .is_some_and(|l| l.contains("Another player holds"))
    );
    // A double-click is one click, not a take and a leave.
    state.players[1].slot = None;
    state.slots = slots(&[(0, 1)]);
    screen.update(Some(&state), None);
    screen.moved(Some((100., 168. + 18. * 3. + 8.)));
    assert_eq!(screen.button(true), Some(Request::Take(3)));
    screen.button(false);
    assert_eq!(
        screen.button(true),
        None,
        "the second press of a double-click"
    );
    screen.button(false);
}

#[test]
fn slot_clicks_are_refused_while_flying_or_unable() {
    let state = sample(2);
    let mut facts = Facts::of(Some(&state), None);
    facts.flying = true;
    assert!(matches!(
        facts::slot_click(&state, &facts, 3),
        SlotClick::Refused(_)
    ));
    let facts = Facts::of(Some(&state), Some("data differs"));
    assert!(matches!(
        facts::slot_click(&state, &facts, 3),
        SlotClick::Refused(_)
    ));
    assert_eq!(
        facts::slot_click(&state, &Facts::of(Some(&state), None), 3),
        SlotClick::Take(3)
    );
    assert_eq!(
        facts::slot_click(&state, &Facts::of(Some(&state), None), 1),
        SlotClick::Leave
    );
}

#[test]
fn enter_on_a_selected_slot_clicks_it() {
    let state = sample(2);
    let mut screen = screen_of(&state, false);
    screen.focus.set(Id::Slots);
    screen.key("ArrowDown", false);
    assert_eq!(screen.selected_slot(), Some(0));
    screen.key("ArrowDown", false);
    screen.key("ArrowDown", false);
    assert_eq!(screen.selected_slot(), Some(2));
    assert_eq!(screen.key("Enter", false), Some(Request::Take(2)));
}

#[test]
fn ready_toggles_and_says_when_the_standard_stores_are_flown() {
    let mut state = sample(2);
    state.players[1].ready = false;
    state.players[1].loadout = false;
    let mut screen = screen_of(&state, false);
    assert_eq!(
        click(&mut screen, button_centre(4)),
        Some(Request::SetReady(true))
    );
    assert!(
        screen
            .message_lines()
            .iter()
            .any(|l| l.contains("standard stores")),
        "{:?}",
        screen.message_lines()
    );
    // Armed: no such line, and Ready again takes it back.
    state.players[1].loadout = true;
    let mut screen = screen_of(&state, false);
    assert_eq!(
        click(&mut screen, button_centre(4)),
        Some(Request::SetReady(true))
    );
    assert!(
        !screen
            .message_lines()
            .iter()
            .any(|l| l.contains("standard"))
    );
    state.players[1].ready = true;
    screen.update(Some(&state), None);
    assert_eq!(
        click(&mut screen, button_centre(4)),
        Some(Request::SetReady(false))
    );
}

#[test]
fn the_kings_leave_asks_first_and_a_joiners_does_not() {
    let state = king();
    let mut screen = screen_of(&state, true);
    assert_eq!(click(&mut screen, button_centre(5)), None);
    assert!(screen.modal_open());
    assert!(screen.modal.as_ref().is_some_and(|m| {
        m.lines()
            .iter()
            .any(|l| l == "Leaving ends the game for everyone.")
    }));
    // Enter on the panel is Cancel (it has the focus); the game goes on.
    assert_eq!(screen.key("Enter", false), None);
    assert!(!screen.modal_open());
    // Esc is Leave: asks again; Esc closes the panel; the second Esc asks.
    assert_eq!(screen.key("Escape", false), None);
    assert!(screen.modal_open());
    assert_eq!(screen.key("Escape", false), None);
    assert!(!screen.modal_open());
    // Leave on the panel.
    screen.key("Escape", false);
    assert_eq!(screen.key("Tab", true), None);
    assert_eq!(screen.key("Enter", false), Some(Request::Leave));
    assert!(!screen.modal_open());

    let mut joiner = screen_of(&sample(2), false);
    assert_eq!(click(&mut joiner, button_centre(5)), Some(Request::Leave));
    assert_eq!(joiner.key("Escape", false), Some(Request::Leave));
}

#[test]
fn a_host_that_has_not_answered_yet_is_still_asked_before_leaving() {
    let mut screen = LobbyScreen::new(Arc::new(test_kit::kit()), "Maverick's game", true);
    assert_eq!(screen.key("Escape", false), None);
    assert!(screen.modal_open());
    let mut joining = LobbyScreen::new(Arc::new(test_kit::kit()), "Friday night", false);
    assert_eq!(joining.key("Escape", false), Some(Request::Leave));
}

#[test]
fn kick_asks_for_a_reason_and_names_the_player() {
    let state = king();
    let mut screen = screen_of(&state, true);
    // Nobody selected: Kick says what to do.
    assert_eq!(click(&mut screen, button_centre(3)), None);
    assert!(
        screen
            .message_lines()
            .last()
            .is_some_and(|l| l.contains("Select another player"))
    );
    // The King's own row is not kickable.
    screen.players.select(0);
    screen.refresh();
    assert_eq!(screen.buttons().kick, Show::Disabled);
    screen.players.select(1);
    screen.refresh();
    assert_eq!(screen.buttons().kick, Show::Enabled);
    assert_eq!(click(&mut screen, button_centre(3)), None);
    assert!(screen.modal_open() && screen.typing());
    screen.text_input("AFK too long");
    assert_eq!(
        screen.key("Enter", false),
        Some(Request::Kick {
            player: 2,
            reason: "AFK too long".into()
        })
    );
    assert!(!screen.modal_open());
    // Cancel keeps everyone.
    click(&mut screen, button_centre(3));
    assert!(screen.modal_open());
    assert_eq!(screen.key("Escape", false), None);
    assert!(!screen.modal_open());
}

#[test]
fn a_button_that_cannot_be_pressed_says_why_when_clicked() {
    let state = king();
    let mut screen = screen_of(&state, true);
    assert_eq!(click(&mut screen, button_centre(4)), None);
    assert_eq!(
        screen.message_lines().last().map(String::as_str),
        Some("Not ready: Maverick.")
    );
}

#[test]
fn enter_in_the_chat_line_sends_it_and_an_empty_line_presses_the_blue_button() {
    let state = king();
    let mut screen = screen_of(&state, true);
    assert!(screen.typing(), "typing starts in the line");
    screen.text_input("  hello all ");
    assert_eq!(screen.typed(), "  hello all ");
    assert_eq!(
        screen.key("Enter", false),
        Some(Request::Chat("hello all".into()))
    );
    assert_eq!(screen.typed(), "");
    // Nothing typed: Enter is the blue button (Ready here).
    assert_eq!(screen.key("Enter", false), Some(Request::SetReady(true)));
}

#[test]
fn a_dedicated_servers_lobby_has_no_kings_controls_and_states_its_rule() {
    let mut state = sample(2);
    state.king = None;
    state.host = None;
    state.start = StartRule::FirstReady;
    let facts = Facts::of(Some(&state), None);
    assert!(facts.server && !facts.king);
    let b = facts::buttons(&facts, true);
    assert_eq!(
        [b.mission, b.kick, b.fly],
        [Show::Hidden, Show::Hidden, Show::Hidden]
    );
    assert!(facts::rule_text(&state).contains("first player holding a slot"));
    state.start = StartRule::Flying;
    assert!(facts::rule_text(&state).contains("always flying"));
    // Leave never asks on a server.
    let mut screen = screen_of(&state, false);
    assert_eq!(click(&mut screen, button_centre(5)), Some(Request::Leave));
    // The rows have no crown: the King's column is empty for everyone.
    let rows = facts::player_rows(&state);
    assert!(
        rows.iter()
            .all(|r| r.cells[0] == crate::widgets::Cell::Empty)
    );
}

#[test]
fn the_players_list_marks_the_crown_the_house_ready_and_unable() {
    let mut state = king();
    state.players.push(player(3, "Viper", None));
    state.players[2].unable = Some("data differs".into());
    let rows = facts::player_rows(&state);
    use crate::widgets::{Cell, Icon};
    assert_eq!(rows[0].cells[0], Cell::Icon(Icon::Crown));
    assert_eq!(rows[0].cells[1], Cell::Icon(Icon::House));
    assert_eq!(rows[1].cells[2], Cell::Icon(Icon::Ready));
    assert_eq!(rows[2].cells[2], Cell::Icon(Icon::Unable));
    assert_eq!(rows[2].cells[5], Cell::Text("Unable".into()));
    // The platform sits beside the name: the sample players are on Linux
    // (id 1), Windows (2) and macOS (3), and an unnamed platform has no mark.
    assert_eq!(rows[0].cells[3], Cell::Icon(Icon::Linux));
    assert_eq!(rows[1].cells[3], Cell::Icon(Icon::Windows));
    assert_eq!(rows[2].cells[3], Cell::Icon(Icon::MacOs));
    assert_eq!(rows[0].cells[4], Cell::Text("Maverick".into()));
    state.players[0].platform = tore_session::wire::Platform::Unknown;
    assert_eq!(facts::player_rows(&state)[0].cells[3], Cell::Empty);
    assert!(rows[2].tint.is_some());
    assert_eq!(
        facts::player_detail(&state, 3).as_deref(),
        Some("Viper cannot play this mission: data differs")
    );
}

#[test]
fn the_slots_list_shows_who_holds_what_with_the_players_own_marked() {
    let state = sample(2);
    let rows = facts::slot_rows(&state);
    use crate::widgets::{Cell, Icon};
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].cells[3], Cell::Text("Maverick".into()));
    assert!(rows[0].dim, "another player's slot is dimmed");
    assert_eq!(rows[1].cells[0], Cell::Icon(Icon::You));
    assert_eq!(rows[1].cells[4], Cell::Icon(Icon::Ready));
    assert_eq!(rows[2].cells[3], Cell::Text("AI".into()));
    assert_eq!(rows[2].cells[1], Cell::Text("Wing 1 #3".into()));
    assert_eq!(rows[4].cells[2], Cell::Text("F/A-18D Hornet".into()));
    assert_eq!(rows[4].cells[1], Cell::Text("Wing 2 #1".into()));
}

#[test]
fn a_change_of_state_is_said_in_messages() {
    let old = king();
    let mut new = old.clone();
    new.players.push(player(3, "Viper", None));
    new.mission = 4;
    new.summary = "UKR, clear".into();
    let lines = facts::change_lines(Some(&old), &new);
    assert!(lines.contains(&"Viper joined the game.".to_string()));
    assert!(lines.contains(&"The mission is now: UKR, clear".to_string()));
    let lines = facts::change_lines(Some(&new), &old);
    assert!(lines.contains(&"Viper left the game.".to_string()));
    let mut flying = old.clone();
    flying.phase = LobbyPhase::Flying;
    assert!(
        facts::change_lines(Some(&old), &flying)
            .iter()
            .any(|l| l.contains("is flying"))
    );
    assert!(
        facts::change_lines(Some(&flying), &old)
            .iter()
            .any(|l| l == "Back in the lobby.")
    );
    assert!(facts::change_lines(Some(&old), &old).is_empty());
    // An unable player is named with the reason, once.
    let mut unable = old.clone();
    unable.players[1].unable = Some("data differs".into());
    assert!(
        facts::change_lines(Some(&old), &unable)
            .iter()
            .any(|l| l == "Goose cannot play this mission: data differs")
    );
    assert!(facts::change_lines(Some(&unable), &unable).is_empty());
}

#[test]
fn the_screen_tells_a_player_whose_own_game_cannot_play_the_mission() {
    let state = sample(2);
    let mut screen = screen_of(&state, false);
    screen.update(Some(&state), Some("Import the same version."));
    assert!(
        screen
            .message_lines()
            .iter()
            .any(|l| l.contains("Your game cannot play this mission: Import the same version."))
    );
    let b = screen.buttons();
    assert_eq!([b.loadout, b.ready], [Show::Disabled, Show::Disabled]);
    assert!(facts::hint(screen.facts()).contains("cannot play this mission"));
}

#[test]
fn hints_walk_the_player_through_the_lobby() {
    let mut state = sample(2);
    state.players[1].slot = None;
    state.players[1].ready = false;
    state.slots = slots(&[(0, 1)]);
    let hint = |s: &LobbyState| facts::hint(&Facts::of(Some(s), None));
    assert!(hint(&state).starts_with("Click a free slot"));
    state.players[1].slot = Some(1);
    state.slots = slots(&[(0, 1), (1, 2)]);
    assert!(hint(&state).starts_with("Choose a loadout"));
    state.players[1].loadout = true;
    assert!(hint(&state).starts_with("Press Ready"));
    state.players[1].ready = true;
    assert!(
        hint(&state).contains("Waiting for the King")
            || hint(&state).contains("Waiting for Maverick")
    );
    assert_eq!(
        facts::hint(&Facts::of(None, None)),
        "Connecting to the game..."
    );
}

#[test]
fn the_screen_draws_with_and_without_a_lobby_and_panels() {
    let kit = Arc::new(test_kit::kit());
    let mut pixels = test_kit::blank();
    let mut screen = LobbyScreen::new(Arc::clone(&kit), "Friday night", false);
    screen.draw(&mut Canvas(&mut pixels));
    let state = king();
    screen.update(Some(&state), None);
    screen.draw(&mut Canvas(&mut pixels));
    screen.focus.set(Id::Slots);
    screen.press(Id::Leave);
    screen.draw(&mut Canvas(&mut pixels));
    assert!(screen.modal_open());
}

/// Times the screen's update and draw on the imported pieces, a full lobby
/// (six players, eight slots, a chat line, a few lines of Messages). Run it
/// with a release build:
///
/// ```text
/// TORE_DATA_DIR=... cargo test --release -p tore-app --locked \
///     lobby_screen::tests::time_lobby_frame -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
fn time_lobby_frame() {
    use crate::widgets::KitSource;
    let data = crate::assets::data_directory().expect("data directory");
    let assets = crate::assets::Assets::load(&data).expect("an imported pack");
    let source = KitSource::of(&assets.pics, &assets.multiplayer_resources);
    let kit = Arc::new(source.build("MODEM3").expect("kit"));
    let mut state = sample(1);
    for (i, name) in ["Slider", "Viper", "Hollywood", "Wolfman"]
        .iter()
        .enumerate()
    {
        let mut p = player(3 + i as u8, name, Some(2 + i as u32));
        p.ready = i % 2 == 0;
        state.players.push(p);
    }
    state.slots = slots(&[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)]);
    let mut screen = LobbyScreen::sample(kit, state.clone(), true);
    for text in [
        "Slider joined the game.",
        "Viper joined the game.",
        "A third line.",
    ] {
        screen.say(text);
    }
    screen.text_input("typing a chat line");
    let mut pixels = test_kit::blank();
    let runs = 500;
    let (mut turn, mut draw, mut both) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..runs + 20 {
        let t = Instant::now();
        screen.update(Some(&state), None);
        let a = t.elapsed();
        let t = Instant::now();
        screen.draw(&mut Canvas(&mut pixels));
        let b = t.elapsed();
        if i >= 20 {
            turn.push(a);
            draw.push(b);
            both.push(a + b);
        }
    }
    for (name, mut times) in [("update", turn), ("draw", draw), ("update and draw", both)] {
        times.sort();
        let total: Duration = times.iter().sum();
        println!(
            "{name}, {runs} runs: mean {:?}, median {:?}, p95 {:?}, max {:?}",
            total / runs as u32,
            times[runs / 2],
            times[runs * 95 / 100],
            times[runs - 1]
        );
    }
}
