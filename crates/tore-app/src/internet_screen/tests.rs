//! The screen's rules, with no window and the synthetic kit: focus order,
//! which button is default, the list (order, filters, paging, selection), the
//! players and the mission line, joining through a master, New, Options, the
//! settings it saves and the one-time notice. The master is a real one on
//! 127.0.0.1 or a scripted one; **no test contacts the built-in master** (the
//! screen's master is set before its first turn).
use super::preview::sample_entries;
use super::*;
use crate::net::browse::testing::LoopbackMaster;
use crate::widgets::test_kit;

/// A master nothing listens on: a literal address, so no name is looked up.
const NOBODY: &str = "127.0.0.1:9";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tore-internet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn screen() -> InternetScreen {
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), None);
    s.settings.master = Some(NOBODY.into());
    s
}

/// A screen kept in `dir`, on a master nothing listens on.
fn kept_in(dir: &std::path::Path) -> InternetScreen {
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.to_path_buf()));
    s.settings.master = Some(NOBODY.into());
    s
}

/// A screen with a callsign and the seven sample games, none selected.
fn with_games() -> InternetScreen {
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.set_listed(sample_entries());
    s
}

fn lines(s: &InternetScreen) -> String {
    s.message_lines().join("\n")
}

fn names(s: &InternetScreen) -> Vec<String> {
    s.listed_names()
}

fn values(telemetry: bool) -> options::Values {
    options::Values {
        port: 26_900,
        password: String::new(),
        name: None,
        master: Some(NOBODY.into()),
        port_forward: true,
        may_host: true,
        telemetry,
    }
}

// ---- the Multi menu ----

#[test]
fn the_multi_menu_opens_this_screen_from_its_second_row() {
    use crate::menu::{Action, State};
    let mut menu = State::new(Vec::new());
    menu.open = Some(2);
    menu.pointer(Some((150.0, 90.0)));
    menu.down();
    assert_eq!(menu.up(), Action::Internet);
    assert_eq!(menu.open, None);
}

// ---- focus and the default button ----

#[test]
fn tab_walks_the_controls_in_order_and_wraps() {
    let mut s = screen();
    // A first visit starts in the callsign.
    assert_eq!(s.focus.current(), Some(Id::Callsign));
    let mut seen = Vec::new();
    for _ in 0..10 {
        s.key("Tab", false);
        seen.push(s.focus.current().unwrap());
    }
    assert_eq!(
        seen,
        [
            Id::Full,
            Id::Other,
            Id::Games,
            Id::Messages,
            Id::New,
            Id::Join,
            Id::Refresh,
            Id::Options,
            Id::Cancel,
            Id::Callsign
        ]
    );
    s.key("Tab", true);
    assert_eq!(s.focus.current(), Some(Id::Cancel));
}

#[test]
fn new_is_the_default_until_a_game_is_selected() {
    let mut s = with_games();
    assert_eq!(s.default_button(), "New");
    s.refresh_buttons();
    assert!(s.new.is_default() && !s.join.is_default());
    s.select_game(0);
    assert_eq!(s.default_button(), "Join");
    s.refresh_buttons();
    assert!(s.join.is_default() && !s.new.is_default());
}

// ---- the list ----

#[test]
fn the_list_hides_full_games_and_other_versions_and_puts_unjoinable_ones_last() {
    let mut s = with_games();
    // Joinable games first in the master's order, then the closing one.
    assert_eq!(
        names(&s),
        [
            "Iceman's lobby",
            "Friday night",
            "Goose and co",
            "Mav's quickie",
            "Going away"
        ]
    );
    assert_eq!(s.games.pages(), 2);
    assert_eq!(s.games.page_counter(), "1  of  2");
    // Show full games: Top Gun joins the unjoinable ones, after the rest.
    s.full.set_checked(true);
    s.rebuild_games();
    assert_eq!(
        names(&s)[4..],
        ["Top Gun test".to_owned(), "Going away".to_owned()]
    );
    // Show other versions: the old build comes in, dimmed, with its version.
    s.other.set_checked(true);
    s.rebuild_games();
    assert_eq!(names(&s).len(), 7);
    let old = s
        .games
        .rows()
        .iter()
        .find(|row| row.cells.get(1) == Some(&Cell::Text("Old build".into())))
        .unwrap();
    assert!(old.dim);
    assert_eq!(old.cells[3], Cell::Text("v0.1.2".into()));
    // A game's row: lock, name, players over capacity, its phase, relay mark.
    let friday = &s.games.rows()[1];
    assert_eq!(
        friday.cells,
        [
            Cell::Icon(Icon::Lock),
            Cell::Text("Friday night".into()),
            Cell::Text("2/8".into()),
            Cell::Text("Flying".into()),
            Cell::Text("R".into())
        ]
    );
    assert!(!friday.dim);
    assert_eq!(s.games.rows()[0].cells[4], Cell::Empty);
}

#[test]
fn the_selection_stays_on_its_game_when_the_list_is_refreshed() {
    let mut s = with_games();
    s.select_game(1);
    assert_eq!(s.selected_entry().unwrap().name, "Friday night");
    // A new list with another game first: the selection follows the game.
    let mut games = sample_entries();
    games.rotate_left(1);
    s.set_listed(games);
    assert_eq!(s.selected_entry().unwrap().name, "Friday night");
    // Gone from the list: nothing is selected, and the players clear.
    let games: Vec<_> = sample_entries()
        .into_iter()
        .filter(|(entry, _)| entry.name != "Friday night")
        .collect();
    s.set_listed(games);
    assert!(s.selected_entry().is_none());
    assert!(s.players.is_empty());
}

#[test]
fn the_selected_games_players_show_with_the_crown_and_its_mission_under_the_list() {
    let mut s = with_games();
    s.select_game(0);
    let rows = s.players.rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].cells[0], Cell::Icon(Icon::Crown));
    assert_eq!(rows[0].cells[1], Cell::Text("Iceman".into()));
    assert_eq!(rows[1].cells[0], Cell::Empty);
    assert!(
        s.selection_line()
            .unwrap()
            .starts_with("Mission: KOLA, clear")
    );
    // A game that says more players than it names: "and 2 more".
    let mut games = sample_entries();
    games[0].1.players = 5;
    s.set_listed(games);
    let rows = s.players.rows();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3].cells[0], Cell::Text("and 2 more".into()));
    // The relay mark has its words under the list.
    s.select_game(1);
    assert!(
        s.selection_line()
            .unwrap()
            .ends_with("(may need the relay)")
    );
    // Details not yet in: only the relay's words.
    s.details.clear();
    s.rebuild_players();
    assert_eq!(
        s.selection_line().unwrap(),
        "This game's router may need the relay."
    );
    // Another version has its own line.
    s.other.set_checked(true);
    s.rebuild_games();
    let at = names(&s).iter().position(|n| n == "Old build").unwrap();
    s.select_game(at);
    assert!(
        s.selection_line()
            .unwrap()
            .contains("runs another version (0.1.2) and cannot be joined")
    );
}

// ---- joining ----

#[test]
fn join_asks_for_a_callsign_and_a_game_first() {
    let mut s = screen();
    s.set_listed(sample_entries());
    s.select_game(0);
    s.focus.set(Id::Games);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(
        lines(&s).contains("Type your callsign first."),
        "{}",
        lines(&s)
    );
    assert_eq!(s.focus.current(), Some(Id::Callsign));
    s.callsign.set_text(" Viper");
    s.focus.set(Id::Games);
    s.key("Enter", false);
    assert!(
        lines(&s).contains("cannot start or end with a space"),
        "{}",
        lines(&s)
    );
    // With a callsign but no game, Join says so.
    let mut s = screen();
    s.callsign.set_text("Viper");
    s.focus.set(Id::Join);
    s.key("Enter", false);
    assert!(lines(&s).contains("Pick a game in the list first."));
}

#[test]
fn a_game_that_cannot_be_joined_is_refused_in_words_before_the_master_is_asked() {
    let mut s = with_games();
    s.full.set_checked(true);
    s.other.set_checked(true);
    s.rebuild_games();
    for (name, wanted) in [
        (
            "Old build",
            "runs another version (0.1.2); it cannot be joined.",
        ),
        ("Top Gun test", "is full."),
        ("Going away", "is closing; try again soon."),
    ] {
        let at = names(&s).iter().position(|n| n == name).unwrap();
        s.select_game(at);
        s.focus.set(Id::Games);
        assert_eq!(s.key("Enter", false), Outcome::None);
        assert!(lines(&s).contains(wanted), "{name}: {}", lines(&s));
        assert!(!s.joining());
    }
}

#[test]
fn join_with_no_browse_says_the_master_cannot_be_reached() {
    let mut s = with_games();
    s.select_game(0);
    s.focus.set(Id::Games);
    // The screen has had no turn, so no browse is running.
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(lines(&s).contains("cannot be reached"), "{}", lines(&s));
}

/// Turns the screen on virtual time, `seconds` of it in steps of a second and
/// a tenth, with a real moment between steps for the threads.
fn turn_virtual(s: &mut InternetScreen, from: Duration, seconds: u64) -> (Duration, Outcome) {
    let mut now = from;
    let mut outcome = Outcome::None;
    for _ in 0..seconds {
        now += Duration::from_millis(1_100);
        let one = s.update_at(now, false);
        if one != Outcome::None {
            outcome = one;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    (now, outcome)
}

/// A screen browsing a master nothing answers (a literal address, so no
/// name is looked up), its browse started and its master known.
fn on_dead_master() -> (InternetScreen, Duration) {
    let mut s = with_games();
    let mut now = Duration::ZERO;
    while s.browse.as_ref().is_none_or(|b| b.masters().is_empty()) {
        s.update_at(now, false);
        now += Duration::from_millis(10);
        std::thread::sleep(Duration::from_millis(5));
    }
    // The browse's first answer would replace the sample games; there is none.
    s.set_listed(sample_entries());
    (s, now)
}

#[test]
fn join_goes_through_the_masters_introduction_to_a_race_of_the_hosts_addresses() {
    let mut master = LoopbackMaster::start();
    let (_id, host) = master.list("Friday night", 1, 8, false);
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), None);
    s.settings.master = Some(master.text());
    s.callsign.set_text("Maverick");
    s.password = "secret".into();
    assert!(master.pump(|| {
        s.update(false);
        !names(&s).is_empty()
    }));
    s.select_game(0);
    s.focus.set(Id::Games);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(s.joining());
    assert!(
        lines(&s).contains("Asking the Internet Lobby to introduce you to 'Friday night'..."),
        "{}",
        lines(&s)
    );
    // While it waits the buttons are off and only Cancel works.
    s.refresh_buttons();
    assert!(!s.join.hit((170, 430)) && s.cancel.hit((520, 430)));
    let mut joined = Outcome::None;
    master.pump(|| {
        joined = s.update(false);
        joined != Outcome::None
    });
    let Outcome::Join(request) = joined else {
        panic!("a join: {}", lines(&s));
    };
    // The race is the host's address as the master saw it (here: itself).
    let host_at = host.local_addr().unwrap();
    assert_eq!(request.join.address, host_at);
    assert_eq!(request.race.targets[0].address, host_at);
    assert_ne!(request.race.introduction, 0);
    assert_eq!(request.join.label, "Friday night");
    assert_eq!(request.join.callsign, "Maverick");
    assert_eq!(request.join.password, "secret");
    assert_eq!(request.master, master.text());
    // No folder to keep an id in (a test), so no report.
    assert_eq!(request.install_id, None);
    assert!(
        lines(&s).contains("Trying 1 address for 'Friday night'..."),
        "{}",
        lines(&s)
    );
    // The socket the introduction ran on goes to the session, once.
    assert!(s.take_through().is_some());
    assert!(s.take_through().is_none());
    // The join takes over from the browse.
    assert!(!s.browsing() && !s.joining());
}

#[test]
fn a_game_the_master_no_longer_lists_is_a_line_in_messages_and_the_screen_goes_on() {
    let mut master = LoopbackMaster::start();
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), None);
    s.settings.master = Some(master.text());
    s.callsign.set_text("Maverick");
    // The first (empty) list has arrived; then the screen is shown a game the
    // master does not know.
    assert!(master.pump(|| {
        s.update(false);
        lines(&s).contains("No games are listed")
    }));
    s.set_listed(sample_entries());
    s.select_game(1);
    s.focus.set(Id::Games);
    s.key("Enter", false);
    assert!(s.joining());
    assert!(master.pump(|| {
        s.update(false);
        !s.joining()
    }));
    assert!(
        lines(&s).ends_with("That game is no longer listed."),
        "{}",
        lines(&s)
    );
    assert_eq!(s.default_button(), "Join");
    assert!(s.browsing());
}

#[test]
fn a_master_that_never_introduces_is_given_up_on_and_the_screen_goes_on() {
    let (mut s, now) = on_dead_master();
    s.select_game(0);
    s.focus.set(Id::Games);
    s.key("Enter", false);
    assert!(s.joining());
    let (_, outcome) = turn_virtual(&mut s, now, 12);
    assert_eq!(outcome, Outcome::None);
    assert!(!s.joining());
    assert!(
        lines(&s).contains("did not introduce you to that game"),
        "{}",
        lines(&s)
    );
    assert_eq!(s.default_button(), "Join");
}

#[test]
fn cancel_stops_an_introduction_and_the_next_escape_leaves() {
    let (mut s, _) = on_dead_master();
    s.select_game(0);
    s.focus.set(Id::Games);
    s.key("Enter", false);
    assert!(s.joining());
    assert_eq!(s.key("Escape", false), Outcome::None);
    assert!(!s.joining());
    assert!(lines(&s).ends_with("Cancelled."));
    assert_eq!(s.key("Escape", false), Outcome::Close);
}

#[test]
fn escape_leaves_a_session_before_it_leaves_the_screen() {
    let mut s = with_games();
    s.update(false);
    assert!(s.browsing());
    s.update(true);
    assert!(s.in_session());
    assert_eq!(s.key("Escape", false), Outcome::EndSession);
    // The browse stopped for the session and the list is empty.
    assert!(!s.browsing());
    assert!(s.games.is_empty());
    // When the session ends the browse starts again.
    s.update(false);
    assert!(s.browsing());
}

// ---- New ----

#[test]
fn new_lists_a_game_on_the_master_after_checking_the_callsign() {
    let mut s = screen();
    s.focus.set(Id::New);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(lines(&s).contains("Type your callsign first."));
    s.callsign.set_text("Maverick");
    s.password = "pw".into();
    let Outcome::Host(request) = s.key("Enter", false) else {
        panic!("a host: {}", lines(&s));
    };
    assert_eq!(request.callsign, "Maverick");
    assert_eq!(request.name, "Maverick's game");
    assert_eq!(request.port, 26_900);
    assert_eq!(request.password.as_deref(), Some("pw"));
    assert_eq!(request.listing.master, NOBODY);
    assert!(request.listing.listed);
    // A game name from Options is used.
    s.settings.game_name = Some("Friday night".into());
    let Outcome::Host(request) = s.key("Enter", false) else {
        panic!("a host");
    };
    assert_eq!(request.name, "Friday night");
}

#[test]
fn the_built_in_master_is_used_only_when_no_other_is_chosen() {
    // Never turned: the screen would browse the built-in master.
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), None);
    assert_eq!(s.master_text(), tore_net::master::DEFAULT_MASTER);
    s.settings.master = Some("master.example.org:26911".into());
    assert_eq!(s.master_text(), "master.example.org:26911");
}

// ---- statistics: the id and the notice ----

#[test]
fn the_notice_is_said_the_first_time_and_never_again() {
    let dir = scratch("notice");
    let first = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.clone()));
    assert_eq!(first.message_lines(), [telemetry::NOTICE]);
    assert!(Remembered::load(&dir).telemetry_notice);
    let second = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.clone()));
    assert!(second.message_lines().is_empty());
    // With statistics already off there is nothing to tell.
    let off = scratch("notice-off");
    let kept = Remembered {
        telemetry: false,
        ..Remembered::default()
    };
    kept.save(&off).unwrap();
    let third = InternetScreen::new(Arc::new(test_kit::kit()), Some(off.clone()));
    assert!(third.message_lines().is_empty());
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(off);
}

#[test]
fn a_game_listed_from_here_reports_under_an_install_id_only_while_statistics_are_on() {
    let dir = scratch("id");
    let mut s = kept_in(&dir);
    s.callsign.set_text("Maverick");
    let Outcome::Host(request) = s.new_game() else {
        panic!("a host");
    };
    let id = request.listing.install_id.expect("an id");
    assert!(dir.join(telemetry::INSTALL_ID_FILE).exists());
    // The same id the next time.
    let Outcome::Host(request) = s.new_game() else {
        panic!("a host");
    };
    assert_eq!(request.listing.install_id, Some(id));
    // Off in Options: no id, and the file is gone.
    s.options_answer(Answer::Ok(values(false)));
    assert!(!s.settings.telemetry);
    assert!(!dir.join(telemetry::INSTALL_ID_FILE).exists());
    let Outcome::Host(request) = s.new_game() else {
        panic!("a host");
    };
    assert_eq!(request.listing.install_id, None);
    assert!(!Remembered::load(&dir).telemetry);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn turning_statistics_off_and_on_again_draws_a_new_id() {
    let dir = scratch("again");
    let mut s = kept_in(&dir);
    let first = s.install_id().expect("an id");
    s.options_answer(Answer::Ok(values(false)));
    assert_eq!(s.install_id(), None);
    s.options_answer(Answer::Ok(values(true)));
    let second = s.install_id().expect("an id");
    assert_ne!(first, second);
    let _ = std::fs::remove_dir_all(dir);
}

// ---- Options ----

fn type_into(s: &mut InternetScreen, text: &str) {
    assert!(s.typing(), "a field has the keyboard");
    s.text_input(text);
}

fn clear_field(s: &mut InternetScreen) {
    for _ in 0..130 {
        s.key("Backspace", false);
        s.key("Delete", false);
    }
}

#[test]
fn options_keeps_the_masters_address_and_refuses_bad_values() {
    let dir = scratch("options");
    let mut s = kept_in(&dir);
    s.open_options();
    assert!(s.options_open() && s.typing());
    // Port: nothing is not a port, and the panel stays.
    clear_field(&mut s);
    s.key("Enter", false);
    assert!(s.options_open());
    type_into(&mut s, "27000");
    // The master: a bad address is refused, a good one is kept.
    for _ in 0..3 {
        s.key("Tab", false);
    }
    clear_field(&mut s);
    type_into(&mut s, "two words");
    s.key("Enter", false);
    assert!(
        s.options_open(),
        "a master that is not an address is refused"
    );
    clear_field(&mut s);
    type_into(&mut s, "master.example.org:26911");
    s.key("Enter", false);
    assert!(!s.options_open());
    assert_eq!(s.settings.port, 27_000);
    assert_eq!(
        s.settings.master.as_deref(),
        Some("master.example.org:26911")
    );
    // Kept in the file.
    let back = Remembered::load(&dir);
    assert_eq!(back.port, 27_000);
    assert_eq!(back.master.as_deref(), Some("master.example.org:26911"));
    assert!(back.port_forward && back.telemetry);
    // Esc closes the panel and keeps nothing.
    s.open_options();
    clear_field(&mut s);
    type_into(&mut s, "9");
    assert_eq!(s.key("Escape", false), Outcome::None);
    assert_eq!(s.settings.port, 27_000);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_empty_master_is_the_built_in_one_and_a_switch_turns_by_key() {
    let mut s = screen();
    s.open_options();
    // Three Tabs reach the master; clear it. One more reaches "Forward the
    // game port", Space turns it off; one more reaches "Let my game take
    // over hosting", Space turns it off; two more reach OK.
    for _ in 0..3 {
        s.key("Tab", false);
    }
    clear_field(&mut s);
    s.key("Tab", false);
    s.key("Space", false);
    s.key("Tab", false);
    s.key("Space", false);
    for _ in 0..2 {
        s.key("Tab", false);
    }
    s.key("Enter", false);
    assert!(!s.options_open());
    assert_eq!(s.settings.master, None);
    assert!(!s.settings.port_forward);
    assert!(!s.settings.may_host);
    assert!(s.settings.telemetry);
}

#[test]
fn another_master_restarts_the_browse() {
    let mut s = screen();
    s.update(false);
    assert!(s.browsing());
    s.options_answer(Answer::Ok(options::Values {
        master: Some("127.0.0.1:10".into()),
        ..values(true)
    }));
    s.update(false);
    assert_eq!(s.browsed, "127.0.0.1:10");
}

// ---- the check boxes and keys ----

#[test]
fn the_check_boxes_filter_the_list_and_are_remembered() {
    let dir = scratch("checks");
    let mut s = kept_in(&dir);
    s.set_listed(sample_entries());
    assert_eq!(names(&s).len(), 5);
    s.focus.set(Id::Full);
    s.key("Space", false);
    s.focus.set(Id::Other);
    s.key("Space", false);
    assert_eq!(names(&s).len(), 7);
    let kept = Remembered::load(&dir);
    assert!(kept.show_full && kept.show_other);
    let again = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.clone()));
    assert!(again.full.checked() && again.other.checked());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn f5_and_refresh_ask_for_the_list_again() {
    let mut s = screen();
    s.update(false);
    assert!(s.browsing());
    s.key("F5", false);
    assert!(lines(&s).ends_with("Asking the Internet Lobby for the list..."));
    s.focus.set(Id::Refresh);
    s.key("Enter", false);
    assert_eq!(
        lines(&s)
            .matches("Asking the Internet Lobby for the list...")
            .count(),
        2
    );
}

#[test]
fn the_mouse_picks_games_and_presses_buttons() {
    let mut s = with_games();
    // The first row of the Games list.
    s.moved(Some((100.0, 194.0)));
    s.button(true);
    s.button(false);
    assert_eq!(s.selected_entry().unwrap().name, "Iceman's lobby");
    assert_eq!(s.default_button(), "Join");
    // Options by the mouse.
    s.moved(Some((400.0, 430.0)));
    s.button(true);
    s.button(false);
    assert!(s.options_open());
    s.key("Escape", false);
    assert!(!s.options_open());
    // New by the mouse (the callsign is set).
    s.moved(Some((50.0, 430.0)));
    s.button(true);
    assert!(matches!(s.button(false), Outcome::Host(_)));
    // The wheel moves the selection.
    s.moved(Some((100.0, 194.0)));
    s.wheel(-1);
    assert_eq!(s.selected_entry().unwrap().name, "Friday night");
}

// ---- the browse, on a real master ----

#[test]
fn the_screen_lists_the_games_a_real_master_lists_with_their_players() {
    let mut master = LoopbackMaster::start();
    let (_a, _host_a) = master.list("Friday night", 3, 8, false);
    let (_b, _host_b) = master.list("Packed", 8, 8, true);
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), None);
    s.settings.master = Some(master.text());
    s.callsign.set_text("Maverick");
    assert!(master.pump(|| {
        s.update(false);
        !names(&s).is_empty()
    }));
    // The full game is not asked for until Show full games is on.
    assert_eq!(names(&s), ["Friday night"]);
    assert!(
        master.pump(|| {
            s.update(false);
            lines(&s).contains("1 game is listed on the Internet Lobby.")
        }),
        "{}",
        lines(&s)
    );
    s.focus.set(Id::Full);
    s.key("Space", false);
    assert!(master.pump(|| {
        s.update(false);
        names(&s).len() == 2
    }));
    // Select a game: its details come from the master.
    s.select_game(0);
    assert!(master.pump(|| {
        s.update(false);
        !s.players.is_empty()
    }));
    assert_eq!(s.players.rows()[0].cells[0], Cell::Icon(Icon::Crown));
    assert!(
        s.selection_line()
            .unwrap()
            .starts_with("Mission: Friday night")
    );
}

#[test]
fn a_master_that_does_not_answer_is_said_once_and_direct_connection_is_named() {
    let mut s = screen();
    // Virtual time on the browse's clock: a step is a second and a tenth.
    let mut now = Duration::ZERO;
    for _ in 0..14 {
        s.update_at(now, false);
        std::thread::sleep(Duration::from_millis(20));
        now += Duration::from_millis(1_100);
    }
    let said = lines(&s);
    assert_eq!(said.matches("does not answer").count(), 1, "{said}");
    assert!(said.contains("Direct Connection still works."));
}

#[test]
fn drawing_every_state_does_not_panic_and_options_cover_the_screen() {
    let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
    let mut s = with_games();
    s.select_game(1);
    s.draw(&mut Canvas(&mut pixels));
    let plain = pixels.clone();
    s.open_options();
    s.draw(&mut Canvas(&mut pixels));
    assert_ne!(plain, pixels);
    s.joining = Some(Joining {
        name: "x".into(),
        asked: Duration::ZERO,
        through: None,
        token: None,
    });
    s.panel = None;
    s.refresh_buttons();
    s.draw(&mut Canvas(&mut pixels));
}
