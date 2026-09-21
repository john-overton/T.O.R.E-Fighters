//! Retail option values with an explicit editable setup; simulation capabilities
//! are validated separately. Popup frame/focus feedback are authored presentation.
use crate::{
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{aircraft::AircraftId, ui::creator::Options};
use tore_sim::ai::{
    AiError,
    experience::EnemySkillOverride,
    launch::{Side, WingId, WingLaunch, WingSelection, legacy_pairs, resolve_wings},
};
type Rect = (i32, i32, i32, i32);
const POPUP: Rect = (185, 100, 270, 370);
const ROWS: usize = 15;
const ROW_BASE: usize = 100;
const OK: usize = 1;
const CANCEL: usize = 2;
const POP_OK: usize = 70;
const POP_CANCEL: usize = 71;
const UP: usize = 72;
const DOWN: usize = 73;
#[derive(Clone, Debug)]
pub struct Draft {
    pub values: [usize; 35],
}
impl Default for Draft {
    fn default() -> Self {
        let mut values = [0; 35];
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
    start_modes: Vec<String>,
    airport_names: Vec<Vec<String>>,
    airport_objects: Vec<Vec<u32>>,
    selector: Option<usize>,
    cursor: usize,
    scroll: usize,
    controls: Vec<(usize, Rect)>,
    pub notice: Option<String>,
    pub help: bool,
    pub shift: bool,
}
/// Maps a creator condition onto the six recovered source weather choices.
/// The two lists are both recovered but the engine holds no table joining
/// them, so this match is by label: dawn, clear, cloudy, foggy, sunset and
/// night each name one choice. The editor omits the duplicate overcast label.
pub fn condition(value: usize) -> Option<usize> {
    Some(match value {
        0 => 3,
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 4,
        5 => 5,
        _ => return None,
    })
}

impl QuickMission {
    pub fn new(id: AircraftId, mut options: Options, data: &BTreeMap<String, Vec<u8>>) -> Self {
        // Keep the imported option inventory intact; only the editor list drops
        // the duplicate. Draft indices below refer to this six-row list.
        options.fields[15]
            .retain(|label| !label.trim_end_matches('.').eq_ignore_ascii_case("overcast"));
        // Metadata for the full retail catalog is also cached. Only expose the
        // exact aircraft identities whose flight profiles were imported.
        let mut catalog: Vec<(String, String)> = AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| {
                data.get(id.pt())
                    .and_then(|bytes| tore_formats::aircraft::Aircraft::parse(bytes).ok())
                    .is_some_and(|aircraft| aircraft.id == id.source())
            })
            .map(|id| (id.selection_key().to_string(), id.label().to_string()))
            .collect();
        catalog.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        let (aircraft_files, aircraft_names): (Vec<_>, Vec<_>) = catalog.into_iter().unzip();
        let selected = aircraft_files
            .iter()
            .position(|n| n == id.selection_key())
            .unwrap_or(0);
        let mut draft = Draft::default();
        for i in [6, 9, 12, 23, 26, 29] {
            draft.values[i] = selected;
        }
        let mut airport_names = Vec::new();
        let mut airport_objects = Vec::new();
        let mut definitions = BTreeMap::new();
        for code in source_theaters() {
            let name = format!("{code}.MM");
            let mut names = Vec::new();
            let mut ids = Vec::new();
            if let Some(bytes) = data.get(&name)
                && let Ok(layout) = tore_formats::mission::Layout::parse(&name, bytes)
            {
                for p in layout.placements {
                    let airport = *definitions.entry(p.object_type.clone()).or_insert_with(|| {
                        data.get(&p.object_type)
                            .and_then(|b| tore_formats::static_object::Definition::parse(b).ok())
                            .is_some_and(|d| {
                                d.main_shape.is_some()
                                    && d.callbacks.iter().any(|c| c == "_STRIPProc")
                            })
                    });
                    if airport {
                        names.push(p.name.unwrap_or(p.object_type));
                        ids.push(0x4000_0000 + p.key.ordinal);
                    }
                }
            }
            airport_names.push(names);
            airport_objects.push(ids);
        }
        Self {
            ordnance: None,
            start_modes: vec!["Airborne".into(), "Ground".into()],
            airport_names,
            airport_objects,
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
        if self.selection != index {
            self.draft.values[34] = 0;
        }
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
        } else if id == 33 {
            &self.start_modes
        } else if id == 34 {
            &self.airport_names[self.draft.values[13]]
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
    /// The six wing rows as the AI launch payload: side, aircraft, wing skill
    /// and one entry per member (`docs/spec/ai-experience.md`, "Experience
    /// channels"). Wing counts live in fields 4, 7, 10 (friendly) and 21, 24,
    /// 27 (enemy); each wing's skill is the next field and its aircraft the one
    /// after that. Friendly wing 1 loses one slot to the player. A wing whose
    /// aircraft choice does not resolve to a supported import is skipped, which
    /// `unsupported` reports separately before a mission may start.
    pub fn wing_launches(
        &self,
        enemy_override: Option<EnemySkillOverride>,
    ) -> Result<Vec<WingLaunch>, AiError> {
        let mut selections = Vec::with_capacity(6);
        for (field, side, index) in [
            (4, Side::Friendly, 0),
            (7, Side::Friendly, 1),
            (10, Side::Friendly, 2),
            (21, Side::Enemy, 0),
            (24, Side::Enemy, 1),
            (27, Side::Enemy, 2),
        ] {
            let Some(aircraft) = self
                .aircraft_files
                .get(self.draft.values[field + 2])
                .and_then(|name| AircraftId::parse(name).ok())
            else {
                continue;
            };
            selections.push(WingSelection {
                wing: WingId::new(side, index)?,
                aircraft,
                count: self.draft.values[field].saturating_sub(usize::from(field == 4)),
                skill_level: self.draft.values[field + 1] as i32,
            });
        }
        resolve_wings(&selections, enemy_override)
    }
    /// The aircraft/count pairs the current mission spawner still takes. This
    /// is the launch payload with side, member and experience dropped; it stays
    /// for the existing spawner call and must not grow new callers.
    ///
    /// `fitted`: an out-of-range wing skill or count would make the payload an
    /// error, and this infallible signature has nowhere to put one, so it
    /// launches nothing. Rule: the decoded setup screen offers exactly four
    /// skills and counts 0 through 5 per wing (`docs/formats/quick-mission.md`),
    /// so the menu cannot reach that state; the empty result is a visible
    /// failure rather than a silent clamp if it ever does.
    pub fn dummy_wings(&self) -> Vec<(AircraftId, usize)> {
        self.wing_launches(None)
            .map(|wings| legacy_pairs(&wings))
            .unwrap_or_default()
    }
    pub fn ground_start(&self) -> bool {
        self.draft.values[33] == 1
    }
    pub fn ground_runway(&self) -> Option<u32> {
        self.ground_start()
            .then(|| {
                self.airport_objects[self.draft.values[13]]
                    .get(self.draft.values[34])
                    .copied()
            })
            .flatten()
    }
    pub fn choose_ground_runway(&mut self, object: u32) -> Result<(), String> {
        let index = self.airport_objects[self.draft.values[13]]
            .iter()
            .position(|id| *id == object)
            .ok_or_else(|| "No imported runway matches the chosen airport".to_string())?;
        self.apply(33, 1);
        self.apply(34, index);
        Ok(())
    }
    pub fn separation_feet(&self) -> f64 {
        [1., 2., 5., 10., 20., 50.][self.draft.values[17]] * 5280.
    }
    pub fn unsupported(&self) -> Option<String> {
        if self.player().is_none() {
            return Some(
                "This aircraft is available for setup only. Choose an imported aircraft from the player list to fly."
                    .into(),
            );
        }
        if self.ground_start() && self.ground_runway().is_none() {
            return Some(
                "No imported runways are available in this theater. Choose Airborne.".into(),
            );
        }
        let v = &self.draft.values;
        for field in [4, 7, 10, 21, 24, 27] {
            if v[field] > 0
                && self
                    .aircraft_files
                    .get(v[field + 2])
                    .and_then(|name| AircraftId::parse(name).ok())
                    .is_none()
            {
                return Some(
                    "Choose a supported imported aircraft for every populated wing.".into(),
                );
            }
        }
        if v[30] != 0 || v[31] != 0 || v[32] != 0 {
            return Some(
                "Ground targets and defenses are not available yet. Select none to fly.".into(),
            );
        }
        if condition(v[15]).is_none() {
            return Some("Choose one of the six available weather conditions.".into());
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
            self.draft.values[34] = 0;
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
        self.scroll = self.cursor / ROWS * ROWS;
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
            "ground-start" => self.apply(33, 1),
            "airports" => {
                self.apply(33, 1);
                self.open(34);
            }
            _ => {
                let id=name.strip_prefix("field-").and_then(|v|v.parse::<usize>().ok()).filter(|v|(3..35).contains(v)).ok_or("snapshot states: normal, aircraft, theaters, help, field-3 through field-34")?;
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
                    if self.values(field).is_empty() {
                        return Action::None;
                    }
                    self.apply(field, self.cursor);
                    self.selector = None;
                    self.focus = field;
                }
                POP_CANCEL => {
                    self.selector = None;
                    self.focus = field;
                }
                UP => self.scroll = self.scroll.saturating_sub(ROWS),
                DOWN => {
                    self.scroll = (self.scroll + ROWS)
                        .min(self.values(field).len().saturating_sub(1) / ROWS * ROWS)
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
                self.notice=Some("Aircraft era filters are not available yet. The list shows only supported imported aircraft.".into());
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
            3..=34 => {
                if id == 34 && !self.ground_start() {
                    return Action::None;
                }
                self.focus = id;
                if matches!(id, 6 | 9 | 12 | 13 | 23 | 26 | 29 | 34) ^ self.shift {
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
            self.scroll = self.cursor / ROWS * ROWS;
            return Action::None;
        }
        match key {
            "Tab" | "ArrowDown" | "ArrowUp" => {
                let backwards = shift || key == "ArrowUp";
                let last = if self.ground_start() { 34 } else { 33 };
                self.focus = if backwards {
                    if self.focus <= 1 {
                        last
                    } else {
                        self.focus - 1
                    }
                } else {
                    self.focus % last + 1
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
        let font = &sprites["QUICKFONT"];
        c.centered_text(
            &sprites["MENUFONT.PIC"],
            "Aircraft",
            (
                103,
                38,
                text_width(&sprites["MENUFONT.PIC"], "Aircraft"),
                20,
            ),
        );
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
                    (
                        if self.ground_start() {
                            "Airborne wings at "
                        } else {
                            "You are at "
                        },
                        None,
                    ),
                    ("", Some(14)),
                    (" feet. It is ", None),
                    ("", Some(15)),
                ],
            ),
            (249, vec![("Your situation is ", None), ("", Some(16))]),
            (
                263,
                vec![
                    ("You are ", None),
                    ("", Some(17)),
                    (" from enemy forces.", None),
                ],
            ),
            (291, vec![("You are carrying ", None), ("", Some(18))]),
            (305, vec![("Air combat is with ", None), ("", Some(19))]),
        ] {
            self.line(&mut c, font, 35, y, &parts);
        }
        self.line(&mut c, font, 35, 277, &[("Start: ", None), ("", Some(33))]);
        if self.ground_start() {
            self.line(
                &mut c,
                font,
                35,
                319,
                &[("Airport: ", None), ("", Some(34))],
            );
        }
        let (mut x, mut y) = (340, 221);
        for (text, id) in [
            ("Friendly ground target is ", None),
            ("", Some(30)),
            ("", Some(31)),
            ("defended by AAA and ", None),
            ("", Some(32)),
            ("defended by SAMs.", None),
        ] {
            let text = id.map(|i| self.value(i)).unwrap_or_else(|| text.into());
            for word in text.split_whitespace() {
                let width = text_width(font, word);
                if x + width > 605 {
                    x = 340;
                    y += 14;
                }
                if let Some(id) = id {
                    let rect = (
                        x - 1,
                        y - 1,
                        width + 2,
                        font.glyphs.iter().map(|g| g[2]).max().unwrap_or(9) as i32 + 2,
                    );
                    self.controls.push((id, rect));
                    c.rect(
                        rect,
                        if self.hover == Some(id) {
                            [127, 139, 144, 255]
                        } else {
                            [101, 107, 109, 255]
                        },
                    );
                    bevel(&mut c, rect, false);
                }
                c.text(font, word, x, y, None);
                x += width + text_width(font, " ") + if id.is_some() { 2 } else { 0 };
            }
        }
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
            // Reuse the original metal panel texture, inset list wells and rocker.
            let background = &sprites["QUIKMIS3.PIC"];
            for y in 0..POPUP.3 {
                for x in 0..POPUP.2 {
                    let source = (((140 + y % 240) as usize * WIDTH) + (50 + x % 240) as usize) * 4;
                    c.rect(
                        (POPUP.0 + x, POPUP.1 + y, 1, 1),
                        background.rgba[source..source + 4].try_into().unwrap(),
                    );
                }
            }
            bevel(&mut c, POPUP, false);
            for row in 0..ROWS {
                let y = 116 + row as i32 * 18;
                c.rect((207, y, 226, 14), [12, 16, 16, 255]);
                bevel(&mut c, (207, y, 226, 14), true);
                let Some(text) = self.values(field).get(self.scroll + row) else {
                    continue;
                };
                stripe(
                    &mut c,
                    (209, y + 1, 11, 12),
                    self.cursor == self.scroll + row,
                );
                c.text(font, &fit(font, text, 208), 223, y + 2, None);
            }
            for row in 0..ROWS.min(self.values(field).len().saturating_sub(self.scroll)) {
                self.controls
                    .push((ROW_BASE + row, (207, 116 + row as i32 * 18, 226, 14)));
            }
            c.text(font, "PAGE", 277, 394, None);
            c.rect((307, 389, 50, 17), [12, 16, 16, 255]);
            bevel(&mut c, (307, 389, 50, 17), true);
            c.text(
                font,
                &format!(
                    "{} of {}",
                    self.scroll / ROWS + 1,
                    self.values(field).len().div_ceil(ROWS).max(1)
                ),
                314,
                394,
                None,
            );
            c.text(font, "PREV", 380, 394, None);
            c.text(font, "NEXT", 380, 417, None);
            let rocker = &sprites["ROCKER00.PIC"];
            c.blit(rocker, (410, 393), 0, rocker.width, 1.);
            self.controls
                .extend([(UP, (410, 393, 18, 17)), (DOWN, (410, 410, 18, 17))]);
            if self.values(field).is_empty() {
                c.text(font, "No available choices.", 209, 118, None);
            }
            self.button(&mut c, sprites, POP_OK, "OK", (217, 437, 85, 24));
            self.button(&mut c, sprites, POP_CANCEL, "Cancel", (312, 437, 85, 24));
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
            if id.is_some() {
                x += 2;
            }
            let text = fit(font, &text, if x < 320 { 299 - x } else { 605 - x });
            let width = text_width(font, &text);
            if let Some(id) = id {
                let r = (
                    x - 1,
                    y - 1,
                    width + 2,
                    font.glyphs.iter().map(|g| g[2]).max().unwrap_or(9) as i32 + 2,
                );
                self.controls.push((*id, r));
                c.rect(
                    r,
                    if self.hover == Some(*id) {
                        [127, 139, 144, 255]
                    } else {
                        [101, 107, 109, 255]
                    },
                );
                bevel(c, r, false);
            }
            c.text(font, &text, x, y, None);
            x += width + if id.is_some() { 2 } else { 0 };
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
        let hit = c.action_button(
            sprites,
            label,
            (r.0, r.1, r.2),
            id == OK || id == POP_OK,
            self.pressed == Some(id),
        );
        self.controls.push((id, hit));
    }
}
fn bevel(c: &mut Canvas, (x, y, w, h): Rect, inset: bool) {
    let light = [115, 120, 119, 255];
    let dark = [24, 27, 27, 255];
    let (top, bottom) = if inset { (dark, light) } else { (light, dark) };
    c.rect((x, y, w, 1), top);
    c.rect((x, y, 1, h), top);
    c.rect((x, y + h - 1, w, 1), bottom);
    c.rect((x + w - 1, y, 1, h), bottom);
}
fn stripe(c: &mut Canvas, (x, y, w, h): Rect, selected: bool) {
    // Fitted diagonal status marker, using the reference's blue/gold treatment.
    let colors = if selected {
        [[230, 181, 39, 255], [73, 53, 17, 255]]
    } else {
        [[180, 193, 215, 255], [58, 94, 168, 255]]
    };
    for yy in 0..h {
        for xx in 0..w {
            c.rect(
                (x + xx, y + yy, 1, 1),
                colors[((xx - yy).rem_euclid(6) / 3) as usize],
            );
        }
    }
}
fn source_theaters() -> [&'static str; 16] {
    [
        "BAL", "CUB", "EGY", "LFA", "FRA", "GRE", "IRA", "KURILE", "TVIET", "SPA", "APA", "PGU",
        "NSK", "WTA", "UKR", "VLA",
    ]
}
pub fn runway_pose(world: &World, object: u32) -> crate::AppResult<([f64; 3], f64)> {
    let runway = world
        .airport_scene
        .runway(object)
        .ok_or("Selected runway is unavailable; choose another airport.")?;
    let (position, heading) = runway.departure_pose();
    if !runway.surface.contains_horizontal(position[0], position[2]) {
        return Err("Selected runway has no supported departure point".into());
    }
    Ok((position, heading))
}
pub fn apply_ground_start(
    world: &World,
    flight: &mut tore_sim::flight::State,
    object: u32,
) -> crate::AppResult<u32> {
    let (mut position, heading) = runway_pose(world, object)?;
    let surface = world.surface(position[0], position[2]);
    if !surface.landable {
        return Err("Selected runway does not provide a ground surface".into());
    }
    position[1] = surface.height;
    let mut candidate = flight.clone();
    candidate.start_on_runway(position, heading)?;
    if world
        .solid_contact(
            candidate.position,
            candidate.position,
            world.airport_scene.objects.iter().map(|o| o.id),
        )
        .is_some()
    {
        return Err("The runway start is obstructed. Choose another airport.".into());
    }
    *flight = candidate;
    Ok(world.airport_scene.runway(object).unwrap().airport)
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
        options.fields[15] = [
            "dawn",
            "clear",
            "cloudy",
            "overcast.",
            "foggy",
            "sunset",
            "night",
        ]
        .map(String::from)
        .to_vec();
        options.fields[4] = (0..6).map(|i| i.to_string()).collect();
        let mut q = QuickMission::new(AircraftId::F18, options, &BTreeMap::new());
        q.aircraft_names = vec!["Hornet".into(), "Rafale".into(), "Other".into()];
        q.aircraft_files = vec!["F18.PT".into(), "RAFALE.PT".into(), "OTHER.PT".into()];
        q
    }
    #[test]
    fn ground_start_airport_picker_cancels_and_resets_on_theater_change() {
        let mut q = setup();
        q.airport_names[0] = vec!["First Field".into(), "Second Field".into()];
        q.airport_objects[0] = vec![0x40000000, 0x40000003];
        assert!(!q.ground_start());
        q.activate(33);
        assert!(q.ground_start());
        q.activate(34);
        assert_eq!(q.selector, Some(34));
        q.activate(ROW_BASE + 1);
        q.activate(POP_CANCEL);
        assert_eq!(q.ground_runway(), Some(0x40000000));
        q.activate(34);
        q.activate(ROW_BASE + 1);
        q.activate(POP_OK);
        assert_eq!(q.ground_runway(), Some(0x40000003));
        q.apply(13, 1);
        assert_eq!(q.draft.values[34], 0);
        assert!(q.ground_runway().is_none());
        assert!(q.unsupported().unwrap().contains("No imported runways"));
        q.apply(33, 0);
        assert!(q.unsupported().is_none());
    }
    #[test]
    fn tab_reaches_ground_controls_without_focusing_hidden_airport() {
        let mut q = setup();
        q.focus = 32;
        q.key("Tab", false);
        assert_eq!(q.focus, 33);
        q.key("Tab", false);
        assert_eq!(q.focus, 1);
        q.apply(33, 1);
        q.focus = 33;
        q.key("Tab", false);
        assert_eq!(q.focus, 34);
        q.key("Tab", false);
        assert_eq!(q.focus, 1);
    }
    #[test]
    fn selectors_page_and_cancel_without_changing_the_draft() {
        let mut q = setup();
        q.activate(13);
        assert_eq!(q.selector, Some(13));
        q.activate(DOWN);
        assert_eq!(q.scroll, 15);
        q.activate(ROW_BASE);
        assert_eq!(q.cursor, 15);
        q.activate(POP_CANCEL);
        assert_eq!(q.draft.values[13], 0);
        q.open(13);
        q.key("PageDown", false);
        assert_eq!((q.scroll, q.cursor), (15, 15));
        q.key("Enter", false);
        assert_eq!(q.draft.values[13], 15);
        q.aircraft_names.clear();
        q.aircraft_files.clear();
        q.open(6);
        assert_eq!(q.activate(POP_OK), Action::None);
        assert!(q.player().is_none());
    }
    #[test]
    fn catalog_metadata_and_variant_aliases_do_not_create_imported_aircraft() {
        let options = setup().options;
        let data = ["F18C.PT", "RAFALEE.PT", "RAFALEF.PT", "OTHER.PT", "F18.PT"]
            .into_iter().map(|name| (name.to_string(), format!(
                "[brent's_relocatable_format]\n:ot_names\nstring \"Plane\"\nstring \"Planes\"\nstring \"{name}\"\nend\n"
            ).into_bytes())).collect();
        let q = QuickMission::new(AircraftId::F18, options, &data);
        assert!(q.aircraft_files.is_empty());
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
    fn all_wings_launch_with_the_selected_identities() {
        let mut q = setup();
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        q.apply(23, 1);
        assert!(q.unsupported().is_none());
        let wings = q.dummy_wings();
        assert_eq!(wings.iter().map(|(_, n)| n).sum::<usize>(), 29);
        assert_eq!(wings[0], (AircraftId::F18, 4));
        assert_eq!(wings[3], (AircraftId::Rafale, 5));
        q.apply(23, 2);
        assert!(q.unsupported().unwrap().contains("populated wing"));
    }
    #[test]
    fn wing_skill_selections_reach_the_launch_payload() {
        use tore_sim::ai::Experience;
        use tore_sim::ai::experience::ExperienceOrigin;
        let mut q = setup();
        // Every wing populated, with a different skill per wing row.
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        for (field, level) in [(5, 0), (8, 1), (11, 2), (22, 3), (25, 0), (28, 1)] {
            q.apply(field, level);
        }
        let wings = q.wing_launches(None).unwrap();
        assert_eq!(wings.len(), 6);
        let levels = [
            Experience::Novice,
            Experience::Average,
            Experience::Experienced,
            Experience::Ace,
            Experience::Novice,
            Experience::Average,
        ];
        for (wing, level) in wings.iter().zip(levels) {
            assert_eq!(wing.selected_level, level);
            assert!(!wing.is_empty());
            for member in &wing.members {
                assert_eq!(member.experience.level, level);
                assert_eq!(
                    member.experience.origin,
                    ExperienceOrigin::QuickMission { selected: level }
                );
            }
            assert!(wing.leader().is_some_and(|m| m.member == 0));
        }
        // Sides and the player's slot in friendly wing 1 are preserved.
        assert_eq!(wings[0].count(), 4);
        assert_eq!(wings[3].count(), 5);
        let sides: Vec<bool> = wings.iter().map(|w| w.wing.side.is_enemy()).collect();
        assert_eq!(sides, [false, false, false, true, true, true]);
        assert_eq!(
            wings.iter().map(|w| w.wing.index).collect::<Vec<_>>(),
            [0, 1, 2, 0, 1, 2]
        );
    }
    #[test]
    fn the_enemy_skill_override_changes_enemy_wings_only() {
        use tore_sim::ai::Experience;
        use tore_sim::ai::experience::ExperienceOrigin;
        let mut q = setup();
        for field in [4, 7, 10, 21, 24, 27] {
            q.apply(field, 5);
        }
        for field in [5, 8, 11, 22, 25, 28] {
            q.apply(field, 3);
        }
        let wings = q
            .wing_launches(Some(EnemySkillOverride::AllNovice))
            .unwrap();
        for wing in &wings {
            assert_eq!(wing.selected_level, Experience::Ace);
            let enemy = wing.wing.side.is_enemy();
            for member in &wing.members {
                if enemy {
                    assert_eq!(member.experience.level, Experience::Novice);
                    assert_eq!(member.experience.origin, ExperienceOrigin::EnemyOverride);
                } else {
                    assert_eq!(member.experience.level, Experience::Ace);
                    assert_eq!(
                        member.experience.origin,
                        ExperienceOrigin::QuickMission {
                            selected: Experience::Ace
                        }
                    );
                }
            }
        }
        // The legacy pairs are unchanged by the override.
        assert_eq!(legacy_pairs(&wings), q.dummy_wings());
    }
    #[test]
    fn placeholders_cannot_silently_launch_as_a_supported_mission() {
        let mut q = setup();
        assert!(q.unsupported().is_none());
        assert_eq!(q.dummy_wings().iter().map(|(_, n)| n).sum::<usize>(), 2);
        q.apply(21, 0);
        assert!(q.unsupported().is_none());
        q.apply(6, 2);
        assert!(q.unsupported().unwrap().contains("setup only"));
        q.apply(6, 0);
        // Every editor row maps to its intended source weather, including night.
        assert_eq!(
            q.values(15),
            ["dawn", "clear", "cloudy", "foggy", "sunset", "night"]
        );
        for (value, source) in [3, 0, 1, 2, 4, 5].into_iter().enumerate() {
            assert_eq!(condition(value), Some(source));
            q.apply(15, value);
            assert!(q.unsupported().is_none(), "condition {value}");
        }
        q.apply(15, 6);
        assert!(q.unsupported().unwrap().contains("six available"));
        q.apply(15, 1);
        q.apply(30, 1);
        assert!(q.unsupported().unwrap().contains("Ground"));
    }
}
