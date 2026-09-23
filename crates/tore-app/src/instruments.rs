//! Small independent raster instruments. Layout fitted to supplied retail captures;
//! data is live, unsupported native sensors/camera modes are explicit.
mod envelope;
pub mod front_view;
use crate::{aircraft::Airframe, flight::State, menu::Sprite, scope};
use tore_formats::{Pic, font::Font};
/// Window raster size: the 81x80 frame picture at double size. See
/// docs/spec/instrument-bezel.md.
pub const WIDTH: usize = 162;
pub const HEIGHT: usize = 160;
/// The page screen inside the window: x, y, width, height. Page content is
/// drawn in screen coordinates, with (0, 0) at this rectangle's top-left.
pub const SCREEN: (i32, i32, i32, i32) = (12, 20, 138, 114);
/// Left edges of the four button click areas, which share one top edge and size.
const BUTTON_X: [i32; 4] = [18, 48, 78, 108];
const BUTTON_Y: i32 = 134;
const BUTTON_SIZE: (i32, i32) = (30, 26);
/// Titles wider than this are shortened and end in "...".
const TITLE_MAX_WIDTH: i32 = 98;
const GREEN: [u8; 4] = [132, 193, 126, 255];
/// Screen background of the green-on-black pages: a faint green, so they read
/// as small CRTs. Opinionated, requested by John on 2026-09-23. Retail screen
/// pixels measure black; see docs/spec/instrument-bezel.md.
const CRT_SCREEN: [u8; 4] = [4, 18, 6, 255];
const DIM: [u8; 4] = [48, 83, 44, 255];
const BRIGHT: [u8; 4] = [206, 240, 200, 255];
pub struct Raster {
    pub pixels: Vec<u8>,
    /// Window position of drawing coordinate (0, 0).
    origin: (i32, i32),
}
impl Raster {
    /// A window raster drawn in window coordinates.
    pub fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT * 4],
            origin: (0, 0),
        }
    }
    /// A window raster drawn in screen coordinates.
    #[cfg(test)]
    fn screen() -> Self {
        Self {
            origin: (SCREEN.0, SCREEN.1),
            ..Self::new()
        }
    }
    /// The pixel at a drawing coordinate.
    #[cfg(test)]
    fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let (x, y) = (x + self.origin.0, y + self.origin.1);
        let i = (y as usize * WIDTH + x as usize) * 4;
        self.pixels[i..i + 4].try_into().unwrap()
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
        let (x, y) = (x + self.origin.0, y + self.origin.1);
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
    /// Paints the frame picture at double size over the whole window, coloured
    /// through the live cockpit palette like the cockpit art around it.
    fn frame(&mut self, panel: &Pic, palette: &[[u8; 3]; 256]) {
        for y in 0..HEIGHT.min(panel.height * 2) {
            for x in 0..WIDTH.min(panel.width * 2) {
                let source = y / 2 * panel.width + x / 2;
                if !panel.mask[source] {
                    continue;
                }
                let [r, g, b] = palette[usize::from(panel.pixels[source])];
                let i = (y * WIDTH + x) * 4;
                self.pixels[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    /// Window label text centred on `centre`: one blank pixel between glyphs.
    fn label(&mut self, font: &Font, s: &str, centre: i32, top: i32, c: [u8; 4]) {
        let mut x = centre - (label_width(font, s) - 1).max(0) / 2;
        for ch in s.bytes() {
            let g = &font.glyphs[ch as usize];
            for &(xx, yy) in &g.pixels {
                self.rect(x + xx as i32, top + yy as i32, 1, 1, c);
            }
            x += g.advance as i32 + 1;
        }
    }
}
/// Envelope and the picture pages keep black; the green-symbology pages,
/// including their failed and switched-off states, get the CRT tint.
fn screen_background(id: u8) -> [u8; 4] {
    match id {
        1..=4 => [0, 0, 0, 255],
        _ => CRT_SCREEN,
    }
}
/// Width of a window label: each glyph's advance plus one, less the last gap.
fn label_width(font: &Font, s: &str) -> i32 {
    s.bytes()
        .map(|ch| font.glyphs[ch as usize].advance as i32 + 1)
        .sum::<i32>()
        .saturating_sub(1)
        .max(0)
}
/// The title as shown: shortened with "..." until it fits the title bar.
fn fit_title(font: &Font, title: &str) -> String {
    if label_width(font, title) <= TITLE_MAX_WIDTH {
        return title.into();
    }
    let mut kept = title.to_string();
    while !kept.is_empty() && label_width(font, &format!("{kept}...")) > TITLE_MAX_WIDTH {
        kept.pop();
    }
    format!("{kept}...")
}
/// The button under a window raster point, if any.
fn button_at(x: f64, y: f64) -> Option<usize> {
    let top = f64::from(BUTTON_Y);
    if !(top..top + f64::from(BUTTON_SIZE.1)).contains(&y) {
        return None;
    }
    BUTTON_X.iter().position(|&left| {
        let left = f64::from(left);
        (left..left + f64::from(BUTTON_SIZE.0)).contains(&x)
    })
}
/// A window raster point in screen coordinates.
fn to_screen((x, y): (f64, f64)) -> (f64, f64) {
    (x - f64::from(SCREEN.0), y - f64::from(SCREEN.1))
}
/// Agent-proposed mouse tolerance in instrument raster pixels.
pub const PICK_TOLERANCE: f64 = 7.;
/// Readability cap on combined noise density, so text, the selector and valid
/// track symbols stay legible over a noisy background.
const MAX_NOISE_DENSITY: f64 = 0.35;
/// Scope origin and plotted extents, in screen coordinates.
const SCOPE_ORIGIN: (f64, f64) = (69., 109.);
const SCOPE_HALF_WIDTH: f64 = 55.;
const SCOPE_DEPTH: f64 = 90.;

#[derive(Clone, Copy, Debug, PartialEq)]
enum RwrPlot {
    Ranged(i32, i32),
    BearingOnly { rim: (i32, i32), inner: (i32, i32) },
    Clipped { rim: (i32, i32), inner: (i32, i32) },
}

/// The RWR scope's largest scale. The RWR shows the shared scope range up to
/// this value and holds here while the radar is set further out.
const RWR_MAX_SCALE_NMI: f64 = 50.;

fn rwr_blink_on(tick: u64) -> bool {
    tick % 120 < 60
}

fn rwr_plot(bearing: f64, distance_nmi: Option<f64>, scale_nmi: f64, radius: f64) -> RwrPlot {
    let direction = (bearing.sin(), -bearing.cos());
    let at = |r: f64| {
        (
            (69. + direction.0 * r).round() as i32,
            (55. + direction.1 * r).round() as i32,
        )
    };
    match distance_nmi {
        Some(distance) if distance.is_finite() && distance <= scale_nmi => {
            let ratio = (distance.max(0.) / scale_nmi).clamp(0., 1.);
            let (x, y) = at(radius * ratio);
            RwrPlot::Ranged(x, y)
        }
        Some(_) => RwrPlot::Clipped {
            rim: at(radius),
            inner: at(radius - 5.),
        },
        None => RwrPlot::BearingOnly {
            rim: at(radius),
            inner: at(radius - 4.),
        },
    }
}

fn draw_rwr_rim(r: &mut Raster, plot: RwrPlot, colour: [u8; 4]) -> Option<(i32, i32)> {
    match plot {
        RwrPlot::Ranged(x, y) => Some((x, y)),
        RwrPlot::BearingOnly { rim, inner } => {
            r.line(rim, inner, colour);
            None
        }
        RwrPlot::Clipped { rim, inner } => {
            // A fork distinguishes a ranged contact clipped by the selected
            // scale from a simple bearing-only tick.
            r.line(rim, inner, colour);
            let tangent = ((rim.1 - inner.1).signum(), -(rim.0 - inner.0).signum());
            r.line(
                (rim.0 - tangent.0 * 2, rim.1 - tangent.1 * 2),
                (rim.0 + tangent.0 * 2, rim.1 + tangent.1 * 2),
                colour,
            );
            None
        }
    }
}

fn draw_rwr_emitter(r: &mut Raster, plot: RwrPlot, kind: scope::EmitterKind, colour: [u8; 4]) {
    let Some((x, y)) = draw_rwr_rim(r, plot, colour) else {
        return;
    };
    match kind {
        scope::EmitterKind::Ground => r.rect(x - 2, y - 2, 5, 5, colour),
        scope::EmitterKind::FriendlyAircraft => {
            r.line((x, y - 3), (x + 3, y), colour);
            r.line((x + 3, y), (x, y + 3), colour);
            r.line((x, y + 3), (x - 3, y), colour);
            r.line((x - 3, y), (x, y - 3), colour);
        }
        scope::EmitterKind::EnemyAircraft => {
            for row in -3i32..=3 {
                let half = 3 - row.abs();
                r.rect(x - half, y + row, half * 2 + 1, 1, colour);
            }
        }
        scope::EmitterKind::Unknown => {
            r.line((x - 2, y), (x + 2, y), colour);
            r.line((x, y - 2), (x, y + 2), colour);
        }
    }
}

fn draw_rwr_missile(r: &mut Raster, plot: RwrPlot, colour: [u8; 4]) {
    if let Some((x, y)) = draw_rwr_rim(r, plot, colour) {
        r.rect(x - 1, y - 1, 3, 3, colour);
    }
}

fn draw_rwr_indicator(
    r: &mut Raster,
    font: &Font,
    label: &str,
    x: i32,
    indicator: scope::Indicator,
    blink_on: bool,
) {
    let colour = match indicator {
        scope::Indicator::Off => return,
        scope::Indicator::Detected => GREEN,
        scope::Indicator::Tracking => BRIGHT,
        scope::Indicator::Incoming if blink_on => BRIGHT,
        scope::Indicator::Incoming => return,
    };
    r.text(font, label, x, 103, colour);
}

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
                let (x, y) = [(8, 8), (8, 312), (470, 312), (470, 8)][slot];
                (x, y, WIDTH as i32, HEIGHT as i32)
            }
            Self::Small => {
                let x = [8, 110, 212, 332, 434, 536][slot];
                (x, 377, 96, 95)
            }
        }
    }
}
#[derive(Default)]
pub struct CombatReadout {
    pub weapons: Vec<(String, u32, bool)>,
    pub chaff: u8,
    pub flares: u8,
    pub target: Option<crate::target_window::Readout>,
    pub scope: scope::Scope,
    pub rcs: scope::Rcs,
    pub rwr: scope::Rwr,
    pub rwr_failed: bool,
    pub envelope_target: Option<Vec<tore_formats::aircraft::Envelope>>,
}
pub struct Instruments {
    pub navigation: crate::navigation::Navigation,
    pub weapon_page: usize,
    envelope_mode: envelope::Mode,
    pub weapon_controls: Vec<usize>,
    pub combat: Option<CombatReadout>,
    pub pages: Vec<u8>,
    pub selected: usize,
    pub layout: Layout,
    pub(crate) other_pages: Vec<u8>,
    pub target_preview: Option<u32>,
    pub camera_target: Option<u32>,
    pub cameras: std::collections::BTreeMap<u8, Vec<u8>>,
    /// Flight data sampled with the requested and the shown forward-view frames.
    pub front_pending: Option<front_view::Symbology>,
    pub front_shown: Option<front_view::Symbology>,
    /// The HUD's primary color, shared by the forward-view symbology.
    pub hud_color: [u8; 3],
    /// The live cockpit palette, set by the host each frame. It colours the
    /// window frame and its labels, as it colours the cockpit art.
    pub palette: [[u8; 3]; 256],
    /// Shared scope range index into `tore_sim::sensors::RANGE_LADDER_NMI`,
    /// used by the radar scope and, capped at 50 miles, by the RWR.
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
            navigation: Default::default(),
            weapon_page: 0,
            envelope_mode: envelope::Mode::Current,
            weapon_controls: Vec::new(),
            combat: None,
            pages: vec![7, 5, 9, 4],
            selected: 0,
            layout: Layout::Large,
            other_pages: vec![7, 5, 6, 4, 9, 8],
            cameras: Default::default(),
            front_pending: None,
            front_shown: None,
            hud_color: [GREEN[0], GREEN[1], GREEN[2]],
            palette: [[0; 3]; 256],
            target_preview: None,
            camera_target: None,
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
    /// Reserve the upper-right weapon diagnostic area, when that panel is
    /// shown, without changing pointer transforms.
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
            let point = to_screen((
                (x - f64::from(ox)) * WIDTH as f64 / f64::from(w),
                (y - f64::from(oy)) * HEIGHT as f64 / f64::from(h),
            ));
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
                let (px, py) = to_screen((
                    (x - f64::from(ox)) * WIDTH as f64 / f64::from(w),
                    (y - f64::from(oy)) * HEIGHT as f64 / f64::from(h),
                ));
                ((0. ..f64::from(SCREEN.2)).contains(&px)
                    && (0. ..f64::from(SCREEN.3)).contains(&py))
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
                button_at(x, y).map(|b| (i, b))
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
        let scales = tore_sim::sensors::passive::SCALE_LADDER_NMI.len() - 1;
        match (self.pages.get(slot), button) {
            (Some(1), 0..=2) => {
                self.envelope_mode = [
                    envelope::Mode::Current,
                    envelope::Mode::All,
                    envelope::Mode::Compare,
                ][button];
            }
            (Some(0), 0) => self.rcs_range = self.rcs_range.saturating_sub(1),
            (Some(0), 1) => self.rcs_range = (self.rcs_range + 1).min(scales),
            (Some(5 | 9), 0) => self.step_range(-1),
            (Some(5 | 9), 1) => self.step_range(1),
            (Some(8), 0..=1) => {
                if self.weapon_controls.len() < 32 {
                    self.weapon_controls.push(button);
                }
            }
            (Some(8), 2) => {
                let pages = self
                    .combat
                    .as_ref()
                    .map_or(1, |c| c.weapons.len().div_ceil(6).max(1));
                self.weapon_page = (self.weapon_page + 1) % pages;
            }
            (Some(6), 0..=2) => {
                if self.navigation.pending.len() < 32 {
                    self.navigation.pending.push(button);
                }
            }
            (Some(9), 2) => self.cycle_channel(),
            (Some(9), 3) => self.history = !self.history,
            _ => return false,
        }
        true
    }
    /// Steps the shared radar and RWR range one ladder setting, stopping at
    /// either end. The RWR and radar buttons both use this.
    pub fn step_range(&mut self, delta: i32) {
        let last = tore_sim::sensors::RANGE_LADDER_NMI.len() as i32 - 1;
        self.radar_range = (self.radar_range as i32 + delta).clamp(0, last) as usize;
    }
    /// The RWR scale in nautical miles: the shared scope range, capped at 50.
    pub fn rwr_scale_nmi(&self) -> f64 {
        tore_sim::sensors::RANGE_LADDER_NMI[self
            .radar_range
            .min(tore_sim::sensors::RANGE_LADDER_NMI.len() - 1)]
        .min(RWR_MAX_SCALE_NMI)
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
    /// The aircraft's frame with its title, number and button letters over
    /// a cleared screen, ready for page content in screen coordinates.
    fn window(&self, id: u8, panel: &Pic, f: &Font, hud: &tore_formats::hud::Hud) -> Raster {
        let color = |index: u8| {
            let [r, g, b] = self.palette[usize::from(index)];
            [r, g, b, 255]
        };
        let mut r = Raster::new();
        r.frame(panel, &self.palette);
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
        let title_color = color(hud.title_color);
        r.label(f, &fit_title(f, title), 81, 6, title_color);
        r.label(f, &(id % 10).to_string(), 28, 6, title_color);
        let slot = self.pages.iter().position(|p| *p == id);
        for (b, &x) in BUTTON_X.iter().enumerate() {
            let label = match (id, b) {
                (1, 0) => "U",
                (1, 1) => "A",
                (1, 2) => "C",
                (0 | 5 | 6 | 8 | 9, 0) => "-",
                (0 | 5 | 6 | 8 | 9, 1) => "+",
                (6, 2) => {
                    if self.navigation.airports_mode {
                        "2"
                    } else {
                        "1"
                    }
                }
                (8, 2) => "P",
                (9, 2) => "M",
                (9, 3) => "Y",
                _ => "",
            };
            if label.is_empty() {
                continue;
            }
            r.label(f, label, x + 16, BUTTON_Y + 8, color(hud.button_color));
            // The press square covers the letter. Fitted: it stays while the
            // button is held, where the original keeps it until a redraw.
            if slot.is_some_and(|slot| self.pressed == Some((slot, b))) {
                r.rect(x + 10, BUTTON_Y + 6, 14, 14, color(hud.press_color));
            }
        }
        // Pages clear their own screen, including over the F/A-18's green inset.
        r.rect(
            SCREEN.0,
            SCREEN.1,
            SCREEN.2,
            SCREEN.3,
            screen_background(id),
        );
        r.origin = (SCREEN.0, SCREEN.1);
        r
    }
    pub fn page(&self, id: u8, h: &Airframe, s: &State) -> Raster {
        let f = &h.font;
        let mut r = self.window(id, &h.panel, f, &h.hud);
        let text = |r: &mut Raster, t: &str, x, y| r.text(f, t, x, y, GREEN);
        if (id == 9 && s.systems.has(32))
            || (id == 5
                && (s.systems.counts[32] > 1
                    || self
                        .combat
                        .as_ref()
                        .is_some_and(|c| c.rwr_failed || !c.rwr.operating)))
        {
            return r;
        }
        if id == 6 && s.systems.has(33) {
            text(&mut r, "NAV FAILED", 35, 52);
            return r;
        }
        match id {
            0 => {
                let (cx, cy, radius) = (69i32, 55i32, 48.);
                r.circle(cx, cy, radius, DIM);
                r.circle(cx, cy, radius / 2., DIM);
                r.line((cx, 1), (cx, 112), DIM);
                r.line((2, cy), (136, cy), DIM);
                for (label, x, y) in [
                    ("0", 66, 3),
                    ("90", 121, 51),
                    ("180", 63, 105),
                    ("270", 5, 51),
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
                    text(&mut r, &format!("{:.0}", c.rcs.scale_nmi), 115, 4);
                    if over_range {
                        text(&mut r, "OVER", 3, 4);
                    }
                    text(&mut r, &format!("SIG {:.0}", c.rcs.signature), 3, 103);
                } else {
                    text(&mut r, "NO EXPOSURE DATA", 19, 52);
                }
            }
            7 => {
                let green = [72, 172, 55, 255];
                let amber = [235, 187, 60, 255];
                let red = [235, 70, 50, 255];
                let width = |value: &str| {
                    value
                        .bytes()
                        .map(|ch| f.glyphs[ch as usize].advance as i32)
                        .sum::<i32>()
                };
                for (i, (label, value, color)) in [
                    ("THR", s.throttle * 100., green),
                    (
                        "TEMP",
                        s.systems.engine.temperature,
                        if s.systems.engine.temperature >= 75. {
                            red
                        } else if s.systems.engine.temperature > 25. {
                            amber
                        } else {
                            green
                        },
                    ),
                    (
                        "OIL",
                        s.systems.oil_pressure() * 100.,
                        if s.systems.oil_pressure() <= 0.25 {
                            red
                        } else if s.systems.oil_pressure() < 0.75 {
                            amber
                        } else {
                            green
                        },
                    ),
                    (
                        "HYD",
                        s.systems.fluids.hydraulic * 100.,
                        if s.systems.fluids.hydraulic <= 0.25 {
                            red
                        } else if s.systems.fluids.hydraulic < 0.75 {
                            amber
                        } else {
                            green
                        },
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    let y = 12 + i as i32 * 14;
                    r.text(f, label, 9, y, color);
                    let value = format!("{value:.0}%");
                    r.text(f, &value, 127 - width(&value), y, color);
                }
                r.line((9, 70), (127, 70), green);
                r.text(f, "FUEL", 9, 80, green);
                let fuel = format!("{:.0} LBS", s.fuel);
                r.text(f, &fuel, 127 - width(&fuel), 80, green);
                r.text(f, "(+ EXT", 9, 94, green);
                let fuel = format!("{:.0} LBS)", s.systems.external_lbs());
                r.text(f, &fuel, 127 - width(&fuel), 94, green);
            }
            5 => {
                let (cx, cy, radius) = (69i32, 55i32, 48.);
                r.line((2, cy), (136, cy), DIM);
                r.line((cx, 1), (cx, 112), DIM);
                r.circle(cx, cy, radius, DIM);
                r.circle(cx, cy, radius / 2., DIM);
                let scale = self.rwr_scale_nmi();
                text(&mut r, &format!("{scale:.0}"), 115, 4);
                r.rect(67, 53, 5, 1, GREEN);
                r.rect(67, 53, 1, 5, GREEN);
                r.rect(71, 53, 1, 5, GREEN);
                r.rect(67, 57, 5, 1, GREEN);
                if let Some(combat) = &self.combat {
                    let rwr = &combat.rwr;
                    let blink_on = rwr_blink_on(rwr.tick);
                    for emitter in &rwr.emitters {
                        if emitter.state == scope::EmitterState::Tracking && !blink_on {
                            continue;
                        }
                        let colour = if emitter.state == scope::EmitterState::Detected {
                            GREEN
                        } else {
                            BRIGHT
                        };
                        let plot =
                            rwr_plot(emitter.bearing_rad, emitter.distance_nmi, scale, radius);
                        draw_rwr_emitter(&mut r, plot, emitter.kind, colour);
                    }
                    for missile in &rwr.missiles {
                        if missile.known_targeting_receiver && !missile.stale && !blink_on {
                            continue;
                        }
                        let plot =
                            rwr_plot(missile.bearing_rad, missile.distance_nmi, scale, radius);
                        draw_rwr_missile(&mut r, plot, if missile.stale { DIM } else { BRIGHT });
                    }
                    draw_rwr_indicator(&mut r, f, "R", 121, rwr.radar_indicator, blink_on);
                    draw_rwr_indicator(&mut r, f, "I", 130, rwr.infrared_indicator, blink_on);
                }
                if s.jammer && s.engine {
                    text(&mut r, "JAM", 6, 103);
                }
            }
            9 => {
                for i in 1..4 {
                    let y = i * SCREEN.3 / 4;
                    r.line((1, y), (137, y), DIM);
                    let x = i * SCREEN.2 / 4;
                    r.line((x, 1), (x, 113), DIM);
                }
                match &self.combat {
                    None => text(&mut r, "NO SENSOR DATA", 19, 52),
                    Some(c) if !c.scope.operating => {
                        let reason = match (c.scope.infrared, c.scope.unavailable) {
                            (_, Some(text)) => text,
                            (true, _) => "IR UNAVAILABLE",
                            _ => "RADAR OFF",
                        };
                        text(&mut r, reason, 29, 54);
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
                            for y in 1..113 {
                                for x in 1..138 {
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
                            r.rect(x - 2, y - 2, 5, 5, colour);
                            if scope.mode == Some("TWS")
                                && let Some(heading) = contact.heading_rad
                            {
                                let tip = (
                                    x + (heading.sin() * 9.).round() as i32,
                                    y - (heading.cos() * 9.).round() as i32,
                                );
                                r.line((x, y), tip, colour);
                            }
                            if contact.selected {
                                r.rect(x - 7, y - 3, 2, 7, colour);
                                r.rect(x + 6, y - 3, 2, 7, colour);
                            } else if self.hovered == Some(contact.id) {
                                for (dx, dy) in [(-4, -4), (4, -4), (-4, 4), (4, 4)] {
                                    r.rect(x + dx, y + dy, 1, 1, colour);
                                }
                            }
                        }
                        text(&mut r, scope.mode.unwrap_or(scope.channel), 6, 4);
                        text(&mut r, &format!("{range:.0}"), 115, 4);
                        if scope.history {
                            text(&mut r, "HIST", 6, 103);
                        }
                        if let Some(status) = scope.status {
                            let width: usize =
                                status.bytes().map(|ch| f.glyphs[ch as usize].advance).sum();
                            text(&mut r, status, 136 - width as i32, 103);
                        }
                    }
                }
                if let Some((x, y)) = self.crosshair {
                    // Takeover and drawing cover the black screen, not just the contact plot.
                    let (right, bottom) = (SCREEN.2 - 1, SCREEN.3 - 1);
                    if x - 4 >= 0 {
                        r.line((0, y), (x - 4, y), GREEN);
                    }
                    if x + 4 <= right {
                        r.line((x + 4, y), (right, y), GREEN);
                    }
                    if y - 4 >= 0 {
                        r.line((x, 0), (x, y - 4), GREEN);
                    }
                    if y + 4 <= bottom {
                        r.line((x, y + 4), (x, bottom), GREEN);
                    }
                }
            }
            8 => {
                if let Some(c) = &self.combat {
                    let page = self.weapon_page % c.weapons.len().div_ceil(6).max(1);
                    for (row, (name, count, selected)) in
                        c.weapons.iter().skip(page * 6).take(6).enumerate()
                    {
                        let y = 5 + row as i32 * 14;
                        let colour = if *selected { BRIGHT } else { GREEN };
                        if *selected {
                            r.text(f, ">", 5, y, colour);
                        }
                        let count = count.to_string();
                        let width: usize =
                            count.bytes().map(|ch| f.glyphs[ch as usize].advance).sum();
                        r.text(f, &count, 48 - width as i32, y, colour);
                        let mut width = 0;
                        let name: String = name
                            .bytes()
                            .take_while(|ch| {
                                width += f.glyphs[*ch as usize].advance;
                                width <= 80
                            })
                            .map(char::from)
                            .collect();
                        r.text(f, &name, 54, y, colour);
                    }
                    text(&mut r, &format!("{} CHAFF", c.chaff), 4, 101);
                    let flare = format!("{} FLARE", c.flares);
                    let width: usize = flare.bytes().map(|ch| f.glyphs[ch as usize].advance).sum();
                    text(&mut r, &flare, 135 - width as i32, 101);
                } else {
                    text(&mut r, "NO WEAPONS", 9, 14);
                }
            }
            6 => {
                let nav = &self.navigation;
                if let Some(selected) = nav.index() {
                    let page = selected / 3 * 3;
                    for (index, entry) in nav.entries().iter().enumerate().skip(page).take(3) {
                        let y = 4 + (index - page) as i32 * 28;
                        let colour = if index == selected { BRIGHT } else { GREEN };
                        let label = format!(
                            "{}. {}",
                            (b'A' + (index % 26) as u8) as char,
                            entry.name.to_ascii_uppercase()
                        );
                        let mut width = 0;
                        let label: String = label
                            .bytes()
                            .take_while(|ch| {
                                width += f.glyphs[*ch as usize].advance;
                                width <= 130
                            })
                            .map(char::from)
                            .collect();
                        r.text(f, &label, 4, y, colour);
                        let bearing = format!("{:03}", entry.bearing(s.position));
                        r.text(f, &bearing, 16, y + 12, colour);
                        let width: usize = bearing
                            .bytes()
                            .map(|ch| f.glyphs[ch as usize].advance)
                            .sum();
                        r.circle(18 + width as i32, y + 14, 1., colour);
                        r.text(
                            f,
                            &format!(", {:.1} NM", entry.distance(s.position) / 6076.12),
                            21 + width as i32,
                            y + 12,
                            colour,
                        );
                    }
                    let speed = s.velocity[0].hypot(s.velocity[2]);
                    let eta = if speed >= 1. {
                        let seconds =
                            (nav.entries()[selected].distance(s.position) / speed).round() as u64;
                        format!("ETA {}:{:02}", seconds / 60, seconds % 60)
                    } else {
                        "ETA --:--".into()
                    };
                    text(&mut r, &eta, 41, 101);
                } else {
                    text(
                        &mut r,
                        if nav.airports_mode {
                            "NO SAFE AIRPORTS"
                        } else {
                            "NO WAYPOINTS"
                        },
                        6,
                        44,
                    );
                    text(
                        &mut r,
                        if nav.airports_mode {
                            "2 AIRPORTS"
                        } else {
                            "1 MISSION"
                        },
                        19,
                        68,
                    );
                    text(&mut r, "ETA --:--", 41, 101);
                }
            }
            1 => envelope::draw(
                &mut r,
                f,
                &h.profile.envelopes,
                s,
                self.envelope_mode,
                self.combat.as_ref(),
            ),
            4 => {
                if let Some(target) = self.combat.as_ref().and_then(|c| c.target.as_ref()) {
                    r.rect(0, 0, SCREEN.2, SCREEN.3, [185, 185, 185, 255]);
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
                            0,
                            0,
                            SCREEN.2,
                            SCREEN.3,
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
                    r.text(f, &label, 1 + (119 - width(&label)) / 2, 3, ink);
                    let activity = fit(&target.activity, 119);
                    r.text(f, &activity, 1 + (119 - width(&activity)) / 2, 16, ink);
                    r.text(f, target.goal, 124, 3, ink);
                    if target.player_goal {
                        r.rect(124, 13, 6, 1, ink);
                    }
                    // Skill dots sit on the title bar, above the screen.
                    for dot in 0..target.skill.unwrap_or(0) {
                        r.rect(122 + i32::from(dot) * 4, -10, 2, 2, [230, 230, 230, 255]);
                    }
                    r.rect(133, 1, 4, 48, [240, 240, 240, 255]);
                    r.rect(134, 2, 2, 46, [0, 0, 0, 255]);
                    let filled = (target.damage * 46.).round() as i32;
                    r.rect(134, 48 - filled, 2, filled, [255, 255, 255, 255]);
                    let objective = match target.objective {
                        Some(crate::target_window::TargetObjective::Survive) => "Obj: Survive",
                        Some(crate::target_window::TargetObjective::Destroy) => "Obj: Destroy",
                        None => "",
                    };
                    r.text(f, objective, 1 + (134 - width(objective)) / 2, 90, ink);
                    r.text(f, &target.bearing, 1, 103, ink);
                    r.text(f, &target.metric, 136 - width(&target.metric), 103, ink);
                } else {
                    text(&mut r, "NO TARGET", 37, 52);
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
                        0,
                        0,
                        SCREEN.2,
                        SCREEN.3,
                    );
                    if id == 2
                        && !s.systems.has(31)
                        && let Some(symbology) = &self.front_shown
                    {
                        front_view::draw(&mut r, f, symbology, self.hud_color);
                    }
                } else {
                    text(&mut r, "CAMERA LOADING", 19, 52);
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
    #[test]
    fn rwr_blink_uses_fixed_simulation_ticks() {
        assert!(rwr_blink_on(0));
        assert!(rwr_blink_on(59));
        assert!(!rwr_blink_on(60));
        assert!(!rwr_blink_on(119));
        assert!(rwr_blink_on(120));
    }

    #[test]
    fn rwr_follows_the_radar_range_and_holds_at_50_miles() {
        let mut i = Instruments::default();
        // Flight start: RWR and radar both read 10 miles, and the weapon
        // envelope opens on the current-conditions (U) mode.
        assert_eq!(i.rwr_scale_nmi(), 10.);
        assert_eq!(i.controls().range_index, 1);
        assert_eq!(i.envelope_mode, envelope::Mode::Current);
        for (index, rwr) in [(0, 5.), (1, 10.), (2, 25.), (3, 50.), (4, 50.), (5, 50.)] {
            i.radar_range = index;
            assert_eq!(i.rwr_scale_nmi(), rwr);
            assert!(rwr <= tore_sim::sensors::RANGE_LADDER_NMI[index]);
        }
    }

    #[test]
    fn rwr_buttons_step_the_shared_range() {
        let mut i = Instruments::default();
        let rwr = i.pages.iter().position(|&p| p == 5).unwrap();
        let radar = i.pages.iter().position(|&p| p == 9).unwrap();
        assert!(i.control(rwr, 0));
        assert_eq!((i.radar_range, i.rwr_scale_nmi()), (0, 5.));
        assert!(i.control(rwr, 0));
        assert_eq!(i.radar_range, 0);
        let mut seen = vec![];
        for _ in 0..8 {
            assert!(i.control(rwr, 1));
            seen.push((i.radar_range, i.rwr_scale_nmi()));
        }
        // Past 50 the RWR buttons keep moving the radar out while the RWR
        // holds at its 50-mile maximum; the ladder stops at 150.
        assert_eq!(
            seen,
            [
                (1, 10.),
                (2, 25.),
                (3, 50.),
                (4, 50.),
                (5, 50.),
                (5, 50.),
                (5, 50.),
                (5, 50.)
            ]
        );
        assert!(i.control(rwr, 0));
        assert_eq!((i.radar_range, i.rwr_scale_nmi()), (4, 50.));
        // Radar buttons move the same setting the RWR reads.
        assert!(i.control(radar, 0));
        assert!(i.control(radar, 0));
        assert_eq!((i.radar_range, i.rwr_scale_nmi()), (2, 25.));
        i.step_range(-9);
        assert_eq!(i.radar_range, 0);
    }

    #[test]
    fn rwr_projection_distinguishes_range_quality() {
        assert_eq!(rwr_plot(0., Some(5.), 10., 48.), RwrPlot::Ranged(69, 31));
        assert_eq!(
            rwr_plot(std::f64::consts::FRAC_PI_2, None, 10., 48.),
            RwrPlot::BearingOnly {
                rim: (117, 55),
                inner: (113, 55)
            }
        );
        assert_eq!(
            rwr_plot(std::f64::consts::PI, Some(11.), 10., 48.),
            RwrPlot::Clipped {
                rim: (69, 103),
                inner: (69, 98)
            }
        );
        for scale in [5., 10., 20., 30., 50.] {
            for (bearing, expected) in [
                (0., (69, 7)),
                (std::f64::consts::FRAC_PI_2, (117, 55)),
                (std::f64::consts::PI, (69, 103)),
                (-std::f64::consts::FRAC_PI_2, (21, 55)),
            ] {
                assert_eq!(
                    rwr_plot(bearing, Some(scale), scale, 48.),
                    RwrPlot::Ranged(expected.0, expected.1)
                );
            }
        }
    }

    #[test]
    fn manual_rwr_symbols_have_distinct_synthetic_rasters() {
        fn pixels(kind: scope::EmitterKind) -> Vec<u8> {
            let mut raster = Raster::screen();
            draw_rwr_emitter(&mut raster, RwrPlot::Ranged(69, 55), kind, GREEN);
            raster.pixels
        }
        let ground = pixels(scope::EmitterKind::Ground);
        let friendly = pixels(scope::EmitterKind::FriendlyAircraft);
        let enemy = pixels(scope::EmitterKind::EnemyAircraft);
        let unknown = pixels(scope::EmitterKind::Unknown);
        assert_ne!(ground, friendly);
        assert_ne!(friendly, enemy);
        assert_ne!(enemy, unknown);
        assert_ne!(ground, unknown);
    }

    /// The centre of a button's click area, in flight-canvas coordinates.
    fn button(i: &Instruments, slot: usize, b: usize) -> (f64, f64) {
        let (x, y, w, h) = i.layout.rect(slot);
        (
            x as f64 + f64::from(BUTTON_X[b] + 15) * w as f64 / WIDTH as f64,
            y as f64 + f64::from(BUTTON_Y + 13) * h as f64 / HEIGHT as f64,
        )
    }
    #[test]
    fn nav_and_weapon_buttons_share_pointer_and_hardware_controls() {
        let mut i = Instruments::new(Layout::Large, Some(6));
        for button_id in 0..3 {
            let point = Some(button(&i, 0, button_id));
            i.pointer(point, true);
            assert!(i.pointer(point, false));
        }
        assert_eq!(i.navigation.pending, [0, 1, 2]);
        i.pages[0] = 8;
        assert!(i.control(0, 0));
        assert!(i.control(0, 1));
        assert_eq!(i.weapon_controls, [0, 1]);
        assert!(i.control(0, 2));
        assert_eq!(i.weapon_page, 0);
        i.combat = Some(CombatReadout {
            weapons: vec![("TEST".into(), 1, false); 7],
            ..Default::default()
        });
        assert!(i.control(0, 2));
        assert_eq!(i.weapon_page, 1);
        assert!(i.control(0, 2));
        assert_eq!(i.weapon_page, 0);
        assert!(!i.control(0, 3));
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
                let point = Some((x + 33. * w / WIDTH as f64, y + 147. * h / HEIGHT as f64));
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
            assert_eq!(i.radar_range, tore_sim::sensors::DEFAULT_RANGE_INDEX);
            for _ in 0..10 {
                i.pointer(Some(minus), true);
                assert!(i.pointer(Some(minus), false));
            }
            assert_eq!(i.radar_range, 0);
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
mod frame_tests {
    use super::*;
    use tore_formats::{font::Glyph, hud::Hud};

    /// Synthetic 81x80 frame: every pixel a different mix of indices 0..64.
    fn panel() -> Pic {
        Pic {
            width: 81,
            height: 80,
            pixels: (0..81 * 80)
                .map(|i| ((i * 7 + i / 81) % 64) as u8)
                .collect(),
            mask: vec![true; 81 * 80],
            palette: vec![],
            glyphs: vec![],
        }
    }
    /// Glyphs light only their top-left cell pixel, so label placement is exact.
    fn font(advance: impl Fn(u8) -> usize, ink: bool) -> Font {
        Font {
            height: 10,
            glyphs: (0..=255u8)
                .map(|ch| Glyph {
                    advance: advance(ch),
                    pixels: if ink { vec![(0, 0)] } else { vec![] },
                })
                .collect(),
        }
    }
    fn hud() -> Hud {
        Hud {
            primary_color: 40,
            panel: Some("~ab_p".into()),
            title_color: 39,
            button_color: 19,
            press_color: 4,
        }
    }
    fn palette(shift: u8) -> [[u8; 3]; 256] {
        std::array::from_fn(|i| {
            let i = i as u8;
            [i.wrapping_add(shift), 255 - i, i / 2]
        })
    }
    fn rgba(c: [u8; 3]) -> [u8; 4] {
        [c[0], c[1], c[2], 255]
    }
    fn inside_screen(x: i32, y: i32) -> bool {
        (SCREEN.0..SCREEN.0 + SCREEN.2).contains(&x) && (SCREEN.1..SCREEN.1 + SCREEN.3).contains(&y)
    }

    #[test]
    fn frame_fills_the_window_at_double_size_through_the_live_palette() {
        let panel = panel();
        let blank = font(|_| 5, false);
        for shift in [0, 90] {
            let i = Instruments {
                palette: palette(shift),
                ..Default::default()
            };
            let r = i.window(9, &panel, &blank, &hud());
            for y in 0..HEIGHT as i32 {
                for x in 0..WIDTH as i32 {
                    let expected = if inside_screen(x, y) {
                        CRT_SCREEN
                    } else {
                        let index = panel.pixels[(y / 2 * 81 + x / 2) as usize];
                        rgba(i.palette[usize::from(index)])
                    };
                    assert_eq!(r.at(x - SCREEN.0, y - SCREEN.1), expected, "{x},{y}");
                }
            }
        }
    }

    #[test]
    fn green_pages_get_the_crt_tint_and_picture_pages_stay_black() {
        let (panel, blank) = (panel(), font(|_| 5, false));
        let i = Instruments::default();
        for id in 0..10 {
            let r = i.window(id, &panel, &blank, &hud());
            let expected = if (1..=4).contains(&id) {
                [0, 0, 0, 255]
            } else {
                CRT_SCREEN
            };
            for (x, y) in [(0, 0), (137, 113), (69, 57)] {
                assert_eq!(r.at(x, y), expected, "page {id}");
            }
        }
        // Faint: dark, and green is the strongest channel.
        assert!(CRT_SCREEN[1] > CRT_SCREEN[0] && CRT_SCREEN[1] > CRT_SCREEN[2]);
        assert!(CRT_SCREEN[..3].iter().all(|c| *c < 32));
    }

    #[test]
    fn title_number_and_letters_sit_at_the_spec_positions_in_hud_colours() {
        // Advances chosen so ENVELOPE is 51 pixels wide, as in WIN11.
        let advance = |ch| match ch {
            b'N' | b'V' | b'O' | b'P' => 6,
            _ => 5,
        };
        let f = font(advance, true);
        let i = Instruments {
            palette: palette(0),
            pages: vec![1],
            ..Default::default()
        };
        let r = i.window(1, &panel(), &f, &hud());
        let at = |x, y| r.at(x - SCREEN.0, y - SCREEN.1);
        let title = rgba(i.palette[39]);
        assert_eq!(label_width(&f, "ENVELOPE"), 51);
        // Centred on x = 81 with one blank pixel between glyphs, top at y = 6.
        let mut x = 56;
        for ch in b"ENVELOPE" {
            assert_eq!(at(x, 6), title);
            x += advance(*ch) as i32 + 1;
        }
        // The page number is centred on x = 28.
        assert_eq!(at(26, 6), title);
        // U, A and C centred on each button's x + 16, top at y + 8.
        let letter = rgba(i.palette[19]);
        for left in [18, 48, 78] {
            assert_eq!(at(left + 14, 142), letter);
        }
        // The fourth Envelope button has no letter: the frame shows through.
        let index = panel().pixels[142 / 2 * 81 + 122 / 2];
        assert_eq!(at(122, 142), rgba(i.palette[usize::from(index)]));
    }

    #[test]
    fn a_held_button_shows_the_press_square_over_its_letter() {
        let f = font(|_| 5, true);
        let mut i = Instruments {
            palette: palette(0),
            pages: vec![7, 9],
            ..Default::default()
        };
        i.pressed = Some((1, 2));
        let r = i.window(9, &panel(), &f, &hud());
        let at = |x, y| r.at(x - SCREEN.0, y - SCREEN.1);
        let press = rgba(i.palette[4]);
        for y in 140..154 {
            for x in 88..102 {
                assert_eq!(at(x, y), press, "{x},{y}");
            }
        }
        for (x, y) in [(87, 140), (102, 153), (95, 139), (95, 154)] {
            let index = panel().pixels[(y / 2 * 81 + x / 2) as usize];
            assert_eq!(at(x, y), rgba(i.palette[usize::from(index)]), "{x},{y}");
        }
        // Other buttons keep their letters; another page's press shows nothing.
        assert_eq!(at(32, 142), rgba(i.palette[19]));
        let other = i.window(7, &panel(), &f, &hud());
        for (x, y) in [(88, 140), (95, 147), (101, 153)] {
            let index = panel().pixels[(y / 2 * 81 + x / 2) as usize];
            let frame = rgba(i.palette[usize::from(index)]);
            assert_eq!(other.at(x - SCREEN.0, y - SCREEN.1), frame);
        }
    }

    #[test]
    fn long_titles_are_shortened_with_an_ellipsis_to_98_pixels() {
        let f = font(|_| 10, true);
        assert_eq!(fit_title(&f, "RADAR"), "RADAR");
        let fitted = fit_title(&f, "OTHER VIEW");
        assert_eq!(fitted, "OTHER ...");
        assert!(label_width(&f, &fitted) <= TITLE_MAX_WIDTH);
        assert!(label_width(&f, "OTHER V...") > TITLE_MAX_WIDTH);
    }

    #[test]
    fn button_click_areas_are_30_by_26_along_the_bottom() {
        for (b, left) in BUTTON_X.iter().enumerate() {
            let left = f64::from(*left);
            assert_eq!(button_at(left, 134.), Some(b));
            assert_eq!(button_at(left + 29.9, 159.9), Some(b));
        }
        for (x, y) in [(17.9, 140.), (138., 140.), (30., 133.9), (30., 160.)] {
            assert_eq!(button_at(x, y), None, "{x},{y}");
        }
        // Buttons still work through the scaled window in both layouts.
        for layout in [Layout::Large, Layout::Small] {
            let mut i = Instruments::new(layout, Some(9));
            let (x, y, w, h) = layout.rect(0);
            let at = |rx: f64, ry: f64| {
                Some((
                    f64::from(x) + rx * f64::from(w) / WIDTH as f64,
                    f64::from(y) + ry * f64::from(h) / HEIGHT as f64,
                ))
            };
            i.pointer(at(110., 150.), true);
            assert!(i.pointer(at(110., 150.), false));
            assert!(i.history);
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
            heading_rad: Some(0.),
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
        let (rx, ry) = (rx + f64::from(SCREEN.0), ry + f64::from(SCREEN.1));
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
            combat: Some(readout(vec![contact(5, 0.3, 30_000.)])),
            ..Default::default()
        };
        // The diagnostic panel is hidden by default and reserves no space.
        assert!(!i.weapon_debug);
        let target = i.combat.as_ref().unwrap().scope.contacts[0].clone();
        for debug in [false, true] {
            i.weapon_debug = debug;
            for size in [[1280f64, 720.], [720., 1000.]] {
                let scale = (size[0] / 640.).min(size[1] / 480.);
                let normal = Layout::Large.rect_on(3, size);
                let shift = if debug { 104. * scale } else { 0. };
                assert_eq!(i.screen_rect(3, size).1, normal.1 + shift);
                let hit = point(&i, 3, &target, size);
                i.hover(Some(hit), size);
                assert_eq!(i.hovered, Some(5));
                assert!(i.crosshair.is_some());
                i.screen_pointer(Some(hit), size, true);
                i.screen_pointer(Some(hit), size, false);
                assert_eq!(i.designation.take(), Some(5));
            }
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
        assert_eq!(i.radar_range, 0);
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
