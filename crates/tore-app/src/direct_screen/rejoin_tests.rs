//! The Rejoin mark and token of Direct Connection (stage K, slice K7b): a
//! game the player holds a rejoin token for says "Rejoin" in the list and
//! joins with the token, a game found by address is rejoined by the address
//! the token was kept for, and "Let my game take over hosting" is an Options
//! switch that is kept. No window; the synthetic kit.
use super::preview::sample_games;
use super::*;
use crate::net::rejoin_store::Store;
use crate::widgets::test_kit;
use tore_net::Token;
use tore_session::client::rejoin::{KeptToken, unix_now};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tore-direct-k7b-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn kept(session: u64, callsign: &str) -> KeptToken {
    KeptToken {
        token: Token(0xfeed_0000 + u128::from(session)),
        session_id: session,
        callsign: callsign.to_owned(),
        life_seconds: 86_400,
        last_seen_unix: unix_now(),
    }
}

/// A screen kept in `dir` with the sample games found, a callsign typed and
/// tokens for sessions 3 (Goose and co) and 4 (Old build, another version).
fn with_tokens(dir: &std::path::Path) -> DirectScreen {
    let mut store = Store::default();
    store.keep(kept(3, "Viper"), "192.168.1.23:26900");
    store.keep(kept(4, "Viper"), "192.168.1.24:26900");
    store.save(dir).unwrap();
    let mut s = DirectScreen::new(
        Arc::new(test_kit::kit()),
        Vec::new(),
        Some(dir.to_path_buf()),
    );
    s.callsign.set_text("Maverick");
    s.set_found(sample_games());
    s
}

fn state_of(s: &DirectScreen, name: &str) -> String {
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
fn a_game_the_player_holds_a_token_for_says_rejoin() {
    let dir = scratch("mark");
    let s = with_tokens(&dir);
    assert_eq!(state_of(&s, "Goose and co"), "Rejoin");
    assert_eq!(state_of(&s, "Iceman's lobby"), "Lobby", "no token, no mark");
    assert_eq!(state_of(&s, "Friday night"), "Flying");
    // A game of another version cannot be rejoined: it shows its version.
    assert_eq!(state_of(&s, "Old build"), "v0.1.2");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_selection_line_and_the_join_carry_the_token() {
    let dir = scratch("join");
    let mut s = with_tokens(&dir);
    let index = s
        .games
        .rows()
        .iter()
        .position(|row| row.cells.contains(&Cell::Text("Goose and co".into())))
        .unwrap();
    s.select_game(index);
    let line = s.selection_line().expect("a line");
    assert!(line.starts_with("You were in this game as Viper"), "{line}");
    s.focus.set(Id::Games);
    let Outcome::Join(request) = s.key("Enter", false) else {
        panic!("a join: {}", s.message_lines().join("\n"));
    };
    assert_eq!(request.token, Some(Token(0xfeed_0003)));
    assert_eq!(
        request.address,
        SocketAddr::from(([192, 168, 1, 23], 26_900))
    );
    assert!(
        s.message_lines()
            .iter()
            .any(|l| l.starts_with("Rejoining 'Goose and co' at 192.168.1.23:26900")),
        "{:?}",
        s.message_lines()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_game_with_no_token_joins_without_one() {
    let dir = scratch("none");
    let mut s = with_tokens(&dir);
    let index = s
        .games
        .rows()
        .iter()
        .position(|row| row.cells.contains(&Cell::Text("Iceman's lobby".into())))
        .unwrap();
    s.select_game(index);
    s.focus.set(Id::Games);
    let Outcome::Join(request) = s.key("Enter", false) else {
        panic!("a join");
    };
    assert_eq!(request.token, None);
    assert!(s.selection_line().unwrap().starts_with("Mission:"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_expired_token_marks_nothing() {
    let dir = scratch("expired");
    let mut store = Store::default();
    let mut old = kept(3, "Viper");
    old.last_seen_unix = unix_now() - 2 * 86_400;
    store.keep(old, "192.168.1.23:26900");
    // The file keeps what it was given; the screen reads it as the game does.
    store.save(&dir).unwrap();
    let mut s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    s.set_found(sample_games());
    assert_eq!(state_of(&s, "Goose and co"), "Lobby");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_game_reached_by_a_typed_address_is_joined_with_the_token_kept_for_it() {
    let (live, stop, thread) = super::tests::host_for_tests();
    let dir = scratch("typed");
    let mut store = Store::default();
    store.keep(kept(9, "Viper"), &live.to_string());
    store.save(&dir).unwrap();
    let mut s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    s.callsign.set_text("Maverick");
    s.focus.set(Id::Address);
    s.text_input(&live.to_string());
    assert_eq!(s.key("Enter", false), Outcome::None);
    let until = Instant::now() + Duration::from_secs(10);
    let request = loop {
        match s.update(false) {
            Outcome::Join(request) => break request,
            _ if Instant::now() > until => panic!("no join: {}", s.message_lines().join("\n")),
            _ => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    assert_eq!(request.address, live);
    assert_eq!(request.token, Some(Token(0xfeed_0009)));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    thread.join().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn let_my_game_take_over_hosting_is_on_turns_by_key_and_mouse_and_is_kept() {
    let dir = scratch("switch");
    let mut s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    assert!(s.settings.may_host, "on by default");
    s.open_options();
    // Port, Password, Name, Forward, then the switch: Space turns it off;
    // OK is two Tabs on (Quick messages, then OK).
    for _ in 0..4 {
        s.key("Tab", false);
    }
    s.key("Space", false);
    s.key("Tab", false);
    s.key("Tab", false);
    s.key("Enter", false);
    assert!(!s.options_open());
    assert!(!s.settings.may_host);
    assert!(s.settings.port_forward, "the other switch is not touched");
    assert!(
        std::fs::read_to_string(Remembered::path(&dir))
            .unwrap()
            .contains("may-host no")
    );
    // The panel shows the saved choice; a click turns it on again.
    s.open_options();
    s.moved(Some((126.0, 281.0)));
    s.button(true);
    s.button(false);
    s.key("Tab", false);
    s.key("Tab", false);
    s.key("Enter", false);
    assert!(!s.options_open());
    assert!(s.settings.may_host);
    assert!(Remembered::load(&dir).may_host);
    // Cancel keeps nothing.
    s.open_options();
    for _ in 0..4 {
        s.key("Tab", false);
    }
    s.key("Space", false);
    s.key("Escape", false);
    assert!(s.settings.may_host);
    let _ = std::fs::remove_dir_all(dir);
}
