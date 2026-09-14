//! Desktop input and the imported in-flight menu. Unported commands stay explicit.
use crate::{hud::Paint, menu::Canvas};
use std::time::{Duration, Instant};
use tore_formats::{font::Font, ui::MenuNode};
#[derive(Debug, PartialEq)]
pub enum Command {
    None,
    Click,
    End,
    Exit,
    Restart,
    Toggle(&'static str),
    View(u8),
    CenterLook,
    Panel(u8),
    WindowLayout,
    Throttle(f64),
    Range(i32),
    Mode,
    Effects(bool),
}
type Control = (usize, (i32, i32, i32, i32), String);
pub struct FlightUi {
    pub menu: bool,
    pub paused: bool,
    pub cockpit: bool,
    pub hud: bool,
    pub ladder: bool,
    pub brightness: u8,
    pub zoom: f32,
    pub look: [f32; 2],
    pub time_scale: f64,
    pub effects: bool,
    pub notice: Option<(String, Instant)>,
    pub help: bool,
    root: usize,
    path: Vec<usize>,
    focus: usize,
    pressed: Option<usize>,
    help_page: usize,
}
impl Default for FlightUi {
    fn default() -> Self {
        Self {
            menu: false,
            paused: false,
            cockpit: true,
            hud: true,
            ladder: true,
            brightness: 7,
            zoom: 1.,
            look: [0.; 2],
            time_scale: 1.,
            effects: true,
            notice: None,
            help: false,
            root: 0,
            path: vec![],
            focus: 0,
            pressed: None,
            help_page: 0,
        }
    }
}
impl FlightUi {
    pub fn frozen(&self) -> bool {
        self.menu || self.paused
    }
    pub fn steps(&self, clock: &mut crate::flight::Clock, elapsed: f64) -> usize {
        if self.frozen() {
            0
        } else if self.time_scale == 1. {
            clock.steps(elapsed)
        } else {
            clock.steps_scaled(elapsed, self.time_scale)
        }
    }
    pub fn cancel_press(&mut self) {
        self.pressed = None;
    }
    pub fn message(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
    }
    fn unavailable(&mut self, label: &str) -> Command {
        self.message(format!("{label}: not implemented yet"));
        Command::Click
    }
    fn rows<'a>(&self, tree: &'a [MenuNode]) -> &'a [MenuNode] {
        let mut rows = &tree[self.root].children[..];
        for &index in &self.path {
            rows = &rows[index].children;
        }
        rows
    }
    pub fn activate(&mut self, label: &str, shortcut: &str) -> Command {
        match label {
            "Resume flight" => {
                self.menu = false;
                self.paused = false;
                Command::Click
            }
            "Restart free flight" => Command::Restart,
            "Keyboard" => {
                self.message("Keyboard controls selected");
                Command::Click
            }
            "End mission" => Command::End,
            "Exit to Windows" => Command::Exit,
            "Keyboard shortcuts" => {
                self.help = true;
                self.help_page = 0;
                Command::Click
            }
            "Paused" => {
                self.paused = !self.frozen();
                self.menu = false;
                Command::Click
            }
            "Large windows?" => Command::WindowLayout,
            "Show cockpit?" => {
                self.cockpit = !self.cockpit;
                Command::Click
            }
            "HUD pitch ladder?" => {
                self.ladder = !self.ladder;
                Command::Click
            }
            "Dim HUD" => {
                self.brightness = self.brightness.saturating_sub(1);
                Command::Click
            }
            "Brighten HUD" => {
                self.brightness = (self.brightness + 1).min(9);
                Command::Click
            }
            "Sound..." => {
                self.effects = !self.effects;
                self.message(if self.effects {
                    "Sound on"
                } else {
                    "Sound off"
                });
                Command::Effects(self.effects)
            }
            "1x" | "2x" | "4x" | "8x" | "Slow-motion" => {
                self.time_scale = match label {
                    "2x" => 2.,
                    "4x" => 4.,
                    "8x" => 8.,
                    "Slow-motion" => 0.5,
                    _ => 1.,
                };
                self.message(format!("Time {}x", self.time_scale));
                Command::Click
            }
            "Current" => Command::Panel(1),
            _ => match shortcut {
                "F1" => Command::View(0),
                "F2" => Command::View(3),
                "F3" => Command::View(4),
                "F10" => Command::View(1),
                "Shift-0" => Command::Panel(0),
                s if s.starts_with("Shift-")
                    && s.len() == 7
                    && s.as_bytes()[6].is_ascii_digit() =>
                {
                    Command::Panel(s.as_bytes()[6] - b'0')
                }
                _ => self.unavailable(label),
            },
        }
    }
    fn select(&mut self, tree: &[MenuNode], index: usize) -> Command {
        self.focus = index;
        let rows = self.rows(tree);
        if index >= rows.len() {
            return Command::None;
        }
        let node = &rows[index];
        if !node.children.is_empty() {
            self.path.push(index);
            self.focus = 0;
            Command::Click
        } else {
            self.activate(&node.label, &node.shortcut)
        }
    }
    pub fn key(
        &mut self,
        key: &str,
        shift: bool,
        ctrl: bool,
        alt: bool,
        tree: &[MenuNode],
    ) -> Command {
        if key == "Escape" {
            self.pressed = None;
            if self.help {
                self.help = false;
            } else if !self.path.is_empty() {
                self.path.pop();
                self.focus = 0;
            } else {
                self.menu = !self.menu;
            }
            return Command::None;
        }
        if ctrl && !shift && !alt && key == "p" {
            return self.activate("Paused", "");
        }
        if ctrl && !shift && !alt && key == "q" {
            return self.activate("End mission", "");
        }
        if self.menu {
            if self.help {
                if matches!(key, "ArrowRight" | "PageDown" | "Space" | "Enter") {
                    self.help_page += 1;
                }
                if matches!(key, "ArrowLeft" | "PageUp") {
                    self.help_page = self.help_page.saturating_sub(1);
                }
                return Command::None;
            }
            let len = self.rows(tree).len();
            match key {
                "ArrowDown" | "Tab" => self.focus = (self.focus + 1) % len,
                "ArrowUp" => self.focus = (self.focus + len - 1) % len,
                "ArrowRight" => {
                    if !self.rows(tree)[self.focus].children.is_empty() {
                        return self.select(tree, self.focus);
                    }
                    self.root = (self.root + 1) % tree.len();
                    self.path.clear();
                    self.focus = 0;
                }
                "ArrowLeft" => {
                    if self.path.pop().is_none() {
                        self.root = (self.root + tree.len() - 1) % tree.len();
                    }
                    self.focus = 0;
                }
                "Enter" | "Space" => return self.select(tree, self.focus),
                _ => {}
            }
            return Command::None;
        }
        if alt && !ctrl && !shift {
            if key.starts_with('F') {
                return self.unavailable("Target-relative camera (no target)");
            }
            if matches!(
                key,
                "1" | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "b"
                    | "c"
                    | "t"
                    | "h"
                    | "v"
                    | "e"
                    | "w"
                    | "r"
                    | "p"
                    | "d"
            ) {
                return self.unavailable("Wingman command (no wingman)");
            }
        }
        if ctrl && key.starts_with('F') {
            return self.unavailable("Missile-relative camera (no missile)");
        }
        let shortcut = format!(
            "{}{}{}{}",
            if ctrl { "Ctrl-" } else { "" },
            if alt { "Alt-" } else { "" },
            if shift { "Shift-" } else { "" },
            if key == "Backspace" { "BS" } else { key }
        );
        fn find<'a>(tree: &'a [MenuNode], key: &str) -> Option<&'a MenuNode> {
            for node in tree {
                if !node.shortcut.is_empty() && node.shortcut.eq_ignore_ascii_case(key) {
                    return Some(node);
                }
                if let Some(node) = find(&node.children, key) {
                    return Some(node);
                }
            }
            None
        }
        // Time cycle has four menu rows with the same accelerator.
        if !ctrl && !alt && !shift && key == "c" {
            self.time_scale = match self.time_scale {
                x if x < 1. => 1.,
                1. => 2.,
                2. => 4.,
                4. => 8.,
                _ => 1.,
            };
            self.message(format!("Time {}x", self.time_scale));
            return Command::Click;
        }
        if let Some(node) = find(tree, &shortcut) {
            return self.activate(&node.label, &node.shortcut);
        }
        if ctrl || alt {
            return Command::None;
        }
        if shift {
            return match key {
                "/" => Command::CenterLook,
                "b" => Command::Toggle("t"),
                "u" => {
                    self.hud = !self.hud;
                    Command::Click
                }
                "j" | "k" => self.unavailable("Jettison stores/fuel"),
                "t" => self.unavailable("Previous target"),
                "w" => self.unavailable("Previous waypoint"),
                _ => Command::None,
            };
        }
        match key {
            "g" => Command::Toggle("g"),
            "f" => Command::Toggle("f"),
            "b" => Command::Toggle("b"),
            "h" => Command::Toggle("h"),
            "e" => Command::Toggle("e"),
            "r" => Command::Toggle("r"),
            "j" => Command::Toggle("j"),
            "0" => Command::Throttle(1.),
            n if n.len() == 1 && n.as_bytes()[0].is_ascii_digit() => {
                Command::Throttle((n.as_bytes()[0] - b'0') as f64 / 10.)
            }
            "=" | "+" => {
                self.zoom = (self.zoom * 1.2).min(4.);
                Command::None
            }
            "-" => {
                self.zoom = (self.zoom / 1.2).max(0.5);
                Command::None
            }
            "," => Command::Range(-1),
            "." => Command::Range(1),
            "o" => Command::Mode,
            "F11" => {
                self.menu = true;
                self.help = true;
                Command::None
            }
            "a" => self.unavailable("Autopilot"),
            "t" => self.unavailable("Next target"),
            "w" => self.unavailable("Next waypoint"),
            "n" => self.unavailable("Navigation / weapons mode"),
            "i" => self.unavailable("IR sensor"),
            "m" => self.unavailable("HARM seeker"),
            "y" => self.unavailable("Radar history"),
            "Enter" | "'" => self.unavailable("Designate target"),
            "Space" => self.unavailable("Fire weapon"),
            "v" => self.unavailable("Store Other View camera"),
            _ => Command::None,
        }
    }
    // Top buttons, source rows and development session actions share hit/render geometry.
    fn controls(&self, tree: &[MenuNode]) -> Vec<Control> {
        let mut out = vec![];
        if self.help {
            return vec![
                (300, (480, 430, 140, 24), "Next page / Enter".into()),
                (301, (20, 430, 140, 24), "Back / Escape".into()),
            ];
        }
        let mut x = 4;
        for (i, n) in tree.iter().enumerate() {
            let w = n.label.len() as i32 * 7 + 16;
            out.push((100 + i, (x, 2, w, 22), n.label.clone()));
            x += w;
        }
        let rows = self.rows(tree);
        for (i, n) in rows.iter().enumerate() {
            let label = if n.label == "Exit to Windows" {
                "Exit to Desktop"
            } else {
                &n.label
            };
            out.push((
                i,
                (142, 50 + i as i32 * 19, 356, 19),
                format!(
                    "{label}  {}{}",
                    n.shortcut,
                    if n.children.is_empty() { "" } else { " >" }
                ),
            ));
        }
        for (i, label) in ["Resume flight", "Restart free flight", "Keyboard shortcuts"]
            .iter()
            .enumerate()
        {
            out.push((
                200 + i,
                (10 + i as i32 * 210, 448, 200, 23),
                (*label).into(),
            ));
        }
        out
    }
    pub fn pointer(&mut self, tree: &[MenuNode], point: Option<(f64, f64)>, down: bool) -> Command {
        let hit = point.and_then(|(x, y)| {
            self.controls(tree)
                .into_iter()
                .find_map(|(id, (rx, ry, w, h), _)| {
                    ((rx as f64..(rx + w) as f64).contains(&x)
                        && (ry as f64..(ry + h) as f64).contains(&y))
                    .then_some(id)
                })
        });
        if down {
            self.pressed = hit;
            return Command::None;
        }
        let pressed = self.pressed.take();
        if hit != pressed {
            return Command::None;
        }
        match hit {
            Some(300) => {
                self.help_page += 1;
                Command::Click
            }
            Some(301) => {
                self.help = false;
                Command::Click
            }
            Some(id) if id >= 200 => self.activate(
                ["Resume flight", "Restart free flight", "Keyboard shortcuts"][id - 200],
                "",
            ),
            Some(id) if id >= 100 => {
                self.root = id - 100;
                self.path.clear();
                self.focus = 0;
                Command::Click
            }
            Some(id) => self.select(tree, id),
            None => Command::None,
        }
    }
    pub fn draw(&self, pixels: &mut [u8], font: &Font, tree: &[MenuNode]) {
        if self.menu {
            Canvas(pixels).rect((0, 0, 640, 26), [200, 207, 219, 255]);
            if self.help {
                Canvas(pixels).rect((10, 40, 620, 398), [24, 34, 45, 255]);
                let mut lines: Vec<String> = vec![
                    "DESKTOP KEYBOARD COMMANDS - Esc returns".into(),
                    "Arrows: pitch/bank | Z/X: rudder | PageUp/Down: throttle".into(),
                    "1..9: 10..90%, 0: full | Shift-B: burner | E: engine".into(),
                    "G: gear | F: flaps | B: brake | H: hook | J: jammer".into(),
                    "Shift/Ctrl-arrows: look/orbit | Shift-/: center | F1: cockpit".into(),
                    "Comma/period: scope range | O: radar mode | Shift-U: HUD".into(),
                    "T/Shift-T: target | Enter/apostrophe: designate | Space: fire".into(),
                    "A: autopilot | W/Shift-W: waypoint | N: nav/weapons".into(),
                    "I: IR | M: HARM | Y: history | V: store Other View".into(),
                    "Shift-J/K: jettison | Alt-1..9,B,C,T,H,V,E,W,R,P,D: wingman".into(),
                    "Unported systems report not implemented; no fake targets.".into(),
                    "SOURCE MENU SHORTCUTS:".into(),
                ];
                fn add(tree: &[MenuNode], lines: &mut Vec<String>) {
                    for n in tree {
                        if !n.shortcut.is_empty() {
                            lines.push(format!("{}: {}", n.shortcut, n.label));
                        }
                        add(&n.children, lines);
                    }
                }
                add(tree, &mut lines);
                let pages = lines.len().div_ceil(23);
                let start = (self.help_page % pages) * 23;
                let mut p = Paint {
                    pixels,
                    clip: (16, 44, 608, 390),
                    color: [223, 233, 240, 255],
                };
                for (i, line) in lines.iter().skip(start).take(23).enumerate() {
                    p.text(font, line, 20, 50 + i as i32 * 16);
                }
            } else {
                Canvas(pixels).rect(
                    (138, 28, 364, 22 + self.rows(tree).len() as i32 * 19),
                    [160, 172, 186, 255],
                );
                let mut p = Paint {
                    pixels,
                    clip: (0, 0, 640, 480),
                    color: [15, 30, 50, 255],
                };
                p.text(
                    font,
                    if self.path.is_empty() {
                        "GAME PAUSED"
                    } else {
                        "SUBMENU - Left / Escape to go back"
                    },
                    146,
                    34,
                );
            }
            for (id, r, label) in self.controls(tree) {
                let selected = (!self.help && (id == self.focus || id == 100 + self.root))
                    || self.pressed == Some(id);
                Canvas(pixels).rect(
                    r,
                    if selected {
                        [62, 86, 118, 255]
                    } else {
                        [201, 210, 222, 255]
                    },
                );
                let mut p = Paint {
                    pixels,
                    clip: r,
                    color: if selected {
                        [247, 250, 255, 255]
                    } else {
                        [20, 39, 65, 255]
                    },
                };
                p.text(font, &label, r.0 + 5, r.1 + 6);
            }
        } else if self.paused {
            Canvas(pixels).rect((222, 35, 196, 22), [20, 30, 40, 230]);
            Paint {
                pixels,
                clip: (0, 0, 640, 480),
                color: [230, 240, 245, 255],
            }
            .text(font, "PAUSED - Ctrl-P to resume", 232, 42);
        }
        if let Some((text, at)) = &self.notice
            && at.elapsed() < Duration::from_secs(4)
        {
            Canvas(pixels).rect((8, 416, 624, 22), [20, 30, 40, 245]);
            Paint {
                pixels,
                clip: (12, 418, 616, 18),
                color: [240, 233, 194, 255],
            }
            .text(font, text, 16, 423);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> Vec<MenuNode> {
        vec![MenuNode {
            label: "?".into(),
            shortcut: String::new(),
            children: vec![MenuNode {
                label: "End mission".into(),
                shortcut: "Ctrl-Q".into(),
                children: vec![],
            }],
        }]
    }
    #[test]
    fn escape_pauses_without_ending_and_modifiers_do_not_toggle_devices() {
        let mut u = FlightUi::default();
        let t = tree();
        assert_eq!(u.key("Escape", false, false, false, &t), Command::None);
        assert!(u.frozen());
        assert_eq!(u.key("g", false, false, false, &t), Command::None);
        u.key("Escape", false, false, false, &t);
        assert!(!u.frozen());
        assert_eq!(u.key("g", false, true, false, &t), Command::None);
        assert_eq!(u.key("g", false, false, false, &t), Command::Toggle("g"));
        assert_eq!(u.key("q", false, true, false, &t), Command::End);
    }
    #[test]
    fn pause_discards_wall_time_and_time_scaling_keeps_fixed_ticks() {
        let mut u = FlightUi::default();
        let mut clock = crate::flight::Clock { remainder: 0. };
        let tree = tree();
        assert_eq!(u.steps(&mut clock, 0.1), 12);
        u.key("Escape", false, false, false, &tree);
        for _ in 0..100 {
            assert_eq!(u.steps(&mut clock, 10.), 0);
        }
        u.key("Escape", false, false, false, &tree);
        assert_eq!(u.steps(&mut clock, 0.1), 12);
        for hz in [30, 60, 144] {
            clock.remainder = 0.;
            u.time_scale = 8.;
            let count: usize = (0..hz).map(|_| u.steps(&mut clock, 1. / hz as f64)).sum();
            assert_eq!(count, 960);
        }
    }
    #[test]
    fn nested_menu_keyboard_navigation_and_shortcuts() {
        let mut tree = tree();
        tree[0].children.push(MenuNode {
            label: "Time".into(),
            shortcut: String::new(),
            children: vec![MenuNode {
                label: "Paused".into(),
                shortcut: "Ctrl-P".into(),
                children: vec![],
            }],
        });
        let mut u = FlightUi::default();
        u.key("Escape", false, false, false, &tree);
        u.key("ArrowDown", false, false, false, &tree);
        u.key("Enter", false, false, false, &tree);
        assert_eq!(u.path, vec![1]);
        u.key("Escape", false, false, false, &tree);
        assert!(u.menu && u.path.is_empty());
        u.key("p", false, true, false, &tree);
        assert!(!u.frozen());
        assert_eq!(
            u.activate("Radar Cross Section", "Shift-0"),
            Command::Panel(0)
        );
        assert_eq!(u.activate("Back", "F2"), Command::View(3));
        assert_eq!(u.activate("External", "F10"), Command::View(1));
        assert_eq!(u.key("/", true, false, false, &tree), Command::CenterLook);
        assert_eq!(u.key("/", true, true, false, &tree), Command::None);
    }
    #[test]
    fn mouse_requires_same_press_release() {
        let mut u = FlightUi {
            menu: true,
            ..Default::default()
        };
        let t = tree();
        u.pointer(&t, Some((160., 55.)), true);
        assert_eq!(u.pointer(&t, Some((160., 100.)), false), Command::None);
        assert_eq!(u.pointer(&t, Some((160., 55.)), false), Command::None);
        u.pointer(&t, Some((160., 55.)), true);
        assert_eq!(u.pointer(&t, Some((160., 55.)), false), Command::End);
    }
}
