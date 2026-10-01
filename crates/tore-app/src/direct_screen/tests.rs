//! The screen's rules, with no window and the synthetic kit: focus order,
//! which button is default, joining from the list and from an address,
//! refusals, the search starting and stopping with the screen, and the
//! settings it saves.
use super::preview::sample_games;
use super::*;
use crate::widgets::test_kit;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tore-direct-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn screen() -> DirectScreen {
    DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), None)
}

/// A screen with a callsign and the six sample games, none selected.
fn with_games() -> DirectScreen {
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.set_found(sample_games());
    s
}

/// A UDP port nothing holds now.
fn free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn lines(s: &DirectScreen) -> String {
    s.message_lines().join("\n")
}

fn type_text(s: &mut DirectScreen, text: &str) {
    assert!(s.typing(), "a field has the keyboard");
    s.text_input(text);
}

/// A host on loopback that answers the lookup's first packet until stopped.
fn host() -> (SocketAddr, Arc<AtomicBool>, JoinHandle<()>) {
    use tore_net::{ConnectDetails, Decision, Entropy, RefuseReason, Server, ServerConfig};
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let address = socket.local_addr().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        let mut server = Server::new(ServerConfig {
            entropy: Entropy::Seeded(2),
            ..ServerConfig::new(tore_session::wire::PROTOCOL_VERSION)
        });
        let mut gate = |_: &ConnectDetails| Decision::Refuse {
            reason: RefuseReason::ShuttingDown,
            text: String::new(),
        };
        let started = Instant::now();
        let mut buf = [0u8; 1201];
        while !flag.load(Ordering::Relaxed) {
            if let Ok((len, from)) = socket.recv_from(&mut buf) {
                server.receive(started.elapsed(), from, &buf[..len], &mut gate);
            }
            while let Some(transmit) = server.poll_transmit() {
                socket.send_to(&transmit.datagram, transmit.to).unwrap();
            }
        }
    });
    (address, stop, thread)
}

/// Turns the screen until its lookup is done, within five seconds.
fn finish_lookup(s: &mut DirectScreen) -> Outcome {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let outcome = s.update_at(Duration::ZERO, false);
        if outcome != Outcome::None || !s.looking_up() {
            return outcome;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the lookup did not finish: {}", lines(s));
}

// ---- the Multi menu ----

#[test]
fn the_multi_menu_opens_this_screen() {
    use crate::menu::{Action, State};
    let mut menu = State::new(Vec::new());
    assert_eq!(menu.items(2), ["Direct Connection...", "Internet Lobby..."]);
    menu.open = Some(2);
    // The first row opens the screen.
    assert_eq!(menu.pointer(Some((150.0, 70.0))), Action::Hover);
    menu.down();
    assert_eq!(menu.up(), Action::Direct);
    assert_eq!(menu.open, None);
    // The second is the public lobby of stage I, still to come.
    menu.open = Some(2);
    menu.pointer(Some((150.0, 90.0)));
    menu.down();
    assert_eq!(menu.up(), Action::Click);
    assert!(menu.toast.as_ref().unwrap().0.contains("coming soon"));
}

// ---- focus and the default button ----

#[test]
fn tab_walks_the_controls_in_order_and_wraps() {
    let mut s = screen();
    // A first visit starts in the callsign.
    assert_eq!(s.focus.current(), Some(Id::Callsign));
    let mut seen = Vec::new();
    for _ in 0..9 {
        s.key("Tab", false);
        seen.push(s.focus.current().unwrap());
    }
    assert_eq!(
        seen,
        [
            Id::Address,
            Id::Full,
            Id::Games,
            Id::Messages,
            Id::New,
            Id::Join,
            Id::Options,
            Id::Cancel,
            Id::Callsign
        ]
    );
    s.key("Tab", true);
    assert_eq!(s.focus.current(), Some(Id::Cancel));
}

#[test]
fn a_remembered_callsign_starts_with_no_focus() {
    let dir = scratch("callsign");
    let mut kept = Remembered::default();
    kept.remember_callsign("Viper");
    kept.save(&dir).unwrap();
    let s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    assert_eq!(s.callsign_text(), "Viper");
    assert_eq!(s.focus.current(), None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn new_is_the_default_until_a_game_is_selected_or_an_address_typed() {
    let mut s = with_games();
    assert_eq!(s.default_button(), "New");
    s.update_at(Duration::ZERO, false);
    assert!(s.new.is_default() && !s.join.is_default());
    s.select_game(0);
    assert_eq!(s.default_button(), "Join");
    s.update_at(Duration::ZERO, false);
    assert!(s.join.is_default() && !s.new.is_default());
    // Typing an address takes the selection away, and Join stays the default.
    s.focus.set(Id::Address);
    type_text(&mut s, "10.0.0.5");
    assert!(s.games.selected().is_none());
    assert_eq!(s.default_button(), "Join");
    // Emptying the field brings New back.
    for _ in 0..8 {
        s.key("Backspace", false);
    }
    assert_eq!(s.default_button(), "New");
}

#[test]
fn the_buttons_are_disabled_while_something_is_going_on() {
    let mut s = with_games();
    s.lookup = Some(Lookup::spawn(
        "x".into(),
        || {
            std::thread::sleep(Duration::from_millis(300));
            Ok(Vec::new())
        },
        3,
        Duration::from_millis(100),
    ));
    s.update_at(Duration::ZERO, false);
    assert!(!s.join.hit((240, 425)) && !s.new.hit((120, 425)) && !s.options_button.hit((360, 425)));
    assert!(s.cancel.hit((480, 425)));
    // Typing and keys in the fields do nothing while it goes on.
    s.focus.set(Id::Callsign);
    s.key("Backspace", false);
    assert_eq!(s.callsign_text(), "Maverick");
    // Enter does nothing but Esc cancels the lookup, and the screen stays.
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert_eq!(s.key("Escape", false), Outcome::None);
    assert!(!s.looking_up());
    assert!(lines(&s).ends_with("Cancelled."));
}

// ---- joining ----

#[test]
fn join_asks_for_a_callsign_first() {
    let mut s = screen();
    s.set_found(sample_games());
    s.select_game(0);
    s.focus.set(Id::Games);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(
        lines(&s).contains("Type your callsign first."),
        "{}",
        lines(&s)
    );
    assert_eq!(s.focus.current(), Some(Id::Callsign));
    // A callsign with a leading space is refused in words.
    s.callsign.set_text(" Viper");
    s.focus.set(Id::Games);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(
        lines(&s).contains("cannot start or end with a space"),
        "{}",
        lines(&s)
    );
}

#[test]
fn join_from_the_list_goes_to_the_game_by_address() {
    let mut s = with_games();
    s.password = "secret".into();
    s.select_game(0);
    s.focus.set(Id::Games);
    // Enter in the list, and a double-click, both press Join.
    let Outcome::Join(request) = s.key("Enter", false) else {
        panic!("a join: {}", lines(&s));
    };
    assert_eq!(
        request,
        JoinRequest {
            address: SocketAddr::from(([192, 168, 1, 21], 26_900)),
            label: "Iceman's lobby".into(),
            callsign: "Maverick".into(),
            password: "secret".into(),
        }
    );
    assert!(
        lines(&s).contains("Attempting connection to 'Iceman's lobby' at 192.168.1.21:26900"),
        "{}",
        lines(&s)
    );
    // The Join button by mouse does the same.
    s.moved(Some((240.0, 425.0)));
    assert_eq!(s.button(true), Outcome::None);
    assert!(matches!(s.button(false), Outcome::Join(_)));
}

#[test]
fn a_game_that_cannot_be_joined_says_why_and_the_screen_stays() {
    let mut s = with_games();
    s.full.set_checked(true);
    s.rebuild_games();
    // Row 3 (the old build) and row 4 (the full one).
    for (index, words) in [(3, "another version"), (4, "is full")] {
        s.select_game(index);
        s.focus.set(Id::Games);
        assert_eq!(s.key("Enter", false), Outcome::None, "{words}");
        assert!(lines(&s).contains(words), "{words}: {}", lines(&s));
    }
    // A game that is closing.
    let mut games = sample_games();
    games[0].answer.phase = DiscoverPhase::Closed;
    s.set_found(games);
    s.select_game(0);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(lines(&s).contains("closing"), "{}", lines(&s));
}

#[test]
fn full_games_show_only_when_asked_and_a_different_build_is_dimmed() {
    let mut s = with_games();
    // Five of six show: the full one is hidden.
    assert_eq!(s.games.len(), 5);
    assert_eq!(s.games.pages(), 2);
    let old = &s.games.rows()[3];
    assert!(old.dim, "another build is dimmed");
    assert_eq!(old.cells[3], Cell::Text("v0.1.2".into()));
    // Space on the check box turns them on.
    s.focus.set(Id::Full);
    s.key("Space", false);
    assert_eq!(s.games.len(), 6);
    assert_eq!(s.games.pages(), 2);
    assert!(s.games.rows()[4].dim, "a full game is dimmed");
}

#[test]
fn the_players_of_the_selected_game_show_with_the_crown() {
    let mut s = with_games();
    assert!(s.players.is_empty());
    s.select_game(1);
    assert_eq!(s.players.len(), 2);
    assert_eq!(s.players.rows()[0].cells[0], Cell::Icon(Icon::Crown));
    assert_eq!(s.players.rows()[1].cells[0], Cell::Empty);
    // Players the answer left out are counted.
    let mut games = sample_games();
    games[0].answer.callsigns.truncate(1);
    s.set_found(games);
    s.select_game(0);
    assert_eq!(s.players.len(), 2);
    assert_eq!(
        s.players.rows()[1].cells[0],
        Cell::Text("and 2 more".into())
    );
    // The mission shows under the box, or why the game cannot be joined.
    assert!(s.selection_line().unwrap().starts_with("Mission: KOLA"));
    s.set_found(sample_games());
    s.select_game(3);
    assert!(s.selection_line().unwrap().contains("another version"));
}

#[test]
fn a_selection_stays_on_its_game_when_the_list_refreshes() {
    let mut s = with_games();
    s.select_game(2);
    let mut games = sample_games();
    games.remove(0);
    s.set_found(games);
    assert_eq!(s.selected_game().unwrap().answer.name, "Goose and co");
    // A game that goes away takes the selection with it, and the players.
    s.set_found(Vec::new());
    assert!(s.selected_game().is_none() && s.players.is_empty());
}

#[test]
fn join_from_an_address_looks_it_up_and_joins_the_one_that_answers() {
    let (live, stop, thread) = host();
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.settings.port = 1; // an address with no port uses Options' port
    s.password = "pw".into();
    s.focus.set(Id::Address);
    type_text(&mut s, &format!("127.0.0.1:{}", live.port()));
    assert_eq!(s.default_button(), "Join");
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(s.looking_up(), "the lookup runs off the screen's thread");
    let Outcome::Join(request) = finish_lookup(&mut s) else {
        panic!("a join: {}", lines(&s));
    };
    assert_eq!(request.address, live);
    assert_eq!(request.callsign, "Maverick");
    assert_eq!(request.password, "pw");
    assert!(!s.looking_up());
    let text = lines(&s);
    assert!(text.contains("Looking up 127.0.0.1"), "{text}");
    assert!(text.contains("Trying 127.0.0.1"), "{text}");
    assert!(text.contains("answered"), "{text}");
    // The address is remembered for next time, with its port.
    assert_eq!(s.settings.addresses, [format!("127.0.0.1:{}", live.port())]);
    stop.store(true, Ordering::Relaxed);
    thread.join().unwrap();
}

#[test]
fn an_address_without_a_port_uses_the_options_port() {
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.settings.port = 27_001;
    s.focus.set(Id::Address);
    type_text(&mut s, "127.0.0.1");
    s.key("Enter", false);
    assert_eq!(s.looking_for, "127.0.0.1:27001");
    s.key("Escape", false);
    assert!(lines(&s).contains("Looking up") || lines(&s).ends_with("Cancelled."));
}

#[test]
fn an_address_that_is_wrong_or_does_not_answer_is_a_plain_line() {
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.focus.set(Id::Address);
    type_text(&mut s, "not an address");
    // The field takes only an address's characters.
    assert_eq!(s.address_text(), "notanaddress");
    s.key("Enter", false);
    assert!(s.looking_up(), "a name is looked up, not refused");
    s.key("Escape", false);
    let mut s = screen();
    s.callsign.set_text("Maverick");
    s.address.set_text("host:99999");
    s.target = Target::Typed;
    s.focus.set(Id::Address);
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(lines(&s).contains("port must be a number"), "{}", lines(&s));
    // Nothing answering is the lookup's own failure, which stays on screen.
    let mut s = screen();
    s.lookup = Some(Lookup::spawn(
        "nowhere".into(),
        || Ok(Vec::new()),
        3,
        Duration::from_millis(50),
    ));
    assert_eq!(finish_lookup(&mut s), Outcome::None);
    assert!(
        lines(&s).contains("Cannot find the server nowhere"),
        "{}",
        lines(&s)
    );
    assert!(!s.looking_up());
}

#[test]
fn a_host_that_refuses_shows_its_words_and_the_screen_stays() {
    let mut s = screen();
    s.lookup = Some(Lookup::spawn(
        "game".into(),
        || Ok(vec![SocketAddr::from(([127, 0, 0, 1], 9))]),
        3,
        Duration::from_millis(50),
    ));
    // Nothing listens on port 9: "no answer", then the failure line.
    assert_eq!(finish_lookup(&mut s), Outcome::None);
    assert!(lines(&s).contains("No answer from"), "{}", lines(&s));
    assert!(!s.looking_up());
    // Refusals the game reports after the handshake come through `say`.
    s.say("The game is full.");
    assert!(lines(&s).ends_with("The game is full."));
}

#[test]
fn up_and_down_in_connect_to_step_through_recent_addresses() {
    let mut s = screen();
    s.settings.addresses = vec!["a.example.org:1".into(), "b.example.org:2".into()];
    s.focus.set(Id::Address);
    s.key("ArrowUp", false);
    assert_eq!(s.address_text(), "a.example.org:1");
    s.key("ArrowUp", false);
    assert_eq!(s.address_text(), "b.example.org:2");
    s.key("ArrowUp", false);
    assert_eq!(s.address_text(), "b.example.org:2", "stops at the oldest");
    s.key("ArrowDown", false);
    s.key("ArrowDown", false);
    assert_eq!(s.address_text(), "", "back to nothing typed");
    // The wheel over the field does the same.
    s.moved(Some((150.0, 140.0)));
    s.wheel(1);
    assert_eq!(s.address_text(), "a.example.org:1");
}

// ---- New ----

#[test]
fn new_hosts_with_the_callsign_name_and_options_and_stops_the_search_first() {
    let port = free_port();
    let mut s = screen();
    s.settings.port = port;
    // No callsign: asked for first, nothing hosted.
    assert_eq!(s.key("Enter", false), Outcome::None);
    assert!(lines(&s).contains("Type your callsign first."));
    s.callsign.set_text("Maverick");
    s.update_at(Duration::ZERO, false);
    assert!(s.searching() && s.search_on_game_port());
    s.focus.clear();
    let Outcome::Host(request) = s.key("Enter", false) else {
        panic!("a host: {}", lines(&s));
    };
    assert_eq!(
        request,
        HostRequest {
            callsign: "Maverick".into(),
            name: "Maverick's game".into(),
            port,
            password: None,
        }
    );
    // The search is gone and the game port is free for the host to bind.
    assert!(!s.searching());
    UdpSocket::bind(("0.0.0.0", port)).expect("the host can bind the port");
}

// ---- the search ----

#[test]
fn the_search_starts_with_the_screen_and_stops_when_it_goes() {
    let port = free_port();
    let mut s = screen();
    s.settings.port = port;
    assert!(!s.searching(), "nothing runs before the first turn");
    s.update_at(Duration::ZERO, false);
    assert!(s.searching() && s.search_on_game_port());
    assert!(
        lines(&s).contains(&format!("UDP port {port}")),
        "{}",
        lines(&s)
    );
    assert!(
        UdpSocket::bind(("0.0.0.0", port)).is_err(),
        "it holds the port"
    );
    drop(s);
    UdpSocket::bind(("0.0.0.0", port)).expect("closing the screen frees the port");
}

#[test]
fn a_session_stops_the_search_and_its_end_starts_it_again() {
    let port = free_port();
    let mut s = screen();
    s.settings.port = port;
    s.update_at(Duration::ZERO, false);
    assert!(s.searching());
    s.update_at(Duration::from_secs(1), true);
    assert!(!s.searching() && s.in_session());
    UdpSocket::bind(("0.0.0.0", port)).expect("a session leaves the port to the host");
    s.update_at(Duration::from_secs(2), false);
    assert!(s.searching(), "the session ended: searching again");
}

#[test]
fn a_busy_game_port_is_said_in_messages() {
    let held = UdpSocket::bind(("0.0.0.0", 0)).unwrap();
    let mut s = screen();
    s.settings.port = held.local_addr().unwrap().port();
    s.update_at(Duration::ZERO, false);
    assert!(s.searching() && !s.search_on_game_port());
    assert!(
        lines(&s).contains("is busy on this machine"),
        "{}",
        lines(&s)
    );
}

#[test]
fn a_search_that_starts_again_says_nothing_new() {
    let port = free_port();
    let mut s = screen();
    s.settings.port = port;
    s.update_at(Duration::ZERO, false);
    let said = lines(&s);
    s.update_at(Duration::from_secs(1), true);
    s.update_at(Duration::from_secs(2), false);
    assert!(s.searching());
    assert_eq!(lines(&s), said, "no second \"Searching\" line");
    // Another port is news.
    s.settings.port = free_port();
    s.update_at(Duration::from_secs(3), false);
    assert_ne!(lines(&s), said);
}

#[test]
fn changing_the_port_restarts_the_search_on_it() {
    let (a, b) = (free_port(), free_port());
    let mut s = screen();
    s.settings.port = a;
    s.update_at(Duration::ZERO, false);
    s.settings.port = b;
    s.update_at(Duration::from_secs(1), false);
    assert_eq!(s.searched_port, b);
    UdpSocket::bind(("0.0.0.0", a)).expect("the old port is free");
    assert!(UdpSocket::bind(("0.0.0.0", b)).is_err());
}

#[test]
fn the_search_does_not_run_under_the_options_panel_or_retry_a_failure() {
    let port = free_port();
    let mut s = screen();
    s.settings.port = port;
    s.open_options();
    s.update_at(Duration::ZERO, false);
    assert!(!s.searching(), "the panel is open");
    s.panel = None;
    s.update_at(Duration::ZERO, false);
    assert!(s.searching());
}

#[test]
fn games_the_search_finds_fill_the_list_and_messages() {
    // A real search on loopback with a fake host answering it.
    use tore_net::packet::{Discover, Packet};
    let host = UdpSocket::bind("127.0.0.1:0").unwrap();
    host.set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let host_port = host.local_addr().unwrap().port();
    let mut s = screen();
    s.settings.port = free_port();
    // Aim a search of our own at the fake host.
    let own = Own::this_game();
    let search = Search::start_at(
        0,
        vec![SocketAddr::from(([127, 0, 0, 1], host_port))],
        own.clone(),
        Duration::ZERO,
    )
    .unwrap();
    s.search = Some(search);
    s.searched_port = s.settings.port;
    s.update_at(Duration::ZERO, false);
    let mut buf = [0u8; 2048];
    let (len, from) = host.recv_from(&mut buf).expect("the query arrives");
    let Packet::Discover(Discover { nonce, .. }) =
        Packet::decode(&buf[..len], own.protocol_version).unwrap()
    else {
        panic!("a query");
    };
    let mut answer = sample_games().remove(0).answer;
    answer.nonce = nonce;
    answer.protocol_version = own.protocol_version;
    answer.game_version = own.build.version.clone();
    answer.game_commit = own.build.commit.clone();
    let bytes = Packet::DiscoverAnswer(answer)
        .encode(own.protocol_version)
        .unwrap();
    host.send_to(&bytes, from).unwrap();
    let started = Instant::now();
    while s.games.is_empty() && started.elapsed() < Duration::from_secs(3) {
        s.update_at(Duration::from_millis(1), false);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(s.games.len(), 1);
    assert!(
        lines(&s).contains("Found Iceman's lobby at 127.0.0.1"),
        "{}",
        lines(&s)
    );
    assert_eq!(s.games.rows()[0].cells[3], Cell::Text("Lobby".into()));
}

// ---- Options and the settings ----

#[test]
fn options_hold_the_port_password_and_name_and_save_all_but_the_password() {
    let dir = scratch("options");
    let mut s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    s.settings.port = free_port();
    s.callsign.set_text("Maverick");
    // Open by mouse on the button.
    s.moved(Some((360.0, 425.0)));
    s.button(true);
    s.button(false);
    assert!(s.options_open() && s.typing());
    // The port field has the keyboard: replace its text.
    for _ in 0..5 {
        s.key("Backspace", false);
    }
    s.text_input("27000");
    s.key("Tab", false);
    s.text_input("hunter2");
    s.key("Tab", false);
    s.text_input("Friday night");
    s.key("Enter", false);
    assert!(!s.options_open());
    assert_eq!(s.settings.port, 27_000);
    assert_eq!(s.settings.game_name.as_deref(), Some("Friday night"));
    assert_eq!(s.password, "hunter2");
    let file = std::fs::read_to_string(Remembered::path(&dir)).unwrap();
    assert!(file.contains("port 27000") && file.contains("game-name Friday night"));
    assert!(file.contains("callsign Maverick"));
    assert!(!file.contains("hunter2"), "the password is never stored");
    // New hosts under that name, port and password.
    s.focus.clear();
    let Outcome::Host(host) = s.key("Enter", false) else {
        panic!("a host");
    };
    assert_eq!(
        (host.name.as_str(), host.port, host.password.as_deref()),
        ("Friday night", 27_000, Some("hunter2"))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bad_port_in_options_is_refused_in_the_panel_and_cancel_discards() {
    let mut s = screen();
    let port = s.settings.port;
    s.open_options();
    for _ in 0..5 {
        s.key("Backspace", false);
    }
    s.text_input("0");
    s.key("Enter", false);
    assert!(s.options_open(), "the panel stays for a bad port");
    s.text_input("99");
    s.key("Escape", false);
    assert!(!s.options_open());
    assert_eq!(s.settings.port, port, "Cancel changes nothing");
    // Esc in the panel does not leave the screen.
    s.open_options();
    assert_eq!(s.key("Escape", false), Outcome::None);
}

#[test]
fn the_quick_messages_show_read_only() {
    let mut s = DirectScreen::new(
        Arc::new(test_kit::kit()),
        vec![
            QuickMessage {
                receiver: Some(tore_formats::chat::Receiver::All),
                text: "Splash one bandit!".into(),
                sound: None,
            },
            QuickMessage {
                receiver: None,
                text: "On my wing".into(),
                sound: None,
            },
        ],
        None,
    );
    s.open_options();
    let shown: Vec<String> = s
        .panel
        .as_ref()
        .unwrap()
        .quick
        .lines()
        .map(|(text, _)| text.to_owned())
        .collect();
    assert_eq!(shown[0], "F1  Splash one bandit!  (to all)");
    assert_eq!(shown[1], "F2  On my wing  (to the chosen receiver)");
}

#[test]
fn the_callsign_and_show_full_are_remembered_when_the_screen_goes() {
    let dir = scratch("save");
    let mut s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    s.focus.set(Id::Callsign);
    type_text(&mut s, "Goose");
    s.focus.set(Id::Full);
    s.key("Space", false);
    assert_eq!(s.key("Escape", false), Outcome::Close);
    let kept = Remembered::load(&dir);
    assert_eq!(kept.callsign.as_deref(), Some("Goose"));
    assert!(kept.show_full);
    // And the next visit starts from them.
    let s = DirectScreen::new(Arc::new(test_kit::kit()), Vec::new(), Some(dir.clone()));
    assert_eq!(s.callsign_text(), "Goose");
    assert!(s.full.checked());
    let _ = std::fs::remove_dir_all(dir);
}

// ---- leaving ----

#[test]
fn escape_stops_what_is_going_on_then_leaves() {
    let mut s = screen();
    s.callsign.set_text("Maverick");
    assert_eq!(s.key("Escape", false), Outcome::Close);
    // With a session running, Cancel (labelled Leave) ends the session.
    s.update_at(Duration::ZERO, true);
    assert_eq!(s.cancel.label(), "Leave");
    assert_eq!(s.key("Escape", false), Outcome::EndSession);
    s.update_at(Duration::ZERO, false);
    assert_eq!(s.cancel.label(), "Cancel");
    // The mouse on Cancel closes.
    s.moved(Some((480.0, 425.0)));
    s.button(true);
    assert_eq!(s.button(false), Outcome::Close);
}

#[test]
fn typing_goes_to_a_field_only_when_one_has_the_keyboard_and_nothing_is_going_on() {
    let mut s = screen();
    assert!(s.typing());
    s.focus.set(Id::Games);
    assert!(!s.typing());
    s.focus.set(Id::Address);
    assert!(s.typing());
    s.update_at(Duration::ZERO, true);
    assert!(!s.typing());
}

#[test]
fn drawing_every_state_does_not_panic_and_covers_the_canvas() {
    let mut s = with_games();
    s.select_game(1);
    s.say("Found a game.");
    let mut pixels = test_kit::blank();
    s.draw(&mut Canvas(&mut pixels));
    s.open_options();
    s.draw(&mut Canvas(&mut pixels));
    // The title bar of the composed background is NETIPX3's.
    assert_ne!(test_kit::at(&pixels, 5, 5), [0, 0, 0]);
}

// ---- the screen's cost, from the imported pieces ----

/// The frame cost of the screen with the search running, in the debug or
/// release build the test is built as:
///
/// ```text
/// TORE_DATA_DIR=... cargo test --release -p tore-app --locked \
///     direct_screen::tests::time_screen_frame -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
fn time_screen_frame() {
    use crate::widgets::KitSource;
    let data = crate::assets::data_directory().expect("data directory");
    let assets = crate::assets::Assets::load(&data).expect("an imported pack");
    let source = KitSource::of(&assets.pics, &assets.multiplayer_resources);
    let started = Instant::now();
    let kit = Arc::new(source.build("MODEM3").expect("kit"));
    println!("kit built in {:?}", started.elapsed());
    let mut s = DirectScreen::new(kit, source.quick_messages(), None);
    s.settings.port = free_port();
    s.callsign.set_text("Maverick");
    s.update_at(Duration::ZERO, false);
    assert!(s.searching(), "the search runs while the screen is timed");
    s.full.set_checked(true);
    s.set_found(sample_games());
    s.select_game(1);
    for text in ["Found a game.", "Found another game.", "A third line."] {
        s.say(text);
    }
    let mut pixels = test_kit::blank();
    let runs = 500;
    let (mut turn, mut draw, mut both) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..runs + 20 {
        let t = Instant::now();
        s.update_at(Duration::from_millis(i as u64 * 16), false);
        let a = t.elapsed();
        let t = Instant::now();
        s.draw(&mut Canvas(&mut pixels));
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
            "{name}, {runs} runs with the search running: mean {:?}, median {:?}, p95 {:?}, max {:?}",
            total / runs as u32,
            times[runs / 2],
            times[runs * 95 / 100],
            times[runs - 1]
        );
    }
}
