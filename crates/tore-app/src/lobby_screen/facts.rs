//! What the lobby screen decides from the lobby's state: who may press which
//! button, the words that say why one cannot be pressed, the slot and player
//! rows, and the lines a change of state puts in Messages. All of it is plain
//! data in and out, so every rule is tested without a window, a kit or a
//! session.
use crate::widgets::{Cell, Icon, Row, tone};
use std::collections::BTreeSet;
use tore_session::client::content;
use tore_session::settings::{self, number};
use tore_session::wire::Path;
use tore_session::wire::messages::{
    ContentGaps, LobbyPhase, LobbyPlayer, LobbySlot, LobbyState, Lock, StandbyMark, StartRule,
};

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
    /// This player watches the flying mission with no plane (protocol 8).
    pub observing: bool,
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
                observing: false,
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
            observing: me.is_some_and(|m| m.observing),
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

/// What the Loadout button's place says and does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadoutAs {
    /// Opens Load Ordnance for the slot held (the lobby, between missions).
    Loadout,
    /// The mission flies: asks the host to stream it to a player with no
    /// plane (slice F2-O1's stream).
    Watch,
    /// The player watches: stops it.
    StopWatch,
}

/// The state of every button for one player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Buttons {
    pub mission: Show,
    pub settings: Show,
    pub players: Show,
    pub loadout: Show,
    pub loadout_as: LoadoutAs,
    pub ready: Show,
    pub fly: Show,
    pub fly_as: FlyAs,
    pub leave: Show,
}

/// Which buttons the player may press. `target` is a player other than
/// oneself selected in Players, whom the King's Players... acts on.
pub fn buttons(facts: &Facts, target: bool) -> Buttons {
    let lobby_phase = facts.phase == LobbyPhase::Lobby;
    let flying = facts.phase == LobbyPhase::Flying;
    let on = |yes: bool| if yes { Show::Enabled } else { Show::Disabled };
    let king_only = |show: Show| if facts.king { show } else { Show::Hidden };
    let may_play = facts.connected && facts.unable.is_none() && !facts.flying;
    Buttons {
        // Everyone can open the mission page (John, 2026-10-09): the King edits
        // it in the lobby, anyone else reads it, and so does the King while the
        // mission flies (it changes only in the lobby).
        mission: on(facts.connected),
        // Every player sees the settings (greyed unless the King's).
        settings: on(facts.connected),
        players: king_only(on(facts.connected && target)),
        // Loadouts are chosen in the lobby only, for the slot one holds; while
        // the mission flies the same place watches it.
        loadout: if flying {
            on(facts.connected && !facts.flying)
        } else {
            on(may_play && lobby_phase && facts.holds.is_some())
        },
        loadout_as: match (flying, facts.observing) {
            (false, _) => LoadoutAs::Loadout,
            (true, false) => LoadoutAs::Watch,
            (true, true) => LoadoutAs::StopWatch,
        },
        // Ready takes a slot; in flight it joins the flight.
        ready: on(may_play && facts.holds.is_some() && facts.phase != LobbyPhase::Ended),
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

/// The line above the buttons, in the dim face: what this player should do
/// next so the mission can start, in plain words (lobby pass L2, John
/// 2026-10-09). It replaces the old line under the mission that explained the
/// King's rule to everyone. `None` while there is nothing to say (not
/// connected, or the mission has just ended).
pub fn ready_hint(facts: &Facts) -> Option<&'static str> {
    if !facts.connected {
        return None;
    }
    // The reason itself is in the hint line above.
    if facts.unable.is_some() {
        return Some("Your game cannot play this mission.");
    }
    match facts.phase {
        LobbyPhase::Ended => None,
        LobbyPhase::Flying => Some(if facts.flying {
            "You are flying the mission."
        } else if facts.observing {
            "You are watching the mission."
        } else if facts.holds.is_none() {
            "The mission is flying: take a slot and press Join."
        } else {
            "The mission is flying: press Join to take your aircraft in."
        }),
        LobbyPhase::Lobby => Some(match (facts.start, facts.king) {
            // A server that starts when its first player is ready, or that
            // is always flying, has no King to wait for.
            (StartRule::Flying, _) => {
                "This server's mission is always flying: take a slot and press Ready to join it."
            }
            (StartRule::FirstReady, _) => {
                "The mission starts as soon as the first player holding a slot is ready."
            }
            // The King: Fly is theirs.
            (StartRule::King, true) if facts.all_ready() => {
                "Everyone is ready: press Fly to start the mission."
            }
            (StartRule::King, true) => "Press Fly when everyone holding a slot is ready.",
            // A game with no King named (the crown is between players).
            (StartRule::King, false) if facts.server => {
                "The mission starts when the King presses Fly and everyone holding a slot is ready."
            }
            (StartRule::King, false) if facts.holds.is_none() => {
                "Take a slot and press Ready so the King can start the mission."
            }
            (StartRule::King, false) if !facts.ready => {
                "Press Ready so the King can start the mission."
            }
            (StartRule::King, false) => "You are ready. The King starts the mission with Fly.",
        }),
    }
}

/// The line under the lists: what to do next, in words.
pub fn hint(facts: &Facts) -> String {
    if !facts.connected {
        return "Connecting to the game...".into();
    }
    // The reason is the host's own words, or the game's own in the second
    // person: it reads as it is (stage L).
    if let Some(reason) = &facts.unable {
        return reason.clone();
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
    } else if player.away {
        // The AI flies the player's aircraft, kept for it (slice F2-A).
        "Away"
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

/// The Players list's rows: the crown, the house (or, for a game that stands
/// by to host, the standby mark; stage K), the ready tick, the platform, the
/// relay mark (slice J6), the name and the state. Unable players are red and
/// the player's own row green.
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
                    } else if p.standby != StandbyMark::None {
                        // A game that stands by to host (stage K).
                        Cell::Icon(Icon::Standby)
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
                    Icon::of_path(p.path).map_or(Cell::Empty, Cell::Icon),
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
                    } else if slot.holder.is_none() && slot.reserved.is_some() {
                        // The AI flies it for a player who is not here; no
                        // one else takes it (stage K).
                        Cell::Icon(Icon::Lock)
                    } else {
                        Cell::Empty
                    },
                    Cell::Text(format!(
                        "Wing {} #{}",
                        slot.wing.display_number(),
                        u32::from(slot.member) + 1
                    )),
                    Cell::Text(slot.aircraft.label().to_owned()),
                    Cell::Text(slot_holder_text(slot, holder)),
                    if holder.is_some_and(|p| p.ready || p.flying) {
                        Cell::Icon(Icon::Ready)
                    } else {
                        Cell::Empty
                    },
                ],
            );
            match (holder, &slot.lock) {
                (Some(_), _) if !mine => row.dimmed(),
                (Some(_), _) => row.tinted(tone::OWN_SIDE),
                // A plane kept for a player who dropped: only that player
                // takes it (the host checks the callsign).
                (None, _) if slot.reserved.as_ref().is_some_and(|c| !is_me(lobby, c)) => {
                    row.dimmed()
                }
                // A slot the King closed, or keeps for another, cannot be
                // taken: dimmed as one someone holds.
                (None, Lock::Closed) => row.dimmed(),
                (None, Lock::Reserved(callsign)) if !is_me(lobby, callsign) => row.dimmed(),
                (None, _) => row,
            }
        })
        .collect()
}

/// What a click on a slot asks for: take it, free one's own, or nothing (a
/// slot someone else holds, or a click that cannot be honoured).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotClick {
    Take(u32),
    Leave,
    /// The reason in words, for Messages.
    Refused(String),
}

/// Whether `callsign` is the receiving player's own.
fn is_me(lobby: &LobbyState, callsign: &str) -> bool {
    lobby.me().is_some_and(|m| m.callsign == callsign)
}

pub fn slot_click(lobby: &LobbyState, facts: &Facts, plane: u32) -> SlotClick {
    let Some(slot) = lobby.slots.iter().find(|s| s.plane == plane) else {
        return SlotClick::Refused("There is no such slot.".into());
    };
    if slot.holder == Some(lobby.you) {
        return if facts.flying {
            SlotClick::Refused("Leave your aircraft before you change your slot.".into())
        } else {
            SlotClick::Leave
        };
    }
    if slot.holder.is_some() {
        return SlotClick::Refused("Another player holds that slot.".into());
    }
    if let Some(callsign) = slot.reserved.as_ref().filter(|c| !is_me(lobby, c)) {
        return SlotClick::Refused(format!(
            "Plane {plane} is kept for {callsign}, who is away."
        ));
    }
    if facts.flying {
        return SlotClick::Refused("Leave your aircraft before you change your slot.".into());
    }
    if facts.unable.is_some() {
        return SlotClick::Refused(
            "Your game cannot play this mission, so you cannot take a slot.".into(),
        );
    }
    match &slot.lock {
        Lock::Closed => {
            return SlotClick::Refused(format!("Plane {plane} is closed: the AI flies it."));
        }
        Lock::Reserved(callsign) if !is_me(lobby, callsign) => {
            return SlotClick::Refused(format!("Plane {plane} is kept for {callsign}."));
        }
        _ => {}
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
        // The reason is the host's words, which start with the player's
        // callsign ("Hawk's game has no Su-27, which this mission flies."),
        // so it is said as it is. The player's own words are its game's, in
        // the second person, and the screen says them itself (`take`).
        if p.id != new.you
            && let Some(why) = &p.unable
            && old
                .player(p.id)
                .is_none_or(|b| b.unable.as_ref() != Some(why))
        {
            lines.push(why.clone());
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
    if new.name != old.name {
        lines.push(format!("The game is now called {}.", new.name));
    }
    let changed: Vec<(u8, u32)> = new
        .settings
        .iter()
        .filter(|(n, v)| {
            old.settings.iter().any(|(k, _)| k == n) && !old.settings.contains(&(*n, *v))
        })
        .copied()
        .collect();
    // The pinned host is a player's id in the registry: said as the player
    // (stage K). The King reads the host's own line.
    let pinned = changed.iter().find(|(n, _)| *n == number::HOST).copied();
    let others: Vec<(u8, u32)> = changed
        .iter()
        .filter(|(n, _)| *n != number::HOST)
        .copied()
        .collect();
    if !others.is_empty() {
        lines.push(format!("Settings: {}.", settings::words(&others)));
    }
    if let Some((_, value)) = pinned
        && !new.is_king()
    {
        lines.push(match pinned_callsign(new, value) {
            Some(callsign) => format!("The King pinned {callsign} as the host."),
            None => "The King left the host to be calculated.".to_owned(),
        });
    }
    for slot in &new.slots {
        if old
            .slots
            .iter()
            .find(|s| s.plane == slot.plane)
            .is_some_and(|before| before.lock != slot.lock)
        {
            lines.push(lock_line(slot.plane, &slot.lock));
        }
    }
    if new.king != old.king
        && let Some(king) = new.king.and_then(|id| new.player(id))
    {
        lines.push(format!("{} is the King now.", king.callsign));
    }
    // This game's own standing by, said when it begins (stage K).
    let mark = |state: &LobbyState| state.me().map_or(StandbyMark::None, |me| me.standby);
    if mark(old) != mark(new) {
        match mark(new) {
            StandbyMark::First => lines.push(
                "Your game stands by to host, first in line: if the host is lost, it takes the game over."
                    .into(),
            ),
            StandbyMark::Second => lines.push(
                "Your game stands by to host, second in line: it takes the game over if the first cannot."
                    .into(),
            ),
            StandbyMark::None => {}
        }
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

/// The callsign of the player the King pinned (`value` of the host setting,
/// a lobby id plus one); `None` for calculated, or a player who is not here.
pub fn pinned_callsign(lobby: &LobbyState, value: u32) -> Option<String> {
    let id = u8::try_from(value.checked_sub(1)?).ok()?;
    lobby.player(id).map(|p| p.callsign.clone())
}

/// How a player reached the host, as the detail line says it (slice J6),
/// after the player's callsign.
pub fn path_phrase(path: Path) -> &'static str {
    match path {
        Path::LocalNetwork => "over the local network",
        Path::ByAddress => "directly",
        Path::MappedPort => "directly (forwarded port)",
        Path::Ipv6 => "directly (IPv6)",
        Path::Punched => "directly (punched through)",
        Path::Relay => "through the relay",
    }
}

/// The detail line for a player selected in Players: an unable player's
/// reason as the host worded it, else the player's system, then how the
/// player reached the host (slice J6; the house's own game says it runs this
/// game, since it needs no path), as in "Hawk: on Linux. Connected
/// directly." (stage L; the build was dropped on John's word, 2026-10-06).
pub fn player_detail(lobby: &LobbyState, id: u8) -> Option<String> {
    let p = lobby.player(id)?;
    if let Some(why) = &p.unable {
        return Some(why.clone());
    }
    let how = if lobby.host == Some(id) {
        "Runs this game.".to_owned()
    } else {
        format!("Connected {}.", path_phrase(p.path))
    };
    let standby = match p.standby {
        StandbyMark::None => "",
        StandbyMark::First => " Stands by to host: first.",
        StandbyMark::Second => " Stands by to host: second.",
    };
    Some(format!("{} {how}{standby}", content::hint_line(p)))
}

/// The Messages lines about how a player's items differ from the host's
/// (stage L): said once for each player, as soon as a gap names it. A
/// player's Fighters Anthology build is not said (John, 2026-10-06: the build
/// audit found no difference a player sees).
#[derive(Debug, Default)]
pub struct GapNotes {
    said: BTreeSet<u8>,
}

impl GapNotes {
    /// The lines now due, given the lobby and the host's newest gaps (none
    /// before they arrive). A player who leaves is forgotten, so one who
    /// returns is told about again.
    pub fn lines(&mut self, lobby: &LobbyState, gaps: Option<&ContentGaps>) -> Vec<String> {
        self.said.retain(|id| lobby.player(*id).is_some());
        let Some(gaps) = gaps else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for p in &lobby.players {
            if self.said.contains(&p.id) {
                continue;
            }
            if let Some(line) = content::differs_line(p, p.id == lobby.you, gaps) {
                self.said.insert(p.id);
                lines.push(line);
            }
        }
        lines
    }
}

/// Why a button that cannot be pressed cannot, in words, for Messages when
/// the player clicks it. `None` for a button that can, or has no reason.
/// `target` is a player other than oneself selected in Players.
pub fn disabled_reason(facts: &Facts, id: super::Id, target: bool) -> Option<String> {
    use super::Id;
    if !facts.connected {
        return Some("Not connected yet.".into());
    }
    let lobby_only = facts.phase != LobbyPhase::Lobby;
    let flying = facts.phase == LobbyPhase::Flying;
    match id {
        Id::Loadout if flying && facts.flying => Some("You are flying.".into()),
        Id::Loadout if flying => None,
        Id::Loadout | Id::Ready if facts.unable.is_some() => facts.unable.clone(),
        Id::Loadout if lobby_only => {
            Some("Loadouts are chosen in the lobby, before the mission flies.".into())
        }
        Id::Loadout | Id::Ready if facts.holds.is_none() => Some("Take a slot first.".into()),
        Id::Ready if facts.flying => Some("You are flying.".into()),
        Id::PlayersPanel if !target => {
            Some("Select another player in Players, or a reserved aircraft in Slots, first.".into())
        }
        Id::Fly => fly_block(facts),
        _ => None,
    }
}

// ---- slot locks ----

/// What a slot's holder column says: who holds it, or "AI", or the King's
/// lock on it.
pub fn slot_holder_text(slot: &LobbySlot, holder: Option<&LobbyPlayer>) -> String {
    match (&slot.lock, holder) {
        // The AI flies the plane while its player is away, kept for it.
        (_, Some(p)) if p.away => format!("AI ({} away)", p.callsign),
        (_, Some(p)) => p.callsign.clone(),
        // A dropped player's plane: the AI flies it, kept for the player
        // (stage K; the same words as an away player's).
        (_, None) if slot.reserved.is_some() => {
            format!("AI ({} away)", slot.reserved.as_deref().unwrap_or_default())
        }
        (Lock::Closed, None) => "Closed (AI)".to_owned(),
        (Lock::Reserved(callsign), None) => format!("Reserved: {callsign}"),
        (Lock::Open, None) => "AI".to_owned(),
    }
}

/// What a King's right click on a slot asks for. With a player selected in
/// Players the slot is reserved for that player (or opened again when it is
/// reserved for that player already); with none it cycles open and closed
/// (a reserved slot opens).
pub fn lock_click(slot: &LobbySlot, selected: Option<&str>) -> Lock {
    match (&slot.lock, selected) {
        (Lock::Reserved(now), Some(callsign)) if now == callsign => Lock::Open,
        (_, Some(callsign)) => Lock::Reserved(callsign.to_owned()),
        (Lock::Open, None) => Lock::Closed,
        (_, None) => Lock::Open,
    }
}

/// The line that tells the King what a lock did.
pub fn lock_line(plane: u32, lock: &Lock) -> String {
    match lock {
        Lock::Open => format!("Plane {plane}'s slot is open."),
        Lock::Closed => format!("Plane {plane}'s slot is closed: the AI flies it."),
        Lock::Reserved(callsign) => format!("Plane {plane}'s slot is kept for {callsign}."),
    }
}

// ---- the head's summary of the settings ----

/// The settings in words, one line, for the lobby's head: "Co-op, friendly
/// fire on, no revival" or "PvP by sides, 5 kills or 10 minutes, revival
/// with unlimited lives". `None` before the host has sent any.
pub fn settings_summary(lobby: &LobbyState) -> Option<String> {
    if lobby.settings.is_empty() {
        return None;
    }
    let get = |n: u8| {
        lobby
            .settings
            .iter()
            .find(|(k, _)| *k == n)
            .map(|(_, v)| *v)
    };
    let pvp = get(number::MODE) == Some(1);
    let mut parts: Vec<String> = Vec::new();
    parts.push(if !pvp {
        "Co-op".to_owned()
    } else if get(number::FIGHT) == Some(1) {
        "PvP, every player for itself".to_owned()
    } else {
        "PvP by sides".to_owned()
    });
    let fire = get(number::FRIENDLY_FIRE).unwrap_or(1) != 0;
    if !pvp || !fire {
        parts.push(format!("friendly fire {}", if fire { "on" } else { "off" }));
    }
    let mut limits: Vec<String> = Vec::new();
    if pvp && let Some(kills) = get(number::KILL_LIMIT).filter(|k| *k > 0) {
        limits.push(format!("{kills} kill{}", if kills == 1 { "" } else { "s" }));
    }
    if let Some(seconds) = get(number::TIME_LIMIT).filter(|t| *t > 0) {
        limits.push(
            settings::setting(number::TIME_LIMIT).map_or_else(String::new, |s| s.text(seconds)),
        );
    }
    if !limits.is_empty() {
        parts.push(limits.join(" or "));
    }
    parts.push(match get(number::RESPAWN) {
        Some(1) => "revival in a free AI aircraft".to_owned(),
        Some(2) => match get(number::LIVES) {
            Some(1) => "revival with 1 life".to_owned(),
            Some(n) if n <= 10 => format!("revival with {n} lives"),
            _ => "revival with unlimited lives".to_owned(),
        },
        _ => "no revival".to_owned(),
    });
    if get(number::LOCK_SIDES) == Some(1) {
        parts.push("sides locked".to_owned());
    }
    if get(number::LOADOUTS) == Some(1) {
        parts.push("any loadout".to_owned());
    }
    Some(parts.join(", "))
}
