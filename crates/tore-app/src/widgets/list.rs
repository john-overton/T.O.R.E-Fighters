//! The paged list: retail's recessed row bars, a selected row, columns and
//! icons, the PREV/NEXT rocker and the "PAGE n of m" box.
use super::{
    Icon, Kit, Outcome, Point, Rect, Widget,
    draw::{bar, blit_part, fit, focus_mark},
    inside,
};
use crate::menu::{Canvas, text_width};
use crate::rocker::Rocker;
use std::time::{Duration, Instant};

/// A second press on the same row this soon after the first is a
/// double-click (the Replays list's 500 ms).
pub const DOUBLE_CLICK: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Centre,
}

/// A column: where its text starts (from the row's text origin, which is 20
/// pixels in from the bar's left edge, after the selection marker), how wide
/// it is, and how its cell sits in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    pub x: i32,
    pub width: i32,
    pub align: Align,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    Text(String),
    Icon(Icon),
    Empty,
}

/// One row: a cell for each column, and a key that tells the row apart from
/// its neighbours across [`List::set_rows`] (the discovery list refreshes
/// every two seconds and the player's selection must stay on the same game).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: String,
    pub cells: Vec<Cell>,
    /// Drawn in a dimmer text (a game from another version, one that is
    /// full).
    pub dim: bool,
}

impl Row {
    pub fn new(key: impl Into<String>, cells: Vec<Cell>) -> Self {
        Self {
            key: key.into(),
            cells,
            dim: false,
        }
    }
    /// A one-column row of text, keyed by the text.
    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self::new(text.clone(), vec![Cell::Text(text)])
    }
    pub fn dimmed(mut self) -> Self {
        self.dim = true;
        self
    }
}

/// Where the paging controls sit, in canvas pixels: the rocker's top left,
/// the PREV and NEXT labels, the PAGE label and the page box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pager {
    pub rocker: Point,
    pub prev: Point,
    pub next: Point,
    pub page_label: Point,
    pub counter_box: Point,
}

impl Pager {
    /// NEWNET's, beside and under its Games list (EF0, `docs/formats/menu.md`).
    pub const NEWNET: Pager = Pager {
        rocker: (280, 200),
        prev: (252, 201),
        next: (252, 224),
        page_label: (91, 265),
        counter_box: (120, 261),
    };
}

/// The rocker's two halves, each 18 by 17 at the sprite's top left (the
/// size the existing rocker users hit).
const HALF: (i32, i32) = (18, 17);
/// A row bar is 17 high; the well inside spans rows 1 to 13.
const BAR_HEIGHT: i32 = 17;
/// The selection marker (`LISTHI`'s gold half) starts 4 pixels in and is 12
/// square; row text starts 4 pixels after it.
const MARKER: i32 = 4;
const TEXT_ORIGIN: i32 = 20;

/// A list of rows in recessed bars, `visible` to a page, 18 pixels apart on
/// NEWNET (4 rows, 200 wide).
///
/// Selection (*agent decisions*): the selected row is always on the shown
/// page. A click selects it (`Changed` when it differs); a second click on it
/// within 500 ms, or Enter, answers `Activated`. Up and Down move the
/// selection a row, PageUp and PageDown a page, Home and End to the ends,
/// the wheel a row; the page follows the selection. The rocker turns a page on
/// press and, with a row selected, moves the selection to the same place on
/// the new page. The selected row is marked with `LISTHI`'s gold stripe.
///
/// The page box reads "PAGE n of m" in retail's format (EF0): the current
/// page right aligned, ` of `, the count of pages, with two spaces before the
/// count under 10. An empty list reads `1  of  0`, as John's screenshot does,
/// and PREV and NEXT are dim at the first and last page of a list that has
/// rows (bright when it is empty, as the screenshot shows).
#[derive(Clone, Debug)]
pub struct List {
    at: Point,
    width: i32,
    visible: usize,
    pitch: i32,
    columns: Vec<Column>,
    rows: Vec<Row>,
    selected: Option<usize>,
    page: usize,
    pager: Option<Pager>,
    rocker: Rocker,
    last_click: Option<(usize, Instant)>,
    enabled: bool,
}

impl List {
    /// A list whose first row bar's top left is `at`, `width` wide, showing
    /// `visible` rows on a page, with one column that spans the row.
    pub fn new(at: Point, width: i32, visible: usize) -> Self {
        Self {
            at,
            width,
            visible: visible.max(1),
            pitch: 18,
            columns: vec![Column {
                x: 0,
                width: width - TEXT_ORIGIN - 8,
                align: Align::Left,
            }],
            rows: Vec::new(),
            selected: None,
            page: 0,
            pager: None,
            rocker: Rocker::new(),
            last_click: None,
            enabled: true,
        }
    }
    pub fn with_pager(mut self, pager: Pager) -> Self {
        self.pager = Some(pager);
        self
    }
    pub fn with_columns(mut self, columns: Vec<Column>) -> Self {
        self.columns = columns;
        self
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn visible(&self) -> usize {
        self.visible
    }
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn selected_row(&self) -> Option<&Row> {
        self.selected.and_then(|i| self.rows.get(i))
    }
    /// The page shown, from 0.
    pub fn page(&self) -> usize {
        self.page
    }
    /// How many pages the rows fill: 0 for an empty list.
    pub fn pages(&self) -> usize {
        self.rows.len().div_ceil(self.visible)
    }

    /// Replaces the rows. The selection stays on the row with the same key,
    /// or is cleared when that row is gone; the page follows the selection,
    /// or is kept within the new page count.
    pub fn set_rows(&mut self, rows: Vec<Row>) {
        let key = self
            .selected
            .and_then(|i| self.rows.get(i))
            .map(|row| row.key.clone());
        self.rows = rows;
        self.selected = key.and_then(|key| self.rows.iter().position(|row| row.key == key));
        self.last_click = None;
        self.page = match self.selected {
            Some(i) => i / self.visible,
            None => self.page.min(self.pages().saturating_sub(1)),
        };
    }
    /// Selects row `index` (clamped to the last row; ignored when empty) and
    /// shows its page.
    pub fn select(&mut self, index: usize) -> Outcome {
        if self.rows.is_empty() {
            return Outcome::None;
        }
        let index = index.min(self.rows.len() - 1);
        self.page = index / self.visible;
        if self.selected == Some(index) {
            Outcome::None
        } else {
            self.selected = Some(index);
            Outcome::Changed
        }
    }
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// The text of the page box, as retail formats it, for example `1  of  3`.
    pub fn page_counter(&self) -> String {
        let (left, right) = self.counter_parts();
        format!("{left}{right}")
    }
    fn counter_parts(&self) -> (String, String) {
        let (page, pages) = (self.page + 1, self.pages());
        let left = if page < 10 {
            format!("{page} ")
        } else {
            page.to_string()
        };
        let right = if pages < 10 {
            format!(" of  {pages}")
        } else {
            format!(" of {pages}")
        };
        (left, right)
    }

    fn turn(&mut self, forward: bool) -> Outcome {
        let pages = self.pages();
        let target = if forward {
            if self.page + 1 >= pages {
                return Outcome::None;
            }
            self.page + 1
        } else {
            if self.page == 0 {
                return Outcome::None;
            }
            self.page - 1
        };
        self.page = target;
        if let Some(i) = self.selected {
            let moved = (target * self.visible + i % self.visible).min(self.rows.len() - 1);
            self.selected = Some(moved);
        }
        Outcome::Changed
    }
    fn bar_rect(&self, slot: usize) -> Rect {
        (
            self.at.0,
            self.at.1 + self.pitch * slot as i32,
            self.width,
            BAR_HEIGHT,
        )
    }
    fn rocker_halves(&self) -> Option<(Rect, Rect)> {
        self.pager.map(|pager| {
            let (x, y) = pager.rocker;
            ((x, y, HALF.0, HALF.1), (x, y + HALF.1, HALF.0, HALF.1))
        })
    }
    fn row_at(&self, point: Point) -> Option<usize> {
        (0..self.visible).find_map(|slot| {
            let index = self.page * self.visible + slot;
            (index < self.rows.len() && inside(self.bar_rect(slot), point)).then_some(index)
        })
    }

    /// The mouse went down. A row selects (a second press within 500 ms
    /// activates it); a rocker half turns the page and tilts the rocker.
    pub fn press(&mut self, point: Point, now: Instant) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        if let Some((prev, next)) = self.rocker_halves() {
            for (rect, forward) in [(prev, false), (next, true)] {
                if inside(rect, point) {
                    let outcome = self.turn(forward);
                    self.rocker.push(forward, true, now);
                    return outcome;
                }
            }
        }
        let Some(index) = self.row_at(point) else {
            return Outcome::None;
        };
        let again = self
            .last_click
            .is_some_and(|(row, at)| row == index && now.duration_since(at) < DOUBLE_CLICK);
        self.last_click = Some((index, now));
        let changed = self.select(index);
        if again {
            // A double-click must not chain into a triple one.
            self.last_click = None;
            Outcome::Activated
        } else {
            changed
        }
    }
    /// The mouse went up: the rocker springs back.
    pub fn release(&mut self, now: Instant) {
        if self.rocker.held() {
            self.rocker.release(now);
        }
    }
    /// Moves the rocker's animation on; true while it is still moving.
    pub fn advance(&mut self, now: Instant) -> bool {
        self.rocker.advance(now)
    }

    /// A key while the list has the focus.
    pub fn key(&mut self, name: &str) -> Outcome {
        if !self.enabled || self.rows.is_empty() {
            return Outcome::None;
        }
        let last = self.rows.len() - 1;
        let target = match name {
            "ArrowDown" => self.selected.map_or(0, |i| (i + 1).min(last)),
            "ArrowUp" => self.selected.map_or(0, |i| i.saturating_sub(1)),
            "PageDown" => self.selected.map_or(0, |i| (i + self.visible).min(last)),
            "PageUp" => self.selected.map_or(0, |i| i.saturating_sub(self.visible)),
            "Home" => 0,
            "End" => last,
            "Enter" => {
                return if self.selected.is_some() {
                    Outcome::Activated
                } else {
                    Outcome::None
                };
            }
            _ => return Outcome::None,
        };
        self.select(target)
    }
    /// A wheel step: positive scrolls up (the previous row), negative down.
    pub fn wheel(&mut self, lines: i32) -> Outcome {
        if !self.enabled || self.rows.is_empty() || lines == 0 {
            return Outcome::None;
        }
        let last = self.rows.len() as i64 - 1;
        let from = self.selected.map_or(-1, |i| i as i64);
        let to = (from - i64::from(lines)).clamp(0, last);
        self.select(to as usize)
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit, focused: bool) {
        let (left, middle, right) = (
            kit.sprite("LISTLFT"),
            kit.sprite("LISTMID"),
            kit.sprite("LISTRT"),
        );
        let font = kit.sprite("SMLFONT");
        let marker = kit.sprite("LISTHI");
        for slot in 0..self.visible {
            let (x, y, w, _) = self.bar_rect(slot);
            bar(canvas, (left, middle, right), (x, y), w);
            let index = self.page * self.visible + slot;
            let Some(row) = self.rows.get(index) else {
                continue;
            };
            if self.selected == Some(index) {
                // The gold half of the stripe picture.
                blit_part(
                    canvas,
                    marker,
                    (x + MARKER, y + 1),
                    (12, 0, 12, 12),
                    (x, y, w, BAR_HEIGHT),
                    1.0,
                );
            }
            let tint = row.dim.then_some([118, 118, 118]);
            let origin = x + TEXT_ORIGIN;
            for (column, cell) in self.columns.iter().zip(&row.cells) {
                match cell {
                    Cell::Empty => {}
                    Cell::Icon(icon) => {
                        let (iw, ih) = icon.size();
                        let ix = match column.align {
                            Align::Left => 0,
                            Align::Centre => (column.width - iw) / 2,
                            Align::Right => column.width - iw,
                        };
                        icon.draw(canvas, (origin + column.x + ix, y + (BAR_HEIGHT - ih) / 2));
                    }
                    Cell::Text(text) => {
                        let text = fit(font, text, column.width);
                        let tw = text_width(font, &text);
                        let tx = match column.align {
                            Align::Left => 0,
                            Align::Centre => (column.width - tw) / 2,
                            Align::Right => column.width - tw,
                        };
                        canvas.text(font, &text, origin + column.x + tx, y + 1, tint);
                    }
                }
            }
        }
        if let Some(pager) = self.pager {
            self.draw_pager(canvas, kit, pager);
        }
        if focused && self.enabled {
            let (x, y, w, _) = self.bar_rect(0);
            let height = self.pitch * (self.visible as i32 - 1) + BAR_HEIGHT;
            focus_mark(canvas, (x - 3, y - 3, w + 6, height + 6));
        }
    }

    fn draw_pager(&self, canvas: &mut Canvas, kit: &Kit, pager: Pager) {
        let (bright, dim) = (kit.sprite("PANELFNT"), kit.sprite("PANELFND"));
        let pages = self.pages();
        // John's screenshot shows both labels bright on an empty list, so
        // they dim only at the ends of a list that has rows (*agent
        // decision*, EF0 read the dim face as "disabled" without saying
        // when), or when the whole list is disabled.
        let can_prev = self.enabled && (pages == 0 || self.page > 0);
        let can_next = self.enabled && (pages == 0 || self.page + 1 < pages);
        canvas.text(
            if can_prev { bright } else { dim },
            "PREV",
            pager.prev.0,
            pager.prev.1,
            None,
        );
        canvas.text(
            if can_next { bright } else { dim },
            "NEXT",
            pager.next.0,
            pager.next.1,
            None,
        );
        let rocker = kit.sprite(&self.rocker.sprite());
        canvas.blit(rocker, pager.rocker, 0, rocker.width, 1.0);
        canvas.text(bright, "PAGE", pager.page_label.0, pager.page_label.1, None);
        let page_box = kit.sprite("PAGEBOX");
        canvas.blit(page_box, pager.counter_box, 0, page_box.width, 1.0);
        // EF0: the string starts at the counter's x + 15 minus the width of
        // its first part, so the page number is right aligned; the counter's
        // origin is one pixel inside the box and its text 3 pixels down.
        let font = kit.sprite("SMLFONT");
        let (left, right) = self.counter_parts();
        let x = pager.counter_box.0 + 1 + 15 - text_width(font, &left);
        canvas.text(
            font,
            &format!("{left}{right}"),
            x,
            pager.counter_box.1 + 3,
            None,
        );
    }
}

impl Widget for List {
    fn bounds(&self) -> Rect {
        let height = self.pitch * (self.visible as i32 - 1) + BAR_HEIGHT;
        (self.at.0, self.at.1, self.width, height)
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, kit, start, tone_of};
    use super::*;

    fn rows(n: usize) -> Vec<Row> {
        (0..n).map(|i| Row::text(format!("game {i}"))).collect()
    }
    fn list(n: usize) -> List {
        let mut l = List::new((48, 185), 200, 4).with_pager(Pager::NEWNET);
        l.set_rows(rows(n));
        l
    }
    /// The middle of a row bar on the shown page.
    fn row_point(slot: i32) -> Point {
        (100, 185 + 18 * slot + 8)
    }

    #[test]
    fn an_empty_list_reads_one_of_zero_and_selects_nothing() {
        let mut l = list(0);
        assert_eq!((l.pages(), l.page(), l.selected()), (0, 0, None));
        assert_eq!(l.page_counter(), "1  of  0");
        for key in ["ArrowDown", "ArrowUp", "PageDown", "Home", "End", "Enter"] {
            assert_eq!(l.key(key), Outcome::None, "{key}");
        }
        assert_eq!(l.wheel(-1), Outcome::None);
        assert_eq!(l.select(3), Outcome::None);
        assert_eq!(l.press(row_point(0), start()), Outcome::None);
        let now = start();
        assert_eq!(
            l.press(Pager::NEWNET.rocker, now),
            Outcome::None,
            "no page to turn to"
        );
        assert_eq!(l.page(), 0);
    }

    #[test]
    fn one_page_has_no_page_to_turn() {
        let mut l = list(3);
        assert_eq!(l.pages(), 1);
        assert_eq!(l.page_counter(), "1  of  1");
        assert_eq!(l.key("ArrowDown"), Outcome::Changed);
        assert_eq!(
            l.selected(),
            Some(0),
            "Down with nothing selected takes the first"
        );
        l.key("ArrowDown");
        l.key("ArrowDown");
        assert_eq!(l.key("ArrowDown"), Outcome::None);
        assert_eq!(l.selected(), Some(2), "Down stops at the last row");
        let now = start();
        let next = (Pager::NEWNET.rocker.0 + 5, Pager::NEWNET.rocker.1 + 25);
        assert_eq!(l.press(next, now), Outcome::None);
        assert_eq!(l.page(), 0);
    }

    #[test]
    fn many_pages_page_with_the_keys_and_the_selection_stays_on_view() {
        let mut l = list(10); // 4 to a page: 3 pages
        assert_eq!(l.pages(), 3);
        assert_eq!(l.page_counter(), "1  of  3");
        l.key("ArrowDown");
        assert_eq!(l.key("PageDown"), Outcome::Changed);
        assert_eq!((l.selected(), l.page()), (Some(4), 1));
        assert_eq!(l.page_counter(), "2  of  3");
        l.key("PageDown");
        l.key("PageDown");
        assert_eq!(
            (l.selected(), l.page()),
            (Some(9), 2),
            "clamped to the last row"
        );
        assert_eq!(l.key("Home"), Outcome::Changed);
        assert_eq!((l.selected(), l.page()), (Some(0), 0));
        l.key("End");
        assert_eq!((l.selected(), l.page()), (Some(9), 2));
        l.key("PageUp");
        assert_eq!((l.selected(), l.page()), (Some(5), 1));
        assert_eq!(l.wheel(1), Outcome::Changed, "wheel up is the previous row");
        assert_eq!(l.selected(), Some(4));
        assert_eq!(l.wheel(-3), Outcome::Changed);
        assert_eq!(l.selected(), Some(7));
        assert_eq!(l.page(), 1);
        assert_eq!(l.key("Enter"), Outcome::Activated);
    }

    #[test]
    fn the_rocker_turns_a_page_on_press_and_keeps_the_row_position() {
        let mut l = list(10);
        l.select(1);
        let now = start();
        let next = (Pager::NEWNET.rocker.0 + 5, Pager::NEWNET.rocker.1 + 25);
        let prev = (Pager::NEWNET.rocker.0 + 5, Pager::NEWNET.rocker.1 + 5);
        assert_eq!(
            l.press(prev, now),
            Outcome::None,
            "PREV on the first page does nothing"
        );
        l.release(now);
        assert_eq!(l.press(next, now), Outcome::Changed);
        assert_eq!((l.page(), l.selected()), (1, Some(5)));
        assert!(l.rocker.held());
        l.release(now);
        assert!(!l.rocker.held());
        l.press(next, now);
        assert_eq!(
            (l.page(), l.selected()),
            (2, Some(9)),
            "the last page is short: clamped"
        );
        assert_eq!(l.press(next, now), Outcome::None);
        l.press(prev, now);
        assert_eq!((l.page(), l.selected()), (1, Some(5)));
        // With nothing selected the page still turns.
        let mut l = list(10);
        l.press(next, now);
        assert_eq!((l.page(), l.selected()), (1, None));
    }

    #[test]
    fn a_click_selects_and_a_quick_second_click_activates() {
        let mut l = list(6);
        let t0 = start();
        assert_eq!(l.press(row_point(2), t0), Outcome::Changed);
        assert_eq!(l.selected(), Some(2));
        assert_eq!(
            l.press(row_point(2), t0 + Duration::from_millis(300)),
            Outcome::Activated
        );
        // Not a triple click.
        assert_eq!(
            l.press(row_point(2), t0 + Duration::from_millis(400)),
            Outcome::None
        );
        // Too slow, or a different row, is a plain click.
        assert_eq!(
            l.press(row_point(2), t0 + Duration::from_millis(1500)),
            Outcome::None
        );
        assert_eq!(
            l.press(row_point(1), t0 + Duration::from_millis(1600)),
            Outcome::Changed
        );
        assert_eq!(
            l.press(row_point(2), t0 + Duration::from_millis(1700)),
            Outcome::Changed
        );
        // A click below the last row of a short page selects nothing new.
        let mut short = list(2);
        short.select(0);
        assert_eq!(short.press(row_point(3), t0), Outcome::None);
        assert_eq!(short.selected(), Some(0));
        // On page 2 a click means the row on that page.
        let mut l = list(6);
        l.key("End");
        assert_eq!(l.page(), 1);
        l.press(row_point(0), t0);
        assert_eq!(l.selected(), Some(4));
    }

    #[test]
    fn new_rows_keep_the_selection_by_key_and_clamp_the_page() {
        let mut l = list(10);
        l.select(5);
        // The same games again in another order, one new in front.
        let mut again = rows(10);
        again.reverse();
        again.insert(0, Row::text("new game"));
        l.set_rows(again);
        assert_eq!(l.selected_row().unwrap().key, "game 5");
        assert_eq!(l.selected(), Some(5));
        assert_eq!(l.page(), 1);
        // The selected game vanishes: nothing is selected and the page is
        // kept within the new count.
        l.set_rows(rows(2));
        assert_eq!((l.selected(), l.page()), (None, 0));
        l.set_rows(rows(0));
        assert_eq!(
            (l.selected(), l.pages(), l.page_counter()),
            (None, 0, "1  of  0".to_string())
        );
        // Browsing a page with nothing selected is kept while it exists.
        let mut l = list(10);
        l.press(
            (Pager::NEWNET.rocker.0 + 5, Pager::NEWNET.rocker.1 + 25),
            start(),
        );
        l.set_rows(rows(10));
        assert_eq!(l.page(), 1);
        l.set_rows(rows(5));
        assert_eq!(l.page(), 1);
        l.set_rows(rows(3));
        assert_eq!(l.page(), 0);
    }

    #[test]
    fn the_counter_has_retails_spacing() {
        let mut l = list(0);
        assert_eq!(l.page_counter(), "1  of  0");
        l.set_rows(rows(36));
        assert_eq!(l.page_counter(), "1  of  9");
        l.set_rows(rows(40));
        assert_eq!(l.page_counter(), "1  of 10");
        l.select(39);
        assert_eq!(l.page_counter(), "10 of 10");
    }

    #[test]
    fn drawing_marks_the_selected_row_and_dims_the_ends() {
        let kit = kit();
        let mut l = list(10);
        l.select(1);
        let mut pixels = blank();
        l.draw(&mut Canvas(&mut pixels), &kit, false);
        // The gold half of LISTHI (synthetic: one colour) starts 4 in.
        let marker = tone_of(&kit, "LISTHI");
        assert_eq!(at(&pixels, 48 + 5, 185 + 18 + 3), marker);
        assert_ne!(at(&pixels, 48 + 5, 185 + 3), marker, "other rows are bare");
        // PREV is dim on the first page, NEXT bright; and the box counter is drawn.
        assert_eq!(
            at(&pixels, Pager::NEWNET.prev.0 + 1, Pager::NEWNET.prev.1 + 1),
            tone_of(&kit, "PANELFND")
        );
        assert_eq!(
            at(&pixels, Pager::NEWNET.next.0 + 1, Pager::NEWNET.next.1 + 1),
            tone_of(&kit, "PANELFNT")
        );
        assert_eq!(
            at(
                &pixels,
                Pager::NEWNET.counter_box.0 + 6,
                Pager::NEWNET.counter_box.1 + 6
            ),
            tone_of(&kit, "SMLFONT")
        );
    }

    #[test]
    fn columns_align_cells_and_icons() {
        let kit = kit();
        let mut l = List::new((48, 185), 200, 4).with_columns(vec![
            Column {
                x: 0,
                width: 11,
                align: Align::Centre,
            },
            Column {
                x: 20,
                width: 60,
                align: Align::Right,
            },
        ]);
        l.set_rows(vec![Row::new(
            "a",
            vec![Cell::Icon(Icon::Crown), Cell::Text("abc".into())],
        )]);
        let mut pixels = blank();
        l.draw(&mut Canvas(&mut pixels), &kit, false);
        // The crown (11 wide) fills its 11 pixel column from x + 20; the text
        // (3 characters of 6 pixels) ends at the column's right edge.
        assert_eq!(at(&pixels, 48 + 20, 185 + 5), [255, 205, 60]);
        let right_edge = 48 + 20 + 20 + 60;
        assert_eq!(
            at(&pixels, right_edge - 1, 185 + 5),
            tone_of(&kit, "SMLFONT")
        );
        assert_ne!(
            at(&pixels, right_edge - 19, 185 + 5),
            tone_of(&kit, "SMLFONT")
        );
        // A long text is cut to its column with a mark.
        assert_eq!(fit(kit.sprite("SMLFONT"), "abcdefghijk", 30), "abcd~");
    }

    use crate::menu::Canvas;
}
