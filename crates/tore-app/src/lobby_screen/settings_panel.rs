//! The Settings... panel of the lobby screen (stage F phase 2, slice F2-L):
//! the King's settings ([`tore_session::settings`]) as rows of text buttons,
//! on four pages (Game, Revival, Scoring, Realism). Every player can open
//! it; for anyone but the King every row is greyed, as retail greys a
//! client's settings, and so is a row that does not apply (scoring in
//! co-op, a lobby setting while the mission flies).
//!
//! A left click on a row turns it to its next value and a right click to the
//! one before (the Quick Mission creator's text buttons); the keyboard does
//! the same with Right and Left. Each turn is one [`Edit`] the lobby sends to
//! the host, which checks it whole and refuses it in words (shown at the
//! foot of the panel); the panel shows the value the host's lobby state
//! carries, never a guess.
//!
//! The Game page also holds the game's name and its password, two text
//! lines the King applies with Enter. The Realism page edits the lobby
//! mission's cheats ([`tore_sim::cheats::Cheats`]) and sends the whole
//! mission again as a mission change; nothing changes a flying mission's
//! cheats, so every client's prediction runs the same rules.
//!
//! Everything about which row is shown, greyed and what a click does lives in
//! plain functions of a [`Context`] (tested without a window); the
//! [`SettingsPanel`] only keeps the page, the selected row, the two text
//! fields and the buttons, and draws.
use crate::menu::{Canvas, text_width};
use crate::ui_text;
use crate::widgets::{
    Button, Filter, Kit, Outcome, Point, Rect, TextField, draw_panel, fit, inside,
};
use tore_session::settings::{self, Allowed, Change, Setting, number};
use tore_session::wire::messages::{LobbyPhase, LobbyState, PasswordChange, SettingsChange};
use tore_sim::ai::Experience;
use tore_sim::cheats::{Cheats, Damage};

/// The panel's place over the lobby.
pub const PANEL: Rect = (45, 100, 550, 362);
/// Where a row starts, the pitch between rows, and a row's height.
const ROWS_TOP: i32 = PANEL.1 + 66;
const PITCH: i32 = 21;
/// The Game page has twelve rows since the Host row (stage K), so its rows
/// sit a little closer to leave the foot's line its place.
const GAME_PITCH: i32 = 19;
const ROW_HEIGHT: i32 = 18;
/// The frame lines' colour and the selected row's.
const LINE: [u8; 4] = [174, 174, 174, 255];
const SELECTED: [u8; 4] = [235, 225, 179, 255];
const BOX: [u8; 4] = [81, 81, 81, 255];
/// What a pin made while the mission flies does (stage K).
const PIN_IN_FLIGHT: &str = "A pin made in flight applies when the lobby returns.";
/// Why a dedicated server's snapshot rate row is greyed (slice R1): its
/// operator sets it in the configuration file.
const RATE_IS_THE_SERVERS: &str = "This server's snapshot rate is set by its operator.";
/// The most bytes of a game's name (the host's rule).
const NAME_BYTES: usize = 64;

/// The four pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Game,
    Revival,
    Scoring,
    Realism,
}

impl Page {
    pub const ALL: [Page; 4] = [Page::Game, Page::Revival, Page::Scoring, Page::Realism];

    pub fn label(self) -> &'static str {
        match self {
            Page::Game => "Game",
            Page::Revival => "Revival",
            Page::Scoring => "Scoring",
            Page::Realism => "Realism",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0)
    }

    fn step(self, forward: bool) -> Page {
        let n = Self::ALL.len();
        Self::ALL[(self.index() + if forward { 1 } else { n - 1 }) % n]
    }
}

/// One cheat switch of the Realism page (the in-flight Cheat menu's rows that
/// work; Guns only is the mission's own and is set in Mission...).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    UnlimitedAmmo,
    UnlimitedFuel,
    NoSpins,
    NoTurbulence,
    ExtraG,
    IgnoreWeights,
    NoSunWhiteout,
    NoGEffects,
    NoScreenShake,
    NoCrashes,
    EasyAiming,
    IgnoreCollisions,
    EasyTargeting,
}

impl Flag {
    pub const ALL: [Flag; 13] = [
        Flag::UnlimitedAmmo,
        Flag::UnlimitedFuel,
        Flag::NoSpins,
        Flag::NoTurbulence,
        Flag::ExtraG,
        Flag::IgnoreWeights,
        Flag::NoSunWhiteout,
        Flag::NoGEffects,
        Flag::NoScreenShake,
        Flag::NoCrashes,
        Flag::EasyAiming,
        Flag::IgnoreCollisions,
        Flag::EasyTargeting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Flag::UnlimitedAmmo => "Unlimited ammo",
            Flag::UnlimitedFuel => "Unlimited fuel",
            Flag::NoSpins => "No spins",
            Flag::NoTurbulence => "No turbulence",
            Flag::ExtraG => "Pull extra G",
            Flag::IgnoreWeights => "Ignore weapon weights",
            Flag::NoSunWhiteout => "No sun whiteout",
            Flag::NoGEffects => "No redout or blackout",
            Flag::NoScreenShake => "No screen shake",
            Flag::NoCrashes => "No crashes",
            Flag::EasyAiming => "Easy aiming",
            Flag::IgnoreCollisions => "Ignore midair collisions",
            Flag::EasyTargeting => "Easy targeting",
        }
    }

    fn slot(self, cheats: &mut Cheats) -> &mut bool {
        match self {
            Flag::UnlimitedAmmo => &mut cheats.unlimited_ammo,
            Flag::UnlimitedFuel => &mut cheats.unlimited_fuel,
            Flag::NoSpins => &mut cheats.no_spins,
            Flag::NoTurbulence => &mut cheats.no_turbulence,
            Flag::ExtraG => &mut cheats.extra_g,
            Flag::IgnoreWeights => &mut cheats.ignore_weapon_weights,
            Flag::NoSunWhiteout => &mut cheats.no_sun_whiteout,
            Flag::NoGEffects => &mut cheats.no_g_effects,
            Flag::NoScreenShake => &mut cheats.no_screen_shake,
            Flag::NoCrashes => &mut cheats.no_crashes,
            Flag::EasyAiming => &mut cheats.easy_aiming,
            Flag::IgnoreCollisions => &mut cheats.ignore_midair_collisions,
            Flag::EasyTargeting => &mut cheats.easy_targeting,
        }
    }

    pub fn get(self, cheats: &Cheats) -> bool {
        let mut copy = *cheats;
        *self.slot(&mut copy)
    }
}

/// What a row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A registry setting, by number.
    Setting(u8),
    /// The game's name (a text line).
    Name,
    /// The password (a text line).
    Password,
    /// The Damage cheat: Normal, Invulnerable, Realistic.
    Damage,
    /// The Enemy AI cheat: Unchanged or a level.
    EnemyAi,
    Cheat(Flag),
    /// The host: calculated, or pinned to a player (setting 21; stage K).
    Host,
}

/// The rows of a page, in order.
pub fn page_rows(page: Page) -> Vec<Kind> {
    match page {
        Page::Game => vec![
            Kind::Name,
            Kind::Setting(number::MODE),
            Kind::Setting(number::MAX_PLAYERS),
            Kind::Setting(number::JOIN_IN_PROGRESS),
            Kind::Setting(number::VISIBILITY),
            Kind::Password,
            Kind::Setting(number::FRIENDLY_FIRE),
            Kind::Setting(number::LOCK_SIDES),
            Kind::Setting(number::LOADOUTS),
            Kind::Setting(number::IDLE_AI),
            Kind::Setting(number::SNAPSHOT_RATE),
            Kind::Host,
        ],
        Page::Revival => vec![
            Kind::Setting(number::RESPAWN),
            Kind::Setting(number::LIVES),
            Kind::Setting(number::REVIVE_DELAY),
            Kind::Setting(number::REVIVE_DISTANCE),
            Kind::Setting(number::REVIVE_WEAPONS),
        ],
        Page::Scoring => vec![
            Kind::Setting(number::FIGHT),
            Kind::Setting(number::TALLY),
            Kind::Setting(number::TIME_LIMIT),
            Kind::Setting(number::KILL_LIMIT),
            Kind::Setting(number::KILL_OWNER),
            Kind::Setting(number::OBSERVER_DELAY),
        ],
        Page::Realism => {
            let mut rows = vec![Kind::Damage, Kind::EnemyAi];
            rows.extend(Flag::ALL.map(Kind::Cheat));
            rows
        }
    }
}

/// The words on a row's left.
pub fn row_label(kind: Kind) -> &'static str {
    match kind {
        Kind::Name => "Game name",
        Kind::Password => "Password",
        Kind::Damage => "Damage",
        Kind::EnemyAi => "Enemy AI",
        Kind::Host => "Host",
        Kind::Cheat(flag) => flag.label(),
        Kind::Setting(n) => match n {
            number::MODE => "Game type",
            number::MAX_PLAYERS => "Players, at most",
            number::JOIN_IN_PROGRESS => "Join in progress",
            number::VISIBILITY => "Who can find it",
            number::FRIENDLY_FIRE => "Friendly fire",
            number::LOCK_SIDES => "Lock sides",
            number::LOADOUTS => "Loadouts",
            number::RESPAWN => "Revival",
            number::LIVES => "Lives",
            number::REVIVE_DELAY => "Revival delay",
            number::REVIVE_DISTANCE => "Revival distance",
            number::REVIVE_WEAPONS => "Revival weapons",
            number::FIGHT => "Fight",
            number::TALLY => "Score by",
            number::TIME_LIMIT => "Time limit",
            number::KILL_LIMIT => "Kill limit",
            number::KILL_OWNER => "Kill limit counts",
            number::OBSERVER_DELAY => "Observer delay",
            number::IDLE_AI => "AI flies idle aircraft after",
            number::SNAPSHOT_RATE => "Snapshot rate",
            _ => "",
        },
    }
}

/// What the panel reads of the lobby: plain data, so every rule on a row is
/// a function of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// The receiving player is the King.
    pub king: bool,
    pub phase: LobbyPhase,
    /// The settings by number, as the lobby state carries them.
    pub settings: Vec<(u8, u32)>,
    /// The lobby mission's cheats, when the mission has arrived.
    pub cheats: Option<Cheats>,
    pub name: String,
    /// The players the King may pin as the host, in join order (stage K).
    pub players: Vec<HostChoice>,
}

/// A player in the Host row's choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostChoice {
    pub id: u8,
    pub callsign: String,
    /// The player reaches the host through the relay, so it cannot host.
    pub relayed: bool,
    /// The player's game runs the host now.
    pub house: bool,
}

impl Context {
    pub fn of(lobby: &LobbyState, cheats: Option<Cheats>) -> Self {
        Self {
            king: lobby.is_king(),
            phase: lobby.phase,
            settings: lobby.settings.clone(),
            cheats,
            name: lobby.name.clone(),
            players: lobby
                .players
                .iter()
                .map(|p| HostChoice {
                    id: p.id,
                    callsign: p.callsign.clone(),
                    relayed: p.path == tore_session::wire::Path::Relay,
                    house: lobby.host == Some(p.id),
                })
                .collect(),
        }
    }

    /// The value of the host setting that pins `player` (a lobby id plus
    /// one).
    pub fn pin_value(player: u8) -> u32 {
        u32::from(player) + 1
    }

    /// The values the Host row turns through: calculated, then each player
    /// who can host (a relayed one never can; the host's own checks decide
    /// the rest, in words).
    pub fn host_choices(&self) -> Vec<u32> {
        std::iter::once(settings::CALCULATED_HOST)
            .chain(
                self.players
                    .iter()
                    .filter(|p| !p.relayed)
                    .map(|p| Self::pin_value(p.id)),
            )
            .collect()
    }

    /// The Host row's value in words: "Calculated (Maverick hosts)", or the
    /// pinned player's callsign.
    fn host_text(&self) -> String {
        match self.value(number::HOST) {
            None => "...".into(),
            Some(settings::CALCULATED_HOST) => match self.players.iter().find(|p| p.house) {
                Some(house) => format!("Calculated ({} hosts)", house.callsign),
                None => "Calculated".into(),
            },
            Some(value) => match self.players.iter().find(|p| Self::pin_value(p.id) == value) {
                Some(player) => player.callsign.clone(),
                None => format!("Player {}", value - 1),
            },
        }
    }

    pub fn value(&self, number: u8) -> Option<u32> {
        self.settings
            .iter()
            .find(|(n, _)| *n == number)
            .map(|(_, v)| *v)
    }

    /// The game is PvP (humans on either side).
    pub fn pvp(&self) -> bool {
        self.value(number::MODE) == Some(1)
    }

    /// A player's game runs the host: not a dedicated server, whose rate is
    /// its operator's file's (slice R1).
    pub fn hosted_by_a_player(&self) -> bool {
        self.players.iter().any(|p| p.house)
    }

    fn password_set(&self) -> bool {
        self.value(number::PASSWORD) == Some(1)
    }
}

/// Why a row is greyed, in the host's words where it has them; `Ok` when the
/// King may turn it now.
pub fn row_state(kind: Kind, ctx: &Context) -> Result<(), String> {
    if !ctx.king {
        return Err("Only the King may change the settings.".into());
    }
    let in_lobby_only = match kind {
        Kind::Setting(n) => settings::setting(n).is_some_and(|s| s.change == Change::InLobby),
        Kind::Damage | Kind::EnemyAi | Kind::Cheat(_) => true,
        Kind::Name | Kind::Password | Kind::Host => false,
    };
    if in_lobby_only && ctx.phase != LobbyPhase::Lobby {
        return Err("Change it in the lobby, between missions.".into());
    }
    if let Kind::Setting(n) = kind
        && settings::setting(n).is_some_and(|s| s.pvp_only)
        && !ctx.pvp()
    {
        return Err("This applies only in a PvP game.".into());
    }
    if matches!(kind, Kind::Damage | Kind::EnemyAi | Kind::Cheat(_)) && ctx.cheats.is_none() {
        return Err("The mission has not arrived yet.".into());
    }
    if kind == Kind::Setting(number::SNAPSHOT_RATE) && !ctx.hosted_by_a_player() {
        return Err(RATE_IS_THE_SERVERS.into());
    }
    Ok(())
}

/// A row's value in words.
pub fn row_value(kind: Kind, ctx: &Context) -> String {
    match kind {
        Kind::Setting(n) => match (settings::setting(n), ctx.value(n)) {
            (Some(setting), Some(value)) => setting.text(value),
            _ => "...".into(),
        },
        Kind::Name => ctx.name.clone(),
        Kind::Host => ctx.host_text(),
        Kind::Password => if ctx.password_set() { "set" } else { "none" }.into(),
        Kind::Damage => match ctx.cheats.map(|c| c.damage) {
            Some(Damage::Normal) => "Normal".into(),
            Some(Damage::Invulnerable) => "Invulnerable".into(),
            Some(Damage::Realistic) => "Realistic".into(),
            None => "...".into(),
        },
        Kind::EnemyAi => match ctx.cheats {
            Some(Cheats { enemy_ai: None, .. }) => "Unchanged".into(),
            Some(Cheats {
                enemy_ai: Some(level),
                ..
            }) => experience_word(level).into(),
            None => "...".into(),
        },
        Kind::Cheat(flag) => match ctx.cheats {
            Some(cheats) if flag.get(&cheats) => "On".into(),
            Some(_) => "Off".into(),
            None => "...".into(),
        },
    }
}

fn experience_word(level: Experience) -> &'static str {
    match level {
        Experience::Novice => "Novice",
        Experience::Average => "Average",
        Experience::Experienced => "Experienced",
        Experience::Ace => "Ace",
    }
}

/// Every value a setting cycles through, in order.
pub fn choices(setting: &Setting) -> Vec<u32> {
    match setting.allowed {
        Allowed::List(values) => values.to_vec(),
        Allowed::Range(min, max, extra) => {
            let mut values: Vec<u32> = (min..=max).collect();
            values.extend_from_slice(extra);
            values
        }
    }
}

/// The value after (or before) `now` among `choices`, wrapping round. A
/// value off the list (a server's file gives a time limit of any whole
/// minute) goes to the nearest listed value in the direction asked.
pub fn turn(choices: &[u32], now: u32, forward: bool) -> Option<u32> {
    if choices.is_empty() {
        return None;
    }
    if let Some(at) = choices.iter().position(|v| *v == now) {
        let n = choices.len();
        return Some(choices[(at + if forward { 1 } else { n - 1 }) % n]);
    }
    if forward {
        choices
            .iter()
            .copied()
            .filter(|v| *v > now)
            .min()
            .or_else(|| choices.iter().copied().min())
    } else {
        choices
            .iter()
            .copied()
            .filter(|v| *v < now)
            .max()
            .or_else(|| choices.iter().copied().max())
    }
}

/// What the panel asks the lobby to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    /// A change of settings (the host checks it whole).
    Settings(SettingsChange),
    /// The lobby mission with these cheats, as a mission change.
    Cheats(Cheats),
}

/// The edit a click on `kind` makes: the next value, or the one before. `None`
/// for a text line (the field applies it) or when the row is greyed.
pub fn click(kind: Kind, ctx: &Context, forward: bool) -> Option<Edit> {
    row_state(kind, ctx).ok()?;
    let setting_edit = |n: u8, value: u32| {
        Edit::Settings(SettingsChange {
            values: vec![(n, value)],
            ..SettingsChange::default()
        })
    };
    match kind {
        Kind::Setting(n) if n == number::PASSWORD => None,
        Kind::Setting(n) => {
            let setting = settings::setting(n)?;
            let now = ctx.value(n)?;
            let next = turn(&choices(setting), now, forward)?;
            Some(setting_edit(n, next))
        }
        Kind::Name | Kind::Password => None,
        Kind::Host => {
            let next = turn(&ctx.host_choices(), ctx.value(number::HOST)?, forward)?;
            Some(setting_edit(number::HOST, next))
        }
        Kind::Damage => {
            let mut cheats = ctx.cheats?;
            cheats.damage = match (cheats.damage, forward) {
                (Damage::Normal, true) | (Damage::Realistic, false) => Damage::Invulnerable,
                (Damage::Invulnerable, true) | (Damage::Normal, false) => Damage::Realistic,
                (Damage::Realistic, true) | (Damage::Invulnerable, false) => Damage::Normal,
            };
            Some(Edit::Cheats(cheats))
        }
        Kind::EnemyAi => {
            let mut cheats = ctx.cheats?;
            let levels = [
                None,
                Some(Experience::Novice),
                Some(Experience::Average),
                Some(Experience::Experienced),
                Some(Experience::Ace),
            ];
            let at = levels.iter().position(|l| *l == cheats.enemy_ai)?;
            let n = levels.len();
            cheats.enemy_ai = levels[(at + if forward { 1 } else { n - 1 }) % n];
            Some(Edit::Cheats(cheats))
        }
        Kind::Cheat(flag) => {
            let mut cheats = ctx.cheats?;
            let slot = flag.slot(&mut cheats);
            *slot = !*slot;
            Some(Edit::Cheats(cheats))
        }
    }
}

/// The edit Enter makes on the name line.
pub fn apply_name(ctx: &Context, text: &str) -> Result<Edit, String> {
    row_state(Kind::Name, ctx)?;
    let text = text.trim();
    if text.is_empty() {
        return Err("The game needs a name.".into());
    }
    if text.len() > NAME_BYTES {
        return Err(format!("A name is at most {NAME_BYTES} bytes."));
    }
    if text == ctx.name {
        return Err("That is the name already.".into());
    }
    Ok(Edit::Settings(SettingsChange {
        name: Some(text.to_owned()),
        ..SettingsChange::default()
    }))
}

/// The edit Enter makes on the password line: a new password, or (empty)
/// taking the password away.
pub fn apply_password(ctx: &Context, text: &str) -> Result<Edit, String> {
    row_state(Kind::Password, ctx)?;
    let change = if text.is_empty() {
        if !ctx.password_set() {
            return Err("No password is set. Type one and press Enter.".into());
        }
        PasswordChange::Clear
    } else {
        PasswordChange::Set(text.to_owned())
    };
    Ok(Edit::Settings(SettingsChange {
        password: Some(change),
        ..SettingsChange::default()
    }))
}

/// What the panel answers to an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    None,
    Close,
    Edit(Edit),
}

/// Where a row is drawn and hit: its whole rectangle, its label's place and
/// its value box. The Realism page has two columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub row: Rect,
    pub label: Point,
    pub label_width: i32,
    pub value: Rect,
}

/// The place of row `index` of `page`.
pub fn place(page: Page, index: usize) -> Place {
    if page == Page::Realism {
        let (column, line) = (index / 8, index % 8);
        let x = PANEL.0 + 22 + column as i32 * 262;
        let y = ROWS_TOP + line as i32 * PITCH;
        Place {
            row: (x - 4, y - 1, 256, ROW_HEIGHT + 2),
            label: (x, y + 3),
            label_width: 160,
            value: (x + 164, y, 88, ROW_HEIGHT),
        }
    } else {
        let x = PANEL.0 + 24;
        let (pitch, row_height) = if page == Page::Game {
            (GAME_PITCH, GAME_PITCH)
        } else {
            (PITCH, ROW_HEIGHT + 2)
        };
        let y = ROWS_TOP + index as i32 * pitch;
        Place {
            row: (x - 4, y - 1, 510, row_height),
            label: (x, y + 3),
            label_width: 200,
            value: (x + 226, y, 270, ROW_HEIGHT),
        }
    }
}

/// The panel.
pub struct SettingsPanel {
    page: Page,
    selected: usize,
    tabs: [Button; 4],
    close: Button,
    name: TextField,
    password: TextField,
    /// The name the field was last filled from, so a change by the host
    /// reaches a field the King is not typing in.
    shown_name: String,
    notice: Option<String>,
    ctx: Context,
}

impl SettingsPanel {
    /// Opens on the Game page.
    pub fn new(ctx: Context) -> Self {
        let tab = |i: usize| {
            Button::new(
                Page::ALL[i].label(),
                (PANEL.0 + 22 + 92 * i as i32, PANEL.1 + 32),
                88,
            )
        };
        let mut panel = Self {
            page: Page::Game,
            selected: 0,
            tabs: [tab(0), tab(1), tab(2), tab(3)],
            close: Button::new("Close", (PANEL.0 + PANEL.2 / 2 - 42, PANEL.1 + 322), 85),
            name: TextField::line(
                (place(Page::Game, 0).value.0, place(Page::Game, 0).value.1),
                270,
                Filter::Text,
            )
            .with_max(NAME_BYTES),
            password: TextField::line(
                (place(Page::Game, 5).value.0, place(Page::Game, 5).value.1),
                270,
                Filter::Text,
            )
            .masked()
            .with_hint("type a password and press Enter"),
            shown_name: ctx.name.clone(),
            notice: None,
            ctx,
        };
        panel.name.set_text(&panel.ctx.name.clone());
        panel.refresh();
        panel
    }

    /// The lobby's state changed: the rows follow it.
    pub fn set_context(&mut self, ctx: Context) {
        if ctx != self.ctx {
            self.ctx = ctx;
            self.refresh();
        }
    }

    fn refresh(&mut self) {
        if self.ctx.name != self.shown_name {
            // The host changed the name: show it unless the King is typing.
            if !(self.page == Page::Game && self.selected == 0 && self.ctx.king) {
                self.name.set_text(&self.ctx.name.clone());
            }
            self.shown_name = self.ctx.name.clone();
        }
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            tab.set_default(Page::ALL[i] == self.page);
        }
    }

    /// The host refused a change: its words go at the foot of the panel.
    pub fn refused(&mut self, reason: &str) {
        self.notice = Some(reason.to_owned());
    }

    fn rows(&self) -> Vec<Kind> {
        page_rows(self.page)
    }

    fn kind(&self) -> Option<Kind> {
        self.rows().get(self.selected).copied()
    }

    /// Typed text goes to a line: the name or the password, selected and the
    /// King's.
    pub fn typing(&self) -> bool {
        self.ctx.king && matches!(self.kind(), Some(Kind::Name | Kind::Password))
    }

    pub fn text_input(&mut self, text: &str) {
        match self.kind() {
            Some(Kind::Name) if self.typing() => {
                self.name.text_input(text);
            }
            Some(Kind::Password) if self.typing() => {
                self.password.text_input(text);
            }
            _ => {}
        }
    }

    /// Shows `page` (the snapshots and tests turn pages this way; the player
    /// clicks a tab or presses Tab).
    pub fn show_page(&mut self, page: Page) {
        self.go_to(page);
    }

    fn go_to(&mut self, page: Page) {
        self.page = page;
        self.selected = 0;
        self.notice = None;
        self.refresh();
    }

    fn select(&mut self, index: usize) {
        let n = self.rows().len();
        self.selected = index.min(n.saturating_sub(1));
    }

    /// What a turn of row `index` answers, and the reason when it is greyed.
    fn turn_row(&mut self, kind: Kind, forward: bool) -> Answer {
        match row_state(kind, &self.ctx) {
            Err(why) => {
                self.notice = Some(why);
                Answer::None
            }
            Ok(()) => {
                self.notice = None;
                match click(kind, &self.ctx, forward) {
                    Some(edit) => {
                        if kind == Kind::Host && self.ctx.phase != LobbyPhase::Lobby {
                            self.notice = Some(PIN_IN_FLIGHT.into());
                        }
                        Answer::Edit(edit)
                    }
                    None => Answer::None,
                }
            }
        }
    }

    fn apply_text(&mut self, kind: Kind) -> Answer {
        let result = match kind {
            Kind::Name => apply_name(&self.ctx, self.name.text()),
            Kind::Password => apply_password(&self.ctx, self.password.text()),
            _ => return Answer::None,
        };
        match result {
            Ok(edit) => {
                self.notice = None;
                if kind == Kind::Password {
                    self.password.set_text("");
                }
                Answer::Edit(edit)
            }
            Err(why) => {
                self.notice = Some(why);
                Answer::None
            }
        }
    }

    /// A key press by the window's name for it.
    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        let Some(kind) = self.kind() else {
            return Answer::None;
        };
        let on_text = self.typing();
        match name {
            "Escape" => Answer::Close,
            "Tab" => {
                self.go_to(self.page.step(!shift));
                Answer::None
            }
            "ArrowDown" => {
                self.select((self.selected + 1) % self.rows().len().max(1));
                Answer::None
            }
            "ArrowUp" => {
                let n = self.rows().len().max(1);
                self.select((self.selected + n - 1) % n);
                Answer::None
            }
            "Enter" if on_text => self.apply_text(kind),
            _ if on_text => {
                let field = if kind == Kind::Name {
                    &mut self.name
                } else {
                    &mut self.password
                };
                field.key(name);
                Answer::None
            }
            "ArrowRight" | "Enter" | "Space" | " " => self.turn_row(kind, true),
            "ArrowLeft" => self.turn_row(kind, false),
            _ => Answer::None,
        }
    }

    pub fn moved(&mut self, point: Option<Point>) {
        for tab in &mut self.tabs {
            tab.pointer_move(point);
        }
        self.close.pointer_move(point);
    }

    /// The row under `point`, if any.
    fn row_at(&self, point: Point) -> Option<usize> {
        (0..self.rows().len()).find(|&i| inside(place(self.page, i).row, point))
    }

    /// A mouse button went down or up at `point`. `right` is the right
    /// button, which turns a row back.
    pub fn button(
        &mut self,
        kit: &Kit,
        point: Option<Point>,
        pressed: bool,
        right: bool,
    ) -> Answer {
        let Some(point) = point else {
            if !pressed {
                for tab in &mut self.tabs {
                    tab.release((-1, -1));
                }
                self.close.release((-1, -1));
            }
            return Answer::None;
        };
        if pressed {
            if right {
                return self.row_pressed(kit, point, false);
            }
            for tab in &mut self.tabs {
                tab.press(point);
            }
            self.close.press(point);
            return self.row_pressed(kit, point, true);
        }
        if right {
            return Answer::None;
        }
        let mut chosen = None;
        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if tab.release(point) == Outcome::Activated {
                chosen = Some(Page::ALL[i]);
            }
        }
        if let Some(page) = chosen {
            self.go_to(page);
            return Answer::None;
        }
        if self.close.release(point) == Outcome::Activated {
            return Answer::Close;
        }
        Answer::None
    }

    fn row_pressed(&mut self, kit: &Kit, point: Point, forward: bool) -> Answer {
        let Some(index) = self.row_at(point) else {
            return Answer::None;
        };
        self.select(index);
        let Some(kind) = self.kind() else {
            return Answer::None;
        };
        match kind {
            Kind::Name | Kind::Password => {
                if self.ctx.king {
                    let field = if kind == Kind::Name {
                        &mut self.name
                    } else {
                        &mut self.password
                    };
                    field.press(point, kit);
                } else {
                    self.notice = row_state(kind, &self.ctx).err();
                }
                Answer::None
            }
            _ => self.turn_row(kind, forward),
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        draw_panel(canvas, kit, PANEL);
        let font = kit.sprite("PANELFNT");
        let dim = kit.sprite("PANELFND");
        let title = if self.ctx.king {
            "Settings"
        } else {
            "Settings (the King changes them)"
        };
        ui_text::text(
            canvas,
            kit,
            font,
            title,
            (
                PANEL.0 + (PANEL.2 - text_width(font, title)) / 2,
                PANEL.1 + 7,
            ),
            None,
            None,
        );
        canvas.outline(
            (PANEL.0 + 12, PANEL.1 + 24, PANEL.2 - 24, PANEL.3 - 36),
            LINE,
        );
        for tab in &self.tabs {
            tab.draw(canvas, kit, false);
        }
        for (i, kind) in self.rows().into_iter().enumerate() {
            let at = place(self.page, i);
            let grey = row_state(kind, &self.ctx).is_err();
            let text_font = if grey { dim } else { font };
            if i == self.selected {
                canvas.outline(at.row, SELECTED);
            }
            let label = fit(text_font, row_label(kind), at.label_width);
            ui_text::text(canvas, kit, text_font, &label, at.label, None, None);
            let live_field = self.ctx.king && matches!(kind, Kind::Name | Kind::Password);
            if live_field {
                let field = if kind == Kind::Name {
                    &self.name
                } else {
                    &self.password
                };
                field.draw(canvas, kit, i == self.selected);
                continue;
            }
            let (x, y, w, h) = at.value;
            canvas.rect(at.value, BOX);
            canvas.outline((x - 1, y - 1, w + 2, h + 2), LINE);
            let value = fit(text_font, &row_value(kind, &self.ctx), w - 12);
            ui_text::text(canvas, kit, text_font, &value, (x + 6, y + 3), None, None);
        }
        let foot = self.notice.clone().or_else(|| {
            self.kind()
                .and_then(|kind| row_state(kind, &self.ctx).err())
        });
        if let Some(foot) = foot {
            ui_text::text(
                canvas,
                kit,
                dim,
                &fit(dim, &foot, PANEL.2 - 48),
                (PANEL.0 + 24, PANEL.1 + 300),
                None,
                None,
            );
        }
        self.close.draw(canvas, kit, false);
    }
}

#[cfg(test)]
impl SettingsPanel {
    pub fn page(&self) -> Page {
        self.page
    }
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
    pub fn context(&self) -> &Context {
        &self.ctx
    }
    pub fn selected(&self) -> usize {
        self.selected
    }
    pub fn name_text(&self) -> &str {
        self.name.text()
    }
    pub fn tab_default(&self, page: Page) -> bool {
        self.tabs[page.index()].is_default()
    }
}
