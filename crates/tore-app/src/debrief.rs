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
}
