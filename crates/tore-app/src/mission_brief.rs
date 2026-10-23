//! Shared, read-only Quick Mission sentences for the in-flight pages.
use crate::{
    menu::{Canvas, Sprite, text_width},
    widgets::scroll_bar::{Press, ScrollBar, View},
};
use std::collections::BTreeMap;
use tore_session::settings::{Mode, Respawn, Store as Settings};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sentence {
    pub x: i32,
    pub y: i32,
    pub wrap: bool,
    pub parts: Vec<(String, Option<usize>)>,
}
impl Sentence {
    pub fn words(&self, font: &Sprite, right: i32) -> Vec<(String, Option<usize>, i32, i32)> {
        let (mut x, mut y) = (self.x, self.y);
        let mut words = Vec::new();
        for (text, id) in &self.parts {
            for word in text.split_whitespace() {
                let width = text_width(font, word);
                if x + width > right && x > self.x {
                    x = self.x;
                    y += 14;
                }
                words.push((word.into(), *id, x, y));
                x += width + text_width(font, " ") + if id.is_some() { 2 } else { 0 };
            }
        }
        words
    }
    pub fn last_y(&self, font: &Sprite, right: i32) -> i32 {
        self.words(font, right).last().map_or(self.y, |w| w.3)
    }
}

pub fn rules(settings: &Settings) -> Vec<String> {
    let on = |value| if value { "On" } else { "Off" };
    let limit = |value: Option<u32>| value.map_or_else(|| "Unlimited".into(), |v| v.to_string());
    vec![
        format!(
            "Mode: {}",
            match settings.mode() {
                Mode::Coop => "Co-op",
                Mode::Pvp => "PvP",
            }
        ),
        format!(
            "Respawn: {}",
            match settings.respawn() {
                Respawn::None => "None",
                Respawn::AiSlot => "Take AI slot",
                Respawn::Revive => "Revive",
            }
        ),
        format!("Lives (revivals): {}", limit(settings.lives())),
        format!("Revive delay: {} seconds", settings.revive_delay_seconds()),
        format!(
            "Time limit: {}",
            settings
                .time_limit_seconds()
                .map_or_else(|| "None".into(), |v| format!("{v} seconds"))
        ),
        format!("Kill limit: {}", limit(settings.kill_limit())),
        format!("Friendly fire: {}", on(settings.friendly_fire())),
        format!("AI aircraft: {}", on(settings.ai())),
        format!("Minimum players: {}", settings.min_players()),
    ]
}

/// Scroll state is independent of the map and of the loadout underneath.
pub struct Brief {
    rows: Vec<Sentence>,
    first: usize,
    bar: ScrollBar,
    close_pressed: bool,
}
const VISIBLE: usize = 24;
const CLOSE: (i32, i32, i32, i32) = (497, 440, 110, 24);
impl Default for Brief {
    fn default() -> Self {
        Self::new(Vec::new(), None, &crate::menu::flat_font([232, 233, 230]))
    }
}
impl Brief {
    pub fn new(mut sentences: Vec<Sentence>, settings: Option<&Settings>, font: &Sprite) -> Self {
        // Read each side's situation down its column, then mission parameters.
        sentences.sort_by_key(|s| (s.y >= 300, s.x >= 320, s.y));
        let mut rows = Vec::new();
        let mut section = None;
        for sentence in sentences {
            let next = (sentence.y >= 300, sentence.x >= 320);
            if section != Some(next) {
                let title = match next {
                    (false, false) => "FRIENDLY SITUATION",
                    (false, true) => "ENEMY SITUATION",
                    (true, false) => "MISSION SETUP",
                    (true, true) => "GROUND TARGET AND ENEMY START",
                };
                rows.push(Self::plain(title));
                section = Some(next);
            }
            // Keep spaces/punctuation exactly as the creator does. Wrap at
            // word boundaries, retaining each variable's filled-field style.
            let mut row = Self::plain("");
            row.parts.clear();
            let mut width = 0;
            let count = sentence.parts.len();
            for (index, (text, id)) in sentence.parts.into_iter().enumerate() {
                let text = if sentence.wrap && index + 1 < count {
                    format!("{} ", text.trim())
                } else {
                    text
                };
                for part in text.split_inclusive(' ') {
                    let joins = row.parts.last().is_some_and(|p| p.1 == id);
                    let mut w = text_width(font, part) + if id.is_some() && !joins { 4 } else { 0 };
                    if width + w > 536 && width > 0 {
                        rows.push(row);
                        row = Self::plain("");
                        row.parts.clear();
                        width = 0;
                        w = text_width(font, part) + if id.is_some() { 4 } else { 0 };
                    }
                    if let Some(last) = row.parts.last_mut().filter(|p| p.1 == id) {
                        last.0.push_str(part);
                    } else {
                        row.parts.push((part.into(), id));
                    }
                    width += w;
                }
            }
            rows.push(row);
        }
        if let Some(settings) = settings {
            rows.push(Self::plain("MULTIPLAYER RULES"));
            rows.extend(rules(settings).iter().map(|s| Self::plain(s)));
        }
        if rows.is_empty() {
            rows.push(Self::plain(
                "Free flight. No Quick Mission setup is available.",
            ));
        }
        Self {
            rows,
            first: 0,
            bar: ScrollBar::new((584, 62), 366),
            close_pressed: false,
        }
    }
    fn plain(text: &str) -> Sentence {
        Sentence {
            x: 28,
            y: 0,
            wrap: false,
            parts: vec![(text.into(), None)],
        }
    }
    fn view(&self) -> View {
        View {
            total: self.rows.len(),
            visible: VISIBLE,
            first: self.first,
        }
    }
    pub fn key(&mut self, key: &str) -> bool {
        match key {
            "Escape" => return true,
            "ArrowUp" => self.first = self.view().shifted(-1),
            "ArrowDown" => self.first = self.view().shifted(1),
            "PageUp" => self.first = self.view().shifted(-(VISIBLE as i64)),
            "PageDown" => self.first = self.view().shifted(VISIBLE as i64),
            "Home" => self.first = 0,
            "End" => self.first = self.view().max_first(),
            _ => {}
        }
        false
    }
    pub fn moved(&mut self, point: Option<(f64, f64)>) {
        if let Some((x, y)) = point
            && let Some(first) = self.bar.drag((x as i32, y as i32), self.view())
        {
            self.first = first;
        }
    }
    pub fn cancel_press(&mut self) {
        self.bar.release();
        self.close_pressed = false;
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> bool {
        let hit = point.is_some_and(|(x, y)| crate::widgets::inside(CLOSE, (x as i32, y as i32)));
        if down {
            self.close_pressed = hit;
            if let Some((x, y)) = point {
                match self.bar.press((x as i32, y as i32), self.view()) {
                    Press::Up => self.first = self.view().shifted(-(VISIBLE as i64)),
                    Press::Down => self.first = self.view().shifted(VISIBLE as i64),
                    _ => {}
                }
            }
            false
        } else {
            let close = self.close_pressed && hit;
            self.cancel_press();
            close
        }
    }
    pub fn draw(&self, pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>) {
        let font = &sprites["QUICKFONT"];
        let mut c = Canvas(pixels);
        c.rect((0, 0, 640, 480), [72, 72, 72, 255]);
        c.text(font, "MISSION BRIEF", 28, 22, None);
        c.text(font, "View only", 493, 22, None);
        c.rect((18, 52, 554, 378), [58, 63, 65, 255]);
        for (i, row) in self.rows.iter().skip(self.first).take(VISIBLE).enumerate() {
            let (mut x, y) = (28, 64 + i as i32 * 15);
            for (text, id) in &row.parts {
                let width = text_width(font, text);
                if id.is_some() {
                    x += 2;
                    let rect = (
                        x - 1,
                        y - 1,
                        width + 2,
                        font.glyphs.iter().map(|g| g[2]).max().unwrap_or(9) as i32 + 2,
                    );
                    c.rect(rect, [101, 107, 109, 255]);
                    crate::quick_mission::bevel(&mut c, rect, false);
                }
                c.text(font, text, x, y, None);
                x += width + if id.is_some() { 2 } else { 0 };
            }
        }
        self.bar.draw_art(&mut c, self.view(), |name| {
            sprites.get(&format!("{name}.PIC"))
        });
        c.text(font, "Up/Down: scroll   PgUp/PgDn: page", 28, 447, None);
        c.action_button(
            sprites,
            "Close",
            (CLOSE.0, CLOSE.1, CLOSE.2),
            true,
            self.close_pressed,
        );
    }
}

impl crate::App {
    pub(crate) fn prepare_mission_brief(&mut self) {
        let spec = self.net.as_ref().and_then(|s| s.client.spec());
        let settings = self
            .net
            .as_ref()
            .and_then(|s| s.client.lobby())
            .map(|lobby| {
                let mut settings = Settings::defaults(Mode::Coop);
                if let Err(error) = settings.apply(&lobby.settings) {
                    log::warn!("Mission brief settings: {error}");
                }
                settings
            });
        let font = crate::menu::flat_font([232, 233, 230]);
        let rows = spec.map(|spec| self.quick.brief_sentences(spec, settings.as_ref(), &font));
        let rows = match rows {
            Some(Ok(rows)) => rows,
            Some(Err(error)) => vec![Brief::plain(&format!("Mission setup unavailable: {error}"))],
            None => self.mission_brief_sentences.clone().unwrap_or_default(),
        };
        self.flight_ui.map.brief = Some(Brief::new(rows, settings.as_ref(), &font));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_show_limits_switches_and_minimum_players() {
        use tore_session::settings::number;
        let mut settings = Settings::defaults(Mode::Pvp);
        settings
            .apply(&[
                (number::MIN_PLAYERS, 4),
                (number::AI, 0),
                (number::FRIENDLY_FIRE, 0),
                (number::TIME_LIMIT, 600),
                (number::KILL_LIMIT, 10),
                (number::LIVES, 3),
            ])
            .unwrap();
        let text = rules(&settings).join("\n");
        for line in [
            "Mode: PvP",
            "Time limit: 600 seconds",
            "Kill limit: 10",
            "Lives (revivals): 3",
            "AI aircraft: Off",
            "Friendly fire: Off",
            "Minimum players: 4",
        ] {
            assert!(text.contains(line), "{text}: missing {line}");
        }
        let defaults = rules(&Settings::defaults(Mode::Coop));
        assert!(defaults.iter().any(|s| s == "Mode: Co-op"));
        assert!(defaults.iter().any(|s| s.starts_with("Respawn: ")));
    }
    #[test]
    fn scrolling_keys_track_drag_and_close_are_bounded() {
        let font = crate::menu::flat_font([232, 233, 230]);
        let mut brief = Brief::new(
            (0..70)
                .map(|i| Brief::plain(&format!("line {i}")))
                .collect(),
            None,
            &font,
        );
        brief.key("PageDown");
        assert_eq!(brief.first, VISIBLE);
        brief.key("End");
        let end = brief.view().max_first();
        assert_eq!(brief.first, end);
        brief.key("ArrowDown");
        assert_eq!(brief.first, end);
        brief.key("Home");
        assert_eq!(brief.first, 0);
        brief.pointer(Some((600., 420.)), true);
        brief.pointer(Some((600., 420.)), false);
        assert_eq!(brief.first, VISIBLE);
        let knob = brief.bar.knob(brief.view()).unwrap();
        brief.pointer(Some(((knob.0 + 2) as f64, (knob.1 + 2) as f64)), true);
        brief.moved(Some((600., 1000.)));
        assert_eq!(brief.first, end);
        assert!(!brief.pointer(Some((600., 1000.)), false));
        assert!(!brief.pointer(Some((530., 450.)), true));
        assert!(brief.pointer(Some((530., 450.)), false));
        assert!(brief.key("Escape"));
    }
}
