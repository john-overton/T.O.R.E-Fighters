use crate::assets::Assets;
use std::{
    collections::BTreeMap,
    hash::BuildHasher,
    time::{Duration, Instant},
};
use tore_formats::Button;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Button(usize),
    Bar(usize),
    Item(usize),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    None,
    Hover,
    Click,
    Exit,
    Music(bool),
    Effects(bool),
}
pub struct State {
    pub buttons: Vec<Button>,
    pub hover: Option<Target>,
    pub pressed: Option<Target>,
    pub focus: Option<Target>,
    pub open: Option<usize>,
    pub music: bool,
    pub effects: bool,
    pub toast: Option<(String, Instant)>,
    keyboard: bool,
    glow: Vec<f32>,
    last_frame: Instant,
    bar_offset: i32,
}
fn enabled(index: usize) -> bool {
    !matches!(index, 3 | 6)
} // Sorted retail rows: replay / continue.
fn in_rect(point: (f64, f64), rect: (i32, i32, i32, i32)) -> bool {
    point.0 >= rect.0 as f64
        && point.1 >= rect.1 as f64
        && point.0 < (rect.0 + rect.2) as f64
        && point.1 < (rect.1 + rect.3) as f64
}
const BARS: [(i32, i32, i32, i32); 3] = [(78, 38, 17, 19), (96, 38, 39, 19), (135, 38, 45, 19)];
impl State {
    pub fn new(buttons: Vec<Button>, music: bool) -> Self {
        let count = buttons.len();
        Self {
            buttons,
            hover: None,
            pressed: None,
            focus: None,
            open: None,
            music,
            effects: true,
            toast: None,
            keyboard: false,
            glow: vec![0.0; count],
            last_frame: Instant::now(),
            bar_offset: 0,
        }
    }
    pub fn items(&self, bar: usize) -> Vec<String> {
        match bar {
            0 => vec![
                "Help...".into(),
                "About Fighters Anthology...".into(),
                "Exit to Desktop".into(),
            ],
            1 => vec![
                "Graphics...".into(),
                "Sound...".into(),
                "Controls...".into(),
                format!("Music: {}", if self.music { "On" } else { "Off" }),
                format!("Effects: {}", if self.effects { "On" } else { "Off" }),
            ],
            _ => vec![
                "Host Game...".into(),
                "Join Game...".into(),
                "Player Setup...".into(),
            ],
        }
    }
    fn popup_rect(&self, bar: usize) -> (i32, i32, i32, i32) {
        (
            self.bar_rect(bar).0,
            59,
            if bar == 0 { 240 } else { 190 },
            self.items(bar).len() as i32 * 20 + 8,
        )
    }
    fn bar_rect(&self, bar: usize) -> (i32, i32, i32, i32) {
        let (x, y, w, h) = BARS[bar];
        (x + self.bar_offset, y, w, h)
    }
    pub fn hit(&self, point: Option<(f64, f64)>) -> Option<Target> {
        let p = point?;
        for i in 0..BARS.len() {
            if in_rect(p, self.bar_rect(i)) {
                return Some(Target::Bar(i));
            }
        }
        if let Some(bar) = self.open {
            let rect = self.popup_rect(bar);
            if in_rect(p, rect) && p.1 >= 63.0 {
                let row = ((p.1 - 63.0) / 20.0) as usize;
                if row < self.items(bar).len() {
                    return Some(Target::Item(row));
                }
            }
            return None;
        }
        self.buttons.iter().enumerate().find_map(|(i, b)| {
            (enabled(i) && in_rect(p, (b.x, b.y, b.width - 10, 22))).then_some(Target::Button(i))
        })
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>) -> Action {
        let next = self.hit(point);
        self.keyboard = false;
        if next == self.hover {
            return Action::None;
        }
        self.hover = next;
        if self.open.is_some()
            && let Some(Target::Bar(bar)) = next
        {
            self.open = Some(bar);
        }
        if next.is_some() {
            Action::Hover
        } else {
            Action::None
        }
    }
    pub fn down(&mut self) {
        self.pressed = self.hover;
        if self.hover.is_none() {
            self.open = None;
            self.focus = None;
        }
    }
    pub fn up(&mut self) -> Action {
        let pressed = self.pressed.take();
        if let Some(target) = pressed.filter(|_| pressed == self.hover) {
            self.activate(target)
        } else {
            Action::None
        }
    }
    fn activate(&mut self, target: Target) -> Action {
        match target {
            Target::Bar(bar) => {
                self.open = if self.open == Some(bar) {
                    None
                } else {
                    Some(bar)
                };
                self.focus = Some(Target::Bar(bar));
            }
            Target::Button(i) if enabled(i) => {
                self.toast = Some((
                    format!("{} - coming soon", self.buttons[i].label),
                    Instant::now(),
                ));
            }
            Target::Item(row) => {
                if let Some(bar) = self.open {
                    if bar == 0 && row == 2 {
                        return Action::Exit;
                    }
                    if bar == 1 && row == 3 {
                        self.music = !self.music;
                        return Action::Music(self.music);
                    }
                    if bar == 1 && row == 4 {
                        self.effects = !self.effects;
                        return Action::Effects(self.effects);
                    }
                    if let Some(item) = self.items(bar).get(row) {
                        self.toast = Some((
                            format!("{} - coming soon", item.trim_end_matches('.')),
                            Instant::now(),
                        ));
                    }
                    self.open = None;
                    self.hover = None;
                    self.focus = None;
                }
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn cancel(&mut self) {
        self.open = None;
        self.pressed = None;
        self.hover = None;
        self.focus = None;
        self.toast = None;
    }
    pub fn key(&mut self, key: &str, shift: bool) -> Action {
        self.keyboard = true;
        match key {
            "Escape" => {
                self.cancel();
                Action::None
            }
            "m" | "M" => {
                self.music = !self.music;
                Action::Music(self.music)
            }
            "Enter" | " " => self.focus.map(|t| self.activate(t)).unwrap_or(Action::None),
            "ArrowDown" | "ArrowUp" if self.open.is_some() => {
                let bar = self.open.unwrap();
                let n = self.items(bar).len();
                let row = match self.focus {
                    Some(Target::Item(i)) => {
                        if key == "ArrowDown" {
                            (i + 1) % n
                        } else {
                            (i + n - 1) % n
                        }
                    }
                    _ => 0,
                };
                self.focus = Some(Target::Item(row));
                Action::Hover
            }
            "Tab" | "ArrowDown" | "ArrowUp" | "ArrowLeft" | "ArrowRight" => {
                let targets: Vec<_> = (0..3)
                    .map(Target::Bar)
                    .chain(
                        (0..self.buttons.len())
                            .filter(|i| enabled(*i))
                            .map(Target::Button),
                    )
                    .collect();
                let reverse = shift || matches!(key, "ArrowUp" | "ArrowLeft");
                let at = targets.iter().position(|t| Some(*t) == self.focus);
                let next = at
                    .map(|i| {
                        if reverse {
                            (i + targets.len() - 1) % targets.len()
                        } else {
                            (i + 1) % targets.len()
                        }
                    })
                    .unwrap_or(0);
                self.open = None;
                self.focus = Some(targets[next]);
                Action::Hover
            }
            _ => Action::None,
        }
    }
    fn highlighted(&self, target: Target) -> bool {
        if self.keyboard {
            self.focus == Some(target)
        } else {
            self.hover == Some(target)
        }
    }
    pub fn animate(&mut self) -> bool {
        let now = Instant::now();
        // An idle window may not have drawn for minutes. Start a new transition
        // smoothly rather than consuming all that idle time in its first frame.
        let step = (now - self.last_frame).as_secs_f32().min(1.0 / 30.0) / 0.12;
        self.last_frame = now;
        let mut active = false;
        for i in 0..self.glow.len() {
            let target = if self.highlighted(Target::Button(i)) {
                1.0
            } else {
                0.0
            };
            let difference = target - self.glow[i];
            self.glow[i] += difference.clamp(-step, step);
            active |= (target - self.glow[i]).abs() > 0.001;
        }
        if self
            .toast
            .as_ref()
            .is_some_and(|(_, at)| now - *at > Duration::from_secs(3))
        {
            self.toast = None;
        }
        active || self.toast.is_some()
    }
}

struct Sprite {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    glyphs: Vec<[usize; 3]>,
}
pub struct Menu {
    pub state: State,
    sprites: BTreeMap<String, Sprite>,
    pub pixels: Vec<u8>,
    background: String,
}
impl Menu {
    pub fn preview_state(&mut self, name: &str) -> crate::AppResult<()> {
        match name {
            "normal" => {}
            "hover" | "pressed" => {
                self.state.hover = Some(Target::Button(0));
                self.state.glow[0] = 1.0;
                if name == "pressed" {
                    self.state.down();
                }
            }
            "help" | "pref" | "multi" => {
                let bar = match name {
                    "help" => 0,
                    "pref" => 1,
                    _ => 2,
                };
                self.state.open = Some(bar);
                self.state.hover = Some(Target::Item(0));
            }
            _ => {
                return Err(
                    "snapshot state must be normal, hover, pressed, help, pref, or multi".into(),
                );
            }
        }
        Ok(())
    }
    pub fn new(mut assets: Assets, selected: Option<&str>) -> crate::AppResult<Self> {
        let choices = [
            "CHOOSEAC.PIC",
            "CHOOSE3.PIC",
            "CHOOSEU.PIC",
            "CHOOSEM.PIC",
            "CHOOSEV.PIC",
        ];
        let background = if let Some(name) = selected {
            let name = name.to_ascii_uppercase();
            let name = if name.ends_with(".PIC") {
                name
            } else {
                format!("{name}.PIC")
            };
            if !choices.contains(&name.as_str()) {
                return Err(
                    "background must be CHOOSEAC, CHOOSE3, CHOOSEU, CHOOSEM, or CHOOSEV".into(),
                );
            }
            name
        } else {
            // Menu-only randomness; never shares state with the future deterministic sim.
            let seed = std::collections::hash_map::RandomState::new().hash_one(());
            choices[seed as usize % choices.len()].to_string()
        };
        assets.palette = assets.pics[&background]
            .palette
            .clone()
            .try_into()
            .map_err(|_| "invalid background palette")?;
        let mut state = State::new(assets.buttons, assets.sounds.contains_key("AIR003.11K"));
        // Native bar origins are 70, 185 and 76, respectively (FA.EXE 0x4a091a..64).
        state.bar_offset = match background.as_str() {
            "CHOOSEAC.PIC" => -6,
            "CHOOSE3.PIC" => 109,
            _ => 0,
        };
        println!("Main-menu background: {background}");
        let sprites = assets
            .pics
            .into_iter()
            .map(|(name, p)| {
                let rgba = p.rgba(&assets.palette);
                (
                    name,
                    Sprite {
                        width: p.width,
                        height: p.height,
                        rgba,
                        glyphs: p.glyphs,
                    },
                )
            })
            .collect();
        Ok(Self {
            state,
            sprites,
            pixels: vec![0; WIDTH * HEIGHT * 4],
            background,
        })
    }
    pub fn render(&mut self) -> bool {
        let active = self.state.animate();
        self.pixels
            .copy_from_slice(&self.sprites[&self.background].rgba);
        let mut canvas = Canvas(&mut self.pixels);
        for (i, b) in self.state.buttons.iter().enumerate() {
            let pressed = self.state.pressed == Some(Target::Button(i))
                && self.state.hover == self.state.pressed;
            let shift = i32::from(pressed);
            let gain = if pressed {
                0.83
            } else {
                1.0 + self.state.glow[i] * 0.15
            };
            let stem = if enabled(i) { "ACTION0" } else { "ACTIOD0" };
            let l = &self.sprites[&format!("{stem}L.PIC")];
            let m = &self.sprites[&format!("{stem}M.PIC")];
            let r = &self.sprites[&format!("{stem}R.PIC")];
            let (x, y) = (b.x + shift, b.y + shift);
            canvas.blit(l, (x, y), 0, l.width, gain);
            let end = b.width - r.width as i32;
            let mut at = l.width as i32;
            while at < end {
                canvas.blit(m, (x + at, y), 0, m.width.min((end - at) as usize), gain);
                at += m.width as i32;
            }
            canvas.blit(r, (x + end, y), 0, r.width, gain);
            let font = &self.sprites[if enabled(i) {
                "FONTACT.PIC"
            } else {
                "FONTACD.PIC"
            }];
            let tx = x + (b.width - 10 - text_width(font, &b.label)) / 2;
            canvas.text(font, &b.label, tx, y + 4, None);
            if self.state.keyboard && self.state.focus == Some(Target::Button(i)) {
                canvas.outline((b.x - 2, b.y - 2, b.width - 7, 25), [204, 225, 205, 255]);
            }
        }
        let menu_font = &self.sprites["MENUFONT.PIC"];
        for (i, label) in ["?", "Pref", "Multi"].iter().enumerate() {
            let rect = self.state.bar_rect(i);
            if self.state.open == Some(i) || self.state.highlighted(Target::Bar(i)) {
                canvas.rect(rect, [190, 200, 215, 255]);
            }
            canvas.text(menu_font, label, rect.0 + 2, rect.1, None);
        }
        let body = &self.sprites["BODYFONT.PIC"];
        if let Some(bar) = self.state.open {
            let (x, y, w, h) = self.state.popup_rect(bar);
            canvas.rect((x + 3, y + 3, w, h), [20, 23, 26, 255]);
            canvas.rect((x, y, w, h), [207, 210, 215, 255]);
            canvas.outline((x, y, w, h), [246, 248, 250, 255]);
            for (row, label) in self.state.items(bar).iter().enumerate() {
                let highlighted = self.state.highlighted(Target::Item(row));
                if highlighted {
                    canvas.rect(
                        (x + 3, y + 4 + row as i32 * 20, w - 6, 20),
                        [242, 245, 249, 255],
                    );
                }
                canvas.text(menu_font, label, x + 9, y + 6 + row as i32 * 20, None);
            }
        }
        if let Some((message, _)) = &self.state.toast {
            let width = (text_width(body, message) + 20).min(610);
            canvas.rect((14, 449, width, 23), [22, 31, 43, 255]);
            canvas.outline((14, 449, width, 23), [145, 161, 180, 255]);
            canvas.text(body, message, 24, 456, Some([235, 239, 243]));
        }
        active
    }
    pub fn save_ppm(&mut self, path: &std::path::Path) -> crate::AppResult<()> {
        use std::io::Write;
        self.render();
        let mut file = std::fs::File::create(path)?;
        write!(file, "P6\n640 480\n255\n")?;
        for p in self.pixels.chunks_exact(4) {
            file.write_all(&p[..3])?;
        }
        Ok(())
    }
}
fn text_width(font: &Sprite, text: &str) -> i32 {
    text.bytes()
        .map(|c| font.glyphs[c as usize][1] as i32)
        .sum()
}
struct Canvas<'a>(&'a mut [u8]);
impl Canvas<'_> {
    fn rect(&mut self, (x, y, w, h): (i32, i32, i32, i32), color: [u8; 4]) {
        for yy in y.max(0)..(y + h).min(HEIGHT as i32) {
            for xx in x.max(0)..(x + w).min(WIDTH as i32) {
                let at = (yy as usize * WIDTH + xx as usize) * 4;
                self.0[at..at + 4].copy_from_slice(&color);
            }
        }
    }
    fn outline(&mut self, (x, y, w, h): (i32, i32, i32, i32), color: [u8; 4]) {
        for rect in [
            (x, y, w, 1),
            (x, y + h - 1, w, 1),
            (x, y, 1, h),
            (x + w - 1, y, 1, h),
        ] {
            self.rect(rect, color);
        }
    }
    fn blit(&mut self, s: &Sprite, (x, y): (i32, i32), sx: usize, width: usize, gain: f32) {
        for yy in 0..s.height {
            for xx in 0..width {
                let (dx, dy) = (x + xx as i32, y + yy as i32);
                if dx < 0 || dy < 0 || dx >= WIDTH as i32 || dy >= HEIGHT as i32 {
                    continue;
                }
                let source = (yy * s.width + sx + xx) * 4;
                if s.rgba[source + 3] == 0 {
                    continue;
                }
                let dest = (dy as usize * WIDTH + dx as usize) * 4;
                for c in 0..3 {
                    self.0[dest + c] = (s.rgba[source + c] as f32 * gain).min(255.0) as u8;
                }
                self.0[dest + 3] = 255;
            }
        }
    }
    fn text(&mut self, font: &Sprite, text: &str, mut x: i32, y: i32, tint: Option<[u8; 3]>) {
        for c in text.bytes() {
            let [sx, w, h] = font.glyphs[c as usize];
            if let Some(rgb) = tint {
                for yy in 0..h {
                    for xx in 0..w {
                        let at = (yy * font.width + sx + xx) * 4;
                        if font.rgba[at + 3] > 0 {
                            self.rect(
                                (x + xx as i32, y + yy as i32, 1, 1),
                                [rgb[0], rgb[1], rgb[2], 255],
                            );
                        }
                    }
                }
            } else {
                self.blit(font, (x, y), sx, w, 1.0);
            }
            x += w as i32;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shifted_background_bar_uses_matching_hit_regions() {
        let mut s = state();
        s.bar_offset = 109;
        assert_eq!(s.hit(Some((85.0, 44.0))), None);
        assert_eq!(s.hit(Some((194.0, 44.0))), Some(Target::Bar(0)));
    }
    fn state() -> State {
        State::new(
            (0..8)
                .map(|i| Button {
                    x: 420,
                    y: 100 + i * 35,
                    width: 145,
                    label: format!("Activity {i}"),
                })
                .collect(),
            true,
        )
    }
    #[test]
    fn click_requires_release_on_same_enabled_control() {
        let mut s = state();
        s.pointer(Some((425.0, 105.0)));
        s.down();
        s.pointer(None);
        assert_eq!(s.up(), Action::None);
        assert_eq!(s.hit(Some((425.0, 210.0))), None);
        s.pointer(Some((425.0, 105.0)));
        s.down();
        assert_eq!(s.up(), Action::Click);
        assert!(s.toast.is_some());
    }
    #[test]
    fn dropdown_exit_and_escape_are_distinct() {
        let mut s = state();
        s.pointer(Some((85.0, 44.0)));
        s.down();
        s.up();
        assert_eq!(s.open, Some(0));
        // An open dropdown captures clicks; underlying activity buttons cannot fire.
        assert_eq!(s.hit(Some((425.0, 105.0))), None);
        s.key("Escape", false);
        assert_eq!(s.open, None);
        s.pointer(Some((85.0, 44.0)));
        s.down();
        s.up();
        s.pointer(Some((90.0, 110.0)));
        s.down();
        assert_eq!(s.up(), Action::Exit);
    }
    #[test]
    fn keyboard_skips_disabled_actions_and_toggles_music() {
        let mut s = state();
        for _ in 0..12 {
            s.key("Tab", false);
            assert!(!matches!(s.focus, Some(Target::Button(3 | 6))));
        }
        assert_eq!(s.key("m", false), Action::Music(false));
    }
}
