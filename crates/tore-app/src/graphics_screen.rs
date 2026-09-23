//! The Graphics options screen, opened from main menu Pref > Graphics... It
//! edits a draft of `graphics::Options`; Apply hands the draft to the renderer
//! and saves it, Cancel or Escape closes without applying, and Defaults puts
//! the recommended values back into the draft. Layout, wording and behaviour
//! are an opinionated agent design (2026-09-22) in the style of the input
//! configuration screen, drawn with the imported raster font through that
//! screen's shared helpers.
use crate::controls_editor::{
    self as chrome, BUTTON, Editor as Chrome, FOCUS, GOOD, HEADER, INK, MUTED, PALE, PANEL, Rect,
    ResultAction, TITLE, WHITE, fit, inside, text_width,
};
use crate::graphics::{AntiAliasing, Options, RENDER_SCALES, SpottingAid};
use crate::menu::Canvas;
use std::path::Path;
use tore_formats::font::Font;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    AntiAliasing,
    RenderScale,
    SpottingAid,
    TerrainFiltering,
}
const ROWS: [Row; 4] = [
    Row::AntiAliasing,
    Row::RenderScale,
    Row::SpottingAid,
    Row::TerrainFiltering,
];
impl Row {
    fn label(self) -> &'static str {
        match self {
            Row::AntiAliasing => "Anti-aliasing",
            Row::RenderScale => "Render scale",
            Row::SpottingAid => "Spotting aid",
            Row::TerrainFiltering => "Terrain filtering",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Row::AntiAliasing => {
                "Smooths the jagged, stair-stepped edges of aircraft, terrain and the horizon."
            }
            Row::RenderScale => {
                "Draws the 3D view with more or fewer pixels than the window: higher is sharper, lower is faster."
            }
            Row::SpottingAid => {
                "Outlines distant aircraft so they are easier to see against the sky and ground."
            }
            Row::TerrainFiltering => "Reduces shimmer and crawling patterns on distant ground.",
        }
    }
    fn choices(self) -> Vec<String> {
        match self {
            Row::AntiAliasing => AntiAliasing::ALL.map(|a| a.label().to_owned()).to_vec(),
            Row::RenderScale => RENDER_SCALES.map(|s| format!("{s}%")).to_vec(),
            Row::SpottingAid => SpottingAid::ALL.map(|a| a.label().to_owned()).to_vec(),
            Row::TerrainFiltering => vec!["Off".into(), "On".into()],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    Row(usize),
    Footer(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    Row(usize),
    /// Row and choice index.
    Choice(usize, usize),
    Footer(usize),
}

const FOOTER: [&str; 3] = ["Apply", "Defaults", "Cancel"];
const PANEL_RECT: Rect = (6, 24, 628, 184);
const ROW_TOP: i32 = 48;
const ROW_STEP: i32 = 30;
const CHOICE_X: i32 = 250;
const CHOICE_W: i32 = 70;
const CHOICE_GAP: i32 = 6;
const ABOUT_RECT: Rect = (6, 214, 628, 96);
const OUTLINE: [u8; 4] = [110, 130, 156, 255];

pub struct Editor {
    /// The values shown and edited; they take effect only on Apply.
    pub draft: Options,
    /// The values in effect when the screen opened or last applied.
    applied: Options,
    /// Whether the adapter can draw each `AntiAliasing::ALL` level exactly.
    supported: [bool; 4],
    pub message: String,
    /// Shown at the top right: where the screen was opened from.
    context: &'static str,
    focus: Focus,
    /// The row the About panel describes; the last row that had focus.
    row: usize,
    pressed: Option<Hit>,
}

impl Editor {
    pub fn new(applied: Options, supported: [bool; 4], context: &'static str) -> Self {
        Self {
            draft: applied,
            applied,
            supported,
            message: "Choose with the arrows or the mouse. Apply uses and saves the settings."
                .into(),
            context,
            focus: Focus::Row(0),
            row: 0,
            pressed: None,
        }
    }
    /// True while the draft differs from the settings in effect.
    pub fn dirty(&self) -> bool {
        self.draft != self.applied
    }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
    }

    // ---- model ----------------------------------------------------------

    fn choice(&self, row: usize) -> usize {
        let d = &self.draft;
        match ROWS[row] {
            Row::AntiAliasing => AntiAliasing::ALL
                .iter()
                .position(|a| *a == d.anti_aliasing)
                .unwrap_or(0),
            Row::RenderScale => RENDER_SCALES
                .iter()
                .position(|s| *s == d.render_scale)
                .unwrap_or(1),
            Row::SpottingAid => SpottingAid::ALL
                .iter()
                .position(|a| *a == d.spotting_aid)
                .unwrap_or(0),
            Row::TerrainFiltering => usize::from(d.terrain_filtering),
        }
    }
    fn set_choice(&mut self, row: usize, i: usize) {
        let d = &mut self.draft;
        match ROWS[row] {
            Row::AntiAliasing => d.anti_aliasing = AntiAliasing::ALL[i],
            Row::RenderScale => d.render_scale = RENDER_SCALES[i],
            Row::SpottingAid => d.spotting_aid = SpottingAid::ALL[i],
            Row::TerrainFiltering => d.terrain_filtering = i == 1,
        }
    }
    fn available(&self, row: usize, i: usize) -> bool {
        ROWS[row] != Row::AntiAliasing || self.supported[i]
    }
    fn unsupported(&self) -> Vec<&'static str> {
        AntiAliasing::ALL
            .iter()
            .zip(self.supported)
            .filter(|(_, s)| !s)
            .map(|(a, _)| a.label())
            .collect()
    }
    /// A one-line note under the focused row's description, if it has one.
    fn note(&self, row: usize) -> Option<String> {
        if ROWS[row] != Row::AntiAliasing {
            return None;
        }
        let chosen = self.draft.anti_aliasing;
        let list = self.unsupported();
        if !self.supported[self.choice(row)] {
            Some(format!(
                "{} is not available on this graphics card; the next lower level is used.",
                chosen.label()
            ))
        } else if list.is_empty() {
            None
        } else {
            Some(format!(
                "{} not available on this graphics card.",
                list.join(" and ")
            ))
        }
    }

    // ---- editing --------------------------------------------------------

    fn pick(&mut self, row: usize, i: usize) -> ResultAction {
        self.focus = Focus::Row(row);
        self.row = row;
        let label = &ROWS[row].choices()[i];
        if !self.available(row, i) {
            self.message = format!("{label} is not available on this graphics card");
            return ResultAction::Changed;
        }
        if self.choice(row) == i {
            return ResultAction::None;
        }
        self.set_choice(row, i);
        self.message = format!("{} set to {label}.", ROWS[row].label());
        ResultAction::Changed
    }
    /// Left/right: the next available choice in that direction, stopping at
    /// the ends.
    fn step(&mut self, row: usize, delta: i32) -> ResultAction {
        let count = ROWS[row].choices().len() as i32;
        let mut i = self.choice(row) as i32 + delta;
        while (0..count).contains(&i) {
            if self.available(row, i as usize) {
                return self.pick(row, i as usize);
            }
            i += delta;
        }
        ResultAction::None
    }
    /// Enter, a click or the gamepad's A: the next available choice, wrapping.
    fn cycle(&mut self, row: usize) -> ResultAction {
        let count = ROWS[row].choices().len();
        let start = self.choice(row);
        (1..count)
            .map(|k| (start + k) % count)
            .find(|i| self.available(row, *i))
            .map_or(ResultAction::None, |i| self.pick(row, i))
    }
    fn defaults(&mut self) -> ResultAction {
        self.draft = Options::default();
        self.message = "Recommended settings restored. Apply to use them.".into();
        ResultAction::Changed
    }
    /// Makes the draft the settings in effect and saves it to `path` when
    /// there is one. Returns the options the host hands to the renderer.
    pub fn apply(&mut self, path: Option<&Path>) -> Options {
        self.applied = self.draft;
        self.message = match path.map(|p| self.draft.save(p)) {
            None => "Graphics settings applied for this session".into(),
            Some(Ok(())) => "Graphics settings applied and saved".into(),
            Some(Err(e)) => format!("Graphics settings applied but not saved: {e}"),
        };
        self.draft
    }

    // ---- navigation -----------------------------------------------------

    fn order() -> Vec<Focus> {
        (0..ROWS.len())
            .map(Focus::Row)
            .chain((0..FOOTER.len()).map(Focus::Footer))
            .collect()
    }
    fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
        if let Focus::Row(row) = focus {
            self.row = row;
        }
    }
    fn move_focus(&mut self, key: &str) -> ResultAction {
        let last = ROWS.len() - 1;
        let focus = match (self.focus, key) {
            (Focus::Row(r), "ArrowUp") => Focus::Row(r.saturating_sub(1)),
            (Focus::Row(r), "ArrowDown") if r < last => Focus::Row(r + 1),
            (Focus::Row(_), "ArrowDown") => Focus::Footer(0),
            (Focus::Footer(_), "ArrowUp") => Focus::Row(last),
            (Focus::Footer(i), "ArrowLeft") => Focus::Footer(i.saturating_sub(1)),
            (Focus::Footer(i), "ArrowRight") => Focus::Footer((i + 1).min(FOOTER.len() - 1)),
            (focus, _) => focus,
        };
        self.set_focus(focus);
        // Moving focus redraws without the click sound activation makes.
        ResultAction::None
    }
    fn activate(&mut self, hit: Hit) -> ResultAction {
        match hit {
            Hit::Row(row) => {
                self.set_focus(Focus::Row(row));
                ResultAction::None
            }
            Hit::Choice(row, i) => self.pick(row, i),
            Hit::Footer(i) => {
                self.focus = Focus::Footer(i);
                match i {
                    0 => ResultAction::Save,
                    1 => self.defaults(),
                    _ => ResultAction::Close,
                }
            }
        }
    }
    /// Keyboard input, and controller menu actions mapped to arrow keys,
    /// Enter and Escape.
    pub fn key(&mut self, key: &str, shift: bool) -> ResultAction {
        match (key, self.focus) {
            ("Escape", _) => ResultAction::Close,
            ("ArrowLeft", Focus::Row(r)) => self.step(r, -1),
            ("ArrowRight", Focus::Row(r)) => self.step(r, 1),
            ("ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight", _) => self.move_focus(key),
            ("Enter" | "Space", Focus::Row(r)) => self.cycle(r),
            ("Enter" | "Space", Focus::Footer(i)) => self.activate(Hit::Footer(i)),
            ("Tab", focus) => {
                let order = Self::order();
                let at = order.iter().position(|f| *f == focus).unwrap_or(0);
                let n = order.len();
                let next = if shift { at + n - 1 } else { at + 1 };
                self.set_focus(order[next % n]);
                ResultAction::None
            }
            _ => ResultAction::None,
        }
    }
    /// A wheel notch moves the focus between rows, up for a positive count.
    pub fn wheel(&mut self, notches: i32) -> ResultAction {
        let key = if notches > 0 { "ArrowUp" } else { "ArrowDown" };
        for _ in 0..notches.unsigned_abs() {
            if matches!(self.focus, Focus::Row(_)) {
                self.move_focus(key);
            }
        }
        ResultAction::None
    }

    // ---- pointer --------------------------------------------------------

    fn row_rect(row: usize) -> Rect {
        (
            PANEL_RECT.0 + 4,
            ROW_TOP + row as i32 * ROW_STEP,
            PANEL_RECT.2 - 8,
            ROW_STEP - 4,
        )
    }
    fn choice_rect(row: usize, i: usize) -> Rect {
        let r = Self::row_rect(row);
        (
            CHOICE_X + i as i32 * (CHOICE_W + CHOICE_GAP),
            r.1 + 4,
            CHOICE_W,
            r.3 - 8,
        )
    }
    /// The same footer positions as the input configuration screen.
    fn footer_rect(i: usize) -> Rect {
        (312 + i as i32 * 108, 456, 100, 18)
    }
    fn hit(&self, p: (f64, f64)) -> Option<Hit> {
        for (row, kind) in ROWS.iter().enumerate() {
            if !inside(p, Self::row_rect(row)) {
                continue;
            }
            return Some(
                (0..kind.choices().len())
                    .find(|i| inside(p, Self::choice_rect(row, *i)))
                    .map_or(Hit::Row(row), |i| Hit::Choice(row, i)),
            );
        }
        (0..FOOTER.len())
            .find(|i| inside(p, Self::footer_rect(*i)))
            .map(Hit::Footer)
    }
    /// Left button: a click needs press and release on the same control.
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> ResultAction {
        let hit = point.and_then(|p| self.hit(p));
        if down {
            self.pressed = hit;
            return ResultAction::None;
        }
        let pressed = self.pressed.take();
        if pressed != hit {
            return ResultAction::None;
        }
        hit.map_or(ResultAction::None, |h| self.activate(h))
    }

    // ---- drawing --------------------------------------------------------

    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        chrome::title_bar(pixels, font, "GRAPHICS OPTIONS   |   3D view", self.context);
        self.draw_rows(pixels, font);
        self.draw_about(pixels, font);
        let message = if self.dirty() {
            format!("{}  (not applied)", self.message)
        } else {
            self.message.clone()
        };
        chrome::status_bar(pixels, font, &message);
        for (i, label) in FOOTER.iter().enumerate() {
            chrome::footer_button(
                pixels,
                font,
                Self::footer_rect(i),
                label,
                self.focus == Focus::Footer(i),
            );
        }
    }
    fn draw_rows(&self, pixels: &mut [u8], font: &Font) {
        let panel = PANEL_RECT;
        Canvas(pixels).rect(panel, PANEL);
        Chrome::text(
            pixels,
            font,
            panel,
            TITLE,
            "3D VIEW SETTINGS",
            (panel.0 + 6, 29),
        );
        let (state, color) = if self.dirty() {
            ("CHANGED, NOT APPLIED", TITLE)
        } else {
            ("IN USE", GOOD)
        };
        let x = panel.0 + panel.2 - 6 - text_width(font, state);
        Chrome::text(pixels, font, panel, color, state, (x, 29));
        for (row, kind) in ROWS.iter().enumerate() {
            let r = Self::row_rect(row);
            let focused = self.focus == Focus::Row(row);
            if focused {
                Canvas(pixels).rect(r, FOCUS);
                Canvas(pixels).rect((r.0, r.1, 3, r.3), GOOD);
            }
            let y = r.1 + (r.3 - font.height as i32) / 2;
            Chrome::text(pixels, font, r, WHITE, kind.label(), (r.0 + 10, y));
            let chosen = self.choice(row);
            for (i, label) in kind.choices().iter().enumerate() {
                let c = Self::choice_rect(row, i);
                let available = self.available(row, i);
                let color = if i == chosen {
                    Canvas(pixels).rect(c, PALE);
                    INK
                } else {
                    Canvas(pixels).outline(c, if available { OUTLINE } else { HEADER });
                    if available { WHITE } else { MUTED }
                };
                let label = fit(font, label, c.2 - 4);
                let w = text_width(font, &label);
                let x = c.0 + (c.2 - w) / 2;
                let y = c.1 + (c.3 - font.height as i32) / 2;
                Chrome::text(pixels, font, c, color, &label, (x, y));
                if !available {
                    // Struck through: shown for completeness, not selectable.
                    let mid = c.1 + c.3 / 2;
                    Canvas(pixels).rect(
                        (x - 2, mid, w + 4, 1),
                        if i == chosen { INK } else { MUTED },
                    );
                }
            }
        }
    }
    fn draw_about(&self, pixels: &mut [u8], font: &Font) {
        let panel = ABOUT_RECT;
        Canvas(pixels).rect(panel, PANEL);
        let (title, description, note) = match self.focus {
            Focus::Footer(i) => (
                FOOTER[i].to_ascii_uppercase(),
                match i {
                    0 => "Use these settings now and save them for next time.",
                    1 => "Put back the recommended settings. Apply to use them.",
                    _ => "Close this screen. Changes you have not applied are discarded.",
                },
                None,
            ),
            Focus::Row(_) => (
                ROWS[self.row].label().to_ascii_uppercase(),
                ROWS[self.row].description(),
                self.note(self.row),
            ),
        };
        Chrome::text(
            pixels,
            font,
            panel,
            TITLE,
            &format!("ABOUT: {title}"),
            (panel.0 + 6, panel.1 + 5),
        );
        let width = panel.2 - 16;
        Chrome::text(
            pixels,
            font,
            panel,
            WHITE,
            &fit(font, description, width),
            (panel.0 + 8, panel.1 + 26),
        );
        if let Some(note) = note {
            Chrome::text(
                pixels,
                font,
                panel,
                TITLE,
                &fit(font, &note, width),
                (panel.0 + 8, panel.1 + 44),
            );
        }
        Canvas(pixels).rect((panel.0 + 6, panel.1 + 66, panel.2 - 12, 1), BUTTON);
        Chrome::text(
            pixels,
            font,
            panel,
            MUTED,
            &fit(
                font,
                "The original game had none of these. All off, at 100%, is closest to its look.",
                width,
            ),
            (panel.0 + 8, panel.1 + 76),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ALL: [bool; 4] = [true; 4];
    fn editor() -> Editor {
        Editor::new(Options::default(), ALL, "Main menu")
    }
    fn press(e: &mut Editor, key: &str) -> ResultAction {
        e.key(key, false)
    }
    fn center(r: Rect) -> (f64, f64) {
        ((r.0 + r.2 / 2) as f64, (r.1 + r.3 / 2) as f64)
    }
    fn click(e: &mut Editor, r: Rect) -> ResultAction {
        e.pointer(Some(center(r)), true);
        e.pointer(Some(center(r)), false)
    }
    /// A synthetic font: every byte a 5 pixel advance with one dot.
    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 5,
                    pixels: vec![(1, 3)],
                })
                .collect(),
        }
    }

    #[test]
    fn navigation_reaches_rows_and_footer() {
        let mut e = editor();
        assert_eq!(e.focus, Focus::Row(0));
        press(&mut e, "ArrowUp");
        assert_eq!(e.focus, Focus::Row(0), "the top row stops the focus");
        for _ in 0..3 {
            press(&mut e, "ArrowDown");
        }
        assert_eq!(e.focus, Focus::Row(3));
        press(&mut e, "ArrowDown");
        assert_eq!(e.focus, Focus::Footer(0));
        press(&mut e, "ArrowRight");
        press(&mut e, "ArrowRight");
        press(&mut e, "ArrowRight");
        assert_eq!(e.focus, Focus::Footer(2));
        press(&mut e, "ArrowUp");
        assert_eq!(e.focus, Focus::Row(3));
        // Tab walks every control and wraps; Shift-Tab goes back.
        for _ in 0..4 {
            press(&mut e, "Tab");
        }
        assert_eq!(e.focus, Focus::Row(0));
        e.key("Tab", true);
        assert_eq!(e.focus, Focus::Footer(2));
        // The About panel keeps describing the last row that had focus.
        assert_eq!(e.row, 0);
        assert_eq!(e.draft, Options::default(), "moving never edits");
    }
    #[test]
    fn values_step_cycle_and_click() {
        let mut e = editor();
        // Anti-aliasing: 4x to 8x, then stops at the end.
        assert_eq!(press(&mut e, "ArrowRight"), ResultAction::Changed);
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::X8);
        assert_eq!(press(&mut e, "ArrowRight"), ResultAction::None);
        // Enter wraps round to Off.
        press(&mut e, "Enter");
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::Off);
        press(&mut e, "ArrowDown");
        press(&mut e, "ArrowLeft");
        assert_eq!(e.draft.render_scale, 75);
        press(&mut e, "ArrowLeft");
        assert_eq!(e.draft.render_scale, 75);
        press(&mut e, "ArrowDown");
        press(&mut e, "ArrowRight");
        assert_eq!(e.draft.spotting_aid, SpottingAid::Strong);
        press(&mut e, "ArrowDown");
        press(&mut e, "Space");
        assert!(!e.draft.terrain_filtering);
        // A click on a choice picks it and focuses its row.
        assert_eq!(
            click(&mut e, Editor::choice_rect(1, 4)),
            ResultAction::Changed
        );
        assert_eq!(e.draft.render_scale, 200);
        assert_eq!(e.focus, Focus::Row(1));
        assert_eq!(
            click(&mut e, Editor::choice_rect(3, 1)),
            ResultAction::Changed
        );
        assert!(e.draft.terrain_filtering);
        // Press and release must land on the same control.
        e.pointer(Some(center(Editor::choice_rect(1, 0))), true);
        e.pointer(Some(center(Editor::choice_rect(1, 2))), false);
        assert_eq!(e.draft.render_scale, 200);
        assert!(e.dirty());
    }
    #[test]
    fn unsupported_anti_aliasing_is_shown_but_skipped() {
        let mut e = Editor::new(Options::default(), [true, false, true, false], "Main menu");
        assert_eq!(press(&mut e, "ArrowRight"), ResultAction::None);
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::X4);
        press(&mut e, "ArrowLeft");
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::Off, "2x is skipped");
        press(&mut e, "Enter");
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::X4);
        click(&mut e, Editor::choice_rect(0, 3));
        assert_eq!(e.draft.anti_aliasing, AntiAliasing::X4);
        assert!(e.message.contains("8x is not available"), "{}", e.message);
        assert_eq!(
            e.note(0).unwrap(),
            "2x and 8x not available on this graphics card."
        );
        // A saved level this adapter lacks stays selected, with a note.
        let saved = Options {
            anti_aliasing: AntiAliasing::X8,
            ..Options::default()
        };
        let e = Editor::new(saved, [true, false, true, false], "Main menu");
        assert!(e.note(0).unwrap().starts_with("8x is not available"));
        assert_eq!(e.note(1), None);
    }
    #[test]
    fn apply_cancel_and_defaults() {
        let mut e = editor();
        press(&mut e, "ArrowLeft");
        assert!(e.dirty());
        // Escape and Cancel close; the host drops the draft with the screen.
        assert_eq!(press(&mut e, "Escape"), ResultAction::Close);
        assert_eq!(click(&mut e, Editor::footer_rect(2)), ResultAction::Close);
        // Apply without a saved file only makes the draft current.
        assert_eq!(click(&mut e, Editor::footer_rect(0)), ResultAction::Save);
        let applied = e.apply(None);
        assert_eq!(applied.anti_aliasing, AntiAliasing::X2);
        assert!(!e.dirty());
        assert!(e.message.contains("this session"));
        // Defaults only change the draft.
        assert_eq!(click(&mut e, Editor::footer_rect(1)), ResultAction::Changed);
        assert_eq!(e.draft, Options::default());
        assert!(e.dirty());
        assert_eq!(e.applied.anti_aliasing, AntiAliasing::X2);
        // Apply with a path saves; a failed save reports the error.
        let dir = std::env::temp_dir().join(format!("tore-graphics-screen-{}", std::process::id()));
        let path = dir.join("graphics-v1.conf");
        e.draft.render_scale = 150;
        e.apply(Some(&path));
        assert!(e.message.ends_with("applied and saved"), "{}", e.message);
        assert_eq!(Options::load(&path).render_scale, 150);
        e.apply(Some(&path.join("inside-a-file")));
        assert!(e.message.contains("not saved"), "{}", e.message);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn draws_every_focus_without_panicking() {
        let font = font();
        let mut pixels = vec![0; 640 * 480 * 4];
        let mut e = Editor::new(
            Options {
                anti_aliasing: AntiAliasing::X8,
                ..Options::default()
            },
            [true, true, true, false],
            "Main menu",
        );
        for _ in 0..Editor::order().len() {
            e.draw(&mut pixels, &font);
            press(&mut e, "Tab");
        }
        // The header strip is the panel colour, the body the paper colour.
        assert_eq!(pixels[..4], PANEL);
        assert_eq!(pixels[(300 * 640 + 2) * 4..][..4], chrome::PAPER);
    }
}
