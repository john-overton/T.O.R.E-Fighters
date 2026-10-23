//! Searchable viewport for the in-flight respawn pages. Presentation is opinionated.
use crate::{
    hud::Paint,
    menu::{Canvas, Sprite},
    widgets::scroll_bar::{Press, ScrollBar, View},
};
use std::collections::BTreeMap;
use tore_formats::font::Font;

const VISIBLE: usize = 16;
const SEARCH: (i32, i32, i32, i32) = (76, 49, 450, 24);
const TOP: i32 = 80;
const HEIGHT: i32 = 19;

pub(super) struct Entry {
    pub label: String,
    pub search: String,
    pub group: &'static str,
}
struct Row {
    label: String,
    choice: Option<usize>,
}
pub(super) struct List {
    entries: Vec<Entry>,
    rows: Vec<Row>,
    pub query: String,
    pub searching: bool,
    focus: usize,
    first: usize,
    pressed: Option<usize>,
    bar: ScrollBar,
}
impl List {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut list = Self {
            entries,
            rows: vec![],
            query: String::new(),
            searching: false,
            focus: 0,
            first: 0,
            pressed: None,
            bar: ScrollBar::new((530, TOP), VISIBLE as i32 * HEIGHT),
        };
        list.filter();
        list
    }
    fn filter(&mut self) {
        self.rows.clear();
        let query = self.query.to_lowercase();
        let mut group = "";
        for (index, entry) in self.entries.iter().enumerate() {
            if !entry.search.to_lowercase().contains(&query) {
                continue;
            }
            if !entry.group.is_empty() && group != entry.group {
                group = entry.group;
                self.rows.push(Row {
                    label: group.into(),
                    choice: None,
                });
            }
            self.rows.push(Row {
                label: entry.label.clone(),
                choice: Some(index),
            });
        }
        self.focus = self
            .rows
            .iter()
            .position(|r| r.choice.is_some())
            .unwrap_or(0);
        self.first = 0;
        self.pressed = None;
        self.bar.release();
    }
    pub fn clear(&mut self) {
        self.query.clear();
        self.filter();
    }
    pub fn text(&mut self, text: &str) {
        if self.searching {
            for c in text.chars().filter(|c| !c.is_control()) {
                if self.query.chars().count() < 64 {
                    self.query.push(c);
                }
            }
            self.filter();
        }
    }
    fn view(&self) -> View {
        View {
            total: self.rows.len(),
            visible: VISIBLE,
            first: self.first,
        }
    }
    fn reveal(&mut self) {
        if self.focus < self.first {
            self.first = self.focus;
        }
        if self.focus >= self.first + VISIBLE {
            self.first = self.focus + 1 - VISIBLE;
        }
        self.first = self.first.min(self.view().max_first());
    }
    pub fn key(&mut self, key: &str) -> Option<usize> {
        match key {
            "Tab" => self.searching = !self.searching,
            "Backspace" if self.searching => {
                self.query.pop();
                self.filter();
            }
            "ArrowDown" | "ArrowUp" | "Home" | "End" | "PageDown" | "PageUp" => {
                let choices: Vec<_> = self
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| r.choice.map(|_| i))
                    .collect();
                if !choices.is_empty() {
                    let current = choices.iter().position(|i| *i == self.focus).unwrap_or(0);
                    let last = choices.len() - 1;
                    let next = match key {
                        "Home" => 0,
                        "End" => last,
                        "ArrowDown" => (current + 1).min(last),
                        "ArrowUp" => current.saturating_sub(1),
                        "PageDown" => (current + VISIBLE).min(last),
                        _ => current.saturating_sub(VISIBLE),
                    };
                    self.focus = choices[next];
                    self.reveal();
                }
            }
            "Enter" => return self.rows.get(self.focus).and_then(|r| r.choice),
            "Space" if !self.searching => return self.rows.get(self.focus).and_then(|r| r.choice),
            _ => {}
        }
        None
    }
    pub fn wheel(&mut self, notches: i32) {
        self.first = self.view().shifted(-i64::from(notches) * 3);
        self.pressed = None;
    }
    pub fn moved(&mut self, point: Option<(f64, f64)>) {
        if let Some((x, y)) = point {
            if let Some(first) = self.bar.drag((x as i32, y as i32), self.view()) {
                self.first = first;
            }
        } else {
            self.bar.release();
            self.pressed = None;
        }
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> Option<usize> {
        let point = point.map(|(x, y)| (x as i32, y as i32));
        if down {
            self.pressed = None;
            if let Some(p) = point {
                self.searching = inside(SEARCH, p);
                match self.bar.press(p, self.view()) {
                    Press::Up => self.first = self.view().shifted(-(VISIBLE as i64)),
                    Press::Down => self.first = self.view().shifted(VISIBLE as i64),
                    Press::Grab => {}
                    Press::Miss => self.pressed = self.hit(p),
                }
            }
            None
        } else {
            self.bar.release();
            let pressed = self.pressed.take();
            if let Some(hit) = point
                .and_then(|p| self.hit(p))
                .filter(|h| Some(*h) == pressed)
            {
                self.focus = hit;
                return self.rows[hit].choice;
            }
            None
        }
    }
    fn hit(&self, p: (i32, i32)) -> Option<usize> {
        if !inside((76, TOP, 450, VISIBLE as i32 * HEIGHT), p) {
            return None;
        }
        let index = self.first + ((p.1 - TOP) / HEIGHT) as usize;
        self.rows
            .get(index)
            .filter(|r| r.choice.is_some())
            .map(|_| index)
    }
    pub fn draw(
        &self,
        pixels: &mut [u8],
        font: &Font,
        sprites: &BTreeMap<String, Sprite>,
        title: &str,
    ) {
        Canvas(pixels).rect((70, 28, 500, 362), [160, 172, 186, 255]);
        Paint {
            pixels,
            clip: (76, 28, 488, 19),
            color: [20, 39, 65, 255],
        }
        .text(font, title, 80, 34);
        Canvas(pixels).rect(
            SEARCH,
            if self.searching {
                [24, 34, 45, 255]
            } else {
                [201, 210, 222, 255]
            },
        );
        let search = format!(
            "Search: {}{}",
            self.query,
            if self.searching {
                "_"
            } else {
                "  (Tab to type)"
            }
        );
        Paint {
            pixels,
            clip: SEARCH,
            color: if self.searching {
                [247, 250, 255, 255]
            } else {
                [20, 39, 65, 255]
            },
        }
        .text(font, &search, SEARCH.0 + 5, SEARCH.1 + 7);
        for (line, row) in self.rows.iter().skip(self.first).take(VISIBLE).enumerate() {
            let rect = (76, TOP + line as i32 * HEIGHT, 450, HEIGHT);
            let selected = self.focus == self.first + line && row.choice.is_some();
            Canvas(pixels).rect(
                rect,
                if selected {
                    [45, 75, 113, 255]
                } else if row.choice.is_none() {
                    [160, 172, 186, 255]
                } else {
                    [201, 210, 222, 255]
                },
            );
            Paint {
                pixels,
                clip: rect,
                color: if selected {
                    [247, 250, 255, 255]
                } else {
                    [20, 39, 65, 255]
                },
            }
            .text(font, &row.label, rect.0 + 5, rect.1 + 6);
        }
        if self.rows.is_empty() {
            Paint {
                pixels,
                clip: (76, TOP, 450, HEIGHT),
                color: [20, 39, 65, 255],
            }
            .text(font, "No matches", 81, TOP + 6);
        }
        self.bar.draw_art(&mut Canvas(pixels), self.view(), |name| {
            sprites.get(&format!("{name}.PIC"))
        });
    }
}
fn inside((x, y, w, h): (i32, i32, i32, i32), p: (i32, i32)) -> bool {
    (x..x + w).contains(&p.0) && (y..y + h).contains(&p.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn list() -> List {
        List::new(
            (0..100)
                .map(|i| Entry {
                    label: format!("AIM-120 x{i}"),
                    search: format!("AMRAAM AIM-120 {i}"),
                    group: "Air-to-air",
                })
                .collect(),
        )
    }
    #[test]
    fn search_designation_partial_case_empty_and_backspace() {
        let mut list = list();
        list.key("Tab");
        list.text("aMrAa");
        assert_eq!(list.rows.len(), 101);
        list.clear();
        list.text("120");
        assert_eq!(list.rows.len(), 101);
        list.text("xyz");
        assert!(list.rows.is_empty());
        assert_eq!(list.key("Enter"), None);
        for _ in 0..3 {
            list.key("Backspace");
        }
        assert_eq!(list.key("Enter"), Some(0));
        list.clear();
        list.text("not present");
        list.key("ArrowDown");
        assert_eq!(list.key("Enter"), None);
    }
    #[test]
    fn keyboard_controller_keys_reveal_last_result_and_skip_headers() {
        let mut list = list();
        assert_eq!(list.key("Enter"), Some(0));
        for _ in 0..100 {
            list.key("ArrowDown");
        }
        assert_eq!(list.key("Enter"), Some(99));
        assert_eq!(list.first, list.view().max_first());
        list.key("Home");
        assert_eq!(list.key("Enter"), Some(0));
        list.key("PageDown");
        assert_eq!(list.key("Enter"), Some(16));
        list.key("PageUp");
        assert_eq!(list.key("Enter"), Some(0));
        list.key("End");
        assert_eq!(list.key("Enter"), Some(99));
    }
    #[test]
    fn wheel_drag_track_and_click_reach_last_result() {
        let mut list = list();
        list.wheel(-100);
        assert_eq!(list.first, list.view().max_first());
        list.wheel(100);
        assert_eq!(list.first, 0);
        let knob = list.bar.knob(list.view()).unwrap();
        list.pointer(Some(((knob.0 + 5) as f64, (knob.1 + 5) as f64)), true);
        list.moved(Some((540., 1000.)));
        assert_eq!(list.first, list.view().max_first());
        assert_eq!(list.pointer(Some((540., 1000.)), false), None);
        let last = Some((100., (TOP + 15 * HEIGHT + 5) as f64));
        list.pointer(last, true);
        assert_eq!(list.pointer(last, false), Some(99));
        list.pointer(Some((540., 81.)), true);
        assert_eq!(list.first, list.view().max_first() - VISIBLE);
        list.moved(None);
        assert!(!list.bar.dragging());
    }
}
