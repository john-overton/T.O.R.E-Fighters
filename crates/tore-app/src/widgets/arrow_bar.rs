//! The map's scroll bar, shared (nr-lobbyui, John 2026-10-16).
//!
//! The planning map draws a flat bar in the menus' greys: a dark track, an
//! arrow button at each end, and a raised thumb. Retail has no list bar and
//! no horizontal bar, so this look is the map's own (*opinionated*, John,
//! 2026-10-11, reused here on his later request). The Sound Prefs slider
//! (`scroll_bar`) stays for that dialog; lists and message boxes use this.
//!
//! The bar holds no content. The owner says how much there is (`total`), how
//! much shows (`visible`) and where the shown part starts (`first`). A list
//! counts rows; the map counts feet. Arrow buttons, the track and the thumb
//! are hit-tested here. How far an arrow or a track click moves is the
//! owner's: the map steps by a fraction of the view, a list by one row or a
//! page.

use super::{Point, Rect, inside};
use crate::menu::Canvas;

/// Thickness of the bar, and the arrow buttons on a bar long enough for them.
pub const THICK: i32 = 16;
/// The shortest a thumb is drawn, so it can be grabbed.
const MIN_THUMB: i32 = 14;
/// Below this length the arrow buttons shrink so a short bar still has a
/// track (*agent decision*, 2026-10-16: the lobby's bars are all taller).
const FULL_ARROWS: i32 = 64;

const TRACK: [u8; 4] = [20, 23, 24, 255];
const LIGHT: [u8; 4] = [115, 120, 119, 255];
const DARK: [u8; 4] = [24, 27, 27, 255];
const FACE: [u8; 4] = [81, 83, 83, 255];
const PRESSED_FACE: [u8; 4] = [70, 72, 72, 255];
const GRIP: [u8; 4] = [40, 43, 44, 255];
/// The map's ink, for the arrow.
const INK: [u8; 4] = [232, 232, 218, 255];

/// The part of a bar a press landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
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

/// What a press on a row bar did. The owner moves its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowPress {
    /// Not on the bar.
    Miss,
    /// The arrow toward the start: one row back.
    Less,
    /// The arrow toward the end: one row on.
    More,
    /// The track before the thumb: a page back.
    PageLess,
    /// The track after the thumb: a page on.
    PageMore,
    /// The thumb is held.
    Grab,
}

/// How many rows (or lines) there are, how many show, and the first shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rows {
    pub total: usize,
    pub visible: usize,
    pub first: usize,
}

impl Rows {
    pub fn max_first(self) -> usize {
        self.total.saturating_sub(self.visible)
    }
    /// `first` moved by `by` (negative toward the start), kept in range.
    pub fn shifted(self, by: i64) -> usize {
        (self.first as i64 + by).clamp(0, self.max_first() as i64) as usize
    }
    fn span(self) -> (f64, f64, f64) {
        (
            self.total as f64,
            self.visible.max(1) as f64,
            self.first.min(self.max_first()) as f64,
        )
    }
}

/// One scroll bar: a rectangle, an arrow at each end, a track and a thumb.
/// It keeps only whether a thumb is held, for [`Self::drag_rows`].
#[derive(Clone, Copy, Debug)]
pub struct ArrowBar {
    rect: Rect,
    vertical: bool,
    /// How far into the thumb the pointer took hold, while it is held.
    grip: Option<i32>,
    pressed: Option<Part>,
}

impl ArrowBar {
    /// A vertical bar whose top left is `at`, `length` tall.
    pub fn vertical(at: Point, length: i32) -> Self {
        Self::new((at.0, at.1, THICK, length.max(THICK)), true)
    }
    /// A bar filling `rect`. The map builds its bars this way.
    pub fn new(rect: Rect, vertical: bool) -> Self {
        Self {
            rect,
            vertical,
            grip: None,
            pressed: None,
        }
    }

    #[cfg(test)]
    pub fn bounds(&self) -> Rect {
        self.rect
    }
    pub fn hit(&self, point: Point) -> bool {
        inside(self.rect, point)
    }
    pub fn is_vertical(&self) -> bool {
        self.vertical
    }
    #[cfg(test)]
    pub fn dragging(&self) -> bool {
        self.grip.is_some()
    }
    fn axis(&self) -> (i32, i32) {
        if self.vertical {
            (self.rect.1, self.rect.3)
        } else {
            (self.rect.0, self.rect.2)
        }
    }
    /// The arrow buttons' length along the bar. Sixteen on a long bar.
    fn button(&self) -> i32 {
        let (_, length) = self.axis();
        if length >= FULL_ARROWS {
            THICK
        } else {
            (length / 4).clamp(8, THICK)
        }
    }
    pub fn along(&self, (x, y): Point) -> i32 {
        if self.vertical { y } else { x }
    }

    /// The thumb's start and length along the axis. `total`, `visible` and
    /// `first` are in the owner's units (feet, or rows as floats).
    pub fn thumb(&self, total: f64, visible: f64, first: f64) -> (i32, i32) {
        let (start, length) = self.axis();
        let button = self.button();
        let track = (length - 2 * button).max(0);
        let ratio = if total > 0. {
            (visible / total).clamp(0., 1.)
        } else {
            1.
        };
        let size = ((f64::from(track) * ratio).round() as i32)
            .max(MIN_THUMB)
            .min(track.max(1));
        let travel = f64::from((track - size).max(0));
        let span = (total - visible).max(0.);
        let at = if span > 0. {
            (first / span).clamp(0., 1.) * travel
        } else {
            0.
        };
        (start + button + at.round() as i32, size)
    }

    /// The `first` a thumb that starts at `thumb_start` stands for.
    pub fn first_at(&self, thumb_start: i32, total: f64, visible: f64) -> f64 {
        let (start, length) = self.axis();
        let button = self.button();
        let track = (length - 2 * button).max(0);
        let (_, size) = self.thumb(total, visible, 0.);
        let travel = f64::from((track - size).max(1));
        let span = (total - visible).max(0.);
        (f64::from(thumb_start - start - button) / travel).clamp(0., 1.) * span
    }

    /// What `point` is on, when the bar has anything to scroll. Arrow
    /// buttons answer even when the view already shows everything.
    pub fn part(&self, point: Point, total: f64, visible: f64, first: f64) -> Option<Part> {
        if !self.hit(point) {
            return None;
        }
        let (start, length) = self.axis();
        let button = self.button();
        let a = self.along(point);
        if a < start + button {
            return Some(Part::Less);
        }
        if a >= start + length - button {
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

    /// A press on a bar that scrolls rows. Arrow buttons and the track are
    /// reported; a thumb is held for [`Self::drag_rows`].
    pub fn press_rows(&mut self, point: Point, rows: Rows) -> RowPress {
        self.grip = None;
        self.pressed = None;
        let (total, visible, first) = rows.span();
        let Some(part) = self.part(point, total, visible, first) else {
            return RowPress::Miss;
        };
        self.pressed = Some(part);
        match part {
            Part::Less => RowPress::Less,
            Part::More => RowPress::More,
            Part::PageLess => RowPress::PageLess,
            Part::PageMore => RowPress::PageMore,
            Part::Thumb => {
                let (thumb, _) = self.thumb(total, visible, first);
                self.grip = Some(self.along(point) - thumb);
                RowPress::Grab
            }
        }
    }

    /// The first row for a held thumb at `point`. None when nothing is held.
    pub fn drag_rows(&self, point: Point, rows: Rows) -> Option<usize> {
        let grip = self.grip?;
        let (total, visible, _) = rows.span();
        let first = self.first_at(self.along(point) - grip, total, visible);
        Some((first.round() as usize).min(rows.max_first()))
    }

    /// The mouse went up, or the window lost it.
    pub fn release(&mut self) {
        self.grip = None;
        self.pressed = None;
    }

    /// Draws the bar. `pressed` sinks an arrow or the thumb.
    pub fn draw(
        &self,
        canvas: &mut Canvas,
        total: f64,
        visible: f64,
        first: f64,
        pressed: Option<Part>,
    ) {
        let (x, y, w, h) = self.rect;
        canvas.rect((x, y, w, h), TRACK);
        bevel(canvas, self.rect, true);
        let (start, length) = self.axis();
        let button = self.button();
        let button_rect = |less: bool| -> Rect {
            let at = if less { start } else { start + length - button };
            if self.vertical {
                (x, at, w, button)
            } else {
                (at, y, button, h)
            }
        };
        for (less, part) in [(true, Part::Less), (false, Part::More)] {
            let rect = button_rect(less);
            let down = pressed == Some(part);
            raised(canvas, rect, down);
            arrow(canvas, rect, self.vertical, less, down);
        }
        if total > visible {
            let (thumb, size) = self.thumb(total, visible, first);
            let rect = if self.vertical {
                (x, thumb, w, size)
            } else {
                (thumb, y, size, h)
            };
            raised(canvas, rect, pressed == Some(Part::Thumb));
            let (cx, cy) = (rect.0 + rect.2 / 2, rect.1 + rect.3 / 2);
            if size >= 20 {
                for d in [-3, 0, 3] {
                    if self.vertical {
                        canvas.rect((rect.0 + 4, cy + d, rect.2 - 8, 1), GRIP);
                    } else {
                        canvas.rect((cx + d, rect.1 + 4, 1, rect.3 - 8), GRIP);
                    }
                }
            }
        }
    }

    /// Draws a bar over rows.
    pub fn draw_rows(&self, canvas: &mut Canvas, rows: Rows) {
        let (total, visible, first) = rows.span();
        let pressed = self
            .pressed
            .filter(|part| matches!(part, Part::Thumb | Part::Less | Part::More));
        self.draw(canvas, total, visible, first, pressed);
    }
}

fn bevel(c: &mut Canvas, (x, y, w, h): Rect, inset: bool) {
    let (top, bottom) = if inset { (DARK, LIGHT) } else { (LIGHT, DARK) };
    c.rect((x, y, w, 1), top);
    c.rect((x, y, 1, h), top);
    c.rect((x, y + h - 1, w, 1), bottom);
    c.rect((x + w - 1, y, 1, h), bottom);
}

pub(crate) fn raised(c: &mut Canvas, rect: Rect, pressed: bool) {
    let (x, y, w, h) = rect;
    c.rect((x, y, w, h), if pressed { PRESSED_FACE } else { FACE });
    bevel(c, rect, pressed);
    if !pressed {
        c.rect((x + 1, y + 1, w - 2, 1), LIGHT);
        c.rect((x + 1, y + 1, 1, h - 2), LIGHT);
    }
}

fn arrow(c: &mut Canvas, (x, y, w, h): Rect, vertical: bool, less: bool, pressed: bool) {
    let shift = i32::from(pressed);
    let (cx, cy) = (x + w / 2 + shift, y + h / 2 + shift);
    for i in 0..4 {
        let half = i;
        let line = if less { i } else { 3 - i } - 2;
        if vertical {
            c.rect((cx - half, cy + line, 2 * half + 1, 1), INK);
        } else {
            c.rect((cx + line, cy - half, 1, 2 * half + 1), INK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(total: usize, first: usize) -> Rows {
        Rows {
            total,
            visible: 5,
            first,
        }
    }

    #[test]
    fn arrows_track_and_thumb_are_distinct_hits() {
        let mut bar = ArrowBar::vertical((100, 50), 90);
        let view = rows(25, 10);
        let (total, visible, first) = view.span();
        let (thumb, size) = bar.thumb(total, visible, first);
        assert_eq!(
            bar.part((108, 50 + 4), total, visible, first),
            Some(Part::Less)
        );
        assert_eq!(
            bar.part((108, 50 + 90 - 4), total, visible, first),
            Some(Part::More)
        );
        assert_eq!(
            bar.part((108, thumb - 2), total, visible, first),
            Some(Part::PageLess)
        );
        assert_eq!(
            bar.part((108, thumb + size + 2), total, visible, first),
            Some(Part::PageMore)
        );
        assert_eq!(
            bar.part((108, thumb + size / 2), total, visible, first),
            Some(Part::Thumb)
        );
        assert_eq!(bar.part((99, 80), total, visible, first), None);
        assert_eq!(bar.press_rows((108, thumb + 2), view), RowPress::Grab);
        assert!(bar.dragging());
        let moved = bar.drag_rows((108, thumb + 2 + 20), view);
        assert!(moved.is_some_and(|first| first > 10));
        assert_eq!(bar.drag_rows((400, thumb + 2 + 20), view), moved);
        bar.release();
        assert!(!bar.dragging());
        assert_eq!(bar.drag_rows((108, 80), view), None);
    }

    #[test]
    fn a_view_that_fits_has_arrows_and_no_thumb() {
        let mut bar = ArrowBar::vertical((0, 0), 90);
        let view = rows(4, 0);
        let (total, visible, first) = view.span();
        assert_eq!(bar.part((8, 4), total, visible, first), Some(Part::Less));
        assert_eq!(bar.part((8, 40), total, visible, first), None);
        assert_eq!(bar.press_rows((8, 40), view), RowPress::Miss);
        assert_eq!(bar.press_rows((8, 4), view), RowPress::Less);
    }

    #[test]
    fn dragging_reaches_both_ends_and_maps_back() {
        let mut bar = ArrowBar::vertical((10, 20), 120);
        let view = rows(30, 0);
        let last = view.max_first();
        for first in [0, last / 2, last] {
            let view = Rows { first, ..view };
            let (total, visible, at) = view.span();
            let (thumb, size) = bar.thumb(total, visible, at);
            bar.press_rows((18, thumb + size / 2), view);
            let back = bar.drag_rows((18, thumb + size / 2), view).unwrap();
            assert!(back.abs_diff(first) <= 1, "first {first} read {back}");
            bar.release();
        }
        let view = rows(30, 5);
        let (thumb, _) = {
            let (t, v, f) = view.span();
            bar.thumb(t, v, f)
        };
        bar.press_rows((18, thumb + 3), view);
        assert_eq!(bar.drag_rows((18, 0), view), Some(0));
        assert_eq!(bar.drag_rows((18, 400), view), Some(last));
    }

    #[test]
    fn the_thumb_is_drawn_only_when_the_view_scrolls() {
        let bar = ArrowBar::vertical((40, 30), 80);
        let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
        bar.draw_rows(
            &mut Canvas(&mut pixels),
            Rows {
                total: 3,
                visible: 5,
                first: 0,
            },
        );
        // The top arrow button's face, clear of the arrow, and no thumb in the track.
        assert_eq!(crate::widgets::test_kit::at(&pixels, 42, 34), [81, 83, 83]);
        assert_eq!(crate::widgets::test_kit::at(&pixels, 48, 70), [20, 23, 24]);
        bar.draw_rows(
            &mut Canvas(&mut pixels),
            Rows {
                total: 20,
                visible: 5,
                first: 0,
            },
        );
        let (thumb, _) = bar.thumb(20., 5., 0.);
        assert_eq!(
            crate::widgets::test_kit::at(&pixels, 48, thumb + 2),
            [81, 83, 83]
        );
    }
}
