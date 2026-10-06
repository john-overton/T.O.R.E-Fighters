//! Headless pictures of the lobby screen for `--snapshot-state lobby...` and
//! the review sheets: a synthetic lobby on the screen built from the imported
//! pieces, no window and no network.
use super::*;
use crate::AppResult;
use crate::widgets::KitSource;
use tore_formats::aircraft::AircraftId;
use tore_session::wire::chat::{ChatFrom, Receiver, Standing};
use tore_session::wire::messages::{
    Build, ContentGaps, Gap, GapPlayer, ItemKind, LobbyPhase, LobbyPlayer, LobbySlot, Lock,
    StandbyMark, StartRule,
};
use tore_session::wire::{Path, Platform};
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::cheats::{Cheats, Damage};

/// The states a snapshot can show.
pub const STATES: [&str; 27] = [
    "lobby-king",
    "lobby-joiner",
    "lobby-unable",
    "lobby-flying",
    "lobby-server",
    "lobby-kick",
    "lobby-leave",
    "lobby-ready",
    // Stage F phase 2 (slice F2-L): the Settings panel's four pages as the
    // King and as a joiner, in PvP, and with the mission flying; the Players
    // panel; the King's slot locks; a joiner who watches.
    "lobby-settings",
    "lobby-settings-revival",
    "lobby-settings-scoring",
    "lobby-settings-realism",
    "lobby-settings-joiner",
    "lobby-settings-pvp",
    "lobby-settings-flying",
    "lobby-players",
    "lobby-players-house",
    "lobby-locks",
    "lobby-watch",
    "lobby-pvp",
    // Stage J's last slice (J6): a player reaching the host through the
    // relay, selected; and a player away from its aircraft.
    "lobby-relay",
    "lobby-away",
    // Stage L (slice L4, renamed from lobby-builds by L5): Messages saying
    // how a player's game differs from the host's, a player selected.
    "lobby-gaps",
    // Stage K (slice K7b): the Settings panel's Host row with a pinned host and
    // the standby marks; a game that stands by, selected; the Players panel's
    // Release for a player who is away; and for a plane kept for a player who
    // dropped, with the slot's lock mark.
    "lobby-host-row",
    "lobby-standby",
    "lobby-release",
    "lobby-reserved",
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
        // The house is on the host's own machine; the rest reached it
        // directly.
        path: if id == 1 {
            Path::LocalNetwork
        } else {
            Path::Punched
        },
        build: match id % 3 {
            1 => Build::V102F,
            2 => Build::V10,
            _ => Build::Unknown,
        },
        standby: Default::default(),
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
            reserved: None,
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
        settings: tore_session::settings::Store::defaults(tore_session::settings::Mode::Coop)
            .lobby_list(),
    }
}

/// The sample lobby in PvP: the PvP defaults (a 10 minute and 5 kill limit,
/// revival with unlimited lives, sides locked).
pub(crate) fn sample_pvp(you: u8) -> LobbyState {
    let mut lobby = sample(you);
    lobby.settings =
        tore_session::settings::Store::defaults(tore_session::settings::Mode::Pvp).lobby_list();
    lobby
}

/// The sample mission's cheats: a few on, so the Realism page shows both
/// states.
pub(crate) fn sample_cheats() -> Cheats {
    Cheats {
        unlimited_fuel: true,
        no_spins: true,
        damage: Damage::Normal,
        ..Cheats::default()
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
            // The host's words, as every player reads them (stage L).
            lobby.players[2].unable =
                Some("Viper's game has no F-14D Tomcat, which this mission flies.".into());
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
        "lobby-settings-joiner" => {
            lobby = sample(2);
            hosting = false;
        }
        "lobby-settings-pvp" | "lobby-pvp" => {
            lobby = sample_pvp(1);
        }
        "lobby-settings-flying" => {
            lobby = sample_pvp(1);
            lobby.phase = LobbyPhase::Flying;
            lobby.players[0].flying = true;
        }
        "lobby-players" | "lobby-players-house" => {
            lobby.players.push(player(3, "Hollywood", None));
        }
        "lobby-relay" => {
            lobby.players.push(player(3, "Hollywood", Some(2)));
            lobby.players[2].path = Path::Relay;
            lobby.slots = slots(&[(0, 1), (1, 2), (2, 3)]);
        }
        "lobby-away" => {
            lobby.players[1].away = true;
        }
        "lobby-gaps" => {
            lobby.players.push(player(3, "Hollywood", None));
        }
        // Goose is first in line to host and Hollywood, who reached the host
        // through the relay and cannot host, is not; the King pinned Goose.
        "lobby-host-row" | "lobby-standby" => {
            lobby.players.push(player(3, "Hollywood", None));
            lobby.players[1].standby = StandbyMark::First;
            lobby.players[2].standby = StandbyMark::Second;
            if state == "lobby-host-row" {
                lobby.players[2].path = Path::Relay;
                lobby.players[2].standby = StandbyMark::None;
                for entry in &mut lobby.settings {
                    if entry.0 == tore_session::settings::number::HOST {
                        entry.1 = 3;
                    }
                }
            }
        }
        // Goose is away: the AI flies its plane, kept for it.
        "lobby-release" => {
            lobby.players[1].away = true;
        }
        // Viper dropped from plane 2, which the AI flies and nobody else may
        // take, and Goose stands by.
        "lobby-reserved" => {
            lobby.players[1].standby = StandbyMark::First;
            lobby.slots[2].reserved = Some("Viper".into());
        }
        "lobby-locks" => {
            lobby.players.push(player(3, "Hollywood", None));
            lobby.slots[2].lock = Lock::Closed;
            lobby.slots[3].lock = Lock::Reserved("Hollywood".into());
            lobby.slots[4].lock = Lock::Reserved("Goose".into());
        }
        "lobby-watch" => {
            lobby = sample(2);
            hosting = false;
            lobby.phase = LobbyPhase::Flying;
            lobby.players[0].flying = true;
            lobby.players[1].slot = None;
            lobby.players[1].ready = false;
            lobby.players[1].observing = true;
            lobby.slots = slots(&[(0, 1)]);
        }
        _ => {}
    }
    let mut screen = LobbyScreen::sample(Arc::clone(&kit), lobby.clone(), hosting);
    screen.set_cheats(Some(sample_cheats()));
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
            screen.modal = Some(modal::Modal::kick(2, "Goose"));
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
        "lobby-settings"
        | "lobby-settings-joiner"
        | "lobby-settings-pvp"
        | "lobby-settings-flying" => {
            screen.press(Id::Settings);
        }
        "lobby-settings-revival" | "lobby-settings-scoring" | "lobby-settings-realism" => {
            screen.press(Id::Settings);
            if let Some(panel) = screen.settings_panel.as_mut() {
                panel.show_page(match state {
                    "lobby-settings-revival" => settings_panel::Page::Revival,
                    "lobby-settings-scoring" => settings_panel::Page::Scoring,
                    _ => settings_panel::Page::Realism,
                });
            }
        }
        "lobby-players" | "lobby-release" => {
            screen.players.select(1);
            screen.refresh();
            screen.press(Id::PlayersPanel);
        }
        // The Game page's last row is the Host row.
        "lobby-host-row" => {
            screen.press(Id::Settings);
        }
        // Goose selected: the hint line says it stands by to host.
        "lobby-standby" => {
            screen.players.select(1);
            screen.refresh();
        }
        // The kept plane selected in Slots, and the panel for it open.
        "lobby-reserved" => {
            screen.slots.select(2);
            screen.picked = Id::Slots;
            screen.refresh();
            screen.press(Id::PlayersPanel);
        }
        "lobby-players-house" => {
            // The house is another player than the King: Goose runs the game.
            let mut state = lobby.clone();
            state.host = Some(2);
            screen.update(Some(&state), None);
            screen.players.select(1);
            screen.refresh();
            screen.press(Id::PlayersPanel);
        }
        "lobby-locks" => {
            screen.slots.select(2);
        }
        // The relayed player selected: the hint line says how it connected.
        "lobby-relay" => {
            screen.players.select(2);
            screen.refresh();
        }
        // Goose selected: the hint gives its system; Messages says how
        // Hollywood's game differs from the host's.
        "lobby-gaps" => {
            let gaps = ContentGaps {
                host_build: Build::V102F,
                host_importer: None,
                gaps: vec![Gap {
                    kind: ItemKind::Aircraft,
                    key: "SU27.PT".into(),
                    label: "Su-27".into(),
                    host_lacks: false,
                    players: vec![GapPlayer {
                        id: 3,
                        differs: false,
                    }],
                }],
            };
            screen.set_gaps(Some(&gaps));
            screen.players.select(1);
            screen.refresh();
        }
        _ => {}
    }
    let mut canvas = Canvas(pixels);
    screen.draw(&mut canvas);
    Ok(())
}
