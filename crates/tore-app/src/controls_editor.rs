//! Authored paused controls editor using the existing raster font/menu canvas.
use crate::{hud::Paint, menu::Canvas};
use tore_formats::font::Font;
use tore_input::profile_text::{action_name, mode_name};
use tore_input::{Action, Axis, Binding, Calibration, Event, Mode, Profile};
use tore_input_native::{Device, Kind};
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResultAction {
    None,
    Changed,
    Save,
    Close,
}
pub struct Editor {
    pub profile: Profile,
    pub devices: Vec<Device>,
    pub capture: bool,
    pub message: String,
    binding: usize,
    focus: usize,
    pressed: Option<(usize, i32)>,
}
fn actions() -> Vec<String> {
    let mut a="pitch roll yaw throttle throttle-rate look-x look-y gear flaps airbrake hook bay engine burner radar jammer autopilot waypoint-autopilot fire weapon-seeker-mode weapon-next designate clear-designation master-arm jettison range-target damage-class fail-station damage-player target-jammer incoming pause menu end-flight restart view-front view-back view-up view-external center-look cockpit hud zoom-in zoom-out range-down range-up radar-mode sensor-channel sensor-infrared sensor-history instrument-next instrument-previous menu-up menu-down menu-left menu-right menu-accept menu-back".split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    for n in 0..10 {
        a.push(format!("page-{n}"));
    }
    for n in 1..=6 {
        a.push(format!("instrument-{n}"));
        for b in 1..=4 {
            a.push(format!("instrument-{n}-control-{b}"));
        }
    }
    for n in 1..=4 {
        a.push(format!("control-{n}"));
    }
    for n in 0..=10 {
        a.push(format!("throttle={}", n as f64 / 10.));
    }
    a
}
fn cycle<T: PartialEq + Clone>(current: &T, items: &[T], delta: i32) -> T {
    let index = items.iter().position(|v| v == current).unwrap_or(0) as i32;
    items[(index + delta).rem_euclid(items.len() as i32) as usize].clone()
}
impl Editor {
    pub fn new(profile: Profile, devices: Vec<Device>) -> Self {
        Self {
            profile,
            devices,
            capture: false,
            message: "Changes apply only with Save & apply. Escape cancels.".into(),
            binding: 0,
            focus: 0,
            pressed: None,
        }
    }
    pub fn cancel_capture(&mut self) {
        self.capture = false;
        self.pressed = None;
    }
    fn modes(b: &Binding) -> Vec<Mode> {
        let mut m = vec![
            Mode::Axis,
            Mode::Unit,
            Mode::Hold(1.),
            Mode::Hold(-1.),
            Mode::Trigger(1.),
            Mode::Trigger(-1.),
            Mode::Press,
            Mode::Release,
            Mode::HoldState,
            Mode::Switch,
            Mode::Follow,
            Mode::Delta,
        ];
        m.extend((-1..=8).map(Mode::Position));
        m.retain(|mode| {
            let mut b = b.clone();
            b.mode = *mode;
            b.calibration = Calibration::default();
            Profile {
                bindings: vec![b],
                ..Default::default()
            }
            .to_text()
            .is_ok()
        });
        m
    }
    fn source_ids(&self) -> Vec<String> {
        let mut ids = vec!["keyboard".into(), "*".into()];
        ids.extend(self.devices.iter().map(|d| d.id.clone()));
        ids.extend(self.profile.aliases.keys().cloned());
        ids
    }
    fn controls(&self, b: &Binding) -> Vec<String> {
        if b.device == "keyboard" {
            return "g f b h e r j ArrowUp ArrowDown ArrowLeft ArrowRight PageUp PageDown z x Space Enter".split_whitespace().map(str::to_owned).collect();
        }
        let id = self.profile.aliases.get(&b.device).unwrap_or(&b.device);
        let mut values = self
            .devices
            .iter()
            .filter(|d| id == "*" || &d.id == id)
            .flat_map(|d| d.controls.iter().map(|c| c.id.clone()))
            .collect::<Vec<_>>();
        if self
            .devices
            .iter()
            .any(|d| (id == "*" || &d.id == id) && d.controls.iter().any(|c| c.id == "button:314"))
        {
            values.extend(
                values
                    .clone()
                    .into_iter()
                    .filter(|c| c != "button:314")
                    .map(|c| format!("button:314+{c}")),
            );
        }
        values.push(b.control.clone());
        values.sort();
        values.dedup();
        if values.is_empty() {
            values.push(b.control.clone());
        }
        values
    }
    fn change(&mut self, row: usize, delta: i32) -> ResultAction {
        if self.capture {
            return ResultAction::None;
        }
        self.focus = row;
        match row {
            0 => self.profile.rumble = !self.profile.rumble,
            1 => {
                if !self.profile.bindings.is_empty() {
                    self.binding = (self.binding as i32 + delta)
                        .rem_euclid(self.profile.bindings.len() as i32)
                        as usize;
                }
            }
            13 => {
                if self.profile.bindings.len() >= 1000 {
                    self.message = "Binding limit reached".into();
                    return ResultAction::None;
                }
                self.profile.bindings.push(Binding {
                    device: "keyboard".into(),
                    control: "unassigned".into(),
                    action: Action::parse("gear").unwrap(),
                    mode: Mode::Press,
                    calibration: Calibration::default(),
                    priority: 10,
                });
                self.binding = self.profile.bindings.len() - 1;
                self.focus = 2;
            }
            14 => {
                if !self.profile.bindings.is_empty() {
                    self.profile.bindings.remove(self.binding);
                    self.binding = self
                        .binding
                        .min(self.profile.bindings.len().saturating_sub(1));
                }
            }
            15 => {
                if !self.profile.bindings.is_empty() {
                    self.capture = true;
                    self.message =
                        "Press a key/button or move an axis. Esc cancels capture.".into();
                }
            }
            16 => return ResultAction::Save,
            17 => self.profile.gamepad_defaults = !self.profile.gamepad_defaults,
            18 => return ResultAction::Close,
            _ => {
                let Some(b) = self.profile.bindings.get(self.binding) else {
                    return ResultAction::None;
                };
                let ids = self.source_ids();
                let controls = self.controls(b);
                let modes = Self::modes(b);
                let b = &mut self.profile.bindings[self.binding];
                let c = &mut b.calibration;
                match row {
                    2 => {
                        b.action =
                            Action::parse(&cycle(&action_name(&b.action), &actions(), delta))
                                .unwrap();
                        b.mode = Self::modes(b)[0];
                    }
                    3 => {
                        b.device = cycle(&b.device, &ids, delta);
                        b.control = "unassigned".into();
                    }
                    4 => b.control = cycle(&b.control, &controls, delta),
                    5 => b.mode = cycle(&b.mode, &modes, delta),
                    6 => c.deadzone = (c.deadzone + delta as f64 * 0.01).clamp(0., 0.9),
                    7 => c.curve = (c.curve + delta as f64 * 0.1).clamp(0.1, 5.),
                    8 => c.scale = if c.scale < 0. { 1. } else { -1. },
                    9 => b.priority = b.priority.saturating_add(delta),
                    10 => c.min = (c.min + delta as f64 * 0.05).min(c.center - 0.05),
                    11 => {
                        c.center = (c.center + delta as f64 * 0.05).clamp(
                            c.min + (c.max - c.min) * 0.01,
                            c.max - (c.max - c.min) * 0.01,
                        )
                    }
                    12 => c.max = (c.max + delta as f64 * 0.05).max(c.center + 0.05),
                    _ => {}
                }
            }
        }
        ResultAction::Changed
    }
    pub fn key(&mut self, key: &str, shift: bool, ctrl: bool, alt: bool) -> ResultAction {
        if key == "Escape" {
            if self.capture {
                self.cancel_capture();
                self.message = "Capture cancelled".into();
                return ResultAction::Changed;
            }
            return ResultAction::Close;
        }
        if self.capture {
            if key.is_empty() || matches!(key, "Shift" | "Control" | "Alt" | "Super") {
                return ResultAction::None;
            }
            self.assign(
                "keyboard".into(),
                format!(
                    "{}{}{}{}",
                    if ctrl { "Ctrl-" } else { "" },
                    if alt { "Alt-" } else { "" },
                    if shift { "Shift-" } else { "" },
                    key
                ),
                Kind::Button,
            );
            return ResultAction::Changed;
        }
        match key {
            "ArrowDown" | "Tab" => {
                self.focus = (self.focus + if shift { 18 } else { 1 }) % 19;
                ResultAction::None
            }
            "ArrowUp" => {
                self.focus = (self.focus + 18) % 19;
                ResultAction::None
            }
            "ArrowLeft" => self.change(self.focus, -1),
            "ArrowRight" | "Enter" | "Space" => self.change(self.focus, 1),
            _ => ResultAction::None,
        }
    }
    fn assign(&mut self, device: String, control: String, kind: Kind) {
        let b = &mut self.profile.bindings[self.binding];
        b.device = device;
        b.control = control;
        b.mode = match b.action {
            Action::Axis(Axis::Throttle) => Mode::Unit,
            Action::Axis(_) => {
                if matches!(kind, Kind::Button) {
                    Mode::Hold(1.)
                } else {
                    Mode::Axis
                }
            }
            Action::Ui(ref name) if name == "fire" => Mode::HoldState,
            _ => Mode::Press,
        };
        self.capture = false;
        self.message =
            "Input assigned; review behavior, then Save & apply. Shared assignments remain.".into();
    }
    pub fn observe(&mut self, event: &Event) -> bool {
        if !self.capture || event.baseline {
            return false;
        }
        let Some(d) = self.devices.iter().find(|d| d.id == event.device) else {
            return false;
        };
        let Some(c) = d.controls.iter().find(|c| c.id == event.control) else {
            return false;
        };
        let moved = match c.kind {
            Kind::Button => event.value > 0.5,
            Kind::Axis => (event.value - c.value).abs() > (c.max - c.min) * 0.3,
            Kind::Relative => event.value != 0.,
            Kind::Position => event.value != c.value,
        };
        if moved {
            let id = if d.id.starts_with("macos-gc-session-") {
                "*".into()
            } else {
                d.id.clone()
            };
            let kind = c.kind.clone();
            self.assign(id, event.control.clone(), kind.clone());
            let b = &mut self.profile.bindings[self.binding];
            if matches!(kind, Kind::Position) && !matches!(b.action, Action::Axis(_)) {
                b.mode = Mode::Position(event.value as i32);
            }
            if matches!(kind, Kind::Relative) && Self::modes(b).contains(&Mode::Delta) {
                b.mode = Mode::Delta;
            }
            if b.device == "*" {
                self.message =
                    "Assigned shared Apple gamepad control; applies to all matching gamepads."
                        .into();
            }
            return true;
        }
        false
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> ResultAction {
        let hit = point.and_then(|(x, y)| {
            if (12. ..628.).contains(&x) && (42. ..403.).contains(&y) {
                Some((((y - 42.) / 19.) as usize, if x < 52. { -1 } else { 1 }))
            } else {
                None
            }
        });
        if down {
            self.pressed = hit;
            return ResultAction::None;
        }
        let old = self.pressed.take();
        if old != hit {
            return ResultAction::None;
        }
        hit.map_or(ResultAction::None, |(r, d)| self.change(r, d))
    }
    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        Canvas(pixels).rect((8, 28, 624, 438), [24, 34, 45, 255]);
        Paint {
            pixels,
            clip: (12, 28, 616, 14),
            color: [240, 233, 194, 255],
        }
        .text(font, "CONTROL MAPPINGS - FLIGHT PAUSED", 16, 30);
        let b = self.profile.bindings.get(self.binding);
        let value =
            |f: fn(&Binding) -> String| b.map(f).unwrap_or_else(|| "(add a binding)".into());
        let labels = vec![
            format!("Rumble: {}", if self.profile.rumble { "On" } else { "Off" }),
            format!(
                "Binding: {} / {}",
                if b.is_some() { self.binding + 1 } else { 0 },
                self.profile.bindings.len()
            ),
            format!("Action: {}", value(|b| action_name(&b.action))),
            format!(
                "Device: {}",
                b.map(|b| {
                    let id = self.profile.aliases.get(&b.device).unwrap_or(&b.device);
                    self.devices
                        .iter()
                        .find(|d| &d.id == id)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| {
                            if id == "*" {
                                "All matching devices".into()
                            } else if id == "keyboard" {
                                "Keyboard".into()
                            } else {
                                format!(
                                    "{} (disconnected)",
                                    id.split('-').take(3).collect::<Vec<_>>().join("-")
                                )
                            }
                        })
                })
                .unwrap_or_default()
            ),
            format!("Input: {}", value(|b| b.control.clone())),
            format!("Behavior: {}", value(|b| mode_name(b.mode))),
            format!(
                "Dead zone: {}",
                value(|b| format!("{:.2}", b.calibration.deadzone))
            ),
            format!(
                "Curve: {}",
                value(|b| format!("{:.1}", b.calibration.curve))
            ),
            format!(
                "Inverted: {}",
                value(|b| (b.calibration.scale < 0.).to_string())
            ),
            format!("Priority: {}", value(|b| b.priority.to_string())),
            format!(
                "Minimum: {}",
                value(|b| format!("{:.2}", b.calibration.min))
            ),
            format!(
                "Center: {}",
                value(|b| format!("{:.2}", b.calibration.center))
            ),
            format!(
                "Maximum: {}",
                value(|b| format!("{:.2}", b.calibration.max))
            ),
            "Add binding".into(),
            "Remove binding (stock keyboard shortcuts remain)".into(),
            if self.capture {
                "Listening... Escape cancels"
            } else {
                "Capture key / button / axis"
            }
            .into(),
            "Save & apply".into(),
            format!(
                "Default mappings for new gamepads: {}",
                if self.profile.gamepad_defaults {
                    "On"
                } else {
                    "Off"
                }
            ),
            "Back / discard unsaved changes".into(),
        ];
        for (row, label) in labels.iter().enumerate() {
            let r = (12, 42 + row as i32 * 19, 616, 18);
            Canvas(pixels).rect(
                r,
                if row == self.focus {
                    [62, 86, 118, 255]
                } else {
                    [201, 210, 222, 255]
                },
            );
            Paint {
                pixels,
                clip: r,
                color: if row == self.focus {
                    [247, 250, 255, 255]
                } else {
                    [20, 39, 65, 255]
                },
            }
            .text(
                font,
                &format!("<  {}", label.chars().take(84).collect::<String>()),
                18,
                r.1 + 5,
            );
        }
        Paint {
            pixels,
            clip: (12, 405, 616, 30),
            color: [240, 233, 194, 255],
        }
        .text(font, &self.message, 16, 410);
        Paint {
            pixels,
            clip: (12, 440, 616, 28),
            color: [240, 233, 194, 255],
        }
        .text(
            font,
            "CONTROLS - Up/Down selects; Left/Right changes; Enter activates",
            16,
            448,
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_ignores_baseline_and_small_axis_motion_then_keeps_direction() {
        let device = Device {
            id: "synthetic".into(),
            name: "Test stick".into(),
            rumble: false,
            controls: vec![tore_input_native::Control {
                id: "x".into(),
                kind: Kind::Axis,
                min: -100.,
                max: 100.,
                value: 0.,
            }],
        };
        let mut e = Editor::new(
            Profile::parse("tore-input 1\nbind synthetic x roll axis").unwrap(),
            vec![device],
        );
        e.change(15, 1);
        let mut event = Event {
            device: "synthetic".into(),
            control: "x".into(),
            value: 100.,
            baseline: true,
        };
        assert!(!e.observe(&event));
        event.baseline = false;
        event.value = 10.;
        assert!(!e.observe(&event));
        event.value = -80.;
        assert!(e.observe(&event));
        assert_eq!(e.profile.bindings[0].mode, Mode::Axis);
        assert!(!e.capture);
    }
    #[test]
    fn draft_capture_and_cancel_never_apply_implicitly() {
        let mut e = Editor::new(Profile::default(), vec![]);
        e.change(13, 1);
        e.change(15, 1);
        e.key("g", true, true, false);
        assert_eq!(e.profile.bindings[0].control, "Ctrl-Shift-g");
        assert_eq!(e.key("Escape", false, false, false), ResultAction::Close);
        assert!(e.profile.to_text().is_ok());
    }
    #[test]
    fn pointer_requires_matching_release_and_rumble_is_a_draft() {
        let mut e = Editor::new(Profile::default(), vec![]);
        e.pointer(Some((100., 45.)), true);
        e.pointer(Some((100., 65.)), false);
        assert!(!e.profile.rumble);
        e.pointer(Some((100., 45.)), true);
        e.pointer(Some((100., 45.)), false);
        assert!(e.profile.rumble);
    }
}
