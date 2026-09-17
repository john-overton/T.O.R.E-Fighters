//! Retail art and recovered card/control geometry over a transactional loadout.
use crate::{
    AppResult,
    menu::{Action, Canvas, Sprite, text_width},
    quick_mission::notice,
};
use std::collections::BTreeMap;
use tore_formats::{Pic, weapons::Weapon};
use tore_sim::combat::loadout::Loadout;
type Rect = (i32, i32, i32, i32);
pub struct Ordnance {
    pub loadout: Loadout,
    pub visible: bool,
    catalog: Vec<Weapon>,
    sprites: BTreeMap<String, Sprite>,
    category: usize,
    pages: [usize; 2],
    selected: Option<usize>,
    station: usize,
    hover: Option<usize>,
    pressed: Option<usize>,
    right_pressed: Option<usize>,
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
                    || [
                        "ORD_AIR3.PIC",
                        "DIAL00.PIC",
                        "DIAL04.PIC",
                        "DIAL11.PIC",
                        "DIAL13.PIC",
                        "LIGHTON.PIC",
                        "LIGHTOFF.PIC",
                        "ACTDFLT.PIC",
                        "ROCKER00.PIC",
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
        let mut catalog: Vec<_> = data
            .iter()
            .filter(|(n, _)| n.ends_with(".JT") && !n.starts_with('~'))
            .filter_map(|(name, b)| Weapon::parse(name, b).ok())
            .filter(|w| {
                loadout
                    .hardpoints
                    .iter()
                    .enumerate()
                    .any(|(i, _)| loadout.capacity(i, w) > 0)
            })
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
        for w in &mut catalog {
            display(w);
        }
        for station in &mut loadout.configuration.stations {
            display(&mut station.weapon);
        }
        catalog.sort_by(|a, b| a.name.cmp(&b.name).then(a.source.cmp(&b.source)));
        Ok(Self {
            loadout,
            visible: true,
            catalog,
            sprites,
            category: 0,
            pages: [0; 2],
            selected: None,
            station: 0,
            hover: None,
            pressed: None,
            right_pressed: None,
            controls: vec![],
            message: None,
            menu: false,
        })
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
    pub fn down(&mut self) {
        self.pressed = self.hover;
    }
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.right_pressed = None;
        self.hover = None;
        self.menu = false;
    }
    pub fn up(&mut self) -> Action {
        let p = self.pressed.take();
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
        self.selected = self
            .page_entries()
            .get(self.pages[self.category] * 8 + id - 100)
            .copied();
    }
    pub fn right(&mut self, down: bool) -> Action {
        if down {
            self.right_pressed = self.hover;
            return Action::None;
        }
        let pressed = self.right_pressed.take();
        if let Some(id) = self
            .hover
            .filter(|i| Some(*i) == pressed && (200..232).contains(i))
        {
            self.station = id - 200;
            self.loadout.change(self.station, -1);
            Action::Click
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
            5 => self.loadout.fuel(true),
            6 => self.loadout.fuel(false),
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
            9 => self.loadout.change(self.station, 1),
            10 => self.loadout.change(self.station, -1),
            11 => self.menu = !self.menu,
            12 => {
                self.loadout.quantities.fill(0);
                self.menu = false;
            }
            13 => self.message = Some("Cheat loading is not available yet.".into()),
            14 => {
                self.message =
                    Some("Airbase aircraft cycling is not available in this setup.".into())
            }
            100..=107 => self.select_card(id),
            200..=231 => {
                self.station = id - 200;
                if let Some(w) = self.selected
                    && let Err(e) = self.loadout.select(self.station, self.catalog[w].clone())
                {
                    self.message = Some(e.to_string());
                }
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str) -> Action {
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
            "ArrowRight" => self.activate(2),
            "ArrowLeft" => self.activate(1),
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
    pub fn render(&mut self, pixels: &mut [u8]) {
        pixels.copy_from_slice(&self.sprites["ORD_AIR3.PIC"].rgba);
        self.controls.clear();
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
            let x = 68 + (row % 2) as i32 * 120;
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
            if *n > 0 || s.internal {
                card(
                    &mut c,
                    &self.sprites,
                    &s.weapon,
                    x + 2,
                    y + 14,
                    self.station == i,
                    false,
                );
            } else {
                c.text(font, "Empty", x + 2, y + 39, None);
            }
            let amount = if s.internal {
                format!("{n} (max {})", self.loadout.capacity(i, &s.weapon))
            } else {
                format!("{n} loaded (max {})", self.loadout.capacity(i, &s.weapon))
            };
            c.text(font, &amount, x + 2, y + 52, None);
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
        let rocker = &self.sprites["ROCKER00.PIC"];
        c.blit(rocker, (260, 408), 0, rocker.width, 1.);
        c.blit(rocker, (547, 356), 0, rocker.width, 1.);
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
                "Cheat (unavailable)",
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
    let edge = if selected {
        [201, 179, 49, 255]
    } else {
        [94, 30, 20, 255]
    };
    c.rect((x, y - 1, 109, 1), edge);
    c.rect((x, y + 21, 109, 1), edge);
    c.rect((x, y - 1, 1, 23), edge);
    c.rect((x + 108, y - 1, 1, 23), edge);
    let pic = format!("${}.PIC", w.source.trim_end_matches(".JT"));
    if let Some(p) = sprites.get(&pic) {
        c.blit(p, (x + 1, y), 0, p.width, 1.);
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

/// Source-cache regression probe. No GPU, native execution, or invented targets.
pub fn validate_sources(
    data: &BTreeMap<String, Vec<u8>>,
    world: &crate::terrain::World,
) -> AppResult<()> {
    for id in tore_formats::aircraft::AircraftId::ALL {
        let airframe = crate::aircraft::Airframe::load(data, id)?;
        let mut load = Loadout::new(&airframe.profile, |n| {
            data.get(n)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing {n}")))
        })?;
        load.validate()?;
        let mut normal = crate::combat::Combat::new(&airframe, data, false)?;
        let mut flight = airframe.start(world);
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
        normal.state.targets[0].hp = 0;
        normal.step(&mut flight, world)?;
        normal.reset(&mut flight)?;
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
            "{}: {alternatives} supported store/placement cases, edited fuel, empty stations, normal weapons, 29 dummy models and restart passed",
            id.label()
        );
    }
    Ok(())
}
