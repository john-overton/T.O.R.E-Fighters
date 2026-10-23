//! Session departure wording and the shared confirmation for every exit destination.
//! Opinionated presentation: docs/spec/multiplayer-menus.md.
use crate::lobby_screen::modal::{Answer, Modal};
use crate::menu::{Action, Canvas, HEIGHT, WIDTH};
use crate::{App, ui_text};
use tore_session::wire::messages::LobbyState;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;

/// Hosting is the running local host, not the crown or the role at initial join.
/// Readiness comes from the same session flag used by hand_over_for_leave.
pub(crate) fn lines(lobby: Option<&LobbyState>, hosting: bool, ready: bool) -> Vec<String> {
    let effect = if hosting {
        if ready {
            Some("Another player will take over hosting.".to_owned())
        } else if lobby.is_some_and(|l| l.players.iter().any(|p| p.id != l.you)) {
            Some("Leaving ends the game for everyone.".to_owned())
        } else {
            Some("Leaving ends the game.".to_owned())
        }
    } else {
        lobby.filter(|l| l.is_king()).and_then(|l| {
            l.players
                .iter()
                .find(|p| p.id != l.you)
                .map(|p| format!("The crown passes to {}.", p.callsign))
        })
    };
    effect
        .into_iter()
        .chain(Some("Are you sure?".to_owned()))
        .collect()
}

pub(crate) struct Dialog {
    action: Action,
    lines: Vec<String>,
    panel: Modal,
}

impl Dialog {
    fn new(action: Action, lines: Vec<String>) -> Self {
        Self {
            action,
            panel: Modal::leave(lines.clone()),
            lines,
        }
    }

    fn refresh(&mut self, lines: Vec<String>) -> bool {
        if self.lines == lines {
            return false;
        }
        *self = Self::new(self.action.clone(), lines);
        true
    }
}

impl App {
    fn departure_lines(&self) -> Option<Vec<String>> {
        self.net
            .as_ref()
            .map(|s| lines(s.client.lobby(), s.hosting(), !s.leaving_ends_game()))
    }

    pub(crate) fn confirm_session_leave(&mut self, action: &Action) -> bool {
        if self.leave_approved.take().as_ref() == Some(action) {
            return false;
        }
        let disconnecting_flight = *action == Action::Back
            && self.screen == crate::Screen::Flight
            && self.net_flight.is_some()
            && self
                .net
                .as_ref()
                .is_some_and(|s| !s.hosting() && !s.lobby_screen);
        if !disconnecting_flight
            && !matches!(
                action,
                Action::Exit | Action::DirectLeave | Action::MultiplayerMain | Action::AwayLeave
            )
        {
            return false;
        }
        let Some(lines) = self.departure_lines() else {
            return false;
        };
        if self.leave_dialog.is_some() {
            return true;
        }
        // Command-line joins may not have opened a lounge and built its kit.
        if self.direct.kit().is_none() {
            match self.menu.kit_source.build("MODEM3") {
                Ok(kit) => self.direct.keep_kit(std::sync::Arc::new(kit)),
                Err(error) => {
                    self.message(format!("Cannot show leave confirmation: {error}"));
                    return true;
                }
            }
        }
        log::info!("Leave confirmation: {}", lines.join(" "));
        self.leave_dialog = Some(Dialog::new(action.clone(), lines));
        self.input.context(true, self.focused);
        if self.screen == crate::Screen::Flight {
            self.release_trigger();
        }
        self.paint_leave_dialog();
        true
    }

    /// A change of host, standby readiness or crown resets to Cancel. The player
    /// must approve the new consequence, even if it changed just before Enter.
    fn refresh_leave_dialog(&mut self) -> bool {
        let Some(dialog) = &mut self.leave_dialog else {
            return false;
        };
        let Some(session) = &self.net else {
            self.leave_dialog = None;
            self.paint_leave_dialog();
            return true;
        };
        let lines = lines(
            session.client.lobby(),
            session.hosting(),
            !session.leaving_ends_game(),
        );
        if !dialog.refresh(lines) {
            return false;
        }
        log::info!("Leave confirmation updated: {}", dialog.lines.join(" "));
        self.paint_leave_dialog();
        true
    }

    fn paint_leave_dialog(&mut self) {
        let pixels = self.leave_dialog.as_ref().and_then(|dialog| {
            let kit = self.direct.kit()?;
            let mut pixels = vec![0; WIDTH * HEIGHT * 4];
            ui_text::begin();
            dialog.panel.draw(&mut Canvas(&mut pixels), &kit);
            let layer = ui_text::finish();
            Some((pixels, layer))
        });
        if let Some(renderer) = &mut self.renderer {
            renderer.set_leave_dialog(
                pixels
                    .as_ref()
                    .map(|(pixels, layer)| (pixels.as_slice(), layer.as_ref())),
            );
            renderer.window.set_cursor_visible(true);
            renderer.window.request_redraw();
        }
    }

    fn leave_dialog_answer(&mut self, answer: Answer) -> Action {
        let action = match answer {
            Answer::None => Action::None,
            Answer::Cancel => {
                self.leave_dialog = None;
                Action::Click
            }
            Answer::Ok(_) => {
                let action = self.leave_dialog.take().expect("open confirmation").action;
                self.leave_approved = Some(action.clone());
                action
            }
        };
        self.paint_leave_dialog();
        action
    }

    pub(crate) fn leave_dialog_key(&mut self, key: &str) -> Action {
        if self.refresh_leave_dialog() {
            return Action::None;
        }
        let Some(dialog) = &mut self.leave_dialog else {
            return Action::None;
        };
        let answer = dialog.panel.key(key, self.modifiers.shift_key());
        self.leave_dialog_answer(answer)
    }

    /// Consume modal input before flight, observer and lobby handlers see it.
    pub(crate) fn leave_dialog_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: &WindowEvent,
    ) -> bool {
        if self.leave_dialog.is_none() {
            return false;
        }
        if self.refresh_leave_dialog() {
            return !matches!(event, WindowEvent::RedrawRequested);
        }
        let action = match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return true;
                }
                let key = match &event.logical_key {
                    Key::Named(key) => format!("{key:?}"),
                    Key::Character(key) => key.to_string(),
                    _ => String::new(),
                };
                self.leave_dialog_key(&key)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = Some((position.x, position.y));
                let point = self
                    .renderer
                    .as_ref()
                    .and_then(|r| r.viewport().point(position.x, position.y))
                    .map(|(x, y)| (x as i32, y as i32));
                self.leave_dialog.as_mut().unwrap().panel.moved(point);
                self.paint_leave_dialog();
                Action::None
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                let point = self
                    .renderer
                    .as_ref()
                    .and_then(|r| self.pointer.and_then(|(x, y)| r.viewport().point(x, y)))
                    .map(|(x, y)| (x as i32, y as i32));
                let kit = self.direct.kit().expect("confirmation kit");
                let answer = self.leave_dialog.as_mut().unwrap().panel.button(
                    &kit,
                    point,
                    *state == ElementState::Pressed,
                );
                self.leave_dialog_answer(answer)
            }
            WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::CloseRequested => Action::None,
            _ => return false,
        };
        self.action(event_loop, action);
        true
    }

    pub(crate) fn leave_dialog_snapshot(&self, pixels: &mut [u8]) {
        if let (Some(dialog), Some(kit)) = (&self.leave_dialog, self.direct.kit()) {
            dialog.panel.draw(&mut Canvas(pixels), &kit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_session::wire::messages::LobbyPhase;

    #[test]
    fn every_role_has_the_same_departure_consequence_in_every_phase() {
        // Revival wait is Flying with no occupied aircraft, debrief is Ended.
        for (phase, revival) in [
            (LobbyPhase::Lobby, false),
            (LobbyPhase::Flying, false),
            (LobbyPhase::Flying, true),
            (LobbyPhase::Ended, false),
        ] {
            for dedicated in [false, true] {
                let mut lobby = crate::lobby_screen::preview::sample(2);
                lobby.phase = phase;
                if dedicated {
                    lobby.host = None;
                }
                if revival {
                    lobby.players[1].slot = None;
                }
                assert_eq!(lines(Some(&lobby), false, false), ["Are you sure?"]);
                lobby.players[1].observing = true;
                assert_eq!(lines(Some(&lobby), false, true), ["Are you sure?"]);
                lobby.king = Some(2);
                assert_eq!(
                    lines(Some(&lobby), false, false),
                    ["The crown passes to Maverick.", "Are you sure?"]
                );
                lobby.players.retain(|p| p.id == 2);
                assert_eq!(lines(Some(&lobby), false, false), ["Are you sure?"]);
            }
            let mut lobby = crate::lobby_screen::preview::sample(1);
            lobby.phase = phase;
            // The actual host warns even when the crown belongs to someone else.
            for king in [Some(1), Some(2)] {
                lobby.king = king;
                assert_eq!(
                    lines(Some(&lobby), true, false),
                    ["Leaving ends the game for everyone.", "Are you sure?"]
                );
                assert_eq!(
                    lines(Some(&lobby), true, true),
                    ["Another player will take over hosting.", "Are you sure?"]
                );
            }
            lobby.players.retain(|p| p.id == 1);
            assert_eq!(
                lines(Some(&lobby), true, false),
                ["Leaving ends the game.", "Are you sure?"]
            );
        }
        assert_eq!(lines(None, false, false), ["Are you sure?"]);
        assert_eq!(
            lines(None, true, false),
            ["Leaving ends the game.", "Are you sure?"]
        );
    }

    #[test]
    fn crown_uses_connection_order_and_never_a_vacant_id() {
        let mut lobby = crate::lobby_screen::preview::sample(1);
        lobby.host = None;
        lobby.players[1].id = 29;
        assert_eq!(
            lines(Some(&lobby), false, false),
            ["The crown passes to Goose.", "Are you sure?"]
        );
    }

    #[test]
    fn all_destinations_cancel_by_default_and_refresh_before_accepting_a_changed_role() {
        for action in [
            Action::DirectLeave,
            Action::MultiplayerMain,
            Action::Exit,
            Action::AwayLeave,
            Action::Back,
        ] {
            let mut dialog = Dialog::new(action.clone(), vec!["Are you sure?".into()]);
            assert_eq!(dialog.panel.lines(), ["Are you sure?"]);
            assert_eq!(dialog.panel.key("Enter", false), Answer::Cancel);
            dialog.panel.key("Tab", false);
            // A migration or lost standby changes the consequence while Leave is selected.
            assert!(dialog.refresh(vec![
                "Leaving ends the game for everyone.".into(),
                "Are you sure?".into()
            ]));
            assert_eq!(dialog.action, action);
            assert_eq!(dialog.panel.key("Enter", false), Answer::Cancel);
            dialog.panel.key("ArrowLeft", false);
            assert!(!dialog.refresh(dialog.lines.clone()));
            assert_eq!(dialog.panel.key("Enter", false), Answer::Ok(String::new()));
            assert_eq!(dialog.panel.key("Escape", false), Answer::Cancel);
        }
    }
}
