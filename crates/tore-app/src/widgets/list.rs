//! The list: retail's recessed row bars, a selected row, columns and icons,
//! and either the PREV/NEXT rocker with the "PAGE n of m" box (a paged list)
//! or the red scroll bar (a scrolling one).
use super::{
    Icon, Kit, Outcome, Point, Rect, Widget,
    draw::{bar, blit_part, fit, focus_mark, ghost},
    inside,
    scroll_bar::{Press, ScrollBar, View},
};
use crate::menu::{Canvas, text_width};
use crate::rocker::Rocker;
use crate::ui_text;
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
    /// Drawn in this colour instead (the lobby's unable players, EF8). A
    /// dimmed row stays dim.
    pub tint: Option<[u8; 3]>,
    /// The well behind the row's text filled with this colour (a taken PvP
    /// slot, lobby pass L3). The text colour is the row's own.
    pub fill: Option<[u8; 3]>,
}

impl Row {
    pub fn new(key: impl Into<String>, cells: Vec<Cell>) -> Self {
        Self {
            key: key.into(),
            cells,
            dim: false,
            tint: None,
            fill: None,
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
    /// The row's text in `colour`.
    pub fn tinted(mut self, colour: [u8; 3]) -> Self {
        self.tint = Some(colour);
        self
    }
    /// The well behind the row filled with `colour`.
    pub fn filled(mut self, colour: [u8; 3]) -> Self {
        self.fill = Some(colour);
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
/// How far a filled row's colour stops short of the bar's ends.
const FILL_INSET: i32 = 2;
const TEXT_ORIGIN: i32 = 20;

/// A list of rows in recessed bars, `visible` to a page, 18 pixels apart on
/// NEWNET (4 rows, 200 wide).
///
/// # Scrolling mode
///
/// [`List::with_scroll_bar`] turns the list from pages into a window that
/// slides over the rows (the lobby pass, *opinionated*, John 2026-10-09):
/// the retail scroll bar ([`ScrollBar`]) is drawn beside it, and
///
/// - the wheel scrolls the *window* a row a notch (see
///   [`List::with_wheel_rows`]) and leaves the selection where it is, which
///   may then be out of sight;
/// - a click on the track above or below the knob scrolls a window's worth,
///   and a drag on the knob follows the pointer ([`List::drag`], with
///   [`List::release`] to let go), neither touching the selection;
/// - the keys move the selection as they do in a paged list, and the window
///   follows the selection, so keyboard use always shows the selected row;
/// - [`List::set_rows`] keeps the window where it is (kept within the new
///   rows), so a list that refreshes every two seconds does not jump;
/// - [`List::select`] and [`List::scroll_to`] are for the screen: the first
///   brings the row into view, the second moves the window only.
///
/// A paged list is as described below.
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
    /// The scroll bar of a scrolling list, and the first row of its window.
    scroll: Option<ScrollBar>,
    first: usize,
    wheel_rows: usize,
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
            scroll: None,
            first: 0,
            wheel_rows: 1,
            rocker: Rocker::new(),
            last_click: None,
            enabled: true,
        }
    }
    pub fn with_pager(mut self, pager: Pager) -> Self {
        self.pager = Some(pager);
        self
    }
    /// A scrolling list with its scroll bar's art at `at` (34 wide, as high
    /// as the list's rows). Instead of a pager: any pager is dropped.
    pub fn with_scroll_bar(mut self, at: Point) -> Self {
        self.pager = None;
        let height = self.pitch * (self.visible as i32 - 1) + BAR_HEIGHT;
        self.scroll = Some(ScrollBar::new(at, height));
        self
    }
    /// How many rows the wheel scrolls a notch in a scrolling list (default
    /// one; at least one).
    pub fn with_wheel_rows(mut self, rows: usize) -> Self {
        self.wheel_rows = rows.max(1);
        self
    }
    pub fn with_columns(mut self, columns: Vec<Column>) -> Self {
        self.columns = columns;
        self
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    #[cfg(test)]
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
    pub fn selected_row(&self) -> Option<&Row> {
        self.selected.and_then(|i| self.rows.get(i))
    }
    /// The page shown, from 0.
    #[cfg(test)]
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
        if self.scroll.is_some() {
            // The window stays where it is.
            self.first = self.first.min(self.view().max_first());
            return;
        }
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
        if self.scroll.is_some() {
            self.reveal(index);
        } else {
            self.page = index / self.visible;
        }
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

    /// The index of the first row shown: the window's top in a scrolling
    /// list, the page's first row in a paged one.
    pub fn first_row(&self) -> usize {
        if self.scroll.is_some() {
            self.first
        } else {
            self.page * self.visible
        }
    }
    /// How many rows show at once.
    #[cfg(test)]
    pub fn visible_rows(&self) -> usize {
        self.visible
    }
    /// What the scroll bar is asked about: the rows, the window and its top.
    fn view(&self) -> View {
        View {
            total: self.rows.len(),
            visible: self.visible,
            first: self.first,
        }
    }
    /// Slides a scrolling list's window to start at row `first` (kept in
    /// range); the selection is not touched. No effect on a paged list.
    pub fn scroll_to(&mut self, first: usize) {
        if self.scroll.is_some() {
            self.first = first.min(self.view().max_first());
        }
    }
    /// Slides the window `rows` rows (negative toward the top).
    pub fn scroll_by(&mut self, rows: i32) {
        if self.scroll.is_some() {
            self.first = self.view().shifted(i64::from(rows));
        }
    }
    /// Slides a scrolling list's window just far enough to show row `index`.
    fn reveal(&mut self, index: usize) {
        if index < self.first {
            self.first = index;
        } else if index >= self.first + self.visible {
            self.first = index + 1 - self.visible;
        }
        self.first = self.first.min(self.view().max_first());
    }

    /// The text of the page box, as retail formats it, for example `1  of  3`.
    #[cfg(test)]
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
    /// The index of the row under `point` on the shown page, if any (the
    /// rocker is not a row).
    pub fn row_at(&self, point: Point) -> Option<usize> {
        (0..self.visible).find_map(|slot| {
            let index = self.first_row() + slot;
            (index < self.rows.len() && inside(self.bar_rect(slot), point)).then_some(index)
        })
    }

    /// The mouse went down. A row selects (a second press within 500 ms
    /// activates it); a rocker half turns the page and tilts the rocker; the
    /// scroll bar's knob is grabbed, and its track pages the window.
    pub fn press(&mut self, point: Point, now: Instant) -> Outcome {
        if !self.enabled {
            return Outcome::None;
        }
        if let Some(mut bar) = self.scroll {
            let view = self.view();
            match bar.press(point, view) {
                Press::Miss => {}
                Press::Grab => {
                    self.scroll = Some(bar);
                    return Outcome::None;
                }
                Press::Up => {
                    self.first = view.shifted(-(self.visible as i64));
                    return Outcome::None;
                }
                Press::Down => {
                    self.first = view.shifted(self.visible as i64);
                    return Outcome::None;
                }
            }
            self.scroll = Some(bar);
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
    /// The mouse went up: the rocker springs back, the scroll bar's knob is
    /// let go.
    pub fn release(&mut self, now: Instant) {
        if self.rocker.held() {
            self.rocker.release(now);
        }
        if let Some(bar) = &mut self.scroll {
            bar.release();
        }
    }
    /// The pointer moved to `point`: a held knob drags the window. Call it on
    /// every pointer move; it does nothing when no knob is held.
    pub fn drag(&mut self, point: Point) {
        if let Some(bar) = self.scroll {
            let view = self.view();
            if let Some(first) = bar.drag(point, view) {
                self.first = first;
            }
        }
    }
    /// True while the scroll bar's knob is held.
    #[cfg(test)]
    pub fn dragging(&self) -> bool {
        self.scroll.is_some_and(|bar| bar.dragging())
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
    /// A paged list moves its selection; a scrolling one slides its window
    /// and answers `None` (the selection did not change).
    pub fn wheel(&mut self, lines: i32) -> Outcome {
        if !self.enabled || self.rows.is_empty() || lines == 0 {
            return Outcome::None;
        }
        if self.scroll.is_some() {
            self.scroll_by(-lines.saturating_mul(self.wheel_rows as i32));
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
            let index = self.first_row() + slot;
            let Some(row) = self.rows.get(index) else {
                continue;
            };
            if let Some([r, g, b]) = row.fill {
                // The well filled behind the text: rows 1 to 13 of the bar, the
                // dark and near-black ones, inside the light edge above and
                // the bevel below.
                canvas.rect(
                    (x + FILL_INSET, y + 1, w - 2 * FILL_INSET, BAR_HEIGHT - 4),
                    [r, g, b, 255],
                );
            }
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
            let tint = if row.dim { Some(ghost(118)) } else { row.tint };
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
                        icon.draw(
                            canvas,
                            (origin + column.x + ix, y + (BAR_HEIGHT - ih) / 2),
                            row.dim.then(|| ghost(118)),
                        );
                    }
                    Cell::Text(text) => {
                        let text = fit(font, text, column.width);
                        let tw = text_width(font, &text);
                        let tx = match column.align {
                            Align::Left => 0,
                            Align::Centre => (column.width - tw) / 2,
                            Align::Right => column.width - tw,
                        };
                        ui_text::text(
                            canvas,
                            kit,
                            font,
                            &text,
                            (origin + column.x + tx, y + 1),
                            None,
                            tint,
                        );
                    }
                }
            }
        }
        if let Some(pager) = self.pager {
            self.draw_pager(canvas, kit, pager);
        }
        if let Some(bar) = &self.scroll {
            bar.draw(canvas, kit, self.view());
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
        let text = ui_text::text;
        text(
            canvas,
            kit,
            if can_prev { bright } else { dim },
            "PREV",
            pager.prev,
            None,
            None,
        );
        text(
            canvas,
            kit,
            if can_next { bright } else { dim },
            "NEXT",
            pager.next,
            None,
            None,
        );
        let rocker = kit.sprite(&self.rocker.sprite());
        canvas.blit(rocker, pager.rocker, 0, rocker.width, 1.0);
        text(canvas, kit, bright, "PAGE", pager.page_label, None, None);
        let page_box = kit.sprite("PAGEBOX");
        canvas.blit(page_box, pager.counter_box, 0, page_box.width, 1.0);
        // EF0: the string starts at the counter's x + 15 minus the width of
        // its first part, so the page number is right aligned; the counter's
        // origin is one pixel inside the box and its text 3 pixels down.
        let font = kit.sprite("SMLFONT");
        let (left, right) = self.counter_parts();
        let x = pager.counter_box.0 + 1 + 15 - text_width(font, &left);
        text(
            canvas,
            kit,
            font,
            &format!("{left}{right}"),
            (x, pager.counter_box.1 + 3),
            None,
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
    /// The rows, and the scroll bar of a scrolling list.
    fn hit(&self, point: Point) -> bool {
        self.enabled
            && (inside(self.bounds(), point) || self.scroll.is_some_and(|bar| bar.hit(point)))
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
                width: 12,
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
        // The crown (12 wide) fills its 12 pixel column from x + 20, in the
        // icons' one colour; the text (3 characters of 6 pixels) ends at the
        // column's right edge.
        let crown = at(&pixels, 48 + 20 + 5, 185 + 3 + 5);
        for c in 0..3 {
            assert!(
                crown[c].abs_diff(crate::widgets::icons::COLOUR[c]) < 10,
                "{crown:?}"
            );
        }
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

    // ---- the scrolling mode ----

    /// A five row scrolling list (the lobby's shape) with `n` rows and its
    /// bar at (350, 185): 34 wide, 89 high.
    fn scrolling(n: usize) -> List {
        let mut l = List::new((48, 185), 286, 5).with_scroll_bar((350, 185));
        l.set_rows(rows(n));
        l
    }
    fn shown(l: &List) -> Vec<String> {
        (l.first_row()..(l.first_row() + 5).min(l.rows().len()))
            .map(|i| match &l.rows()[i].cells[0] {
                Cell::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn a_scrolling_list_has_no_pager_and_shows_a_window() {
        let l = scrolling(12);
        assert_eq!(l.first_row(), 0);
        assert_eq!(l.visible_rows(), 5);
        assert!(l.pager.is_none());
        assert_eq!(
            shown(&l),
            ["game 0", "game 1", "game 2", "game 3", "game 4"]
        );
        // A pager given first is dropped.
        let l = List::new((0, 0), 100, 5)
            .with_pager(Pager::NEWNET)
            .with_scroll_bar((110, 0));
        assert!(l.pager.is_none());
    }

    #[test]
    fn the_wheel_slides_the_window_and_leaves_the_selection() {
        let mut l = scrolling(12);
        l.select(1);
        assert_eq!(l.wheel(-1), Outcome::None, "the selection did not change");
        assert_eq!(l.first_row(), 1);
        assert_eq!(l.wheel(-3), Outcome::None);
        assert_eq!(l.first_row(), 4);
        assert_eq!(l.selected(), Some(1), "scrolled out of sight, still chosen");
        assert_eq!(shown(&l)[0], "game 4");
        // Ends: 12 rows, 5 show, the last window starts at 7.
        l.wheel(-100);
        assert_eq!(l.first_row(), 7);
        l.wheel(100);
        assert_eq!(l.first_row(), 0);
        // A wheel that moves three rows a notch.
        let mut l = List::new((48, 185), 286, 5)
            .with_scroll_bar((350, 185))
            .with_wheel_rows(3);
        l.set_rows(rows(20));
        l.wheel(-2);
        assert_eq!(l.first_row(), 6);
    }

    #[test]
    fn the_keys_move_the_selection_and_the_window_follows() {
        let mut l = scrolling(12);
        for _ in 0..7 {
            l.key("ArrowDown");
        }
        assert_eq!(l.selected(), Some(6));
        assert_eq!(l.first_row(), 2, "row 6 is the window's last");
        l.key("ArrowUp");
        assert_eq!(l.first_row(), 2, "still in view: no move");
        l.key("End");
        assert_eq!((l.selected(), l.first_row()), (Some(11), 7));
        l.key("Home");
        assert_eq!((l.selected(), l.first_row()), (Some(0), 0));
        l.key("PageDown");
        assert_eq!((l.selected(), l.first_row()), (Some(5), 1));
        // After the wheel took the selection out of sight, a key brings the
        // window back to it.
        l.wheel(-100);
        assert_eq!(l.first_row(), 7);
        l.key("ArrowUp");
        assert_eq!((l.selected(), l.first_row()), (Some(4), 4));
    }

    #[test]
    fn selecting_shows_the_row_and_the_window_stays_put_otherwise() {
        let mut l = scrolling(30);
        assert_eq!(l.select(20), Outcome::Changed);
        assert_eq!(l.first_row(), 16);
        assert_eq!(l.select(18), Outcome::Changed);
        assert_eq!(l.first_row(), 16, "already in view");
        l.select(0);
        assert_eq!(l.first_row(), 0);
        l.scroll_to(1000);
        assert_eq!(l.first_row(), 25, "kept in range");
        l.scroll_by(-3);
        assert_eq!(l.first_row(), 22);
        // A paged list ignores the window calls.
        let mut p = list(12);
        p.scroll_to(3);
        p.scroll_by(2);
        assert_eq!(p.first_row(), 0);
    }

    #[test]
    fn new_rows_keep_the_window_where_it_is() {
        let mut l = scrolling(30);
        l.scroll_to(10);
        l.select(12);
        l.set_rows(rows(30));
        assert_eq!((l.first_row(), l.selected()), (10, Some(12)));
        // The list shrinks under the window.
        l.set_rows(rows(8));
        assert_eq!(l.first_row(), 3);
        l.set_rows(rows(2));
        assert_eq!(l.first_row(), 0);
        l.set_rows(Vec::new());
        assert_eq!(l.first_row(), 0);
    }

    #[test]
    fn a_click_on_a_row_picks_the_row_in_the_window() {
        let mut l = scrolling(12);
        l.wheel(-4);
        assert_eq!(l.press(row_point(1), start()), Outcome::Changed);
        assert_eq!(l.selected(), Some(5));
        assert_eq!(l.row_at(row_point(4)), Some(8));
        // A click on the bar is not a row.
        let before = l.selected();
        l.press((360, 200), start());
        assert_eq!(l.selected(), before);
    }

    #[test]
    fn the_track_pages_the_window_and_the_knob_drags_it() {
        let mut l = scrolling(30);
        // The knob rests at the top: a click below it pages down a window.
        assert_eq!(l.press((360, 185 + 80), start()), Outcome::None);
        assert_eq!(l.first_row(), 5);
        l.press((360, 185 + 80), start());
        assert_eq!(l.first_row(), 10);
        // Above the knob pages back.
        l.press((360, 185 + 1), start());
        assert_eq!(l.first_row(), 5);
        assert!(!l.dragging());
        // Grab the knob (25 rows over 48 pixels of travel) and drag to the
        // bottom and back.
        l.scroll_to(0);
        l.press((360, 185 + 6 + 10), start());
        assert!(l.dragging());
        l.drag((360, 400));
        assert_eq!(l.first_row(), 25);
        l.drag((500, 185 + 6 + 10 + 24));
        assert!((12..=13).contains(&l.first_row()), "{}", l.first_row());
        l.release(start());
        assert!(!l.dragging());
        let held = l.first_row();
        l.drag((360, 190));
        assert_eq!(l.first_row(), held, "let go: it stays");
        // None of it touched the selection.
        assert_eq!(l.selected(), None);
        // The bar counts as the list for the screen's hit test.
        assert!(l.hit((360, 200)) && l.hit((100, 200)) && !l.hit((400, 200)));
        l.set_enabled(false);
        assert!(!l.hit((360, 200)));
        assert_eq!(l.press((360, 185 + 80), start()), Outcome::None);
        assert_eq!(l.first_row(), held);
    }

    #[test]
    fn a_list_that_fits_has_no_knob_and_the_track_does_not_react() {
        let mut l = scrolling(5);
        assert_eq!(l.press((360, 185 + 80), start()), Outcome::None);
        assert_eq!(l.first_row(), 0);
        assert!(!l.dragging());
        let kit = kit();
        let mut pixels = blank();
        l.draw(&mut Canvas(&mut pixels), &kit, false);
        assert_eq!(at(&pixels, 351, 200), tone_of(&kit, "SLIDEMID"));
        for y in 185..185 + 89 {
            assert_ne!(at(&pixels, 360, y), tone_of(&kit, "SLIDERV"), "y {y}");
        }
    }

    #[test]
    fn the_scrolling_list_draws_the_window_and_its_bar() {
        let kit = kit();
        let mut l = scrolling(30);
        l.wheel(-10);
        let mut pixels = blank();
        l.draw(&mut Canvas(&mut pixels), &kit, false);
        // The knob a third of the way along: 10 of 25.
        let knob = l.scroll.unwrap().knob(l.view()).unwrap();
        assert_eq!(knob, (356, 185 + 6 + 19, 26, 30));
        assert_eq!(
            at(&pixels, knob.0 + 1, knob.1 + 1),
            tone_of(&kit, "SLIDERV")
        );
        // The track is where the pager would have been: no rocker, no PAGE box.
        assert_eq!(at(&pixels, 351, 186), tone_of(&kit, "SLIDETOP"));
        // The row bars are the list's own width.
        assert_eq!(at(&pixels, 49, 185 + 2), at(&pixels, 49, 185 + 18 * 3 + 2));
    }
}
