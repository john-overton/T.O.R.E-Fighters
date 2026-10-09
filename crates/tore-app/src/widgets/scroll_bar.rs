//! The scroll bar: retail's vertical slider, the Sound Prefs knob in its grey
//! slotted track, used as a list scroll bar (lobby pass, slice L1).
//!
//! Retail Fighters Anthology has no list scroll bar (its lists page with the
//! PREV/NEXT rocker), so this is an *opinionated* reuse of the slider art
//! (John, 2026-10-09: "the red one"). The pieces and the way they stack are
//! the Sound Prefs dialog's (`docs/spec/sound-prefs.md`, "Each slider is a
//! track and a knob"); the numbers it adds are in `docs/formats/menu.md`
//! ("The scroll bar").
//!
//! # How a screen uses it
//!
//! The bar holds no content. It is a rectangle, a grip while the knob is
//! dragged, and three questions the owner asks with a [`View`] (how many
//! lines there are, how many show, which is first):
//!
//! ```ignore
//! let bar = ScrollBar::new((336, 168), 89);       // art's top left, height
//! let view = View { total: rows, visible: 5, first };
//! bar.draw(canvas, kit, view);                    // track, and knob if it scrolls
//! match bar.press(point, view) {                  // mouse down
//!     Press::Grab => {}                           // the knob is held now
//!     Press::Up => first = view.shifted(-5),      // track above the knob
//!     Press::Down => first = view.shifted(5),     // track below it
//!     Press::Miss => {}
//! }
//! if let Some(f) = bar.drag(point, view) { first = f; }   // pointer moved
//! bar.release();                                  // mouse up, focus lost
//! ```
//!
//! [`List`](super::List) (a scroll mode: `with_scroll_bar`) and
//! [`MessageBox`](super::MessageBox) do all of that themselves; a screen
//! passes them the pointer events and nothing more.
use super::{Kit, Point, Rect, draw::blit_part, inside};
use crate::menu::Canvas;

/// The track art is 34 wide: `SLIDETOP`, `SLIDEMID` and `SLIDEBOT` all are.
pub const WIDTH: i32 = 34;
/// The knob, `SLIDERV`: 26 by 30. Retail's knob never stretches.
const KNOB: (i32, i32) = (26, 30);
/// The knob sits 6 pixels in from the track's left edge (the Sound Prefs
/// column table).
const KNOB_X: i32 = 6;
/// The knob's top stops 6 pixels below the track's top, and its bottom 5
/// above the track's bottom, so a 90 pixel track (retail's) gives the
/// slider's own 49 pixels of travel.
const INSET_TOP: i32 = 6;
const INSET_BOTTOM: i32 = 5;
/// Piece heights: `SLIDETOP` 9, `SLIDEMID` 8, `SLIDEBOT` 15. The middle tiles
/// start 8 rows down (one over the cap's last row) and the bottom cap's top is
/// 15 above the track's bottom (`_DrawSliderVert`).
const TOP_HEIGHT: i32 = 9;
const MID_HEIGHT: i32 = 8;
const BOTTOM_HEIGHT: i32 = 15;
/// The shortest track that has any travel at all.
pub const MIN_HEIGHT: i32 = INSET_TOP + KNOB.1 + INSET_BOTTOM;

/// The pieces of the track; a player whose import predates them gets
/// [`flat`]. The knob (`SLIDERV`) is a menu piece every import has.
const TRACK_PIECES: [&str; 3] = ["SLIDETOP", "SLIDEMID", "SLIDEBOT"];

/// Retail's greys, measured on the pieces: the face (the panel's own grey,
/// so the track blends in), the bevel and the dark of the slot. The values
/// are 8-bit grey levels.
const FACE: [u8; 4] = [81, 81, 81, 255];
const SHADOW: [u8; 4] = [57, 57, 57, 255];
const EDGE: [u8; 4] = [24, 24, 24, 255];
const HIGHLIGHT: [u8; 4] = [113, 113, 113, 255];
const GROOVE: [u8; 4] = [12, 12, 12, 255];

/// What the bar needs to know about the content it scrolls, in the owner's
/// own units (rows, or text lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct View {
    /// How many there are in all.
    pub total: usize,
    /// How many show at once.
    pub visible: usize,
    /// The index of the first one shown.
    pub first: usize,
}

impl View {
    /// How far the first line can go: 0 when everything fits.
    pub fn max_first(&self) -> usize {
        self.total.saturating_sub(self.visible)
    }
    /// True when there is nothing to scroll to (the bar draws no knob).
    pub fn fits(&self) -> bool {
        self.max_first() == 0
    }
    /// `first` moved by `by` (negative goes toward the top), kept in range.
    pub fn shifted(&self, by: i64) -> usize {
        (self.first as i64 + by).clamp(0, self.max_first() as i64) as usize
    }
}

/// What a mouse press on the bar did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Not on a bar that scrolls (outside it, or everything fits).
    Miss,
    /// On the knob: it is held, and [`ScrollBar::drag`] moves it.
    Grab,
    /// On the track above the knob: the owner pages toward the top.
    Up,
    /// On the track below the knob: the owner pages toward the end.
    Down,
}

/// A vertical scroll bar at a place on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollBar {
    at: Point,
    height: i32,
    /// While the knob is held: how far below its top the pointer grabbed it.
    grip: Option<i32>,
}

impl ScrollBar {
    /// A bar whose art's top left is `at`, `height` high (at least
    /// [`MIN_HEIGHT`]; a shorter one is made that tall). The art is 34 wide.
    pub fn new(at: Point, height: i32) -> Self {
        Self {
            at,
            height: height.max(MIN_HEIGHT),
            grip: None,
        }
    }

    /// The art's rectangle: the bar's click area.
    pub fn bounds(&self) -> Rect {
        (self.at.0, self.at.1, WIDTH, self.height)
    }
    pub fn hit(&self, point: Point) -> bool {
        inside(self.bounds(), point)
    }
    /// The knob is held.
    pub fn dragging(&self) -> bool {
        self.grip.is_some()
    }

    /// The knob's top at its highest and how far it can go from there.
    fn range(&self) -> (i32, i32) {
        (
            self.at.1 + INSET_TOP,
            (self.height - INSET_TOP - INSET_BOTTOM - KNOB.1).max(0),
        )
    }

    /// The knob's top edge for `view`; none when everything fits (no knob is
    /// drawn, and the track does not react).
    pub fn knob_top(&self, view: View) -> Option<i32> {
        if view.fits() {
            return None;
        }
        let (top, travel) = self.range();
        let max = view.max_first() as i64;
        let first = view.first.min(view.max_first()) as i64;
        Some(top + ((i64::from(travel) * first + max / 2) / max) as i32)
    }
    /// The knob's rectangle for `view`, when it has one.
    pub fn knob(&self, view: View) -> Option<Rect> {
        self.knob_top(view)
            .map(|top| (self.at.0 + KNOB_X, top, KNOB.0, KNOB.1))
    }

    /// The first line for a knob whose top is at `top`, rounded to the
    /// nearest line and kept in range.
    fn first_at(&self, top: i32, view: View) -> usize {
        let (highest, travel) = self.range();
        if travel == 0 || view.fits() {
            return 0;
        }
        let offset = i64::from((top - highest).clamp(0, travel));
        let max = view.max_first() as i64;
        ((offset * max + i64::from(travel) / 2) / i64::from(travel)) as usize
    }

    /// The mouse went down at `point`. A press on the knob holds it; on the
    /// track above or below it, the owner is told which way to page.
    pub fn press(&mut self, point: Point, view: View) -> Press {
        self.grip = None;
        let Some(knob) = self.knob(view) else {
            return Press::Miss;
        };
        if !self.hit(point) {
            return Press::Miss;
        }
        if inside(knob, point) {
            self.grip = Some(point.1 - knob.1);
            Press::Grab
        } else if point.1 < knob.1 {
            Press::Up
        } else {
            Press::Down
        }
    }

    /// The pointer moved to `point` while the knob is held: the line the knob
    /// is now at, with the grip kept (the knob does not jump to the pointer).
    /// None when it is not held. The pointer may leave the bar sideways and
    /// the knob still follows its height.
    pub fn drag(&self, point: Point, view: View) -> Option<usize> {
        self.grip.map(|grip| self.first_at(point.1 - grip, view))
    }

    /// The mouse went up, or the window lost it: the knob is let go.
    pub fn release(&mut self) {
        self.grip = None;
    }

    /// Draws the track in the retail pieces, or flat when the import has not
    /// kept them, and the knob over it when `view` scrolls.
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, view: View) {
        let (x, y, w, h) = self.bounds();
        if TRACK_PIECES.iter().all(|name| kit.has(name)) {
            let (top, middle, bottom) = (
                kit.sprite("SLIDETOP"),
                kit.sprite("SLIDEMID"),
                kit.sprite("SLIDEBOT"),
            );
            let clip = self.bounds();
            blit_part(
                canvas,
                top,
                (x, y),
                (0, 0, top.width, top.height),
                clip,
                1.0,
            );
            // The middle tiles from 8 rows down to where the bottom cap
            // starts, the last tile cut short.
            let end = y + h - BOTTOM_HEIGHT;
            let mut row = y + TOP_HEIGHT - 1;
            while row < end {
                let rows = (end - row).min(MID_HEIGHT) as usize;
                blit_part(
                    canvas,
                    middle,
                    (x, row),
                    (0, 0, middle.width, rows),
                    clip,
                    1.0,
                );
                row += MID_HEIGHT;
            }
            blit_part(
                canvas,
                bottom,
                (x, end),
                (0, 0, bottom.width, bottom.height),
                clip,
                1.0,
            );
        } else {
            flat(canvas, (x, y, w, h));
        }
        if let Some((kx, ky, _, _)) = self.knob(view) {
            let knob = kit.sprite("SLIDERV");
            canvas.blit(knob, (kx, ky), 0, knob.width, 1.0);
        }
    }
}

/// The track drawn flat, for an import made before the pieces were kept
/// (*agent decision*: no forced re-import for a scroll bar; the log suggests
/// one). It copies the pieces' layout, measured on a middle row: a recessed
/// slot with a dark left edge, a groove down the middle with its highlight,
/// and a shadow and highlight on the right, all on the panel's own grey.
fn flat(canvas: &mut Canvas, (x, y, w, h): Rect) {
    canvas.rect((x, y, w, h), FACE);
    // The slot's box: x 2 to 31 of the 34, from 2 rows down to 7 above the
    // bottom.
    let (top, bottom) = (y + 2, y + h - 7);
    let rows = bottom - top;
    canvas.rect((x + 2, top, 1, rows), EDGE);
    canvas.rect((x + 3, top, 2, rows), GROOVE);
    // The top lip: the dark runs across for two rows.
    canvas.rect((x + 3, top + 1, 28, 2), GROOVE);
    // The groove down the middle, 2 wide, with its edges.
    canvas.rect((x + 15, top + 3, 1, rows - 3), SHADOW);
    canvas.rect((x + 16, top + 3, 2, rows - 3), GROOVE);
    canvas.rect((x + 18, top + 3, 1, rows - 3), HIGHLIGHT);
    // The right edge, and the bottom's shadow and highlight.
    canvas.rect((x + 31, top + 2, 1, rows - 2), SHADOW);
    canvas.rect((x + 32, top + 2, 1, rows - 1), HIGHLIGHT);
    canvas.rect((x + 3, bottom, 29, 1), SHADOW);
    canvas.rect((x + 3, bottom + 1, 30, 1), HIGHLIGHT);
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, kit, tone_of};
    use super::*;

    /// A 90 pixel bar (retail's) at (100, 50).
    fn bar() -> ScrollBar {
        ScrollBar::new((100, 50), 90)
    }
    fn view(total: usize, first: usize) -> View {
        View {
            total,
            visible: 5,
            first,
        }
    }

    #[test]
    fn the_knob_runs_the_sliders_own_49_pixels() {
        let bar = bar();
        // 25 lines, 5 show: first 0 to 20.
        assert_eq!(bar.knob_top(view(25, 0)), Some(56));
        assert_eq!(bar.knob_top(view(25, 20)), Some(56 + 49));
        // Halfway: 10 of 20 is 24.5 pixels, rounded to the nearest.
        assert_eq!(bar.knob_top(view(25, 10)), Some(56 + 25));
        assert_eq!(bar.knob(view(25, 0)), Some((106, 56, 26, 30)));
        // The knob's bottom stays inside the track.
        let (_, top, _, h) = bar.knob(view(25, 20)).unwrap();
        assert!(top + h <= 50 + 90);
    }

    #[test]
    fn no_knob_when_everything_fits() {
        let mut bar = bar();
        for total in [0, 1, 5] {
            assert_eq!(bar.knob_top(view(total, 0)), None);
            assert_eq!(bar.press((110, 100), view(total, 0)), Press::Miss);
            assert_eq!(bar.drag((110, 100), view(total, 0)), None);
        }
        assert!(!bar.dragging());
        // One line more and it scrolls.
        assert!(bar.knob_top(view(6, 0)).is_some());
    }

    #[test]
    fn a_first_line_past_the_end_is_held_at_the_end() {
        let bar = bar();
        assert_eq!(bar.knob_top(view(25, 99)), bar.knob_top(view(25, 20)));
        assert_eq!(view(25, 3).shifted(100), 20);
        assert_eq!(view(25, 3).shifted(-100), 0);
        assert_eq!(view(25, 3).shifted(4), 7);
    }

    #[test]
    fn a_press_on_the_track_pages_and_on_the_knob_grabs() {
        let mut bar = bar();
        let v = view(25, 10);
        let knob = bar.knob(v).unwrap();
        // Above the knob, below it, beside it on the track.
        assert_eq!(bar.press((110, knob.1 - 2), v), Press::Up);
        assert_eq!(bar.press((110, knob.1 + 31), v), Press::Down);
        assert_eq!(bar.press((101, knob.1 + 5), v), Press::Down);
        assert!(!bar.dragging());
        assert_eq!(bar.press((120, knob.1 + 7), v), Press::Grab);
        assert!(bar.dragging());
        // Off the bar altogether.
        assert_eq!(bar.press((99, 100), v), Press::Miss);
        assert_eq!(bar.press((134, 100), v), Press::Miss);
        assert_eq!(bar.press((110, 49), v), Press::Miss);
        assert!(!bar.dragging());
    }

    #[test]
    fn dragging_keeps_the_grip_and_stops_at_both_ends() {
        let mut bar = bar();
        let v = view(25, 10);
        let knob = bar.knob(v).unwrap();
        // Grab 7 pixels below the knob's top; a drag by nothing changes
        // nothing.
        assert_eq!(bar.press((115, knob.1 + 7), v), Press::Grab);
        assert_eq!(bar.drag((115, knob.1 + 7), v), Some(10));
        // 12 pixels down: 12 / 49 of 20 lines is 4.9, so 5.
        assert_eq!(bar.drag((115, knob.1 + 7 + 12), v), Some(15));
        // Sideways off the bar does not let go.
        assert_eq!(bar.drag((400, knob.1 + 7 + 12), v), Some(15));
        // Past either end.
        assert_eq!(bar.drag((115, 0), v), Some(0));
        assert_eq!(bar.drag((115, 479), v), Some(20));
        bar.release();
        assert!(!bar.dragging());
        assert_eq!(bar.drag((115, 100), v), None);
    }

    #[test]
    fn dragging_to_a_knob_position_and_back_agrees() {
        // Every line's knob position maps back to the same line.
        let mut bar = bar();
        for total in [6usize, 7, 12, 30, 200] {
            let last = view(total, 0).max_first();
            for first in 0..=last {
                let v = view(total, first);
                let knob = bar.knob(v).unwrap();
                bar.press((knob.0 + 3, knob.1 + 4), v);
                let moved = bar.drag((knob.0 + 3, knob.1 + 4), v);
                // With 200 lines a pixel is several lines: within one pixel.
                if last <= 49 {
                    assert_eq!(moved, Some(first), "total {total} first {first}");
                } else {
                    let step = last.div_ceil(49);
                    assert!(moved.unwrap().abs_diff(first) <= step);
                }
                bar.release();
            }
        }
    }

    #[test]
    fn a_short_bar_is_made_tall_enough_for_the_knob() {
        let bar = ScrollBar::new((0, 0), 10);
        assert_eq!(bar.bounds().3, MIN_HEIGHT);
        // No travel: the knob sits at the top and every drag is line 0.
        let mut bar = bar;
        let v = view(25, 0);
        assert_eq!(bar.knob_top(v), Some(6));
        let knob = bar.knob(v).unwrap();
        bar.press((knob.0 + 1, knob.1 + 1), v);
        assert_eq!(bar.drag((knob.0, 300), v), Some(0));
    }

    #[test]
    fn the_track_is_drawn_from_the_pieces_and_the_knob_over_it() {
        let kit = kit();
        let bar = bar();
        let mut pixels = blank();
        let mut canvas = Canvas(&mut pixels);
        bar.draw(&mut canvas, &kit, view(25, 0));
        let (top, middle, bottom) = (
            tone_of(&kit, "SLIDETOP"),
            tone_of(&kit, "SLIDEMID"),
            tone_of(&kit, "SLIDEBOT"),
        );
        assert_eq!(at(&pixels, 101, 50), top);
        assert_eq!(at(&pixels, 101, 58), middle);
        // The middle runs to the bottom cap, which starts 15 above the end.
        assert_eq!(at(&pixels, 101, 50 + 90 - 16), middle);
        assert_eq!(at(&pixels, 101, 50 + 90 - 15), bottom);
        assert_eq!(at(&pixels, 101, 50 + 89), bottom);
        // The knob, 6 in and 6 down, over the track; nothing outside it.
        assert_eq!(at(&pixels, 106, 56), tone_of(&kit, "SLIDERV"));
        assert_eq!(at(&pixels, 131, 85), tone_of(&kit, "SLIDERV"));
        assert_eq!(at(&pixels, 100 + 33, 86 + 10), middle);
        assert_eq!(at(&pixels, 99, 60), [0, 0, 0]);
        assert_eq!(at(&pixels, 134, 60), [0, 0, 0]);
        // Scrolled to the end the knob is at the bottom of the travel.
        let mut pixels = blank();
        bar.draw(&mut Canvas(&mut pixels), &kit, view(25, 20));
        assert_eq!(at(&pixels, 106, 56 + 49 + 29), tone_of(&kit, "SLIDERV"));
        assert_ne!(at(&pixels, 106, 56), tone_of(&kit, "SLIDERV"));
    }

    #[test]
    fn an_empty_track_has_no_knob() {
        let kit = kit();
        let mut pixels = blank();
        bar().draw(&mut Canvas(&mut pixels), &kit, view(3, 0));
        for y in 50..140 {
            assert_ne!(at(&pixels, 108, y), tone_of(&kit, "SLIDERV"), "y {y}");
        }
        assert_eq!(at(&pixels, 101, 60), tone_of(&kit, "SLIDEMID"));
    }

    #[test]
    fn the_cut_last_tile_does_not_run_past_the_cap() {
        // A 95 pixel bar: the middle is 95 - 23 = 72 rows, nine tiles of 8;
        // a 93 pixel one has 70, the last tile cut to 6.
        let kit = kit();
        let bar = ScrollBar::new((100, 50), 93);
        let mut pixels = blank();
        bar.draw(&mut Canvas(&mut pixels), &kit, view(3, 0));
        assert_eq!(at(&pixels, 101, 50 + 93 - 16), tone_of(&kit, "SLIDEMID"));
        assert_eq!(at(&pixels, 101, 50 + 93 - 15), tone_of(&kit, "SLIDEBOT"));
        assert_eq!(at(&pixels, 101, 50 + 93), [0, 0, 0]);
    }

    #[test]
    fn without_the_pieces_the_track_is_flat_and_the_knob_stays() {
        let mut kit = kit();
        for name in TRACK_PIECES {
            kit.sprites.remove(&format!("{name}.PIC"));
        }
        let bar = bar();
        let mut pixels = blank();
        bar.draw(&mut Canvas(&mut pixels), &kit, view(25, 0));
        // The slot, on the panel's own grey: the face between the left edge
        // and the groove, the groove, its highlight, the right edge.
        assert_eq!(at(&pixels, 100 + 9, 50 + 60), [81; 3]);
        assert_eq!(at(&pixels, 100 + 3, 50 + 60), [12; 3]);
        assert_eq!(at(&pixels, 100 + 16, 50 + 80), [12; 3]);
        assert_eq!(at(&pixels, 100 + 18, 50 + 80), [113; 3]);
        assert_eq!(at(&pixels, 100 + 31, 50 + 60), [57; 3]);
        assert_eq!(at(&pixels, 100 + 25, 50 + 60), [81; 3]);
        // The knob is the retail piece.
        assert_eq!(at(&pixels, 110, 60), tone_of(&kit, "SLIDERV"));
        // Nothing past the rectangle.
        assert_eq!(at(&pixels, 134, 60), [0, 0, 0]);
        assert_eq!(at(&pixels, 117, 50 + 90), [0, 0, 0]);
        // One missing piece is enough to go flat.
        let mut kit = super::super::test_kit::kit();
        kit.sprites.remove("SLIDEMID.PIC");
        let mut pixels = blank();
        bar.draw(&mut Canvas(&mut pixels), &kit, view(3, 0));
        assert_eq!(at(&pixels, 103, 110), [12; 3]);
    }

    /// The bar in every state, a scrolling list and a Messages box, drawn from
    /// the imported pieces and again flat (a pack without the track), for the
    /// eye. `TORE_MOCK_OUT` names the folder; the pictures are PPM.
    #[test]
    #[ignore = "needs an imported data profile (TORE_DATA_DIR) and TORE_MOCK_OUT"]
    fn render_scroll_bar_sheets() {
        use super::super::{Background, List, MessageBox, Row, draw_panel};
        let out =
            std::path::PathBuf::from(std::env::var_os("TORE_MOCK_OUT").expect("TORE_MOCK_OUT"));
        let dir = crate::assets::data_directory().expect("data directory");
        let assets = crate::assets::Assets::load(&dir).expect("imported pack");
        for (name, flat_track) in [("with-pieces", false), ("flat", true)] {
            let mut kit =
                Kit::new(&assets.pics, &assets.multiplayer_resources, "NETIPX3").expect("kit");
            if flat_track {
                for piece in TRACK_PIECES {
                    kit.sprites.remove(&format!("{piece}.PIC"));
                }
            }
            let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
            let mut c = Canvas(&mut pixels);
            Background::single("NETIPX3").draw(&mut c, &kit);
            draw_panel(&mut c, &kit, (10, 10, 619, 460));
            // Bare bars: top, a third, the end, and one that fits.
            for (i, (total, first)) in [(30, 0), (30, 8), (30, 25), (4, 0)].into_iter().enumerate()
            {
                ScrollBar::new((40 + i as i32 * 60, 30), 90).draw(
                    &mut c,
                    &kit,
                    View {
                        total,
                        visible: 5,
                        first,
                    },
                );
            }
            // A scrolling list, its window in the middle, one row chosen.
            let mut list = List::new((300, 30), 286, 5).with_scroll_bar((592, 30));
            list.set_rows(
                (0..30)
                    .map(|i| Row::text(format!("Wing {} #{}", i / 5 + 1, i % 5 + 1)))
                    .collect(),
            );
            list.select(12);
            list.draw(&mut c, &kit, true);
            // A Messages box with a long log, scrolled back a little.
            let mut messages = MessageBox::new((45, 150, 549, 78));
            for i in 0..40 {
                messages.push(
                    &kit,
                    &format!("Line {i}: the retail slider is the bar."),
                    tone_sys(),
                );
            }
            messages.scroll(12);
            messages.draw(&mut c, &kit, false);
            let mut short = MessageBox::new((45, 250, 549, 78));
            short.push(&kit, "Nothing to scroll: the track is empty.", tone_sys());
            short.draw(&mut c, &kit, false);
            let mut ppm =
                format!("P6\n{} {}\n255\n", crate::menu::WIDTH, crate::menu::HEIGHT).into_bytes();
            for px in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&px[..3]);
            }
            std::fs::write(out.join(format!("scroll-bar-{name}.ppm")), ppm).expect("write");
        }
    }
    fn tone_sys() -> [u8; 3] {
        super::super::tone::SYSTEM
    }
}
