//! Retail option values with an explicit editable setup; simulation capabilities
//! are validated separately. Popup frame/focus feedback are authored presentation.
use crate::{
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, ui::creator::Options};
type Rect = (i32, i32, i32, i32);
const POPUP: Rect = (185, 100, 270, 370);
const ROWS: usize = 21;
const ROW_BASE: usize = 100;
const OK: usize = 1;
const CANCEL: usize = 2;
const POP_OK: usize = 70;
const POP_CANCEL: usize = 71;
const UP: usize = 72;
const DOWN: usize = 73;
#[derive(Clone, Debug)]
pub struct Draft {
    pub values: [usize; 33],
}
impl Default for Draft {
    fn default() -> Self {
        let mut values = [0; 33];
        for (id, value) in [
            (4, 1),
            (5, 1),
            (15, 1),
            (16, 1),
            (17, 2),
            (19, 1),
            (21, 2),
            (22, 2),
        ] {
            values[id] = value;
        }
        Self { values }
    }
}
pub struct QuickMission {
    pub ordnance: Option<crate::ordnance::Ordnance>,
    pub hover: Option<usize>,
    pressed: Option<usize>,
    pub focus: usize,
    pub selection: usize,
    pub aircraft_selection: usize,
    pub aircraft_names: Vec<String>,
    pub aircraft_files: Vec<String>,
    pub draft: Draft,
    options: Options,
    selector: Option<usize>,
    cursor: usize,
    scroll: usize,
    controls: Vec<(usize, Rect)>,
    pub notice: Option<String>,
    pub help: bool,
    pub shift: bool,
}
impl QuickMission {
    pub fn new(id: AircraftId, options: Options, data: &BTreeMap<String, Vec<u8>>) -> Self {
        let mut catalog: Vec<(String, String)> = data
            .iter()
            .filter(|(n, _)| n.ends_with(".PT") && !n.starts_with('~'))
            .filter_map(|(file, bytes)| {
                let brf = tore_formats::aircraft::Brf::parse(bytes).ok()?;
                let names = brf.strings("ot_names").ok()?;
                (names.len() == 3 && names[2].eq_ignore_ascii_case(file))
                    .then(|| (file.clone(), names[0].clone()))
            })
            .collect();
        catalog.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let (aircraft_files, aircraft_names): (Vec<_>, Vec<_>) = catalog.into_iter().unzip();
        let selected = aircraft_files
            .iter()
            .position(|n| n == id.pt())
            .unwrap_or(0);
        let mut draft = Draft::default();
        for i in [6, 9, 12, 23, 26, 29] {
            draft.values[i] = selected;
        }
        Self {
            ordnance: None,
            hover: None,
            pressed: None,
            focus: 6,
            selection: 0,
            aircraft_selection: selected,
            aircraft_names,
            aircraft_files,
            draft,
            options,
            selector: None,
            cursor: 0,
            scroll: 0,
            controls: vec![],
            notice: None,
            help: false,
            shift: false,
        }
    }
    pub fn theater(&mut self, index: usize) {
        self.selection = index;
        // Source order differs from the app's theater catalog.
        if let Some((code, _)) = tore_formats::theater::THEATERS.get(index) {
            self.draft.values[13] = source_theaters()
                .iter()
                .position(|c| c == code)
                .unwrap_or(0);
        }
        self.nationalities();
    }
    fn nationalities(&mut self) {
        self.draft.values[3] = 0;
        self.draft.values[20] =
            [10, 33, 14, 57, 3, 41, 23, 10, 20, 37, 34, 24, 9, 2, 10, 2][self.draft.values[13]];
    }
    pub fn player(&self) -> Option<AircraftId> {
        self.aircraft_files
            .get(self.draft.values[6])
            .and_then(|n| AircraftId::parse(n).ok())
    }
    pub fn theater_index(&self) -> usize {
        let code = source_theaters()[self.draft.values[13]];
        tore_formats::theater::THEATERS
            .iter()
            .position(|(c, _)| *c == code)
            .unwrap_or(0)
    }
    fn values(&self, id: usize) -> &[String] {
        if matches!(id, 6 | 9 | 12 | 23 | 26 | 29) {
            &self.aircraft_names
        } else if id == 30 {
            &self.options.targets[self.draft.values[13]]
        } else {
            &self.options.fields[id]
        }
    }
    fn value(&self, id: usize) -> String {
        self.values(id)
            .get(self.draft.values[id])
            .cloned()
            .unwrap_or_else(|| "Unavailable".into())
    }
    pub fn unsupported(&self) -> Option<String> {
        if self.player().is_none() {
            return Some(
                "This aircraft is available for setup only. Choose F/A-18D or Rafale C to fly."
                    .into(),
            );
        }
        let v = &self.draft.values;
        if v[4] != 1 || [7, 10, 21, 24, 27].iter().any(|i| v[*i] != 0) {
            return Some("Additional aircraft are not available yet. Set friendly Wing 1 to one and all other wings to zero.".into());
        }
        if v[30] != 0 || v[31] != 0 || v[32] != 0 {
            return Some(
                "Ground targets and defenses are not available yet. Select none to fly.".into(),
            );
        }
        if v[15] != 1 {
            return Some("Only clear daytime conditions are available for flight yet.".into());
        }
        None
    }
    fn apply(&mut self, id: usize, value: usize) {
        self.draft.values[id] = value;
        self.draft.values[4] = self.draft.values[4].max(1);
        if self.draft.values[30] == 0 {
            self.draft.values[31] = 0;
            self.draft.values[32] = 0;
        }
        if id == 13 {
            self.draft.values[30] = 0;
            self.nationalities();
        }
        self.aircraft_selection = self.draft.values[6];
        self.selection = self.theater_index();
        self.notice = None;
    }
    fn open(&mut self, id: usize) {
        self.selector = Some(id);
        self.cursor = self.draft.values[id];
        self.scroll = self.cursor.saturating_sub(ROWS - 1);
        self.hover = None;
        self.pressed = None;
        self.help = false;
    }
    pub fn preview_selector(&mut self, name: &str) -> crate::AppResult<()> {
        match name {
            "normal" | "ordnance" => {}
            "aircraft" => self.open(6),
            "theaters" => self.open(13),
            "help" => self.help = true,
            _ => {
                let id=name.strip_prefix("field-").and_then(|v|v.parse::<usize>().ok()).filter(|v|(3..33).contains(v)).ok_or("snapshot states: normal, aircraft, theaters, help, field-3 through field-32")?;
                self.open(id);
            }
        }
        Ok(())
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            o.pointer(p);
            return;
        }
        self.hover = p.and_then(|p| {
            self.controls
                .iter()
                .rev()
                .find(|(_, r)| inside(p, *r))
                .map(|(i, _)| *i)
        });
    }
    pub fn down(&mut self) {
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            o.down();
            return;
        }
        self.pressed = self.hover;
    }
    pub fn up(&mut self) -> Action {
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.up();
        }
        let p = self.pressed.take();
        if let Some(i) = p.filter(|p| Some(*p) == self.hover) {
            self.activate(i)
        } else {
            Action::None
        }
    }
    pub fn cancel(&mut self) {
        if let Some(o) = &mut self.ordnance {
            o.cancel();
        }
        self.pressed = None;
        self.hover = None;
        self.selector = None;
        self.help = false;
    }
    fn activate(&mut self, id: usize) -> Action {
        if let Some(field) = self.selector {
            match id {
                POP_OK => {
                    self.apply(field, self.cursor);
                    self.selector = None;
                    self.focus = field;
                }
                POP_CANCEL => {
                    self.selector = None;
                    self.focus = field;
                }
                UP => self.scroll = self.scroll.saturating_sub(1),
                DOWN => {
                    self.scroll =
                        (self.scroll + 1).min(self.values(field).len().saturating_sub(ROWS))
                }
                ROW_BASE.. => {
                    let index = self.scroll + id - ROW_BASE;
                    if index < self.values(field).len() {
                        self.cursor = index;
                    }
                }
                _ => return Action::None,
            }
            self.hover = None;
            return Action::Click;
        }
        match id {
            0 => {
                self.help = !self.help;
            }
            60 => {
                self.notice=Some("Aircraft era filters are not available yet. The list shows imported aircraft; only F/A-18D and Rafale C can fly.".into());
            }
            61 => return Action::Exit,
            OK => {
                if let Some(message) = self.unsupported() {
                    self.notice = Some(message);
                } else {
                    return Action::Mission;
                }
            }
            CANCEL => return Action::Back,
            3..=32 => {
                self.focus = id;
                if matches!(id, 6 | 9 | 12 | 23 | 26 | 29) ^ self.shift {
                    self.open(id);
                } else {
                    let n = self.values(id).len();
                    if n > 0 {
                        self.apply(id, (self.draft.values[id] + 1) % n);
                    }
                }
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str, shift: bool) -> Action {
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            return o.key(key);
        }
        self.shift = shift;
        if key == "Escape" {
            if self.selector.is_some() || self.help || self.notice.is_some() {
                self.cancel();
                self.notice = None;
                return Action::None;
            }
            return Action::Back;
        }
        if let Some(field) = self.selector {
            let n = self.values(field).len();
            if n == 0 {
                return Action::None;
            }
            match key {
                "ArrowDown" => self.cursor = (self.cursor + 1) % n,
                "ArrowUp" => self.cursor = (self.cursor + n - 1) % n,
                "Home" => self.cursor = 0,
                "End" => self.cursor = n - 1,
                "PageDown" => self.cursor = (self.cursor + ROWS).min(n - 1),
                "PageUp" => self.cursor = self.cursor.saturating_sub(ROWS),
                "Enter" => return self.activate(POP_OK),
                _ => {}
            }
            if self.cursor < self.scroll {
                self.scroll = self.cursor;
            }
            if self.cursor >= self.scroll + ROWS {
                self.scroll = self.cursor + 1 - ROWS;
            }
            return Action::None;
        }
        match key {
            "Tab" | "ArrowDown" | "ArrowUp" => {
                let backwards = shift || key == "ArrowUp";
                self.focus = if backwards {
                    if self.focus <= 1 { 32 } else { self.focus - 1 }
                } else {
                    self.focus % 32 + 1
                };
                self.hover = Some(self.focus);
            }
            "Enter" | " " => return self.activate(self.focus),
            _ => {}
        }
        Action::None
    }
    pub fn render(
        &mut self,
        pixels: &mut [u8],
        sprites: &BTreeMap<String, Sprite>,
        _world: &World,
    ) {
        if let Some(o) = self.ordnance.as_mut().filter(|o| o.visible) {
            o.render(pixels);
            return;
        }
        pixels.copy_from_slice(&sprites["QUIKMIS3.PIC"].rgba);
        self.controls.clear();
        let mut c = Canvas(pixels);
        let font = &sprites["ARMFONT.PIC"];
        c.text(&sprites["MENUFONT.PIC"], "Aircraft", 103, 36, None);
        self.controls
            .extend([(0, (84, 35, 18, 24)), (60, (103, 35, 95, 24))]);
        c.text(font, "FRIENDLY SITUATION", 116, 104, None);
        c.text(font, "ENEMY SITUATION", 429, 104, None);
        self.line(
            &mut c,
            font,
            35,
            137,
            &[("Friendly forces are ", None), ("", Some(3))],
        );
        self.line(
            &mut c,
            font,
            340,
            137,
            &[("Enemy forces are ", None), ("", Some(20))],
        );
        for (x, start) in [(35, 4), (340, 21)] {
            for wing in 0..3 {
                let id = start + wing * 3;
                self.line(
                    &mut c,
                    font,
                    x,
                    165 + wing as i32 * 14,
                    &[
                        (&format!("Wing {}: ", wing + 1), None),
                        ("", Some(id)),
                        (" ", None),
                        ("", Some(id + 1)),
                        (" ", None),
                        ("", Some(id + 2)),
                    ],
                );
            }
        }
        for (y, parts) in [
            (221, vec![("You are flying over ", None), ("", Some(13))]),
            (
                235,
                vec![
                    ("You are at ", None),
                    ("", Some(14)),
                    (". It is ", None),
                    ("", Some(15)),
                ],
            ),
            (249, vec![("Your situation is ", None), ("", Some(16))]),
            (263, vec![("Enemy forces are ", None), ("", Some(17))]),
            (291, vec![("You are carrying ", None), ("", Some(18))]),
            (305, vec![("Air combat is ", None), ("", Some(19))]),
        ] {
            self.line(&mut c, font, 35, y, &parts);
        }
        self.line(
            &mut c,
            font,
            340,
            221,
            &[("Ground target: ", None), ("", Some(30))],
        );
        self.line(&mut c, font, 340, 249, &[("AAA: ", None), ("", Some(31))]);
        self.line(&mut c, font, 340, 263, &[("SAM: ", None), ("", Some(32))]);
        c.text(
            &sprites["SMLFONT.PIC"],
            "Airborne patrol preview; no enemy AI or objectives yet.",
            35,
            348,
            None,
        );
        self.button(&mut c, sprites, OK, "OK", (387, 419, 85, 24));
        self.button(&mut c, sprites, CANCEL, "Cancel", (492, 419, 85, 24));
        if let Some(message) = &self.notice {
            notice(&mut c, &sprites["SMLFONT.PIC"], message);
        }
        if self.help {
            c.rect((84, 60, 180, 25), [212, 215, 218, 255]);
            c.text(&sprites["MENUFONT.PIC"], "Exit to Desktop", 89, 64, None);
            self.controls = vec![(0, (84, 35, 18, 24)), (61, (84, 60, 180, 25))];
        }
        if let Some(field) = self.selector {
            self.controls.clear();
            c.rect(POPUP, [176, 183, 179, 255]);
            c.rect((205, 115, 230, 304), [213, 216, 209, 255]);
            for (row, text) in self
                .values(field)
                .iter()
                .skip(self.scroll)
                .take(ROWS)
                .enumerate()
            {
                let y = 115 + row as i32 * 14;
                if self.cursor == self.scroll + row {
                    c.rect((205, y, 230, 14), [125, 148, 129, 255]);
                }
                let label = fit(font, text, 224);
                c.text(font, &label, 208, y + 1, Some([32, 42, 36]));
            }
            for row in 0..ROWS.min(self.values(field).len().saturating_sub(self.scroll)) {
                self.controls
                    .push((ROW_BASE + row, (205, 115 + row as i32 * 14, 230, 14)));
            }
            self.button(&mut c, sprites, POP_OK, "OK", (217, 437, 85, 24));
            self.button(&mut c, sprites, POP_CANCEL, "Cancel", (312, 437, 85, 24));
            self.button(&mut c, sprites, UP, "-", (440, 120, 20, 24));
            self.button(&mut c, sprites, DOWN, "+", (440, 385, 20, 24));
        }
    }
    fn line(
        &mut self,
        c: &mut Canvas,
        font: &Sprite,
        x: i32,
        y: i32,
        parts: &[(&str, Option<usize>)],
    ) {
        let mut x = x;
        for (text, id) in parts {
            let text = id
                .map(|i| self.value(i))
                .unwrap_or_else(|| text.to_string());
            let text = fit(font, &text, if x < 320 { 299 - x } else { 605 - x });
            let width = text_width(font, &text);
            if let Some(id) = id {
                let r = (x - 1, y - 1, width + 2, 13);
                self.controls.push((*id, r));
                c.rect(
                    r,
                    if self.hover == Some(*id) {
                        [127, 139, 144, 255]
                    } else {
                        [101, 107, 109, 255]
                    },
                );
            }
            c.text(font, &text, x, y, Some([224, 225, 221]));
            x += width;
        }
    }
    fn button(
        &mut self,
        c: &mut Canvas,
        sprites: &BTreeMap<String, Sprite>,
        id: usize,
        label: &str,
        r: Rect,
    ) {
        self.controls.push((id, r));
        c.button_style(
            sprites,
            label,
            (r.0, r.1, r.2),
            if self.pressed == Some(id) { 0.8 } else { 1.0 },
            if id == OK || id == POP_OK {
                "ACTDFT0"
            } else {
                "ACTION0"
            },
        );
    }
}
fn source_theaters() -> [&'static str; 16] {
    [
        "BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU",
        "NSK", "WTA", "UKR", "VLA",
    ]
}
fn inside(p: (f64, f64), r: Rect) -> bool {
    p.0 >= r.0 as f64 && p.1 >= r.1 as f64 && p.0 < (r.0 + r.2) as f64 && p.1 < (r.1 + r.3) as f64
}
fn fit(font: &Sprite, text: &str, width: i32) -> String {
    let mut s = text.to_string();
    if text_width(font, &s) > width {
        while !s.is_empty() && text_width(font, &format!("{s}...")) > width {
            s.pop();
        }
        s.push_str("...");
    }
    s
}
pub fn notice(c: &mut Canvas, font: &Sprite, text: &str) {
    c.rect((30, 335, 580, 65), [35, 44, 46, 255]);
    let mut line = String::new();
    let mut y = 340;
    for word in text.split_whitespace() {
        let next = format!("{line}{word} ");
        if text_width(font, &next) > 565 {
            c.text(font, &line, 36, y, Some([235, 225, 179]));
            line.clear();
            y += 13;
        }
        line.push_str(word);
        line.push(' ');
    }
    c.text(font, &line, 36, y, Some([235, 225, 179]));
}
pub fn hud(pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>, camera: &Camera, world: &World) {
    pixels.fill(0);
    let mut c = Canvas(pixels);
    let font = &sprites["SMLFONT.PIC"];
    c.rect((0, 0, WIDTH as i32, 37), [18, 26, 30, 230]);
    c.text(
        font,
        &format!(
            "{}   TERRAIN VIEWER     ALT {:.0} ft",
            world.theater.name.to_ascii_uppercase(),
            camera.position[1]
        ),
        12,
        8,
        Some([230, 234, 222]),
    );
    c.text(
        font,
        "Arrows: move   Shift: fast   Q/E: down/up   WASD: look   Esc: return",
        12,
        24,
        Some([210, 218, 220]),
    );
    c.rect((8, HEIGHT as i32 - 25, 365, 19), [18, 26, 30, 225]);
    c.text(
        font,
        "Retail terrain / midday preview - environment parity in progress",
        13,
        HEIGHT as i32 - 20,
        Some([210, 218, 220]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> QuickMission {
        let mut options = Options {
            fields: vec![vec!["value".into(); 60]; 33],
            targets: vec![vec!["none".into(), "target".into()]; 16],
        };
        options.fields[4] = (0..6).map(|i| i.to_string()).collect();
        let mut q = QuickMission::new(AircraftId::F18, options, &BTreeMap::new());
        q.aircraft_names = vec!["Hornet".into(), "Rafale".into(), "Other".into()];
        q.aircraft_files = vec!["F18.PT".into(), "RAFALE.PT".into(), "OTHER.PT".into()];
        q
    }
    #[test]
    fn draft_cancel_and_pointer_release_are_transactional() {
        let mut q = setup();
        q.open(6);
        q.key("ArrowDown", false);
        assert_eq!(q.draft.values[6], 0);
        q.key("Escape", false);
        assert_eq!(q.draft.values[6], 0);
        q.open(6);
        q.key("ArrowDown", false);
        q.key("Enter", false);
        assert_eq!(q.player(), Some(AircraftId::Rafale));
        q.controls = vec![(4, (10, 10, 20, 20))];
        q.pointer(Some((15., 15.)));
        q.down();
        q.pointer(None);
        assert_eq!(q.up(), Action::None);
        assert_eq!(q.draft.values[4], 1);
    }
    #[test]
    fn source_dependencies_and_theater_identity_are_preserved() {
        let mut q = setup();
        q.apply(4, 0);
        assert_eq!(q.draft.values[4], 1);
        q.draft.values[31] = 3;
        q.draft.values[32] = 3;
        q.apply(30, 0);
        assert_eq!(&q.draft.values[30..33], &[0, 0, 0]);
        for i in 0..16 {
            q.theater(i);
            assert_eq!(q.theater_index(), i);
        }
        q.apply(13, 3);
        assert_eq!(q.draft.values[20], 57);
        assert_eq!(q.draft.values[3], 0);
        q.apply(13, 9);
        assert_eq!(q.draft.values[20], 37);
    }
    #[test]
    fn placeholders_cannot_silently_launch_as_a_supported_mission() {
        let mut q = setup();
        assert!(q.unsupported().is_some());
        q.apply(21, 0);
        assert!(q.unsupported().is_none());
        q.apply(6, 2);
        assert!(q.unsupported().unwrap().contains("setup only"));
        q.apply(6, 0);
        q.apply(15, 6);
        assert!(q.unsupported().unwrap().contains("clear"));
        q.apply(15, 1);
        q.apply(30, 1);
        assert!(q.unsupported().unwrap().contains("Ground"));
    }
}
