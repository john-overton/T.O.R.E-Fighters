//! Shared multiplayer ? menu. The caller carries out the normal session teardown.
use crate::menu::{Action, Canvas};
use crate::widgets::{Kit, Point, inside};

pub const ITEMS: [&str; 2] = ["Return to Main Menu", "Exit"];
const BAR: (i32, i32, i32, i32) = (80, 38, 24, 20);
const POPUP: (i32, i32, i32, i32) = (80, 58, 216, 46);

#[derive(Default)]
pub struct Menu {
    open: bool,
    selected: usize,
    pressed: Option<usize>,
}

impl Menu {
    pub fn key(&mut self, key: &str) -> Option<Action> {
        if key == "F1" {
            self.open = !self.open;
            self.selected = 0;
            return Some(Action::Click);
        }
        if !self.open {
            return None;
        }
        let action = match key {
            "Escape" => {
                self.open = false;
                Action::Click
            }
            "ArrowDown" | "ArrowUp" | "Tab" => {
                self.selected = 1 - self.selected;
                Action::Click
            }
            "Enter" | "Space" | " " => self.choose(self.selected),
            "m" => self.choose(0),
            "x" => self.choose(1),
            _ => Action::None,
        };
        Some(action)
    }

    fn choose(&mut self, row: usize) -> Action {
        self.open = false;
        if row == 0 {
            Action::MultiplayerMain
        } else {
            Action::Exit
        }
    }

    pub fn pointer(&mut self, point: Option<Point>, down: bool) -> Option<Action> {
        let point = point.unwrap_or((-1, -1));
        if inside(BAR, point) {
            if down {
                self.open = !self.open;
                self.selected = 0;
            }
            return Some(Action::Click);
        }
        if !self.open {
            return None;
        }
        let row =
            inside(POPUP, point).then(|| ((point.1 - POPUP.1 - 3).max(0) / 20).min(1) as usize);
        if down {
            self.pressed = row;
        } else if let Some(row) = row
            && self.pressed.take() == Some(row)
        {
            return Some(self.choose(row));
        }
        if let Some(row) = row {
            self.selected = row;
        } else if down {
            self.open = false;
        }
        Some(Action::None)
    }

    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        if !self.open {
            return;
        }
        crate::ui_text::occlude((80, 58, 219, 49));
        canvas.rect((83, 61, 216, 46), [20, 23, 26, 255]);
        canvas.rect(POPUP, [207, 210, 215, 255]);
        canvas.outline(POPUP, [246, 248, 250, 255]);
        let font = kit.sprite("MENUFONT");
        for (i, item) in ITEMS.iter().enumerate() {
            let y = 62 + i as i32 * 20;
            if i == self.selected {
                canvas.rect((83, y, 210, 20), [242, 245, 249, 255]);
            }
            canvas.text(font, item, 89, y + 3, None);
        }
    }
}

impl crate::App {
    pub(crate) fn multiplayer_menu_active(&self) -> bool {
        matches!(self.screen, crate::Screen::Main | crate::Screen::Quick)
            && (self.direct.screen.is_some()
                || self.internet.screen.is_some()
                || self.lobby.screen.is_some())
    }

    pub(crate) fn multiplayer_main(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.pending_host = None;
        self.pending_join = None;
        self.leave_direct_session(event_loop);
        self.close_lobby();
        self.close_direct();
        self.close_internet();
        self.multiplayer_menu = Menu::default();
        self.screen = crate::Screen::Main;
        self.menu.state.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_and_controller_navigation_reach_both_destinations() {
        let mut menu = Menu::default();
        assert_eq!(menu.key("Enter"), None);
        menu.key("F1");
        assert_eq!(menu.key("Enter"), Some(Action::MultiplayerMain));
        menu.key("F1");
        menu.key("ArrowDown");
        assert_eq!(menu.key("Enter"), Some(Action::Exit));
        menu.key("F1");
        menu.key("Escape");
        assert_eq!(menu.key("x"), None);
        menu.key("F1");
        assert_eq!(menu.key("m"), Some(Action::MultiplayerMain));
        menu.key("F1");
        assert_eq!(menu.key("x"), Some(Action::Exit));
    }
    #[test]
    fn mouse_requires_press_and_release_on_the_same_item() {
        let mut menu = Menu::default();
        menu.pointer(Some((90, 45)), true);
        menu.pointer(Some((100, 67)), true);
        assert_eq!(menu.pointer(Some((100, 87)), false), Some(Action::None));
        menu.pointer(Some((100, 87)), true);
        assert_eq!(menu.pointer(Some((100, 87)), false), Some(Action::Exit));
    }
}
