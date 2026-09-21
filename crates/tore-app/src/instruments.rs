//! Small independent raster instruments. Layout fitted to supplied retail captures;
//! data is live, unsupported native sensors/camera modes are explicit.
use crate::{aircraft::Airframe, flight::State, menu::Sprite, scope};
use tore_formats::font::Font;
pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 156;
const GREEN: [u8; 4] = [132, 193, 126, 255];
const DIM: [u8; 4] = [48, 83, 44, 255];
const BRIGHT: [u8; 4] = [206, 240, 200, 255];
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
/// Agent-proposed mouse tolerance in instrument raster pixels.
pub const PICK_TOLERANCE: f64 = 7.;
/// Readability cap on combined noise density, so text, the selector and valid
/// track symbols stay legible over a noisy background.
const MAX_NOISE_DENSITY: f64 = 0.35;
/// Scope origin and plotted extents inside the instrument window.
const SCOPE_ORIGIN: (f64, f64) = (80., 130.);
const SCOPE_HALF_WIDTH: f64 = 55.;
const SCOPE_DEPTH: f64 = 90.;

/// One projection for drawing and picking: bearing across, distance up.
/// A contact outside the plotted range returns None and is not drawn, which
/// does not mean the sensor lost it.
pub fn project(bearing_rad: f64, distance_ft: f64, range_nmi: f64) -> Option<(f64, f64)> {
    let range = range_nmi * tore_sim::sensors::FEET_PER_NAUTICAL_MILE;
    if range <= 0. || distance_ft > range || bearing_rad.abs() > std::f64::consts::FRAC_PI_2 {
        return None;
    }
    Some((
        SCOPE_ORIGIN.0 + bearing_rad.sin() * SCOPE_HALF_WIDTH,
        SCOPE_ORIGIN.1 - distance_ft / range * SCOPE_DEPTH,
    ))
}

/// Deterministic display texture. It uses the simulation tick and an
/// independent display seed, never the combat RNG, so noise cannot change
/// detection or replay.
fn speckle(x: i32, y: i32, phase: u64) -> f64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ 0x5241_4441_5f4e_4f49;
    for v in [x as u64 & 0xffff, y as u64 & 0xffff, phase] {
        h = (h ^ v).wrapping_mul(0x100_0000_01b3);
        h ^= h >> 29;
    }
    f64::from((h >> 40) as u32 & 0xffff) / 65535.
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
    pub systems: String,
    pub readiness: &'static str,
    pub damage: Option<String>,
    pub ammo: u16,
    pub loaded: bool,
    pub target: Option<crate::target_window::Readout>,
    pub scope: scope::Scope,
    pub rcs: scope::Rcs,
}
pub struct Instruments {
    pub combat: Option<CombatReadout>,
    pub pages: Vec<u8>,
    pub selected: usize,
    pub layout: Layout,
    pub(crate) other_pages: Vec<u8>,
    pub target_preview: Option<u32>,
    pub camera_target: Option<u32>,
    pub cameras: std::collections::BTreeMap<u8, Vec<u8>>,
    pub rwr_range: usize,
    pub radar_range: usize,
    pub rcs_range: usize,
    /// Requested scope channel: 0 radar, 1 infrared.
    pub channel: usize,
    pub history: bool,
    pub pressed: Option<(usize, usize)>,
    pressed_contact: Option<(usize, u32)>,
    /// The contact under the pointer, drawn with a selector.
    pub hovered: Option<u32>,
    pub crosshair: Option<(i32, i32)>,
    pub weapon_debug: bool,
    /// One pending designation request, drained by the host each frame.
    pub designation: Option<u32>,
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
            target_preview: None,
            camera_target: None,
            rwr_range: 4,
            radar_range: tore_sim::sensors::DEFAULT_RANGE_INDEX,
            rcs_range: tore_sim::sensors::passive::DEFAULT_SCALE_INDEX,
            channel: 0,
            history: false,
            pressed: None,
            pressed_contact: None,
            hovered: None,
            crosshair: None,
            weapon_debug: false,
            designation: None,
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
    /// Cancel a pending press, on focus loss or a layout change.
    pub fn cancel_press(&mut self) {
        self.pressed = None;
        self.pressed_contact = None;
        self.hovered = None;
        self.crosshair = None;
        self.designation = None;
    }
    pub fn toggle_layout(&mut self) {
        self.selected = 0;
        self.pressed = None;
        self.pressed_contact = None;
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
        self.pressed_contact = None;
        if let Some(i) = self.pages.iter().position(|p| *p == page) {
            self.pages.remove(i);
        } else {
            if self.pages.len() == self.layout.capacity() {
                self.pages.remove(0);
            }
            self.pages.push(page);
        }
    }
    /// Reserve the upper-right diagnostic area without changing pointer transforms.
    pub fn screen_rect(&self, slot: usize, size: [f64; 2]) -> (f64, f64, f64, f64) {
        let (x, mut y, w, h) = self.layout.rect_on(slot, size);
        if self.weapon_debug && self.layout == Layout::Large && slot == 3 {
            y += 104. * (size[0] / 640.).min(size[1] / 480.);
        }
        (x, y, w, h)
    }
    pub fn screen_pointer(
        &mut self,
        point: Option<(f64, f64)>,
        size: [f64; 2],
        down: bool,
    ) -> bool {
        let point = self.canvas_point(point, size);
        self.pointer(point, down)
    }
    /// The contact under a flight-canvas point, if any. Drawing and picking
    /// share one projection, so this is the contact the player sees at every
    /// window size and in both layouts.
    fn contact_at(&self, p: Option<(f64, f64)>) -> Option<(usize, u32)> {
        let (x, y) = p?;
        let readout = self.combat.as_ref()?;
        self.pages.iter().enumerate().find_map(|(i, page)| {
            if *page != 9 {
                return None;
            }
            let (ox, oy, w, h) = self.layout.rect(i);
            let point = (
                (x - f64::from(ox)) * WIDTH as f64 / f64::from(w),
                (y - f64::from(oy)) * HEIGHT as f64 / f64::from(h),
            );
            let range = readout.scope.range_nmi;
            scope::pick(
                &readout.scope.contacts,
                |c| project(c.bearing_rad, c.distance_ft, range),
                point,
                PICK_TOLERANCE,
            )
            .map(|id| (i, id))
        })
    }
    /// Hover feedback: the contact a click would designate right now.
    pub fn hover(&mut self, point: Option<(f64, f64)>, size: [f64; 2]) {
        self.crosshair = self.canvas_point(point, size).and_then(|(x, y)| {
            self.pages.iter().enumerate().find_map(|(i, page)| {
                if *page != 9 {
                    return None;
                }
                let (ox, oy, w, h) = self.layout.rect(i);
                let px = (x - f64::from(ox)) * WIDTH as f64 / f64::from(w);
                let py = (y - f64::from(oy)) * HEIGHT as f64 / f64::from(h);
                ((11. ..149.).contains(&px) && (21. ..135.).contains(&py))
                    .then_some((px as i32, py as i32))
            })
        });
        self.hovered = self
            .contact_at(self.canvas_point(point, size))
            .map(|(_, id)| id);
    }
    fn canvas_point(&self, point: Option<(f64, f64)>, size: [f64; 2]) -> Option<(f64, f64)> {
        let (px, py) = point?;
        (0..self.pages.len()).find_map(|i| {
            let (x, y, w, h) = self.screen_rect(i, size);
            if px < x || py < y || px >= x + w || py >= y + h {
                return None;
            }
            let (lx, ly, lw, lh) = self.layout.rect(i);
            Some((
                lx as f64 + (px - x) * lw as f64 / w,
                ly as f64 + (py - y) * lh as f64 / h,
            ))
        })
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
        let contact = self.contact_at(p);
        if down {
            self.pressed = hit;
            self.pressed_contact = contact;
            return false;
        }
        let before = self.pressed.take();
        let contact_before = self.pressed_contact.take();
        if let Some((i, b)) = hit.filter(|h| Some(*h) == before) {
            return self.control(i, b);
        }
        // An empty click leaves the current designation alone.
        if let Some((_, id)) = contact.filter(|c| Some(*c) == contact_before) {
            self.designation = Some(id);
            return true;
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
    /// Stock scope operations for pointer and hardware buttons. Range and
    /// channel are player controls; they never change a sensor's coverage.
    pub fn control(&mut self, slot: usize, button: usize) -> bool {
        let last = tore_sim::sensors::RANGE_LADDER_NMI.len() - 1;
        let scales = tore_sim::sensors::passive::SCALE_LADDER_NMI.len() - 1;
        match (self.pages.get(slot), button) {
            (Some(0), 0) => self.rcs_range = self.rcs_range.saturating_sub(1),
            (Some(0), 1) => self.rcs_range = (self.rcs_range + 1).min(scales),
            (Some(5), 0) => self.rwr_range = self.rwr_range.saturating_sub(1),
            (Some(5), 1) => self.rwr_range = (self.rwr_range + 1).min(4),
            (Some(9), 0) => self.radar_range = self.radar_range.saturating_sub(1),
            (Some(9), 1) => self.radar_range = (self.radar_range + 1).min(last),
            (Some(9), 2) => self.cycle_channel(),
            (Some(9), 3) => self.history = !self.history,
            _ => return false,
        }
        true
    }
    /// The on-screen M button cycles the available radar and infrared
    /// channels. Availability is decided by the simulation, not here.
    pub fn cycle_channel(&mut self) {
        self.channel = (self.channel + 1) % 2;
        self.pressed_contact = None;
    }
    pub fn controls(&self) -> tore_sim::sensors::Controls {
        tore_sim::sensors::Controls {
            channel: if self.channel == 0 {
                tore_sim::sensors::Channel::Radar
            } else {
                tore_sim::sensors::Channel::Infrared
            },
            range_index: self
                .radar_range
                .min(tore_sim::sensors::RANGE_LADDER_NMI.len() - 1),
            history: self.history,
        }
    }
    pub fn rcs_scale_nmi(&self) -> f64 {
        tore_sim::sensors::passive::SCALE_LADDER_NMI[self
            .rcs_range
            .min(tore_sim::sensors::passive::SCALE_LADDER_NMI.len() - 1)]
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
            4 => "TARGET CAM",
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
                (0 | 5 | 9, 0) => "-",
                (0 | 5 | 9, 1) => "+",
                (9, 2) => "M",
                (9, 3) => "Y",
                _ => "",
            };
            r.text(f, label, x + 6, 141, [20, 35, 46, 255]);
        }
        let text = |r: &mut Raster, t: &str, x, y| r.text(f, t, x, y, GREEN);
        match id {
            0 => {
                let (cx, cy, radius) = (80i32, 76i32, 48.);
                r.circle(cx, cy, radius, DIM);
                r.circle(cx, cy, radius / 2., DIM);
                r.line((cx, 22), (cx, 133), DIM);
                r.line((13, cy), (147, cy), DIM);
                for (label, x, y) in [
                    ("0", 77, 24),
                    ("90", 132, 72),
                    ("180", 74, 126),
                    ("270", 16, 72),
                ] {
                    text(&mut r, label, x, y);
                }
                if let Some(c) = &self.combat {
                    let scale = c.rcs.scale_nmi.max(1e-6);
                    let point = |bearing: f64, nmi: f64| {
                        let ratio = (nmi / scale).min(1.) * radius;
                        (
                            cx + (bearing.sin() * ratio).round() as i32,
                            cy - (bearing.cos() * ratio).round() as i32,
                        )
                    };
                    // The exposure contour is a reference estimate of
                    // directional vulnerability, not a detection boundary.
                    let mut over_range = false;
                    for i in 0..c.rcs.contour.len() {
                        let (ba, na) = c.rcs.contour[i];
                        let (bb, nb) = c.rcs.contour[(i + 1) % c.rcs.contour.len()];
                        over_range |= na > scale;
                        r.line(point(ba, na), point(bb, nb), GREEN);
                    }
                    for e in &c.rcs.emitters {
                        let (x, y) = match e.distance_nmi {
                            Some(nmi) => point(e.bearing_rad, nmi),
                            // Bearing without independent range stays on the
                            // outer ring with an unknown-range marker.
                            None => point(e.bearing_rad, scale),
                        };
                        match (e.symbol, e.distance_nmi) {
                            (tore_sim::sensors::passive::Symbol::Ground, _) => {
                                r.rect(x - 2, y - 2, 5, 5, GREEN)
                            }
                            (_, Some(_)) => {
                                r.line((x, y - 2), (x + 2, y), GREEN);
                                r.line((x + 2, y), (x, y + 2), GREEN);
                                r.line((x, y + 2), (x - 2, y), GREEN);
                                r.line((x - 2, y), (x, y - 2), GREEN);
                            }
                            _ => {
                                r.rect(x - 2, y - 2, 5, 1, GREEN);
                                r.rect(x - 2, y + 2, 5, 1, GREEN);
                                r.rect(x - 2, y - 2, 1, 5, GREEN);
                                r.rect(x + 2, y - 2, 1, 5, GREEN);
                            }
                        }
                    }
                    text(&mut r, &format!("{:.0}", c.rcs.scale_nmi), 126, 25);
                    if over_range {
                        text(&mut r, "OVER", 14, 25);
                    }
                    text(&mut r, &format!("SIG {:.0}", c.rcs.signature), 14, 124);
                } else {
                    text(&mut r, "NO EXPOSURE DATA", 30, 73);
                }
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
                for i in 1..4 {
                    let y = 21 + i * 114 / 4;
                    r.line((12, y), (148, y), DIM);
                    let x = 11 + i * 138 / 4;
                    r.line((x, 22), (x, 134), DIM);
                }
                match &self.combat {
                    None => text(&mut r, "NO SENSOR DATA", 30, 73),
                    Some(c) if !c.scope.operating => {
                        let reason = match (c.scope.infrared, c.scope.unavailable) {
                            (_, Some(text)) => text,
                            (true, _) => "IR UNAVAILABLE",
                            _ => "RADAR OFF",
                        };
                        text(&mut r, reason, 40, 75);
                    }
                    Some(c) => {
                        let scope = &c.scope;
                        let range = scope.range_nmi;
                        // Received noise is drawn under the returns. On this
                        // bearing/range projection the strobe is a vertical
                        // band: passive noise supplies no target range.
                        let phase = scope.tick / 12;
                        // Indicated bearing is quantized to 2 degrees with a
                        // 2-degree minimum half width, so noise never discloses
                        // perfect angular precision.
                        let bands: Vec<(f64, f64, f64, f64)> = scope
                            .strobes
                            .iter()
                            .map(|strobe| {
                                (
                                    ((strobe.bearing_rad.to_degrees() / 2.).round() * 2.)
                                        .to_radians(),
                                    strobe.half_width_rad.max(2f64.to_radians()),
                                    strobe.density,
                                    strobe.density * strobe.sidelobe,
                                )
                            })
                            .collect();
                        if !bands.is_empty() {
                            for y in 22..134 {
                                for x in 12..149 {
                                    let bearing = ((f64::from(x) - SCOPE_ORIGIN.0)
                                        / SCOPE_HALF_WIDTH)
                                        .clamp(-1., 1.)
                                        .asin();
                                    // Emitters combine with bounded brightness:
                                    // a strong one cannot whiten every pixel.
                                    let density = bands
                                        .iter()
                                        .map(|(centre, half, inside, outside)| {
                                            if (bearing - centre).abs() <= *half {
                                                *inside
                                            } else {
                                                *outside
                                            }
                                        })
                                        .sum::<f64>()
                                        .min(MAX_NOISE_DENSITY);
                                    if density > 0. && speckle(x, y, phase) < density {
                                        r.rect(x, y, 1, 1, DIM);
                                    }
                                }
                            }
                        }
                        for contact in &scope.contacts {
                            if self.history && !contact.stale {
                                // Past observations only, never extrapolated
                                // and never connected across a missing sample.
                                for (age, (bearing, distance)) in
                                    contact.trail.iter().rev().enumerate()
                                {
                                    if let Some((x, y)) = project(*bearing, *distance, range) {
                                        let fade = 1. - age as f64 / 10.;
                                        let shade = DIM.map(|v| (f64::from(v) * fade) as u8);
                                        r.rect(
                                            x as i32,
                                            y as i32,
                                            1,
                                            1,
                                            [shade[0], shade[1], shade[2], 255],
                                        );
                                    }
                                }
                            }
                            let Some((x, y)) =
                                project(contact.bearing_rad, contact.distance_ft, range)
                            else {
                                continue;
                            };
                            let (x, y) = (x as i32, y as i32);
                            if contact.stale {
                                // Visibly stale coasting plot, not selectable.
                                r.rect(x - 1, y, 3, 1, DIM);
                                r.rect(x, y - 1, 1, 3, DIM);
                                continue;
                            }
                            let colour = if contact.selected { BRIGHT } else { GREEN };
                            r.rect(x - 2, y - 2, 5, 1, colour);
                            r.rect(x - 2, y + 2, 5, 1, colour);
                            r.rect(x - 2, y - 2, 1, 5, colour);
                            r.rect(x + 2, y - 2, 1, 5, colour);
                            if contact.track_eligible {
                                let tail = (
                                    x + (contact.heading_rad.sin() * 6.) as i32,
                                    y - (contact.heading_rad.cos() * 6.) as i32,
                                );
                                r.line((x, y), tail, DIM);
                            }
                            if contact.selected || self.hovered == Some(contact.id) {
                                for (dx, dy) in [(-4, -4), (4, -4), (-4, 4), (4, 4)] {
                                    r.rect(x + dx, y + dy, 1, 1, colour);
                                }
                            }
                            if contact.acquired {
                                r.rect(x - 1, y - 1, 3, 3, colour);
                            }
                        }
                        text(&mut r, scope.mode.unwrap_or(scope.channel), 17, 25);
                        text(&mut r, &format!("{range:.0}"), 126, 25);
                        if scope.history {
                            text(&mut r, "HIST", 17, 124);
                        }
                        if let Some(status) = scope.status {
                            let width: usize =
                                status.bytes().map(|ch| f.glyphs[ch as usize].advance).sum();
                            text(&mut r, status, 147 - width as i32, 124);
                        }
                    }
                }
                if let Some((x, y)) = self.crosshair {
                    // Takeover and drawing cover the black screen, not just the contact plot.
                    if x - 4 >= 11 {
                        r.line((11, y), (x - 4, y), GREEN);
                    }
                    if x + 4 <= 148 {
                        r.line((x + 4, y), (148, y), GREEN);
                    }
                    if y - 4 >= 21 {
                        r.line((x, 21), (x, y - 4), GREEN);
                    }
                    if y + 4 <= 134 {
                        r.line((x, y + 4), (x, 134), GREEN);
                    }
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
                    text(&mut r, &c.systems, 20, 110);
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
                if let Some(target) = self.combat.as_ref().and_then(|c| c.target.as_ref()) {
                    r.rect(11, 21, 138, 114, [185, 185, 185, 255]);
                    if self.camera_target == Some(target.id)
                        && let Some(pixels) = self.cameras.get(&4)
                    {
                        let mut gray = pixels.clone();
                        crate::target_window::monochrome(&mut gray);
                        r.sprite(
                            &Sprite {
                                width: 138,
                                height: 114,
                                rgba: gray,
                                glyphs: vec![],
                            },
                            11,
                            21,
                            138,
                            114,
                        );
                    }
                    let ink = [20, 20, 20, 255];
                    let fit = |value: &str, limit: usize| {
                        let mut width = 0;
                        value
                            .bytes()
                            .take_while(|ch| {
                                width += f.glyphs[*ch as usize].advance;
                                width <= limit
                            })
                            .map(char::from)
                            .collect::<String>()
                    };
                    let width = |value: &str| {
                        value
                            .bytes()
                            .map(|ch| f.glyphs[ch as usize].advance as i32)
                            .sum::<i32>()
                    };
                    let label = fit(&target.name.to_ascii_uppercase(), 108);
                    r.text(f, &label, 12 + (119 - width(&label)) / 2, 24, ink);
                    let activity = fit(&target.activity, 119);
                    r.text(f, &activity, 12 + (119 - width(&activity)) / 2, 37, ink);
                    r.text(f, target.goal, 135, 24, ink);
                    if target.player_goal {
                        r.rect(135, 34, 6, 1, ink);
                    }
                    for dot in 0..target.skill.unwrap_or(0) {
                        r.rect(133 + i32::from(dot) * 4, 11, 2, 2, [230, 230, 230, 255]);
                    }
                    r.rect(144, 22, 4, 48, [240, 240, 240, 255]);
                    r.rect(145, 23, 2, 46, [0, 0, 0, 255]);
                    let filled = (target.damage * 46.).round() as i32;
                    r.rect(145, 69 - filled, 2, filled, [255, 255, 255, 255]);
                    let objective = match target.objective {
                        Some(true) => "MISSION OBJECTIVE",
                        Some(false) => "",
                        None => "OBJECTIVE ?",
                    };
                    r.text(f, objective, 12 + (134 - width(objective)) / 2, 111, ink);
                    r.text(f, &target.bearing, 12, 124, ink);
                    r.text(f, &target.metric, 147 - width(&target.metric), 124, ink);
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
                assert_eq!(instruments.radar_range, 0);
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
mod picking_tests {
    use super::*;
    fn contact(id: u32, bearing_rad: f64, distance_ft: f64) -> scope::Contact {
        scope::Contact {
            id,
            bearing_rad,
            distance_ft,
            heading_rad: 0.,
            track_eligible: true,
            destroyed: false,
            selected: false,
            acquired: false,
            stale: false,
            trail: vec![],
        }
    }
    fn readout(contacts: Vec<scope::Contact>) -> CombatReadout {
        CombatReadout {
            scope: scope::Scope {
                range_nmi: 10.,
                operating: true,
                contacts,
                ..Default::default()
            },
            ..Default::default()
        }
    }
    /// Screen position of one drawn contact, through the same projection and
    /// panel rectangle the renderer uses.
    fn point(i: &Instruments, slot: usize, c: &scope::Contact, size: [f64; 2]) -> (f64, f64) {
        let (rx, ry) = project(c.bearing_rad, c.distance_ft, 10.).expect("drawn contact");
        let (x, y, w, h) = i.screen_rect(slot, size);
        (x + rx * w / WIDTH as f64, y + ry * h / HEIGHT as f64)
    }
    #[test]
    fn a_click_designates_the_drawn_contact_in_both_layouts_and_at_every_size() {
        for layout in [Layout::Large, Layout::Small] {
            let mut i = Instruments::new(layout, None);
            let slot = i.pages.iter().position(|p| *p == 9).expect("radar page");
            for size in [[640., 480.], [1920., 1080.], [2560., 1080.], [800., 1200.]] {
                i.combat = Some(readout(vec![
                    contact(4, -0.4, 20_000.),
                    contact(7, 0.4, 40_000.),
                ]));
                let target = i.combat.as_ref().expect("readout").scope.contacts[1].clone();
                let hit = point(&i, slot, &target, size);
                let p = Some(hit);
                i.screen_pointer(p, size, true);
                assert!(i.screen_pointer(p, size, false));
                assert_eq!(i.designation.take(), Some(7));
                // An empty click leaves the designation alone.
                let empty = Some((hit.0, hit.1 - 40.));
                i.screen_pointer(empty, size, true);
                i.screen_pointer(empty, size, false);
                assert_eq!(i.designation.take(), None);
                // A press that is released elsewhere selects nothing.
                i.screen_pointer(p, size, true);
                i.screen_pointer(empty, size, false);
                assert_eq!(i.designation.take(), None);
            }
        }
    }
    #[test]
    fn hover_marks_the_contact_a_click_would_designate() {
        let mut i = Instruments::new(Layout::Large, None);
        let slot = i.pages.iter().position(|p| *p == 9).expect("radar page");
        let size = [1280., 960.];
        i.combat = Some(readout(vec![contact(5, 0.3, 30_000.)]));
        let target = i.combat.as_ref().expect("readout").scope.contacts[0].clone();
        let hit = point(&i, slot, &target, size);
        i.hover(Some(hit), size);
        assert_eq!(i.hovered, Some(5));
        assert!(i.crosshair.is_some());
        i.hover(Some((hit.0, hit.1 - 40.)), size);
        assert_eq!(i.hovered, None);
        i.hover(None, size);
        assert_eq!(i.hovered, None);
        assert_eq!(i.crosshair, None);
        // Hovering never designates by itself.
        assert_eq!(i.designation, None);
        i.hover(Some(hit), size);
        i.cancel_press();
        assert_eq!(i.hovered, None);
    }
    #[test]
    fn top_right_scope_picking_follows_reserved_debug_space() {
        let mut i = Instruments {
            pages: vec![7, 5, 4, 9],
            weapon_debug: true,
            combat: Some(readout(vec![contact(5, 0.3, 30_000.)])),
            ..Default::default()
        };
        let target = i.combat.as_ref().unwrap().scope.contacts[0].clone();
        for size in [[1280., 720.], [720., 1000.]] {
            let hit = point(&i, 3, &target, size);
            i.hover(Some(hit), size);
            assert_eq!(i.hovered, Some(5));
            assert!(i.crosshair.is_some());
            i.screen_pointer(Some(hit), size, true);
            i.screen_pointer(Some(hit), size, false);
            assert_eq!(i.designation.take(), Some(5));
        }
    }
    #[test]
    fn equal_distance_ties_resolve_by_stable_identity_and_stale_plots_never_pick() {
        let mut i = Instruments::new(Layout::Large, None);
        let slot = i.pages.iter().position(|p| *p == 9).expect("radar page");
        let size = [640., 480.];
        let mut overlapping = readout(vec![contact(9, 0., 20_000.), contact(3, 0., 20_000.)]);
        let p = Some(point(
            &i,
            slot,
            &overlapping.scope.contacts[0].clone(),
            size,
        ));
        overlapping.scope.contacts[0].stale = false;
        i.combat = Some(overlapping);
        i.screen_pointer(p, size, true);
        assert!(i.screen_pointer(p, size, false));
        assert_eq!(i.designation.take(), Some(3));
        let mut stale = readout(vec![contact(3, 0., 20_000.)]);
        stale.scope.contacts[0].stale = true;
        i.combat = Some(stale);
        i.screen_pointer(p, size, true);
        assert!(!i.screen_pointer(p, size, false));
        assert_eq!(i.designation.take(), None);
    }
    #[test]
    fn a_contact_beyond_the_plotted_range_is_neither_drawn_nor_picked() {
        assert!(project(0., 10. * 6076., 10.).is_some());
        assert!(project(0., 10.1 * 6076., 10.).is_none());
        assert!(project(std::f64::consts::PI, 1000., 10.).is_none());
        assert!(project(0., 1000., 0.).is_none());
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
        assert_eq!(i.channel, 1);
        assert!(i.control(i.selected, 3));
        assert!(i.history);
        assert!(!i.control(0, 3));
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
