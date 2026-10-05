//! Tests of the lobby screen's stage F phase 2 panels (slice F2-L): who may
//! press what, every Settings row's greying and turning, the King's slot
//! locks, the Players panel, Watch, the head's summary and the lines a
//! change of settings puts in Messages. Built on the synthetic kit; no
//! window, no session.
use super::facts::{self, LoadoutAs, Show, SlotClick};
use super::players_panel::PlayersPanel;
use super::preview::{player, sample, sample_cheats, sample_pvp, slots};
use super::settings_panel::{
    self as sp, Context, Edit, Flag, Kind, Page, SettingsPanel, choices, page_rows, row_state,
    row_value, turn,
};
use super::*;
use crate::widgets::test_kit;
use tore_session::settings::{self, REGISTRY, number};
use tore_session::wire::messages::{LobbyPhase, PasswordChange};
use tore_sim::ai::Experience;
use tore_sim::cheats::Damage;

fn screen_of(lobby: &LobbyState, hosting: bool) -> LobbyScreen {
    let mut screen = LobbyScreen::sample(Arc::new(test_kit::kit()), lobby.clone(), hosting);
    screen.set_cheats(Some(sample_cheats()));
    screen
}

fn click(screen: &mut LobbyScreen, at: Point) -> Option<Request> {
    screen.moved(Some((f64::from(at.0), f64::from(at.1))));
    let down = screen.button(true);
    let up = screen.button(false);
    down.or(up)
}

fn right_click(screen: &mut LobbyScreen, at: Point) -> Option<Request> {
    screen.moved(Some((f64::from(at.0), f64::from(at.1))));
    let down = screen.right_button(true);
    let up = screen.right_button(false);
    down.or(up)
}

fn button_centre(place: usize) -> Point {
    (SLOT_X[place] + 37, BUTTON_Y + 12)
}

fn slot_row(index: i32) -> Point {
    (100, 168 + 18 * index + 8)
}

fn centre((x, y, w, h): crate::widgets::Rect) -> Point {
    (x + w / 2, y + h / 2)
}

/// The place of the value box of row `index` of `page`.
fn value_of(page: Page, index: usize) -> Point {
    centre(sp::place(page, index).value)
}

fn king() -> LobbyState {
    sample(1)
}

fn ctx(lobby: &LobbyState) -> Context {
    Context::of(lobby, Some(sample_cheats()))
}

fn row_index(page: Page, kind: Kind) -> usize {
    page_rows(page)
        .iter()
        .position(|k| *k == kind)
        .expect("the page has the row")
}

fn set(lobby: &mut LobbyState, number: u8, value: u32) {
    for entry in &mut lobby.settings {
        if entry.0 == number {
            entry.1 = value;
        }
    }
}

// ---- the buttons ----

#[test]
fn settings_is_for_everyone_and_players_for_the_king_with_a_target() {
    let king_facts = facts::Facts::of(Some(&king()), None);
    let b = facts::buttons(&king_facts, false);
    assert_eq!((b.settings, b.players), (Show::Enabled, Show::Disabled));
    assert_eq!(facts::buttons(&king_facts, true).players, Show::Enabled);
    let joiner = facts::buttons(&facts::Facts::of(Some(&sample(2)), None), true);
    assert_eq!(
        (joiner.settings, joiner.players),
        (Show::Enabled, Show::Hidden)
    );
    // Before the lobby's state arrives nothing opens.
    let waiting = facts::buttons(&facts::Facts::of(None, None), false);
    assert_eq!(waiting.settings, Show::Disabled);
    assert_eq!(
        facts::disabled_reason(
            &facts::Facts::of(Some(&king()), None),
            Id::PlayersPanel,
            false
        )
        .as_deref(),
        Some("Select another player in Players first.")
    );
}

#[test]
fn the_loadout_button_watches_while_the_mission_flies() {
    let mut state = sample(2);
    state.phase = LobbyPhase::Flying;
    state.players[0].flying = true;
    // A player with no plane may watch, with or without a slot held.
    for holds in [true, false] {
        let mut state = state.clone();
        if !holds {
            state.players[1].slot = None;
            state.slots = slots(&[(0, 1)]);
        }
        let b = facts::buttons(&facts::Facts::of(Some(&state), None), false);
        assert_eq!((b.loadout, b.loadout_as), (Show::Enabled, LoadoutAs::Watch));
    }
    // Watching: the same place stops it.
    state.players[1].observing = true;
    let b = facts::buttons(&facts::Facts::of(Some(&state), None), false);
    assert_eq!(
        (b.loadout, b.loadout_as),
        (Show::Enabled, LoadoutAs::StopWatch)
    );
    // A player flying already has nothing to watch.
    state.players[1].observing = false;
    state.players[1].flying = true;
    let b = facts::buttons(&facts::Facts::of(Some(&state), None), false);
    assert_eq!(b.loadout, Show::Disabled);
    assert_eq!(
        facts::disabled_reason(&facts::Facts::of(Some(&state), None), Id::Loadout, false)
            .as_deref(),
        Some("You are flying.")
    );
    // In the lobby it is the Loadout button again.
    let b = facts::buttons(&facts::Facts::of(Some(&sample(2)), None), false);
    assert_eq!(b.loadout_as, LoadoutAs::Loadout);
}

#[test]
fn watch_and_stop_watch_are_requested_by_the_button() {
    let mut state = sample(2);
    state.phase = LobbyPhase::Flying;
    state.players[0].flying = true;
    let mut screen = screen_of(&state, false);
    assert_eq!(screen.loadout.label(), "Watch");
    // The joiner's row: Settings 3, Loadout 4, Ready 5, Leave 6.
    assert_eq!(click(&mut screen, button_centre(4)), Some(Request::Watch));
    state.players[1].observing = true;
    screen.update(Some(&state), None);
    assert_eq!(screen.loadout.label(), "Stop Watch");
    assert_eq!(
        click(&mut screen, button_centre(4)),
        Some(Request::StopWatch)
    );
}

#[test]
fn the_kings_seven_buttons_and_the_joiners_four_have_their_places() {
    let mut screen = screen_of(&king(), true);
    let labels: Vec<&str> = [
        &screen.mission,
        &screen.settings,
        &screen.players_button,
        &screen.loadout,
        &screen.ready,
        &screen.fly,
        &screen.leave,
    ]
    .iter()
    .map(|b| b.label())
    .collect();
    assert_eq!(
        labels,
        [
            "Mission...",
            "Settings...",
            "Players...",
            "Loadout",
            "Ready",
            "Fly",
            "Leave"
        ]
    );
    // All seven fit the 549 wide row without touching.
    for pair in SLOT_X.windows(2) {
        assert!(pair[1] - pair[0] >= BUTTON_W, "{pair:?}");
    }
    assert!(SLOT_X[6] + BUTTON_W <= 45 + 549);
    // Settings... is the second button for the King, the first of four for a
    // joiner (right-aligned so Leave stays last).
    assert_eq!(click(&mut screen, button_centre(1)), None);
    assert!(screen.settings_open().is_some());
    let mut joiner = screen_of(&sample(2), false);
    assert_eq!(click(&mut joiner, button_centre(3)), None);
    assert!(joiner.settings_open().is_some());
}

// ---- the Settings rows ----

#[test]
fn every_setting_of_the_registry_is_on_a_page() {
    let mut seen: Vec<u8> = Vec::new();
    for page in Page::ALL {
        for kind in page_rows(page) {
            match kind {
                Kind::Setting(n) => seen.push(n),
                Kind::Password => seen.push(number::PASSWORD),
                _ => {}
            }
        }
    }
    seen.sort_unstable();
    let all: Vec<u8> = REGISTRY.iter().map(|s| s.number).collect();
    assert_eq!(
        seen, all,
        "each registry setting is one row, the password on the Game page"
    );
    // The name is the Game page's first row; Realism has Damage, Enemy AI
    // and the thirteen switches.
    assert_eq!(page_rows(Page::Game)[0], Kind::Name);
    assert_eq!(page_rows(Page::Realism).len(), 15);
}

#[test]
fn every_row_fits_inside_the_panel_and_rows_do_not_overlap() {
    let inside = |(x, y, w, h): crate::widgets::Rect| {
        x >= sp::PANEL.0 + 12
            && y >= sp::PANEL.1 + 24
            && x + w <= sp::PANEL.0 + sp::PANEL.2 - 12
            && y + h <= sp::PANEL.1 + 300
    };
    for page in Page::ALL {
        let rows = page_rows(page);
        for i in 0..rows.len() {
            let a = sp::place(page, i);
            assert!(inside(a.row), "{page:?} row {i}: {:?}", a.row);
            assert!(inside(a.value), "{page:?} row {i} value");
            for j in i + 1..rows.len() {
                let b = sp::place(page, j);
                let apart = a.row.0 + a.row.2 <= b.row.0
                    || b.row.0 + b.row.2 <= a.row.0
                    || a.row.1 + a.row.3 <= b.row.1
                    || b.row.1 + b.row.3 <= a.row.1;
                assert!(apart, "{page:?} rows {i} and {j} overlap");
            }
        }
    }
}

#[test]
fn only_the_king_turns_a_row_and_a_joiner_is_told_so() {
    let joiner = ctx(&sample(2));
    for page in Page::ALL {
        for kind in page_rows(page) {
            assert_eq!(
                row_state(kind, &joiner),
                Err("Only the King may change the settings.".to_owned()),
                "{kind:?}"
            );
            assert_eq!(sp::click(kind, &joiner, true), None);
        }
    }
    let king = ctx(&king());
    assert_eq!(
        row_state(Kind::Setting(number::FRIENDLY_FIRE), &king),
        Ok(())
    );
}

#[test]
fn pvp_rows_are_greyed_in_co_op_except_the_time_limit() {
    let coop = ctx(&king());
    for n in [
        number::FIGHT,
        number::TALLY,
        number::KILL_LIMIT,
        number::KILL_OWNER,
        number::OBSERVER_DELAY,
    ] {
        assert_eq!(
            row_state(Kind::Setting(n), &coop),
            Err("This applies only in a PvP game.".to_owned()),
            "setting {n}"
        );
    }
    // The time limit ends a co-op mission too.
    assert_eq!(row_state(Kind::Setting(number::TIME_LIMIT), &coop), Ok(()));
    let pvp = ctx(&sample_pvp(1));
    for n in [number::FIGHT, number::KILL_LIMIT, number::OBSERVER_DELAY] {
        assert_eq!(row_state(Kind::Setting(n), &pvp), Ok(()), "setting {n}");
    }
}

#[test]
fn while_the_mission_flies_only_the_any_time_rows_turn() {
    let mut state = sample_pvp(1);
    state.phase = LobbyPhase::Flying;
    let flying = ctx(&state);
    let words = "Change it in the lobby, between missions.".to_owned();
    for n in [
        number::MODE,
        number::FRIENDLY_FIRE,
        number::LOCK_SIDES,
        number::LOADOUTS,
        number::RESPAWN,
        number::TIME_LIMIT,
        number::KILL_LIMIT,
    ] {
        assert_eq!(
            row_state(Kind::Setting(n), &flying),
            Err(words.clone()),
            "setting {n}"
        );
    }
    for kind in [
        Kind::Setting(number::MAX_PLAYERS),
        Kind::Setting(number::JOIN_IN_PROGRESS),
        Kind::Setting(number::VISIBILITY),
        Kind::Setting(number::IDLE_AI),
        Kind::Name,
        Kind::Password,
    ] {
        assert_eq!(row_state(kind, &flying), Ok(()), "{kind:?}");
    }
    // The cheats are the mission's: fixed once it flies.
    for kind in [Kind::Damage, Kind::EnemyAi, Kind::Cheat(Flag::NoSpins)] {
        assert_eq!(row_state(kind, &flying), Err(words.clone()), "{kind:?}");
    }
    // Before the mission arrives the Realism rows wait for it.
    let none = Context::of(&king(), None);
    assert_eq!(
        row_state(Kind::Damage, &none),
        Err("The mission has not arrived yet.".to_owned())
    );
}

#[test]
fn a_click_turns_a_setting_through_every_value_and_round() {
    // Every setting's rows wrap: forward n times returns to the start, and
    // the first value after the last is the first.
    for setting in &REGISTRY {
        if setting.number == number::PASSWORD {
            continue;
        }
        let values = choices(setting);
        assert!(!values.is_empty());
        for (i, value) in values.iter().enumerate() {
            let next = turn(&values, *value, true).expect("a next value");
            assert_eq!(next, values[(i + 1) % values.len()], "{}", setting.name);
            assert_eq!(turn(&values, next, false), Some(*value), "{}", setting.name);
            assert!(setting.allows(next), "{} allows {next}", setting.name);
        }
    }
}

#[test]
fn clicks_make_the_edit_the_host_checks() {
    let mut state = king();
    // Mode: co-op to pvp, in one change.
    let edit = sp::click(Kind::Setting(number::MODE), &ctx(&state), true);
    assert_eq!(
        edit,
        Some(Edit::Settings(SettingsChange {
            values: vec![(number::MODE, 1)],
            ..SettingsChange::default()
        }))
    );
    // Right click turns back (wrapping to the last value).
    set(&mut state, number::VISIBILITY, 0);
    let back = sp::click(Kind::Setting(number::VISIBILITY), &ctx(&state), false);
    assert_eq!(
        back,
        Some(Edit::Settings(SettingsChange {
            values: vec![(number::VISIBILITY, 2)],
            ..SettingsChange::default()
        }))
    );
    // Lives pass 10 to unlimited (255).
    set(&mut state, number::LIVES, 10);
    let lives = sp::click(Kind::Setting(number::LIVES), &ctx(&state), true);
    assert_eq!(
        lives,
        Some(Edit::Settings(SettingsChange {
            values: vec![(number::LIVES, settings::UNLIMITED_LIVES)],
            ..SettingsChange::default()
        }))
    );
    // A time limit off the King's list (a server's file gives any minute)
    // goes to the next listed value in the direction asked.
    set(&mut state, number::TIME_LIMIT, 420);
    for (forward, want) in [(true, 600), (false, 300)] {
        let edit = sp::click(Kind::Setting(number::TIME_LIMIT), &ctx(&state), forward);
        assert_eq!(
            edit,
            Some(Edit::Settings(SettingsChange {
                values: vec![(number::TIME_LIMIT, want)],
                ..SettingsChange::default()
            }))
        );
    }
}

#[test]
fn rows_read_the_hosts_values_in_words() {
    let state = sample_pvp(1);
    let c = ctx(&state);
    let word = |n| row_value(Kind::Setting(n), &c);
    assert_eq!(word(number::MODE), "pvp");
    assert_eq!(word(number::TIME_LIMIT), "10 minutes");
    assert_eq!(word(number::KILL_LIMIT), "5");
    assert_eq!(word(number::LIVES), "unlimited");
    assert_eq!(word(number::REVIVE_DISTANCE), "10 nm");
    assert_eq!(word(number::RESPAWN), "revive");
    assert_eq!(word(number::IDLE_AI), "10 seconds");
    assert_eq!(row_value(Kind::Name, &c), "Maverick's game");
    assert_eq!(row_value(Kind::Password, &c), "none");
    let mut with = state.clone();
    set(&mut with, number::PASSWORD, 1);
    assert_eq!(row_value(Kind::Password, &ctx(&with)), "set");
}

// ---- Realism ----

#[test]
fn the_realism_rows_edit_the_missions_cheats() {
    let c = ctx(&king());
    // A switch toggles both ways.
    let Some(Edit::Cheats(on)) = sp::click(Kind::Cheat(Flag::EasyAiming), &c, true) else {
        panic!("a cheat edit");
    };
    assert!(on.easy_aiming);
    assert!(on.unlimited_fuel && on.no_spins, "the rest are kept");
    let Some(Edit::Cheats(off)) = sp::click(Kind::Cheat(Flag::UnlimitedFuel), &c, true) else {
        panic!("a cheat edit");
    };
    assert!(!off.unlimited_fuel);
    // Damage: Normal, Invulnerable, Realistic and round.
    let mut cheats = sample_cheats();
    let mut seen = Vec::new();
    for _ in 0..3 {
        let c = Context::of(&king(), Some(cheats));
        let Some(Edit::Cheats(next)) = sp::click(Kind::Damage, &c, true) else {
            panic!("a cheat edit");
        };
        seen.push(next.damage);
        cheats = next;
    }
    assert_eq!(
        seen,
        [Damage::Invulnerable, Damage::Realistic, Damage::Normal]
    );
    let c = Context::of(&king(), Some(cheats));
    let Some(Edit::Cheats(back)) = sp::click(Kind::Damage, &c, false) else {
        panic!("a cheat edit");
    };
    assert_eq!(back.damage, Damage::Realistic);
    // Enemy AI: unchanged, then the four levels, and back round.
    let mut cheats = sample_cheats();
    let mut levels = Vec::new();
    for _ in 0..5 {
        let c = Context::of(&king(), Some(cheats));
        let Some(Edit::Cheats(next)) = sp::click(Kind::EnemyAi, &c, true) else {
            panic!("a cheat edit");
        };
        levels.push(next.enemy_ai);
        cheats = next;
    }
    assert_eq!(
        levels,
        [
            Some(Experience::Novice),
            Some(Experience::Average),
            Some(Experience::Experienced),
            Some(Experience::Ace),
            None
        ]
    );
    assert_eq!(row_value(Kind::Cheat(Flag::UnlimitedFuel), &c), "On");
    assert_eq!(row_value(Kind::Cheat(Flag::EasyAiming), &c), "Off");
    assert_eq!(row_value(Kind::EnemyAi, &c), "Unchanged");
}

#[test]
fn a_cheat_edit_becomes_a_mission_change_request() {
    let mut screen = screen_of(&king(), true);
    click(&mut screen, button_centre(1));
    screen
        .settings_panel
        .as_mut()
        .unwrap()
        .show_page(Page::Realism);
    let at = value_of(
        Page::Realism,
        row_index(Page::Realism, Kind::Cheat(Flag::NoCrashes)),
    );
    let Some(Request::Cheats(cheats)) = click(&mut screen, at) else {
        panic!("a cheats request");
    };
    assert!(cheats.no_crashes && cheats.no_spins);
}

// ---- the name and the password ----

#[test]
fn the_name_and_password_lines_apply_with_enter() {
    let c = ctx(&king());
    assert_eq!(
        sp::apply_name(&c, "Night owls"),
        Ok(Edit::Settings(SettingsChange {
            name: Some("Night owls".into()),
            ..SettingsChange::default()
        }))
    );
    assert!(sp::apply_name(&c, "   ").is_err());
    assert_eq!(
        sp::apply_name(&c, "Maverick's game"),
        Err("That is the name already.".to_owned())
    );
    assert!(sp::apply_name(&c, &"x".repeat(65)).is_err());
    assert_eq!(
        sp::apply_password(&c, "swordfish"),
        Ok(Edit::Settings(SettingsChange {
            password: Some(PasswordChange::Set("swordfish".into())),
            ..SettingsChange::default()
        }))
    );
    // An empty line with no password set has nothing to clear.
    assert!(sp::apply_password(&c, "").is_err());
    let mut with = king();
    set(&mut with, number::PASSWORD, 1);
    assert_eq!(
        sp::apply_password(&ctx(&with), ""),
        Ok(Edit::Settings(SettingsChange {
            password: Some(PasswordChange::Clear),
            ..SettingsChange::default()
        }))
    );
    // A joiner changes neither.
    assert!(sp::apply_name(&ctx(&sample(2)), "Mine").is_err());
}

// ---- the panel on the screen ----

#[test]
fn the_panel_turns_a_row_forward_on_left_and_back_on_right() {
    let mut screen = screen_of(&king(), true);
    assert_eq!(click(&mut screen, button_centre(1)), None);
    let panel = screen.settings_open().expect("open");
    assert_eq!(panel.page(), Page::Game);
    let at = value_of(
        Page::Game,
        row_index(Page::Game, Kind::Setting(number::FRIENDLY_FIRE)),
    );
    let want = |value| {
        Some(Request::Settings(SettingsChange {
            values: vec![(number::FRIENDLY_FIRE, value)],
            ..SettingsChange::default()
        }))
    };
    assert_eq!(click(&mut screen, at), want(0));
    assert_eq!(right_click(&mut screen, at), want(0));
    // The panel shows what the host says, so a lobby state turns the row.
    let mut state = king();
    set(&mut state, number::FRIENDLY_FIRE, 0);
    screen.update(Some(&state), None);
    assert_eq!(
        row_value(
            Kind::Setting(number::FRIENDLY_FIRE),
            screen.settings_open().unwrap().context()
        ),
        "off"
    );
    assert_eq!(click(&mut screen, at), want(1));
}

#[test]
fn a_joiner_can_read_the_panel_but_changes_nothing() {
    let mut screen = screen_of(&sample(2), false);
    click(&mut screen, button_centre(3));
    let at = value_of(
        Page::Game,
        row_index(Page::Game, Kind::Setting(number::FRIENDLY_FIRE)),
    );
    assert_eq!(click(&mut screen, at), None);
    assert_eq!(right_click(&mut screen, at), None);
    assert_eq!(
        screen.settings_open().unwrap().notice(),
        Some("Only the King may change the settings.")
    );
    // Keys do nothing either.
    assert_eq!(screen.key("ArrowRight", false), None);
    assert!(!screen.typing(), "a joiner types in no line of the panel");
    assert_eq!(screen.key("Escape", false), None);
    assert!(screen.settings_open().is_none());
}

#[test]
fn tab_and_the_tabs_change_the_page_and_escape_closes() {
    let mut screen = screen_of(&king(), true);
    click(&mut screen, button_centre(1));
    let mut seen = Vec::new();
    for _ in 0..4 {
        seen.push(screen.settings_open().unwrap().page());
        screen.key("Tab", false);
    }
    assert_eq!(seen, Page::ALL);
    assert_eq!(
        screen.settings_open().unwrap().page(),
        Page::Game,
        "wrapped"
    );
    screen.key("Tab", true);
    assert_eq!(screen.settings_open().unwrap().page(), Page::Realism);
    // A tab click.
    let revival = (sp::PANEL.0 + 22 + 92 + 40, sp::PANEL.1 + 32 + 12);
    click(&mut screen, revival);
    assert_eq!(screen.settings_open().unwrap().page(), Page::Revival);
    assert!(screen.settings_open().unwrap().tab_default(Page::Revival));
    assert!(!screen.settings_open().unwrap().tab_default(Page::Game));
    // The lobby behind takes no click while the panel is up, and Esc closes
    // the panel (it does not ask to leave).
    assert_eq!(click(&mut screen, button_centre(6)), None);
    assert!(
        !screen.modal_open(),
        "Leave behind the panel was not pressed"
    );
    assert_eq!(screen.key("Escape", false), None);
    assert!(screen.settings_open().is_none() && !screen.modal_open());
    // Close by its button.
    click(&mut screen, button_centre(1));
    let close = (sp::PANEL.0 + sp::PANEL.2 / 2, sp::PANEL.1 + 322 + 12);
    click(&mut screen, close);
    assert!(screen.settings_open().is_none());
}

#[test]
fn the_keyboard_selects_and_turns_rows_and_types_in_the_lines() {
    let mut screen = screen_of(&king(), true);
    click(&mut screen, button_centre(1));
    // The first row is the name line: typing goes to it.
    assert!(screen.typing());
    for _ in 0..4 {
        screen.key("Backspace", false);
    }
    screen.text_input("pack");
    let panel = screen.settings_open().unwrap();
    assert_eq!(panel.name_text(), "Maverick's pack");
    assert_eq!(
        screen.key("Enter", false),
        Some(Request::Settings(SettingsChange {
            name: Some("Maverick's pack".into()),
            ..SettingsChange::default()
        }))
    );
    // Down to Game type: Right turns it, Left turns it back.
    screen.key("ArrowDown", false);
    assert!(!screen.typing());
    let mode = |value| {
        Some(Request::Settings(SettingsChange {
            values: vec![(number::MODE, value)],
            ..SettingsChange::default()
        }))
    };
    assert_eq!(screen.key("ArrowRight", false), mode(1));
    assert_eq!(screen.key("ArrowLeft", false), mode(1));
    assert_eq!(screen.key("Enter", false), mode(1));
    // Up wraps to the last row.
    screen.key("ArrowUp", false);
    screen.key("ArrowUp", false);
    assert_eq!(
        screen.settings_open().unwrap().selected(),
        page_rows(Page::Game).len() - 1
    );
}

#[test]
fn the_password_line_sets_and_clears() {
    let mut screen = screen_of(&king(), true);
    click(&mut screen, button_centre(1));
    let at = value_of(Page::Game, row_index(Page::Game, Kind::Password));
    assert_eq!(click(&mut screen, at), None);
    assert!(screen.typing());
    screen.text_input("swordfish");
    assert_eq!(
        screen.key("Enter", false),
        Some(Request::Settings(SettingsChange {
            password: Some(PasswordChange::Set("swordfish".into())),
            ..SettingsChange::default()
        }))
    );
    // Sent: the line is empty again, and the host's state says it is set.
    let mut state = king();
    set(&mut state, number::PASSWORD, 1);
    screen.update(Some(&state), None);
    assert_eq!(
        screen.key("Enter", false),
        Some(Request::Settings(SettingsChange {
            password: Some(PasswordChange::Clear),
            ..SettingsChange::default()
        }))
    );
}

#[test]
fn a_refusal_from_the_host_is_read_in_the_panel() {
    let mut screen = screen_of(&king(), true);
    assert!(!screen.refused_in_panel("closed"), "no panel, no taker");
    click(&mut screen, button_centre(1));
    assert!(screen.refused_in_panel("2 players are connected: the limit cannot be lower."));
    assert_eq!(
        screen.settings_open().unwrap().notice(),
        Some("2 players are connected: the limit cannot be lower.")
    );
    // The next turn clears it.
    let at = value_of(
        Page::Game,
        row_index(Page::Game, Kind::Setting(number::MODE)),
    );
    click(&mut screen, at);
    assert_eq!(screen.settings_open().unwrap().notice(), None);
}

#[test]
fn every_page_draws_for_the_king_and_a_joiner() {
    let kit = Arc::new(test_kit::kit());
    for king in [true, false] {
        for page in Page::ALL {
            let mut screen = if king {
                screen_of(&sample_pvp(1), true)
            } else {
                screen_of(&sample(2), false)
            };
            click(&mut screen, button_centre(if king { 1 } else { 3 }));
            screen.settings_panel.as_mut().unwrap().show_page(page);
            let mut pixels = test_kit::blank();
            screen.draw(&mut Canvas(&mut pixels));
        }
    }
    let mut panel = PlayersPanel::new(2, "Goose", false);
    let mut pixels = test_kit::blank();
    panel.draw(&mut Canvas(&mut pixels), &kit);
    panel = PlayersPanel::new(1, "Maverick", true);
    panel.draw(&mut Canvas(&mut pixels), &kit);
    let _ = SettingsPanel::new(ctx(&king()));
}

// ---- the Players panel ----

#[test]
fn give_crown_passes_it_and_closes_the_panel() {
    let mut screen = screen_of(&king(), true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, button_centre(2));
    let panel = screen.players_open().expect("open");
    assert!(panel.lines()[0].contains("Goose"));
    assert!(!panel.house());
    // The Give crown button, at the panel's left.
    let give = (110 + 40 + 50, 150 + 132 + 12);
    assert_eq!(click(&mut screen, give), Some(Request::PassCrown(2)));
    assert!(screen.players_open().is_none());
}

#[test]
fn the_house_cannot_be_kicked_from_the_panel() {
    let mut state = king();
    state.host = Some(2);
    let mut screen = screen_of(&state, true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, button_centre(2));
    let panel = screen.players_open().expect("open");
    assert!(panel.house() && !panel.kick_enabled());
    assert!(panel.lines()[1].contains("cannot be kicked"));
    // Shift+Tab from Close skips Kick and lands on Give crown.
    assert_eq!(screen.key("Tab", true), None);
    assert_eq!(screen.key("Enter", false), Some(Request::PassCrown(2)));
    assert!(screen.players_open().is_none());
    click(&mut screen, button_centre(2));
    assert!(screen.players_open().is_some());
    // The Kick button itself does nothing.
    let kick = (110 + 160 + 40, 150 + 132 + 12);
    assert_eq!(click(&mut screen, kick), None);
    assert!(screen.players_open().is_some() && !screen.modal_open());
    assert_eq!(screen.key("Escape", false), None);
    assert!(screen.players_open().is_none());
}

#[test]
fn the_panel_closes_when_its_player_leaves_or_the_crown_goes() {
    let mut state = king();
    state.players.push(player(3, "Hollywood", None));
    let mut screen = screen_of(&state, true);
    screen.players.select(2);
    screen.refresh();
    click(&mut screen, button_centre(2));
    assert!(screen.players_open().is_some());
    // Hollywood leaves.
    state.players.pop();
    screen.update(Some(&state), None);
    assert!(screen.players_open().is_none());
    // The crown passes to Goose while the panel is up: it is not ours now.
    let mut state = king();
    let mut screen = screen_of(&state, true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, button_centre(2));
    assert!(screen.players_open().is_some());
    state.king = Some(2);
    screen.update(Some(&state), None);
    assert!(screen.players_open().is_none());
    assert_eq!(screen.buttons().players, Show::Hidden);
}

// ---- slot locks ----

#[test]
fn the_kings_right_click_closes_opens_and_reserves_slots() {
    let mut screen = screen_of(&king(), true);
    // Plane 2 is free: closed.
    assert_eq!(
        right_click(&mut screen, slot_row(2)),
        Some(Request::Lock {
            plane: 2,
            lock: Lock::Closed
        })
    );
    // With Goose selected in Players the same click keeps it for Goose.
    screen.players.select(1);
    screen.refresh();
    assert_eq!(
        right_click(&mut screen, slot_row(2)),
        Some(Request::Lock {
            plane: 2,
            lock: Lock::Reserved("Goose".into())
        })
    );
    // A closed slot opens again; a slot reserved for the selected player
    // opens too.
    let mut state = king();
    state.slots[2].lock = Lock::Closed;
    state.slots[3].lock = Lock::Reserved("Goose".into());
    screen.update(Some(&state), None);
    screen.players.clear_selection();
    screen.refresh();
    assert_eq!(
        right_click(&mut screen, slot_row(2)),
        Some(Request::Lock {
            plane: 2,
            lock: Lock::Open
        })
    );
    screen.players.select(1);
    screen.refresh();
    assert_eq!(
        right_click(&mut screen, slot_row(3)),
        Some(Request::Lock {
            plane: 3,
            lock: Lock::Open
        })
    );
    // Not a row: nothing.
    assert_eq!(right_click(&mut screen, (60, 270)), None);
    // A joiner's right click does nothing.
    let mut joiner = screen_of(&sample(2), false);
    assert_eq!(right_click(&mut joiner, slot_row(2)), None);
}

#[test]
fn locks_show_in_the_slot_rows_and_refuse_the_wrong_take() {
    let mut state = king();
    state.players.push(player(3, "Hollywood", None));
    state.slots[2].lock = Lock::Closed;
    state.slots[3].lock = Lock::Reserved("Hollywood".into());
    state.slots[4].lock = Lock::Reserved("Maverick".into());
    let rows = facts::slot_rows(&state);
    let text = |row: usize| match &rows[row].cells[3] {
        crate::widgets::Cell::Text(t) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(text(2), "Closed (AI)");
    assert_eq!(text(3), "Reserved: Hollywood");
    assert_eq!(text(4), "Reserved: Maverick");
    assert_eq!(text(5), "AI");
    // Maverick's own take of a slot kept for him goes through.
    let facts = facts::Facts::of(Some(&state), None);
    assert_eq!(facts::slot_click(&state, &facts, 4), SlotClick::Take(4));
    assert_eq!(
        facts::slot_click(&state, &facts, 2),
        SlotClick::Refused("Plane 2 is closed: the AI flies it.".into())
    );
    assert_eq!(
        facts::slot_click(&state, &facts, 3),
        SlotClick::Refused("Plane 3 is kept for Hollywood.".into())
    );
    // The screen says the same in Messages.
    let mut screen = screen_of(&state, true);
    assert_eq!(click(&mut screen, slot_row(2)), None);
    assert_eq!(
        screen.message_lines().last().map(String::as_str),
        Some("Plane 2 is closed: the AI flies it.")
    );
    assert_eq!(click(&mut screen, slot_row(4)), Some(Request::Take(4)));
}

#[test]
fn the_lock_click_and_its_words() {
    let mut state = king();
    let slot = &mut state.slots[2];
    assert_eq!(facts::lock_click(slot, None), Lock::Closed);
    assert_eq!(
        facts::lock_click(slot, Some("Hawk")),
        Lock::Reserved("Hawk".into())
    );
    slot.lock = Lock::Closed;
    assert_eq!(facts::lock_click(slot, None), Lock::Open);
    slot.lock = Lock::Reserved("Hawk".into());
    assert_eq!(facts::lock_click(slot, None), Lock::Open);
    assert_eq!(facts::lock_click(slot, Some("Hawk")), Lock::Open);
    assert_eq!(
        facts::lock_click(slot, Some("Owl")),
        Lock::Reserved("Owl".into())
    );
    assert_eq!(facts::lock_line(3, &Lock::Open), "Plane 3's slot is open.");
    assert_eq!(
        facts::lock_line(3, &Lock::Closed),
        "Plane 3's slot is closed: the AI flies it."
    );
    assert_eq!(
        facts::lock_line(3, &Lock::Reserved("Hawk".into())),
        "Plane 3's slot is kept for Hawk."
    );
}

// ---- the head and Messages ----

#[test]
fn the_head_sums_up_the_settings_in_words() {
    assert_eq!(
        facts::settings_summary(&king()).as_deref(),
        Some("Co-op, friendly fire on, no revival")
    );
    assert_eq!(
        facts::settings_summary(&sample_pvp(1)).as_deref(),
        Some("PvP by sides, 5 kills or 10 minutes, revival with unlimited lives, sides locked")
    );
    let mut state = sample_pvp(1);
    set(&mut state, number::KILL_LIMIT, 1);
    set(&mut state, number::TIME_LIMIT, 0);
    set(&mut state, number::LIVES, 1);
    set(&mut state, number::FRIENDLY_FIRE, 0);
    set(&mut state, number::FIGHT, 1);
    set(&mut state, number::LOCK_SIDES, 0);
    set(&mut state, number::LOADOUTS, 1);
    assert_eq!(
        facts::settings_summary(&state).as_deref(),
        Some(
            "PvP, every player for itself, friendly fire off, 1 kill, revival with 1 life, any loadout"
        )
    );
    let mut co = king();
    set(&mut co, number::TIME_LIMIT, 1_800);
    set(&mut co, number::RESPAWN, 1);
    assert_eq!(
        facts::settings_summary(&co).as_deref(),
        Some("Co-op, friendly fire on, 30 minutes, revival in a free AI aircraft")
    );
    // The kill limit is a PvP setting: a co-op value is not mentioned.
    set(&mut co, number::KILL_LIMIT, 5);
    assert!(!facts::settings_summary(&co).unwrap().contains("kill"));
    // A host that sent none (an older state) leaves the line out.
    let mut none = king();
    none.settings.clear();
    assert_eq!(facts::settings_summary(&none), None);
}

#[test]
fn a_change_of_settings_name_or_lock_is_said_in_messages() {
    let old = king();
    let mut new = old.clone();
    set(&mut new, number::MODE, 1);
    set(&mut new, number::KILL_LIMIT, 3);
    new.name = "Night owls".into();
    new.slots[2].lock = Lock::Closed;
    let lines = facts::change_lines(Some(&old), &new);
    assert!(
        lines.contains(&"The game is now called Night owls.".to_owned()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Settings: mode pvp, kill-limit 3.".to_owned()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"Plane 2's slot is closed: the AI flies it.".to_owned()),
        "{lines:?}"
    );
    // Nothing changed, nothing said.
    assert!(facts::change_lines(Some(&old), &old.clone()).is_empty());
    // The first state (joining) says only that the player joined.
    let first = facts::change_lines(None, &new);
    assert_eq!(first, ["Joined Night owls."]);
}

#[test]
fn the_head_line_is_drawn_and_the_screen_keeps_it_with_the_panels_up() {
    let mut screen = screen_of(&sample_pvp(1), true);
    let mut pixels = test_kit::blank();
    screen.draw(&mut Canvas(&mut pixels));
    click(&mut screen, button_centre(1));
    screen.draw(&mut Canvas(&mut pixels));
    // The lobby behind the panel takes the wheel and keys only after it
    // closes.
    screen.wheel(1);
    assert!(screen.settings_open().is_some());
}
