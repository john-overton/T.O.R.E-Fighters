//! Tests of what stage K (slice K7b) put on the lobby screen: the marks for a
//! reserved slot, a player away and a game that stands by to host; the Host
//! row of the Settings panel (calculated, or a player the King pins); Release
//! in the Players panel, for an away player and for a plane kept for a player
//! who dropped; and the words that say them in Messages. Built on the
//! synthetic kit; no window, no session.
use super::facts::{self, SlotClick};
use super::players_panel::PlayersPanel;
use super::preview::{player, sample, sample_cheats, slots};
use super::settings_panel::{
    self as sp, Context, Edit, Kind, Page, SettingsPanel, page_rows, row_state, row_value,
};
use super::*;
use crate::widgets::{Cell, Icon, test_kit};
use tore_session::settings::{self, number};
use tore_session::wire::Path as Reach;
use tore_session::wire::messages::{LobbyPhase, StandbyMark};

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

fn button_centre(place: usize) -> Point {
    (SLOT_X[place] + 37, BUTTON_Y + 12)
}

fn slot_row(index: i32) -> Point {
    (100, 168 + 18 * index + 8)
}

fn centre((x, y, w, h): crate::widgets::Rect) -> Point {
    (x + w / 2, y + h / 2)
}

/// The King's lobby (Maverick, the house) and Goose, with Viper's plane kept
/// for it: plane 2's slot has no holder and is reserved for Viper.
fn kept_for_viper() -> LobbyState {
    let mut state = sample(1);
    state.slots[2].reserved = Some("Viper".into());
    state
}

fn set(lobby: &mut LobbyState, number: u8, value: u32) {
    for entry in &mut lobby.settings {
        if entry.0 == number {
            entry.1 = value;
        }
    }
}

// ---- a reserved slot ----

#[test]
fn a_plane_kept_for_a_dropped_player_reads_ai_and_its_name_with_a_lock_mark() {
    let state = kept_for_viper();
    let rows = facts::slot_rows(&state);
    assert_eq!(rows[2].cells[3], Cell::Text("AI (Viper away)".into()));
    assert_eq!(
        rows[2].cells[0],
        Cell::Icon(Icon::Lock),
        "the reserved mark"
    );
    assert!(rows[2].dim, "nobody else takes it");
    // The other planes read as before: no mark.
    assert_eq!(rows[3].cells[0], Cell::Empty);
    assert_eq!(rows[3].cells[3], Cell::Text("AI".into()));
    // Viper itself, back in the lobby, sees it open to itself.
    let mut back = sample(3);
    back.players.push(player(3, "Viper", None));
    back.slots[2].reserved = Some("Viper".into());
    let rows = facts::slot_rows(&back);
    assert!(!rows[2].dim, "the player it is kept for may take it");
}

#[test]
fn only_the_player_a_plane_is_kept_for_may_click_it() {
    let state = kept_for_viper();
    let facts = facts::Facts::of(Some(&state), None);
    assert_eq!(
        facts::slot_click(&state, &facts, 2),
        SlotClick::Refused("Plane 2 is kept for Viper, who is away.".into())
    );
    let mut viper = sample(3);
    viper.players.push(player(3, "Viper", None));
    viper.slots[2].reserved = Some("Viper".into());
    let facts = facts::Facts::of(Some(&viper), None);
    assert_eq!(facts::slot_click(&viper, &facts, 2), SlotClick::Take(2));
}

#[test]
fn an_away_player_reads_in_its_slot_and_the_players_row_has_no_state_word() {
    let mut state = sample(1);
    state.players[1].away = true;
    // The slot it holds says the AI flies it; the Players row ends at the name.
    let slot = state.players[1]
        .slot
        .expect("the sample player holds a slot");
    let rows = facts::slot_rows(&state);
    let at = state.slots.iter().position(|s| s.plane == slot).unwrap();
    assert_eq!(rows[at].cells[3], Cell::Text("AI (Goose away)".into()));
    let players = facts::player_rows(&state);
    assert_eq!(players[1].cells.len(), 6);
    assert_eq!(
        players[1].cells[5],
        Cell::Text(state.players[1].callsign.clone())
    );
}

// ---- a game that stands by ----

#[test]
fn a_game_that_stands_by_has_the_outlined_house_where_the_host_has_the_solid_one() {
    let mut state = sample(1);
    state.players.push(player(3, "Viper", None));
    state.players[1].standby = StandbyMark::First;
    state.players[2].standby = StandbyMark::Second;
    let rows = facts::player_rows(&state);
    assert_eq!(rows[0].cells[1], Cell::Icon(Icon::House), "the house");
    assert_eq!(rows[1].cells[1], Cell::Icon(Icon::Standby));
    assert_eq!(rows[2].cells[1], Cell::Icon(Icon::Standby));
    let detail = |id| facts::player_detail(&state, id).unwrap();
    assert!(
        detail(2).ends_with("Stands by to host: first."),
        "{}",
        detail(2)
    );
    assert!(
        detail(3).ends_with("Stands by to host: second."),
        "{}",
        detail(3)
    );
    assert!(!detail(1).contains("Stands by"), "the house hosts");
}

#[test]
fn a_game_is_told_when_it_begins_to_stand_by() {
    let old = sample(2);
    let mut new = old.clone();
    new.players[1].standby = StandbyMark::First;
    let lines = facts::change_lines(Some(&old), &new);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Your game stands by to host, first in line")),
        "{lines:?}"
    );
    // Another player's standing by is not said, and nothing is said again.
    let mut other = old.clone();
    other.players[0].standby = StandbyMark::First;
    assert!(facts::change_lines(Some(&old), &other).is_empty());
    assert!(facts::change_lines(Some(&new), &new).is_empty());
    new.players[1].standby = StandbyMark::Second;
    assert!(
        facts::change_lines(Some(&old), &new)
            .iter()
            .any(|l| l.contains("second in line"))
    );
}

// ---- the Host row ----

fn host_ctx(lobby: &LobbyState) -> Context {
    Context::of(lobby, Some(sample_cheats()))
}

#[test]
fn the_game_page_ends_with_the_host_row_and_it_fits_the_panel() {
    let rows = page_rows(Page::Game);
    assert_eq!(rows.last(), Some(&Kind::Host));
    assert_eq!(sp::row_label(Kind::Host), "Host");
    let place = sp::place(Page::Game, rows.len() - 1);
    // The row sits above the foot line, which has the panel's y + 300.
    assert!(place.row.1 + place.row.3 <= sp::PANEL.1 + 300, "{place:?}");
}

#[test]
fn the_host_row_says_calculated_with_the_house_or_the_player_pinned() {
    let mut state = sample(1);
    assert_eq!(
        row_value(Kind::Host, &host_ctx(&state)),
        "Calculated (Maverick hosts)"
    );
    set(&mut state, number::HOST, 3); // player id 2
    assert_eq!(row_value(Kind::Host, &host_ctx(&state)), "Goose");
    set(&mut state, number::HOST, 9); // a player who is not here
    assert_eq!(row_value(Kind::Host, &host_ctx(&state)), "Player 8");
}

#[test]
fn the_king_turns_the_host_row_through_calculated_and_each_player_who_can_host() {
    let mut state = sample(1);
    state.players.push(player(3, "Viper", None));
    state.players.push(player(4, "Hawk", None));
    state.players[3].path = Reach::Relay;
    let ctx = host_ctx(&state);
    assert_eq!(
        ctx.host_choices(),
        vec![0, 2, 3, 4],
        "a relayed player cannot host"
    );
    let turn = |ctx: &Context, forward| match sp::click(Kind::Host, ctx, forward) {
        Some(Edit::Settings(change)) => change.values,
        other => panic!("a settings edit: {other:?}"),
    };
    assert_eq!(turn(&ctx, true), vec![(number::HOST, 2)], "Maverick");
    assert_eq!(
        turn(&ctx, false),
        vec![(number::HOST, 4)],
        "back round to the last"
    );
    set(&mut state, number::HOST, 3);
    let pinned = host_ctx(&state);
    assert_eq!(turn(&pinned, true), vec![(number::HOST, 4)]);
    assert_eq!(turn(&pinned, false), vec![(number::HOST, 2)]);
    set(&mut state, number::HOST, 4);
    assert_eq!(
        turn(&host_ctx(&state), true),
        vec![(number::HOST, 0)],
        "calculated again"
    );
}

#[test]
fn only_the_king_turns_the_host_row() {
    let joiner = host_ctx(&sample(2));
    assert_eq!(
        row_state(Kind::Host, &joiner),
        Err("Only the King may change the settings.".to_owned())
    );
    assert_eq!(sp::click(Kind::Host, &joiner, true), None);
    // It turns in the lobby and while the mission flies.
    let mut flying = sample(1);
    flying.phase = LobbyPhase::Flying;
    assert_eq!(row_state(Kind::Host, &host_ctx(&flying)), Ok(()));
}

#[test]
fn a_pin_made_in_flight_says_it_applies_when_the_lobby_returns() {
    let mut state = sample(1);
    state.phase = LobbyPhase::Flying;
    let mut panel = SettingsPanel::new(host_ctx(&state));
    // Down to the Host row, which is the Game page's last.
    for _ in 1..page_rows(Page::Game).len() {
        panel.key("ArrowDown", false);
    }
    assert_eq!(panel.selected(), page_rows(Page::Game).len() - 1);
    let answer = panel.key("ArrowRight", false);
    assert!(
        matches!(answer, sp::Answer::Edit(Edit::Settings(_))),
        "{answer:?}"
    );
    assert_eq!(
        panel.notice(),
        Some("A pin made in flight applies when the lobby returns.")
    );
    // In the lobby it applies at once: no note.
    let mut panel = SettingsPanel::new(host_ctx(&sample(1)));
    for _ in 1..page_rows(Page::Game).len() {
        panel.key("ArrowDown", false);
    }
    panel.key("ArrowRight", false);
    assert_eq!(panel.notice(), None);
}

#[test]
fn the_host_row_is_sent_from_the_screen_as_a_pin() {
    let mut state = sample(1);
    state.players.push(player(3, "Viper", None));
    let mut screen = screen_of(&state, true);
    click(&mut screen, button_centre(1));
    let at = centre(sp::place(Page::Game, page_rows(Page::Game).len() - 1).value);
    match click(&mut screen, at) {
        Some(Request::Settings(change)) => {
            assert_eq!(change.values, vec![(number::HOST, 2)]);
        }
        other => panic!("a pin: {other:?}"),
    }
    // The host refused it in words: they are at the panel's foot.
    assert!(screen.refused_in_panel("Viper connects through the relay and cannot host."));
}

#[test]
fn a_pinned_host_is_said_to_the_players_who_are_not_the_king() {
    let mut old = sample(2);
    set(&mut old, number::HOST, 0);
    let mut new = old.clone();
    set(&mut new, number::HOST, 3);
    let lines = facts::change_lines(Some(&old), &new);
    assert!(
        lines.contains(&"The King pinned Goose as the host.".to_owned()),
        "{lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("Settings:")),
        "the player's id is not worded as a setting: {lines:?}"
    );
    let lines = facts::change_lines(Some(&new), &old);
    assert!(lines.contains(&"The King left the host to be calculated.".to_owned()));
    // The King reads the host's own words, not a second line.
    let mut king_old = sample(1);
    set(&mut king_old, number::HOST, 0);
    let mut king_new = king_old.clone();
    set(&mut king_new, number::HOST, 3);
    assert!(facts::change_lines(Some(&king_old), &king_new).is_empty());
    // Every other setting is worded as before.
    set(&mut king_new, number::FRIENDLY_FIRE, 0);
    assert_eq!(
        facts::change_lines(Some(&king_old), &king_new),
        vec![format!(
            "Settings: {}.",
            settings::words(&[(number::FRIENDLY_FIRE, 0)])
        )]
    );
}

// ---- Release ----

#[test]
fn release_frees_the_aircraft_kept_for_a_player_who_is_away() {
    let mut state = sample(1);
    state.players[1].away = true;
    let mut screen = screen_of(&state, true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, button_centre(2));
    let panel = screen.players_open().expect("the Players panel");
    assert_eq!(panel.release_plane(), Some(1), "Goose holds plane 1");
    assert!(
        panel
            .lines()
            .iter()
            .any(|l| l.contains("Plane 1 is kept for Goose"))
    );
    assert!(!panel.slot_only());
    // Release is the third button: Give crown, Kick..., Release, Close.
    let release = (110 + 218 + 40, 150 + 132 + 12);
    assert_eq!(click(&mut screen, release), Some(Request::Release(1)));
    assert!(screen.players_open().is_none());
}

#[test]
fn a_player_who_is_not_away_has_nothing_to_release() {
    let mut screen = screen_of(&sample(1), true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, button_centre(2));
    let panel = screen.players_open().expect("the Players panel");
    assert_eq!(panel.release_plane(), None);
    let release = (110 + 218 + 40, 150 + 132 + 12);
    assert_eq!(click(&mut screen, release), None);
    assert!(screen.players_open().is_some());
}

#[test]
fn players_works_on_a_reserved_slot_for_a_player_who_dropped() {
    let state = kept_for_viper();
    let mut screen = screen_of(&state, true);
    // Nothing selected: the button says what to select.
    assert_eq!(screen.buttons().players, facts::Show::Disabled);
    // Select the reserved slot (plane 2, third row).
    click(&mut screen, slot_row(2));
    assert_eq!(screen.selected_slot(), Some(2));
    assert_eq!(screen.buttons().players, facts::Show::Enabled);
    assert_eq!(click(&mut screen, button_centre(2)), None);
    let panel = screen
        .players_open()
        .expect("the panel for the reserved slot");
    assert!(panel.slot_only());
    assert_eq!(panel.release_plane(), Some(2));
    assert!(panel.lines()[0].contains("Plane 2 is kept for Viper"));
    // Give crown and Kick do nothing; Release frees the plane.
    assert_eq!(click(&mut screen, (110 + 18 + 50, 150 + 132 + 12)), None);
    assert!(screen.players_open().is_some() && !screen.modal_open());
    assert_eq!(
        click(&mut screen, (110 + 218 + 40, 150 + 132 + 12)),
        Some(Request::Release(2))
    );
    assert!(screen.players_open().is_none());
}

#[test]
fn the_slot_panel_closes_when_the_plane_is_released_or_taken() {
    let state = kept_for_viper();
    let mut screen = screen_of(&state, true);
    click(&mut screen, slot_row(2));
    click(&mut screen, button_centre(2));
    assert!(screen.players_open().is_some());
    // The host freed it: the reservation is gone.
    let mut freed = state.clone();
    freed.slots[2].reserved = None;
    screen.update(Some(&freed), None);
    assert!(screen.players_open().is_none());
    // A slot with a holder is not a reserved one.
    let mut screen = screen_of(&state, true);
    click(&mut screen, slot_row(2));
    click(&mut screen, button_centre(2));
    let mut taken = state.clone();
    taken.players.push(player(3, "Viper", Some(2)));
    taken.slots[2].holder = Some(3);
    screen.update(Some(&taken), None);
    assert!(screen.players_open().is_none());
}

#[test]
fn a_player_in_players_wins_over_a_slot_unless_slots_has_the_focus() {
    let mut state = kept_for_viper();
    state.slots[2].reserved = Some("Viper".into());
    let mut screen = screen_of(&state, true);
    screen.players.select(1);
    screen.refresh();
    click(&mut screen, slot_row(2));
    // The click focused Slots, with the reserved plane selected.
    click(&mut screen, button_centre(2));
    assert!(screen.players_open().unwrap().slot_only());
    screen.key("Escape", false);
    // With Players focused, Players... is for Goose.
    screen.picked = Id::Players;
    screen.refresh();
    click(&mut screen, button_centre(2));
    assert!(!screen.players_open().unwrap().slot_only());
}

#[test]
fn the_new_panels_and_rows_draw() {
    let kit = Arc::new(test_kit::kit());
    let mut pixels = test_kit::blank();
    PlayersPanel::new(2, "Goose", false)
        .with_release(Some(1))
        .draw(&mut Canvas(&mut pixels), &kit);
    PlayersPanel::for_slot(2, "Viper").draw(&mut Canvas(&mut pixels), &kit);
    let screen = screen_of(&kept_for_viper(), true);
    screen.draw(&mut Canvas(&mut pixels));
    let _ = slots(&[]);
}
