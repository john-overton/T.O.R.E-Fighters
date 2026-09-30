//! Post-mission debrief over the retail clipboard art. The BRIEFSCR.DLG
//! controls, page text fonts and mission text are imported; page layout is
//! fitted to John's retail screenshots. Spec: docs/spec/debrief.md.
use crate::ai_wings::outcome::{self, Requirements, Standing};
use crate::rocker::Rocker;
use crate::{
    AppResult,
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    seats::{PlaneId, Roster, SeatId},
    world::World,
};
use std::{collections::BTreeMap, time::Instant};
use tore_formats::text::GlyphCodes;
use tore_formats::{Pic, mission_text::MissionText};
use tore_sim::combat::ledger::{Kill, Ledger, ShotKind, Tally};

type Rect = (i32, i32, i32, i32);

/// Debrief resources beyond the shared menu art, by archive.
pub const ART: &[&str] = &[
    "DEBSCR.PIC",
    "DEBSC3.PIC",
    "DEBSCU.PIC",
    "DEBSCV.PIC",
    "PANLFNT2.PIC",
    "PANELFNT.PIC",
    "BODYFONT.PIC",
    "BOLDFONT.PIC",
    "HEADFONT.PIC",
];
pub const DATA: &[&str] = &["BRIEFSCR.DLG", "QUICK.MT", "&ROCKUP.11K", "&ROCKDN.11K"];

const OK: usize = 1;
const PREVIOUS: usize = 3;
const NEXT: usize = 4;
const HELP: usize = 5;
const EXIT: usize = 6;
const CLIPBOARD: usize = 7;
const BACKGROUNDS: [&str; 4] = ["DEBSCR.PIC", "DEBSC3.PIC", "DEBSCU.PIC", "DEBSCV.PIC"];

/// Clipboard text column: left margin, centering axis and tab stops.
const LEFT: i32 = 294;
const CENTER: i32 = 418;
const TABS: [i32; 2] = [403, 478];
const TOP: i32 = 162;
const LINE: i32 = 12;
const HEADER_LINE: i32 = 15;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    Success,
    #[default]
    Failure,
}
impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failure => "FAILURE",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Alive,
    Ejected,
    Dead,
}

/// Kill table rows, in retail order.
pub const KILL_ROWS: [&str; 10] = [
    "Fighter",
    "Bomber",
    "Helicopter",
    "Ship",
    "SAM",
    "AAA",
    "Tank",
    "Vehicle",
    "Structure",
    "Other",
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pilot {
    pub status: Status,
    /// Airframe damage, 0 to 1; a dead or ejected pilot shows 100%.
    pub damage: f64,
    /// Average landing score as a percentage.
    pub landing_grade: Option<u32>,
    /// `overspeed` or `out of bounds` when the airframe was lost that way.
    pub cause: Option<&'static str>,
    pub kills: [u32; 10],
    pub friendly_fire: u32,
    pub air_to_air: Tally,
    pub air_to_ground: Tally,
    pub gun: Tally,
    pub bombs: Tally,
    /// Enemy fire aimed at this pilot. SAM and AAA sites do not exist yet.
    pub enemy_aam: Tally,
    pub enemy_sam: Tally,
    pub enemy_gun: Tally,
    pub enemy_aaa: Tally,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    Destroy { destroyed: u32, total: u32 },
    Protect { protected: u32, total: u32 },
}
impl Objective {
    fn sentence(self) -> String {
        match self {
            Self::Destroy { destroyed, total } => match (destroyed, total) {
                (1, 1) => "Destroyed the target.".into(),
                (0, 1) => "Failed to destroy the target.".into(),
                (d, t) if d == t => format!("Destroyed the {t} targets."),
                (d, t) => format!("Destroyed {d} of {t} targets."),
            },
            Self::Protect { protected, total } => match (protected, total) {
                (1, 1) => "Protected the friendly objective.".into(),
                (0, 1) => "Failed to protect the friendly objective.".into(),
                (p, t) if p == t => format!("Protected the {t} friendly objectives."),
                (p, t) => format!("Protected {p} of {t} friendly objectives."),
            },
        }
    }
}

/// Everything the five pages show, captured when the mission ends, for one
/// seat.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub outcome: Outcome,
    pub objectives: Vec<Objective>,
    pub elapsed_seconds: u64,
    /// The pilot column: the plane the seat flies. The page calls it the
    /// player.
    pub player: Pilot,
    /// The one tracked wingman, the first other member of the seat's wing;
    /// `None` when the seat's plane flew alone.
    pub wingman: Option<Pilot>,
}

/// One aircraft as the mission ends. `id` is the plane's id, and `friendly`
/// means on the side of the plane whose debrief this is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airframe {
    pub id: u32,
    pub friendly: bool,
    /// Still flying with its pilot aboard.
    pub alive: bool,
    /// The pilot escaped; a dead pilot is never ejected.
    pub ejected: bool,
    /// Airframe damage, 0 to 1.
    pub damage: f64,
    pub landing_grade: Option<u32>,
    /// Lost to overspeed or to leaving the map rather than to combat, a crash
    /// or the pilot (requested by John, 2026-09-29).
    pub cause: Option<tore_sim::aircraft_systems::LossCause>,
}

/// Host facts at mission end, gathered before the flight is torn down.
pub struct Ending<'a> {
    pub ledger: &'a Ledger,
    pub ticks: u64,
    /// The plane whose debrief this is: the pilot column.
    pub player: Airframe,
    /// Every other aircraft, both sides: the AI's and the other human-flown
    /// planes.
    pub aircraft: Vec<Airframe>,
    /// The aircraft shown in the WINGMAN column, if the plane had one.
    pub wingman: Option<u32>,
    /// What the mission asks of the plane.
    pub requirements: Requirements,
}

impl Ending<'_> {
    fn airframe(&self, id: u32) -> Option<&Airframe> {
        if id == self.player.id {
            Some(&self.player)
        } else {
            self.aircraft.iter().find(|a| a.id == id)
        }
    }
    fn hostile(&self, id: u32) -> bool {
        self.airframe(id).is_some_and(|a| !a.friendly)
    }
    /// The mission result rule, shared with the in-flight result check.
    fn fates(&self) -> Vec<outcome::Aircraft> {
        std::iter::once(&self.player)
            .chain(&self.aircraft)
            .map(|a| outcome::Aircraft {
                id: a.id,
                friendly: a.friendly,
                alive: a.alive,
            })
            .collect()
    }
    fn pilot(&self, airframe: &Airframe, standing: &Standing, kills: &[Kill]) -> Pilot {
        let id = airframe.id;
        let own = |kind: ShotKind| self.ledger.total(|k| k.owner == id && k.kind == kind);
        // Enemy aircraft fire splits only into guns and everything else.
        let at = |gun: bool| {
            self.ledger.total(|k| {
                k.aim == Some(id) && (k.kind == ShotKind::Gun) == gun && self.hostile(k.owner)
            })
        };
        let status = if airframe.ejected {
            Status::Ejected
        } else if airframe.alive {
            Status::Alive
        } else {
            Status::Dead
        };
        let mut pilot = Pilot {
            status,
            damage: if status == Status::Alive {
                airframe.damage
            } else {
                1.
            },
            landing_grade: airframe.landing_grade,
            cause: airframe.cause.map(|c| c.label()),
            air_to_air: own(ShotKind::AirToAir),
            air_to_ground: own(ShotKind::AirToGround),
            gun: own(ShotKind::Gun),
            bombs: own(ShotKind::Bomb),
            enemy_aam: at(false),
            enemy_gun: at(true),
            ..Pilot::default()
        };
        for kill in kills.iter().filter(|k| k.owner == id) {
            if standing.friendly(kill.victim) {
                pilot.friendly_fire += 1;
            } else if let Some(row) = kill_row(kill.category) {
                pilot.kills[row] += 1;
            }
        }
        pilot
    }
}

/// The kill row for a victim's object class word: the first matching bit of
/// 0x8000 fighter, 0x4000 bomber, 0x2000 ship, 0x1000 SAM, 0x800 AAA,
/// 0x400 tank, 0x200 vehicle, 0x100 structure and 0x40 other. Retail puts
/// helicopters first by a PT flag; no supported aircraft is a helicopter yet.
pub fn kill_row(category: u16) -> Option<usize> {
    const BITS: [(u16, usize); 9] = [
        (0x8000, 0),
        (0x4000, 1),
        (0x2000, 3),
        (0x1000, 4),
        (0x800, 5),
        (0x400, 6),
        (0x200, 7),
        (0x100, 8),
        (0x40, 9),
    ];
    BITS.iter()
        .find(|(bit, _)| category & bit != 0)
        .map(|(_, row)| *row)
}

/// The aircraft in the wingman column for the plane `plane`: the first other
/// member of its wing, human-flown or not.
pub fn wingman_of(roster: &Roster, plane: PlaneId) -> Option<u32> {
    let wing = roster.plane(plane)?.slot.wing;
    roster
        .planes()
        .iter()
        .filter(|p| p.id != plane && p.slot.wing == wing)
        .min_by_key(|p| p.slot.member)
        .map(|p| p.id.0)
}

/// A human-flown plane as the mission ends, from its cockpit's flight and its
/// ownship's hit points.
pub fn cockpit_airframe(
    id: u32,
    friendly: bool,
    flight: &crate::flight::State,
    hp: i32,
) -> Airframe {
    let pilot = &flight.systems.pilot;
    Airframe {
        id,
        friendly,
        alive: !flight.crashed && hp > 0 && !pilot.dead && !pilot.ejected,
        ejected: pilot.ejected && !pilot.dead,
        damage: flight.damage_fraction,
        landing_grade: flight.research.as_ref().and_then(|r| r.landings.grade()),
        cause: flight.systems.structure.cause,
    }
}

/// Reads the mission's results for `seat` from the world: the pilot column is
/// the plane the seat flies, the wingman column the first other member of its
/// wing, the objectives are that plane's and friendly fire counts the kills it
/// made. `None` when the seat flies no plane. Call before `Action::Back` drops
/// the AI wings.
pub fn capture(world: &World, seat: SeatId) -> Option<Report> {
    let plane = world.roster.seat(seat)?.plane?;
    let side = world.roster.plane(plane)?.slot.wing.side;
    let state = &world.combat.state;
    let fate = |cockpit: &crate::world::Cockpit| {
        let id = cockpit.plane.0;
        let hp = state.ownship(id).map_or(0, |own| own.hp);
        let friendly = world
            .roster
            .plane(cockpit.plane)
            .is_some_and(|p| p.slot.wing.side == side);
        cockpit_airframe(id, friendly, &cockpit.flight, hp)
    };
    let player = fate(world.cockpits.iter().find(|c| c.plane == plane)?);
    // Every other human-flown plane, then the AI's aircraft.
    let mut aircraft: Vec<Airframe> = world
        .cockpits
        .iter()
        .filter(|c| c.plane != plane)
        .map(fate)
        .collect();
    let mut requirements = Requirements::default();
    if let Some(wings) = world.ai_wings.as_ref() {
        let mission = wings.mission();
        for slot in wings.slots() {
            if aircraft.iter().any(|a| a.id == slot.id) {
                continue;
            }
            let actor = mission.actor(slot.id);
            aircraft.push(Airframe {
                id: slot.id,
                friendly: slot.side == side,
                alive: actor.is_some_and(|a| a.alive() && a.flight().escape.is_none()),
                ejected: actor.is_some_and(|a| a.flight().escape.is_some()),
                damage: actor.map_or(1., |a| a.flight().damage_fraction),
                // AI aircraft do not land yet.
                landing_grade: None,
                cause: actor.and_then(|a| a.flight().systems.structure.cause),
            });
        }
        requirements = Requirements::of(wings, plane.0, side);
    }
    Some(report(&Ending {
        ledger: &state.ledger,
        ticks: state.tick(),
        player,
        aircraft,
        wingman: wingman_of(&world.roster, plane),
        requirements,
    }))
}

pub fn report(end: &Ending) -> Report {
    let fates = end.fates();
    let standing = Standing {
        ledger: end.ledger,
        plane: end.player.id,
        aircraft: &fates,
        requirements: &end.requirements,
    };
    let kills = standing.kills();
    let mut objectives = Vec::new();
    if !end.requirements.destroy.is_empty() {
        objectives.push(Objective::Destroy {
            destroyed: standing.destroyed(),
            total: end.requirements.destroy.len() as u32,
        });
    }
    if !end.requirements.protect.is_empty() {
        objectives.push(Objective::Protect {
            protected: standing.protected(),
            total: end.requirements.protect.len() as u32,
        });
    }
    let player = end.pilot(&end.player, &standing, &kills);
    Report {
        outcome: if standing.succeeded() {
            Outcome::Success
        } else {
            Outcome::Failure
        },
        objectives,
        elapsed_seconds: end.ticks / 120,
        player,
        wingman: end
            .wingman
            .and_then(|id| end.airframe(id))
            .map(|a| end.pilot(a, &standing, &kills)),
    }
}

impl Report {
    /// One line for headless probes: outcome, objectives and both columns.
    pub fn summary(&self) -> String {
        let pilot = |p: &Pilot| {
            format!(
                "{:?} damage={:.0}% kills={:?} ff={} a2a={}/{} dmg={} gun={}/{} enemy_aam={}/{} enemy_gun={}/{}{}",
                p.status,
                p.damage * 100.,
                p.kills,
                p.friendly_fire,
                p.air_to_air.hit,
                p.air_to_air.launched,
                p.air_to_air.damage,
                p.gun.hit,
                p.gun.launched,
                p.enemy_aam.hit,
                p.enemy_aam.launched,
                p.enemy_gun.hit,
                p.enemy_gun.launched,
                p.cause.map_or_else(String::new, |c| format!(" cause={c}")),
            )
        };
        format!(
            "{} {:?} elapsed={}s player[{}] wingman[{}]",
            self.outcome.label(),
            self.objectives,
            self.elapsed_seconds,
            pilot(&self.player),
            self.wingman.as_ref().map_or_else(|| "-".into(), pilot),
        )
    }
    /// The retail reference case: a failed Quick Mission flown alone for one
    /// second with nothing fired. Used by `--snapshot-state debrief-N`.
    pub fn sample() -> Self {
        Self {
            outcome: Outcome::Failure,
            objectives: vec![Objective::Destroy {
                destroyed: 0,
                total: 3,
            }],
            elapsed_seconds: 1,
            player: Pilot::default(),
            wingman: None,
        }
    }
}

fn count(value: u32) -> String {
    if value == 0 {
        "-".into()
    } else {
        value.to_string()
    }
}
/// Whole percent, rounded down, or "-" with nothing to divide by.
fn percent(part: u32, whole: u32) -> String {
    if whole == 0 {
        "-".into()
    } else {
        format!("{}%", u64::from(part.min(whole)) * 100 / u64::from(whole))
    }
}
/// Hit rate, followed by the damage those hits did when there was any.
fn hit(t: Tally) -> String {
    let rate = percent(t.hit, t.launched);
    if t.damage == 0 {
        rate
    } else {
        format!("{rate} ({})", t.damage)
    }
}
fn cell(pilot: Option<&Pilot>, value: impl Fn(&Pilot) -> String) -> String {
    pilot.map(value).unwrap_or_else(|| "-".into())
}

/// Page markup in the mission-text grammar: `.center`, `.left`, `.header`,
/// `.body`, `.bold`/`..bold`, `.underline`/`..underline` and tab columns.
pub fn pages(report: &Report, text: &MissionText) -> Vec<Vec<String>> {
    let success = report.outcome == Outcome::Success;
    let first = text
        .debrief(success)
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    let player = Some(&report.player);
    let wing = report.wingman.as_ref();
    let row = |label: &str, value: &dyn Fn(&Pilot) -> String| {
        format!("{label}\t{}\t{}", cell(player, value), cell(wing, value))
    };
    let heading = |title: &str| {
        vec![
            ".center".into(),
            ".underline".into(),
            ".bold".into(),
            title.to_string(),
            "..underline".into(),
            "..bold".into(),
            ".left".into(),
        ]
    };
    let columns = |first: &str| {
        vec![
            ".bold".into(),
            format!("{first}\tPLAYER\tWINGMAN"),
            "..bold".into(),
        ]
    };
    let group = |title: &str| vec![".bold".to_string(), title.to_string(), "..bold".into()];

    let mut outcome = heading(&format!("MISSION OUTCOME : {}", report.outcome.label()));
    for objective in &report.objectives {
        outcome.extend([String::new(), objective.sentence()]);
    }
    outcome.extend([
        String::new(),
        format!(
            "Elapsed time: {}:{:02}",
            report.elapsed_seconds / 60,
            report.elapsed_seconds % 60
        ),
        String::new(),
    ]);
    outcome.extend(heading("PILOT STATUS"));
    outcome.push(String::new());
    outcome.extend(columns(""));
    outcome.push(row("Status", &|p| {
        match p.status {
            Status::Alive => "Alive",
            Status::Ejected => "Ejected",
            Status::Dead => "Dead",
        }
        .into()
    }));
    // Only when someone was lost to overspeed or the map edge, so an ordinary
    // debrief keeps the retail rows.
    if player.is_some_and(|p| p.cause.is_some()) || wing.is_some_and(|p| p.cause.is_some()) {
        outcome.push(row("Cause", &|p| {
            p.cause.map_or_else(|| "-".into(), |c| c.to_string())
        }));
    }
    outcome.push(row("Damage", &|p| {
        format!("{}%", (p.damage * 100.).clamp(0., 100.) as u32)
    }));
    outcome.push(row("Landing grade", &|p| {
        p.landing_grade
            .map_or_else(|| "-".into(), |grade| format!("{grade}%"))
    }));

    let mut kills = heading("KILLS");
    kills.push(String::new());
    kills.extend(columns(""));
    for (index, label) in KILL_ROWS.iter().enumerate() {
        kills.push(row(label, &|p| count(p.kills[index])));
    }
    kills.push(String::new());
    kills.push(row("Friendly fire", &|p| count(p.friendly_fire)));

    let missiles = |lines: &mut Vec<String>,
                    title: &str,
                    get: &dyn Fn(&Pilot) -> Tally,
                    spoofed_first: bool| {
        lines.extend(group(title));
        lines.push(row("    Launches", &|p| count(get(p).launched)));
        lines.push(row("    Hit", &|p| hit(get(p))));
        lines.push(row("    Failed", &|p| {
            let t = get(p);
            percent(t.failed(), t.launched)
        }));
        let spoofed = row("    Spoofed", &|p| {
            let t = get(p);
            percent(t.spoofed, t.launched)
        });
        let jammed = row("    Jammed", &|p| {
            let t = get(p);
            percent(t.jammed, t.launched)
        });
        if spoofed_first {
            lines.extend([spoofed, jammed]);
        } else {
            lines.extend([jammed, spoofed]);
        }
        lines.push(String::new());
    };
    let shots = |lines: &mut Vec<String>, title: &str, get: &dyn Fn(&Pilot) -> Tally| {
        lines.extend(group(title));
        lines.push(row("    Hit", &|p| hit(get(p))));
    };

    let mut hits = heading("HIT PERCENTAGES");
    hits.push(String::new());
    hits.extend(columns(""));
    missiles(&mut hits, "Air-to-Air", &|p| p.air_to_air, true);
    missiles(&mut hits, "Air-to-Ground", &|p| p.air_to_ground, true);
    shots(&mut hits, "Gun", &|p| p.gun);
    hits.push(String::new());
    shots(&mut hits, "Bomb", &|p| p.bombs);

    let mut enemy = heading("ENEMY HIT PERCENTAGES");
    enemy.push(String::new());
    enemy.extend(columns("ON"));
    missiles(&mut enemy, "AAM", &|p| p.enemy_aam, false);
    missiles(&mut enemy, "SAM", &|p| p.enemy_sam, false);
    shots(&mut enemy, "Gun", &|p| p.enemy_gun);
    enemy.push(String::new());
    shots(&mut enemy, "AAA", &|p| p.enemy_aaa);

    vec![first, outcome, kills, hits, enemy]
}

pub struct Debrief {
    pages: Vec<Vec<String>>,
    pub page: usize,
    sprites: BTreeMap<String, Sprite>,
    background: String,
    hover: Option<usize>,
    pressed: Option<usize>,
    right_pressed: bool,
    help: bool,
    rocker: Rocker,
    controls: Vec<(usize, Rect)>,
}

impl Debrief {
    /// `background` pins one of the four retail backgrounds; otherwise one
    /// is chosen at random, as retail does.
    pub fn new(
        report: Report,
        data: &BTreeMap<String, Vec<u8>>,
        background: Option<&str>,
    ) -> AppResult<Self> {
        let background = background.map_or_else(
            || {
                // Menu-only randomness; never shares state with the simulation.
                use std::hash::BuildHasher;
                let seed = std::collections::hash_map::RandomState::new().hash_one(());
                BACKGROUNDS[seed as usize % BACKGROUNDS.len()].to_string()
            },
            str::to_string,
        );
        let parse = |name: &str| -> AppResult<Pic> {
            Ok(Pic::parse(data.get(name).ok_or_else(|| {
                format!("missing debrief resource {name}; re-import media")
            })?)?)
        };
        let palette: [[u8; 3]; 256] = parse(&background)?
            .palette
            .try_into()
            .map_err(|_| "debrief palette missing")?;
        let mut sprites = BTreeMap::new();
        for name in ART.iter().copied().chain([
            "ROCKER00.PIC",
            "ROCKER01.PIC",
            "ROCKER02.PIC",
            "ROCKER03.PIC",
            "ROCKER04.PIC",
            "MENUFONT.PIC",
            "ACTDFLT.PIC",
            "ACTDFT0L.PIC",
            "ACTDFT0M.PIC",
            "ACTDFT0R.PIC",
            "ACTIOD0L.PIC",
            "ACTIOD0M.PIC",
            "ACTIOD0R.PIC",
            "FONTACT.PIC",
        ]) {
            let p = parse(name)?;
            let mut rgba = p.rgba(&palette);
            // DEBSCR carries 60 blank rows below its 640 by 480 picture.
            let height = if BACKGROUNDS.contains(&name) {
                if p.width != WIDTH || p.height < HEIGHT {
                    return Err(format!("{name}: unexpected debrief background size").into());
                }
                rgba.truncate(WIDTH * HEIGHT * 4);
                HEIGHT
            } else {
                p.height
            };
            sprites.insert(
                name.to_string(),
                Sprite {
                    width: p.width,
                    height,
                    rgba,
                    glyphs: p.glyphs,
                },
            );
        }
        for font in [
            "BODYFONT.PIC",
            "BOLDFONT.PIC",
            "HEADFONT.PIC",
            "PANELFNT.PIC",
            "PANLFNT2.PIC",
        ] {
            if sprites[font].glyphs.len() != 256 {
                return Err(format!("{font}: missing glyph table").into());
            }
        }
        sprites.insert("QUICKFONT".into(), crate::menu::flat_font([232, 233, 230]));
        sprites.insert("QUICKFONTD".into(), crate::menu::flat_font([150, 152, 150]));
        let text = MissionText::parse(
            data.get("QUICK.MT")
                .ok_or("missing debrief resource QUICK.MT; re-import media")?,
        )?;
        Ok(Self {
            pages: pages(&report, &text),
            page: 0,
            sprites,
            background,
            hover: None,
            pressed: None,
            right_pressed: false,
            help: false,
            rocker: Rocker::new(),
            controls: vec![],
        })
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        self.hover = p.and_then(|(x, y)| {
            self.controls
                .iter()
                .rev()
                .find(|(_, r)| {
                    x >= f64::from(r.0)
                        && y >= f64::from(r.1)
                        && x < f64::from(r.0 + r.2)
                        && y < f64::from(r.1 + r.3)
                })
                .map(|(id, _)| *id)
        });
    }
    pub fn hovering(&self) -> bool {
        self.hover.is_some_and(|id| id != CLIPBOARD)
    }
    /// The rocker acts on press, as retail does: the page turns and the
    /// rocker tilts while the button is down.
    pub fn down(&mut self) -> Action {
        self.right_pressed = false;
        self.pressed = self.hover;
        match self.hover {
            Some(PREVIOUS) if !self.help => self.push(false, true),
            Some(NEXT) if !self.help => self.push(true, true),
            _ => Action::None,
        }
    }
    /// `Some(action)` while the debrief stays open; `None` once OK closes it.
    pub fn up(&mut self) -> Option<Action> {
        let pressed = self.pressed.take();
        if matches!(pressed, Some(PREVIOUS | NEXT)) && self.rocker.held() {
            self.rocker.release(Instant::now());
            return Some(Action::RockerUp);
        }
        match pressed.filter(|p| Some(*p) == self.hover) {
            Some(id) => self.activate(id),
            None => Some(Action::None),
        }
    }
    /// Right-clicking the clipboard turns back a page.
    pub fn right(&mut self, down: bool) -> Action {
        if self.help {
            self.right_pressed = false;
            return Action::None;
        }
        if down {
            self.pressed = None;
            self.right_pressed = self.hover == Some(CLIPBOARD);
            return Action::None;
        }
        if std::mem::take(&mut self.right_pressed) && self.hover == Some(CLIPBOARD) {
            self.push(false, false)
        } else {
            Action::None
        }
    }
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.right_pressed = false;
        self.hover = None;
        self.help = false;
        if self.rocker.held() {
            self.rocker.release(Instant::now());
        }
    }
    /// Turns one page and tilts the rocker toward it. Paging stops at either
    /// end; the rocker still moves.
    fn push(&mut self, forward: bool, held: bool) -> Action {
        self.page = if forward {
            (self.page + 1).min(self.pages.len() - 1)
        } else {
            self.page.saturating_sub(1)
        };
        self.rocker.push(forward, held, Instant::now());
        Action::RockerDown
    }
    fn activate(&mut self, id: usize) -> Option<Action> {
        if self.help && id != HELP && id != EXIT {
            self.help = false;
            return Some(Action::None);
        }
        Some(match id {
            OK => return None,
            CLIPBOARD => self.push(true, false),
            HELP => {
                self.help = !self.help;
                Action::Click
            }
            EXIT => Action::Exit,
            _ => Action::None,
        })
    }
    pub fn key(&mut self, key: &str) -> Option<Action> {
        if self.help {
            if key == "Escape" {
                self.help = false;
            }
            return Some(Action::None);
        }
        match key {
            "Enter" | "Escape" | " " => None,
            "PageUp" | "ArrowLeft" | "ArrowUp" => Some(self.push(false, false)),
            "PageDown" | "ArrowRight" | "ArrowDown" => Some(self.push(true, false)),
            "Home" => {
                self.page = 1;
                Some(self.push(false, false))
            }
            "End" => {
                self.page = self.pages.len() - 2;
                Some(self.push(true, false))
            }
            _ => Some(Action::None),
        }
    }
    /// Draws the screen; true while the rocker is still moving.
    pub fn render(&mut self, pixels: &mut [u8]) -> bool {
        let animating = self.rocker.advance(Instant::now());
        pixels.copy_from_slice(&self.sprites[&self.background].rgba);
        self.controls.clear();
        let s = &self.sprites;
        let mut c = Canvas(pixels);
        // The clipboard board turns pages; the controls sit on top of it.
        let board = if matches!(self.background.as_str(), "DEBSCV.PIC" | "DEBSCU.PIC") {
            (248, 66, 333, 404)
        } else {
            (278, 66, 303, 404)
        };
        self.controls.push((CLIPBOARD, board));
        // BRIEFSCR.DLG places its controls from origin (48, 363).
        let (ox, oy) = (48, 363);
        c.centered_text(&s["MENUFONT.PIC"], "?", (84, 38, 18, 20));
        self.controls.push((HELP, (84, 35, 18, 24)));
        // Every background already carries the black page box.
        c.centered_text(
            &s["PANLFNT2.PIC"],
            &format!("{} of  {}", self.page + 1, self.pages.len()),
            (ox + 30, oy + 18, 50, 17),
        );
        let panel = &s["PANELFNT.PIC"];
        c.text(panel, "PREV", ox + 20, oy + 41, None);
        c.text(panel, "NEXT", ox + 20, oy + 63, None);
        let rocker = &s[&self.rocker.sprite()];
        c.blit(rocker, (ox + 48, oy + 39), 0, rocker.width, 1.);
        self.controls.extend([
            (PREVIOUS, (ox + 48, oy + 39, 18, 16)),
            (NEXT, (ox + 48, oy + 55, 18, 16)),
        ]);
        // Cancel is present but never available on the debrief.
        c.button_style(s, "", (ox + 97, oy + 14, 70), 1., "ACTIOD0");
        label(&mut c, &s["QUICKFONTD"], "Cancel", (ox + 97, oy + 14, 70));
        let ok = c.action_button(
            s,
            "OK",
            (ox + 97, oy + 47, 60),
            true,
            self.pressed == Some(OK) && self.hover == Some(OK),
        );
        self.controls.push((OK, ok));
        if let Some(lines) = self.pages.get(self.page) {
            clipboard(&mut c, s, lines, self.page == 0);
        }
        if self.help {
            c.rect((84, 60, 180, 25), [212, 215, 218, 255]);
            c.text(&s["MENUFONT.PIC"], "Exit to Desktop", 89, 64, None);
            self.controls = vec![(HELP, (84, 35, 18, 24)), (EXIT, (84, 60, 180, 25))];
        }
        animating
    }
}

fn label(c: &mut Canvas, font: &Sprite, text: &str, (x, y, w): (i32, i32, i32)) {
    let height = text
        .glyph_codes()
        .map(|b| font.glyphs[b as usize][2])
        .max()
        .unwrap_or(0) as i32;
    c.text(
        font,
        text,
        x + (w - 10 - text_width(font, text)) / 2,
        y + (21 - height) / 2 + 2,
        None,
    );
}

/// Lays out one page of mission-text markup on the clipboard.
fn clipboard(c: &mut Canvas, s: &BTreeMap<String, Sprite>, lines: &[String], first_page: bool) {
    let (mut center, mut header, mut bold, mut underline) = (false, false, false, false);
    let mut y = TOP;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('.') {
            for directive in trimmed.split_whitespace() {
                match directive {
                    ".center" => center = true,
                    ".left" => center = false,
                    ".header" => header = true,
                    ".body" => header = false,
                    ".bold" => bold = true,
                    "..bold" => bold = false,
                    ".underline" => underline = true,
                    "..underline" => underline = false,
                    _ => {}
                }
            }
            continue;
        }
        let font = &s[if header {
            "HEADFONT.PIC"
        } else if bold {
            "BOLDFONT.PIC"
        } else {
            "BODYFONT.PIC"
        }];
        let cells: Vec<&str> = line.split('\t').collect();
        let width = text_width(font, line.trim_end());
        // The first-page result sentence shares its heading's clipboard axis,
        // even though QUICK.MT switches back to .left for the body.
        let mut x = if (center || first_page) && cells.len() == 1 {
            CENTER - width / 2
        } else {
            LEFT
        };
        for (index, text) in cells.iter().enumerate() {
            if index > 0 {
                x = TABS.iter().copied().find(|stop| *stop > x).unwrap_or(x);
            }
            c.text(font, text, x, y, None);
            if underline && !text.trim().is_empty() {
                let baseline = font.glyphs[b'H' as usize][2] as i32;
                let bottom = (0..baseline)
                    .rev()
                    .find(|row| {
                        let [sx, w, _] = font.glyphs[b'H' as usize];
                        (sx..sx + w)
                            .any(|col| font.rgba[(*row as usize * font.width + col) * 4 + 3] > 0)
                    })
                    .unwrap_or(baseline - 1);
                c.rect(
                    (x, y + bottom + 2, text_width(font, text), 1),
                    [0, 0, 0, 255],
                );
            }
            x += text_width(font, text);
        }
        y += if header { HEADER_LINE } else { LINE };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text() -> MissionText {
        MissionText::parse(
            b".section 3\n.center\n.header\nWON\n.section 4\n.center\n.header\nLOST\n",
        )
        .unwrap()
    }
    #[test]
    fn five_pages_with_mission_text_first() {
        let lost = pages(&Report::default(), &text());
        assert_eq!(lost.len(), 5);
        assert_eq!(lost[0], [".center", ".header", "LOST"]);
        let won = pages(
            &Report {
                outcome: Outcome::Success,
                ..Report::default()
            },
            &text(),
        );
        assert_eq!(won[0][2], "WON");
        assert!(won[1].contains(&"MISSION OUTCOME : SUCCESS".to_string()));
    }
    #[test]
    fn empty_cells_show_a_dash_and_missing_wingman_is_all_dashes() {
        let mut report = Report::default();
        report.player.kills[0] = 2;
        report.player.air_to_air = Tally {
            launched: 3,
            hit: 1,
            damage: 340,
            spoofed: 1,
            ..Tally::default()
        };
        report.player.gun = Tally {
            launched: 200,
            hit: 3,
            ..Tally::default()
        };
        let pages = pages(&report, &text());
        assert!(pages[1].contains(&"Status\tAlive\t-".to_string()));
        assert!(pages[1].contains(&"Damage\t0%\t-".to_string()));
        assert!(pages[1].contains(&"Landing grade\t-\t-".to_string()));
        assert!(pages[2].contains(&"Fighter\t2\t-".to_string()));
        assert!(pages[2].contains(&"Bomber\t-\t-".to_string()));
        assert!(pages[3].contains(&"    Launches\t3\t-".to_string()));
        // Rates round down: one of three is 33%.
        assert!(pages[3].contains(&"    Hit\t33% (340)\t-".to_string()));
        assert!(pages[3].contains(&"    Failed\t33%\t-".to_string()));
        assert!(pages[3].contains(&"    Spoofed\t33%\t-".to_string()));
        assert!(pages[3].contains(&"    Hit\t1%\t-".to_string()));
    }
    fn airframe(id: u32, friendly: bool, alive: bool) -> Airframe {
        Airframe {
            id,
            friendly,
            alive,
            ejected: false,
            damage: 0.25,
            landing_grade: None,
            cause: None,
        }
    }
    fn ending(ledger: &Ledger) -> Ending<'_> {
        Ending {
            ledger,
            ticks: 125 * 120 + 60,
            player: airframe(0, true, true),
            aircraft: vec![
                airframe(1, true, true),
                airframe(2, true, true),
                airframe(10, false, false),
                airframe(11, false, true),
            ],
            wingman: Some(1),
            requirements: Requirements {
                destroy: vec![10, 11],
                protect: vec![],
            },
        }
    }
    #[test]
    fn an_out_of_bounds_enemy_is_a_lost_aircraft_credited_to_nobody() {
        use tore_sim::aircraft_systems::LossCause;
        use tore_sim::combat::ledger::Kill;
        let mut ledger = Ledger::default();
        // The player had hit enemy 10 earlier, then it flew off the map.
        ledger.damaged(Kill {
            owner: 0,
            victim: 10,
            category: 0x8000,
            aircraft: true,
        });
        ledger.lose_without_credit(10);
        let mut end = ending(&ledger);
        end.aircraft[2].cause = Some(LossCause::OutOfBounds);
        let plain = report(&end);
        assert_eq!(plain.player.kills, [0; 10]);
        // The debrief keeps its retail rows when nobody's cause is set.
        assert!(
            !pages(&plain, &text())
                .concat()
                .iter()
                .any(|l| l.starts_with("Cause"))
        );
        end.player.alive = false;
        end.player.cause = Some(LossCause::Overspeed);
        let lost = report(&end);
        assert_eq!(lost.player.status, Status::Dead);
        assert_eq!(lost.player.cause, Some("overspeed"));
        assert!(lost.summary().contains("cause=overspeed"));
        assert!(
            pages(&lost, &text())
                .concat()
                .iter()
                .any(|l| l.starts_with("Cause\toverspeed"))
        );
    }
    #[test]
    fn overspeed_and_edge_losses_credit_no_kill_for_the_player_or_an_enemy() {
        use tore_sim::aircraft_systems::LossCause;
        use tore_sim::combat::ledger::Kill;
        let mut ledger = Ledger::default();
        // The player hit enemy 10 earlier, and enemy 10 had hit the player.
        for (owner, victim) in [(0, 10), (10, 0)] {
            ledger.damaged(Kill {
                owner,
                victim,
                category: 0x8000,
                aircraft: true,
            });
        }
        // Enemy 10 is then lost to overspeed or the map edge alike:
        // the ledger holds no credit for it.
        ledger.lose_without_credit(10);
        let mut end = ending(&ledger);
        end.aircraft[2].cause = Some(LossCause::Overspeed);
        // The player is lost to overspeed too; a lost player is never credited
        // to the last aircraft that hit it.
        end.player.alive = false;
        end.player.cause = Some(LossCause::Overspeed);
        let report = report(&end);
        assert_eq!(report.player.kills, [0; 10]);
        assert_eq!(report.player.status, Status::Dead);
        let fates = end.fates();
        let standing = Standing {
            ledger: end.ledger,
            plane: end.player.id,
            aircraft: &fates,
            requirements: &end.requirements,
        };
        assert!(
            standing
                .kills()
                .iter()
                .all(|k| k.victim != 10 && k.victim != 0)
        );
    }
    #[test]
    fn a_surviving_target_or_friendly_kill_fails_the_mission() {
        use tore_sim::combat::ledger::{Kill, Resolution};
        let mut ledger = Ledger::default();
        let first = report(&ending(&ledger));
        assert_eq!(first.outcome, Outcome::Failure);
        assert_eq!(
            first.objectives,
            [Objective::Destroy {
                destroyed: 1,
                total: 2
            }]
        );
        assert_eq!(first.elapsed_seconds, 125);
        let mut all_down = ending(&ledger);
        all_down.aircraft[3].alive = false;
        assert_eq!(report_outcome(&all_down), Outcome::Success);
        // Shooting down a friendly turns the same result into a failure.
        ledger.launch(5, 0, Some(2), ShotKind::AirToAir);
        ledger.resolve(5, Resolution::Hit(90));
        ledger.kill(Kill {
            owner: 0,
            victim: 2,
            category: 0x8000,
            aircraft: true,
        });
        let mut all_down = ending(&ledger);
        all_down.aircraft[3].alive = false;
        let result = report(&all_down);
        assert_eq!(result.outcome, Outcome::Failure);
        assert_eq!(result.player.friendly_fire, 1);
        assert_eq!(result.player.kills, [0; 10]);
    }
    fn report_outcome(end: &Ending) -> Outcome {
        report(end).outcome
    }
    #[test]
    fn wingman_column_and_enemy_fire_follow_owner_and_aim() {
        use tore_sim::combat::ledger::{Kill, Resolution};
        let mut ledger = Ledger::default();
        // The wingman downs a bomber; an enemy guns the wingman and fires a
        // missile at the player.
        ledger.kill(Kill {
            owner: 1,
            victim: 10,
            category: 0x4000,
            aircraft: true,
        });
        ledger.aim(20, 1);
        ledger.launch(20, 11, None, ShotKind::Gun);
        ledger.resolve(20, Resolution::Hit(15));
        ledger.launch(21, 11, Some(0), ShotKind::AirToAir);
        let mut end = ending(&ledger);
        end.player.alive = false;
        let result = report(&end);
        let wingman = result.wingman.as_ref().unwrap();
        assert_eq!(wingman.kills[1], 1);
        assert_eq!(wingman.enemy_gun.hit, 1);
        assert_eq!(wingman.enemy_gun.damage, 15);
        assert_eq!(result.player.enemy_aam.launched, 1);
        assert_eq!(result.player.enemy_gun.launched, 0);
        assert_eq!(result.player.status, Status::Dead);
        assert_eq!(result.player.damage, 1.);
        assert_eq!(wingman.damage, 0.25);
        // An enemy whose pilot ejects after the player's hit is the player's
        // kill; an ejected wingman shows as ejected, with full damage.
        let mut ledger = Ledger::default();
        ledger.damaged(Kill {
            owner: 0,
            victim: 11,
            category: 0x8000,
            aircraft: true,
        });
        let mut end = ending(&ledger);
        end.aircraft[3].alive = false;
        end.aircraft[3].ejected = true;
        end.aircraft[0].alive = false;
        end.aircraft[0].ejected = true;
        let result = report(&end);
        assert_eq!(result.player.kills[0], 1);
        assert_eq!(result.outcome, Outcome::Success);
        let wingman = result.wingman.unwrap();
        assert_eq!(wingman.status, Status::Ejected);
        assert_eq!(wingman.damage, 1.);
        let alone = report(&Ending {
            wingman: None,
            ..ending(&ledger)
        });
        assert!(alone.wingman.is_none());
    }
    /// Two humans in Friendly Wing 1 (plane 0, the lead, is seat 0; plane 2 is
    /// seat 1 and the wing's second member), an AI third member (plane 1) and
    /// an enemy pair (10, 11).
    fn two_seat_roster() -> Roster {
        use tore_sim::ai::launch::{Side, WingId};
        let slot = |side, member| crate::seats::Slot {
            wing: WingId { side, index: 0 },
            member,
        };
        Roster::with_humans(
            [
                (PlaneId(0), slot(Side::Friendly, 0), SeatId(0), None),
                (PlaneId(2), slot(Side::Friendly, 1), SeatId(1), None),
            ],
            [
                (PlaneId(1), slot(Side::Friendly, 2)),
                (PlaneId(10), slot(Side::Enemy, 0)),
                (PlaneId(11), slot(Side::Enemy, 1)),
            ],
        )
    }
    /// The debrief of the plane `plane` in the two seat mission: every other
    /// aircraft is a member of the ending, friendly when it flies the same
    /// side. Enemy 11 is the only aircraft still flying.
    fn seat_ending<'a>(ledger: &'a Ledger, roster: &Roster, plane: u32) -> Ending<'a> {
        let side = roster.plane(PlaneId(plane)).unwrap().slot.wing.side;
        let frame = |id: u32| {
            let p = roster.plane(PlaneId(id)).unwrap();
            airframe(id, p.slot.wing.side == side, id == 11)
        };
        let requirements = Requirements {
            destroy: [10, 11]
                .into_iter()
                .filter(|id| roster.plane(PlaneId(*id)).unwrap().slot.wing.side != side)
                .collect(),
            protect: vec![],
        };
        Ending {
            ledger,
            ticks: 600,
            player: airframe(plane, true, true),
            aircraft: [0, 1, 2, 10, 11]
                .into_iter()
                .filter(|id| *id != plane)
                .map(frame)
                .collect(),
            wingman: wingman_of(roster, PlaneId(plane)),
            requirements,
        }
    }
    #[test]
    fn the_wingman_is_the_first_other_member_of_the_planes_own_wing() {
        let roster = two_seat_roster();
        // Seat 0 keeps the lead's wingman, now the other human.
        assert_eq!(wingman_of(&roster, PlaneId(0)), Some(2));
        // Seat 1's is the lead, not the AI member and not an enemy.
        assert_eq!(wingman_of(&roster, PlaneId(2)), Some(0));
        assert_eq!(wingman_of(&roster, PlaneId(1)), Some(0));
        assert_eq!(wingman_of(&roster, PlaneId(10)), Some(11));
        // Alone in its wing: nobody.
        let alone = Roster::single_player(None, []);
        assert_eq!(wingman_of(&alone, PlaneId(0)), None);
        // Single player as it is today: the lowest AI member of the wing.
        use tore_sim::ai::launch::{Side, WingId};
        let wing = WingId {
            side: Side::Friendly,
            index: 0,
        };
        let solo = Roster::single_player(
            None,
            [
                (PlaneId(3), crate::seats::Slot { wing, member: 2 }),
                (PlaneId(1), crate::seats::Slot { wing, member: 1 }),
            ],
        );
        assert_eq!(wingman_of(&solo, PlaneId(0)), Some(1));
    }
    #[test]
    fn a_seats_debrief_puts_its_own_plane_in_the_pilot_column() {
        use tore_sim::combat::ledger::Kill;
        let roster = two_seat_roster();
        let mut ledger = Ledger::default();
        // Seat 1's plane shoots down enemy 10; the lead fires its gun at
        // enemy 11 and gets no kill.
        ledger.launch(1, 2, Some(10), ShotKind::AirToAir);
        ledger.launch(2, 0, Some(11), ShotKind::Gun);
        ledger.kill(Kill {
            owner: 2,
            victim: 10,
            category: 0x8000,
            aircraft: true,
        });
        let second = report(&seat_ending(&ledger, &roster, 2));
        let first = report(&seat_ending(&ledger, &roster, 0));
        // Seat 1: its own shot and kill in the pilot column, the lead's gun in
        // the wingman column.
        assert_eq!(second.player.kills[0], 1);
        assert_eq!(second.player.air_to_air.launched, 1);
        assert_eq!(second.player.gun.launched, 0);
        assert_eq!(second.wingman.as_ref().unwrap().gun.launched, 1);
        assert_eq!(second.wingman.as_ref().unwrap().kills, [0; 10]);
        // Seat 0 reads the same ledger from the other side.
        assert_eq!(first.player.kills, [0; 10]);
        assert_eq!(first.player.gun.launched, 1);
        assert_eq!(first.wingman.as_ref().unwrap().kills[0], 1);
        // Both must still destroy enemy 11.
        let open = [Objective::Destroy {
            destroyed: 1,
            total: 2,
        }];
        assert_eq!(second.objectives, open);
        assert_eq!(first.objectives, open);
    }
    #[test]
    fn friendly_fire_counts_only_the_kills_the_seats_own_plane_made() {
        use tore_sim::combat::ledger::Kill;
        let roster = two_seat_roster();
        let mut ledger = Ledger::default();
        // Seat 1's plane shoots down the AI member of its own wing.
        ledger.kill(Kill {
            owner: 2,
            victim: 1,
            category: 0x8000,
            aircraft: true,
        });
        let ending = |plane| {
            let mut end = seat_ending(&ledger, &roster, plane);
            // Every enemy is down, so only friendly fire can fail the mission.
            for a in end.aircraft.iter_mut().filter(|a| !a.friendly) {
                a.alive = false;
            }
            end
        };
        let (second, first) = (report(&ending(2)), report(&ending(0)));
        assert_eq!(second.player.friendly_fire, 1);
        assert_eq!(second.outcome, Outcome::Failure);
        assert_eq!(first.player.friendly_fire, 0);
        assert_eq!(first.outcome, Outcome::Success);
        assert_eq!(first.wingman.unwrap().friendly_fire, 1);
    }
    #[test]
    fn a_cockpits_airframe_reads_its_own_flight_and_ownship() {
        let mut flight = crate::combat_view::render_hash_tests::player();
        flight.damage_fraction = 0.4;
        let flying = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((flying.id, flying.alive, flying.ejected), (2, true, false));
        assert_eq!(flying.damage, 0.4);
        // No hit points left: shot down, though the flight itself is intact.
        assert!(!cockpit_airframe(2, true, &flight, 0).alive);
        // The pilot escaped: ejected, not alive; a dead pilot is never ejected.
        flight.systems.pilot.ejected = true;
        let out = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((out.alive, out.ejected), (false, true));
        flight.systems.pilot.dead = true;
        let dead = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((dead.alive, dead.ejected), (false, false));
        flight.systems.pilot = Default::default();
        flight.crashed = true;
        assert!(!cockpit_airframe(2, true, &flight, 500).alive);
    }
    #[test]
    fn kill_rows_take_the_first_matching_class_bit() {
        assert_eq!(kill_row(0x8000), Some(0));
        assert_eq!(kill_row(0x4000), Some(1));
        assert_eq!(kill_row(0x2100), Some(3));
        assert_eq!(kill_row(0x100), Some(8));
        assert_eq!(kill_row(0x40), Some(9));
        assert_eq!(kill_row(0x1), None);
    }
    #[test]
    fn objective_sentences_follow_counts() {
        let destroy = |destroyed, total| Objective::Destroy { destroyed, total }.sentence();
        assert_eq!(destroy(0, 3), "Destroyed 0 of 3 targets.");
        assert_eq!(destroy(3, 3), "Destroyed the 3 targets.");
        assert_eq!(destroy(1, 1), "Destroyed the target.");
        assert_eq!(destroy(0, 1), "Failed to destroy the target.");
        let protect = |protected, total| Objective::Protect { protected, total }.sentence();
        assert_eq!(protect(1, 2), "Protected 1 of 2 friendly objectives.");
        assert_eq!(protect(0, 1), "Failed to protect the friendly objective.");
    }
}
