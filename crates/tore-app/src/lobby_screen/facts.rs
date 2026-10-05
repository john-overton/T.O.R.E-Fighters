//! What the lobby screen decides from the lobby's state: who may press which
//! button, the words that say why one cannot be pressed, the slot and player
//! rows, and the lines a change of state puts in Messages. All of it is plain
//! data in and out, so every rule is tested without a window, a kit or a
//! session.
use crate::widgets::{Cell, Icon, Row, tone};
use tore_session::wire::messages::{LobbyPhase, LobbyPlayer, LobbyState, StartRule};

/// What a button does to the player: it is not there, there but cannot be
/// pressed, or can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// Not offered to this player (the King's buttons on a joiner, or on a
    /// dedicated server's lobby, which has no King).
    Hidden,
    Disabled,
    Enabled,
}

impl Show {
    pub fn is_shown(self) -> bool {
        self != Show::Hidden
    }
    pub fn is_enabled(self) -> bool {
        self == Show::Enabled
    }
}

/// What the Fly button says and does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlyAs {
    /// Starts the mission.
    Fly,
    /// The mission is flying: the King ends it for everyone (*agent
    /// decision*: a King who never joined the flight would otherwise have no
    /// way to stop it from the lobby).
    EndMission,
}

/// The facts the screen reads off the lobby state for one player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    /// The lobby's state has arrived.
    pub connected: bool,
    /// This player is the King.
    pub king: bool,
    /// A dedicated server's lobby: no King at all.
    pub server: bool,
    pub phase: LobbyPhase,
    pub start: StartRule,
    /// The plane of the slot this player holds.
    pub holds: Option<u32>,
    pub ready: bool,
    /// This player sent a loadout of its own.
    pub armed: bool,
    /// This player flies the mission now.
    pub flying: bool,
    /// Why this player's game cannot play the mission, when it cannot.
    pub unable: Option<String>,
    /// Callsigns of the players holding a slot who are not ready.
    pub waiting: Vec<String>,
    /// How many players hold a slot.
    pub holders: usize,
}

impl Facts {
    /// The facts of `lobby` for the player it was sent to; `unable` is the
    /// client's own word on whether its game plays the mission.
    pub fn of(lobby: Option<&LobbyState>, unable: Option<&str>) -> Self {
        let Some(lobby) = lobby else {
            return Self {
                connected: false,
                king: false,
                server: false,
                phase: LobbyPhase::Lobby,
                start: StartRule::King,
                holds: None,
                ready: false,
                armed: false,
                flying: false,
                unable: unable.map(str::to_owned),
                waiting: Vec::new(),
                holders: 0,
            };
        };
        let me = lobby.me();
        let holders: Vec<&LobbyPlayer> =
            lobby.players.iter().filter(|p| p.slot.is_some()).collect();
        Self {
            connected: true,
            king: lobby.is_king(),
            server: lobby.king.is_none(),
            phase: lobby.phase,
            start: lobby.start,
            holds: me.and_then(|m| m.slot),
            ready: me.is_some_and(|m| m.ready),
            armed: me.is_some_and(|m| m.loadout),
            flying: me.is_some_and(|m| m.flying),
            unable: unable
                .map(str::to_owned)
                .or_else(|| me.and_then(|m| m.unable.clone())),
            waiting: holders
                .iter()
                .filter(|p| !p.ready)
                .map(|p| p.callsign.clone())
                .collect(),
            holders: holders.len(),
        }
    }

    /// The King's Start is accepted now (the host's own rule).
    pub fn all_ready(&self) -> bool {
        self.holders > 0 && self.waiting.is_empty()
    }
}

/// The state of every button for one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub mission: Show,
    pub loadout: Show,
    pub ready: Show,
    pub kick: Show,
    pub fly: Show,
    pub fly_as: FlyAs,
    pub leave: Show,
}

/// Which buttons the player may press. `kickable` is a player other than the
/// King is selected in Players.
pub fn buttons(facts: &Facts, kickable: bool) -> Buttons {
    let lobby_phase = facts.phase == LobbyPhase::Lobby;
    let flying = facts.phase == LobbyPhase::Flying;
    let on = |yes: bool| if yes { Show::Enabled } else { Show::Disabled };
    let king_only = |show: Show| if facts.king { show } else { Show::Hidden };
    let may_play = facts.connected && facts.unable.is_none() && !facts.flying;
    Buttons {
        // The mission changes only in the lobby.
        mission: king_only(on(facts.connected && lobby_phase)),
        // Loadouts are chosen in the lobby only, for the slot one holds.
        loadout: on(may_play && lobby_phase && facts.holds.is_some()),
        // Ready takes a slot; in flight it joins the flight.
        ready: on(may_play && facts.holds.is_some() && facts.phase != LobbyPhase::Ended),
        kick: king_only(on(facts.connected && kickable)),
        fly: king_only(on(
            facts.connected && ((lobby_phase && facts.all_ready()) || flying)
        )),
        fly_as: if flying {
            FlyAs::EndMission
        } else {
            FlyAs::Fly
        },
        leave: Show::Enabled,
    }
}

/// Why Fly cannot be pressed, in the host's words where it has them
/// ("Not ready: Hawk, Viper."); `None` when it can.
pub fn fly_block(facts: &Facts) -> Option<String> {
    if !facts.connected {
        return Some("Not connected yet.".into());
    }
    match facts.phase {
        LobbyPhase::Flying => return None,
        LobbyPhase::Ended => return Some("The mission has just ended.".into()),
        LobbyPhase::Lobby => {}
    }
    if facts.holders == 0 {
        return Some("Nobody holds a slot.".into());
    }
    if !facts.waiting.is_empty() {
        return Some(format!("Not ready: {}.", facts.waiting.join(", ")));
    }
    None
}

/// The button Enter presses (the blue one), if any: Fly when it can be
/// pressed, else Ready once a slot is held and the player is not ready.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultButton {
    Fly,
    Ready,
}

pub fn default_button(facts: &Facts, buttons: &Buttons) -> Option<DefaultButton> {
    if buttons.fly.is_enabled() && facts.phase == LobbyPhase::Lobby && facts.all_ready() {
        Some(DefaultButton::Fly)
    } else if buttons.ready.is_enabled() && !facts.ready {
        Some(DefaultButton::Ready)
    } else {
        None
    }
}

/// The label of the Ready button.
pub fn ready_label(facts: &Facts) -> &'static str {
    match (facts.phase, facts.ready) {
        (LobbyPhase::Flying, _) => "Join",
        (_, true) => "Not Ready",
        (_, false) => "Ready",
    }
}

/// The start rule in plain words: the line under the mission.
pub fn rule_text(lobby: &LobbyState) -> String {
    let king = lobby
        .king
        .and_then(|id| lobby.player(id))
        .map(|p| p.callsign.as_str());
    match (lobby.start, king) {
        (StartRule::King, Some(king)) => format!(
            "{king} is the King: the mission starts when {king} presses Fly and everyone holding a slot is ready."
        ),
        (StartRule::King, None) => {
            "The mission starts when the King presses Fly and everyone holding a slot is ready."
                .into()
        }
        (StartRule::FirstReady, _) => {
            "This server starts the mission as soon as the first player holding a slot is ready."
                .into()
        }
        (StartRule::Flying, _) => {
            "This server's mission is always flying: take a slot and press Ready to join it.".into()
        }
    }
}

/// The line under the lists: what to do next, in words.
pub fn hint(facts: &Facts) -> String {
    if !facts.connected {
        return "Connecting to the game...".into();
    }
    if let Some(reason) = &facts.unable {
        return format!("Your game cannot play this mission: {reason}");
    }
    match facts.phase {
        LobbyPhase::Flying => match (facts.flying, facts.holds, facts.ready) {
            (true, ..) => "You are flying.".into(),
            (_, None, _) => {
                "The mission is flying. Click a free slot, then press Join to fly with them.".into()
            }
            (_, Some(_), _) => "Press Join to take your aircraft in the mission.".into(),
        },
        LobbyPhase::Ended => "The mission has ended. Back to the lobby in a moment.".into(),
        LobbyPhase::Lobby => {
            if facts.holds.is_none() {
                return "Click a free slot to take it, then choose a loadout and press Ready."
                    .into();
            }
            if !facts.ready {
                return if facts.armed {
                    "Press Ready when you are set. Loadout changes your stores.".into()
                } else {
                    "Choose a loadout, or press Ready to fly the standard stores.".into()
                };
            }
            match (facts.king, facts.start) {
                (true, _) if facts.all_ready() => "Everyone is ready. Press Fly.".into(),
                (_, StartRule::Flying) => "Ready.".into(),
                (_, StartRule::FirstReady) => "Ready. The mission starts at once.".into(),
                _ if facts.waiting.is_empty() => "Ready. Waiting for the King to start.".into(),
                (true, _) => format!("Waiting for {} to be ready.", facts.waiting.join(", ")),
                _ => format!("Ready. Waiting for {}.", facts.waiting.join(", ")),
            }
        }
    }
}

/// A player's state as a short word for the Players list.
pub fn status_word(player: &LobbyPlayer) -> &'static str {
    if player.unable.is_some() {
        "Unable"
    } else if player.flying {
        "Flying"
    } else if player.ready {
        "Ready"
    } else if player.loadout {
        "Armed"
    } else if player.slot.is_some() {
        "Slot"
    } else {
        ""
    }
}

/// The Players list's rows: the crown, the house, the ready tick, the
/// platform, the name and the state. Unable players are red and the player's own row green.
pub fn player_rows(lobby: &LobbyState) -> Vec<Row> {
    lobby
        .players
        .iter()
        .map(|p| {
            let row = Row::new(
                p.id.to_string(),
                vec![
                    if lobby.king == Some(p.id) {
                        Cell::Icon(Icon::Crown)
                    } else {
                        Cell::Empty
                    },
                    if lobby.host == Some(p.id) {
                        Cell::Icon(Icon::House)
                    } else {
                        Cell::Empty
                    },
                    if p.unable.is_some() {
                        Cell::Icon(Icon::Unable)
                    } else if p.ready || p.flying {
                        Cell::Icon(Icon::Ready)
                    } else {
                        Cell::Empty
                    },
                    Icon::of_platform(p.platform).map_or(Cell::Empty, Cell::Icon),
                    Cell::Text(p.callsign.clone()),
                    Cell::Text(status_word(p).to_owned()),
                ],
            );
            if p.unable.is_some() {
                row.tinted(tone::ENEMY)
            } else if p.id == lobby.you {
                row.tinted(tone::OWN_SIDE)
            } else {
                row
            }
        })
        .collect()
}

/// The Slots list's rows: the player's own mark, wing and member, aircraft,
/// who holds it (AI when nobody does) and the holder's ready tick. A slot
/// someone else holds is dimmed: it cannot be clicked.
pub fn slot_rows(lobby: &LobbyState) -> Vec<Row> {
    lobby
        .slots
        .iter()
        .map(|slot| {
            let holder = slot.holder.and_then(|id| lobby.player(id));
            let mine = slot.holder == Some(lobby.you);
            let row = Row::new(
                slot.plane.to_string(),
                vec![
                    if mine {
                        Cell::Icon(Icon::You)
                    } else {
                        Cell::Empty
                    },
                    Cell::Text(format!(
                        "Wing {} #{}",
                        slot.wing.display_number(),
                        u32::from(slot.member) + 1
                    )),
                    Cell::Text(slot.aircraft.label().to_owned()),
                    Cell::Text(holder.map_or_else(|| "AI".to_owned(), |p| p.callsign.clone())),
                    if holder.is_some_and(|p| p.ready || p.flying) {
                        Cell::Icon(Icon::Ready)
                    } else {
                        Cell::Empty
                    },
                ],
            );
            match holder {
                Some(_) if !mine => row.dimmed(),
                Some(_) => row.tinted(tone::OWN_SIDE),
                None => row,
            }
        })
        .collect()
}

/// What a click on a slot asks for: take it, free one's own, or nothing (a
/// slot someone else holds, or a click that cannot be honoured).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotClick {
    Take(u32),
    Leave,
    /// The reason in words, for Messages.
    Refused(&'static str),
}

pub fn slot_click(lobby: &LobbyState, facts: &Facts, plane: u32) -> SlotClick {
    let Some(slot) = lobby.slots.iter().find(|s| s.plane == plane) else {
        return SlotClick::Refused("There is no such slot.");
    };
    if slot.holder == Some(lobby.you) {
        return if facts.flying {
            SlotClick::Refused("Leave your aircraft before you change your slot.")
        } else {
            SlotClick::Leave
        };
    }
    if slot.holder.is_some() {
        return SlotClick::Refused("Another player holds that slot.");
    }
    if facts.flying {
        return SlotClick::Refused("Leave your aircraft before you change your slot.");
    }
    if facts.unable.is_some() {
        return SlotClick::Refused(
            "Your game cannot play this mission, so you cannot take a slot.",
        );
    }
    SlotClick::Take(plane)
}

/// The lines a change of the lobby's state puts in Messages: who came and
/// went, a new mission, the mission starting and ending, a player's game that
/// cannot play it. `old` is the state before (`None` for the first).
pub fn change_lines(old: Option<&LobbyState>, new: &LobbyState) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(old) = old else {
        lines.push(format!("Joined {}.", new.name));
        return lines;
    };
    for p in &new.players {
        match old.player(p.id) {
            None if p.id != new.you => lines.push(format!("{} joined the game.", p.callsign)),
            Some(before) if before.callsign != p.callsign => {}
            _ => {}
        }
        if let Some(why) = &p.unable
            && old
                .player(p.id)
                .is_none_or(|b| b.unable.as_ref() != Some(why))
        {
            lines.push(if p.id == new.you {
                format!("Your game cannot play this mission: {why}")
            } else {
                format!("{} cannot play this mission: {why}", p.callsign)
            });
        }
    }
    for p in &old.players {
        if new.player(p.id).is_none() && p.id != old.you {
            lines.push(format!("{} left the game.", p.callsign));
        }
    }
    if new.mission != old.mission {
        lines.push(format!("The mission is now: {}", new.summary));
    }
    if new.king != old.king
        && let Some(king) = new.king.and_then(|id| new.player(id))
    {
        lines.push(format!("{} is the King now.", king.callsign));
    }
    match (old.phase, new.phase) {
        (LobbyPhase::Lobby, LobbyPhase::Flying) => lines.push(
            if new.me().is_some_and(|m| m.flying) {
                "The mission is flying."
            } else {
                "The mission is flying. Take a slot and press Join to fly with them."
            }
            .to_owned(),
        ),
        (LobbyPhase::Flying | LobbyPhase::Ended, LobbyPhase::Lobby) => {
            lines.push("Back in the lobby.".into());
        }
        _ => {}
    }
    lines
}

/// The detail line for a player selected in Players: an unable player's
/// reason, else nothing.
pub fn player_detail(lobby: &LobbyState, id: u8) -> Option<String> {
    let p = lobby.player(id)?;
    p.unable
        .as_ref()
        .map(|why| format!("{} cannot play this mission: {why}", p.callsign))
}

/// Why a button that cannot be pressed cannot, in words, for Messages when
/// the player clicks it. `None` for a button that can, or has no reason.
pub fn disabled_reason(facts: &Facts, id: super::Id, kickable: bool) -> Option<String> {
    use super::Id;
    if !facts.connected {
        return Some("Not connected yet.".into());
    }
    let lobby_only = facts.phase != LobbyPhase::Lobby;
    match id {
        Id::Mission if lobby_only => Some("The mission can change only in the lobby.".into()),
        Id::Loadout | Id::Ready if facts.unable.is_some() => facts
            .unable
            .as_ref()
            .map(|why| format!("Your game cannot play this mission: {why}")),
        Id::Loadout if lobby_only => {
            Some("Loadouts are chosen in the lobby, before the mission flies.".into())
        }
        Id::Loadout | Id::Ready if facts.holds.is_none() => Some("Take a slot first.".into()),
        Id::Ready if facts.flying => Some("You are flying.".into()),
        Id::Kick if !kickable => Some("Select another player in Players to kick.".into()),
        Id::Fly => fly_block(facts),
        _ => None,
    }
}
