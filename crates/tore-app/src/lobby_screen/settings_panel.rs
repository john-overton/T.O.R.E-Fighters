//! The Settings... panel of the lobby screen (stage F phase 2, slice F2-L):
//! the King's settings ([`tore_session::settings`]) as rows of text buttons,
//! on four pages (Game, Revival, Scoring, Realism). Every player can open
//! it; for anyone but the King every row is greyed, as retail greys a
//! client's settings, and so is a row that does not apply (scoring in
//! co-op, a lobby setting while the mission flies).
//!
//! A left click on a row turns it to its next value and a right click to the
//! one before (the Quick Mission creator's text buttons); the keyboard does
//! the same with Right and Left. Changes remain in a local draft across tabs.
//! Close or F10 sends one SettingsChange, including Realism; Escape discards.
//! Enter stages name and password text. The host validates the combined edit
//! and rebuilds the mission once at most. Realism stays fixed during flight.
//!
//! Everything about which row is shown, greyed and what a click does lives in
//! plain functions of a [`Context`] (tested without a window); the
//! [`SettingsPanel`] only keeps the page, the selected row, the two text
//! fields and the buttons, and draws.
use crate::menu::{Canvas, text_width};
use crate::ui_text;
use crate::widgets::{
    ArrowBar, ArrowRows, Button, Filter, Kit, Outcome, Point, Rect, RowPress, TextField,
    draw_panel, fit, inside,
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
/// The row window ends at the foot line. A page taller than this scrolls.
const VIEW_BOTTOM: i32 = PANEL.1 + 300;
/// The map's arrow bar, inside the panel's right edge.
const BAR_X: i32 = PANEL.0 + PANEL.2 - 36;
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
/// A dedicated server keeps its operator's minimum while settings are locked.
const MIN_IS_THE_OPERATORS: &str =
    "The server operator sets the minimum players (king-mission locked).";
/// Why the AI respawn row is greyed: with revival `none` nothing comes back.
pub const AI_RESPAWN_NEEDS_REVIVAL: &str = "With Revival none, no aircraft comes back.";
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
    EasyPhysics,
}

impl Flag {
    pub const ALL: [Flag; 14] = [
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
        Flag::EasyPhysics,
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
            Flag::EasyPhysics => "Easy flight physics",
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
            Flag::EasyPhysics => &mut cheats.easy_physics,
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
            Kind::Setting(number::AI),
            Kind::Setting(number::MIN_PLAYERS),
            Kind::Setting(number::RESPAWN),
            Kind::Setting(number::LIVES),
            Kind::Setting(number::REVIVE_DELAY),
            Kind::Setting(number::REVIVE_DISTANCE),
            Kind::Setting(number::REVIVE_WEAPONS),
            Kind::Setting(number::AI_RESPAWN),
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
            number::LOCK_SIDES => "Sides",
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
            number::MIN_PLAYERS => "Minimum players",
            number::AI_RESPAWN => "AI respawn",
            number::AI => "AI aircraft",
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
    pub mission_locked: bool,
    /// Open human seats, before the player limit is applied.
    pub open_slots: u32,
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
            mission_locked: lobby.mission_locked,
            open_slots: lobby
                .slots
                .iter()
                .filter(|s| s.lock != tore_session::wire::messages::Lock::Closed)
                .count() as u32,
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

    fn capacity(&self) -> u32 {
        self.open_slots
            .min(self.value(number::MAX_PLAYERS).unwrap_or(0))
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
    if kind == Kind::Setting(number::MIN_PLAYERS) && ctx.mission_locked {
        return Err(MIN_IS_THE_OPERATORS.into());
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
    if matches!(kind, Kind::Setting(number::AI_RESPAWN | number::IDLE_AI))
        && ctx.value(number::AI) == Some(0)
    {
        return Err("AI is off: AI respawn and idle handoff have no effect.".into());
    }
    // AI respawn follows the revival rule: greyed while nothing revives
    // (John, 2026-10-09).
    if kind == Kind::Setting(number::AI_RESPAWN)
        && ctx.value(number::RESPAWN) == Some(settings::Respawn::None.value())
    {
        return Err(AI_RESPAWN_NEEDS_REVIVAL.into());
    }
    Ok(())
}

/// A row's value in words.
pub fn row_value(kind: Kind, ctx: &Context) -> String {
    match kind {
        // Lobby pass L3: the sides rule in the words a player reads, not the
        // configuration file's (`lock-sides off|on|balanced`).
        Kind::Setting(number::LOCK_SIDES) => match ctx.value(number::LOCK_SIDES) {
            Some(0) => "Free".into(),
            Some(1) => "Locked once flown".into(),
            Some(2) => "Balanced by the host".into(),
            _ => "...".into(),
        },
        Kind::Setting(number::MIN_PLAYERS) => match ctx.value(number::MIN_PLAYERS) {
            Some(value) => settings::setting(number::MIN_PLAYERS)
                .unwrap()
                .text(value.min(ctx.capacity())),
            None => "...".into(),
        },
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
            let values = if n == number::MIN_PLAYERS {
                (0..=ctx.capacity()).collect()
            } else {
                choices(setting)
            };
            let next = turn(&values, now, forward)?;
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
    Apply(SettingsChange),
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
            row: (x - 4, y - 1, 230, ROW_HEIGHT + 2),
            label: (x, y + 3),
            label_width: 140,
            value: (x + 144, y, 80, ROW_HEIGHT),
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
            row: (x - 4, y - 1, 490, row_height),
            label: (x, y + 3),
            label_width: 200,
            value: (x + 226, y, 250, ROW_HEIGHT),
        }
    }
}

/// How many rows of `page` fit above the foot line.
pub fn window_rows(page: Page) -> usize {
    if page == Page::Realism {
        // Two columns of eight: the page fits, and still has the bar.
        page_rows(page).len()
    } else {
        let pitch = if page == Page::Game {
            GAME_PITCH
        } else {
            PITCH
        };
        ((VIEW_BOTTOM - ROWS_TOP) / pitch).max(1) as usize
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
    original: Context,
    password_change: Option<PasswordChange>,
    /// The first row of the window. The host row scrolls in when the page
    /// is taller than the window.
    first: usize,
    bar: ArrowBar,
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
                250,
                Filter::Text,
            )
            .with_max(NAME_BYTES),
            password: TextField::line(
                (place(Page::Game, 5).value.0, place(Page::Game, 5).value.1),
                250,
                Filter::Text,
            )
            .masked()
            .with_hint("type a password and press Enter"),
            shown_name: ctx.name.clone(),
            notice: None,
            original: ctx.clone(),
            password_change: None,
            ctx,
            first: 0,
            bar: ArrowBar::vertical((BAR_X, ROWS_TOP), VIEW_BOTTOM - ROWS_TOP),
        };
        panel.name.set_text(&panel.ctx.name.clone());
        panel.refresh();
        panel
    }

    /// Keep local edits across updates; discard when permissions or phase change.
    pub fn set_context(&mut self, ctx: Context) {
        // Merge fresh authoritative values into untouched fields, preserving
        // edited values. This also enables Realism if the mission arrives
        // after the panel opened. Losing permission discards the draft.
        if ctx.king != self.ctx.king
            || ctx.phase != self.ctx.phase
            || ctx.mission_locked != self.ctx.mission_locked
        {
            self.ctx = ctx.clone();
            self.password_change = None;
            self.name.set_text(&ctx.name);
            self.password.set_text("");
        } else {
            for &(number, value) in &ctx.settings {
                if self.ctx.value(number) == self.original.value(number)
                    && let Some((_, draft)) =
                        self.ctx.settings.iter_mut().find(|(n, _)| *n == number)
                {
                    *draft = value;
                }
            }
            if self.ctx.cheats == self.original.cheats {
                self.ctx.cheats = ctx.cheats;
            }
            if self.ctx.name == self.original.name {
                self.ctx.name.clone_from(&ctx.name);
            }
            self.ctx.players.clone_from(&ctx.players);
            self.ctx.open_slots = ctx.open_slots;
        }
        self.original = ctx;
        self.refresh();
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
        self.first = 0;
        self.notice = None;
        self.refresh();
        self.sync_fields();
    }

    fn window(&self) -> usize {
        window_rows(self.page).min(self.rows().len().max(1))
    }

    fn max_first(&self) -> usize {
        self.rows().len().saturating_sub(self.window())
    }

    /// Where row `index` is drawn now, or none when it is scrolled away.
    fn drawn_place(&self, index: usize) -> Option<Place> {
        if index < self.first || index >= self.first + self.window() {
            return None;
        }
        let mut at = place(self.page, index);
        if self.page != Page::Realism {
            let pitch = if self.page == Page::Game {
                GAME_PITCH
            } else {
                PITCH
            };
            let shift = self.first as i32 * pitch;
            at.row.1 -= shift;
            at.label.1 -= shift;
            at.value.1 -= shift;
        }
        Some(at)
    }

    fn scroll_to(&mut self, first: usize) {
        self.first = first.min(self.max_first());
        self.sync_fields();
    }

    fn scroll_by(&mut self, rows: i32) {
        let next = (self.first as i64 + i64::from(rows)).clamp(0, self.max_first() as i64);
        self.scroll_to(next as usize);
    }

    fn reveal(&mut self) {
        if self.selected == self.rows().len() {
            return;
        }
        let window = self.window();
        if self.selected < self.first {
            self.first = self.selected;
        } else if self.selected >= self.first + window {
            self.first = self.selected + 1 - window;
        }
        self.first = self.first.min(self.max_first());
        self.sync_fields();
    }

    fn sync_fields(&mut self) {
        let name = self.rows().iter().position(|k| *k == Kind::Name);
        let password = self.rows().iter().position(|k| *k == Kind::Password);
        if let Some(index) = name {
            let at = self
                .drawn_place(index)
                .map(|p| (p.value.0, p.value.1))
                .unwrap_or((-400, -400));
            self.name.move_to(at);
        }
        if let Some(index) = password {
            let at = self
                .drawn_place(index)
                .map(|p| (p.value.0, p.value.1))
                .unwrap_or((-400, -400));
            self.password.move_to(at);
        }
    }

    fn bar_rows(&self) -> ArrowRows {
        ArrowRows {
            total: self.rows().len(),
            visible: self.window(),
            first: self.first,
        }
    }

    /// The wheel over the panel: positive goes up, toward the first row.
    pub fn wheel(&mut self, notches: i32) {
        if notches != 0 {
            self.scroll_by(-notches);
        }
    }

    fn select(&mut self, index: usize) {
        let n = self.rows().len();
        self.selected = index.min(n);
        self.reveal();
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
                        self.stage(edit);
                        Answer::None
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
                self.stage(edit);
                Answer::None
            }
            Err(why) => {
                self.notice = Some(why);
                Answer::None
            }
        }
    }

    fn stage(&mut self, edit: Edit) {
        match edit {
            Edit::Cheats(cheats) => self.ctx.cheats = Some(cheats),
            Edit::Settings(change) => {
                for (n, value) in change.values {
                    if n == number::MODE && self.ctx.value(n) != Some(value) {
                        let mode = settings::Mode::from_value(value).expect("row choice");
                        for (id, current) in &mut self.ctx.settings {
                            if let Some(setting) = settings::setting(*id)
                                && setting.change == Change::InLobby
                                && !matches!(*id, number::SNAPSHOT_RATE | number::MIN_PLAYERS)
                            {
                                *current = setting.default_in(mode);
                            }
                        }
                    }
                    if let Some((_, current)) =
                        self.ctx.settings.iter_mut().find(|(id, _)| *id == n)
                    {
                        *current = value;
                    }
                }
                if let Some(name) = change.name {
                    self.ctx.name = name;
                }
                if let Some(password) = change.password {
                    if let Some((_, value)) = self
                        .ctx
                        .settings
                        .iter_mut()
                        .find(|(n, _)| *n == number::PASSWORD)
                    {
                        *value = u32::from(matches!(password, PasswordChange::Set(_)));
                    }
                    self.password_change = Some(password);
                }
            }
        }
        self.refresh();
    }

    fn apply(&mut self) -> Answer {
        let change = SettingsChange {
            values: self
                .ctx
                .settings
                .iter()
                .copied()
                .filter(|(n, v)| {
                    *n != number::PASSWORD
                        && self.original.value(*n) != Some(*v)
                        && settings::setting(*n).is_none_or(|s| !s.pvp_only || self.ctx.pvp())
                })
                .collect(),
            name: (self.ctx.name != self.original.name).then(|| self.ctx.name.clone()),
            password: self.password_change.take(),
            cheats: (self.ctx.cheats != self.original.cheats)
                .then_some(self.ctx.cheats)
                .flatten(),
        };
        if change == SettingsChange::default() {
            Answer::Close
        } else {
            Answer::Apply(change)
        }
    }

    /// A key press by the window's name for it.
    pub fn key(&mut self, name: &str, shift: bool) -> Answer {
        let Some(kind) = self.kind() else {
            return match name {
                "Escape" => Answer::Close,
                "F10" | "Enter" | "Space" | " " => self.apply(),
                "ArrowUp" => {
                    self.select(self.rows().len().saturating_sub(1));
                    Answer::None
                }
                "ArrowDown" => {
                    self.select(0);
                    Answer::None
                }
                "Tab" => {
                    self.go_to(self.page.step(!shift));
                    Answer::None
                }
                _ => Answer::None,
            };
        };
        let on_text = self.typing();
        match name {
            "Escape" => Answer::Close,
            "F10" => self.apply(),
            "Tab" => {
                self.go_to(self.page.step(!shift));
                Answer::None
            }
            "ArrowDown" => {
                self.select(self.selected + 1);
                Answer::None
            }
            "ArrowUp" => {
                let n = self.rows().len();
                self.select(if self.selected == 0 {
                    n
                } else {
                    self.selected - 1
                });
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
        if let Some(point) = point
            && let Some(first) = self.bar.drag_rows(point, self.bar_rows())
        {
            self.scroll_to(first);
        }
    }

    /// The row under `point`, if any.
    fn row_at(&self, point: Point) -> Option<usize> {
        (0..self.rows().len())
            .find(|&i| self.drawn_place(i).is_some_and(|at| inside(at.row, point)))
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
            match self.bar.press_rows(point, self.bar_rows()) {
                RowPress::Miss => {}
                RowPress::Less => {
                    self.scroll_by(-1);
                    return Answer::None;
                }
                RowPress::More => {
                    self.scroll_by(1);
                    return Answer::None;
                }
                RowPress::PageLess => {
                    self.scroll_by(-(self.window() as i32));
                    return Answer::None;
                }
                RowPress::PageMore => {
                    self.scroll_by(self.window() as i32);
                    return Answer::None;
                }
                RowPress::Grab => return Answer::None,
            }
            if right {
                return self.row_pressed(kit, point, false);
            }
            for tab in &mut self.tabs {
                tab.press(point);
            }
            self.close.press(point);
            return self.row_pressed(kit, point, true);
        }
        self.bar.release();
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
            return self.apply();
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
            let Some(at) = self.drawn_place(i) else {
                continue;
            };
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
        let foot = Some(foot.unwrap_or_else(|| "Close / F10 applies; Escape cancels.".into()));
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
        self.close
            .draw(canvas, kit, self.selected == self.rows().len());
        self.bar.draw_rows(canvas, self.bar_rows());
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

#[cfg(test)]
mod bar_layout {
    use super::*;

    #[test]
    fn the_bar_sits_inside_the_panel_and_clear_of_the_rows() {
        let bar = (BAR_X, ROWS_TOP, 16, VIEW_BOTTOM - ROWS_TOP);
        let (x, y, w, h) = bar;
        assert!(x >= PANEL.0 + 12 && y >= PANEL.1 + 24);
        assert!(x + w <= PANEL.0 + PANEL.2 - 12);
        assert!(y + h <= PANEL.1 + 300);
        for page in Page::ALL {
            for i in 0..page_rows(page).len() {
                let row = place(page, i).row;
                let apart =
                    row.0 + row.2 <= x || x + w <= row.0 || row.1 + row.3 <= y || y + h <= row.1;
                assert!(apart, "{page:?} row {i} overlaps the bar");
            }
        }
    }
}
