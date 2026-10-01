//! Headless pictures of the Direct Connection screen for `--snapshot-state
//! direct...` and the review sheets: synthetic games and messages on the
//! screen built from the imported pieces, no window and no network.
use super::*;
use crate::AppResult;
use crate::widgets::KitSource;
use tore_net::packet::DiscoverAnswer;

/// The states a snapshot can show.
pub const STATES: [&str; 5] = [
    "direct",
    "direct-games",
    "direct-trying",
    "direct-refused",
    "direct-options",
];

fn game(
    n: u8,
    name: &str,
    summary: &str,
    players: &[&str],
    capacity: u8,
    flags: (bool, bool, DiscoverPhase, Compat),
) -> Game {
    let (password, full, phase, compat) = flags;
    Game {
        address: SocketAddr::from(([192, 168, 1, 20 + n], 26_900)),
        answer: DiscoverAnswer {
            nonce: 1,
            protocol_version: 3,
            game_version: if compat == Compat::Same {
                "0.1.3".into()
            } else {
                "0.1.2".into()
            },
            game_commit: "1a2b3c4d5e6f".into(),
            session_id: u64::from(n),
            name: name.into(),
            summary: summary.into(),
            players: players.len() as u8,
            capacity,
            password,
            full,
            phase,
            king: players.first().map(|p| (*p).to_owned()).unwrap_or_default(),
            callsigns: players.iter().map(|p| (*p).to_owned()).collect(),
            truncated: false,
        },
        compat,
    }
}

/// Six games for the list's two pages (the full one shows with "Show full games").
pub(super) fn sample_games() -> Vec<Game> {
    use DiscoverPhase::{Flying, Lobby};
    let kola = "KOLA, clear, airborne at 20000 ft: F-14A Tomcat x4 against MiG-29 Fulcrum-C x4";
    vec![
        game(
            1,
            "Iceman's lobby",
            kola,
            &["Iceman", "Goose", "Slider"],
            8,
            (false, false, Lobby, Compat::Same),
        ),
        game(
            2,
            "Friday night",
            "UKR, overcast, ground start: F/A-18D Hornet x2 against Su-27 Flanker-B x2",
            &["Maverick", "Viper"],
            8,
            (true, false, Flying, Compat::Same),
        ),
        game(
            3,
            "Goose and co",
            kola,
            &["Goose"],
            4,
            (false, false, Lobby, Compat::Same),
        ),
        game(
            4,
            "Old build",
            kola,
            &["Hollywood"],
            8,
            (false, false, Lobby, Compat::OtherBuild),
        ),
        game(
            5,
            "Top Gun test",
            kola,
            &["Jester", "Wolfman"],
            2,
            (true, true, Lobby, Compat::Same),
        ),
        game(
            6,
            "Mav's quickie",
            "BAL, clear, airborne at 20000 ft: F-16C Fighting Falcon x2 against MiG-21 Fishbed x2",
            &["Mav", "Rooster", "Hangman"],
            6,
            (false, false, Lobby, Compat::Same),
        ),
    ]
}

impl DirectScreen {
    /// Replaces the games found (a preview or a test stands in for the
    /// search).
    pub(crate) fn set_found(&mut self, games: Vec<Game>) {
        self.found = games;
        self.rebuild_games();
    }
    /// Selects the row `index` of the list, as a click does.
    pub(crate) fn select_game(&mut self, index: usize) {
        self.games.select(index);
        self.game_selected();
    }
}

/// Draws `state` into `pixels` (640 by 480 RGBA).
pub fn render(source: &KitSource, state: &str, pixels: &mut [u8]) -> AppResult<()> {
    if !STATES.contains(&state) {
        return Err(format!(
            "snapshot state must be one of {} for the Direct Connection screen",
            STATES.join(", ")
        )
        .into());
    }
    let kit = Arc::new(source.build("MODEM3")?);
    let mut screen = DirectScreen::new(kit, source.quick_messages(), None);
    screen.callsign.set_text("Maverick");
    screen.focus.set(Id::Callsign);
    screen.say("Searching for games on the local network (UDP port 26900)...");
    match state {
        "direct" => {}
        "direct-games" => {
            screen.full.set_checked(true);
            screen.set_found(sample_games());
            screen.say("Found Iceman's lobby at 192.168.1.21:26900.");
            screen.say("Found Friday night at 192.168.1.22:26900.");
            screen.say("Found Goose and co at 192.168.1.23:26900.");
            // The second page holds the full game and the last one.
            screen.select_game(5);
        }
        "direct-trying" => {
            screen.address.set_text("games.example.org");
            screen.say("Looking up games.example.org:26900...");
            screen.say("Found 203.0.113.9:26900, [2001:db8::9]:26900");
            screen.say("No answer from 203.0.113.9:26900");
            screen.say("Trying [2001:db8::9]:26900...");
            screen.looking_for = "games.example.org:26900".into();
            // A lookup that is still going, for the buttons' look.
            screen.lookup = Some(Lookup::spawn(
                "preview".into(),
                || {
                    std::thread::sleep(Duration::from_secs(3));
                    Ok(Vec::new())
                },
                3,
                Duration::from_secs(3),
            ));
            screen.refresh_buttons();
        }
        "direct-refused" => {
            screen.address.set_text("192.168.1.22");
            screen.say("Looking up 192.168.1.22:26900...");
            screen.say("Found 192.168.1.22:26900");
            screen.say("Trying 192.168.1.22:26900...");
            screen.say("192.168.1.22:26900 answered");
            screen.say("Joining 192.168.1.22:26900...");
            screen.say("The game is full.");
            screen.say("Cannot join: that game runs another version.");
        }
        _ => {
            screen.set_found(sample_games());
            screen.open_options();
        }
    }
    screen.refresh_buttons();
    let mut canvas = Canvas(pixels);
    screen.draw(&mut canvas);
    Ok(())
}
