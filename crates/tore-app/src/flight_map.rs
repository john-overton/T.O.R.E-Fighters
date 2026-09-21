//! Opinionated map presentation. Knowledge comes from simulation observations.
use crate::{
    flight,
    hud::Paint,
    menu::{Canvas, Sprite},
    terrain::World,
};
use tore_formats::{font::Font, theater::CELL_FEET};
use tore_sim::{combat::live, sensors::FEET_PER_NAUTICAL_MILE as NMI};

const AREA: (i32, i32, i32, i32) = (12, 30, 476, 408);
const INK: [u8; 4] = [232, 232, 218, 255];
const UNKNOWN: [u8; 4] = [255, 223, 92, 255];

#[derive(Clone, Copy)]
enum Category {
    Aircraft,
    Airfields,
    Buildings,
    Surface,
    Emitters,
}
impl Category {
    const ALL: [Self; 5] = [
        Self::Aircraft,
        Self::Airfields,
        Self::Buildings,
        Self::Surface,
        Self::Emitters,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Aircraft => "AIRCRAFT",
            Self::Airfields => "AIRFIELDS",
            Self::Buildings => "BUILDINGS",
            Self::Surface => "SURFACE",
            Self::Emitters => "EMITTERS",
        }
    }
    fn rect(self) -> (i32, i32, i32, i32) {
        (504, 76 + self as i32 * 44, 120, 30)
    }
}
struct Filters {
    visible: [bool; 5],
    pressed: Option<usize>,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            visible: [true, true, false, true, true],
            pressed: None,
        }
    }
}
impl Filters {
    fn shows(&self, category: Category) -> bool {
        self.visible[category as usize]
    }
    fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) {
        let hit = point.and_then(|(x, y)| {
            Category::ALL.iter().position(|category| {
                let (a, b, w, h) = category.rect();
                x >= a as f64 && x < (a + w) as f64 && y >= b as f64 && y < (b + h) as f64
            })
        });
        if down {
            self.pressed = hit;
        } else if let Some(index) = self.pressed.take().filter(|index| Some(*index) == hit) {
            self.visible[index] = !self.visible[index];
        }
    }
}

// Authored structural grouping. Do not use damage classes: they mix buildings,
// defenses and scenery. Unlisted resources remain in the Surface category.
fn is_building(resource: &str) -> bool {
    matches!(
        resource,
        "APTM.OT"
            | "APTLA.OT"
            | "APTLB.OT"
            | "APTLC.OT"
            | "APTB1.OT"
            | "APTOLD.OT"
            | "BARKSA.OT"
            | "BARKSB.OT"
            | "BLDG1.OT"
            | "BLDG2.OT"
            | "BLDG3.OT"
            | "BNK1.OT"
            | "BNK2.OT"
            | "BNK3.OT"
            | "BNK4.OT"
            | "BNK5.OT"
            | "BNK6.OT"
            | "BNK7.OT"
            | "BNK8.OT"
            | "BNK9.OT"
            | "~BNK5.OT"
            | "~BNK6.OT"
            | "~BNK8.OT"
            | "BUNKER.OT"
            | "SHELT.OT"
            | "BR1END.OT"
            | "BR1MID.OT"
            | "BR2END.OT"
            | "BR2MID.OT"
            | "BR3END.OT"
            | "BR3MID.OT"
            | "BRD1.OT"
            | "BRD2.OT"
            | "BRD3.OT"
            | "BRD4.OT"
            | "BRDEND.OT"
            | "BRDMID.OT"
            | "CASTLE.OT"
            | "CGRP1.OT"
            | "CGRP2.OT"
            | "CGRP3.OT"
            | "CMHQ1.OT"
            | "CMHQ2.OT"
            | "COLTWR.OT"
            | "~COLTWR.OT"
            | "COMM.OT"
            | "COMM1.OT"
            | "COMM2.OT"
            | "CTOWR.OT"
            | "CTWR1.OT"
            | "CTWR2.OT"
            | "TOWER.OT"
            | "DKHOS1.OT"
            | "DKHOS2.OT"
            | "DOCK1.OT"
            | "FACT1.OT"
            | "FACTD.OT"
            | "FCTYA.OT"
            | "FCTYB.OT"
            | "FUEL.OT"
            | "HANGR.OT"
            | "HANGRB.OT"
            | "AV8TNT.OT"
            | "HILT.OT"
            | "IND1.OT"
            | "IND2.OT"
            | "NUCCT.OT"
            | "NUCOB.OT"
            | "NUCRD.OT"
            | "OILW.OT"
            | "REACTB.OT"
            | "REACTR.OT"
            | "REDOM.OT"
            | "RELAY.OT"
            | "SLUM01.OT"
            | "SLUM02.OT"
            | "STORE.OT"
            | "HGRP1.OT"
            | "HGRP2.OT"
            | "HGRP3.OT"
            | "HOOCH.OT"
            | "HOUS.OT"
            | "HOUSB.OT"
            | "CITY1.OT"
            | "CITY2.OT"
            | "CITY3.OT"
            | "RES1.OT"
            | "RES2.OT"
            | "SGRP1.OT"
            | "SGRP2.OT"
            | "CTYBKA.OT"
            | "CTYBKB.OT"
            | "CTYBKC.OT"
            | "CTYBKD.OT"
            | "CTYBKE.OT"
            | "CTYBKF.OT"
            | "CTYBKG.OT"
            | "TWNBKA.OT"
            | "TWNBKB.OT"
            | "TWNBKC.OT"
            | "TWNBKD.OT"
            | "TWNBKE.OT"
            | "TWNBKF.OT"
    )
}

#[derive(Default)]
pub struct Map {
    pub open: bool,
    filters: Filters,
    zoom: i32,
    offset: [f64; 2],
    anchor: Option<[f64; 2]>,
    last_player: [f64; 2],
}
impl Map {
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) {
        self.filters.pointer(point, down);
    }
    pub fn cancel_press(&mut self) {
        self.filters.pressed = None;
    }

    fn width_nmi(&self) -> f64 {
        100. * 2f64.powi(self.zoom)
    }
    pub fn key(&mut self, key: &str) {
        let step = self.width_nmi() * NMI * 0.25;
        if key.starts_with("Arrow") && self.anchor.is_none() {
            self.anchor = Some(self.last_player);
        }
        match key {
            "+" | "=" => self.zoom = (self.zoom - 1).max(-2),
            "-" | "_" => self.zoom = (self.zoom + 1).min(3),
            "ArrowLeft" => self.offset[0] -= step,
            "ArrowRight" => self.offset[0] += step,
            "ArrowUp" => self.offset[1] += step,
            "ArrowDown" => self.offset[1] -= step,
            "Home" => {
                self.offset = [0.; 2];
                self.anchor = None;
            }
            _ => {}
        }
    }
    fn projection(&self, state: &flight::State) -> Projection {
        let anchor = self
            .anchor
            .unwrap_or([state.position[0], state.position[2]]);
        Projection {
            center: [anchor[0] + self.offset[0], anchor[1] + self.offset[1]],
            feet_per_pixel: self.width_nmi() * NMI / f64::from(AREA.2),
        }
    }
    fn draw_filters(
        &self,
        pixels: &mut [u8],
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
    ) {
        Canvas(pixels).rect((500, 30, 128, 408), [42, 45, 47, 255]);
        Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        }
        .text(font, "SHOW OBJECTS", 507, 45);
        for category in Category::ALL {
            let (x, y, w, h) = category.rect();
            let enabled = self.filters.shows(category);
            let pressed = self.filters.pressed == Some(category as usize);
            if [
                "ACTION0L.PIC",
                "ACTION0M.PIC",
                "ACTION0R.PIC",
                "FONTACT.PIC",
            ]
            .iter()
            .all(|key| sprites.contains_key(*key))
            {
                Canvas(pixels).button_style(
                    sprites,
                    "",
                    (x, y, w),
                    if pressed { 0.7 } else { 1. },
                    "ACTION0",
                );
            } else {
                Canvas(pixels).rect((x, y, w, h), [66, 70, 73, 255]);
            }
            Canvas(pixels).rect((x + 5, y + 6, 12, 12), [20, 25, 27, 255]);
            let mut p = Paint {
                pixels,
                clip: (x, y, w, h),
                color: if enabled { UNKNOWN } else { INK },
            };
            if enabled {
                p.line(
                    ((x + 7) as f64, (y + 11) as f64),
                    ((x + 10) as f64, (y + 15) as f64),
                );
                p.line(
                    ((x + 10) as f64, (y + 15) as f64),
                    ((x + 15) as f64, (y + 8) as f64),
                );
            }
            p.text(font, category.label(), x + 22, y + 7);
        }
        Paint {
            pixels,
            clip: (500, 30, 128, 408),
            color: INK,
        }
        .text(font, "CLICK TO TOGGLE", 504, 310);
    }
    pub fn draw(
        &mut self,
        pixels: &mut [u8],
        world: &World,
        state: &flight::State,
        combat: &live::State,
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
    ) {
        Canvas(pixels).rect((0, 0, 640, 480), [72, 72, 72, 255]);
        self.last_player = [state.position[0], state.position[2]];
        let projection = self.projection(state);
        let icons = sprites.get("MCICONS.PIC");
        let background = sprites.get(&world.theater.map);
        for y in AREA.1..AREA.1 + AREA.3 {
            for x in AREA.0..AREA.0 + AREA.2 {
                let [east, north] = projection.world(x, y);
                let col = (east / f64::from(CELL_FEET)).floor() as i32;
                let row = (north / f64::from(CELL_FEET)).floor() as i32;
                let rgb = match world.theater.lookup(col, row, 1) {
                    Some(cell) => background.map_or_else(
                        || {
                            if cell.color == 255 {
                                [65, 119, 137]
                            } else {
                                world.palette[usize::from(cell.color)]
                            }
                        },
                        |map| {
                            let u = (east / (world.theater.cols as f64 * f64::from(CELL_FEET))
                                * map.width as f64)
                                .clamp(0., (map.width - 1) as f64)
                                as usize;
                            let v = ((1.
                                - north / (world.theater.rows as f64 * f64::from(CELL_FEET)))
                                * map.height as f64)
                                .clamp(0., (map.height - 1) as f64)
                                as usize;
                            let at = (v * map.width + u) * 4;
                            map.rgba[at..at + 3].try_into().unwrap()
                        },
                    ),
                    None => [18; 3],
                };
                let at = (y as usize * 640 + x as usize) * 4;
                pixels[at..at + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        if let Some((x, y)) = projection.point(state.position) {
            markers.push((x, y));
            let w = "PLAYER"
                .bytes()
                .map(|c| font.glyphs[c as usize].advance as i32)
                .sum::<i32>();
            labels.push((
                x - w / 2 - 2,
                y + 11,
                x + w / 2 + 2,
                y + 13 + font.height as i32,
            ));
        }
        // Navigation landmarks are known independently of aircraft sensors.
        for runway in world
            .airport_scene
            .runways
            .iter()
            .filter(|_| self.filters.shows(Category::Airfields))
        {
            if let Some(point) = projection.point(runway.surface.center) {
                let (x, y) = place_marker(pixels, point, &mut markers, &mut labels);
                symbol(pixels, icons, x, y, Some(15), false);
                let name = world
                    .airport_scene
                    .airports
                    .iter()
                    .find(|a| a.id == runway.airport)
                    .map_or(runway.name.as_str(), |a| a.name.as_str());
                label(pixels, font, name, (x, y + 12), INK, &mut labels);
            }
        }
        let contacts = combat.sensors.map_contacts();
        for observed in contacts
            .iter()
            .filter(|c| c.airborne)
            .chain(contacts.iter().filter(|c| !c.airborne))
        {
            let contact = &observed.contact;
            if world
                .airport_scene
                .runways
                .iter()
                .any(|r| r.object == contact.id)
            {
                continue;
            }
            let category = if observed.airborne {
                Category::Aircraft
            } else if world
                .airport_scene
                .objects
                .iter()
                .any(|o| o.id == contact.id && is_building(&o.object_type))
            {
                Category::Buildings
            } else {
                Category::Surface
            };
            if !self.filters.shows(category) {
                continue;
            }
            let Some(point) = projection.point(contact.position) else {
                continue;
            };
            let (x, y) = place_marker(pixels, point, &mut markers, &mut labels);
            let identified = observed.identified;
            let target = identified
                .then(|| combat.targets.iter().find(|t| t.id == contact.id))
                .flatten();
            let name = if identified {
                target
                    .and_then(|t| t.aircraft)
                    .map(|a| a.label())
                    .or_else(|| {
                        world
                            .airport_scene
                            .objects
                            .iter()
                            .find(|o| o.id == contact.id)
                            .map(|o| o.name.as_str())
                    })
                    .unwrap_or(if observed.airborne {
                        "AIRCRAFT"
                    } else {
                        "SURFACE OBJECT"
                    })
            } else if observed.airborne {
                "UNKNOWN AIRCRAFT"
            } else {
                "UNKNOWN SURFACE"
            };
            let icon = identified.then_some(if observed.airborne { 0 } else { 14 });
            symbol(pixels, icons, x, y, icon, false);
            label(
                pixels,
                font,
                if contact.destroyed { "DESTROYED" } else { name },
                (x, y + 12),
                if identified { INK } else { UNKNOWN },
                &mut labels,
            );
        }
        // Passive noise has no measured location. Show its direction at ownship.
        if let Some((x, y)) = projection.point(state.position) {
            for emitter in combat
                .emitters
                .iter()
                .filter(|e| e.distance_nmi.is_none() && self.filters.shows(Category::Emitters))
            {
                let angle = state.yaw + emitter.bearing_rad;
                let end = (
                    f64::from(x) + angle.sin() * 45.,
                    f64::from(y) - angle.cos() * 45.,
                );
                if projection.inside(end.0 as i32, end.1 as i32) {
                    Paint {
                        clip: AREA,
                        pixels,
                        color: UNKNOWN,
                    }
                    .line((x as f64, y as f64), end);
                    label(
                        pixels,
                        font,
                        "UNKNOWN EMITTER",
                        (end.0 as i32, end.1 as i32),
                        UNKNOWN,
                        &mut labels,
                    );
                }
            }
            // The eight original aircraft headings are clockwise from north.
            let heading = (state.yaw.rem_euclid(std::f64::consts::TAU)
                / std::f64::consts::FRAC_PI_4)
                .round() as usize
                % 8;
            symbol(pixels, icons, x, y, Some(heading), true);
            label(
                pixels,
                font,
                "PLAYER",
                (x, y + 12),
                UNKNOWN,
                &mut Vec::new(),
            );
        }
        self.draw_filters(pixels, font, sprites);
        // Redraw the frame so text and ray endpoints never escape the plot.
        let mut c = Canvas(pixels);
        c.rect((0, 0, 640, 30), [72, 72, 72, 255]);
        c.rect((0, 438, 640, 42), [72, 72, 72, 255]);
        let mut p = Paint {
            pixels,
            clip: (0, 0, 640, 480),
            color: INK,
        };
        p.line((12., 15.), (12. + AREA.2 as f64 / 4., 15.));
        for i in 0..=4 {
            let x = 12. + i as f64 * AREA.2 as f64 / 16.;
            p.line((x, 12.), (x, 18.));
        }
        p.text(
            font,
            &format!("{} NAUTICAL MILES", self.width_nmi() / 4.),
            174,
            9,
        );
        p.text(
            font,
            &format!(
                "HEADING: {:03.0} DEG",
                state.yaw.to_degrees().rem_euclid(360.)
            ),
            449,
            9,
        );
        p.text(
            font,
            &format!("SPEED: {:.0} KTS", state.speed * 3600. / NMI),
            12,
            443,
        );
        p.text(
            font,
            &format!("ALTITUDE: {:.0} FT", state.position[1]),
            436,
            443,
        );
        p.text(
            font,
            "SHIFT-M / ESC: CLOSE   +/-: ZOOM   ARROWS: PAN   HOME: FOLLOW",
            12,
            463,
        );
    }
}
struct Projection {
    center: [f64; 2],
    feet_per_pixel: f64,
}
impl Projection {
    fn inside(&self, x: i32, y: i32) -> bool {
        (AREA.0 + 12..AREA.0 + AREA.2 - 12).contains(&x)
            && (AREA.1 + 12..AREA.1 + AREA.3 - 12).contains(&y)
    }
    fn point(&self, position: [f64; 3]) -> Option<(i32, i32)> {
        if !position.iter().all(|p| p.is_finite()) {
            return None;
        }
        let x =
            f64::from(AREA.0 + AREA.2 / 2) + (position[0] - self.center[0]) / self.feet_per_pixel;
        let y = 234. - (position[2] - self.center[1]) / self.feet_per_pixel;
        self.inside(x as i32, y as i32)
            .then_some((x as i32, y as i32))
    }
    fn world(&self, x: i32, y: i32) -> [f64; 2] {
        [
            self.center[0] + (x as f64 - f64::from(AREA.0 + AREA.2 / 2)) * self.feet_per_pixel,
            self.center[1] - (y as f64 - 234.) * self.feet_per_pixel,
        ]
    }
}
// Keep neighboring symbols readable, with a leader to the actual observation.
fn place_marker(
    pixels: &mut [u8],
    point: (i32, i32),
    occupied: &mut Vec<(i32, i32)>,
    labels: &mut Vec<(i32, i32, i32, i32)>,
) -> (i32, i32) {
    let free = |(x, y): (i32, i32)| {
        x >= AREA.0 + 12
            && x < AREA.0 + AREA.2 - 12
            && y >= AREA.1 + 12
            && y < AREA.1 + AREA.3 - 24
            && !labels
                .iter()
                .any(|&(a, b, c, d)| x - 12 < c && x + 12 > a && y - 10 < d && y + 10 > b)
            && occupied
                .iter()
                .all(|&(a, b)| (x - a).abs() >= 26 || (y - b).abs() >= 34)
    };
    let mut placed = point;
    'search: for radius in [0, 34, 68, 102] {
        for (dx, dy) in [
            (1, 0),
            (0, -1),
            (-1, 0),
            (0, 1),
            (1, -1),
            (-1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let candidate = (point.0 + dx * radius, point.1 + dy * radius);
            if free(candidate) {
                placed = candidate;
                break 'search;
            }
        }
    }
    if placed != point {
        let mut p = Paint {
            pixels,
            clip: AREA,
            color: INK,
        };
        p.line(
            (point.0 as f64, point.1 as f64),
            (placed.0 as f64, placed.1 as f64),
        );
        p.rect(point.0 - 1, point.1 - 1, 3, 3);
    }
    occupied.push(placed);
    labels.push((placed.0 - 12, placed.1 - 10, placed.0 + 12, placed.1 + 10));
    placed
}
fn symbol(
    pixels: &mut [u8],
    icons: Option<&Sprite>,
    x: i32,
    y: i32,
    index: Option<usize>,
    player: bool,
) {
    if let (Some(sheet), Some(index)) = (icons, index)
        && sheet.width >= (index + 1) * 24
        && sheet.height >= 20
    {
        for dy in 0..20 {
            for dx in 0..23 {
                let src = (dy * sheet.width + index * 24 + dx) * 4;
                let mut color: [u8; 4] = sheet.rgba[src..src + 4].try_into().unwrap();
                // Neutral tiles do not assert friend/foe knowledge.
                if !player && color[2] > color[0] {
                    color = [48, 53, 60, 255];
                }
                Canvas(pixels).rect((x - 11 + dx as i32, y - 10 + dy as i32, 1, 1), color);
            }
        }
    } else {
        Canvas(pixels).rect((x - 9, y - 9, 19, 19), [38, 42, 46, 255]);
        let mut p = Paint {
            clip: AREA,
            pixels,
            color: UNKNOWN,
        };
        for (a, b) in [
            ((x, y - 8), (x + 8, y)),
            ((x + 8, y), (x, y + 8)),
            ((x, y + 8), (x - 8, y)),
            ((x - 8, y), (x, y - 8)),
        ] {
            p.line((a.0 as f64, a.1 as f64), (b.0 as f64, b.1 as f64));
        }
        if index.is_none() {
            p.line(
                ((x - 2) as f64, (y - 3) as f64),
                ((x + 2) as f64, (y - 3) as f64),
            );
            p.line(((x + 2) as f64, (y - 3) as f64), (x as f64, y as f64));
            p.rect(x, y + 3, 1, 1);
        } else {
            p.line((x as f64, (y - 5) as f64), (x as f64, (y + 5) as f64));
        }
    }
}
fn label(
    pixels: &mut [u8],
    font: &Font,
    text: &str,
    (x, y): (i32, i32),
    color: [u8; 4],
    occupied: &mut Vec<(i32, i32, i32, i32)>,
) {
    let text: String = text.chars().filter(char::is_ascii).take(28).collect();
    let w = text
        .bytes()
        .map(|c| font.glyphs[c as usize].advance as i32)
        .sum::<i32>();
    let h = font.height as i32;
    let x = (x - w / 2).clamp(AREA.0 + 2, (AREA.0 + AREA.2 - w - 2).max(AREA.0 + 2));
    let fits = |y: i32| {
        y > AREA.1
            && y + h < AREA.1 + AREA.3
            && !occupied
                .iter()
                .any(|&(a, b, c, d)| x - 2 < c && x + w + 2 > a && y - 1 < d && y + h + 1 > b)
    };
    let Some(y) = [y, y - h - 24].into_iter().find(|y| fits(*y)) else {
        return;
    };
    occupied.push((x - 2, y - 1, x + w + 2, y + h + 1));
    Canvas(pixels).rect((x - 2, y - 1, w + 4, h + 2), [30, 36, 40, 255]);
    Paint {
        pixels,
        clip: AREA,
        color,
    }
    .text(font, &text, x, y);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buildings_default_off_and_defenses_are_not_buildings() {
        let filters = Filters::default();
        assert!(!filters.shows(Category::Buildings));
        for category in [
            Category::Aircraft,
            Category::Airfields,
            Category::Surface,
            Category::Emitters,
        ] {
            assert!(filters.shows(category));
        }
        for name in ["HANGR.OT", "CTYBKA.OT", "FUEL.OT", "TOWER.OT", "BUNKER.OT"] {
            assert!(is_building(name));
        }
        for name in [
            "SA3SITE.OT",
            "HAWKSITE.OT",
            "KING.OT",
            "PRDR1.OT",
            "STRIP3.OT",
            "UNKNOWN.OT",
        ] {
            assert!(!is_building(name));
        }
    }
    #[test]
    fn category_click_requires_matching_press_release_and_cancel_discards_press() {
        let mut map = Map::default();
        let buildings = Some((510., 170.));
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(Some((510., 82.)), false);
        assert!(!map.filters.shows(Category::Buildings));
        assert!(map.filters.shows(Category::Aircraft));
        map.pointer(buildings, true);
        map.cancel_press();
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(buildings, false);
        assert!(map.filters.shows(Category::Buildings));
        map.open = false;
        map.open = true;
        assert!(map.filters.shows(Category::Buildings));
        map.pointer(Some((490., 170.)), true);
        map.pointer(buildings, false);
        assert!(map.filters.shows(Category::Buildings));
        map.pointer(buildings, true);
        map.pointer(buildings, false);
        assert!(!map.filters.shows(Category::Buildings));
    }
    #[test]
    fn north_up_projection_scale_and_edges() {
        let p = Projection {
            center: [1000., 2000.],
            feet_per_pixel: 100. * NMI / 476.,
        };
        assert_eq!(p.point([1000., 0., 2000.]), Some((250, 234)));
        assert_eq!(p.point([1000. + 25. * NMI, 0., 2000.]), Some((369, 234)));
        assert_eq!(p.point([1000., 0., 2000. + 25. * NMI]), Some((250, 115)));
        assert_eq!(p.point([1000. + 100. * NMI, 0., 2000.]), None);
        assert_eq!(p.point([f64::NAN, 0., 0.]), None);
        assert_eq!(p.world(250, 234), p.center);
    }
    #[test]
    fn zoom_is_bounded_and_home_resets_pan() {
        let mut m = Map::default();
        for _ in 0..20 {
            m.key("+");
        }
        assert_eq!(m.width_nmi(), 25.);
        m.key("ArrowUp");
        assert_eq!(m.offset, [0., 6.25 * NMI]);
        m.key("Home");
        assert_eq!(m.offset, [0.; 2]);
        for _ in 0..20 {
            m.key("-");
        }
        assert_eq!(m.width_nmi(), 800.);
    }
}
