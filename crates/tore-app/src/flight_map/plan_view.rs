//! The creator's planning map view (slices CL-R and CL-R2, John 2026-10-11).
//!
//! It opens zoomed in on the objective area and the starts, never shows a
//! black border (the widest zoom is the one where the theater just fills the
//! plot, and panning stops at the theater's edges), scrolls with a horizontal
//! and a vertical scroll bar beside the plot, and zooms with the retail
//! vertical rocker as well as the in-game map's keys.
//!
//! **Opinionated** (John, 2026-10-11): the scroll bars and the zoom rocker are
//! additions, since neither retail's creator nor its in-game map has a
//! picture to scroll. The bars are drawn flat in the menus' greys (retail has
//! no horizontal bar or arrow ends decoded); the rocker is the retail rocker's
//! own art and tilts as the debrief's does.
use super::{
    INK, OBJECTIVE, Placed, Plan, PlanSide, Projection, Tile, advance, draw_ground, draw_objective,
    draw_placed, objective_mark, place_marker_in, place_starts, symbol,
};
use crate::hud::Paint;
use crate::menu::{Canvas, Sprite};
use crate::rocker::Rocker;
use crate::terrain::Terrain;
use std::time::Instant;
use tore_formats::font::Font;
use tore_formats::theater::CELL_FEET;
use tore_sim::sensors::FEET_PER_NAUTICAL_MILE as NMI;

type Rect = (i32, i32, i32, i32);

/// The plot: the in-game map's, less the room for the two scroll bars.
pub const PLOT: Rect = (12, 30, 458, 390);
/// The scroll bars' thickness, and the arrow buttons at their ends.
const BAR: i32 = 16;
/// The shortest a thumb is drawn, so it can be grabbed.
const MIN_THUMB: i32 = 14;
/// The horizontal bar under the plot, the vertical one beside it and the
/// square where they meet.
const HORIZONTAL: Rect = (PLOT.0, PLOT.1 + PLOT.3 + 2, PLOT.2, BAR);
const VERTICAL: Rect = (PLOT.0 + PLOT.2 + 2, PLOT.1, BAR, PLOT.3);
/// The rocker's top left; its two halves are 18 by 16 (the retail rocker).
const ROCKER: (i32, i32) = (582, 214);
const HALF: (i32, i32) = (18, 16);
/// How far an arrow button or an arrow key moves the view, and a click on the
/// track, as parts of the view's own width or height.
const ARROW_STEP: f64 = 0.125;
const KEY_STEP: f64 = 0.25;
const PAGE_STEP: f64 = 0.875;
/// The planning map's Close button.
const PLAN_CLOSE: (i32, i32, i32) = (518, 404, 95);
/// The narrowest view the map opens at (the opening view is never tighter
/// than this, so the picture is not magnified to a blur; `fitted`).
const OPENING_NARROWEST_NMI: f64 = 50.;
/// The in-game map's widest zoom steps, nautical miles, that a theater wider
/// than them can zoom to (25 to 800 nm).
const STEPS_NMI: [f64; 6] = [800., 400., 200., 100., 50., 25.];

/// The part of a scroll bar a press landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// The arrow button at the start (left or top).
    Less,
    /// The arrow button at the end (right or bottom).
    More,
    /// The track before the thumb: a page back.
    PageLess,
    /// The track after the thumb: a page on.
    PageMore,
    Thumb,
}

/// One scroll bar: a rectangle, an arrow button at each end, a track between
/// and a thumb on it. It holds no state: the view says how much there is
/// (`total`), how much shows (`visible`) and where the shown part starts
/// (`first`), all in feet.
#[derive(Clone, Copy, Debug)]
struct Bar {
    rect: Rect,
    vertical: bool,
}

impl Bar {
    /// The bar's start and length along its own axis, in pixels.
    fn axis(&self) -> (i32, i32) {
        if self.vertical {
            (self.rect.1, self.rect.3)
        } else {
            (self.rect.0, self.rect.2)
        }
    }
    fn along(&self, (x, y): (i32, i32)) -> i32 {
        if self.vertical { y } else { x }
    }
    /// The thumb's start and length along the axis for a view.
    fn thumb(&self, total: f64, visible: f64, first: f64) -> (i32, i32) {
        let (start, length) = self.axis();
        let track = (length - 2 * BAR).max(0);
        let ratio = if total > 0. {
            (visible / total).clamp(0., 1.)
        } else {
            1.
        };
        let size = ((f64::from(track) * ratio).round() as i32)
            .max(MIN_THUMB)
            .min(track);
        let travel = f64::from(track - size);
        let span = (total - visible).max(0.);
        let at = if span > 0. {
            (first / span).clamp(0., 1.) * travel
        } else {
            0.
        };
        (start + BAR + at.round() as i32, size)
    }
    /// The `first` a thumb that starts at `thumb_start` pixels stands for.
    fn first_at(&self, thumb_start: i32, total: f64, visible: f64) -> f64 {
        let (start, length) = self.axis();
        let track = (length - 2 * BAR).max(0);
        let (_, size) = self.thumb(total, visible, 0.);
        let travel = f64::from((track - size).max(1));
        let span = (total - visible).max(0.);
        (f64::from(thumb_start - start - BAR) / travel).clamp(0., 1.) * span
    }
    /// What a point is on, if the bar is under it and has anything to scroll.
    fn part(&self, point: (i32, i32), total: f64, visible: f64, first: f64) -> Option<Part> {
        let (x, y, w, h) = self.rect;
        if point.0 < x || point.1 < y || point.0 >= x + w || point.1 >= y + h {
            return None;
        }
        let (start, length) = self.axis();
        let a = self.along(point);
        if a < start + BAR {
            return Some(Part::Less);
        }
        if a >= start + length - BAR {
            return Some(Part::More);
        }
        if total <= visible {
            return None;
        }
        let (thumb, size) = self.thumb(total, visible, first);
        Some(if a < thumb {
            Part::PageLess
        } else if a >= thumb + size {
            Part::PageMore
        } else {
            Part::Thumb
        })
    }
    /// The bar drawn: a dark track, raised arrow buttons and a raised thumb
    /// (sunk while pressed), in the menus' greys.
    fn draw(&self, pixels: &mut [u8], total: f64, visible: f64, first: f64, pressed: Option<Part>) {
        let (x, y, w, h) = self.rect;
        let mut c = Canvas(pixels);
        c.rect((x, y, w, h), [20, 23, 24, 255]);
        bevel(&mut c, self.rect, true);
        let (start, length) = self.axis();
        let button = |less: bool| -> Rect {
            let at = if less { start } else { start + length - BAR };
            if self.vertical {
                (x, at, BAR, BAR)
            } else {
                (at, y, BAR, BAR)
            }
        };
        for (less, part) in [(true, Part::Less), (false, Part::More)] {
            let rect = button(less);
            raised(&mut c, rect, pressed == Some(part));
            arrow(&mut c, rect, self.vertical, less, pressed == Some(part));
        }
        if total > visible {
            let (thumb, size) = self.thumb(total, visible, first);
            let rect = if self.vertical {
                (x, thumb, BAR, size)
            } else {
                (thumb, y, size, BAR)
            };
            raised(&mut c, rect, pressed == Some(Part::Thumb));
            // Three grip lines across the middle of the thumb.
            let (cx, cy) = (rect.0 + rect.2 / 2, rect.1 + rect.3 / 2);
            for d in [-3, 0, 3] {
                if size >= 20 {
                    if self.vertical {
                        c.rect((rect.0 + 4, cy + d, rect.2 - 8, 1), [40, 43, 44, 255]);
                    } else {
                        c.rect((cx + d, rect.1 + 4, 1, rect.3 - 8), [40, 43, 44, 255]);
                    }
                }
            }
        }
    }
}

const LIGHT: [u8; 4] = [115, 120, 119, 255];
const DARK: [u8; 4] = [24, 27, 27, 255];
const FACE: [u8; 4] = [81, 83, 83, 255];

fn bevel(c: &mut Canvas, (x, y, w, h): Rect, inset: bool) {
    let (top, bottom) = if inset { (DARK, LIGHT) } else { (LIGHT, DARK) };
    c.rect((x, y, w, 1), top);
    c.rect((x, y, 1, h), top);
    c.rect((x, y + h - 1, w, 1), bottom);
    c.rect((x + w - 1, y, 1, h), bottom);
}
/// A raised face (a button or a thumb), sunk while pressed.
fn raised(c: &mut Canvas, rect: Rect, pressed: bool) {
    let (x, y, w, h) = rect;
    c.rect((x, y, w, h), if pressed { [70, 72, 72, 255] } else { FACE });
    bevel(c, rect, pressed);
    if !pressed {
        c.rect((x + 1, y + 1, w - 2, 1), LIGHT);
        c.rect((x + 1, y + 1, 1, h - 2), LIGHT);
    }
}
/// A small solid triangle in a button, pointing to the bar's start or end.
fn arrow(c: &mut Canvas, (x, y, w, h): Rect, vertical: bool, less: bool, pressed: bool) {
    let shift = i32::from(pressed);
    let (cx, cy) = (x + w / 2 + shift, y + h / 2 + shift);
    for i in 0..4 {
        // Row i of a 4-high triangle is 2i+1 wide.
        let half = i;
        let line = if less { i } else { 3 - i } - 2;
        if vertical {
            c.rect((cx - half, cy + line, 2 * half + 1, 1), INK);
        } else {
            c.rect((cx + line, cy - half, 1, 2 * half + 1), INK);
        }
    }
}

/// A press that is held: the thumb being dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Grab {
    vertical: bool,
    /// How far into the thumb the pointer took hold.
    offset: i32,
}

/// The creator's map view: the plan's own opening zoom and centre, then the
/// player's zoom and pan from there.
#[derive(Clone, Debug)]
pub struct PlanView {
    /// The theater's width and height, feet.
    size: [f64; 2],
    /// The view widths, feet, widest first: the one where the theater fills
    /// the plot, then the in-game map's steps narrower than it.
    levels: Vec<f64>,
    level: usize,
    /// What the view is centred on, east and north feet; always inside the
    /// range that keeps the theater under the whole plot.
    center: [f64; 2],
    /// Where the view opened, for Home.
    home: (usize, [f64; 2]),
    pointer: Option<(f64, f64)>,
    grab: Option<Grab>,
    /// The scroll bar part held down, for its look.
    pressed: Option<(bool, Part)>,
    rocker: Rocker,
    /// Which rocker half is held down.
    rocker_held: bool,
    /// Where the rocker's top left is (the side panel differs between the
    /// planning map and the in-flight one).
    rocker_at: (i32, i32),
    /// The in-flight map's view: it follows the player until the player
    /// scrolls it away, and Home goes back to following.
    flight: Option<Following>,
}

/// The in-flight map's following state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Following {
    on: bool,
}

impl PlanView {
    /// A view of `world`'s theater for `plan`.
    pub fn new(plan: &Plan, world: &Terrain) -> Self {
        let theater = &world.theater;
        Self::over(
            plan,
            [
                theater.cols as f64 * f64::from(CELL_FEET),
                theater.rows as f64 * f64::from(CELL_FEET),
            ],
        )
    }

    /// A view of a theater `size` feet across (east, north) for `plan`.
    pub(super) fn over(plan: &Plan, size: [f64; 2]) -> Self {
        // The widest view is the one whose plot the theater just fills: as
        // wide as the theater, or as tall, whichever is the tighter.
        let fill = size[0].min(size[1] * f64::from(PLOT.2) / f64::from(PLOT.3));
        let mut levels = vec![fill];
        levels.extend(
            STEPS_NMI
                .iter()
                .map(|nm| nm * NMI)
                .filter(|width| *width < fill * 0.95),
        );
        let mut view = Self {
            size,
            levels,
            level: 0,
            center: [size[0] / 2., size[1] / 2.],
            home: (0, [0.; 2]),
            pointer: None,
            grab: None,
            pressed: None,
            rocker: Rocker::new(),
            rocker_held: false,
            rocker_at: ROCKER,
            flight: None,
        };
        view.open_on(plan);
        view.home = (view.level, view.center);
        view
    }

    /// The in-flight map's view of a theater `size` feet across (east,
    /// north): the same zoom steps, scroll bars and rocker as the planning
    /// map, with the rocker at `rocker_at`. It opens 100 nm across (or the
    /// nearest step) and follows the player until the player scrolls.
    pub fn flight(size: [f64; 2], rocker_at: (i32, i32)) -> Self {
        let mut view = Self::over(&Plan::default(), size);
        view.level = view
            .levels
            .iter()
            .position(|width| *width <= 100.1 * NMI)
            .unwrap_or(view.levels.len() - 1);
        view.rocker_at = rocker_at;
        view.flight = Some(Following { on: true });
        view.clamp();
        view.home = (view.level, view.center);
        view
    }

    /// Centres the view on `position` (east, north feet) while it follows.
    pub fn follow(&mut self, position: [f64; 2]) {
        if self.flight.is_some_and(|f| f.on) {
            self.center = position;
            self.clamp();
        }
    }
    /// The player moved the view by hand: it stays where it is.
    fn let_go(&mut self) {
        if let Some(f) = &mut self.flight {
            f.on = false;
        }
    }

    /// Zooms in past the whole theater, onto the objective area and the
    /// starts: all of them if one of the closer views holds them, else the
    /// first start and the objective area, else just one step in. The
    /// opening view is never narrower than [`OPENING_NARROWEST_NMI`].
    fn open_on(&mut self, plan: &Plan) {
        if self.levels.len() < 2 {
            return;
        }
        let around = |point: [f64; 2], reach: f64| {
            [
                [point[0] - reach, point[1] - reach],
                [point[0] + reach, point[1] + reach],
            ]
        };
        let objective: Vec<[f64; 2]> = plan
            .objective
            .iter()
            .flat_map(|(center, radius)| around(*center, *radius))
            .collect();
        let marks = |count: usize| -> Vec<[f64; 2]> {
            plan.markers
                .iter()
                .take(count)
                .flat_map(|m| around([m.position[0], m.position[2]], 6_000.))
                .collect()
        };
        for points in [
            [marks(usize::MAX), objective.clone()].concat(),
            [marks(1), objective].concat(),
        ] {
            if points.is_empty() {
                continue;
            }
            let low = |i: usize| points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min);
            let high = |i: usize| {
                points
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            };
            let span =
                (high(0) - low(0)).max((high(1) - low(1)) * f64::from(PLOT.2) / f64::from(PLOT.3));
            // A fifth of the plot is left for the symbols and their words.
            let found = (1..self.levels.len())
                .rev()
                .filter(|i| self.levels[*i] >= OPENING_NARROWEST_NMI * NMI * 0.99)
                .find(|i| self.levels[*i] * 0.8 >= span);
            if let Some(level) = found {
                self.level = level;
                self.center = [(low(0) + high(0)) / 2., (low(1) + high(1)) / 2.];
                self.clamp();
                return;
            }
        }
        // Nothing holds even the first start: one step in, on what there is.
        self.level = 1;
        if let Some(marker) = plan.markers.first() {
            self.center = [marker.position[0], marker.position[2]];
        }
        self.clamp();
    }

    /// How wide and how tall the view is now, feet.
    fn extent(&self) -> [f64; 2] {
        let width = self.levels[self.level];
        [width, width * f64::from(PLOT.3) / f64::from(PLOT.2)]
    }
    fn width_nmi(&self) -> f64 {
        self.levels[self.level] / NMI
    }
    /// Keeps the view inside the theater, so no black shows at any zoom:
    /// the centre stays at least half the view from every edge (and in the
    /// middle on an axis where the view is as large as the theater).
    fn clamp(&mut self) {
        let extent = self.extent();
        for (axis, half) in extent.iter().map(|e| e / 2.).enumerate() {
            let (low, high) = (half, self.size[axis] - half);
            self.center[axis] = if low >= high {
                self.size[axis] / 2.
            } else {
                self.center[axis].clamp(low, high)
            };
        }
    }
    fn zoom(&mut self, closer: bool) {
        self.level = if closer {
            (self.level + 1).min(self.levels.len() - 1)
        } else {
            self.level.saturating_sub(1)
        };
        self.clamp();
    }
    /// How much of the theater is out of view each way as the three numbers a
    /// scroll bar needs: the horizontal bar's (total, visible, first) and the
    /// vertical bar's. The vertical bar counts down from the north edge.
    fn scrolls(&self) -> [(f64, f64, f64); 2] {
        let extent = self.extent();
        [
            (
                self.size[0],
                extent[0],
                (self.center[0] - extent[0] / 2.).max(0.),
            ),
            (
                self.size[1],
                extent[1],
                (self.size[1] - self.center[1] - extent[1] / 2.).max(0.),
            ),
        ]
    }
    fn set_first(&mut self, vertical: bool, first: f64) {
        self.let_go();
        let extent = self.extent();
        if vertical {
            self.center[1] = self.size[1] - first - extent[1] / 2.;
        } else {
            self.center[0] = first + extent[0] / 2.;
        }
        self.clamp();
    }
    fn bars(&self) -> [Bar; 2] {
        [
            Bar {
                rect: HORIZONTAL,
                vertical: false,
            },
            Bar {
                rect: VERTICAL,
                vertical: true,
            },
        ]
    }

    /// The in-game map's keys: + and - zoom, the arrows pan, Home goes back
    /// to the opening view.
    pub fn key(&mut self, key: &str) {
        let extent = self.extent();
        match key {
            "+" | "=" => {
                self.zoom(true);
                self.rocker.push(false, false, Instant::now());
            }
            "-" | "_" => {
                self.zoom(false);
                self.rocker.push(true, false, Instant::now());
            }
            "ArrowLeft" => {
                self.let_go();
                self.center[0] -= extent[0] * KEY_STEP;
            }
            "ArrowRight" => {
                self.let_go();
                self.center[0] += extent[0] * KEY_STEP;
            }
            "ArrowUp" => {
                self.let_go();
                self.center[1] += extent[1] * KEY_STEP;
            }
            "ArrowDown" => {
                self.let_go();
                self.center[1] -= extent[1] * KEY_STEP;
            }
            "Home" => match &mut self.flight {
                // The in-flight map goes back to following the player.
                Some(f) => f.on = true,
                None => (self.level, self.center) = self.home,
            },
            _ => {}
        }
        self.clamp();
    }

    /// The pointer moved (or left): drags a held thumb.
    pub fn pointer(&mut self, point: Option<(f64, f64)>) {
        self.pointer = point;
        let (Some(grab), Some((x, y))) = (self.grab, point) else {
            return;
        };
        let scrolls = self.scrolls();
        let bar = self.bars()[usize::from(grab.vertical)];
        let (total, visible, _) = scrolls[usize::from(grab.vertical)];
        let along = bar.along((x as i32, y as i32));
        let first = bar.first_at(along - grab.offset, total, visible);
        self.set_first(grab.vertical, first);
    }

    /// The mouse went down at the pointer: true when the scroll bars or the
    /// rocker took it. A press on an arrow button or the track moves the view
    /// at once; one on the thumb holds it.
    pub fn down(&mut self) -> bool {
        let Some((x, y)) = self.pointer else {
            return false;
        };
        let point = (x as i32, y as i32);
        if let Some(closer) = self.rocker_half(point) {
            self.zoom(closer);
            self.rocker.push(!closer, true, Instant::now());
            self.rocker_held = true;
            return true;
        }
        let scrolls = self.scrolls();
        for (axis, bar) in self.bars().into_iter().enumerate() {
            let (total, visible, first) = scrolls[axis];
            let Some(part) = bar.part(point, total, visible, first) else {
                continue;
            };
            let step = |fraction: f64| visible * fraction;
            match part {
                Part::Less => self.set_first(bar.vertical, first - step(ARROW_STEP)),
                Part::More => self.set_first(bar.vertical, first + step(ARROW_STEP)),
                Part::PageLess => self.set_first(bar.vertical, first - step(PAGE_STEP)),
                Part::PageMore => self.set_first(bar.vertical, first + step(PAGE_STEP)),
                Part::Thumb => {
                    let (thumb, _) = bar.thumb(total, visible, first);
                    self.grab = Some(Grab {
                        vertical: bar.vertical,
                        offset: bar.along(point) - thumb,
                    });
                }
            }
            self.pressed = Some((bar.vertical, part));
            return true;
        }
        false
    }

    /// The mouse went up: a held thumb or rocker is let go. True when
    /// either was held.
    pub fn up(&mut self) -> bool {
        let held = self.grab.take().is_some()
            || self.pressed.take().is_some()
            || std::mem::take(&mut self.rocker_held);
        if held {
            self.rocker.release(Instant::now());
        }
        held
    }

    /// Which half of the rocker a point is on: true for the top half (zoom
    /// in), false for the bottom (zoom out).
    fn rocker_half(&self, (x, y): (i32, i32)) -> Option<bool> {
        let at = self.rocker_at;
        if x < at.0 || x >= at.0 + HALF.0 {
            return None;
        }
        if (at.1..at.1 + HALF.1).contains(&y) {
            Some(true)
        } else if (at.1 + HALF.1..at.1 + 2 * HALF.1).contains(&y) {
            Some(false)
        } else {
            None
        }
    }

    /// Steps the rocker's tilt; true while it is still moving.
    pub fn advance(&mut self, now: Instant) -> bool {
        self.rocker.advance(now)
    }

    pub(super) fn projection(&self) -> Projection {
        Projection {
            center: self.center,
            feet_per_pixel: self.levels[self.level] / f64::from(PLOT.2),
            area: PLOT,
            bounds: self.size,
        }
    }
    /// The scale bar's label: a quarter of the view's width, nm.
    pub(super) fn scale_nmi(&self) -> f64 {
        (self.width_nmi() / 4.).round()
    }

    /// Draws the plan over `world` in the in-game map's look, with
    /// `sprites` the creator's (the theater picture, MCICONS.PIC, the rocker
    /// and the button pieces). Returns the Close button's rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        pixels: &mut [u8],
        world: &Terrain,
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
        plan: &Plan,
        title: &str,
        close_pressed: bool,
    ) -> Rect {
        Canvas(pixels).rect((0, 0, 640, 480), [72, 72, 72, 255]);
        let projection = self.projection();
        let icons = sprites.get("MCICONS.PIC");
        // Without the theater picture the cells are drawn in a plain land
        // grey (the creator has no scenery palette).
        draw_ground(
            pixels,
            world,
            &projection,
            sprites.get(&world.theater.map),
            |_| [118, 114, 98],
        );
        let mut labels = Vec::new();
        let mut markers = Vec::new();
        // The objective area and the route first, under everything else
        // (shared with the in-flight map), then the starts, then the airfields
        // around them. The leaders are drawn as they are placed, so every
        // symbol and label is drawn over them.
        draw_objective(
            pixels,
            font,
            &projection,
            plan,
            Some(PlanSide::Blue),
            &mut markers,
            &mut labels,
        );
        let mut placed = place_starts(pixels, &projection, plan, &mut markers, &mut labels);
        let scene = &world.airport_scene;
        for airport in &scene.airports {
            if airport
                .runway_objects
                .iter()
                .any(|object| plan.start_fields.contains(object))
            {
                continue;
            }
            let runways: Vec<[f64; 3]> = scene
                .runways
                .iter()
                .filter(|r| r.airport == airport.id)
                .map(|r| r.surface.center)
                .collect();
            if runways.is_empty() {
                continue;
            }
            let center: [f64; 3] = std::array::from_fn(|i| {
                runways.iter().map(|p| p[i]).sum::<f64>() / runways.len() as f64
            });
            let Some((point, _)) = projection.locate(center) else {
                continue;
            };
            let at = place_marker_in(PLOT, pixels, point, &mut markers, &mut labels);
            let tile = match airport.allegiance {
                tore_sim::airport::Allegiance::Friendly => Tile::Friendly,
                tore_sim::airport::Allegiance::Hostile => Tile::Hostile,
                _ => Tile::Neutral,
            };
            placed.push(Placed {
                at,
                icon: 15,
                tile,
                words: &airport.name,
                off_map: false,
            });
        }
        draw_placed(pixels, icons, font, &placed, &mut labels);
        self.draw_bars(pixels);
        self.draw_legend(pixels, font, icons, plan, sprites);
        // The frame, as the in-game map redraws it.
        let mut c = Canvas(pixels);
        c.rect((0, 0, 640, 30), [72, 72, 72, 255]);
        c.rect((0, 438, 640, 42), [72, 72, 72, 255]);
        let mut p = Paint {
            pixels,
            clip: (0, 0, 640, 480),
            color: INK,
        };
        let scale = f64::from(PLOT.2);
        p.line((12., 15.), (12. + scale / 4., 15.));
        for i in 0..=4 {
            let x = 12. + f64::from(i) * scale / 16.;
            p.line((x, 12.), (x, 18.));
        }
        p.text(
            font,
            &format!("{} NAUTICAL MILES", (self.width_nmi() / 4.).round()),
            174,
            9,
        );
        let title = title.to_ascii_uppercase();
        p.text(font, &title, 628 - advance(font, &title), 9);
        p.text(font, &plan.summary, 12, 443);
        p.text(
            font,
            "ESC / ENTER: CLOSE   +/-: ZOOM   ARROWS: PAN   HOME: RESET",
            12,
            463,
        );
        let (x, y, w) = PLAN_CLOSE;
        if ["ACTION0L.PIC", "ACTION0M.PIC", "ACTION0R.PIC", "QUICKFONT"]
            .iter()
            .all(|key| sprites.contains_key(*key))
        {
            Canvas(pixels).action_button(sprites, "Close", (x, y, w), false, close_pressed)
        } else {
            Canvas(pixels).rect((x, y, w, 24), [66, 70, 73, 255]);
            Paint {
                pixels,
                clip: (x, y, w, 24),
                color: INK,
            }
            .text(font, "CLOSE", x + (w - advance(font, "CLOSE")) / 2, y + 7);
            (x, y, w, 24)
        }
    }

    /// The scroll bars, over whatever a symbol or label spilled onto them.
    pub(super) fn draw_bars(&self, pixels: &mut [u8]) {
        Canvas(pixels).rect((VERTICAL.0, HORIZONTAL.1, BAR, BAR), [20, 23, 24, 255]);
        let scrolls = self.scrolls();
        for (axis, bar) in self.bars().into_iter().enumerate() {
            let (total, visible, first) = scrolls[axis];
            let held = self
                .pressed
                .filter(|(vertical, _)| *vertical == bar.vertical)
                .map(|(_, part)| part)
                .filter(|part| matches!(part, Part::Thumb | Part::Less | Part::More));
            bar.draw(pixels, total, visible, first, held);
        }
    }

    /// The zoom rocker: the top half zooms in, the bottom half out, with the
    /// words beside it as the debrief's PREV and NEXT stand by its.
    pub(super) fn draw_rocker(
        &self,
        pixels: &mut [u8],
        font: &Font,
        sprites: &std::collections::BTreeMap<String, Sprite>,
        clip: Rect,
    ) {
        let mut p = Paint {
            pixels: &mut *pixels,
            clip,
            color: INK,
        };
        let at = font.height as i32;
        let (x, y) = self.rocker_at;
        let zoom_in = "ZOOM IN";
        let zoom_out = "ZOOM OUT";
        p.text(
            font,
            zoom_in,
            x - 8 - advance(font, zoom_in),
            y + (HALF.1 - at) / 2,
        );
        p.text(
            font,
            zoom_out,
            x - 8 - advance(font, zoom_out),
            y + HALF.1 + (HALF.1 - at) / 2,
        );
        match sprites.get(&self.rocker.sprite()) {
            Some(rocker) => Canvas(pixels).blit(rocker, (x, y), 0, rocker.width, 1.),
            None => {
                // An import without the rocker's art gets two plain halves.
                let mut c = Canvas(pixels);
                for half in 0..2 {
                    raised(&mut c, (x, y + half * HALF.1, HALF.0, HALF.1), false);
                }
            }
        }
    }

    /// The side panel: what each mark means, the zoom rocker and the plan's
    /// note.
    fn draw_legend(
        &self,
        pixels: &mut [u8],
        font: &Font,
        icons: Option<&Sprite>,
        plan: &Plan,
        sprites: &std::collections::BTreeMap<String, Sprite>,
    ) {
        const PANEL: Rect = (500, 30, 128, 408);
        Canvas(pixels).rect(PANEL, [42, 45, 47, 255]);
        let mut p = Paint {
            pixels,
            clip: PANEL,
            color: INK,
        };
        p.text(font, "MISSION MAP", 507, 45);
        let rows: [(&str, Option<usize>, Tile); 4] = [
            ("BLUE START", Some(2), Tile::Friendly),
            ("RED START", Some(6), Tile::Hostile),
            ("AIRFIELD", Some(15), Tile::Neutral),
            ("OBJECTIVE AREA", None, Tile::Neutral),
        ];
        for (i, (words, icon, tile)) in rows.into_iter().enumerate() {
            let y = 74 + i as i32 * 26;
            match icon {
                Some(icon) => symbol(pixels, icons, 517, y, Some(icon), tile),
                None => objective_mark(pixels, (517., f64::from(y)), 8., PANEL),
            }
            Paint {
                pixels,
                clip: PANEL,
                color: if icon.is_none() { OBJECTIVE } else { INK },
            }
            .text(font, words, 533, y - 4);
        }
        let mut p = Paint {
            pixels,
            clip: PANEL,
            color: INK,
        };
        p.text(font, "AIRFIELD TILES:", 507, 182);
        p.text(font, "BLUE, RED OR GREY", 507, 182 + font.height as i32 + 3);
        self.draw_rocker(pixels, font, sprites, PANEL);
        if let Some(note) = &plan.note {
            let mut p = Paint {
                pixels,
                clip: PANEL,
                color: INK,
            };
            let mut y = 262;
            let mut line = String::new();
            for word in note.split_whitespace() {
                let next = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if advance(font, &next) > 114 && !line.is_empty() {
                    p.text(font, &line, 507, y);
                    y += font.height as i32 + 3;
                    line = word.to_owned();
                } else {
                    line = next;
                }
            }
            p.text(font, &line, 507, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight_map::PlanMarker;

    /// A theater 283 by 312 nm (the Ukraine's is about that), in feet.
    const SIZE: [f64; 2] = [283. * 6076.115, 312. * 6076.115];

    fn marker(east: f64, north: f64, side: PlanSide) -> PlanMarker {
        PlanMarker {
            position: [east, 0., north],
            heading: 0.,
            side,
            label: "START".into(),
        }
    }
    fn plan() -> Plan {
        Plan {
            markers: vec![
                marker(100. * NMI, 150. * NMI, PlanSide::Blue),
                marker(106. * NMI, 152. * NMI, PlanSide::Red),
            ],
            objective: Some(([108. * NMI, 156. * NMI], 5. * NMI)),
            ..Plan::default()
        }
    }
    /// True when the view lies wholly inside the theater.
    fn inside(view: &PlanView) -> bool {
        let extent = view.extent();
        let eps = 1e-6;
        (0..2).all(|axis| {
            let low = view.center[axis] - extent[axis] / 2.;
            let high = view.center[axis] + extent[axis] / 2.;
            // An axis the view is as large as the theater has nothing to
            // scroll: it is centred.
            low >= -eps && high <= view.size[axis] + eps
        })
    }

    #[test]
    fn the_widest_zoom_fills_the_plot_with_the_theater() {
        let view = PlanView::over(&plan(), SIZE);
        // 458 by 390 pixels: the tighter axis fits the theater exactly.
        let widest = view.levels[0];
        let tall = widest * 390. / 458.;
        assert!(widest <= SIZE[0] + 1e-6 && tall <= SIZE[1] + 1e-6);
        assert!((widest - SIZE[0]).abs() < 1e-6 || (tall - SIZE[1]).abs() < 1e-6);
        // Every other step is narrower, down to the in-game map's 25 nm.
        assert!(view.levels.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(view.levels.last().copied(), Some(25. * NMI));
    }

    #[test]
    fn the_view_opens_zoomed_in_on_the_starts_and_the_objective() {
        let view = PlanView::over(&plan(), SIZE);
        assert!(view.level >= 1, "level {}", view.level);
        assert!(view.width_nmi() >= 50. && view.width_nmi() < 283.);
        assert!(inside(&view));
        // Both starts and the whole objective circle are in view.
        let extent = view.extent();
        for (east, north) in [(100., 150.), (106., 152.), (103., 151.), (113., 161.)] {
            let (east, north) = (east * NMI, north * NMI);
            assert!((east - view.center[0]).abs() <= extent[0] / 2., "{east}");
            assert!((north - view.center[1]).abs() <= extent[1] / 2., "{north}");
        }
    }

    #[test]
    fn a_start_in_a_corner_is_clamped_so_no_black_shows() {
        let mut corner = plan();
        corner.markers = vec![marker(1_000., 1_000., PlanSide::Blue)];
        corner.objective = None;
        let view = PlanView::over(&corner, SIZE);
        assert!(view.level >= 1);
        assert!(inside(&view));
        let projection = view.projection();
        // The theater's south-west corner is at or beyond the plot's corner.
        let corner = projection.unclipped([0., 0., 0.]).unwrap();
        assert!(
            corner.0 >= f64::from(PLOT.0) - 1e-6 || view.center[0] - view.extent()[0] / 2. <= 1e-6
        );
    }

    #[test]
    fn no_zoom_and_no_pan_ever_shows_black() {
        let mut view = PlanView::over(&plan(), SIZE);
        let keys = [
            "+",
            "+",
            "ArrowLeft",
            "ArrowLeft",
            "ArrowLeft",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "ArrowDown",
            "-",
            "ArrowRight",
            "ArrowRight",
            "ArrowUp",
            "-",
            "-",
            "-",
            "-",
            "+",
            "+",
            "+",
            "+",
            "+",
            "+",
            "ArrowUp",
            "ArrowUp",
            "ArrowUp",
            "ArrowUp",
            "ArrowUp",
            "ArrowRight",
            "ArrowRight",
            "ArrowRight",
            "ArrowRight",
            "ArrowRight",
            "Home",
            "-",
        ];
        for key in keys {
            view.key(key);
            assert!(
                inside(&view),
                "{key}: {:?} level {}",
                view.center,
                view.level
            );
        }
        // Zoomed all the way out the view is the theater, and nothing scrolls.
        for _ in 0..10 {
            view.key("-");
        }
        assert!(view.level == 0 && inside(&view));
        let [(h_total, h_visible, _), (v_total, v_visible, _)] = view.scrolls();
        assert!(h_visible <= h_total + 1e-6 && v_visible <= v_total + 1e-6);
        // Zoom is bounded at 25 nm.
        for _ in 0..20 {
            view.key("+");
        }
        assert_eq!(view.level + 1, view.levels.len());
        assert_eq!(view.width_nmi(), 25.);
    }

    #[test]
    fn home_returns_to_the_opening_view() {
        let mut view = PlanView::over(&plan(), SIZE);
        let opened = (view.level, view.center);
        for key in ["-", "ArrowLeft", "ArrowUp", "+", "+"] {
            view.key(key);
        }
        view.key("Home");
        assert_eq!((view.level, view.center), opened);
    }

    #[test]
    fn the_scroll_bars_arrows_track_and_thumb_move_the_view() {
        let mut view = PlanView::over(&plan(), SIZE);
        // A narrow view so there is room to scroll.
        for _ in 0..3 {
            view.key("+");
        }
        let [(total, visible, first), _] = view.scrolls();
        assert!(visible < total);
        let click = |view: &mut PlanView, x: i32, y: i32| {
            view.pointer(Some((f64::from(x), f64::from(y))));
            let taken = view.down();
            view.pointer(Some((f64::from(x), f64::from(y))));
            view.up();
            taken
        };
        // The horizontal bar's right arrow moves the view east by an eighth.
        let y = HORIZONTAL.1 + BAR / 2;
        assert!(click(&mut view, HORIZONTAL.0 + HORIZONTAL.2 - 4, y));
        let [(_, _, after), _] = view.scrolls();
        assert!((after - (first + visible * ARROW_STEP)).abs() < 1.);
        // The left arrow moves it back.
        assert!(click(&mut view, HORIZONTAL.0 + 4, y));
        let [(_, _, back), _] = view.scrolls();
        assert!((back - first).abs() < 1.);
        // A click on the track after the thumb pages on by most of the view.
        let bar = view.bars()[0];
        let (thumb, size) = bar.thumb(total, visible, back);
        assert!(click(&mut view, thumb + size + 3, y));
        let [(_, _, paged), _] = view.scrolls();
        assert!((paged - (back + visible * PAGE_STEP)).abs() < 1. || paged >= total - visible - 1.);
        // Dragging the thumb to the bar's far end puts the view on the east edge.
        let [(_, _, now), _] = view.scrolls();
        let (thumb, size) = bar.thumb(total, visible, now);
        view.pointer(Some((f64::from(thumb + size / 2), f64::from(y))));
        assert!(view.down());
        view.pointer(Some((
            f64::from(HORIZONTAL.0 + HORIZONTAL.2 + 50),
            f64::from(y),
        )));
        view.up();
        assert!(
            (view.center[0] + view.extent()[0] / 2. - SIZE[0]).abs() < 1.,
            "{:?}",
            view.center
        );
        assert!(inside(&view));
        // The vertical bar scrolls north and south the same way: its top
        // arrow moves the view north.
        let before = view.center[1];
        let x = VERTICAL.0 + BAR / 2;
        assert!(click(&mut view, x, VERTICAL.1 + 4));
        assert!(
            view.center[1] > before
                || (view.center[1] + view.extent()[1] / 2. - SIZE[1]).abs() < 1.
        );
        assert!(inside(&view));
    }

    #[test]
    fn the_thumb_follows_the_view_and_is_never_smaller_than_it_can_be_grabbed() {
        let bar = Bar {
            rect: HORIZONTAL,
            vertical: false,
        };
        let (start, size) = bar.thumb(1000., 100., 0.);
        assert_eq!(start, HORIZONTAL.0 + BAR);
        let (end, _) = bar.thumb(1000., 100., 900.);
        assert_eq!(end + size, HORIZONTAL.0 + HORIZONTAL.2 - BAR);
        let (_, tiny) = bar.thumb(1_000_000., 1., 0.);
        assert_eq!(tiny, MIN_THUMB);
        // The thumb's place reads back as the same offset.
        for first in [0., 123., 450., 900.] {
            let (at, _) = bar.thumb(1000., 100., first);
            assert!(
                (bar.first_at(at, 1000., 100.) - first).abs() < 9.,
                "{first}"
            );
        }
    }

    #[test]
    fn the_rocker_zooms_in_with_its_top_half_and_out_with_its_bottom() {
        let mut view = PlanView::over(&plan(), SIZE);
        let level = view.level;
        let press = |view: &mut PlanView, y: i32| {
            view.pointer(Some((f64::from(ROCKER.0 + 4), f64::from(y))));
            let taken = view.down();
            view.up();
            taken
        };
        assert!(press(&mut view, ROCKER.1 + 4));
        assert_eq!(view.level, (level + 1).min(view.levels.len() - 1));
        assert!(press(&mut view, ROCKER.1 + HALF.1 + 4));
        assert_eq!(view.level, level);
        // It tilts toward the half pressed, and the keys tap it too.
        view.key("+");
        assert!(view.advance(Instant::now() + Rocker::FRAME) || view.level >= level);
        // A press beside it is nobody's.
        view.pointer(Some((f64::from(ROCKER.0 + 40), f64::from(ROCKER.1 + 4))));
        assert!(!view.down());
    }

    #[test]
    fn a_theater_no_wider_than_the_narrowest_step_has_nothing_to_zoom() {
        // Smaller than the 25 nm step: one level, the whole theater.
        let view = PlanView::over(&plan(), [20. * NMI, 20. * NMI]);
        assert_eq!(view.levels.len(), 1);
        assert!(inside(&view));
    }
}
