use crate::{
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
#[derive(Default)]
pub struct QuickMission {
    pub hover: Option<usize>,
    pressed: Option<usize>,
    pub focus: usize,
    pub theaters: bool,
    pub help: bool,
    pub message: bool,
    pub selection: usize,
}
const CONTROLS: [(i32, i32, i32, i32); 5] = [
    (84, 35, 18, 24),
    (103, 35, 95, 24),
    (350, 155, 240, 25),
    (355, 417, 150, 24),
    (510, 417, 80, 24),
];
fn inside(p: (f64, f64), r: (i32, i32, i32, i32)) -> bool {
    p.0 >= r.0 as f64 && p.0 < (r.0 + r.2) as f64 && p.1 >= r.1 as f64 && p.1 < (r.1 + r.3) as f64
}
impl QuickMission {
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        self.hover = p.and_then(|p| {
            if self.help {
                return if inside(p, (84, 60, 170, 25)) {
                    Some(5)
                } else if inside(p, (84, 85, 170, 25)) {
                    Some(6)
                } else if inside(p, CONTROLS[0]) {
                    Some(0)
                } else {
                    None
                };
            }
            if self.theaters {
                return if inside(p, (350, 180, 240, 18 * 16)) {
                    Some(7 + ((p.1 - 180.0) / 18.0) as usize)
                } else if inside(p, CONTROLS[2]) {
                    Some(2)
                } else {
                    None
                };
            }
            CONTROLS.iter().position(|r| inside(p, *r))
        });
    }
    pub fn down(&mut self) {
        self.pressed = self.hover;
    }
    pub fn up(&mut self) -> Action {
        let p = self.pressed.take();
        if let Some(p) = p.filter(|_| p == self.hover) {
            self.activate(p)
        } else {
            Action::None
        }
    }
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.hover = None;
    }
    fn activate(&mut self, i: usize) -> Action {
        match i {
            0 => {
                self.help = !self.help;
                self.theaters = false;
            }
            1 => self.message = true,
            2 => {
                self.theaters = !self.theaters;
                self.help = false;
            }
            3 => return Action::FreeFlight,
            4 | 5 => return Action::Back,
            6 => return Action::Exit,
            7..=22 => {
                self.theaters = false;
                self.selection = i - 7;
                return Action::Theater(self.selection);
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str, shift: bool) -> Action {
        match key {
            "Escape" => {
                if self.help || self.theaters {
                    self.help = false;
                    self.theaters = false;
                } else {
                    return Action::Back;
                }
            }
            "Tab" | "ArrowDown" | "ArrowUp" => {
                let reverse = shift || key == "ArrowUp";
                if self.theaters {
                    self.selection = (self.selection + if reverse { 15 } else { 1 }) % 16;
                    return Action::None;
                }
                self.focus = (self.focus + if reverse { 4 } else { 1 }) % 5;
                self.hover = Some(self.focus);
            }
            "Enter" | " " => {
                return self.activate(if self.theaters {
                    7 + self.selection
                } else if self.help {
                    5
                } else {
                    self.focus
                });
            }
            _ => {}
        }
        Action::None
    }
    pub fn render(&self, pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>, world: &World) {
        pixels.copy_from_slice(&sprites["QUIKMIS3.PIC"].rgba);
        let mut c = Canvas(pixels);
        let font = &sprites["ARMFONT.PIC"];
        let menu = &sprites["MENUFONT.PIC"];
        c.text(menu, "Aircraft", 103, 36, None);
        let start = Camera::for_world(world);
        let altitude = 5000f32.max(world.height(start.position[0], start.position[2]) + 2000.);
        let text = |c: &mut Canvas<'_>, s: &str, x, y| c.text(font, s, x, y, Some([220, 220, 216]));
        text(&mut c, "FRIENDLY SITUATION", 116, 104);
        text(&mut c, "FLIGHT SETUP", 429, 104);
        for (i, line) in [
            "Friendly forces are American.",
            "Wing 1: 1 .... F/A-18D Hornet ....",
            "Wing 2: 0 .... aircraft ....",
            "Wing 3: 0 .... aircraft ....",
            "",
            "You are flying over Ukraine.",
            "You are at 5,000 feet .... clear ....",
            "Your situation is .... free flight ....",
            "",
            "You carry .... no external stores ....",
            "Air combat .... disabled ....",
        ]
        .iter()
        .enumerate()
        {
            text(
                &mut c,
                &line
                    .replace("Ukraine", &world.theater.name)
                    .replace("5,000", &format!("{altitude:.0}")),
                34,
                141 + i as i32 * 16,
            );
        }
        text(&mut c, "Theater", 350, 136);
        c.rect(CONTROLS[2], [102, 106, 103, 255]);
        text(&mut c, &format!("{}   v", world.theater.name), 357, 162);
        text(&mut c, "Weather .... clear / midday ....", 350, 191);
        text(
            &mut c,
            &format!("Source: {}", world.environment.layer),
            350,
            208,
        );
        // Original briefing map inset. This is a temporary selector layout.
        if let Some(map) = sprites.get(&world.theater.map) {
            c.scaled(map, (375, 231, 184, 170));
        }
        for (i, label) in [(3, "Free Flight"), (4, "Cancel")] {
            let r = CONTROLS[i];
            let gain = if self.pressed == Some(i) && self.hover == Some(i) {
                0.8
            } else if self.hover == Some(i) {
                1.15
            } else {
                1.0
            };
            let (x, y) = (
                r.0 + i32::from(self.pressed == Some(i)),
                r.1 + i32::from(self.pressed == Some(i)),
            );
            c.button(sprites, label, (x, y, r.2 + 10), gain);
        }
        if self.message {
            text(&mut c, "Aircraft selection is coming later.", 34, 385);
        }
        if self.theaters {
            c.rect((349, 179, 242, 18 * 16 + 4), [205, 209, 210, 255]);
            for (i, (_, name)) in world.catalog.iter().enumerate() {
                if i == self.selection {
                    c.rect((351, 180 + i as i32 * 18, 238, 18), [140, 160, 148, 255]);
                }
                c.text(font, name, 355, 183 + i as i32 * 18, Some([55, 65, 58]));
            }
        }
        if self.help {
            c.rect((84, 60, 170, 50), [212, 215, 218, 255]);
            c.text(menu, "Back to Main Menu", 89, 64, None);
            c.text(menu, "Exit to Desktop", 89, 89, None);
        }
    }
}
pub fn hud(pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>, camera: &Camera, world: &World) {
    pixels.fill(0);
    let mut c = Canvas(pixels);
    let font = &sprites["SMLFONT.PIC"];
    c.rect((0, 0, WIDTH as i32, 37), [18, 26, 30, 230]);
    c.text(
        font,
        &format!(
            "{}   TERRAIN VIEWER     ALT {:.0} ft",
            world.theater.name.to_ascii_uppercase(),
            camera.position[1]
        ),
        12,
        8,
        Some([230, 234, 222]),
    );
    c.text(
        font,
        "Arrows: move   Shift: fast   Q/E: down/up   WASD: look   Esc: return",
        12,
        24,
        Some([210, 218, 220]),
    );
    c.rect((8, HEIGHT as i32 - 25, 365, 19), [18, 26, 30, 225]);
    c.text(
        font,
        "Retail terrain / midday preview - environment parity in progress",
        13,
        HEIGHT as i32 - 20,
        Some([210, 218, 220]),
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selector_mouse_and_keyboard_reach_all_theaters() {
        let mut q = QuickMission::default();
        for i in 0..16 {
            q.theaters = true;
            q.pointer(Some((370.0, 181.0 + i as f64 * 18.0)));
            q.down();
            assert_eq!(q.up(), Action::Theater(i));
            assert!(!q.theaters);
        }
        q.theaters = true;
        q.key("ArrowDown", false);
        assert_eq!(q.selection, 0);
        q.key("ArrowUp", false);
        assert_eq!(q.key("Enter", false), Action::Theater(15));
    }
    #[test]
    fn activation_requires_matching_release() {
        let mut q = QuickMission::default();
        q.pointer(Some((380.0, 425.0)));
        q.down();
        q.pointer(None);
        assert_eq!(q.up(), Action::None);
        q.pointer(Some((380.0, 425.0)));
        q.down();
        assert_eq!(q.up(), Action::FreeFlight);
    }
    #[test]
    fn escape_dismisses_selector_then_returns() {
        let mut q = QuickMission {
            theaters: true,
            ..Default::default()
        };
        assert_eq!(q.key("Escape", false), Action::None);
        assert_eq!(q.key("Escape", false), Action::Back);
    }
}
