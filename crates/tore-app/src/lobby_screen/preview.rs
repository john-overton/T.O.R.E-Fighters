//! Headless pictures of the lobby screen for `--snapshot-state lobby...` and
//! the review sheets: a synthetic lobby on the screen built from the imported
//! pieces, no window and no network.
use super::*;
use crate::AppResult;
use crate::widgets::KitSource;
use tore_formats::aircraft::AircraftId;
use tore_session::wire::Platform;
use tore_session::wire::chat::{ChatFrom, Receiver, Standing};
use tore_session::wire::messages::{LobbyPhase, LobbyPlayer, LobbySlot, StartRule};
use tore_sim::ai::launch::{Side, WingId};

/// The states a snapshot can show.
pub const STATES: [&str; 8] = [
    "lobby-king",
    "lobby-joiner",
    "lobby-unable",
    "lobby-flying",
    "lobby-server",
    "lobby-kick",
    "lobby-leave",
    "lobby-ready",
];

/// A player of the sample lobby, on a platform picked by its id so the
/// sample shows a mix: Maverick on Linux, Goose on Windows, the third on
/// macOS.
pub(crate) fn player(id: u8, name: &str, slot: Option<u32>) -> LobbyPlayer {
    LobbyPlayer {
        id,
        callsign: name.to_owned(),
        slot,
        ready: false,
        loadout: false,
        flying: false,
        observing: false,
        away: false,
        unable: None,
        platform: match id % 3 {
            1 => Platform::Linux,
            2 => Platform::Windows,
            _ => Platform::MacOs,
        },
    }
}

/// The sample mission's slots: the player's wing of four Tomcats and a
/// wing of two Hornets.
pub(crate) fn slots(holders: &[(u32, u8)]) -> Vec<LobbySlot> {
    let mut slots = Vec::new();
    for plane in 0..6u32 {
        let (index, member, aircraft) = if plane < 4 {
            (0, plane as u8, AircraftId::F14)
        } else {
            (1, (plane - 4) as u8, AircraftId::F18)
        };
        slots.push(LobbySlot {
            plane,
            wing: WingId {
                side: Side::Friendly,
                index,
            },
            member,
            aircraft,
            holder: holders.iter().find(|(p, _)| *p == plane).map(|(_, id)| *id),
            lock: Default::default(),
        });
    }
    slots
}

pub(crate) const SUMMARY: &str = "KOLA, clear, airborne at 20000 ft: F-14D Tomcat x4 and F/A-18D Hornet x2 against MiG-29 Fulcrum-C x4";

/// The sample lobby: Maverick (King and house) and Goose, one ready.
pub(crate) fn sample(you: u8) -> LobbyState {
    let mut maverick = player(1, "Maverick", Some(0));
    maverick.loadout = true;
    let mut goose = player(2, "Goose", Some(1));
    goose.ready = true;
    LobbyState {
        name: "Maverick's game".into(),
        summary: SUMMARY.into(),
        mission: 3,
        phase: LobbyPhase::Lobby,
        start: StartRule::King,
        king: Some(1),
        host: Some(1),
        you,
        players: vec![maverick, goose],
        slots: slots(&[(0, 1), (1, 2)]),
        settings: Vec::new(),
    }
}

fn said(name: &str, standing: Standing, you: bool, text: &str) -> ChatLine {
    ChatLine {
        from: ChatFrom::Player {
            callsign: name.into(),
            standing,
            you,
        },
        receiver: Receiver::All,
        text: text.into(),
        sound: None,
    }
}

impl LobbyScreen {
    /// A screen on `state` as the player `state.you` sees it, with a few
    /// lines in Messages (a preview or a test stands in for the session).
    pub(crate) fn sample(kit: Arc<Kit>, state: LobbyState, hosting: bool) -> Self {
        let mut screen = Self::new(kit, "Maverick's game", hosting);
        screen.update(Some(&state), None);
        screen
    }
}

/// Draws `state` into `pixels` (640 by 480 RGBA).
pub fn render(source: &KitSource, state: &str, pixels: &mut [u8]) -> AppResult<()> {
    if !STATES.contains(&state) {
        return Err(format!(
            "snapshot state must be one of {} for the lobby screen",
            STATES.join(", ")
        )
        .into());
    }
    let kit = Arc::new(source.build("MODEM3")?);
    let mut lobby = sample(1);
    let mut hosting = true;
    match state {
        "lobby-joiner" => {
            lobby = sample(2);
            hosting = false;
        }
        "lobby-unable" => {
            lobby = sample(1);
            lobby.players.push(player(3, "Viper", None));
            lobby.players[2].unable =
                Some("Your game data differs from the host's in 2 file(s), such as F14.PT. Import the same version of the game.".into());
        }
        "lobby-flying" => {
            lobby = sample(2);
            hosting = false;
            lobby.phase = LobbyPhase::Flying;
            lobby.players[0].flying = true;
            lobby.players[1].ready = false;
            lobby.players.push(player(3, "Hollywood", None));
            lobby.players[1].slot = None;
            lobby.slots = slots(&[(0, 1)]);
        }
        "lobby-server" => {
            lobby = sample(2);
            hosting = false;
            lobby.name = "T.O.R.E dedicated 1".into();
            lobby.king = None;
            lobby.host = None;
            lobby.start = StartRule::FirstReady;
            lobby.players[0].callsign = "Iceman".into();
            lobby.players[0].loadout = false;
        }
        "lobby-ready" => {
            lobby.players[0].ready = true;
        }
        _ => {}
    }
    let mut screen = LobbyScreen::sample(Arc::clone(&kit), lobby.clone(), hosting);
    if lobby.players.len() > 1 && state != "lobby-server" {
        screen.say("Goose joined the game.");
    }
    screen.chat_line(&said(
        "Goose",
        Standing::Neutral,
        false,
        "Ready when you are, King.",
    ));
    screen.chat_line(&said(
        "Maverick",
        Standing::Neutral,
        true,
        "Take a slot and arm up.",
    ));
    match state {
        "lobby-kick" => {
            screen.players.select(1);
            screen.refresh();
            screen.press(Id::Kick);
        }
        "lobby-leave" => {
            screen.press(Id::Leave);
        }
        "lobby-king" => {
            screen.slots.select(2);
        }
        // The unable player selected: the hint line gives the reason.
        "lobby-unable" => {
            screen.players.select(2);
            screen.refresh();
        }
        _ => {}
    }
    let mut canvas = Canvas(pixels);
    screen.draw(&mut canvas);
    Ok(())
}
