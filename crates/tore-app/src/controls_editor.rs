//! The input configuration screen, shared by Pref > Controls on the main menu
//! and Escape > Control in flight. Devices are listed on the left; the right
//! pane shows the selected device's settings above its action mappings,
//! grouped as in `input_catalog`. It edits a draft profile that is applied
//! only by Apply. Layout and colours are an opinionated agent design after
//! John's 2026-09-22 mockup, drawn with the imported raster font.
use crate::input_catalog::{self as catalog, ENTRIES, Entry, Group, Kind as Row};
use crate::{hud::Paint, menu::Canvas};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::font::Font;
use tore_input::profile_text::action_name;
use tore_input::{Action, Axis, Binding, Calibration, Event, Mode, Profile};
use tore_input_native::{Device, Kind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResultAction {
    None,
    Changed,
    Save,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    Gamepad,
    Stick,
    Throttle,
    Pedals,
    HeadTracker,
    Other,
}
impl DeviceClass {
    pub fn title(self) -> &'static str {
        match self {
            DeviceClass::Gamepad => "Controller",
            DeviceClass::Stick => "Flight stick",
            DeviceClass::Throttle => "Throttle",
            DeviceClass::Pedals => "Pedals",
            DeviceClass::HeadTracker => "Head tracker",
            DeviceClass::Other => "Device",
        }
    }
}
/// Fitted device classification from the name and exposed controls; it only
/// chooses a tab title and which settings are offered.
pub fn classify(device: &Device) -> DeviceClass {
    let name = device.name.to_ascii_lowercase();
    let has = |id: &str| device.controls.iter().any(|c| c.id == id);
    if name.contains("pedal") || name.contains("rudder") {
        DeviceClass::Pedals
    } else if name.contains("headpose")
        || name.contains("opentrack")
        || name.contains("trackir")
        || name.contains("head track")
    {
        DeviceClass::HeadTracker
    } else if name.contains("throttle") {
        DeviceClass::Throttle
    } else if device.id.starts_with("macos-gc-session-")
        || (has("button:304") && has("axis:0") && has("axis:1"))
        || ["gamepad", "controller", "xbox", "dualsense", "dualshock"]
            .iter()
            .any(|n| name.contains(n))
    {
        DeviceClass::Gamepad
    } else if device.controls.iter().any(|c| matches!(c.kind, Kind::Axis)) {
        DeviceClass::Stick
    } else {
        DeviceClass::Other
    }
}

/// A standard Xbox-layout gamepad as Linux reports it, for snapshots and tests.
pub fn preview_device() -> Device {
    let axes = [
        ("axis:0", -32768., 32767.),
        ("axis:1", -32768., 32767.),
        ("axis:2", 0., 1023.),
        ("axis:3", -32768., 32767.),
        ("axis:4", -32768., 32767.),
        ("axis:5", 0., 1023.),
        ("axis:16", -1., 1.),
        ("axis:17", -1., 1.),
    ];
    let buttons = [304, 305, 307, 308, 310, 311, 314, 315, 316, 317, 318];
    Device {
        id: "linux-045e-0b12-preview".into(),
        name: "Xbox Wireless Controller".into(),
        rumble: true,
        controls: axes
            .iter()
            .map(|(id, min, max)| tore_input_native::Control {
                id: (*id).into(),
                kind: Kind::Axis,
                min: *min,
                max: *max,
                value: if *min == 0. { 0. } else { (min + max) / 2. },
            })
            .chain(buttons.iter().map(|b| tore_input_native::Control {
                id: format!("button:{b}"),
                kind: Kind::Button,
                min: 0.,
                max: 1.,
                value: 0.,
            }))
            .collect(),
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Tab {
    Keyboard,
    Mouse,
    Head,
    /// A native device identity, connected or referenced by the profile.
    Device(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Setting {
    Info(&'static str),
    Status,
    Rumble,
    GamepadDefaults,
    Modifiers,
    Deadzone,
    StickSensitivity,
    TriggerSensitivity,
    MouseLook,
    MouseSensitivity,
    MouseInvert,
    HeadSource,
    HeadYaw,
    HeadPitch,
    HeadInvertYaw,
    HeadInvertPitch,
}
impl Setting {
    fn focusable(self) -> bool {
        !matches!(self, Setting::Info(_) | Setting::Status)
    }
    fn toggle(self) -> bool {
        matches!(
            self,
            Setting::Rumble
                | Setting::GamepadDefaults
                | Setting::MouseLook
                | Setting::MouseInvert
                | Setting::HeadSource
                | Setting::HeadInvertYaw
                | Setting::HeadInvertPitch
        )
    }
    /// Stepped values show arrows and take left/right. Toggles flip on any
    /// activation; Modifiers starts a capture only when activated, so the
    /// D-pad can move past it without starting one.
    fn arrows(self) -> bool {
        !self.toggle() && self != Setting::Modifiers
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Line {
    /// `None` is the group of profile bindings the catalog does not list.
    Header(Option<Group>),
    Entry(usize),
    Other(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Focus {
    Tab(usize),
    Setting(usize),
    /// Visible mapping line and column: 0 primary, 1 secondary, 2 invert,
    /// 3 curve, 4 clear.
    Cell(usize, usize),
    Footer(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hit {
    Tab(usize),
    Setting(usize, i32),
    Cell(usize, usize),
    Footer(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum Source {
    Stock(&'static str),
    Binding(usize),
}

#[derive(Clone, Debug, PartialEq)]
enum Target {
    Slot(usize, usize),
    Modifier,
}

struct Capture {
    target: Target,
    /// Control values when capture began, so small drifts are ignored.
    rest: BTreeMap<String, f64>,
}

type Rect = (i32, i32, i32, i32);
const LEFT: Rect = (6, 24, 150, 412);
const TAB_TOP: i32 = 42;
const TAB_HEIGHT: i32 = 34;
const RIGHT_X: i32 = 162;
const RIGHT_W: i32 = 472;
const SETTINGS_TOP: i32 = 42;
const SETTING_HEIGHT: i32 = 15;
const MAX_SETTINGS: usize = 6;
const MAP_TOP: i32 = 144;
const LIST_TOP: i32 = 178;
const LINE_HEIGHT: i32 = 14;
const LIST_LINES: usize = 18;
const FOOTER_Y: i32 = 456;
const FOOTER: [&str; 3] = ["Apply", "Reset device", "Back"];
const COLUMNS: [(i32, i32); 5] = [(158, 104), (264, 104), (372, 16), (390, 34), (428, 40)];

const INK: [u8; 4] = [20, 39, 65, 255];
const PAPER: [u8; 4] = [24, 34, 45, 255];
const PANEL: [u8; 4] = [36, 52, 72, 255];
const TITLE: [u8; 4] = [240, 233, 194, 255];
const PALE: [u8; 4] = [201, 210, 222, 255];
const PALE_ALT: [u8; 4] = [190, 200, 214, 255];
const GROUP: [u8; 4] = [150, 166, 188, 255];
const FOCUS: [u8; 4] = [62, 86, 118, 255];
const WHITE: [u8; 4] = [247, 250, 255, 255];
const MUTED: [u8; 4] = [150, 162, 178, 255];
const GOOD: [u8; 4] = [120, 214, 140, 255];
const BUTTON: [u8; 4] = [98, 118, 146, 255];
const HEADER: [u8; 4] = [44, 64, 88, 255];

pub struct Editor {
    pub profile: Profile,
    pub devices: Vec<Device>,
    pub message: String,
    /// Shown at the top right: where the screen was opened from.
    pub context: &'static str,
    /// Head-tracker receiver status, refreshed by the host each poll.
    pub head_status: String,
    capture: Option<Capture>,
    tab: usize,
    focus: Focus,
    scroll: usize,
    collapsed: BTreeSet<Option<Group>>,
    pressed: Option<Hit>,
    /// Latest raw value of every native control seen, for held modifiers.
    held: BTreeMap<(String, String), f64>,
    dirty: bool,
}

fn norm(control: &tore_input_native::Control, value: f64) -> f64 {
    if matches!(control.kind, Kind::Axis) && control.max > control.min {
        ((value - control.min) / (control.max - control.min) * 2. - 1.).clamp(-1., 1.)
    } else {
        value
    }
}
/// Linux hats are integer axes from -1 to 1.
fn is_hat(control: &tore_input_native::Control) -> bool {
    matches!(control.kind, Kind::Position)
        || (matches!(control.kind, Kind::Axis) && control.min == -1. && control.max == 1.)
}
fn text_width(font: &Font, text: &str) -> i32 {
    text.bytes()
        .map(|c| font.glyphs[c as usize].advance as i32)
        .sum()
}
fn fit(font: &Font, text: &str, width: i32) -> String {
    if text_width(font, text) <= width {
        return text.into();
    }
    let advance = |c: u8| font.glyphs[c as usize].advance as i32;
    let mut out = String::new();
    let mut used = 2 * advance(b'.');
    for c in text.bytes() {
        used += advance(c);
        if used > width {
            break;
        }
        out.push(c as char);
    }
    out + ".."
}
fn inside(p: (f64, f64), r: Rect) -> bool {
    p.0 >= r.0 as f64 && p.1 >= r.1 as f64 && p.0 < (r.0 + r.2) as f64 && p.1 < (r.1 + r.3) as f64
}
/// Removal order that keeps the remaining binding indices valid.
fn highest_first(sources: &mut [Source]) {
    sources.sort_by_key(|s| match s {
        Source::Binding(i) => std::cmp::Reverse(*i),
        Source::Stock(_) => std::cmp::Reverse(usize::MAX),
    });
}

impl Editor {
    pub fn new(profile: Profile, devices: Vec<Device>, context: &'static str) -> Self {
        let mut editor = Self {
            profile,
            devices,
            message: "Select a device, then click an input to change it.".into(),
            context,
            head_status: String::new(),
            capture: None,
            tab: 0,
            focus: Focus::Tab(0),
            scroll: 0,
            collapsed: BTreeSet::new(),
            pressed: None,
            held: BTreeMap::new(),
            dirty: false,
        };
        // Older chord profiles name their modifiers only inside bindings.
        let implied: Vec<(String, String)> = editor
            .profile
            .bindings
            .iter()
            .flat_map(|b| {
                let (mods, _) = tore_input::chord_parts(&b.control);
                mods.into_iter()
                    .map(|m| (b.device.clone(), m.to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect();
        for m in implied {
            if !editor.profile.modifiers.contains(&m) {
                editor.profile.modifiers.push(m);
            }
        }
        // Open on the first connected device: it is what players come to set up.
        if !editor.devices.is_empty() {
            editor.tab = 2;
        }
        editor.focus = Focus::Tab(editor.tab);
        editor
    }
    /// Snapshot helper: `""` keeps the opening tab; `keyboard`, `mouse` and
    /// `head` select those tabs.
    pub fn preview(&mut self, tab: &str) -> Result<(), String> {
        let tabs = self.tabs();
        self.tab = match tab {
            "" => self.tab,
            "keyboard" => 0,
            "mouse" => 1,
            "head" => tabs.len() - 1,
            _ => return Err("controls snapshot tab must be keyboard, mouse or head".into()),
        };
        self.focus = Focus::Tab(self.tab);
        Ok(())
    }
    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }
    pub fn cancel_capture(&mut self) {
        self.capture = None;
        self.pressed = None;
    }
    pub fn saved(&mut self) {
        self.dirty = false;
    }

    // ---- model ----------------------------------------------------------

    fn tabs(&self) -> Vec<Tab> {
        let mut tabs = vec![Tab::Keyboard, Tab::Mouse];
        let mut ids: Vec<String> = self.devices.iter().map(|d| d.id.clone()).collect();
        for b in &self.profile.bindings {
            let id = self.profile.aliases.get(&b.device).unwrap_or(&b.device);
            if !matches!(id.as_str(), "keyboard" | "mouse" | "*") && !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        tabs.extend(ids.into_iter().map(Tab::Device));
        tabs.push(Tab::Head);
        tabs
    }
    fn current(&self) -> Tab {
        let tabs = self.tabs();
        tabs[self.tab.min(tabs.len() - 1)].clone()
    }
    fn device(&self, id: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
    fn class(&self, id: &str) -> DeviceClass {
        self.device(id).map_or(DeviceClass::Other, classify)
    }
    /// The device reference new bindings use: an existing alias, `*` for
    /// session-only Apple gamepads, otherwise the identity.
    fn reference(&self, id: &str) -> String {
        if let Some((alias, _)) = self.profile.aliases.iter().find(|(_, v)| *v == id) {
            return alias.clone();
        }
        if id.starts_with("macos-gc-session-") {
            return "*".into();
        }
        id.into()
    }
    fn names_device(&self, reference: &str, id: &str) -> bool {
        reference == id
            || self.profile.aliases.get(reference).is_some_and(|v| v == id)
            || (reference == "*"
                && (id.starts_with("macos-gc-session-") || self.class(id) == DeviceClass::Gamepad))
    }
    fn on_tab(&self, binding: &Binding, tab: &Tab) -> bool {
        match tab {
            Tab::Keyboard => binding.device == "keyboard",
            Tab::Mouse => binding.device == "mouse",
            Tab::Head => false,
            Tab::Device(id) => self.names_device(&binding.device, id),
        }
    }
    /// Xbox control names apply to gamepads and to disconnected Linux pads.
    fn gamepad_tab(&self, tab: &Tab) -> bool {
        matches!(tab, Tab::Device(id) if self.class(id) == DeviceClass::Gamepad
            || (id.starts_with("linux-") && self.device(id).is_none()))
    }
    fn stock(&self, entry: &Entry, tab: &Tab) -> Vec<&'static str> {
        let (device, stock) = match tab {
            Tab::Keyboard => ("keyboard", entry.keys),
            Tab::Mouse => ("mouse", entry.mouse),
            _ => return vec![],
        };
        stock
            .iter()
            .copied()
            .filter(|k| {
                !self
                    .profile
                    .disabled
                    .contains(&(device.to_owned(), (*k).to_owned()))
            })
            .collect()
    }
    fn slots(&self, index: usize, tab: &Tab) -> Vec<Source> {
        let entry = &ENTRIES[index];
        let mut slots: Vec<Source> = self
            .stock(entry, tab)
            .into_iter()
            .map(Source::Stock)
            .collect();
        if entry.bindable() {
            slots.extend(
                self.profile
                    .bindings
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| self.on_tab(b, tab) && entry.matches(b))
                    .map(|(i, _)| Source::Binding(i)),
            );
        }
        slots
    }
    fn source_label(&self, source: &Source, tab: &Tab) -> String {
        match source {
            Source::Stock(key) => match tab {
                Tab::Mouse => catalog::mouse_label(key),
                _ => catalog::key_label(key),
            },
            Source::Binding(i) => {
                let b = &self.profile.bindings[*i];
                catalog::control_label(&b.device, &b.control, b.mode, self.gamepad_tab(tab))
            }
        }
    }
    fn shown(&self, entry: &Entry, tab: &Tab) -> bool {
        match tab {
            Tab::Keyboard => entry.digital() && !catalog::menu_only(entry),
            Tab::Mouse => entry.bindable() && entry.digital(),
            Tab::Head => false,
            Tab::Device(_) => entry.bindable(),
        }
    }
    fn lines(&self) -> Vec<Line> {
        let tab = self.current();
        let mut lines = vec![];
        for group in Group::ALL {
            let entries: Vec<usize> = ENTRIES
                .iter()
                .enumerate()
                .filter(|(_, e)| e.group == group && self.shown(e, &tab))
                .map(|(i, _)| i)
                .collect();
            if entries.is_empty() {
                continue;
            }
            lines.push(Line::Header(Some(group)));
            if !self.collapsed.contains(&Some(group)) {
                lines.extend(entries.into_iter().map(Line::Entry));
            }
        }
        let others: Vec<usize> = self
            .profile
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                self.on_tab(b, &tab) && !ENTRIES.iter().any(|e| self.shown(e, &tab) && e.matches(b))
            })
            .map(|(i, _)| i)
            .collect();
        if !others.is_empty() {
            lines.push(Line::Header(None));
            if !self.collapsed.contains(&None) {
                lines.extend(others.into_iter().map(Line::Other));
            }
        }
        lines
    }
    fn settings(&self) -> Vec<Setting> {
        match self.current() {
            Tab::Keyboard => vec![
                Setting::Info("Stock keys are listed first. Change or clear any of them."),
                Setting::Info("Hold Ctrl, Alt or Shift, in any mix, while capturing a key."),
                Setting::Info("A key taken for one action is removed from its old action."),
                Setting::Info("Esc, Alt+Enter and Alt+F4 always keep their stock meaning."),
            ],
            Tab::Mouse => vec![
                Setting::MouseLook,
                Setting::MouseSensitivity,
                Setting::MouseInvert,
                Setting::Info("Left click always operates instruments and the HUD."),
            ],
            Tab::Head => vec![
                Setting::HeadSource,
                Setting::HeadYaw,
                Setting::HeadPitch,
                Setting::HeadInvertYaw,
                Setting::HeadInvertPitch,
                Setting::Status,
            ],
            tab @ Tab::Device(_) => {
                let mut s = vec![];
                if let Tab::Device(id) = &tab
                    && self.device(id).is_some_and(|d| d.rumble)
                {
                    s.push(Setting::Rumble);
                }
                s.push(Setting::Modifiers);
                s.push(Setting::Deadzone);
                s.push(Setting::StickSensitivity);
                if self.gamepad_tab(&tab) {
                    s.push(Setting::TriggerSensitivity);
                    s.push(Setting::GamepadDefaults);
                }
                s.truncate(MAX_SETTINGS);
                s
            }
        }
    }
    fn modifiers(&self, tab: &Tab) -> Vec<usize> {
        self.profile
            .modifiers
            .iter()
            .enumerate()
            .filter(|(_, (d, _))| matches!(tab, Tab::Device(id) if self.names_device(d, id)))
            .map(|(i, _)| i)
            .collect()
    }
    /// Bindings on this device whose calibration a device-wide setting drives.
    fn calibrated(&self, tab: &Tab, trigger: bool) -> Vec<usize> {
        self.profile
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                self.on_tab(b, tab)
                    && if trigger {
                        matches!(b.mode, Mode::Trigger(_))
                    } else {
                        b.mode == Mode::Axis
                            && !matches!(b.action, Action::Axis(Axis::HeadYaw | Axis::HeadPitch))
                    }
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn deadzone(&self, tab: &Tab) -> f64 {
        let mut all = self.calibrated(tab, false);
        all.extend(self.calibrated(tab, true));
        all.first()
            .map_or(0.1, |i| self.profile.bindings[*i].calibration.deadzone)
    }
    fn sensitivity(&self, tab: &Tab, trigger: bool) -> f64 {
        self.calibrated(tab, trigger)
            .first()
            .map_or(1., |i| self.profile.bindings[*i].calibration.scale.abs())
    }
    fn setting_text(&self, setting: Setting) -> (String, String) {
        let tab = self.current();
        let on = |v: bool| if v { "On" } else { "Off" }.to_owned();
        match setting {
            Setting::Info(text) => (text.into(), String::new()),
            Setting::Status => ("Status".into(), self.head_status.clone()),
            Setting::Rumble => ("Rumble / vibration".into(), on(self.profile.rumble)),
            Setting::GamepadDefaults => (
                "Defaults for new gamepads".into(),
                on(self.profile.gamepad_defaults),
            ),
            Setting::Modifiers => {
                let names: Vec<String> = self
                    .modifiers(&tab)
                    .into_iter()
                    .map(|i| {
                        let (_, m) = &self.profile.modifiers[i];
                        catalog::control_label("pad", m, Mode::Press, self.gamepad_tab(&tab))
                    })
                    .collect();
                (
                    "Modifier buttons".into(),
                    if names.is_empty() {
                        "None".into()
                    } else {
                        names.join(", ")
                    },
                )
            }
            Setting::Deadzone => ("Deadzone".into(), format!("{:.2}", self.deadzone(&tab))),
            Setting::StickSensitivity => (
                "Stick sensitivity".into(),
                format!("{:.2}", self.sensitivity(&tab, false)),
            ),
            Setting::TriggerSensitivity => (
                "Trigger sensitivity".into(),
                format!("{:.2}", self.sensitivity(&tab, true)),
            ),
            Setting::MouseLook => (
                "Mouse look (hold right button)".into(),
                on(self.profile.mouse_look),
            ),
            Setting::MouseSensitivity => (
                "Mouse look sensitivity".into(),
                format!("{:.1}", self.profile.mouse_sensitivity),
            ),
            Setting::MouseInvert => (
                "Invert mouse look up/down".into(),
                on(self.profile.mouse_invert),
            ),
            Setting::HeadSource => (
                "opentrack UDP receiver".into(),
                self.profile
                    .head_port
                    .map_or("Off".into(), |p| format!("On, port {p}")),
            ),
            Setting::HeadYaw => (
                "Yaw sensitivity".into(),
                format!("{:.1}", self.profile.head_scale[0].abs()),
            ),
            Setting::HeadPitch => (
                "Pitch sensitivity".into(),
                format!("{:.1}", self.profile.head_scale[1].abs()),
            ),
            Setting::HeadInvertYaw => ("Invert yaw".into(), on(self.profile.head_scale[0] < 0.)),
            Setting::HeadInvertPitch => {
                ("Invert pitch".into(), on(self.profile.head_scale[1] < 0.))
            }
        }
    }

    // ---- editing --------------------------------------------------------

    fn changed(&mut self) -> ResultAction {
        self.dirty = true;
        ResultAction::Changed
    }
    fn adjust(&mut self, setting: Setting, delta: i32) -> ResultAction {
        let tab = self.current();
        let step = f64::from(delta.signum());
        match setting {
            Setting::Info(_) | Setting::Status => return ResultAction::None,
            Setting::Rumble => self.profile.rumble = !self.profile.rumble,
            Setting::GamepadDefaults => {
                self.profile.gamepad_defaults = !self.profile.gamepad_defaults
            }
            Setting::Modifiers => {
                // Adding and removing both happen in the capture that follows.
                self.begin(Target::Modifier);
                return ResultAction::Changed;
            }
            Setting::Deadzone => {
                let v = ((self.deadzone(&tab) + 0.01 * step) * 100.).round() / 100.;
                let mut all = self.calibrated(&tab, false);
                all.extend(self.calibrated(&tab, true));
                for i in all {
                    self.profile.bindings[i].calibration.deadzone = v.clamp(0., 0.5);
                }
            }
            Setting::StickSensitivity | Setting::TriggerSensitivity => {
                let trigger = setting == Setting::TriggerSensitivity;
                let v = ((self.sensitivity(&tab, trigger) + 0.05 * step) * 20.).round() / 20.;
                for i in self.calibrated(&tab, trigger) {
                    let c = &mut self.profile.bindings[i].calibration;
                    c.scale = v.clamp(0.1, 1.).copysign(c.scale);
                }
            }
            Setting::MouseLook => self.profile.mouse_look = !self.profile.mouse_look,
            Setting::MouseSensitivity => {
                self.profile.mouse_sensitivity = ((self.profile.mouse_sensitivity + 0.1 * step)
                    * 10.)
                    .round()
                    .clamp(1., 50.)
                    / 10.
            }
            Setting::MouseInvert => self.profile.mouse_invert = !self.profile.mouse_invert,
            Setting::HeadSource => {
                self.profile.head_port = match self.profile.head_port {
                    Some(_) => None,
                    None => Some(4242),
                }
            }
            Setting::HeadYaw | Setting::HeadPitch => {
                let i = usize::from(setting == Setting::HeadPitch);
                let s = self.profile.head_scale[i];
                self.profile.head_scale[i] =
                    ((s.abs() + 0.1 * step) * 10.).round().clamp(1., 30.) / 10. * s.signum();
            }
            Setting::HeadInvertYaw => self.profile.head_scale[0] = -self.profile.head_scale[0],
            Setting::HeadInvertPitch => self.profile.head_scale[1] = -self.profile.head_scale[1],
        }
        self.changed()
    }
    fn begin(&mut self, target: Target) {
        let tab = self.current();
        let mut rest = BTreeMap::new();
        if let Tab::Device(id) = &tab
            && let Some(d) = self.device(id)
        {
            for c in &d.controls {
                let v = self
                    .held
                    .get(&(id.clone(), c.id.clone()))
                    .copied()
                    .unwrap_or(c.value);
                rest.insert(c.id.clone(), v);
            }
        }
        self.message = match (&target, &tab) {
            (Target::Modifier, _) => {
                "Press a button or D-pad direction to add it as a modifier, or a modifier to remove it. Esc cancels.".into()
            }
            (Target::Slot(i, _), Tab::Keyboard) => format!(
                "Press a key for {}. Hold Ctrl/Alt/Shift to combine. Esc cancels.",
                ENTRIES[*i].label
            ),
            (Target::Slot(i, _), Tab::Mouse) => format!(
                "Click the middle, side or right button, or turn the wheel, for {}.",
                ENTRIES[*i].label
            ),
            (Target::Slot(i, _), _) => format!(
                "Press a button or move an axis for {}. Hold modifiers first. Esc cancels.",
                ENTRIES[*i].label
            ),
        };
        self.capture = Some(Capture { target, rest });
    }
    fn remove_source(&mut self, source: &Source, tab: &Tab) {
        match source {
            Source::Stock(key) => {
                let device = if *tab == Tab::Mouse {
                    "mouse"
                } else {
                    "keyboard"
                };
                self.profile.disabled.insert((device.into(), (*key).into()));
            }
            Source::Binding(i) => {
                self.profile.bindings.remove(*i);
            }
        }
    }
    fn clear_slot(&mut self, index: usize, slot: Option<usize>) -> ResultAction {
        let tab = self.current();
        let entry = &ENTRIES[index];
        if entry.fixed {
            self.message = format!("{} is built in and cannot be cleared", entry.label);
            return ResultAction::Changed;
        }
        let mut slots = self.slots(index, &tab);
        if let Some(slot) = slot {
            if slot >= slots.len() {
                return ResultAction::None;
            }
            slots = vec![slots.remove(slot)];
        }
        highest_first(&mut slots);
        for source in slots {
            self.remove_source(&source, &tab);
        }
        self.message = format!("{} cleared on this device", entry.label);
        self.keep_visible();
        self.changed()
    }
    fn axis_option(&mut self, index: usize, invert: bool) -> ResultAction {
        let tab = self.current();
        let bindings: Vec<usize> = self
            .slots(index, &tab)
            .into_iter()
            .filter_map(|s| match s {
                Source::Binding(i) => Some(i),
                Source::Stock(_) => None,
            })
            .collect();
        if bindings.is_empty() {
            self.message = "Assign an axis first".into();
            return ResultAction::Changed;
        }
        for i in bindings {
            let c = &mut self.profile.bindings[i].calibration;
            if invert {
                c.scale = -c.scale;
            } else {
                c.curve = match c.curve {
                    x if x < 1.25 => 1.5,
                    x if x < 1.75 => 2.,
                    x if x < 2.5 => 3.,
                    _ => 1.,
                };
            }
        }
        self.changed()
    }
    fn reset_device(&mut self) -> ResultAction {
        let tab = self.current();
        match &tab {
            Tab::Keyboard | Tab::Mouse => {
                let device = if tab == Tab::Mouse {
                    "mouse"
                } else {
                    "keyboard"
                };
                self.profile.disabled.retain(|(d, _)| d != device);
                self.profile.bindings.retain(|b| b.device != device);
                if tab == Tab::Mouse {
                    let d = Profile::default();
                    self.profile.mouse_look = d.mouse_look;
                    self.profile.mouse_sensitivity = d.mouse_sensitivity;
                    self.profile.mouse_invert = d.mouse_invert;
                }
            }
            Tab::Head => {
                let d = Profile::default();
                self.profile.head_port = d.head_port;
                self.profile.head_scale = d.head_scale;
            }
            Tab::Device(id) => {
                for i in self.modifiers(&tab).into_iter().rev() {
                    self.profile.modifiers.remove(i);
                }
                let keep: Vec<Binding> = self
                    .profile
                    .bindings
                    .iter()
                    .filter(|b| !self.on_tab(b, &tab))
                    .cloned()
                    .collect();
                self.profile.bindings = keep;
                if let Some(d) = self.device(id) {
                    let defaults = crate::input::gamepad_defaults(d);
                    self.profile.bindings.extend(defaults.bindings);
                    self.profile.modifiers.extend(defaults.modifiers);
                }
            }
        }
        self.message = "Device restored to its defaults. Apply to keep it.".into();
        self.scroll = 0;
        self.changed()
    }
    /// Stores a captured input in the target slot, replacing what was there.
    fn assign(&mut self, index: usize, slot: usize, mut binding: Binding) {
        let tab = self.current();
        let entry = &ENTRIES[index];
        let digital = matches!(tab, Tab::Keyboard | Tab::Mouse);
        let mut notes = vec![];
        // On a keyboard or mouse one control does one thing, so it is taken
        // from any other action. Devices keep shared assignments (A is gear
        // in flight and select in menus); those are only reported.
        let mut remove: Vec<Source> = vec![];
        for (other, e) in ENTRIES.iter().enumerate() {
            if other == index || catalog::menu_only(e) || e.fixed {
                continue;
            }
            for source in self.slots(other, &tab) {
                let same = match &source {
                    Source::Stock(k) => *k == binding.control,
                    Source::Binding(i) => {
                        let b = &self.profile.bindings[*i];
                        b.control == binding.control && b.mode == binding.mode
                    }
                };
                if same {
                    notes.push(e.label);
                    if digital {
                        remove.push(source);
                    }
                }
            }
        }
        let previous = self.slots(index, &tab).get(slot).cloned();
        if let Some(previous) = &previous {
            remove.push(previous.clone());
        }
        // Giving an entry back its own stock key needs no custom binding.
        let own_stock = match tab {
            Tab::Keyboard => entry.keys,
            Tab::Mouse => entry.mouse,
            _ => &[],
        };
        let restores_stock = own_stock.contains(&binding.control.as_str());
        highest_first(&mut remove);
        remove.dedup();
        // A replaced binding keeps its place, so primary stays primary.
        let insert_at = match previous {
            Some(Source::Binding(at)) => Some(
                at - remove
                    .iter()
                    .filter(|s| matches!(s, Source::Binding(i) if *i < at))
                    .count(),
            ),
            _ => None,
        };
        for source in &remove {
            self.remove_source(source, &tab);
        }
        let shown = match tab {
            Tab::Keyboard => catalog::key_label(&binding.control),
            Tab::Mouse => catalog::mouse_label(&binding.control),
            _ => catalog::control_label(
                &binding.device,
                &binding.control,
                binding.mode,
                self.gamepad_tab(&tab),
            ),
        };
        if restores_stock {
            let device = if tab == Tab::Mouse {
                "mouse"
            } else {
                "keyboard"
            };
            self.profile
                .disabled
                .remove(&(device.into(), binding.control.clone()));
        } else {
            if let Some(action) = entry.parsed() {
                binding.action = action;
            }
            match insert_at {
                Some(at) => self.profile.bindings.insert(at, binding),
                None => self.profile.bindings.push(binding),
            }
        }
        self.message = if notes.is_empty() {
            format!("{} set to {shown}. Apply to keep it.", entry.label)
        } else if digital {
            format!(
                "{} set to {shown}; removed from {}.",
                entry.label,
                notes.join(", ")
            )
        } else {
            format!(
                "{} set to {shown}; also used for {}.",
                entry.label,
                notes.join(", ")
            )
        };
        self.capture = None;
        self.dirty = true;
    }
    fn digital_binding(&self, index: usize, device: &str, control: String) -> Binding {
        let entry = &ENTRIES[index];
        // Keyboard directions match the stock keyboard axes' priority.
        let (mode, priority) = match entry.kind {
            Row::Direction(sign) => (Mode::Hold(sign), 100),
            Row::Hold => (Mode::HoldState, 10),
            _ => (Mode::Press, 10),
        };
        Binding {
            device: device.into(),
            control,
            action: entry.parsed().unwrap_or(Action::Ui("pause".into())),
            mode,
            calibration: Calibration {
                deadzone: 0.,
                ..Calibration::default()
            },
            priority,
        }
    }

    // ---- navigation -----------------------------------------------------

    fn columns(&self, line: usize) -> Vec<usize> {
        match self.lines().get(line) {
            Some(Line::Entry(i)) => {
                if matches!(ENTRIES[*i].kind, Row::Axis | Row::Lever | Row::Head)
                    && matches!(self.current(), Tab::Device(_))
                {
                    vec![0, 1, 2, 3, 4]
                } else {
                    vec![0, 1, 4]
                }
            }
            Some(Line::Other(_)) => vec![4],
            _ => vec![0],
        }
    }
    fn keep_visible(&mut self) {
        let total = self.lines().len();
        if let Focus::Cell(line, _) = self.focus
            && line >= total
            && total > 0
        {
            self.focus = Focus::Cell(total - 1, 0);
        }
        if let Focus::Cell(line, _) = self.focus {
            if line < self.scroll {
                self.scroll = line;
            } else if line >= self.scroll + LIST_LINES {
                self.scroll = line + 1 - LIST_LINES;
            }
        }
        self.scroll = self.scroll.min(total.saturating_sub(LIST_LINES));
    }
    fn select_tab(&mut self, tab: usize) {
        if tab != self.tab {
            self.tab = tab;
            self.scroll = 0;
            self.capture = None;
        }
    }
    fn nearest(&self, line: usize, column: usize) -> usize {
        *self
            .columns(line)
            .iter()
            .min_by_key(|c| (**c as i32 - column as i32).abs())
            .unwrap_or(&0)
    }
    fn move_focus(&mut self, key: &str) -> ResultAction {
        let tabs = self.tabs().len();
        let lines = self.lines().len();
        let focusable: Vec<usize> = self
            .settings()
            .iter()
            .enumerate()
            .filter(|(_, s)| s.focusable())
            .map(|(i, _)| i)
            .collect();
        let into_list = if lines > 0 {
            Focus::Cell(self.scroll, 0)
        } else {
            Focus::Footer(0)
        };
        self.focus = match (self.focus, key) {
            (Focus::Tab(i), "ArrowUp") => Focus::Tab((i + tabs - 1) % tabs),
            (Focus::Tab(i), "ArrowDown") => Focus::Tab((i + 1) % tabs),
            (Focus::Tab(_), "ArrowRight") => {
                focusable.first().map_or(into_list, |s| Focus::Setting(*s))
            }
            (Focus::Setting(s), "ArrowUp") => focusable
                .iter()
                .rev()
                .find(|i| **i < s)
                .map_or(Focus::Tab(self.tab), |i| Focus::Setting(*i)),
            (Focus::Setting(s), "ArrowDown") => focusable
                .iter()
                .find(|i| **i > s)
                .map_or(into_list, |i| Focus::Setting(*i)),
            (Focus::Cell(l, c), "ArrowUp") if l > 0 => Focus::Cell(l - 1, self.nearest(l - 1, c)),
            (Focus::Cell(_, _), "ArrowUp") => focusable
                .last()
                .map_or(Focus::Tab(self.tab), |s| Focus::Setting(*s)),
            (Focus::Cell(l, c), "ArrowDown") if l + 1 < lines => {
                Focus::Cell(l + 1, self.nearest(l + 1, c))
            }
            (Focus::Cell(_, _), "ArrowDown") => Focus::Footer(0),
            (Focus::Cell(l, c), "ArrowLeft") => self
                .columns(l)
                .iter()
                .rev()
                .find(|x| **x < c)
                .map_or(Focus::Tab(self.tab), |x| Focus::Cell(l, *x)),
            (Focus::Cell(l, c), "ArrowRight") => {
                Focus::Cell(l, *self.columns(l).iter().find(|x| **x > c).unwrap_or(&c))
            }
            (Focus::Footer(_), "ArrowUp") if lines > 0 => Focus::Cell(lines - 1, 0),
            (Focus::Footer(_), "ArrowUp") => Focus::Tab(self.tab),
            (Focus::Footer(i), "ArrowLeft") => Focus::Footer(i.saturating_sub(1)),
            (Focus::Footer(i), "ArrowRight") => Focus::Footer((i + 1).min(FOOTER.len() - 1)),
            (focus, _) => focus,
        };
        if let Focus::Tab(i) = self.focus {
            self.select_tab(i);
        }
        self.keep_visible();
        // Moving focus redraws without the click sound activation makes.
        ResultAction::None
    }
    fn activate(&mut self, hit: Hit) -> ResultAction {
        match hit {
            Hit::Tab(i) => {
                self.select_tab(i);
                self.focus = Focus::Tab(i);
                ResultAction::Changed
            }
            Hit::Setting(i, delta) => {
                self.focus = Focus::Setting(i);
                match self.settings().get(i) {
                    Some(s) => self.adjust(*s, delta),
                    None => ResultAction::None,
                }
            }
            Hit::Footer(0) => ResultAction::Save,
            Hit::Footer(1) => self.reset_device(),
            Hit::Footer(_) => ResultAction::Close,
            Hit::Cell(line, column) => {
                self.focus = Focus::Cell(line, column);
                match self.lines().get(line).copied() {
                    Some(Line::Header(group)) => {
                        if !self.collapsed.remove(&group) {
                            self.collapsed.insert(group);
                        }
                        self.keep_visible();
                        ResultAction::Changed
                    }
                    Some(Line::Other(i)) => {
                        let name = action_name(&self.profile.bindings[i].action);
                        self.profile.bindings.remove(i);
                        self.message = format!("Removed the {name} binding");
                        self.keep_visible();
                        self.changed()
                    }
                    Some(Line::Entry(i)) => match column {
                        0 | 1 => {
                            let entry = &ENTRIES[i];
                            if entry.fixed || !entry.bindable() {
                                self.message =
                                    format!("{} is built in and cannot be changed", entry.label);
                                return ResultAction::Changed;
                            }
                            // An empty primary is filled before the secondary.
                            let filled = self.slots(i, &self.current()).len();
                            self.begin(Target::Slot(i, column.min(filled)));
                            ResultAction::Changed
                        }
                        2 => self.axis_option(i, true),
                        3 => self.axis_option(i, false),
                        _ => self.clear_slot(i, None),
                    },
                    None => ResultAction::None,
                }
            }
        }
    }
    fn focused_hit(&self, delta: i32) -> Option<Hit> {
        Some(match self.focus {
            Focus::Tab(_) => return None,
            Focus::Setting(i) => Hit::Setting(i, delta),
            Focus::Cell(l, c) => Hit::Cell(l, c),
            Focus::Footer(i) => Hit::Footer(i),
        })
    }
    /// Keyboard input, and controller menu actions mapped to arrow keys,
    /// Enter and Escape.
    pub fn key(&mut self, key: &str, shift: bool, ctrl: bool, alt: bool) -> ResultAction {
        if let Some(capture) = &self.capture {
            if key == "Escape" {
                self.capture = None;
                self.message = "Capture cancelled".into();
                return ResultAction::Changed;
            }
            let Target::Slot(index, slot) = capture.target else {
                return ResultAction::None;
            };
            if self.current() != Tab::Keyboard
                || key.is_empty()
                || matches!(key, "Shift" | "Control" | "Alt" | "Super")
            {
                return ResultAction::None;
            }
            let control = format!(
                "{}{}{}{}",
                if ctrl { "Ctrl-" } else { "" },
                if alt { "Alt-" } else { "" },
                if shift { "Shift-" } else { "" },
                key
            );
            if catalog::PROTECTED_KEYS.contains(&control.as_str()) {
                self.message = format!(
                    "{} is reserved; press another key",
                    catalog::key_label(&control)
                );
                return ResultAction::Changed;
            }
            let binding = self.digital_binding(index, "keyboard", control);
            self.assign(index, slot, binding);
            return ResultAction::Changed;
        }
        match key {
            "Escape" => ResultAction::Close,
            "ArrowLeft" | "ArrowRight" if matches!(self.focus, Focus::Setting(_)) => {
                if let Focus::Setting(i) = self.focus
                    && self
                        .settings()
                        .get(i)
                        .is_some_and(|s| *s == Setting::Modifiers)
                {
                    return ResultAction::None;
                }
                let delta = if key == "ArrowLeft" { -1 } else { 1 };
                match self.focused_hit(delta) {
                    Some(hit) => self.activate(hit),
                    None => ResultAction::None,
                }
            }
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" => self.move_focus(key),
            "Tab" => {
                let n = self.tabs().len();
                let next = if shift {
                    (self.tab + n - 1) % n
                } else {
                    (self.tab + 1) % n
                };
                self.select_tab(next);
                self.focus = Focus::Tab(next);
                ResultAction::None
            }
            "PageUp" | "PageDown" => {
                let lines = self.lines().len();
                if lines == 0 {
                    return ResultAction::None;
                }
                let at = match self.focus {
                    Focus::Cell(l, _) => l,
                    _ => self.scroll,
                };
                let target = if key == "PageUp" {
                    at.saturating_sub(LIST_LINES)
                } else {
                    (at + LIST_LINES).min(lines - 1)
                };
                self.focus = Focus::Cell(target, 0);
                self.keep_visible();
                ResultAction::None
            }
            "Delete" | "Backspace" => match self.focus {
                Focus::Cell(l, c) if c < 2 => match self.lines().get(l) {
                    Some(Line::Entry(i)) => self.clear_slot(*i, Some(c)),
                    _ => ResultAction::None,
                },
                _ => ResultAction::None,
            },
            "Enter" | "Space" => match self.focus {
                Focus::Tab(_) => self.move_focus("ArrowRight"),
                _ => match self.focused_hit(1) {
                    Some(hit) => self.activate(hit),
                    None => ResultAction::None,
                },
            },
            _ => ResultAction::None,
        }
    }
    /// A mouse button other than the left one, or a wheel notch, while the
    /// screen captures on the Mouse tab.
    pub fn mouse(&mut self, control: &str) -> ResultAction {
        let Some(Capture {
            target: Target::Slot(index, slot),
            ..
        }) = self.capture
        else {
            return ResultAction::None;
        };
        if self.current() != Tab::Mouse {
            return ResultAction::None;
        }
        if control == "button:right" && self.profile.mouse_look {
            self.message = "The right button is mouse look; turn mouse look off to bind it".into();
            return ResultAction::Changed;
        }
        let binding = self.digital_binding(index, "mouse", control.into());
        self.assign(index, slot, binding);
        ResultAction::Changed
    }
    pub fn wheel(&mut self, notches: i32) -> ResultAction {
        if self.capture.is_some() {
            return self.mouse(if notches > 0 {
                "wheel:up"
            } else {
                "wheel:down"
            });
        }
        let max = self.lines().len().saturating_sub(LIST_LINES);
        self.scroll = (self.scroll as i32 - notches * 3).clamp(0, max as i32) as usize;
        ResultAction::None
    }
    /// Native samples: tracks held modifiers and completes a device capture.
    /// Returns true when the draft changed.
    pub fn observe(&mut self, event: &Event) -> bool {
        let previous = self
            .held
            .insert((event.device.clone(), event.control.clone()), event.value);
        let Some(capture) = &self.capture else {
            return false;
        };
        let Tab::Device(id) = self.current() else {
            return false;
        };
        if event.baseline || event.device != id {
            return false;
        }
        let Some(device) = self.device(&id).cloned() else {
            return false;
        };
        let Some(control) = device.controls.iter().find(|c| c.id == event.control) else {
            return false;
        };
        let rest = capture
            .rest
            .get(&control.id)
            .copied()
            .unwrap_or(control.value);
        let target = capture.target.clone();
        let hat = is_hat(control);
        // What this sample presses, as a button token, if anything.
        let pressed: Option<String> = match control.kind {
            Kind::Button if event.value > 0.5 && previous.is_none_or(|v| v <= 0.5) => {
                Some(control.id.clone())
            }
            _ if hat && event.value != 0. && event.value != rest => {
                Some(format!("{}={}", control.id, event.value as i32))
            }
            _ => None,
        };
        let tab = self.current();
        let reference = self.reference(&id);
        let declared: Vec<String> = self
            .modifiers(&tab)
            .into_iter()
            .map(|i| self.profile.modifiers[i].1.clone())
            .collect();
        if target == Target::Modifier {
            let Some(token) = pressed else {
                return false;
            };
            let name = catalog::control_label("pad", &token, Mode::Press, self.gamepad_tab(&tab));
            if let Some(i) = self
                .modifiers(&tab)
                .into_iter()
                .find(|i| self.profile.modifiers[*i].1 == token)
            {
                self.profile.modifiers.remove(i);
                self.message = format!("{name} is an ordinary button again.");
            } else if declared.len() >= 4 {
                self.message =
                    "A device can have four modifier buttons; press one to remove it".into();
                return true;
            } else {
                self.profile.modifiers.push((reference, token));
                self.message =
                    format!("{name} is now a modifier. Hold it while capturing to combine.");
            }
            self.capture = None;
            self.dirty = true;
            return true;
        }
        let Target::Slot(index, slot) = target else {
            return false;
        };
        if pressed.as_ref().is_some_and(|t| declared.contains(t)) {
            return false; // A modifier going down; wait for the control itself.
        }
        let held_mods: Vec<String> = declared
            .iter()
            .filter(|m| {
                let base = tore_input::token_base(m);
                let Some(c) = device.controls.iter().find(|c| c.id == base) else {
                    return false;
                };
                let raw = self
                    .held
                    .get(&(id.clone(), base.to_owned()))
                    .copied()
                    .unwrap_or(c.value);
                let value = norm(c, raw);
                tore_input::token_value(m, value).unwrap_or(value) != 0.
            })
            .take(2)
            .cloned()
            .collect();
        let chord = |base: &str| {
            let mut parts = held_mods.clone();
            parts.push(base.to_owned());
            parts.join("+")
        };
        let entry = &ENTRIES[index];
        let span = control.max - control.min;
        let moved =
            matches!(control.kind, Kind::Axis) && !hat && (event.value - rest).abs() > span * 0.3;
        let relative = matches!(control.kind, Kind::Relative) && event.value != 0.;
        // Triggers rest at one end of their travel; sticks rest centered.
        let trigger = matches!(control.kind, Kind::Axis) && norm(control, rest) < -0.8;
        let direction = (event.value - rest).signum();
        let mut calibration = Calibration {
            deadzone: self.deadzone(&tab),
            ..Calibration::default()
        };
        let sensitivity = self.sensitivity(&tab, false);
        let mut binding = Binding {
            device: reference,
            control: String::new(),
            action: entry.parsed().unwrap_or(Action::Ui("pause".into())),
            mode: Mode::Press,
            calibration,
            priority: 10,
        };
        match entry.kind {
            Row::Axis | Row::Lever | Row::Head => {
                if relative && entry.action == "throttle-rate" {
                    binding.mode = Mode::Delta;
                    binding.control = chord(&control.id);
                } else if moved {
                    binding.control = chord(&control.id);
                    binding.mode = if entry.kind == Row::Lever {
                        Mode::Unit
                    } else {
                        Mode::Axis
                    };
                    if entry.kind == Row::Head {
                        calibration.deadzone = 0.;
                    } else if entry.kind == Row::Axis {
                        // Pushing a stick forward to look up reads negative.
                        let sign = if entry.action == "look-y" { -1. } else { 1. };
                        calibration.scale = sensitivity * sign;
                    }
                    binding.calibration = calibration;
                } else if pressed.is_some() || relative {
                    self.message = format!(
                        "{} needs an axis; put buttons on its direction rows",
                        entry.label
                    );
                    return true;
                } else {
                    return false;
                }
            }
            Row::Direction(sign) => {
                if let Some(token) = &pressed {
                    binding.control = chord(token);
                    binding.mode = Mode::Hold(sign);
                } else if moved && trigger {
                    binding.control = chord(&control.id);
                    binding.mode = Mode::Trigger(sign);
                    calibration.scale = self.sensitivity(&tab, true);
                    binding.calibration = calibration;
                } else if moved {
                    // A centered stick drives the whole axis row instead.
                    let Some(axis_row) = ENTRIES
                        .iter()
                        .position(|e| e.action == entry.action && e.kind == Row::Axis)
                    else {
                        return false;
                    };
                    binding.control = chord(&control.id);
                    binding.mode = Mode::Axis;
                    calibration.scale = sensitivity * sign * direction;
                    binding.calibration = calibration;
                    let slot = self.slots(axis_row, &tab).len().min(1);
                    self.assign(axis_row, slot, binding);
                    return true;
                } else {
                    return false;
                }
            }
            Row::Command | Row::Hold => {
                let mode = if entry.kind == Row::Hold {
                    Mode::HoldState
                } else {
                    Mode::Press
                };
                if let Some(token) = &pressed {
                    if hat && entry.kind == Row::Command {
                        binding.control = chord(&control.id);
                        binding.mode = Mode::Position(event.value as i32);
                    } else {
                        binding.control = chord(token);
                        binding.mode = mode;
                    }
                } else if relative && entry.kind == Row::Command {
                    binding.control = chord(&control.id);
                    binding.mode = Mode::Delta;
                    binding.calibration.scale = direction;
                } else if moved {
                    let threshold = if trigger {
                        format!("{}>0", control.id)
                    } else if direction > 0. {
                        format!("{}>0.5", control.id)
                    } else {
                        format!("{}<-0.5", control.id)
                    };
                    binding.control = chord(&threshold);
                    binding.mode = mode;
                } else {
                    return false;
                }
            }
        }
        self.assign(index, slot, binding);
        true
    }

    // ---- pointer --------------------------------------------------------

    fn tab_rect(i: usize) -> Rect {
        (
            LEFT.0 + 2,
            TAB_TOP + i as i32 * TAB_HEIGHT,
            LEFT.2 - 4,
            TAB_HEIGHT - 2,
        )
    }
    fn setting_rect(i: usize) -> Rect {
        (
            RIGHT_X + 4,
            SETTINGS_TOP + i as i32 * SETTING_HEIGHT,
            RIGHT_W - 8,
            SETTING_HEIGHT - 1,
        )
    }
    fn line_rect(row: usize) -> Rect {
        (
            RIGHT_X + 2,
            LIST_TOP + row as i32 * LINE_HEIGHT,
            RIGHT_W - 6,
            LINE_HEIGHT - 1,
        )
    }
    fn cell_rect(row: usize, column: usize) -> Rect {
        let (x, w) = COLUMNS[column];
        (
            RIGHT_X + x,
            LIST_TOP + row as i32 * LINE_HEIGHT,
            w,
            LINE_HEIGHT - 1,
        )
    }
    fn value_rect(r: Rect) -> Rect {
        (r.0 + 234, r.1 + 1, 220, r.3 - 2)
    }
    fn footer_rect(i: usize) -> Rect {
        (RIGHT_X + 150 + i as i32 * 108, FOOTER_Y, 100, 18)
    }
    fn hit(&self, p: (f64, f64)) -> Option<Hit> {
        for i in 0..self.tabs().len() {
            if inside(p, Self::tab_rect(i)) {
                return Some(Hit::Tab(i));
            }
        }
        for (i, s) in self.settings().iter().enumerate() {
            let r = Self::setting_rect(i);
            if s.focusable() && inside(p, r) {
                // The left third of the value box steps down; the rest steps up.
                let v = Self::value_rect(r);
                let delta = if s.arrows() && p.0 < (v.0 + v.2 / 3) as f64 {
                    -1
                } else {
                    1
                };
                return Some(Hit::Setting(i, delta));
            }
        }
        let lines = self.lines();
        for row in 0..LIST_LINES {
            let line = self.scroll + row;
            let Some(kind) = lines.get(line) else {
                break;
            };
            if !inside(p, Self::line_rect(row)) {
                continue;
            }
            if matches!(kind, Line::Header(_)) {
                return Some(Hit::Cell(line, 0));
            }
            return self
                .columns(line)
                .into_iter()
                .find(|c| inside(p, Self::cell_rect(row, *c)))
                .map(|c| Hit::Cell(line, c));
        }
        (0..FOOTER.len())
            .find(|i| inside(p, Self::footer_rect(*i)))
            .map(Hit::Footer)
    }
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> ResultAction {
        let hit = point.and_then(|p| self.hit(p));
        if down {
            self.pressed = hit;
            return ResultAction::None;
        }
        let pressed = self.pressed.take();
        if pressed != hit || self.capture.is_some() {
            return ResultAction::None;
        }
        hit.map_or(ResultAction::None, |h| self.activate(h))
    }

    // ---- drawing --------------------------------------------------------

    fn text(
        pixels: &mut [u8],
        font: &Font,
        clip: Rect,
        color: [u8; 4],
        text: &str,
        at: (i32, i32),
    ) {
        Paint {
            pixels,
            clip,
            color,
        }
        .text(font, text, at.0, at.1);
    }
    fn button(pixels: &mut [u8], font: &Font, r: Rect, label: &str, focused: bool) {
        Canvas(pixels).rect(r, if focused { FOCUS } else { BUTTON });
        let x = r.0 + (r.2 - text_width(font, label)) / 2;
        Self::text(pixels, font, r, WHITE, label, (x, r.1 + 2));
    }
    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        let full = (0, 0, 640, 480);
        Canvas(pixels).rect(full, PAPER);
        Canvas(pixels).rect((0, 0, 640, 20), PANEL);
        Self::text(
            pixels,
            font,
            full,
            TITLE,
            "INPUT CONFIGURATION   |   Control mappings",
            (8, 5),
        );
        let context = self.context.to_ascii_uppercase();
        Self::text(
            pixels,
            font,
            full,
            TITLE,
            &context,
            (632 - text_width(font, &context), 5),
        );
        self.draw_tabs(pixels, font);
        self.draw_settings(pixels, font);
        self.draw_mappings(pixels, font);
        Canvas(pixels).rect((0, 438, 640, 42), PANEL);
        let message = if self.dirty && self.capture.is_none() {
            format!("{}  (not applied)", self.message)
        } else {
            self.message.clone()
        };
        Self::text(
            pixels,
            font,
            (8, 440, 624, 14),
            TITLE,
            &fit(font, &message, 620),
            (8, 442),
        );
        Self::text(
            pixels,
            font,
            (8, 456, 300, 20),
            MUTED,
            "Arrows move, Enter selects, Esc backs out",
            (8, 461),
        );
        for (i, label) in FOOTER.iter().enumerate() {
            let r = Self::footer_rect(i);
            Canvas(pixels).rect(
                r,
                if self.focus == Focus::Footer(i) {
                    FOCUS
                } else {
                    PALE
                },
            );
            let color = if self.focus == Focus::Footer(i) {
                WHITE
            } else {
                INK
            };
            let x = r.0 + (r.2 - text_width(font, label)) / 2;
            Self::text(pixels, font, r, color, label, (x, r.1 + 4));
        }
    }
    fn draw_tabs(&self, pixels: &mut [u8], font: &Font) {
        Canvas(pixels).rect(LEFT, PANEL);
        Self::text(
            pixels,
            font,
            LEFT,
            TITLE,
            "INPUT DEVICES",
            (LEFT.0 + 6, LEFT.1 + 5),
        );
        for (i, tab) in self.tabs().iter().enumerate() {
            let r = Self::tab_rect(i);
            if r.1 + r.3 > LEFT.1 + LEFT.3 {
                break;
            }
            let fill = if self.focus == Focus::Tab(i) {
                FOCUS
            } else if i == self.tab {
                [52, 74, 100, 255]
            } else {
                PAPER
            };
            Canvas(pixels).rect(r, fill);
            if i == self.tab {
                Canvas(pixels).rect((r.0, r.1, 3, r.3), GOOD);
            }
            let (title, subtitle, live) = match tab {
                Tab::Keyboard => ("KEYBOARD".to_owned(), "Standard keyboard".to_owned(), true),
                Tab::Mouse => ("MOUSE".into(), "Look and buttons".into(), true),
                Tab::Head => (
                    "HEAD TRACKER".into(),
                    if self.head_status.starts_with("Receiving") {
                        "Receiving".into()
                    } else {
                        self.profile
                            .head_port
                            .map_or("Off".into(), |p| format!("opentrack UDP {p}"))
                    },
                    self.head_status.starts_with("Receiving"),
                ),
                Tab::Device(id) => match self.device(id) {
                    Some(d) => (
                        classify(d).title().to_ascii_uppercase(),
                        d.name.clone(),
                        true,
                    ),
                    None => ("DEVICE".into(), "Not connected".into(), false),
                },
            };
            let color = if live { WHITE } else { MUTED };
            Self::text(pixels, font, r, color, &title, (r.0 + 8, r.1 + 5));
            Self::text(
                pixels,
                font,
                r,
                MUTED,
                &fit(font, &subtitle, r.2 - 12),
                (r.0 + 8, r.1 + 18),
            );
        }
    }
    fn draw_settings(&self, pixels: &mut [u8], font: &Font) {
        let tab = self.current();
        let panel = (RIGHT_X, 24, RIGHT_W, 116);
        Canvas(pixels).rect(panel, PANEL);
        let (title, device, live) = match &tab {
            Tab::Keyboard => ("KEYBOARD SETTINGS".to_owned(), String::new(), true),
            Tab::Mouse => ("MOUSE SETTINGS".into(), String::new(), true),
            Tab::Head => (
                "HEAD TRACKER SETTINGS".into(),
                if self.head_status.starts_with("Receiving") {
                    "CONNECTED".into()
                } else {
                    "NOT DETECTED".into()
                },
                self.head_status.starts_with("Receiving"),
            ),
            Tab::Device(id) => match self.device(id) {
                Some(d) => (
                    format!("{} SETTINGS", classify(d).title().to_ascii_uppercase()),
                    format!("{}   CONNECTED", d.name.to_ascii_uppercase()),
                    true,
                ),
                None => ("DEVICE SETTINGS".into(), "NOT CONNECTED".into(), false),
            },
        };
        Self::text(pixels, font, panel, TITLE, &title, (RIGHT_X + 6, 29));
        let device = fit(font, &device, 280);
        let x = RIGHT_X + RIGHT_W - 6 - text_width(font, &device);
        Self::text(
            pixels,
            font,
            panel,
            if live { GOOD } else { MUTED },
            &device,
            (x, 29),
        );
        for (i, setting) in self.settings().iter().enumerate() {
            let r = Self::setting_rect(i);
            if self.focus == Focus::Setting(i) {
                Canvas(pixels).rect(r, FOCUS);
            }
            let (label, value) = self.setting_text(*setting);
            if !setting.focusable() {
                let text = if value.is_empty() {
                    label
                } else {
                    format!("{label}: {value}")
                };
                Self::text(
                    pixels,
                    font,
                    r,
                    MUTED,
                    &fit(font, &text, r.2 - 12),
                    (r.0 + 6, r.1 + 3),
                );
                continue;
            }
            Self::text(pixels, font, r, WHITE, &label, (r.0 + 6, r.1 + 3));
            let v = Self::value_rect(r);
            Canvas(pixels).outline(v, [110, 130, 156, 255]);
            let value = if !setting.arrows() {
                fit(font, &value, 200)
            } else {
                format!("<   {}   >", fit(font, &value, 170))
            };
            let x = v.0 + (v.2 - text_width(font, &value)) / 2;
            Self::text(pixels, font, v, WHITE, &value, (x, r.1 + 3));
        }
    }
    fn draw_mappings(&self, pixels: &mut [u8], font: &Font) {
        let tab = self.current();
        let panel = (RIGHT_X, MAP_TOP, RIGHT_W, 292);
        Canvas(pixels).rect(panel, PANEL);
        Self::text(
            pixels,
            font,
            panel,
            TITLE,
            "CONTROL MAPPINGS",
            (RIGHT_X + 6, MAP_TOP + 4),
        );
        if tab == Tab::Head {
            for (i, text) in [
                "Head tracking turns the view like the right stick or mouse look, and",
                "keeps the cockpit limit: you cannot look below the forward eye line.",
                "Center view (Shift+/) makes your current head position straight ahead.",
                "",
                "TrackIR's own software only feeds games registered with NaturalPoint.",
                "Use opentrack instead: input TrackIR (or a webcam or phone tracker),",
                "output 'UDP over network' to 127.0.0.1 port 4242.",
                "",
                "Trackers that appear as a joystick can bind Head tracker yaw and",
                "pitch on that device's tab instead.",
            ]
            .iter()
            .enumerate()
            {
                Self::text(
                    pixels,
                    font,
                    panel,
                    WHITE,
                    text,
                    (RIGHT_X + 10, MAP_TOP + 24 + i as i32 * 15),
                );
            }
            return;
        }
        let lines = self.lines();
        let count = self
            .profile
            .bindings
            .iter()
            .filter(|b| self.on_tab(b, &tab))
            .count();
        let summary = match tab {
            Tab::Keyboard | Tab::Mouse => format!("{count} custom"),
            _ => format!("{count} bindings"),
        };
        let x = RIGHT_X + RIGHT_W - 6 - text_width(font, &summary);
        Self::text(pixels, font, panel, MUTED, &summary, (x, MAP_TOP + 4));
        let header = (RIGHT_X + 2, LIST_TOP - 16, RIGHT_W - 4, 14);
        Canvas(pixels).rect(header, HEADER);
        let (first, second) = if tab == Tab::Keyboard {
            ("PRIMARY KEY", "SECONDARY KEY")
        } else {
            ("PRIMARY INPUT", "SECONDARY INPUT")
        };
        for (text, x) in [
            ("ACTION", 6),
            (first, COLUMNS[0].0),
            (second, COLUMNS[1].0),
            ("OPTIONS", COLUMNS[2].0),
        ] {
            Self::text(
                pixels,
                font,
                header,
                TITLE,
                text,
                (RIGHT_X + x, header.1 + 3),
            );
        }
        for row in 0..LIST_LINES {
            let line = self.scroll + row;
            let Some(kind) = lines.get(line) else {
                break;
            };
            let r = Self::line_rect(row);
            match kind {
                Line::Header(group) => {
                    Canvas(pixels).rect(r, GROUP);
                    if matches!(self.focus, Focus::Cell(l, _) if l == line) {
                        Canvas(pixels).outline(r, WHITE);
                    }
                    let open = !self.collapsed.contains(group);
                    let count = lines
                        .iter()
                        .skip(line + 1)
                        .take_while(|l| !matches!(l, Line::Header(_)))
                        .count();
                    let title = group
                        .map_or("Other bindings", Group::title)
                        .to_ascii_uppercase();
                    // A small drawn triangle: down when open, right when collapsed.
                    for i in 0..4 {
                        let rect = if open {
                            (r.0 + 5 + i, r.1 + 4 + i, 7 - 2 * i, 1)
                        } else {
                            (r.0 + 6 + i, r.1 + 3 + i, 1, 7 - 2 * i)
                        };
                        Canvas(pixels).rect(rect, INK);
                    }
                    let text = if open {
                        format!("{title} ({count})")
                    } else {
                        title
                    };
                    Self::text(pixels, font, r, INK, &text, (r.0 + 18, r.1 + 2));
                }
                Line::Entry(i) => self.draw_entry(pixels, font, &tab, row, line, *i),
                Line::Other(i) => {
                    let b = &self.profile.bindings[*i];
                    Canvas(pixels).rect(
                        r,
                        if row.is_multiple_of(2) {
                            PALE
                        } else {
                            PALE_ALT
                        },
                    );
                    let name = fit(font, &action_name(&b.action), COLUMNS[0].0 - 10);
                    Self::text(pixels, font, r, INK, &name, (r.0 + 4, r.1 + 2));
                    let label = catalog::control_label(
                        &b.device,
                        &b.control,
                        b.mode,
                        self.gamepad_tab(&tab),
                    );
                    let cell = Self::cell_rect(row, 0);
                    let width = COLUMNS[1].0 + COLUMNS[1].1 - COLUMNS[0].0;
                    Self::text(
                        pixels,
                        font,
                        r,
                        INK,
                        &fit(font, &label, width),
                        (cell.0 + 2, cell.1 + 2),
                    );
                    let focused = self.focus == Focus::Cell(line, 4);
                    Self::button(pixels, font, Self::cell_rect(row, 4), "Clear", focused);
                }
            }
        }
        let total = lines.len();
        if total > LIST_LINES {
            let track = (
                RIGHT_X + RIGHT_W - 3,
                LIST_TOP,
                2,
                LIST_LINES as i32 * LINE_HEIGHT,
            );
            Canvas(pixels).rect(track, HEADER);
            let h = (track.3 * LIST_LINES as i32 / total as i32).max(8);
            let y = track.1 + (track.3 - h) * self.scroll as i32 / (total - LIST_LINES) as i32;
            Canvas(pixels).rect((track.0, y, 2, h), TITLE);
        }
    }
    fn draw_entry(
        &self,
        pixels: &mut [u8],
        font: &Font,
        tab: &Tab,
        row: usize,
        line: usize,
        index: usize,
    ) {
        let entry = &ENTRIES[index];
        let r = Self::line_rect(row);
        Canvas(pixels).rect(
            r,
            if row.is_multiple_of(2) {
                PALE
            } else {
                PALE_ALT
            },
        );
        let label = fit(font, entry.label, COLUMNS[0].0 - 10);
        Self::text(pixels, font, r, INK, &label, (r.0 + 4, r.1 + 2));
        let slots = self.slots(index, tab);
        let capturing = match &self.capture {
            Some(Capture {
                target: Target::Slot(i, s),
                ..
            }) if *i == index => Some(*s),
            _ => None,
        };
        let bindings: Vec<&Binding> = slots
            .iter()
            .filter_map(|s| match s {
                Source::Binding(i) => Some(&self.profile.bindings[*i]),
                Source::Stock(_) => None,
            })
            .collect();
        for column in self.columns(line) {
            let cell = Self::cell_rect(row, column);
            let focused = self.focus == Focus::Cell(line, column);
            match column {
                0 | 1 => {
                    let mut text = if capturing.is_some_and(|s| s.min(1) == column) {
                        "Press...".to_owned()
                    } else {
                        slots
                            .get(column)
                            .map_or("-".to_owned(), |s| self.source_label(s, tab))
                    };
                    if column == 1 && slots.len() > 2 {
                        text = format!("{text} +{}", slots.len() - 2);
                    }
                    let color = if focused {
                        Canvas(pixels).rect(cell, FOCUS);
                        WHITE
                    } else if entry.fixed {
                        MUTED
                    } else {
                        INK
                    };
                    Self::text(
                        pixels,
                        font,
                        cell,
                        color,
                        &fit(font, &text, cell.2 - 4),
                        (cell.0 + 2, cell.1 + 2),
                    );
                }
                2 => {
                    let inverted = bindings.iter().any(|b| b.calibration.scale < 0.);
                    let color = if focused { WHITE } else { INK };
                    if focused {
                        Canvas(pixels).rect(cell, FOCUS);
                    }
                    let tick = (cell.0 + 3, cell.1 + 2, 9, 9);
                    Canvas(pixels).outline(tick, color);
                    if inverted {
                        Canvas(pixels).rect((tick.0 + 2, tick.1 + 2, 5, 5), color);
                    }
                }
                3 => {
                    let curve = bindings.first().map(|b| b.calibration.curve);
                    let text = curve.map_or("curve".to_owned(), |c| format!("x{c:.1}"));
                    Self::button(pixels, font, cell, &text, focused);
                }
                _ => Self::button(pixels, font, cell, "Clear", focused),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pad() -> Device {
        let control = |id: &str, kind: Kind, min: f64, max: f64| tore_input_native::Control {
            id: id.into(),
            kind,
            min,
            max,
            value: if min == 0. && max > 1. {
                0.
            } else {
                (min + max) / 2.
            },
        };
        Device {
            id: "linux-pad".into(),
            name: "Test pad".into(),
            rumble: true,
            controls: vec![
                control("axis:0", Kind::Axis, -100., 100.),
                control("axis:1", Kind::Axis, -100., 100.),
                control("axis:5", Kind::Axis, 0., 255.),
                control("axis:16", Kind::Axis, -1., 1.),
                control("button:304", Kind::Button, 0., 1.),
                control("button:314", Kind::Button, 0., 1.),
            ],
        }
    }
    fn event(e: &mut Editor, control: &str, value: f64) -> bool {
        e.observe(&Event {
            device: "linux-pad".into(),
            control: control.into(),
            value,
            baseline: false,
        })
    }
    fn entry(label: &str) -> usize {
        ENTRIES.iter().position(|e| e.label == label).unwrap()
    }
    fn line(e: &Editor, label: &str) -> usize {
        let index = entry(label);
        e.lines()
            .iter()
            .position(|l| *l == Line::Entry(index))
            .unwrap()
    }
    #[test]
    fn opens_on_the_connected_device_with_grouped_rows() {
        let e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        assert_eq!(e.current(), Tab::Device("linux-pad".into()));
        assert_eq!(classify(&pad()), DeviceClass::Gamepad);
        assert!(matches!(e.lines()[0], Line::Header(Some(Group::Flight))));
        assert!(e.settings().contains(&Setting::Rumble));
        assert!(e.settings().contains(&Setting::Modifiers));
        assert_eq!(e.tabs().last(), Some(&Tab::Head));
    }
    #[test]
    fn keyboard_stock_key_moves_and_reports_the_previous_owner() {
        let mut e = Editor::new(Profile::default(), vec![], "Main menu");
        assert_eq!(e.current(), Tab::Keyboard);
        e.activate(Hit::Cell(line(&e, "Landing gear"), 0));
        assert!(e.capturing());
        e.key("f", false, false, false);
        // G is disabled for gear, F is taken from flaps and bound to gear.
        assert!(
            e.profile
                .disabled
                .contains(&("keyboard".into(), "g".into()))
        );
        assert!(
            e.profile
                .disabled
                .contains(&("keyboard".into(), "f".into()))
        );
        assert_eq!(e.profile.bindings.len(), 1);
        assert_eq!(e.profile.bindings[0].control, "f");
        assert!(e.message.contains("Flaps"), "{}", e.message);
        assert!(e.slots(entry("Flaps"), &Tab::Keyboard).is_empty());
        // Giving flaps back its own stock key re-enables it without a binding.
        e.activate(Hit::Cell(line(&e, "Flaps"), 0));
        e.key("f", false, false, false);
        assert!(
            !e.profile
                .disabled
                .contains(&("keyboard".into(), "f".into()))
        );
        assert!(e.profile.bindings.is_empty());
        assert!(e.profile.to_text().is_ok());
    }
    #[test]
    fn keyboard_combinations_and_protected_keys() {
        let mut e = Editor::new(Profile::default(), vec![], "Flight paused");
        e.activate(Hit::Cell(line(&e, "Pitch: nose down"), 1));
        e.key("w", true, true, false);
        let b = &e.profile.bindings[0];
        assert_eq!(
            (b.control.as_str(), b.mode, b.priority),
            ("Ctrl-Shift-w", Mode::Hold(-1.), 100)
        );
        // The stock Up arrow stays primary; the new key is secondary.
        assert_eq!(
            e.slots(entry("Pitch: nose down"), &Tab::Keyboard)[0],
            Source::Stock("ArrowUp")
        );
        e.activate(Hit::Cell(line(&e, "Landing gear"), 0));
        e.key("Enter", false, false, true);
        assert!(e.capturing(), "Alt+Enter is reserved");
        assert_eq!(e.key("Escape", false, false, false), ResultAction::Changed);
        assert!(!e.capturing());
        assert_eq!(e.key("Escape", false, false, false), ResultAction::Close);
    }
    #[test]
    fn gamepad_capture_uses_held_modifiers_and_dpad_directions() {
        let mut e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        e.adjust(Setting::Modifiers, 1);
        event(&mut e, "button:314", 1.);
        event(&mut e, "button:314", 0.);
        e.adjust(Setting::Modifiers, 1);
        event(&mut e, "axis:16", -1.);
        event(&mut e, "axis:16", 0.);
        assert_eq!(e.profile.modifiers.len(), 2);
        e.activate(Hit::Cell(line(&e, "Flaps"), 0));
        event(&mut e, "button:314", 1.);
        event(&mut e, "axis:16", -1.);
        assert!(e.capturing(), "modifiers alone do not complete a capture");
        event(&mut e, "button:304", 1.);
        assert_eq!(
            e.profile.bindings[0].control,
            "button:314+axis:16=-1+button:304"
        );
        assert!(e.profile.to_text().is_ok());
        assert_eq!(
            e.source_label(&Source::Binding(0), &e.current()),
            "View + D-pad left + A"
        );
    }
    #[test]
    fn axes_triggers_and_device_settings() {
        let mut e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        e.activate(Hit::Cell(line(&e, "Roll (bank left/right)"), 0));
        event(&mut e, "axis:0", 90.);
        e.activate(Hit::Cell(line(&e, "Rudder right"), 0));
        event(&mut e, "axis:5", 250.);
        event(&mut e, "axis:5", 0.);
        e.activate(Hit::Cell(line(&e, "Fire / release weapon"), 0));
        event(&mut e, "axis:5", 250.);
        let modes: Vec<_> = e
            .profile
            .bindings
            .iter()
            .map(|b| (b.control.clone(), b.mode))
            .collect();
        assert_eq!(
            modes,
            [
                ("axis:0".into(), Mode::Axis),
                ("axis:5".into(), Mode::Trigger(1.)),
                ("axis:5>0".into(), Mode::HoldState)
            ]
        );
        e.adjust(Setting::StickSensitivity, -1);
        assert!((e.profile.bindings[0].calibration.scale - 0.95).abs() < 1e-9);
        let roll = line(&e, "Roll (bank left/right)");
        e.activate(Hit::Cell(roll, 2));
        assert!(e.profile.bindings[0].calibration.scale < 0.);
        assert!(e.profile.to_text().is_ok());
        e.activate(Hit::Cell(roll, 4));
        assert_eq!(e.profile.bindings.len(), 2);
    }
    #[test]
    fn modifiers_start_only_on_activation_and_toggle_in_capture() {
        let mut e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        let row = e
            .settings()
            .iter()
            .position(|s| *s == Setting::Modifiers)
            .unwrap();
        e.focus = Focus::Setting(row);
        // D-pad left/right arrive as arrow keys and must not start a capture.
        assert_eq!(e.key("ArrowRight", false, false, false), ResultAction::None);
        assert_eq!(e.key("ArrowLeft", false, false, false), ResultAction::None);
        assert!(!e.capturing());
        // A arrives as Enter.
        e.key("Enter", false, false, false);
        assert!(e.capturing());
        event(&mut e, "axis:16", 1.);
        event(&mut e, "axis:16", 0.);
        assert_eq!(
            e.profile.modifiers,
            [("linux-pad".into(), "axis:16=1".into())]
        );
        // Pressing an existing modifier in the next capture removes it.
        e.key("Enter", false, false, false);
        event(&mut e, "axis:16", 1.);
        assert!(e.profile.modifiers.is_empty());
        assert!(!e.capturing());
    }
    #[test]
    fn replacing_a_primary_keeps_it_primary() {
        let mut e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        let flaps = line(&e, "Flaps");
        e.activate(Hit::Cell(flaps, 0));
        event(&mut e, "button:304", 1.);
        event(&mut e, "button:304", 0.);
        e.activate(Hit::Cell(flaps, 1));
        event(&mut e, "button:314", 1.);
        event(&mut e, "button:314", 0.);
        e.activate(Hit::Cell(flaps, 0));
        event(&mut e, "axis:16", -1.);
        let tab = e.current();
        let labels: Vec<_> = e
            .slots(entry("Flaps"), &tab)
            .iter()
            .map(|s| e.source_label(s, &tab))
            .collect();
        assert_eq!(labels, ["D-pad left", "View"]);
    }
    #[test]
    fn keyboard_navigation_reaches_every_region() {
        let mut e = Editor::new(Profile::default(), vec![pad()], "Main menu");
        assert_eq!(e.focus, Focus::Tab(2));
        e.key("ArrowRight", false, false, false);
        assert!(matches!(e.focus, Focus::Setting(_)));
        for _ in 0..8 {
            e.key("ArrowDown", false, false, false);
        }
        assert!(matches!(e.focus, Focus::Cell(_, _)));
        e.key("PageDown", false, false, false);
        e.key("PageDown", false, false, false);
        assert!(e.scroll > 0);
        for _ in 0..400 {
            e.key("ArrowDown", false, false, false);
        }
        assert_eq!(e.focus, Focus::Footer(0));
        assert_eq!(e.key("Enter", false, false, false), ResultAction::Save);
    }
    #[test]
    fn pointer_requires_matching_release() {
        let mut e = Editor::new(Profile::default(), vec![], "Main menu");
        let r = Editor::tab_rect(1);
        let p = ((r.0 + 5) as f64, (r.1 + 5) as f64);
        e.pointer(Some(p), true);
        e.pointer(Some((1., 470.)), false);
        assert_eq!(e.current(), Tab::Keyboard);
        e.pointer(Some(p), true);
        e.pointer(Some(p), false);
        assert_eq!(e.current(), Tab::Mouse);
    }
}
