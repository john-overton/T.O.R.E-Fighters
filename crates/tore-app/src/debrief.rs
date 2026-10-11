//! Post-mission debrief over the retail clipboard art. The BRIEFSCR.DLG
//! controls, page text fonts and mission text are imported; page layout is
//! fitted to John's retail screenshots. Spec: docs/spec/debrief.md. The
//! evaluator that builds the report is `tore_world::debrief`.
use crate::rocker::Rocker;
use crate::{
    AppResult,
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
};
use std::{collections::BTreeMap, time::Instant};
use tore_formats::text::GlyphCodes;
use tore_formats::{Pic, mission_text::MissionText};
use tore_sim::combat::ledger::Tally;
// The evaluator lives in the mission core; the rest of the app reads it here.
pub use tore_world::debrief::{KILL_ROWS, Objective, Outcome, Pilot, Report, Status, capture};

type Rect = (i32, i32, i32, i32);

/// Debrief art beyond the shared menu art. The import keeps it, so the lists
/// live in `tore-import`.
pub use tore_import::selection::DEBRIEF_ART as ART;

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

/// How a networked flight's debrief says it ended when its objectives did
/// not decide the result: the first page's heading and sentence, and the
/// outcome line's word. Single player never has one (its debrief is
/// unchanged); see `net::debrief::ending`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ended {
    pub title: String,
    pub sentence: String,
}

/// The first page of an ended mission: the retail page's own layout with its
/// heading and result sentence replaced.
fn ended_page(retail: &[String], ended: &Ended) -> Vec<String> {
    let mut page = Vec::new();
    let mut seen = 0;
    for line in retail {
        let trimmed = line.trim();
        if trimmed.starts_with('.') || trimmed.is_empty() {
            page.push(line.clone());
            continue;
        }
        seen += 1;
        match seen {
            1 => page.push(ended.title.clone()),
            2 => page.push(ended.sentence.clone()),
            _ => {}
        }
    }
    if seen == 0 {
        page = vec![".center".into(), ".header".into(), ended.title.clone()];
    }
    if seen < 2 {
        page.extend([".body".into(), String::new(), ended.sentence.clone()]);
    }
    page
}

/// The pages with none of a networked flight's extra ones.
#[cfg(test)]
pub fn pages_ended(report: &Report, text: &MissionText, ended: Option<&Ended>) -> Vec<Vec<String>> {
    pages_with(report, text, ended, &[])
}

/// Page markup in the mission-text grammar: `.center`, `.left`, `.header`,
/// `.body`, `.bold`/`..bold`, `.underline`/`..underline` and tab columns.
/// `ended` is a networked flight's, which its objectives did not decide.
/// `extra` pages (a networked flight's SCORES and RESULTS, see
/// `net::debrief::extra_pages`) are turned in after the first.
pub fn pages_with(
    report: &Report,
    text: &MissionText,
    ended: Option<&Ended>,
    extra: &[Vec<String>],
) -> Vec<Vec<String>> {
    let success = report.outcome == Outcome::Success;
    let retail = text
        .debrief(success)
        .map(<[String]>::to_vec)
        .unwrap_or_default();
    let first = match ended {
        Some(ended) => ended_page(&retail, ended),
        None => retail,
    };
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

    let mut outcome = heading(&format!(
        "MISSION OUTCOME : {}",
        if ended.is_some() {
            "INCOMPLETE"
        } else {
            report.outcome.label()
        }
    ));
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
    // Only when a surface unit (a SAM site, a gun, a ship) scored the loss.
    if player.is_some_and(|p| p.shot_down_by.is_some())
        || wing.is_some_and(|p| p.shot_down_by.is_some())
    {
        outcome.push(row("Shot down by", &|p| {
            p.shot_down_by.clone().unwrap_or_else(|| "-".into())
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

    let mut pages = vec![first];
    pages.extend(extra.iter().cloned());
    pages.extend([outcome, kills, hits, enemy]);
    pages
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
        Self::networked(report, data, background, None, &[])
    }

    /// A networked flight's debrief: `ended` is set when the flight ended
    /// without its objectives deciding the result (the King, the server or
    /// the player ended it), so it shows no success or failure it did not
    /// earn.
    pub fn networked(
        report: Report,
        data: &BTreeMap<String, Vec<u8>>,
        background: Option<&str>,
        ended: Option<&Ended>,
        extra: &[Vec<String>],
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
            pages: pages_with(&report, &text, ended, extra),
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
    // `.columns X1 X2 ...` (the networked pages' own directive, not retail's)
    // starts cell n of every following line at the n-th x, whatever the
    // cells before it hold.
    let mut columns: Vec<i32> = Vec::new();
    let mut y = TOP;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('.') {
            let mut words = trimmed.split_whitespace();
            while let Some(directive) = words.next() {
                match directive {
                    ".columns" => {
                        columns.clear();
                        let mut rest = words.clone().peekable();
                        while let Some(x) = rest.peek().and_then(|w| w.parse::<i32>().ok()) {
                            columns.push(x);
                            rest.next();
                            words.next();
                        }
                    }
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
            if let Some(column) = columns.get(index).filter(|_| cells.len() > 1) {
                x = *column;
            } else if index > 0 {
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
    fn pages(report: &Report, text: &MissionText) -> Vec<Vec<String>> {
        pages_ended(report, text, None)
    }
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
    fn a_networked_flights_extra_pages_follow_the_first_and_single_player_has_none() {
        let extra = vec![vec!["SCORES".to_owned()], vec!["RESULTS".to_owned()]];
        let report = Report::default();
        let with = pages_with(&report, &text(), None, &extra);
        assert_eq!(with.len(), 7);
        assert_eq!(with[0], [".center", ".header", "LOST"]);
        assert_eq!(with[1], ["SCORES"]);
        assert_eq!(with[2], ["RESULTS"]);
        assert!(with[3].contains(&"MISSION OUTCOME : FAILURE".to_string()));
        // The retail pages after them are the five-page debrief's, unchanged.
        let plain = pages(&report, &text());
        assert_eq!([&with[..1], &with[3..]].concat(), plain);
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
    #[test]
    fn a_lost_airframes_cause_shows_only_when_one_is_set() {
        let plain = Report::default();
        assert!(
            !pages(&plain, &text())
                .concat()
                .iter()
                .any(|l| l.starts_with("Cause"))
        );
        let lost = Report {
            player: Pilot {
                status: Status::Dead,
                cause: Some("overspeed"),
                ..Pilot::default()
            },
            ..Report::default()
        };
        assert!(
            pages(&lost, &text())
                .concat()
                .iter()
                .any(|l| l.starts_with("Cause\toverspeed"))
        );
    }
    #[test]
    fn a_surface_units_kill_names_it_and_its_fire_fills_the_sam_and_aaa_rows() {
        let plain = Report::default();
        assert!(
            !pages(&plain, &text())
                .concat()
                .iter()
                .any(|l| l.starts_with("Shot down by"))
        );
        let tally = |launched, hit| Tally {
            launched,
            hit,
            damage: hit * 40,
            ..Tally::default()
        };
        let shot = Report {
            player: Pilot {
                status: Status::Dead,
                shot_down_by: Some("SA-6".into()),
                enemy_sam: tally(4, 1),
                enemy_aaa: tally(30, 3),
                ..Pilot::default()
            },
            wingman: Some(Pilot {
                status: Status::Dead,
                shot_down_by: Some("ZSU-23-4".into()),
                ..Pilot::default()
            }),
            ..Report::default()
        };
        let pages = pages(&shot, &text());
        assert!(pages[1].contains(&"Shot down by\tSA-6\tZSU-23-4".to_string()));
        // The enemy page: SAM launches and the AAA hit percentage.
        assert!(pages[4].contains(&"    Launches\t4\t-".to_string()));
        assert!(pages[4].contains(&"    Hit\t25% (40)\t-".to_string()));
        assert!(pages[4].contains(&"    Hit\t10% (120)\t-".to_string()));
    }
    #[test]
    fn an_ended_mission_keeps_the_retail_layout_and_says_it_was_not_decided() {
        let ended = Ended {
            title: "MISSION ENDED".into(),
            sentence: "The King ended the mission.".into(),
        };
        let retail = b".section 4\n.center\n.header\nMISSION FAILURE\n.left\n.body\n\nYou failed this Quick Mission.\n";
        let text = MissionText::parse(retail).unwrap();
        let ended_pages = pages_ended(&Report::default(), &text, Some(&ended));
        assert_eq!(
            ended_pages[0],
            [
                ".center",
                ".header",
                "MISSION ENDED",
                ".left",
                ".body",
                "",
                "The King ended the mission."
            ]
        );
        assert!(ended_pages[1].contains(&"MISSION OUTCOME : INCOMPLETE".to_string()));
        // Without it the page is the retail one, word for word.
        let plain = pages_ended(&Report::default(), &text, None);
        assert!(plain[0].contains(&"You failed this Quick Mission.".to_string()));
        assert!(plain[1].contains(&"MISSION OUTCOME : FAILURE".to_string()));
        // A mission text with no section still gets a page.
        let empty = MissionText::parse(b".section 1\nX\n").unwrap();
        let bare = pages_ended(&Report::default(), &empty, Some(&ended));
        assert!(bare[0].contains(&"MISSION ENDED".to_string()));
        assert!(bare[0].contains(&"The King ended the mission.".to_string()));
    }
}

/// The multiplayer pages as they draw (stage F phase 2, slice F2-D): thirty
/// aircraft and a dozen players, headless. The committed tests lay the pages
/// out in a bundled font (the retail ones need the user's media), checking
/// that every page stays on the clipboard's height; the ignored one draws
/// them with the retail art of an imported profile and checks every cell
/// against its column in the retail fonts.
#[cfg(test)]
mod net_pages {
    use super::*;
    use crate::net::debrief::{PER_PAGE, extra_pages, sample_results};

    const WHITE: [u8; 3] = [255, 255, 255];
    /// The clipboard paper's right edge, and the lowest line that fits.
    const RIGHT: i32 = 581;
    const BOTTOM: i32 = 466;

    /// The retail fonts' places taken by the bundled font.
    fn bundled_fonts() -> BTreeMap<String, Sprite> {
        ["BODYFONT.PIC", "BOLDFONT.PIC", "HEADFONT.PIC"]
            .into_iter()
            .map(|name| (name.to_owned(), crate::menu::flat_font(WHITE)))
            .collect()
    }

    /// Draws the page and returns the box of pixels it touched.
    fn bounds(fonts: &BTreeMap<String, Sprite>, lines: &[String]) -> (i32, i32, i32, i32) {
        let mut pixels = vec![0u8; WIDTH * HEIGHT * 4];
        clipboard(&mut Canvas(&mut pixels), fonts, lines, false);
        let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, 0, 0);
        for (n, px) in pixels.chunks_exact(4).enumerate() {
            if px[3] > 0 {
                let (x, y) = ((n % WIDTH) as i32, (n / WIDTH) as i32);
                (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
            }
        }
        (left, top, right, bottom)
    }

    /// Every cell of a `.columns` page against its column: where the cell
    /// ends must be left of the next column (a pixel to spare), and the last
    /// one inside the paper. Returns what does not fit.
    fn overflows(fonts: &BTreeMap<String, Sprite>, lines: &[String]) -> Vec<String> {
        let (mut columns, mut bold) = (Vec::<i32>::new(), false);
        let mut problems = Vec::new();
        for line in lines {
            if line.trim().starts_with('.') {
                let mut words = line.split_whitespace();
                while let Some(word) = words.next() {
                    match word {
                        ".columns" => {
                            columns = words.clone().map_while(|w| w.parse().ok()).collect();
                        }
                        ".bold" => bold = true,
                        "..bold" => bold = false,
                        _ => {}
                    }
                }
                continue;
            }
            let cells: Vec<&str> = line.split('\t').collect();
            if cells.len() < 2 {
                continue;
            }
            let font = &fonts[if bold { "BOLDFONT.PIC" } else { "BODYFONT.PIC" }];
            for (n, cell) in cells.iter().enumerate() {
                let end = columns[n] + text_width(font, cell.trim_end());
                // A cell may run on over the empty ones after it.
                let limit = (n + 1..cells.len())
                    .find(|m| !cells[*m].is_empty())
                    .map_or(RIGHT, |m| columns[m] - 1);
                if end > limit {
                    problems.push(format!("{cell:?} ends at {end}, past {limit}"));
                }
            }
        }
        problems
    }

    #[test]
    fn thirty_aircraft_and_a_dozen_players_fit_the_clipboard_and_page_as_designed() {
        let fonts = bundled_fonts();
        let results = sample_results(15, 6);
        let pages = extra_pages(&results);
        // One page of scores (12 players) and two of results (15 and 15).
        assert_eq!(pages.len(), 3);
        let text = |page: &[String]| page.iter().filter(|l| !l.starts_with('.')).count();
        assert!(pages[0].contains(&"SCORES".to_string()));
        assert!(pages[1].contains(&"RESULTS : FRIENDLY SIDE".to_string()));
        assert!(pages[2].contains(&"RESULTS : ENEMY SIDE".to_string()));
        // A heading, a blank, the column heads and the fifteen rows.
        assert_eq!(text(&pages[1]), 1 + 1 + 1 + PER_PAGE);
        assert_eq!(text(&pages[2]), 1 + 1 + 1 + PER_PAGE);
        for (n, page) in pages.iter().enumerate() {
            let (left, top, _, bottom) = bounds(&fonts, page);
            assert!(left >= LEFT, "page {n} starts at {left}");
            assert!(top >= TOP - 4, "page {n} starts at {top}");
            assert!(bottom <= BOTTOM, "page {n} runs to {bottom}");
        }
    }

    #[test]
    fn thirty_players_take_two_score_pages_and_the_sides_totals_come_last() {
        let fonts = bundled_fonts();
        let results = sample_results(15, 15);
        let pages = extra_pages(&results);
        // Two of scores (15 and 15), two of results.
        assert_eq!(pages.len(), 4);
        let totals = |page: &[String]| page.iter().filter(|l| l.contains(" SIDE\t")).count();
        assert_eq!((totals(&pages[0]), totals(&pages[1])), (0, 2));
        // Ranks run on over the pages.
        assert!(pages[1].iter().any(|l| l.starts_with("16\t")));
        for page in &pages {
            assert!(bounds(&fonts, page).3 <= BOTTOM);
        }
    }

    #[test]
    fn a_lopsided_mission_pages_each_side_apart() {
        // 20 against 10: two pages for the first side, one for the second.
        let mut results = sample_results(20, 0);
        results
            .rows
            .retain(|r| r.wing.side == tore_sim::ai::launch::Side::Friendly || r.plane < 30);
        results.scores = None;
        let pages = extra_pages(&results);
        let titles: Vec<&str> = pages
            .iter()
            .map(|p| {
                p.iter()
                    .find(|l| l.starts_with("RESULTS"))
                    .unwrap()
                    .as_str()
            })
            .collect();
        assert_eq!(
            titles,
            [
                "RESULTS : FRIENDLY SIDE",
                "RESULTS : FRIENDLY SIDE",
                "RESULTS : ENEMY SIDE"
            ]
        );
    }

    #[test]
    fn a_column_starts_where_the_columns_directive_says_whatever_comes_before() {
        let fonts = bundled_fonts();
        let wide = ".columns 300 420".to_owned();
        let alone = bounds(&fonts, &[wide.clone(), "A\tB".into()]);
        let far = bounds(&fonts, &[wide, "AAAAAAAAAA\tB".into()]);
        // The second cell sits at 420 either way: the right edge is the same.
        assert_eq!(alone.2, far.2);
    }

    /// Draws the pages with the retail fonts and clipboard of an imported
    /// profile, one picture each into `TORE_CREATOR_DUMP` when it is set, and
    /// checks every cell against its column in the retail fonts:
    ///
    /// ```text
    /// TORE_DATA_DIR=... TORE_CREATOR_DUMP=/some/folder cargo test --locked \
    ///     -p tore-app debrief::net_pages::retail_art -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs an imported data profile (TORE_DATA_DIR)"]
    fn retail_art_draws_the_multiplayer_pages() {
        let data = crate::assets::data_directory().expect("data directory");
        let assets = crate::assets::Assets::load(&data).expect("an imported pack");
        for (name, per_side, humans) in [("even", 15, 6), ("lopsided", 20, 3), ("full", 15, 15)] {
            let results = sample_results(per_side, humans);
            let extra = extra_pages(&results);
            let report = Report::sample();
            let pages = 5 + extra.len();
            for page in 0..pages {
                let mut debrief = Debrief::networked(
                    report.clone(),
                    &assets.theater_resources,
                    Some("DEBSCV.PIC"),
                    None,
                    &extra,
                )
                .unwrap();
                debrief.page = page;
                let mut shot = vec![0u8; WIDTH * HEIGHT * 4];
                debrief.render(&mut shot);
                if let Some(folder) = std::env::var_os("TORE_CREATOR_DUMP") {
                    let folder = std::path::PathBuf::from(folder);
                    std::fs::create_dir_all(&folder).unwrap();
                    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
                    for p in shot.chunks_exact(4) {
                        out.extend_from_slice(&p[..3]);
                    }
                    std::fs::write(
                        folder.join(format!("debrief-net-{name}-{}.ppm", page + 1)),
                        out,
                    )
                    .unwrap();
                }
                // The pages after the first and before the retail ones.
                if (1..=extra.len()).contains(&page) {
                    let problems = overflows(&debrief.sprites, &extra[page - 1]);
                    assert!(
                        problems.is_empty(),
                        "{name} page {}: {problems:?}",
                        page + 1
                    );
                }
            }
        }
    }
}
