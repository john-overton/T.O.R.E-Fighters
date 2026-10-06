//! Headless pictures of the Internet Lobby screen for `--snapshot-state
//! internet...` and the review sheets: synthetic games and messages on the
//! screen built from the imported pieces, no window and no network.
use super::*;
use crate::AppResult;
use crate::widgets::KitSource;

/// The states a snapshot can show.
pub const STATES: [&str; 6] = [
    "internet",
    "internet-games",
    "internet-joining",
    "internet-options",
    "internet-unreachable",
    // Stage K (slice K7b): a game the player holds a rejoin token for.
    "internet-rejoin",
];

/// What a sample game says about itself.
struct Sample {
    name: &'static str,
    players: &'static [&'static str],
    capacity: u8,
    password: bool,
    full: bool,
    phase: DiscoverPhase,
    other_build: Option<&'static str>,
    relay: bool,
    mission: &'static str,
}

const KOLA: &str = "KOLA, clear, airborne at 20000 ft: F-14A Tomcat x4 against MiG-29 Fulcrum-C x4";

fn samples() -> [Sample; 7] {
    use DiscoverPhase::{Closed, Flying, Lobby};
    let sample =
        |name, players, capacity, (password, full, phase, other_build, relay), mission| Sample {
            name,
            players,
            capacity,
            password,
            full,
            phase,
            other_build,
            relay,
            mission,
        };
    [
        sample(
            "Iceman's lobby",
            &["Iceman", "Goose", "Slider"],
            8,
            (false, false, Lobby, None, false),
            KOLA,
        ),
        sample(
            "Friday night",
            &["Maverick", "Viper"],
            8,
            (true, false, Flying, None, true),
            "UKR, overcast, ground start: F/A-18D Hornet x2 against Su-27 Flanker-B x2",
        ),
        sample(
            "Goose and co",
            &["Goose"],
            4,
            (false, false, Lobby, None, false),
            KOLA,
        ),
        sample(
            "Old build",
            &["Hollywood"],
            8,
            (false, false, Lobby, Some("0.1.2"), false),
            KOLA,
        ),
        sample(
            "Top Gun test",
            &["Jester", "Wolfman"],
            2,
            (true, true, Lobby, None, false),
            KOLA,
        ),
        sample(
            "Mav's quickie",
            &["Mav", "Rooster", "Hangman"],
            6,
            (false, false, Lobby, None, true),
            "BAL, clear, airborne at 20000 ft: F-16C Fighting Falcon x2 against MiG-21 Fishbed x2",
        ),
        sample(
            "Going away",
            &["Cougar"],
            8,
            (false, false, Closed, None, false),
            KOLA,
        ),
    ]
}

/// Seven listed games and the summaries the master gave for them: the
/// second page of the list holds the full game and the closing one.
pub(super) fn sample_entries() -> Vec<(PageEntry, ListingSummary)> {
    samples()
        .into_iter()
        .zip(1u64..)
        .map(|(sample, id)| {
            let entry = PageEntry {
                listing_id: id,
                password: sample.password,
                full: sample.full,
                dedicated: false,
                other_build: sample.other_build.map(str::to_owned),
                relay_likely: sample.relay,
                phase: sample.phase,
                players: sample.players.len() as u8,
                capacity: sample.capacity,
                platform: 3,
                name: sample.name.to_owned(),
            };
            let summary = ListingSummary {
                protocol_version: tore_session::wire::PROTOCOL_VERSION,
                password: sample.password,
                full: sample.full,
                phase: sample.phase,
                players: sample.players.len() as u8,
                capacity: sample.capacity,
                session_id: id,
                game_version: sample.other_build.unwrap_or("0.1.3").to_owned(),
                game_commit: "1a2b3c4d5e6f".into(),
                name: sample.name.to_owned(),
                mission: sample.mission.to_owned(),
                king: sample
                    .players
                    .first()
                    .map(|p| (*p).to_owned())
                    .unwrap_or_default(),
                callsigns: sample.players.iter().map(|p| (*p).to_owned()).collect(),
                ..ListingSummary::default()
            };
            (entry, summary)
        })
        .collect()
}

impl InternetScreen {
    /// Replaces the games the master listed, and what it said of each (a
    /// preview or a test stands in for the browse).
    pub(crate) fn set_listed(&mut self, games: Vec<(PageEntry, ListingSummary)>) {
        self.entries = games.iter().map(|(entry, _)| entry.clone()).collect();
        self.details = games
            .into_iter()
            .map(|(entry, summary)| (entry.listing_id, summary))
            .collect();
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
            "snapshot state must be one of {} for the Internet Lobby screen",
            STATES.join(", ")
        )
        .into());
    }
    let kit = Arc::new(source.build("MODEM3")?);
    let mut screen = InternetScreen::new(kit, None);
    screen.callsign.set_text("Maverick");
    screen.focus.set(Id::Callsign);
    screen.say("Asking the Internet Lobby at master.jroverton.com:26901 for games...");
    match state {
        "internet" => {}
        "internet-games" => {
            screen.full.set_checked(true);
            screen.set_listed(sample_entries());
            screen.say("7 games are listed on the Internet Lobby.");
            // The first game: it has the crown, three players and a mission.
            screen.select_game(0);
            screen.focus.set(Id::Games);
        }
        // Friday night is a game the player was in as Viper: it says Rejoin.
        "internet-rejoin" => {
            let mut store = crate::net::rejoin_store::Store::default();
            store.keep(
                tore_session::client::rejoin::KeptToken {
                    token: tore_net::Token(1),
                    session_id: 2,
                    callsign: "Viper".into(),
                    life_seconds: 86_400,
                    last_seen_unix: tore_session::client::rejoin::unix_now(),
                },
                "",
            );
            screen.tokens = store;
            screen.set_listed(sample_entries());
            screen.say("7 games are listed on the Internet Lobby.");
            screen.select_game(1);
            screen.focus.set(Id::Games);
        }
        "internet-joining" => {
            screen.set_listed(sample_entries());
            screen.select_game(1);
            screen.say("Asking the Internet Lobby to introduce you to 'Friday night'...");
            screen.joining = Some(Joining {
                name: "Friday night".into(),
                asked: Duration::ZERO,
                through: None,
                token: None,
            });
        }
        "internet-unreachable" => {
            screen.say(
                "Cannot find the Internet Lobby at master.jroverton.com:26901: no address. Direct Connection still works.",
            );
        }
        _ => {
            screen.set_listed(sample_entries());
            screen.open_options();
        }
    }
    screen.refresh_buttons();
    let mut canvas = Canvas(pixels);
    screen.draw(&mut canvas);
    Ok(())
}
