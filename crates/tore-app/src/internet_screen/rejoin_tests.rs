//! The Rejoin mark and token of the Internet Lobby (stage K, slice K7b): a
//! game whose session the player holds a rejoin token for says "Rejoin" once
//! the master has said which session it is, and the join carries the token.
//! "Let my game take over hosting" is an Options switch that is kept. No
//! window; the synthetic kit; a master on 127.0.0.1 and no other.
use super::preview::sample_entries;
use super::*;
use crate::net::browse::testing::LoopbackMaster;
use crate::net::rejoin_store::Store;
use crate::widgets::test_kit;
use tore_net::Token;
use tore_session::client::rejoin::{KeptToken, unix_now};

/// A master nothing listens on: a literal address, so no name is looked up.
const NOBODY: &str = "127.0.0.1:9";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tore-internet-k7b-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn kept(session: u64, callsign: &str) -> KeptToken {
    KeptToken {
        token: Token(0xbeef_0000 + u128::from(session)),
        session_id: session,
        callsign: callsign.to_owned(),
        life_seconds: 86_400,
        last_seen_unix: unix_now(),
    }
}

/// A screen kept in `dir` that holds a token for session 2 ("Friday night" in
/// the sample list, whose summary names session 2).
fn with_token(dir: &std::path::Path) -> InternetScreen {
    let mut store = Store::default();
    store.keep(kept(2, "Viper"), "");
    store.save(dir).unwrap();
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.to_path_buf()));
    s.settings.master = Some(NOBODY.into());
    s.callsign.set_text("Maverick");
    s
}

fn state_of(s: &InternetScreen, name: &str) -> String {
    let row = s
        .games
        .rows()
        .iter()
        .find(|row| row.cells.contains(&Cell::Text(name.into())))
        .unwrap_or_else(|| panic!("{name} is listed"));
    match &row.cells[3] {
        Cell::Text(text) => text.clone(),
        other => panic!("a state: {other:?}"),
    }
}

#[test]
fn a_listed_game_whose_session_the_player_holds_a_token_for_says_rejoin() {
    let dir = scratch("mark");
    let mut s = with_token(&dir);
    s.set_listed(sample_entries());
    assert_eq!(state_of(&s, "Friday night"), "Rejoin");
    assert_eq!(state_of(&s, "Iceman's lobby"), "Lobby");
    s.select_game(
        s.games
            .rows()
            .iter()
            .position(|row| row.cells.contains(&Cell::Text("Friday night".into())))
            .unwrap(),
    );
    let line = s.selection_line().expect("a line");
    assert!(line.starts_with("You were in this game as Viper"), "{line}");
    // Before the master has said which session it is, nothing is marked.
    s.details.clear();
    s.rebuild_games();
    assert_eq!(state_of(&s, "Friday night"), "Flying");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_join_carries_the_token_kept_for_the_games_session() {
    let dir = scratch("join");
    let mut master = LoopbackMaster::start();
    let mut store = Store::default();
    store.keep(kept(2, "Viper"), "");
    store.save(&dir).unwrap();
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.clone()));
    s.settings.master = Some(master.text());
    s.callsign.set_text("Maverick");
    assert!(master.pump(|| {
        s.update(false);
        lines(&s).contains("No games are listed")
    }));
    s.set_listed(sample_entries());
    let at = s
        .games
        .rows()
        .iter()
        .position(|row| row.cells.contains(&Cell::Text("Friday night".into())))
        .unwrap();
    s.select_game(at);
    s.focus.set(Id::Games);
    s.key("Enter", false);
    assert_eq!(
        s.joining.as_ref().map(|j| j.token),
        Some(Some(Token(0xbeef_0002))),
        "{}",
        lines(&s)
    );
    // Another game, another session: no token.
    s.joining = None;
    let at = s
        .games
        .rows()
        .iter()
        .position(|row| row.cells.contains(&Cell::Text("Iceman's lobby".into())))
        .unwrap();
    s.select_game(at);
    s.key("Enter", false);
    assert_eq!(s.joining.as_ref().map(|j| j.token), Some(None));
    let _ = std::fs::remove_dir_all(dir);
}

fn lines(s: &InternetScreen) -> String {
    s.message_lines().join("\n")
}

#[test]
fn let_my_game_take_over_hosting_is_on_turns_by_key_and_mouse_and_is_kept() {
    let dir = scratch("switch");
    let mut s = InternetScreen::new(Arc::new(test_kit::kit()), Some(dir.clone()));
    s.settings.master = Some(NOBODY.into());
    assert!(s.settings.may_host, "on by default");
    s.open_options();
    // Port, Password, Name, Master, Forward, then the switch.
    for _ in 0..5 {
        s.key("Tab", false);
    }
    s.key("Space", false);
    // Statistics, then OK.
    s.key("Tab", false);
    s.key("Tab", false);
    s.key("Enter", false);
    assert!(!s.options_open());
    assert!(!s.settings.may_host);
    assert!(s.settings.port_forward && s.settings.telemetry);
    assert!(
        std::fs::read_to_string(Remembered::path(&dir))
            .unwrap()
            .contains("may-host no")
    );
    // The panel shows the saved choice; a click turns it on again.
    s.open_options();
    s.moved(Some((126.0, 287.0)));
    s.button(true);
    s.button(false);
    s.key("Tab", false);
    s.key("Tab", false);
    s.key("Enter", false);
    assert!(!s.options_open());
    assert!(s.settings.may_host);
    assert!(Remembered::load(&dir).may_host);
    let _ = std::fs::remove_dir_all(dir);
}
