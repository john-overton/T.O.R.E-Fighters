//! Retail art and recovered card/control geometry over a transactional loadout.
use crate::rocker::Rocker;
use crate::{
    AppResult,
    menu::{Action, Canvas, Sprite, text_width},
    quick_mission::notice,
};
use std::collections::BTreeMap;
use std::time::Instant;
use tore_formats::{Pic, weapons::Weapon};
use tore_sim::{
    combat::loadout::{Loadout, supported},
    models::FlightModel,
};
type Rect = (i32, i32, i32, i32);
const CATALOG: Rect = (64, 103, 235, 272);
const CATALOG_AREA: usize = 15;
const CARD_WIDTH: i32 = 109;
const CARD_HEIGHT: i32 = 23;
#[derive(Clone, Copy)]
enum DragSource {
    Catalog(usize),
    Station(usize),
}
struct Drag {
    source: DragSource,
    origin: (f64, f64),
    moved: bool,
}
pub struct Ordnance {
    pub loadout: Loadout,
    pub visible: bool,
    /// Every imported weapon; the catalog applies flight support and Cheat rules.
    weapons: Vec<Weapon>,
    catalog: Vec<Weapon>,
    sprites: BTreeMap<String, Sprite>,
    category: usize,
    pages: [usize; 2],
    selected: Option<usize>,
    station: usize,
    hover: Option<usize>,
    pressed: Option<usize>,
    right_pressed: Option<usize>,
    /// Page rocker, then fuel rocker.
    rockers: [Rocker; 2],
    pointer: Option<(f64, f64)>,
    drag: Option<Drag>,
    controls: Vec<(usize, Rect)>,
    pub message: Option<String>,
    menu: bool,
}
impl Ordnance {
    pub fn new(mut loadout: Loadout, data: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let background = Pic::parse(
            data.get("ORD_AIR3.PIC")
                .ok_or("missing ordnance background; re-import media")?,
        )?;
        let palette: [[u8; 3]; 256] = background
            .palette
            .clone()
            .try_into()
            .map_err(|_| "ordnance palette missing")?;
        let mut sprites = BTreeMap::new();
        for (name, bytes) in data.iter().filter(|(n, _)| {
            n.ends_with(".PIC")
                && (n.starts_with('$')
                    || n.starts_with("ACTION0")
                    || n.starts_with("ACTDFT0")
                    || n.starts_with("ROCKER0")
                    || [
                        "ORD_AIR3.PIC",
                        "DIAL00.PIC",
                        "DIAL04.PIC",
                        "DIAL11.PIC",
                        "DIAL13.PIC",
                        "LIGHTON.PIC",
                        "LIGHTOFF.PIC",
                        "ACTDFLT.PIC",
                        "PANELFNT.PIC",
                        "ARMFONT.PIC",
                        "SMLFONT.PIC",
                        "MENUFONT.PIC",
                        "FONTACT.PIC",
                        "FNTWPNB.PIC",
                        "FNTWPNY.PIC",
                    ]
                    .contains(&n.as_str()))
        }) {
            let p = Pic::parse(bytes)?;
            sprites.insert(
                name.clone(),
                Sprite {
                    width: p.width,
                    height: p.height,
                    rgba: p.rgba(&palette),
                    glyphs: p.glyphs,
                },
            );
        }
        for (name, color) in [
            ("QUICKFONT", [232, 233, 230]),
            ("WPNBLUE", [42, 124, 217]),
            ("WPNYELLOW", [255, 211, 38]),
        ] {
            sprites.insert(name.into(), crate::menu::flat_font(color));
        }
        let mut weapons: Vec<_> = data
            .iter()
            .filter(|(n, _)| n.ends_with(".JT") && !n.starts_with('~'))
            .filter_map(|(name, b)| Weapon::parse(name, b).ok())
            .collect();
        let display = |w: &mut Weapon| {
            if let Some(bytes) = data.get(&w.source)
                && let Ok(brf) = tore_formats::aircraft::Brf::parse(bytes)
                && let Ok(names) = brf.strings("si_names")
                && let Some(long) = names.get(1)
                && text_width(&sprites["QUICKFONT"], long) <= 111
            {
                w.name = long.clone();
            }
        };
        for w in &mut weapons {
            display(w);
        }
        for station in &mut loadout.configuration.stations {
            display(&mut station.weapon);
        }
        weapons.sort_by(|a, b| a.name.cmp(&b.name).then(a.source.cmp(&b.source)));
        let mut ordnance = Self {
            loadout,
            visible: true,
            weapons,
            catalog: vec![],
            sprites,
            category: 0,
            pages: [0; 2],
            selected: None,
            station: 0,
            hover: None,
            pressed: None,
            right_pressed: None,
            rockers: Default::default(),
            pointer: None,
            drag: None,
            controls: vec![],
            message: None,
            menu: false,
        };
        ordnance.rebuild_catalog();
        Ok(ordnance)
    }
    fn rebuild_catalog(&mut self) {
        let load = &self.loadout;
        self.catalog = self
            .weapons
            .iter()
            .filter(|w| supported(&w.source))
            .filter(|w| {
                load.cheat
                    || (0..load.hardpoints.len()).any(|i| (1..32767).contains(&load.capacity(i, w)))
            })
            .cloned()
            .collect();
        self.pages = [0; 2];
        self.selected = None;
    }
    fn page_entries(&self) -> Vec<usize> {
        self.catalog
            .iter()
            .enumerate()
            .filter(|(_, w)| usize::from(w.flags & 0x10000 == 0) == self.category)
            .map(|(i, _)| i)
            .collect()
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        self.pointer = p;
        if let (Some(drag), Some(p)) = (&mut self.drag, p) {
            drag.moved |= (p.0 - drag.origin.0).powi(2) + (p.1 - drag.origin.1).powi(2) >= 9.;
        }
        if p.is_none() {
            self.drag = None;
            self.pressed = None;
            self.right_pressed = None;
        }
        self.hover = p.and_then(|p| {
            self.controls
                .iter()
                .rev()
                .find(|(_, r)| {
                    p.0 >= r.0 as f64
                        && p.0 < (r.0 + r.2) as f64
                        && p.1 >= r.1 as f64
                        && p.1 < (r.1 + r.3) as f64
                })
                .map(|(i, _)| *i)
        });
    }
    pub fn dragging(&self) -> bool {
        self.drag.as_ref().is_some_and(|drag| drag.moved)
    }
    /// The page (1, 2) and fuel (5, 6) rockers act on press and tilt while
    /// held, as retail rockers do.
    pub fn down(&mut self) -> Action {
        self.right_pressed = None;
        self.pressed = self.hover;
        if let Some(id @ (1 | 2 | 5 | 6)) = self.hover.filter(|_| !self.menu) {
            return self.rock(id, true);
        }
        let source = match self.hover {
            Some(id @ 100..=107) => self.card_index(id).map(DragSource::Catalog),
            Some(id @ 200..=231) if self.loadout.quantities[id - 200] > 0 => {
                Some(DragSource::Station(id - 200))
            }
            _ => None,
        };
        self.drag = source.zip(self.pointer).map(|(source, origin)| Drag {
            source,
            origin,
            moved: false,
        });
        Action::None
    }
    /// Tilts rocker control `id` and applies it. Page turns sound the rocker;
    /// fuel keeps its own cue.
    fn rock(&mut self, id: usize, held: bool) -> Action {
        self.rockers[usize::from(id > 2)].push(matches!(id, 2 | 6), held, Instant::now());
        match self.activate(id) {
            Action::Click => Action::RockerDown,
            action => action,
        }
    }
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.right_pressed = None;
        for rocker in &mut self.rockers {
            if rocker.held() {
                rocker.release(Instant::now());
            }
        }
        self.hover = None;
        self.pointer = None;
        self.drag = None;
        self.menu = false;
    }
    pub fn up(&mut self) -> Action {
        let p = self.pressed.take();
        if let Some(id @ (1 | 2 | 5 | 6)) = p {
            let rocker = &mut self.rockers[usize::from(id > 2)];
            if rocker.held() {
                rocker.release(Instant::now());
                return Action::RockerUp;
            }
        }
        if let Some(drag) = self.drag.take().filter(|drag| drag.moved) {
            self.message = None;
            if let Some(target) = self.hover.filter(|id| (200..232).contains(id)) {
                self.station = target - 200;
                let station = self.station;
                return match drag.source {
                    DragSource::Catalog(index) => {
                        self.selected = Some(index);
                        let weapon = self.catalog[index].clone();
                        self.edit_loadout(|load| load.select(station, weapon))
                    }
                    DragSource::Station(source) => {
                        self.selected = None;
                        self.edit_loadout(|load| load.transfer(source, station))
                    }
                };
            }
            if let DragSource::Station(source) = drag.source
                && self
                    .hover
                    .is_some_and(|id| id == CATALOG_AREA || (100..108).contains(&id))
            {
                self.selected = None;
                self.station = source;
                return self.edit_loadout(|load| {
                    load.quantities[source] = 0;
                    Ok(())
                });
            }
            return Action::None;
        }
        if let (Some(source), Some(target)) = (p, self.hover) {
            if (100..108).contains(&source) && (200..232).contains(&target) {
                self.select_card(source);
                return self.activate(target);
            }
            if source == target {
                return self.activate(target);
            }
        }
        Action::None
    }
    fn select_card(&mut self, id: usize) {
        self.selected = self.card_index(id);
    }
    fn card_index(&self, id: usize) -> Option<usize> {
        self.page_entries()
            .get(self.pages[self.category] * 8 + id - 100)
            .copied()
    }
    pub fn right(&mut self, down: bool) -> Action {
        if down {
            self.pressed = None;
            self.drag = None;
            self.right_pressed = self.hover;
            return Action::None;
        }
        let pressed = self.right_pressed.take();
        if let Some(id) = self
            .hover
            .filter(|i| Some(*i) == pressed && (200..232).contains(i))
        {
            self.station = id - 200;
            let station = self.station;
            self.edit_loadout(|load| {
                load.change(station, -1);
                Ok(())
            })
        } else {
            Action::None
        }
    }
    /// Sound follows a completed edit, independent of the input gesture.
    fn edit_loadout(
        &mut self,
        edit: impl FnOnce(&mut Loadout) -> tore_formats::Result<()>,
    ) -> Action {
        let before: Vec<_> = self
            .loadout
            .configuration
            .stations
            .iter()
            .zip(&self.loadout.quantities)
            .map(|(station, count)| (station.weapon.source.clone(), station.weapon.flags, *count))
            .collect();
        let fuel = self.loadout.fuel_lbs;
        self.message = None;
        if let Err(error) = edit(&mut self.loadout) {
            self.message = Some(error.to_string());
            return Action::None;
        }
        for ((station, count), (source, old_flags, old_count)) in self
            .loadout
            .configuration
            .stations
            .iter()
            .zip(&self.loadout.quantities)
            .zip(before)
        {
            if *count != old_count || station.weapon.source != source {
                let flags = if *count == 0 {
                    old_flags
                } else {
                    station.weapon.flags
                };
                return if flags & 0x80 != 0 {
                    Action::OrdnanceAmmunition
                } else {
                    Action::OrdnanceWeapon
                };
            }
        }
        if self.loadout.fuel_lbs != fuel {
            Action::OrdnanceFuel
        } else {
            Action::None
        }
    }
    fn activate(&mut self, id: usize) -> Action {
        self.message = None;
        match id {
            1 => self.pages[self.category] = self.pages[self.category].saturating_sub(1),
            2 => {
                self.pages[self.category] = (self.pages[self.category] + 1)
                    .min(self.page_entries().len().saturating_sub(1) / 8)
            }
            3 | 4 => {
                self.category = id - 3;
                self.selected = None;
            }
            5 | 6 => {
                return self.edit_loadout(|load| {
                    load.fuel(id == 5);
                    Ok(())
                });
            }
            7 => {
                if let Err(e) = self.loadout.validate() {
                    self.message = Some(e.to_string());
                } else {
                    return Action::MissionFly;
                }
            }
            8 => {
                self.visible = false;
                self.cancel();
            }
            9 | 10 => {
                let station = self.station;
                return self.edit_loadout(|load| {
                    load.change(station, if id == 9 { 1 } else { -1 });
                    Ok(())
                });
            }
            11 => self.menu = !self.menu,
            12 => {
                self.loadout.quantities.fill(0);
                self.menu = false;
            }
            13 => {
                // Toggling unloads every station, then rebuilds the catalog.
                self.loadout.quantities.fill(0);
                self.loadout.cheat = !self.loadout.cheat;
                self.rebuild_catalog();
                self.menu = false;
                self.message = Some(
                    if self.loadout.cheat {
                        "Cheat loading on."
                    } else {
                        "Cheat loading off."
                    }
                    .into(),
                );
            }
            14 => {
                self.message =
                    Some("Airbase aircraft cycling is not available in this setup.".into())
            }
            100..=107 => self.select_card(id),
            200..=231 => {
                self.station = id - 200;
                let station = self.station;
                if self.loadout.quantities[station] > 0 {
                    self.selected = None;
                    return self.edit_loadout(|load| {
                        let capacity =
                            load.capacity(station, &load.configuration.stations[station].weapon);
                        if i32::from(load.quantities[station]) < capacity {
                            load.quantities[station] += 1;
                        }
                        Ok(())
                    });
                } else if let Some(w) = self.selected {
                    let weapon = self.catalog[w].clone();
                    return self.edit_loadout(|load| load.select(station, weapon));
                }
                return Action::None;
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str) -> Action {
        if self.drag.is_some() {
            if key == "Escape" {
                self.cancel();
            }
            return Action::None;
        }
        match key {
            "Escape" => {
                if self.menu {
                    self.menu = false;
                } else {
                    self.visible = false;
                    self.cancel();
                }
                Action::None
            }
            "Enter" => self.activate(7),
            "+" | "=" => self.activate(9),
            "-" => self.activate(10),
            "ArrowRight" => self.rock(2, false),
            "ArrowLeft" => self.rock(1, false),
            "a" | "A" => self.activate(3),
            "s" | "S" => self.activate(4),
            "Tab" => {
                self.station = (self.station + 1) % self.loadout.quantities.len();
                self.selected = None;
                Action::None
            }
            _ => Action::None,
        }
    }
    /// Reproducible presentation fixtures for the existing menu snapshot command.
    pub fn preview(&mut self, state: &str) {
        match state {
            "ordnance-empty" => self.loadout.quantities.fill(0),
            "ordnance-drag" => {
                self.pointer(Some((100., 120.)));
                self.down();
                self.pointer(Some((325., 190.)));
            }
            _ => {}
        }
    }
    /// Draws the screen; true while a rocker is still moving.
    pub fn render(&mut self, pixels: &mut [u8]) -> bool {
        let now = Instant::now();
        let animating = self
            .rockers
            .iter_mut()
            .fold(false, |moving, rocker| rocker.advance(now) | moving);
        pixels.copy_from_slice(&self.sprites["ORD_AIR3.PIC"].rgba);
        self.controls.clear();
        self.controls.push((CATALOG_AREA, CATALOG));
        let mut c = Canvas(pixels);
        let font = &self.sprites["QUICKFONT"];
        let menu_font = &self.sprites["MENUFONT.PIC"];
        for (label, x) in [("?", 87), ("Weapons", 103), ("Airbase", 178)] {
            c.centered_text(menu_font, label, (x, 38, text_width(menu_font, label), 20));
        }
        self.controls
            .extend([(11, (103, 35, 75, 24)), (14, (178, 35, 90, 24))]);
        let title = self.loadout.aircraft.label();
        c.text(
            &self.sprites["ARMFONT.PIC"],
            title,
            466 - text_width(&self.sprites["ARMFONT.PIC"], title) / 2,
            101,
            None,
        );
        let entries = self.page_entries();
        let page = self.pages[self.category];
        for (row, index) in entries.iter().skip(page * 8).take(8).enumerate() {
            // Imported black wells are 113 pixels wide at x=66/185.
            let x = 68 + (row % 2) as i32 * 119;
            let y = 108 + (row / 2) as i32 * 68;
            self.controls.push((100 + row, (x - 4, y - 5, 115, 68)));
            card(
                &mut c,
                &self.sprites,
                &self.catalog[*index],
                x,
                y,
                self.selected == Some(*index),
                true,
            );
        }
        for (i, (s, n)) in self
            .loadout
            .configuration
            .stations
            .iter()
            .zip(&self.loadout.quantities)
            .enumerate()
        {
            let x = 350 + (i % 2) as i32 * 119;
            let y = 121 + (i / 2) as i32 * 71;
            self.controls.push((200 + i, (x - 1, y - 6, 115, 71)));
            let location = [
                "Centerline",
                "Fuselage",
                "Internal Gun",
                "Internal Bay",
                "Wing",
                "Wingtip",
            ]
            .get(self.loadout.hardpoints[i].location as usize)
            .copied()
            .unwrap_or("Station");
            c.text(font, location, x + 1, y, None);
            if *n > 0 {
                card(
                    &mut c,
                    &self.sprites,
                    &s.weapon,
                    x + 3,
                    y + 14,
                    self.station == i,
                    false,
                );
                let amount = if s.internal {
                    format!("{n} (max {})", self.loadout.capacity(i, &s.weapon))
                } else {
                    format!("{n} loaded (max {})", self.loadout.capacity(i, &s.weapon))
                };
                c.text(font, &amount, x + 3, y + 52, None);
            } else {
                // Imported black wells start at x+1; keep two black pixels
                // on each side of the 109-pixel outline, even when empty.
                card_outline(&mut c, x + 3, y + 14, false);
            }
        }
        let total = self.loadout.total_lbs();
        for (y, value) in [
            (351, self.loadout.maximum_lbs),
            (365, total),
            (381, self.loadout.maximum_lbs - total),
        ] {
            let text = format!("{} lbs", grouped(value));
            c.centered_text(
                font,
                &text,
                (
                    467 - text_width(font, &text),
                    y,
                    text_width(font, &text),
                    12,
                ),
            );
        }
        c.centered_text(
            font,
            &format!("{} lbs", grouped(self.loadout.fuel_lbs)),
            (487, 354, 56, 14),
        );
        // The original background already supplies the percent sign.
        c.centered_text(
            font,
            &format!(
                "{:.0}",
                100. * self.loadout.fuel_lbs / self.loadout.internal_capacity_lbs
            ),
            (487, 375, 27, 14),
        );
        c.centered_text(
            font,
            &format!("{} of {}", page + 1, entries.len().div_ceil(8).max(1)),
            (248, 388, 48, 15),
        );
        for (rocker, at) in self.rockers.iter().zip([(260, 408), (547, 356)]) {
            let sprite = &self.sprites[&rocker.sprite()];
            c.blit(sprite, at, 0, sprite.width, 1.);
        }
        self.controls.extend([
            (1, (260, 408, 18, 17)),
            (2, (260, 425, 18, 17)),
            (5, (547, 356, 18, 17)),
            (6, (547, 373, 18, 17)),
            (3, (68, 390, 160, 30)),
            (4, (68, 420, 160, 31)),
        ]);
        let dial = &self.sprites[if self.category == 0 {
            "DIAL13.PIC"
        } else {
            "DIAL11.PIC"
        }];
        c.blit(dial, (148, 393), 0, dial.width, 1.);
        for (category, y) in [(0, 394), (1, 422)] {
            let lamp = &self.sprites[if self.category == category {
                "LIGHTON.PIC"
            } else {
                "LIGHTOFF.PIC"
            }];
            c.blit(lamp, (115, y), 0, lamp.width, 1.);
        }
        for (id, label, r) in [
            (7, "Fly", (493, 414, 80, 24)),
            (8, "Select Plane", (363, 414, 100, 24)),
        ] {
            let hit = c.action_button(
                &self.sprites,
                label,
                (r.0, r.1, r.2),
                id == 7,
                self.pressed == Some(id),
            );
            self.controls.push((id, hit));
        }
        if self.menu {
            c.rect((103, 60, 230, 50), [210, 214, 211, 255]);
            c.text(&self.sprites["MENUFONT.PIC"], "Unload All", 108, 64, None);
            c.text(
                &self.sprites["MENUFONT.PIC"],
                if self.loadout.cheat {
                    "Cheat  On"
                } else {
                    "Cheat  Off"
                },
                108,
                89,
                None,
            );
            self.controls = vec![
                (11, (103, 35, 75, 24)),
                (12, (103, 60, 230, 25)),
                (13, (103, 85, 230, 25)),
            ];
        }
        if let Some(message) = &self.message {
            notice(&mut c, font, message);
        }
        if let Some(drag) = self.drag.as_ref().filter(|drag| drag.moved)
            && let Some((x, y)) = self.pointer
        {
            let weapon = match drag.source {
                DragSource::Catalog(index) => &self.catalog[index],
                DragSource::Station(index) => &self.loadout.configuration.stations[index].weapon,
            };
            if let Some(sprite) = thumbnail(&self.sprites, weapon) {
                c.blit(
                    sprite,
                    (
                        x as i32 - sprite.width as i32 / 2,
                        y as i32 - sprite.height as i32 / 2,
                    ),
                    0,
                    sprite.width,
                    1.,
                );
            }
        }
        animating
    }
}
fn grouped(value: f64) -> String {
    let digits = format!("{value:.0}");
    let mut result = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0
            && ch.is_ascii_digit()
            && (digits.len() - i).is_multiple_of(3)
            && digits.as_bytes()[i - 1].is_ascii_digit()
        {
            result.push(',');
        }
        result.push(ch);
    }
    result
}
fn card(
    c: &mut Canvas,
    sprites: &BTreeMap<String, Sprite>,
    w: &Weapon,
    x: i32,
    y: i32,
    selected: bool,
    catalog: bool,
) {
    card_outline(c, x, y, selected);
    if let Some(p) = thumbnail(sprites, w) {
        c.blit(
            p,
            (
                x + (CARD_WIDTH - p.width as i32) / 2,
                y - 1 + (CARD_HEIGHT - p.height as i32) / 2,
            ),
            0,
            p.width,
            1.,
        );
    }
    let font = &sprites[if selected { "WPNYELLOW" } else { "QUICKFONT" }];
    let mut name = w.name.clone();
    while text_width(font, &name) > 111 && !name.is_empty() {
        name.pop();
    }
    c.text(font, &name, x, y + 25, None);
    if catalog {
        let font = &sprites["WPNBLUE"];
        c.text(font, &format!("{} lbs", w.weight), x, y + 37, None);
        let guidance = if w.flags & 1 == 0 {
            "unguided"
        } else {
            match w.seeker.signature {
                0 => "optical",
                1 => "laser",
                2 => "IR",
                3 if w.flags & 0x200 != 0 => "SARH",
                3 => "active rdr",
                _ => "",
            }
        };
        c.text(
            font,
            guidance,
            x + 111 - text_width(font, guidance),
            y + 37,
            None,
        );
    }
}
fn thumbnail<'a>(sprites: &'a BTreeMap<String, Sprite>, weapon: &Weapon) -> Option<&'a Sprite> {
    sprites.get(&format!("${}.PIC", weapon.source.trim_end_matches(".JT")))
}
fn card_outline(c: &mut Canvas, x: i32, y: i32, selected: bool) {
    let edge = if selected {
        [201, 179, 49, 255]
    } else {
        [94, 30, 20, 255]
    };
    c.rect((x, y - 1, CARD_WIDTH, 1), edge);
    c.rect((x, y + CARD_HEIGHT - 2, CARD_WIDTH, 1), edge);
    c.rect((x, y - 1, 1, CARD_HEIGHT), edge);
    c.rect((x + CARD_WIDTH - 1, y - 1, 1, CARD_HEIGHT), edge);
}

/// Source-cache regression probe. No GPU, native execution, or invented targets.
pub fn validate_sources(
    data: &BTreeMap<String, Vec<u8>>,
    world: &crate::terrain::World,
) -> AppResult<()> {
    // Run editor/loadout checks for the complete roster before unrelated flight
    // appearance probes, so their failures cannot hide preflight coverage.
    for id in tore_formats::aircraft::AircraftId::SELECTABLE {
        let airframe = crate::aircraft::Airframe::load(data, id)?;
        let load = Loadout::new(&airframe.profile, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
        })?;
        let mut combat = crate::combat::Combat::with_loadout(&airframe, data, &load)?;
        combat.mission_dummies(&[(id, 29)], 5280., data)?;
        combat.reset(&mut airframe.start(world))?;
        validate_guns_only(&load, &airframe, &combat.state.targets, data, world)?;
        validate_dragging(&load, data)?;
        println!(
            "{}: guns-only across all six wings, player/wing restart, standard loads and ordnance dragging passed",
            id.label()
        );
    }
    for id in tore_formats::aircraft::AircraftId::SELECTABLE {
        let airframe = crate::aircraft::Airframe::load(data, id)?;
        let mut load = Loadout::new(&airframe.profile, |n| {
            data.get(n)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {n}")))
        })?;
        load.validate()?;
        let mut normal = crate::combat::Combat::new(&airframe, data, false)?;
        let mut flight = airframe.start(world);
        let mtow = flight.model().configuration().mass.max_takeoff_lbs;
        let wind_limits =
            tore_sim::runway_wind::limits(mtow).ok_or("invalid imported MTOW wind class")?;
        println!(
            "{id:?} runway wind: MTOW={mtow:.0} lb noticeable={} rough={} limit={} kt tailwind=10 kt",
            wind_limits.noticeable_knots, wind_limits.rough_knots, wind_limits.limit_knots
        );

        normal.reset(&mut flight)?;
        if normal.state.ammo != load.quantities || normal.range || !normal.state.targets.is_empty()
        {
            return Err("normal start lost default weapons or spawned a range target".into());
        }
        normal.state.ammo.fill(0);
        normal.reset(&mut flight)?;
        if normal.state.ammo != load.quantities {
            return Err("normal restart lost weapons".into());
        }
        normal.clean_recording = true;
        normal.reset(&mut flight)?;
        if flight.payload_lbs != 0.
            || normal
                .state
                .configuration()
                .stations
                .iter()
                .zip(&normal.state.ammo)
                .any(|(station, count)| !station.internal && *count != 0)
        {
            return Err("pilot-only recording gained external stores".into());
        }
        normal.clean_recording = false;
        normal.mission_dummies(&[(id, 29)], 5280., data)?;
        normal.reset(&mut flight)?;
        let positions: Vec<_> = normal.state.targets.iter().map(|t| t.position).collect();
        if positions.len() != 29 {
            return Err("mission wing count mismatch".into());
        }
        let camera = airframe.panel_camera(&flight, 3);
        let geometry = normal.dummy_geometry(&camera, world);
        if geometry.len() != 1 || geometry[0].0.profile.id != id || geometry[0].1.is_empty() {
            return Err("dummy model identity or geometry mismatch".into());
        }
        let intact = airframe.vertices(&flight, &camera, world);
        // Exercise the shared effects against every imported mesh and gun,
        // including variants, rather than inferring coverage from F/A-18D.
        for region in [3usize, 4, 5] {
            for fraction in [0.1, 0.4, 0.8] {
                flight.damage_fraction = fraction;
                flight.damage_variant = (fraction >= 0.75).then_some(region);
                flight.damage_regions = [0.; tore_sim::combat::live::DAMAGE_SECTIONS];
                flight.damage_regions[region] = fraction;
                let mesh = airframe.vertices(&flight, &camera, world);
                if mesh.is_empty() || mesh.iter().any(|v| !v.is_finite()) || mesh == intact {
                    return Err(format!("{id:?}: damage region {region} at {fraction} has no distinct finite geometry").into());
                }
            }
        }
        let mut gun = crate::combat::Combat::new(&airframe, data, false)?;
        gun.state.armed = true;
        let gun_flight = airframe.start(world);
        let launcher = crate::combat::launcher(&gun_flight);
        for _ in 0..120 {
            gun.state.step(true, launcher, |_, _| 0.);
            if !gun.state.projectiles.is_empty() {
                break;
            }
        }
        let round = gun
            .state
            .projectiles
            .first()
            .ok_or("imported gun failed to fire")?;
        if !tore_sim::combat::live::is_gun(round.weapon(gun.state.configuration())) {
            return Err(format!("{id:?}: imported gun is missing shared gun behavior").into());
        }
        let gun_station = &gun.state.configuration().stations[gun.state.selected];
        let pipper = tore_sim::combat::gunsight::solve(
            &gun_station.weapon,
            &launcher,
            gun_station.mount,
            None,
        )?
        .ok_or_else(|| format!("{id:?}: imported gun has no 1000-foot sight solution"))?;
        if !pipper.point.iter().all(|v| v.is_finite())
            || (pipper.range_ft - 1000.).abs() > 0.01
            || pipper.maximum_range_ft <= 100.
        {
            return Err(format!("{id:?}: invalid imported gun sight/range").into());
        }
        let tracer = gun.vertices(&airframe, &gun_flight, &camera, world);
        if !tracer.vertices.chunks_exact(10).any(|v| v[5] == -8.) {
            return Err(format!("{id:?}: imported gun has no luminous tracer geometry").into());
        }
        flight.damage_fraction = 0.8;
        flight.damage_variant = Some(tore_sim::combat::live::DamageSection::LeftWing as usize);
        flight.damage_regions = [0.; tore_sim::combat::live::DAMAGE_SECTIONS];
        flight.damage_regions[tore_sim::combat::live::DamageSection::LeftWing as usize] = 0.8;
        let damaged = airframe.vertices(&flight, &camera, world);
        let fragment = airframe.fragment_vertices(&flight, &camera, world);
        if tore_sim::combat::debris::damage_variant(
            id,
            tore_sim::combat::live::DamageSection::LeftWing as usize,
        )
        .is_some()
            && (fragment.is_empty() || fragment == damaged)
        {
            return Err("detached model missing or substituted".into());
        }
        if intact == damaged
            || damaged.is_empty()
            || damaged
                .chunks_exact(10)
                .any(|v| !v.iter().all(|x| x.is_finite()))
        {
            return Err("damaged body did not produce distinct valid geometry".into());
        }
        flight.damage_fraction = 0.;
        flight.damage_variant = None;
        flight.damage_regions = [0.; tore_sim::combat::live::DAMAGE_SECTIONS];
        normal.state.targets[0].hp = 0;
        normal.step(&mut flight, world)?;
        if normal.dummy_geometry(&camera, world)[0].1.is_empty() {
            return Err("falling wreck disappeared".into());
        }
        normal.reset(&mut flight)?;
        if !normal.state.smoke.puffs.is_empty()
            || !normal.state.debris.is_empty()
            || flight.damage_fraction != 0.
        {
            return Err("reset retained damage appearance".into());
        }
        if normal
            .state
            .targets
            .iter()
            .map(|t| t.position)
            .collect::<Vec<_>>()
            != positions
            || normal.state.targets.iter().any(|t| t.hp <= 0)
        {
            return Err("restart did not restore dummy formation".into());
        }
        load.fuel(false);
        for i in 0..load.quantities.len() {
            load.change(i, -1);
        }
        let mut alternatives = 0;
        for (name, bytes) in data
            .iter()
            .filter(|(n, _)| tore_sim::combat::loadout::supported(n))
        {
            let weapon = Weapon::parse(name, bytes)?;
            if let Some(i) = (0..load.quantities.len()).find(|i| {
                !load.configuration.stations[*i].internal && load.capacity(*i, &weapon) > 0
            }) {
                let mut candidate = load.clone();
                candidate.select(i, weapon)?;
                candidate.validate()?;
                let mut combat = crate::combat::Combat::with_loadout(&airframe, data, &candidate)?;
                let mut flight = airframe.start(world);
                flight.fuel = candidate.fuel_lbs;
                combat.reset(&mut flight)?;
                if combat.state.ammo != candidate.quantities
                    || combat.range
                    || (flight.payload_lbs - combat.state.payload_lbs()).abs() > 0.01
                {
                    return Err("custom loadout initialization mismatch".into());
                }
                combat.state.ammo.fill(0);
                combat.reset(&mut flight)?;
                if combat.state.ammo != candidate.quantities {
                    return Err("restart lost accepted loadout".into());
                }
                alternatives += 1;
            }
        }
        if alternatives == 0 {
            return Err("no supported compatible loadout exercised".into());
        }
        load.quantities.fill(0);
        load.fuel_lbs = 0.;
        load.validate()?;
        let mut combat = crate::combat::Combat::with_loadout(&airframe, data, &load)?;
        let mut flight = airframe.start(world);
        flight.fuel = 0.;
        combat.reset(&mut flight)?;
        if combat.state.ammo.iter().any(|n| *n != 0) {
            return Err("empty loadout gained ammunition".into());
        }
        println!(
            "{}: {alternatives} supported store/placement cases, edited fuel, empty stations, normal weapons, guns-only for all six wings and restart, 1000-foot gun sights, glowing gun tracers, nine regional damage stages, 29 dummy models and restart passed",
            id.label()
        );
    }
    Ok(())
}

fn validate_dragging(load: &Loadout, data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let mut ui = Ordnance::new(load.clone(), data)?;
    let mut pixels = vec![0; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
    ui.render(&mut pixels);
    let source = load
        .configuration
        .stations
        .iter()
        .position(|station| !station.internal)
        .ok_or("no external station in ordnance validation")?;
    let from = (
        360. + (source % 2) as f64 * 119.,
        145. + (source / 2) as f64 * 71.,
    );
    ui.pointer(Some(from));
    ui.down();
    ui.pointer(Some((290., 370.)));
    ui.up();
    if ui.loadout.quantities[source] != 0 {
        return Err("catalog drop did not empty the imported station".into());
    }
    let weapon = &load.configuration.stations[source].weapon;
    let index = ui
        .catalog
        .iter()
        .position(|candidate| candidate.source == weapon.source)
        .ok_or("imported station weapon missing from catalog")?;
    ui.category = usize::from(weapon.flags & 0x10000 == 0);
    let entry = ui.page_entries().iter().position(|i| *i == index).unwrap();
    ui.pages[ui.category] = entry / 8;
    ui.render(&mut pixels);
    let row = entry % 8;
    ui.pointer(Some((
        80. + (row % 2) as f64 * 120.,
        120. + (row / 2) as f64 * 68.,
    )));
    ui.down();
    ui.pointer(Some(from));
    ui.up();
    if i32::from(ui.loadout.quantities[source]) != load.capacity(source, weapon) {
        return Err("catalog drag did not refill the imported station".into());
    }
    if let Some(target) =
        (0..load.quantities.len()).find(|i| *i != source && load.capacity(*i, weapon) > 0)
    {
        ui.loadout.quantities[target] = 0;
        let before = ui.loadout.quantities[source];
        ui.pointer(Some(from));
        ui.down();
        ui.pointer(Some((
            360. + (target % 2) as f64 * 119.,
            145. + (target / 2) as f64 * 71.,
        )));
        ui.up();
        if ui.loadout.quantities[target] == 0
            || ui.loadout.quantities[source] + ui.loadout.quantities[target] != before
        {
            return Err("station transfer did not conserve imported ammunition".into());
        }
    }
    Ok(())
}

fn validate_guns_only(
    load: &Loadout,
    airframe: &crate::aircraft::Airframe,
    targets: &[tore_sim::combat::live::Target],
    data: &BTreeMap<String, Vec<u8>>,
    world: &crate::terrain::World,
) -> AppResult<()> {
    use tore_sim::ai::{
        launch::{Side, WingId, WingSelection, resolve_wings},
        weapon_service::Rounds,
    };
    let selections: Vec<_> = (0..6)
        .map(|group| WingSelection {
            wing: WingId::new(
                if group < 3 {
                    Side::Friendly
                } else {
                    Side::Enemy
                },
                group % 3,
            )
            .unwrap(),
            aircraft: load.aircraft,
            count: if group == 0 { 4 } else { 5 },
            skill_level: 1,
        })
        .collect();
    let wings = resolve_wings(&selections, None)?;
    // Rebuilding is also the live restart path. Standard loads after guns-only
    // must still come from the unmodified imported aircraft records.
    for guns_only in [true, true, false] {
        let bridge = crate::ai_wings::AiWings::build(&wings, targets, guns_only, data)?;
        if bridge.len() != 29 {
            return Err("guns-only validation lost wing members".into());
        }
        for actor in bridge.mission().actors() {
            if actor.stations().len() != load.configuration.stations.len() {
                return Err("guns-only validation lost stations".into());
            }
            for (actual, original) in actor.stations().iter().zip(&load.configuration.stations) {
                let count = if guns_only && original.weapon.source != load.aircraft.gun() {
                    0
                } else {
                    u32::from(original.count)
                };
                if actual.rounds() != Rounds::Finite(count) {
                    return Err("wing weapon restriction mismatch".into());
                }
            }
        }
    }
    let mut guns = load.clone();
    guns.restrict_to_guns();
    let mut combat = crate::combat::Combat::with_loadout(airframe, data, &guns)?;
    let mut flight = airframe.start(world);
    for _ in 0..2 {
        combat.reset(&mut flight)?;
        if combat.state.ammo != guns.quantities {
            return Err("player guns-only load lost at launch or restart".into());
        }
        combat.state.ammo.fill(0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{HEIGHT, WIDTH};
    use tore_formats::aircraft::Hardpoint;

    fn fixture() -> Ordnance {
        let mut config = crate::ai_wings::tests::combat_fixture(true)
            .configuration()
            .clone();
        config.stations[0].weapon.source = "AIM9M.JT".into();
        config.stations[0].weapon.flags |= 0x10002;
        config.stations[0].count = 4;
        let mut gun = config.stations[0].clone();
        gun.weapon.source = "M61.JT".into();
        gun.weapon.flags |= 0x80;
        gun.internal = true;
        gun.count = 500;
        config.stations.push(config.stations[0].clone());
        config.stations.push(gun);
        let catalog = vec![config.stations[0].weapon.clone()];
        let loadout = Loadout {
            aircraft: config.aircraft,
            configuration: config,
            quantities: vec![2, 0, 500],
            fuel_lbs: 1000.,
            internal_capacity_lbs: 1000.,
            empty_lbs: 10000.,
            maximum_lbs: 20000.,
            cheat: false,
            hardpoints: (0..3)
                .map(|i| Hardpoint {
                    location: if i == 2 { 2 } else { 4 },
                    flags: if i == 2 { 8 } else { 0x20 },
                    position: [0; 3],
                    store: Some(if i == 2 { "M61.JT" } else { "AIM9M.JT" }.into()),
                    count: if i == 2 { 500 } else { 4 },
                    weight_class: 0,
                })
                .collect(),
        };
        let mut sprites = BTreeMap::new();
        sprites.insert(
            "ORD_AIR3.PIC".into(),
            Sprite {
                width: WIDTH,
                height: HEIGHT,
                rgba: [17, 19, 23, 255].repeat(WIDTH * HEIGHT),
                glyphs: vec![],
            },
        );
        for name in [
            "QUICKFONT",
            "MENUFONT.PIC",
            "ARMFONT.PIC",
            "WPNBLUE",
            "WPNYELLOW",
            "FONTACT.PIC",
        ] {
            sprites.insert(name.into(), crate::menu::flat_font([200, 200, 200]));
        }
        for name in [
            "ROCKER00.PIC",
            "ROCKER01.PIC",
            "ROCKER02.PIC",
            "ROCKER03.PIC",
            "ROCKER04.PIC",
            "DIAL11.PIC",
            "DIAL13.PIC",
            "LIGHTON.PIC",
            "LIGHTOFF.PIC",
            "ACTDFLT.PIC",
            "ACTDFT0L.PIC",
            "ACTDFT0M.PIC",
            "ACTDFT0R.PIC",
            "ACTION0L.PIC",
            "ACTION0M.PIC",
            "ACTION0R.PIC",
        ] {
            sprites.insert(
                name.into(),
                Sprite {
                    width: 1,
                    height: 1,
                    rgba: vec![0; 4],
                    glyphs: vec![],
                },
            );
        }
        let mut rgba = vec![0; 5 * 3 * 4];
        rgba[(5 + 2) * 4..(5 + 3) * 4].copy_from_slice(&[0, 255, 0, 255]);
        sprites.insert(
            "$AIM9M.PIC".into(),
            Sprite {
                width: 5,
                height: 3,
                rgba,
                glyphs: vec![],
            },
        );
        let mut ui = Ordnance {
            loadout,
            visible: true,
            weapons: catalog.clone(),
            catalog,
            sprites,
            category: 0,
            pages: [0; 2],
            selected: None,
            station: 0,
            hover: None,
            pressed: None,
            right_pressed: None,
            rockers: Default::default(),
            pointer: None,
            drag: None,
            controls: vec![],
            message: None,
            menu: false,
        };
        ui.render(&mut vec![0; WIDTH * HEIGHT * 4]);
        ui
    }
    fn drag(ui: &mut Ordnance, from: (f64, f64), to: (f64, f64)) -> Action {
        ui.pointer(Some(from));
        ui.down();
        ui.pointer(Some(to));
        ui.up()
    }
    #[test]
    fn catalog_drag_draws_only_transparent_thumbnail_and_loads_station() {
        let mut ui = fixture();
        let mut before = vec![0; WIDTH * HEIGHT * 4];
        ui.render(&mut before);
        ui.pointer(Some((100., 120.)));
        ui.down();
        ui.pointer(Some((310., 280.)));
        let mut after = before.clone();
        ui.render(&mut after);
        let changed: Vec<_> = before
            .chunks_exact(4)
            .zip(after.chunks_exact(4))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(changed, vec![280 * WIDTH + 310]);
        assert_eq!(
            &after[changed[0] * 4..changed[0] * 4 + 4],
            &[0, 255, 0, 255]
        );
        ui.pointer(Some((500., 145.)));
        ui.up();
        assert_eq!(ui.loadout.quantities, [2, 4, 500]);
        assert!(ui.drag.is_none());
    }
    #[test]
    fn station_transfer_and_catalog_unload_preserve_other_stations() {
        let mut ui = fixture();
        drag(&mut ui, (400., 145.), (500., 145.));
        assert_eq!(ui.loadout.quantities, [1, 1, 500]);
        // Unused space in the catalog is a valid unload destination.
        drag(&mut ui, (500., 145.), (280., 360.));
        assert_eq!(ui.loadout.quantities, [1, 0, 500]);
        // An occupied catalog card unloads too, without selecting that card.
        drag(&mut ui, (400., 145.), (100., 120.));
        assert_eq!(ui.loadout.quantities, [0, 0, 500]);
        assert!(ui.selected.is_none());
    }
    #[test]
    fn normal_catalog_hides_unimplemented_weapons_even_when_they_fit() {
        let mut ui = fixture();
        let mut missile = ui.weapons[0].clone();
        missile.source = "AIM120.JT".into();
        missile.flags = 0x10003;
        missile.seeker.signature = 3;
        ui.weapons.push(missile.clone());
        missile.source = "AIM7.JT".into();
        assert_eq!(ui.loadout.capacity(0, &missile), 4);
        ui.weapons.push(missile);
        ui.weapons
            .push(ui.loadout.configuration.stations[2].weapon.clone());
        ui.rebuild_catalog();
        assert_eq!(
            ui.catalog
                .iter()
                .map(|w| w.source.as_str())
                .collect::<Vec<_>>(),
            ["AIM9M.JT", "AIM120.JT", "M61.JT"]
        );
        // A compatible alternative can be selected, loaded and flown.
        ui.activate(101);
        ui.activate(201);
        assert_eq!(
            ui.loadout.configuration.stations[1].weapon.source,
            "AIM120.JT"
        );
        assert_eq!(ui.loadout.quantities[1], 4);
        assert_eq!(ui.activate(7), Action::MissionFly);
    }
    #[test]
    fn cheat_button_unloads_and_toggles_any_store_on_any_station() {
        let mut ui = fixture();
        let mut bomb = ui.weapons[0].clone();
        bomb.source = "MK82.JT".into();
        bomb.flags = 2;
        bomb.seeker.signature = 0;
        ui.weapons.push(bomb.clone());
        let mut unsupported = bomb.clone();
        unsupported.source = "GBU10.JT".into();
        ui.weapons.push(unsupported.clone());
        let mut gun = bomb.clone();
        gun.source = "DEFA.JT".into();
        gun.flags = 0x80;
        ui.weapons.push(gun.clone());
        ui.rebuild_catalog();
        let names = |ui: &Ordnance| {
            ui.catalog
                .iter()
                .map(|w| w.source.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&ui), ["AIM9M.JT"]);
        ui.activate(13);
        assert!(ui.loadout.cheat);
        assert_eq!(ui.loadout.quantities, [0, 0, 0]);
        assert_eq!(names(&ui), ["AIM9M.JT", "MK82.JT", "DEFA.JT"]);
        // Even a cheat-compatible unimplemented weapon stays hidden.
        assert_eq!(ui.loadout.capacity(0, &unsupported), 4);
        // Cheat lists all supported weapons, including other aircraft's guns.
        assert!((0..3).all(|i| ui.loadout.capacity(i, &gun) == 0));
        assert_eq!(ui.loadout.capacity(0, &bomb), 4);
        assert_eq!(ui.loadout.capacity(2, &bomb), 0, "the fixed gun station");
        ui.activate(4);
        ui.activate(100);
        ui.activate(200);
        assert_eq!(
            ui.loadout.configuration.stations[0].weapon.source,
            "MK82.JT"
        );
        assert_eq!(ui.loadout.quantities, [4, 0, 0]);
        assert_eq!(ui.activate(7), Action::MissionFly);
        ui.pages = [1, 2];
        ui.activate(13);
        assert!(!ui.loadout.cheat);
        assert_eq!(ui.loadout.quantities, [0, 0, 0]);
        assert_eq!(names(&ui), ["AIM9M.JT"]);
        assert_eq!(ui.pages, [0; 2]);
        assert!(ui.selected.is_none());
    }
    #[test]
    fn filtered_empty_catalog_has_no_selectable_cards_or_extra_pages() {
        let mut ui = fixture();
        ui.weapons[0].source = "AIM7.JT".into();
        for cheat in [false, true] {
            ui.loadout.cheat = cheat;
            ui.rebuild_catalog();
            assert!(ui.catalog.is_empty());
            for category in [3, 4] {
                ui.activate(category);
                ui.activate(2);
                ui.activate(100);
                assert_eq!(ui.pages, [0; 2]);
                assert!(ui.selected.is_none());
                let mut pixels = vec![0; WIDTH * HEIGHT * 4];
                ui.render(&mut pixels);
                assert!(!ui.controls.iter().any(|(id, _)| (100..108).contains(id)));
            }
        }
    }
    #[test]
    fn invalid_same_station_outside_and_cancelled_drops_do_not_mutate_loads() {
        let mut ui = fixture();
        drag(&mut ui, (400., 145.), (400., 215.));
        assert!(ui.message.is_some());
        assert_eq!(ui.loadout.quantities, [2, 0, 500]);
        for destination in [(410., 145.), (600., 300.), (520., 430.)] {
            drag(&mut ui, (400., 145.), destination);
            assert_eq!(ui.loadout.quantities, [2, 0, 500]);
        }
        for cancel in 0..3 {
            ui.pointer(Some((400., 145.)));
            ui.down();
            ui.pointer(Some((100., 120.)));
            match cancel {
                0 => ui.cancel(),
                1 => {
                    ui.key("Escape");
                }
                _ => ui.pointer(None),
            }
            ui.pointer(Some((100., 120.)));
            ui.up();
            assert_eq!(ui.loadout.quantities, [2, 0, 500]);
            assert!(ui.visible);
        }
    }
    #[test]
    fn empty_station_has_only_red_card_outline_below_location_heading() {
        let mut ui = fixture();
        ui.loadout.quantities.fill(0);
        let mut pixels = vec![0; WIDTH * HEIGHT * 4];
        ui.render(&mut pixels);
        for (x, y) in [(353, 135), (472, 135), (353, 206)] {
            for yy in y - 1..y + 51 {
                for xx in x..x + 109 {
                    let edge =
                        yy <= y + 21 && (yy == y - 1 || yy == y + 21 || xx == x || xx == x + 108);
                    let expected = if edge {
                        [94, 30, 20, 255]
                    } else {
                        [17, 19, 23, 255]
                    };
                    let at = (yy * WIDTH + xx) * 4;
                    assert_eq!(pixels[at..at + 4], expected, "pixel {xx},{yy}");
                }
            }
        }
    }
    #[test]
    fn click_loading_and_right_click_decrement_still_work() {
        let mut ui = fixture();
        drag(&mut ui, (100., 120.), (101., 120.));
        assert_eq!(ui.selected, Some(0));
        drag(&mut ui, (500., 145.), (500., 145.));
        assert_eq!(ui.loadout.quantities, [2, 4, 500]);
        ui.right(true);
        ui.right(false);
        assert_eq!(ui.loadout.quantities, [2, 3, 500]);
    }
    #[test]
    fn loaded_station_click_adds_exactly_one_without_replacing_or_overfilling() {
        let mut ui = fixture();
        let mut other = ui.catalog[0].clone();
        other.source = "AIM120.JT".into();
        ui.catalog.push(other);
        ui.selected = Some(1);
        drag(&mut ui, (400., 145.), (401., 145.));
        assert_eq!(ui.loadout.quantities, [3, 0, 500]);
        assert_eq!(
            ui.loadout.configuration.stations[0].weapon.source,
            "AIM9M.JT"
        );
        drag(&mut ui, (400., 145.), (400., 145.));
        drag(&mut ui, (400., 145.), (400., 145.));
        assert_eq!(ui.loadout.quantities, [4, 0, 500]);
        // Click adds one even where keyboard/right-click steps are 100 rounds.
        ui.loadout.quantities[2] = 498;
        drag(&mut ui, (400., 215.), (400., 215.));
        assert_eq!(ui.loadout.quantities, [4, 0, 499]);
    }
    #[test]
    fn ordnance_sounds_follow_successful_edits_and_not_failed_or_empty_actions() {
        let mut ui = fixture();
        assert_eq!(
            drag(&mut ui, (400., 145.), (400., 145.)),
            Action::OrdnanceWeapon
        );
        ui.right(true);
        assert_eq!(ui.right(false), Action::OrdnanceWeapon);
        assert_eq!(ui.key("+"), Action::OrdnanceWeapon);
        assert_eq!(
            drag(&mut ui, (100., 120.), (500., 145.)),
            Action::OrdnanceWeapon
        );
        assert_eq!(drag(&mut ui, (400., 145.), (500., 145.)), Action::None);
        ui.loadout.quantities[1] = 0;
        assert_eq!(
            drag(&mut ui, (400., 145.), (500., 145.)),
            Action::OrdnanceWeapon
        );
        assert_eq!(
            drag(&mut ui, (500., 145.), (100., 120.)),
            Action::OrdnanceWeapon
        );
        assert_eq!(drag(&mut ui, (400., 145.), (400., 215.)), Action::None);
        assert!(ui.message.is_some());
        assert_eq!(drag(&mut ui, (400., 145.), (410., 145.)), Action::None);
        ui.loadout.quantities[2] = 499;
        assert_eq!(
            drag(&mut ui, (400., 215.), (400., 215.)),
            Action::OrdnanceAmmunition
        );
        assert_eq!(drag(&mut ui, (400., 215.), (400., 215.)), Action::None);
        assert_eq!(
            drag(&mut ui, (400., 215.), (100., 120.)),
            Action::OrdnanceAmmunition
        );
        ui.pointer(Some((400., 215.)));
        ui.right(true);
        assert_eq!(ui.right(false), Action::None);
        // The imported flag chooses the sound, including ammunition in a pod.
        ui.loadout.configuration.stations[0].weapon.flags |= 0x80;
        assert_eq!(
            drag(&mut ui, (400., 145.), (400., 145.)),
            Action::OrdnanceAmmunition
        );
    }
    #[test]
    fn fuel_sound_requires_an_actual_fuel_change() {
        let mut ui = fixture();
        assert_eq!(ui.activate(5), Action::None);
        assert_eq!(ui.activate(6), Action::OrdnanceFuel);
        assert_eq!(ui.activate(6), Action::OrdnanceFuel);
        assert_eq!(ui.activate(6), Action::None);
        assert_eq!(ui.activate(5), Action::OrdnanceFuel);
    }
    #[test]
    fn rockers_act_on_press_and_spring_back_on_release() {
        let mut ui = fixture();
        // The fuel rocker's bottom half removes fuel as soon as it is pressed.
        let fuel = ui.loadout.fuel_lbs;
        ui.pointer(Some((550., 380.)));
        assert_eq!(ui.down(), Action::OrdnanceFuel);
        assert!(ui.loadout.fuel_lbs < fuel);
        assert!(ui.rockers[1].held());
        assert_eq!(ui.up(), Action::RockerUp);
        assert!(!ui.rockers[1].held());
        assert!(ui.loadout.fuel_lbs < fuel);
        // The page rocker sounds its own press, even at the first page.
        ui.pointer(Some((265., 410.)));
        assert_eq!(ui.down(), Action::RockerDown);
        ui.pointer(Some((300., 300.)));
        assert_eq!(ui.up(), Action::RockerUp);
        assert_eq!(ui.pages, [0, 0]);
    }
}
