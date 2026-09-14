//! Small independent raster instruments. Layout fitted to supplied retail captures;
//! data is live, unsupported native sensors/camera modes are explicit.
use crate::{aircraft::Airframe, flight::State, menu::Sprite};
use tore_formats::font::Font;
pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 156;
const GREEN: [u8; 4] = [132, 193, 126, 255];
const DIM: [u8; 4] = [48, 83, 44, 255];
pub struct Raster {
    pub pixels: Vec<u8>,
}
impl Raster {
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT * 4],
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        for yy in y.max(0)..(y + h).min(HEIGHT as i32) {
            for xx in x.max(0)..(x + w).min(WIDTH as i32) {
                let i = (yy as usize * WIDTH + xx as usize) * 4;
                self.pixels[i..i + 4].copy_from_slice(&c);
            }
        }
    }
    fn line(&mut self, a: (i32, i32), b: (i32, i32), c: [u8; 4]) {
        let n = (b.0 - a.0).abs().max((b.1 - a.1).abs()).max(1);
        for i in 0..=n {
            self.rect(
                a.0 + (b.0 - a.0) * i / n,
                a.1 + (b.1 - a.1) * i / n,
                1,
                1,
                c,
            );
        }
    }
    fn text(&mut self, font: &Font, s: &str, mut x: i32, y: i32, c: [u8; 4]) {
        for ch in s.bytes() {
            let g = &font.glyphs[ch as usize];
            for &(xx, yy) in &g.pixels {
                self.rect(x + xx as i32, y + yy as i32, 1, 1, c);
            }
            x += g.advance as i32;
        }
    }
    fn circle(&mut self, cx: i32, cy: i32, r: f64, c: [u8; 4]) {
        for i in 0..360 {
            let t = i as f64 * std::f64::consts::TAU / 360.;
            self.rect(
                cx + (t.cos() * r).round() as i32,
                cy + (t.sin() * r).round() as i32,
                1,
                1,
                c,
            );
        }
    }
    fn sprite(&mut self, s: &Sprite, x: i32, y: i32, w: i32, h: i32) {
        for yy in 0..h {
            for xx in 0..w {
                let i = (yy as usize * s.height / h as usize * s.width
                    + xx as usize * s.width / w as usize)
                    * 4;
                if s.rgba[i + 3] > 0 {
                    self.rect(x + xx, y + yy, 1, 1, s.rgba[i..i + 4].try_into().unwrap());
                }
            }
        }
    }
}
/// Positions are in the shared 640x480 flight canvas. Small windows form
/// two groups of three across the bottom, leaving a central gutter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Large,
    Small,
}
impl Layout {
    pub fn capacity(self) -> usize {
        if self == Self::Large { 4 } else { 6 }
    }
    pub fn rect_on(self, slot: usize, size: [f64; 2]) -> (f64, f64, f64, f64) {
        let scale = (size[0] / 640.).min(size[1] / 480.);
        let (x, y, w, h) = self.rect(slot);
        let right = match self {
            Self::Large => slot >= 2,
            Self::Small => slot >= 3,
        };
        let bottom = self == Self::Small || matches!(slot, 1 | 2);
        (
            if right {
                size[0] - (640 - x) as f64 * scale
            } else {
                x as f64 * scale
            },
            if bottom {
                size[1] - (480 - y) as f64 * scale
            } else {
                y as f64 * scale
            },
            w as f64 * scale,
            h as f64 * scale,
        )
    }
    pub fn rect(self, slot: usize) -> (i32, i32, i32, i32) {
        assert!(slot < self.capacity());
        match self {
            Self::Large => {
                let (x, y) = [(8, 8), (8, 316), (472, 316), (472, 8)][slot];
                (x, y, 160, 156)
            }
            Self::Small => {
                let x = [8, 110, 212, 332, 434, 536][slot];
                (x, 378, 96, 94)
            }
        }
    }
}
#[derive(Default)]
pub struct CombatReadout {
    pub weapon: String,
    pub guided: bool,
    pub readiness: &'static str,
    pub damage: Option<String>,
    pub ammo: u16,
    pub loaded: bool,
    pub target: Option<(u32, i32, bool)>,
    pub contacts: Vec<(f64, f64)>,
}
pub struct Instruments {
    pub combat: Option<CombatReadout>,
    pub pages: Vec<u8>,
    pub selected: usize,
    pub layout: Layout,
    pub(crate) other_pages: Vec<u8>,
    pub cameras: std::collections::BTreeMap<u8, Vec<u8>>,
    pub rwr_range: usize,
    pub radar_range: usize,
    pub mode: usize,
    pub pressed: Option<(usize, usize)>,
}
impl Default for Instruments {
    fn default() -> Self {
        Self {
            combat: None,
            pages: vec![7, 5, 9, 4],
            selected: 0,
            layout: Layout::Large,
            other_pages: vec![7, 5, 6, 4, 9, 8],
            cameras: Default::default(),
            rwr_range: 4,
            radar_range: 3,
            mode: 0,
            pressed: None,
        }
    }
}
impl Instruments {
    pub fn new(layout: Layout, page: Option<u8>) -> Self {
        let mut result = Self::default();
        if layout == Layout::Small {
            result.toggle_layout();
        }
        if let Some(page) = page {
            result.pages = vec![page];
        }
        result
    }
    pub fn toggle_layout(&mut self) {
        self.selected = 0;
        self.pressed = None;
        std::mem::swap(&mut self.pages, &mut self.other_pages);
        self.layout = if self.layout == Layout::Large {
            Layout::Small
        } else {
            Layout::Large
        };
    }

    pub fn toggle(&mut self, page: u8) {
        self.selected = 0;
        self.pressed = None;
        if let Some(i) = self.pages.iter().position(|p| *p == page) {
            self.pages.remove(i);
        } else {
            if self.pages.len() == self.layout.capacity() {
                self.pages.remove(0);
            }
            self.pages.push(page);
        }
    }
    pub fn screen_pointer(
        &mut self,
        point: Option<(f64, f64)>,
        size: [f64; 2],
        down: bool,
    ) -> bool {
        let point = point.and_then(|(px, py)| {
            (0..self.pages.len()).find_map(|i| {
                let (x, y, w, h) = self.layout.rect_on(i, size);
                if px < x || py < y || px >= x + w || py >= y + h {
                    return None;
                }
                let (lx, ly, lw, lh) = self.layout.rect(i);
                Some((
                    lx as f64 + (px - x) * lw as f64 / w,
                    ly as f64 + (py - y) * lh as f64 / h,
                ))
            })
        });
        self.pointer(point, down)
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>, down: bool) -> bool {
        let hit = p.and_then(|(x, y)| {
            self.pages.iter().enumerate().find_map(|(i, _)| {
                let (ox, oy, w, h) = self.layout.rect(i);
                // Invert the same transform used to draw the raster.
                let x = (x - ox as f64) * WIDTH as f64 / w as f64;
                let y = (y - oy as f64) * HEIGHT as f64 / h as f64;
                if (136. ..154.).contains(&y) {
                    (0..4)
                        .find(|b| (26. + *b as f64 * 29. ..44. + *b as f64 * 29.).contains(&x))
                        .map(|b| (i, b))
                } else {
                    None
                }
            })
        });
        if down {
            self.pressed = hit;
            return false;
        }
        let before = self.pressed.take();
        if let Some((i, b)) = hit.filter(|h| Some(*h) == before) {
            return self.control(i, b);
        }
        false
    }
    pub fn select(&mut self, slot: usize) -> bool {
        if slot >= self.pages.len() {
            return false;
        }
        self.selected = slot;
        true
    }
    pub fn cycle_selection(&mut self, delta: i32) -> bool {
        if self.pages.is_empty() {
            return false;
        }
        self.selected = (self.selected as i32 + delta).rem_euclid(self.pages.len() as i32) as usize;
        true
    }
    /// Same stock scope operations for pointer and hardware buttons; no new sensor behavior.
    pub fn control(&mut self, slot: usize, button: usize) -> bool {
        match (self.pages.get(slot), button) {
            (Some(5), 0) => self.rwr_range = self.rwr_range.saturating_sub(1),
            (Some(5), 1) => self.rwr_range = (self.rwr_range + 1).min(4),
            (Some(9), 0) => self.radar_range = self.radar_range.saturating_sub(1),
            (Some(9), 1) => self.radar_range = (self.radar_range + 1).min(4),
            (Some(9), 2) => self.mode = (self.mode + 1) % 3,
            _ => return false,
        }
        true
    }
    pub fn page(&self, id: u8, h: &Airframe, s: &State) -> Raster {
        let mut r = Raster::new();
        r.rect(0, 0, 160, 156, [98, 115, 143, 255]);
        r.rect(0, 0, 160, 2, [190, 207, 216, 255]);
        r.rect(0, 0, 2, 156, [190, 207, 216, 255]);
        r.rect(158, 0, 2, 156, [24, 34, 44, 255]);
        r.rect(0, 154, 160, 2, [24, 34, 44, 255]);
        for (name, x, y) in [
            ("EDGETL.PIC", 1, 1),
            ("EDGETR.PIC", 144, 1),
            ("EDGEBL.PIC", 1, 138),
            ("EDGEBR.PIC", 144, 138),
        ] {
            if let Some(p) = h.sprites.get(name) {
                r.sprite(p, x, y, 15, 17);
            }
        }
        r.rect(10, 20, 140, 116, [188, 204, 208, 255]);
        r.rect(11, 21, 138, 114, [0, 0, 0, 255]);
        r.rect(23, 5, 12, 12, [54, 68, 88, 255]);
        let f = &h.font;
        let title = match id {
            0 => "RCS",
            1 => "ENVELOPE",
            2 => "FRONT VIEW",
            3 => "OTHER VIEW",
            4 => "RADAR/VISUAL",
            5 => "RWR",
            6 => "NAV INFO",
            7 => "SYSTEMS",
            8 => "WEAPONS",
            _ => "RADAR",
        };
        r.text(f, &id.to_string(), 26, 7, [224, 235, 241, 255]);
        let title_width: usize = title.bytes().map(|ch| f.glyphs[ch as usize].advance).sum();
        r.text(
            f,
            title,
            (WIDTH as i32 - title_width as i32) / 2,
            7,
            [224, 235, 241, 255],
        );
        for b in 0..4 {
            let x = 26 + b * 29;
            r.rect(x - 2, 137, 22, 17, [45, 57, 70, 255]);
            r.rect(x, 137, 18, 15, [207, 227, 237, 255]);
            r.rect(x, 137, 18, 1, [245, 251, 255, 255]);
            r.rect(x + 18, 137, 2, 16, [52, 67, 86, 255]);
            if b < 3 {
                r.rect(x + 23, 137, 2, 17, [170, 187, 203, 255]);
            }
            let label = match (id, b) {
                (5 | 9, 0) => "-",
                (5 | 9, 1) => "+",
                (9, 2) => "M",
                _ => "",
            };
            r.text(f, label, x + 6, 141, [20, 35, 46, 255]);
        }
        let text = |r: &mut Raster, t: &str, x, y| r.text(f, t, x, y, GREEN);
        match id {
            0 => {
                text(&mut r, "RCS NOT IMPLEMENTED", 20, 70);
            }
            7 => {
                for (i, (label, value)) in [
                    ("THR", format!("{:.0}%", s.throttle * 100.)),
                    ("TEMP", "---".into()),
                    ("OIL", "---".into()),
                    ("HYD", "---".into()),
                ]
                .iter()
                .enumerate()
                {
                    text(&mut r, label, 20, 33 + i as i32 * 14);
                    text(&mut r, value, 107, 33 + i as i32 * 14);
                }
                r.line((20, 91), (138, 91), GREEN);
                text(&mut r, "FUEL", 20, 101);
                text(&mut r, &format!("{:.0} LBS", s.fuel), 76, 101);
                text(&mut r, "STORES", 20, 115);
                text(&mut r, &format!("{:.0} LBS", s.payload_lbs), 76, 115);
            }
            5 => {
                r.line((13, 76), (147, 76), DIM);
                r.line((80, 22), (80, 133), DIM);
                r.circle(80, 76, 48., DIM);
                r.circle(80, 76, 24., DIM);
                text(
                    &mut r,
                    ["5", "10", "20", "30", "50"][self.rwr_range],
                    126,
                    25,
                );
                r.rect(78, 74, 5, 1, GREEN);
                r.rect(78, 74, 1, 5, GREEN);
                r.rect(82, 74, 1, 5, GREEN);
                r.rect(78, 78, 5, 1, GREEN);
                if s.jammer && s.engine {
                    text(&mut r, "JAM", 17, 124);
                }
            }
            9 => {
                if !s.radar || !s.engine {
                    text(&mut r, "RADAR OFF", 48, 75);
                } else {
                    for i in 1..4 {
                        let y = 21 + i * 114 / 4;
                        r.line((12, y), (148, y), DIM);
                        let x = 11 + i * 138 / 4;
                        r.line((x, 22), (x, 134), DIM);
                    }
                    text(&mut r, ["RWS", "TWS", "A-G"][self.mode], 17, 25);
                    text(
                        &mut r,
                        &format!(
                            "{:.0}",
                            [10f64, 20., 40., 80., 160.][self.radar_range]
                                .min(h.equipment["F18R.SEE"].number("zone0.maxRange") / 6076.)
                        ),
                        126,
                        25,
                    );
                }
            }
            8 => {
                if let Some(c) = &self.combat {
                    text(&mut r, &c.weapon, 20, 35);
                    text(&mut r, &format!("{} RDS", c.ammo), 94, 35);
                    text(
                        &mut r,
                        if c.loaded {
                            "PT STORES LOADED"
                        } else {
                            "EXTERNAL: CLEAN"
                        },
                        20,
                        56,
                    );
                    text(&mut r, c.readiness, 20, 77);
                    if let Some(damage) = &c.damage {
                        text(&mut r, damage, 20, 94);
                    }
                    text(&mut r, "CM NOT ACTIVE", 20, 110);
                } else {
                    text(&mut r, "WEAPONS SAFE", 20, 35);
                }
            }
            6 => {
                text(&mut r, "HDG", 20, 35);
                text(&mut r, &format!("{:03.0}", s.yaw.to_degrees()), 105, 35);
                text(&mut r, "ALT", 20, 53);
                text(&mut r, &format!("{:.0}", s.position[1]), 94, 53);
                text(&mut r, "NO WAYPOINT", 20, 81);
                text(&mut r, "FREE FLIGHT", 20, 105);
            }
            1 => {
                for e in &h.profile.envelopes {
                    for i in 0..e.points.len() {
                        let a = e.points[i];
                        let b = e.points[(i + 1) % e.points.len()];
                        let point = |p: [f64; 2]| {
                            (
                                17 + (p[0] / 2000. * 126.) as i32,
                                130 - (p[1] / 70000. * 98.) as i32,
                            )
                        };
                        r.line(point(a), point(b), if e.g == 1 { GREEN } else { DIM });
                    }
                }
                r.rect(
                    17 + (s.speed / 2000. * 126.) as i32,
                    130 - (s.position[1] / 70000. * 98.) as i32,
                    3,
                    3,
                    [240, 240, 170, 255],
                );
                text(&mut r, "FT / FT/SEC", 20, 25);
            }
            4 => {
                if let Some((id, hp, locked)) = self.combat.as_ref().and_then(|c| c.target) {
                    text(&mut r, &format!("TARGET {id}"), 30, 42);
                    text(&mut r, &format!("HP {hp}"), 30, 62);
                    text(
                        &mut r,
                        if hp == 0 {
                            "DESTROYED"
                        } else if self.combat.as_ref().is_some_and(|c| !c.guided) {
                            "VISUAL"
                        } else if locked {
                            "LOCK"
                        } else {
                            "NO LOCK"
                        },
                        30,
                        82,
                    );
                } else {
                    text(&mut r, "NO TARGET", 48, 73);
                }
            }
            2 | 3 => {
                if let Some(pixels) = self.cameras.get(&id) {
                    r.sprite(
                        &Sprite {
                            width: 138,
                            height: 114,
                            rgba: pixels.clone(),
                            glyphs: vec![],
                        },
                        11,
                        21,
                        138,
                        114,
                    );
                } else {
                    text(&mut r, "CAMERA LOADING", 30, 73);
                }
            }
            _ => {}
        }
        if id == 9
            && s.radar
            && let Some(c) = &self.combat
        {
            let range = [10., 20., 40., 80., 160.][self.radar_range] * 6076.;
            for &(bearing, distance) in &c.contacts {
                if distance <= range && bearing.abs() <= std::f64::consts::FRAC_PI_2 {
                    let x = 80 + (bearing.sin() * 55.) as i32;
                    let y = 130 - (distance / range * 90.) as i32;
                    r.rect(x - 2, y - 2, 5, 5, GREEN);
                }
            }
        }
        r
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn button(i: &Instruments, slot: usize, b: usize) -> (f64, f64) {
        let (x, y, w, h) = i.layout.rect(slot);
        (
            x as f64 + (35. + b as f64 * 29.) * w as f64 / 160.,
            y as f64 + 145. * h as f64 / 156.,
        )
    }
    #[test]
    fn responsive_edges_and_right_hand_buttons_match_the_screen() {
        for size in [[640., 480.], [1920., 1080.], [2560., 1080.], [800., 1200.]] {
            for layout in [Layout::Large, Layout::Small] {
                let scale = (size[0] / 640f64).min(size[1] / 480.);
                let mut instruments = Instruments::new(layout, None);
                for slot in 0..layout.capacity() {
                    let (x, y, w, h) = layout.rect_on(slot, size);
                    assert!(x >= 8. * scale - 0.001 && y >= 8. * scale - 0.001);
                    assert!(
                        x + w <= size[0] - 8. * scale + 0.001
                            && y + h <= size[1] - 8. * scale + 0.001
                    );
                }
                let slot = if layout == Layout::Large { 2 } else { 4 };
                let (x, y, w, h) = layout.rect_on(slot, size);
                let point = Some((x + 35. * w / 160., y + 145. * h / 156.));
                instruments.screen_pointer(point, size, true);
                assert!(instruments.screen_pointer(point, size, false));
                assert_eq!(instruments.radar_range, 2);
                instruments.screen_pointer(point, size, true);
                assert!(!instruments.screen_pointer(
                    Some((size[0] - 1., size[1] - 1.)),
                    size,
                    false
                ));
            }
        }
    }
    #[test]
    fn scaled_buttons_require_matching_release_and_respect_margins() {
        for layout in [Layout::Large, Layout::Small] {
            let mut i = Instruments::new(layout, None);
            let minus = button(&i, 1, 0);
            let plus = button(&i, 1, 1);
            i.pointer(Some(minus), true);
            assert!(!i.pointer(Some(plus), false));
            assert_eq!(i.rwr_range, 4);
            for _ in 0..10 {
                i.pointer(Some(minus), true);
                assert!(i.pointer(Some(minus), false));
            }
            assert_eq!(i.rwr_range, 0);
            i.pointer(Some(minus), true);
            assert!(!i.pointer(Some((0., 479.)), false));
        }
    }
    #[test]
    fn layouts_stay_inside_margins_and_do_not_overlap() {
        for layout in [Layout::Large, Layout::Small] {
            for i in 0..layout.capacity() {
                let (x, y, w, h) = layout.rect(i);
                assert!(x >= 8 && y >= 8 && x + w <= 632 && y + h <= 472);
                for j in 0..i {
                    let (a, b, c, d) = layout.rect(j);
                    assert!(x >= a + c || a >= x + w || y >= b + d || b >= y + h);
                }
            }
        }
    }
    #[test]
    fn switching_preserves_each_layout_and_cancels_pending_clicks() {
        let mut i = Instruments::default();
        i.toggle(1);
        assert_eq!(i.pages, [5, 9, 4, 1]);
        let large = i.pages.clone();
        i.pressed = Some((0, 0));
        i.toggle_layout();
        assert!(i.pressed.is_none());
        assert_eq!(i.pages.len(), 6);
        i.toggle(2);
        let small = i.pages.clone();
        i.toggle_layout();
        assert_eq!(i.pages, large);
        i.toggle_layout();
        assert_eq!(i.pages, small);
        for page in 0..10 {
            i.toggle(page);
            assert!(i.pages.len() <= 6);
        }
    }
}

#[cfg(test)]
mod input_selection_tests {
    use super::*;
    #[test]
    fn logical_focus_uses_existing_buttons_without_changing_pages_or_rasters() {
        let mut i = Instruments::default();
        let pages = i.pages.clone();
        assert!(i.select(1));
        assert!(i.control(i.selected, 0));
        assert_eq!(i.rwr_range, 3);
        assert!(i.cycle_selection(1));
        assert!(i.control(i.selected, 2));
        assert_eq!(i.mode, 1);
        assert!(!i.control(0, 0));
        assert!(!i.control(2, 3));
        assert!(!i.select(5));
        assert_eq!(i.pages, pages);
        i.toggle_layout();
        assert!(i.select(5));
        assert!(i.cycle_selection(1));
        assert_eq!(i.selected, 0);
        i.pages.clear();
        assert!(!i.cycle_selection(1));
        assert!(!i.control(0, 0));
    }
}
