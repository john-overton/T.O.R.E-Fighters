use crate::{
    menu::{Action, Canvas, HEIGHT, Sprite, WIDTH, text_width},
    terrain::{Camera, World},
};
use std::collections::BTreeMap;

type Rect = (i32, i32, i32, i32);
const HELP: usize = 0;
const AIRCRAFT: usize = 1;
const THEATER: usize = 2;
const LAUNCH: usize = 3;
const CANCEL: usize = 4;
const AIRCRAFT_TEXT: usize = 5;
const FIRST_ROW: usize = 8;
const ROW_HEIGHT: i32 = 14;

#[derive(Clone, Copy, PartialEq)]
enum Selector {
    Aircraft,
    Theater,
}

pub struct QuickMission {
    pub hover: Option<usize>,
    pressed: Option<usize>,
    pub focus: usize,
    pub help: bool,
    pub selection: usize,
    pub aircraft_selection: usize,
    pub aircraft_names: Vec<String>,
    selector: Option<Selector>,
    cursor: usize,
    controls: [Rect; 6],
    popup: Rect,
}
impl Default for QuickMission {
    fn default() -> Self {
        Self {
            hover: None,
            pressed: None,
            focus: 0,
            help: false,
            selection: 0,
            aircraft_selection: 0,
            aircraft_names: tore_formats::aircraft::AircraftId::ALL
                .iter()
                .map(|id| id.label().into())
                .collect(),
            selector: None,
            cursor: 0,
            controls: [
                (84, 35, 18, 24),
                (103, 35, 95, 24),
                (0, 0, 0, 0),
                (369, 417, 90, 24),
                (492, 417, 75, 24),
                (0, 0, 0, 0),
            ],
            popup: (0, 0, 0, 0),
        }
    }
}
fn inside(p: (f64, f64), r: Rect) -> bool {
    p.0 >= r.0 as f64 && p.0 < (r.0 + r.2) as f64 && p.1 >= r.1 as f64 && p.1 < (r.1 + r.3) as f64
}
impl QuickMission {
    pub fn for_aircraft(id: tore_formats::aircraft::AircraftId) -> Self {
        Self {
            aircraft_selection: tore_formats::aircraft::AircraftId::ALL
                .iter()
                .position(|item| *item == id)
                .unwrap_or(0),
            ..Self::default()
        }
    }

    fn row_count(&self) -> usize {
        match self.selector {
            Some(Selector::Theater) => tore_formats::theater::THEATERS.len(),
            Some(Selector::Aircraft) => self.aircraft_names.len(),
            None => 0,
        }
    }
    pub fn preview_selector(&mut self, name: &str) -> crate::AppResult<()> {
        match name {
            "normal" => {}
            "aircraft" => self.open(Selector::Aircraft),
            "theaters" => self.open(Selector::Theater),
            "help" => self.help = true,
            _ => {
                return Err(
                    "quick mission snapshot states: normal, aircraft, theaters, help".into(),
                );
            }
        }
        Ok(())
    }
    pub fn pointer(&mut self, p: Option<(f64, f64)>) {
        self.hover = p.and_then(|p| {
            if self.help {
                return if inside(p, (84, 60, 170, 25)) {
                    Some(6)
                } else if inside(p, (84, 85, 170, 25)) {
                    Some(7)
                } else if inside(p, self.controls[HELP]) {
                    Some(HELP)
                } else {
                    None
                };
            }
            if self.selector.is_some() && inside(p, self.popup) {
                let row = ((p.1 - self.popup.1 as f64) / ROW_HEIGHT as f64) as usize;
                return (row < self.row_count()).then_some(FIRST_ROW + row);
            }
            self.controls.iter().position(|r| inside(p, *r))
        });
    }
    pub fn down(&mut self) {
        self.pressed = self.hover;
    }
    pub fn up(&mut self) -> Action {
        let pressed = self.pressed.take();
        if let Some(i) = pressed.filter(|_| pressed == self.hover) {
            self.activate(i)
        } else {
            Action::None
        }
    }
    pub fn cancel(&mut self) {
        self.pressed = None;
        self.hover = None;
        self.selector = None;
        self.help = false;
    }
    fn open(&mut self, selector: Selector) {
        self.selector = if self.selector == Some(selector) {
            None
        } else {
            Some(selector)
        };
        self.help = false;
        self.hover = None;
        self.cursor = match selector {
            Selector::Aircraft => self.aircraft_selection,
            Selector::Theater => self.selection,
        };
        let anchor = self.controls[match selector {
            Selector::Aircraft => AIRCRAFT_TEXT,
            Selector::Theater => THEATER,
        }];
        let height = self.row_count() as i32 * ROW_HEIGHT;
        self.popup = (
            anchor.0.min(380),
            (anchor.1 + anchor.3 + 2).min(462 - height),
            245,
            height,
        );
    }
    fn activate(&mut self, i: usize) -> Action {
        match i {
            HELP => {
                self.help = !self.help;
                self.selector = None;
            }
            AIRCRAFT | AIRCRAFT_TEXT => self.open(Selector::Aircraft),
            THEATER => self.open(Selector::Theater),
            LAUNCH if self.selector.is_none() && !self.help => return Action::FreeFlight,
            CANCEL | 6 => return Action::Back,
            7 => return Action::Exit,
            FIRST_ROW.. => {
                let row = i - FIRST_ROW;
                if row >= self.row_count() {
                    return Action::None;
                }
                let selector = self.selector.take();
                self.hover = None;
                match selector {
                    Some(Selector::Theater) => {
                        self.selection = row;
                        self.focus = THEATER;
                        return Action::Theater(row);
                    }
                    Some(Selector::Aircraft) => {
                        self.aircraft_selection = row;
                        self.focus = AIRCRAFT_TEXT;
                        return Action::Aircraft(row);
                    }
                    None => return Action::None,
                }
            }
            _ => return Action::None,
        }
        Action::Click
    }
    pub fn key(&mut self, key: &str, shift: bool) -> Action {
        match key {
            "Escape" => {
                if self.help || self.selector.is_some() {
                    self.help = false;
                    self.selector = None;
                    self.cancel();
                } else {
                    return Action::Back;
                }
            }
            "Tab" | "ArrowDown" | "ArrowUp" => {
                let reverse = shift || key == "ArrowUp";
                if self.selector.is_some() {
                    let count = self.row_count();
                    self.cursor = (self.cursor + if reverse { count - 1 } else { 1 }) % count;
                    self.hover = None;
                } else if self.help {
                    self.focus = if self.focus == 6 { 7 } else { 6 };
                    self.hover = Some(self.focus);
                } else {
                    self.focus = (self.focus + if reverse { 5 } else { 1 }) % 6;
                    self.hover = Some(self.focus);
                }
            }
            "Enter" | " " => {
                return self.activate(if self.selector.is_some() {
                    FIRST_ROW + self.cursor
                } else if self.help {
                    if self.focus == 7 { 7 } else { 6 }
                } else {
                    self.focus
                });
            }
            _ => {}
        }
        Action::None
    }
    pub fn render(&mut self, pixels: &mut [u8], sprites: &BTreeMap<String, Sprite>, world: &World) {
        pixels.copy_from_slice(&sprites["QUIKMIS3.PIC"].rgba);
        let mut c = Canvas(pixels);
        let font = &sprites["ARMFONT.PIC"];
        let menu = &sprites["MENUFONT.PIC"];
        c.text(menu, "Aircraft", 103, 36, None);
        let mut line = |x: i32, y: i32, parts: &[(&str, Option<usize>)]| {
            let mut x = x;
            for &(text, field) in parts {
                let width = text_width(font, text);
                if let Some(id) = field {
                    let r = (x - 1, y - 1, width + 2, 12);
                    let active = id < self.controls.len();
                    if active {
                        self.controls[id] = r;
                    }
                    let lit = active && self.hover == Some(id);
                    c.rect(
                        r,
                        if lit {
                            [127, 139, 144, 255]
                        } else if active {
                            [101, 107, 109, 255]
                        } else {
                            [81, 84, 85, 255]
                        },
                    );
                }
                let disabled = field.is_some_and(|i| i >= self.controls.len());
                c.text(
                    font,
                    text,
                    x,
                    y,
                    Some(if disabled {
                        [159, 161, 158]
                    } else {
                        [224, 225, 221]
                    }),
                );
                x += width;
            }
        };
        let disabled = Some(usize::MAX);
        let aircraft = self.aircraft_names[self.aircraft_selection].clone();
        line(116, 104, &[("FRIENDLY SITUATION", None)]);
        line(429, 104, &[("ENEMY SITUATION", None)]);
        line(
            34,
            141,
            &[
                ("Friendly forces are ", None),
                (
                    if self.aircraft_selection == 0 {
                        "American"
                    } else {
                        "French"
                    },
                    disabled,
                ),
                (".", None),
            ],
        );
        line(
            34,
            169,
            &[
                ("Wing 1: ", None),
                ("1", disabled),
                (" ", None),
                ("average", disabled),
                (" ", None),
                (&aircraft, Some(AIRCRAFT_TEXT)),
                (".", None),
            ],
        );
        line(
            34,
            183,
            &[
                ("Wing 2: ", None),
                ("0", disabled),
                (" ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        line(
            34,
            197,
            &[
                ("Wing 3: ", None),
                ("0", disabled),
                (" ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        line(
            34,
            226,
            &[
                ("You are flying over ", None),
                (&world.theater.name, Some(THEATER)),
                (".", None),
            ],
        );
        let start = Camera::for_world(world);
        let altitude = 5000f32.max(world.height(start.position[0], start.position[2]) + 2000.);
        line(
            34,
            240,
            &[
                ("You are at ", None),
                (&format!("{altitude:.0} feet"), disabled),
                (". It is ", None),
                ("clear", disabled),
                (".", None),
            ],
        );
        line(
            34,
            254,
            &[
                ("Your situation is ", None),
                ("free flight", disabled),
                (".", None),
            ],
        );
        line(
            34,
            268,
            &[
                ("Enemy forces are ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        line(
            34,
            297,
            &[
                ("You are carrying ", None),
                ("no external stores", disabled),
                (".", None),
            ],
        );
        line(
            34,
            311,
            &[
                ("Air combat is ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        line(
            339,
            141,
            &[
                ("Enemy forces are ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        for y in [169, 183, 197] {
            let wing = format!("Wing {}: ", (y - 169) / 14 + 1);
            line(
                339,
                y,
                &[
                    (&wing, None),
                    ("0", disabled),
                    (" ", None),
                    ("unavailable", disabled),
                    (".", None),
                ],
            );
        }
        line(
            339,
            226,
            &[
                ("Friendly ground target is ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        line(
            339,
            240,
            &[
                ("AAA and SAMs are ", None),
                ("unavailable", disabled),
                (".", None),
            ],
        );
        for (i, label) in [(LAUNCH, "OK"), (CANCEL, "Cancel")] {
            let r = self.controls[i];
            let pressed = self.pressed == Some(i) && self.hover == Some(i);
            let gain = if pressed {
                0.8
            } else if self.hover == Some(i) {
                1.15
            } else {
                1.0
            };
            c.button_style(
                sprites,
                label,
                (r.0 + i32::from(pressed), r.1 + i32::from(pressed), r.2 + 10),
                gain,
                if i == LAUNCH { "ACTDFT0" } else { "ACTION0" },
            );
        }
        if let Some(selector) = self.selector {
            let r = self.popup;
            c.rect((r.0 - 2, r.1 - 2, r.2 + 4, r.3 + 4), [196, 201, 203, 255]);
            let names: Vec<&str> = match selector {
                Selector::Aircraft => self.aircraft_names.iter().map(String::as_str).collect(),
                Selector::Theater => world
                    .catalog
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect(),
            };
            for (i, name) in names.iter().enumerate() {
                let y = r.1 + i as i32 * ROW_HEIGHT;
                if self.hover == Some(FIRST_ROW + i) || self.hover.is_none() && i == self.cursor {
                    c.rect((r.0, y, r.2, ROW_HEIGHT), [140, 160, 148, 255]);
                }
                c.text(font, name, r.0 + 4, y + 2, Some([48, 58, 54]));
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
    fn setup() -> QuickMission {
        let mut q = QuickMission::default();
        q.controls[THEATER] = (165, 225, 75, 12);
        q.controls[AIRCRAFT_TEXT] = (150, 168, 100, 12);
        q
    }
    #[test]
    fn selectors_require_matching_release_and_reach_every_item() {
        let mut q = setup();
        for index in 0..16 {
            q.activate(THEATER);
            q.pointer(Some((
                q.popup.0 as f64 + 5.,
                q.popup.1 as f64 + index as f64 * 14. + 3.,
            )));
            q.down();
            assert_eq!(q.up(), Action::Theater(index));
            assert!(q.selector.is_none());
        }
        q.activate(AIRCRAFT_TEXT);
        q.pointer(Some((155., q.popup.1 as f64 + 17.)));
        q.down();
        q.pointer(None);
        assert_eq!(q.up(), Action::None);
        assert_eq!(q.aircraft_selection, 0);
        q.pointer(Some((155., q.popup.1 as f64 + 17.)));
        q.down();
        assert_eq!(q.up(), Action::Aircraft(1));
    }
    #[test]
    fn keyboard_cursor_does_not_commit_until_enter_and_escape_cancels() {
        let mut q = setup();
        q.selection = 15;
        q.activate(THEATER);
        q.key("ArrowDown", false);
        assert_eq!(q.selection, 15);
        assert_eq!(q.key("Enter", false), Action::Theater(0));
        q.activate(AIRCRAFT);
        q.key("ArrowDown", false);
        assert_eq!(q.key("Escape", false), Action::None);
        assert_eq!(q.aircraft_selection, 0);
        assert_eq!(q.key("Escape", false), Action::Back);
        q.activate(AIRCRAFT);
        q.key("ArrowUp", false);
        assert_eq!(q.key("Enter", false), Action::Aircraft(1));
    }
    #[test]
    fn opponents_are_inert_and_open_selector_cannot_launch_flight() {
        let mut q = setup();
        for point in [(400., 145.), (480., 170.), (360., 242.)] {
            q.pointer(Some(point));
            q.down();
            assert_eq!(q.up(), Action::None);
        }
        q.activate(THEATER);
        assert_eq!(q.activate(LAUNCH), Action::None);
        q.key("Escape", false);
        q.pointer(Some((400., 425.)));
        q.down();
        assert_eq!(q.up(), Action::FreeFlight);
        q.activate(THEATER);
        q.cancel();
        assert!(q.selector.is_none());
        assert_eq!(q.up(), Action::None);
    }
}
