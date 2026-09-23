//! Every player action the controls screen lists, with its stock keyboard and
//! mouse assignments. This table is the one source for the editor rows, the
//! stock-key remapping and `docs/CONTROLS.md`, which a test keeps in step.
//! Groups, labels and the stock keys added by T.O.R.E are opinionated agent
//! choices (2026-09-22); the keys themselves are the shipped behaviour.
use tore_input::{Action, Binding, Mode};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Flight,
    Systems,
    Weapons,
    Sensors,
    View,
    Communication,
    Game,
}
impl Group {
    pub const ALL: [Group; 7] = [
        Group::Flight,
        Group::Systems,
        Group::Weapons,
        Group::Sensors,
        Group::View,
        Group::Communication,
        Group::Game,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Group::Flight => "Flight controls",
            Group::Systems => "Systems",
            Group::Weapons => "Weapons",
            Group::Sensors => "Sensors and instruments",
            Group::View => "View",
            Group::Communication => "Communication",
            Group::Game => "Game and menus",
        }
    }
}

/// How an entry is bound, which decides the behaviour a captured control gets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// A centered analog action: stick, rudder, look.
    Axis,
    /// The absolute throttle lever.
    Lever,
    /// An absolute head-tracker angle.
    Head,
    /// One direction of a centered action, for keys and buttons.
    Direction(f64),
    /// A one-shot command on press.
    Command,
    /// Held for as long as the control is held (the trigger).
    Hold,
}

pub struct Entry {
    /// Profile action token; empty for built-in shortcuts that cannot be bound.
    pub action: &'static str,
    pub label: &'static str,
    pub group: Group,
    pub kind: Kind,
    /// Stock keyboard assignments, in `Input::key` naming.
    pub keys: &'static [&'static str],
    /// Stock mouse assignments.
    pub mouse: &'static [&'static str],
    /// Stock keys that the player cannot remove (menu, window, exit).
    pub fixed: bool,
}
impl Entry {
    pub fn parsed(&self) -> Option<Action> {
        Action::parse(self.action).ok()
    }
    /// Whether an existing binding performs this entry.
    pub fn matches(&self, binding: &Binding) -> bool {
        if self.parsed().as_ref() != Some(&binding.action) {
            return false;
        }
        match self.kind {
            Kind::Direction(sign) => match binding.mode {
                Mode::Hold(s) | Mode::Trigger(s) => s == sign,
                _ => false,
            },
            Kind::Axis | Kind::Lever | Kind::Head => {
                matches!(binding.mode, Mode::Axis | Mode::Unit | Mode::Delta)
            }
            Kind::Command | Kind::Hold => true,
        }
    }
    pub fn bindable(&self) -> bool {
        !self.action.is_empty()
    }
    /// Keyboard keys and mouse buttons can only drive one-shot, held and
    /// directional entries; analog rows are for axes.
    pub fn digital(&self) -> bool {
        matches!(self.kind, Kind::Direction(_) | Kind::Command | Kind::Hold)
    }
}

const fn e(
    action: &'static str,
    label: &'static str,
    group: Group,
    kind: Kind,
    keys: &'static [&'static str],
) -> Entry {
    Entry {
        action,
        label,
        group,
        kind,
        keys,
        mouse: &[],
        fixed: false,
    }
}
const fn cmd(
    action: &'static str,
    label: &'static str,
    group: Group,
    keys: &'static [&'static str],
) -> Entry {
    e(action, label, group, Kind::Command, keys)
}
const fn fixed(
    action: &'static str,
    label: &'static str,
    group: Group,
    keys: &'static [&'static str],
) -> Entry {
    Entry {
        action,
        label,
        group,
        kind: Kind::Command,
        keys,
        mouse: &[],
        fixed: true,
    }
}
const fn mouse(
    action: &'static str,
    label: &'static str,
    group: Group,
    keys: &'static [&'static str],
    mouse: &'static [&'static str],
) -> Entry {
    Entry {
        action,
        label,
        group,
        kind: Kind::Command,
        keys,
        mouse,
        fixed: false,
    }
}
use Group::*;
use Kind::{Axis, Direction, Head, Hold, Lever};

pub const ENTRIES: &[Entry] = &[
    e("pitch", "Pitch (nose up/down)", Flight, Axis, &[]),
    e(
        "pitch",
        "Pitch: nose down",
        Flight,
        Direction(-1.),
        &["ArrowUp"],
    ),
    e(
        "pitch",
        "Pitch: nose up",
        Flight,
        Direction(1.),
        &["ArrowDown"],
    ),
    e("roll", "Roll (bank left/right)", Flight, Axis, &[]),
    e("roll", "Roll left", Flight, Direction(-1.), &["ArrowLeft"]),
    e("roll", "Roll right", Flight, Direction(1.), &["ArrowRight"]),
    e("yaw", "Rudder (yaw)", Flight, Axis, &[]),
    e("yaw", "Rudder left", Flight, Direction(-1.), &["z"]),
    e("yaw", "Rudder right", Flight, Direction(1.), &["x"]),
    e("throttle", "Throttle lever", Flight, Lever, &[]),
    e("throttle-rate", "Throttle rate (axis)", Flight, Axis, &[]),
    e(
        "throttle-rate",
        "Throttle up",
        Flight,
        Direction(1.),
        &["PageUp"],
    ),
    e(
        "throttle-rate",
        "Throttle down",
        Flight,
        Direction(-1.),
        &["PageDown"],
    ),
    cmd("throttle=0", "Throttle idle", Flight, &[]),
    cmd("throttle=0.1", "Throttle 10%", Flight, &["1"]),
    cmd("throttle=0.2", "Throttle 20%", Flight, &["2"]),
    cmd("throttle=0.3", "Throttle 30%", Flight, &["3"]),
    cmd("throttle=0.4", "Throttle 40%", Flight, &["4"]),
    cmd("throttle=0.5", "Throttle 50%", Flight, &["5"]),
    cmd("throttle=0.6", "Throttle 60%", Flight, &["6"]),
    cmd("throttle=0.7", "Throttle 70%", Flight, &["7"]),
    cmd("throttle=0.8", "Throttle 80%", Flight, &["8"]),
    cmd("throttle=0.9", "Throttle 90%", Flight, &["9"]),
    cmd("throttle=1", "Throttle 100%", Flight, &["0"]),
    cmd("burner", "Afterburner", Flight, &["Shift-b"]),
    cmd("autopilot", "Autopilot (heading/altitude)", Flight, &["a"]),
    cmd(
        "waypoint-autopilot",
        "Waypoint autopilot",
        Flight,
        &["Ctrl-a"],
    ),
    cmd("eject", "Eject (press twice)", Systems, &["Shift-e"]),
    cmd("gear", "Landing gear", Systems, &["g"]),
    cmd("flaps", "Flaps", Systems, &["f"]),
    cmd("airbrake", "Airbrake / wheel brakes", Systems, &["b"]),
    cmd("hook", "Tailhook", Systems, &["h"]),
    cmd("engine", "Engine on/off", Systems, &["e"]),
    cmd("bay", "Weapon bays (F-22)", Systems, &["Shift-o"]),
    cmd("damage-report", "Damage report", Systems, &["d"]),
    e("fire", "Fire / release weapon", Weapons, Hold, &["Space"]),
    cmd("weapon-next", "Next weapon / NAV", Weapons, &["]"]),
    cmd("weapon-previous", "Previous weapon / NAV", Weapons, &["["]),
    cmd("designate", "Next radar target", Weapons, &["t"]),
    cmd(
        "designate-previous",
        "Previous radar target",
        Weapons,
        &["Shift-t"],
    ),
    cmd(
        "designate-visual",
        "Select visual target",
        Weapons,
        &["Enter", "'"],
    ),
    cmd("clear-designation", "Clear designation", Weapons, &["l"]),
    cmd(
        "weapon-seeker-mode",
        "Seeker mode (bore/cued)",
        Weapons,
        &[],
    ),
    cmd("jettison", "Jettison selected stores", Weapons, &["k"]),
    cmd("range-target", "Reset range target", Weapons, &["\\"]),
    cmd(
        "incoming",
        "Incoming missile (range)",
        Weapons,
        &["Shift-i"],
    ),
    cmd(
        "target-jammer",
        "Target jammer (range)",
        Weapons,
        &["Shift-y"],
    ),
    cmd("damage-class", "Next damage class (test)", Weapons, &[]),
    cmd("fail-station", "Fail station (test)", Weapons, &[]),
    cmd("damage-player", "Damage player (test)", Weapons, &[]),
    cmd("radar", "Radar power / radar channel", Sensors, &["r"]),
    cmd("jammer", "Jammer (ECM)", Sensors, &["j"]),
    cmd(
        "sensor-channel",
        "Cycle sensor channel",
        Sensors,
        &["m", "o"],
    ),
    cmd("sensor-infrared", "Infrared channel", Sensors, &["i"]),
    cmd("sensor-history", "Contact history", Sensors, &["y"]),
    cmd("range-down", "Scope range down", Sensors, &[","]),
    cmd("range-up", "Scope range up", Sensors, &["."]),
    cmd("instrument-next", "Next instrument", Sensors, &["Ctrl-Tab"]),
    cmd(
        "instrument-previous",
        "Previous instrument",
        Sensors,
        &["Ctrl-Shift-Tab"],
    ),
    cmd("instrument-1", "Select instrument 1", Sensors, &["Ctrl-1"]),
    cmd("instrument-2", "Select instrument 2", Sensors, &["Ctrl-2"]),
    cmd("instrument-3", "Select instrument 3", Sensors, &["Ctrl-3"]),
    cmd("instrument-4", "Select instrument 4", Sensors, &["Ctrl-4"]),
    cmd("instrument-5", "Select instrument 5", Sensors, &["Ctrl-5"]),
    cmd("instrument-6", "Select instrument 6", Sensors, &["Ctrl-6"]),
    cmd(
        "control-1",
        "Instrument button 1",
        Sensors,
        &["Ctrl-Shift-1"],
    ),
    cmd(
        "control-2",
        "Instrument button 2",
        Sensors,
        &["Ctrl-Shift-2"],
    ),
    cmd(
        "control-3",
        "Instrument button 3",
        Sensors,
        &["Ctrl-Shift-3"],
    ),
    cmd(
        "control-4",
        "Instrument button 4",
        Sensors,
        &["Ctrl-Shift-4"],
    ),
    cmd("page-1", "Window: Envelope", Sensors, &["Shift-1"]),
    cmd("page-2", "Window: Forward view", Sensors, &["Shift-2"]),
    cmd("page-3", "Window: Other view", Sensors, &["Shift-3"]),
    cmd("page-4", "Window: Radar/Visual", Sensors, &["Shift-4"]),
    cmd("page-5", "Window: RWR", Sensors, &["Shift-5"]),
    cmd("page-6", "Window: Navigation", Sensors, &["Shift-6"]),
    cmd("page-7", "Window: Systems", Sensors, &["Shift-7"]),
    cmd("page-8", "Window: Weapons", Sensors, &["Shift-8"]),
    cmd("page-9", "Window: Radar", Sensors, &["Shift-9"]),
    cmd(
        "page-0",
        "Window: Radar cross section",
        Sensors,
        &["Shift-0"],
    ),
    cmd("view-front", "Front cockpit view", View, &["F1"]),
    cmd("view-back", "Look back", View, &["F2"]),
    cmd("view-up", "Look up (view)", View, &["F3"]),
    cmd("view-external", "External view", View, &["F10"]),
    e("look-x", "Look left/right", View, Axis, &[]),
    e(
        "look-x",
        "Look left",
        View,
        Direction(-1.),
        &["Shift-ArrowLeft", "Ctrl-ArrowLeft"],
    ),
    e(
        "look-x",
        "Look right",
        View,
        Direction(1.),
        &["Shift-ArrowRight", "Ctrl-ArrowRight"],
    ),
    e("look-y", "Look up/down", View, Axis, &[]),
    e(
        "look-y",
        "Look up",
        View,
        Direction(1.),
        &["Shift-ArrowUp", "Ctrl-ArrowUp"],
    ),
    e(
        "look-y",
        "Look down",
        View,
        Direction(-1.),
        &["Shift-ArrowDown", "Ctrl-ArrowDown"],
    ),
    e("head-yaw", "Head tracker yaw", View, Head, &[]),
    e("head-pitch", "Head tracker pitch", View, Head, &[]),
    cmd("center-look", "Center view", View, &["Shift-/"]),
    mouse("zoom-in", "Zoom in", View, &["="], &["wheel:up"]),
    mouse("zoom-out", "Zoom out", View, &["-"], &["wheel:down"]),
    cmd("cockpit", "Cockpit art", View, &["Backspace"]),
    cmd("hud", "HUD", View, &["Shift-u"]),
    cmd("key:Shift-[", "Dim HUD", View, &["Shift-["]),
    cmd("key:Shift-]", "Brighten HUD", View, &["Shift-]"]),
    cmd("key:Shift-m", "Live map", View, &["Shift-m"]),
    cmd("key:Alt-b", "Wing: break left", Communication, &["Alt-b"]),
    cmd("key:Alt-r", "Wing: break right", Communication, &["Alt-r"]),
    cmd("key:Alt-h", "Wing: break high", Communication, &["Alt-h"]),
    cmd("key:Alt-v", "Wing: break low", Communication, &["Alt-v"]),
    cmd("key:Alt-t", "Wing: fly straight", Communication, &["Alt-t"]),
    cmd(
        "key:Alt-e",
        "Wing: engage my target",
        Communication,
        &["Alt-e"],
    ),
    cmd("key:Alt-p", "Wing: protect me", Communication, &["Alt-p"]),
    cmd(
        "key:Alt-w",
        "Wing: attack on contact",
        Communication,
        &["Alt-w"],
    ),
    cmd(
        "key:Alt-f",
        "Wing: engage from formation",
        Communication,
        &["Alt-f"],
    ),
    cmd("key:Alt-d", "Wing: disengage", Communication, &["Alt-d"]),
    cmd("key:Alt-1", "Wing: echelon", Communication, &["Alt-1"]),
    cmd("key:Alt-2", "Wing: line abreast", Communication, &["Alt-2"]),
    cmd("key:Alt-3", "Wing: line astern", Communication, &["Alt-3"]),
    cmd("key:Alt-8", "Wing: spacing", Communication, &["Alt-8"]),
    cmd("key:Alt-k", "Wing: stacking", Communication, &["Alt-k"]),
    cmd(
        "key:Alt-c",
        "Wing: loose/medium control",
        Communication,
        &["Alt-c"],
    ),
    cmd(
        "key:Alt-Shift-b",
        "Wing: approach target left",
        Communication,
        &["Alt-Shift-b"],
    ),
    cmd(
        "key:Alt-Shift-r",
        "Wing: approach target right",
        Communication,
        &["Alt-Shift-r"],
    ),
    cmd(
        "key:Alt-Shift-h",
        "Wing: approach target high",
        Communication,
        &["Alt-Shift-h"],
    ),
    cmd(
        "key:Alt-Shift-v",
        "Wing: approach target low",
        Communication,
        &["Alt-Shift-v"],
    ),
    cmd(
        "key:Alt-0",
        "Address whole flight",
        Communication,
        &["Alt-0"],
    ),
    cmd("key:Alt-4", "Address wingman 1", Communication, &["Alt-4"]),
    cmd("key:Alt-5", "Address wingman 2", Communication, &["Alt-5"]),
    cmd("key:Alt-6", "Address wingman 3", Communication, &["Alt-6"]),
    cmd("key:Alt-7", "Address wingman 4", Communication, &["Alt-7"]),
    cmd(
        "airport-nav",
        "Airport NAV / ILS",
        Communication,
        &["Shift-n"],
    ),
    cmd("airport-next", "Next airport", Communication, &["Shift-a"]),
    cmd(
        "airport-request-landing",
        "Request landing",
        Communication,
        &["Shift-l"],
    ),
    cmd(
        "airport-repeat",
        "Repeat tower reply",
        Communication,
        &["Shift-r"],
    ),
    cmd(
        "airport-cancel",
        "Cancel approach",
        Communication,
        &["Shift-c"],
    ),
    fixed("menu", "Flight menu / back", Game, &["Escape"]),
    cmd("pause", "Pause", Game, &["Ctrl-p"]),
    cmd("key:c", "Time compression", Game, &["c"]),
    cmd("end-flight", "End mission", Game, &["Ctrl-q"]),
    cmd("restart", "Restart flight", Game, &[]),
    cmd("key:F11", "Keyboard help", Game, &["F11"]),
    fixed("menu-up", "Menu up", Game, &["ArrowUp"]),
    fixed("menu-down", "Menu down", Game, &["ArrowDown"]),
    fixed("menu-left", "Menu left", Game, &["ArrowLeft"]),
    fixed("menu-right", "Menu right", Game, &["ArrowRight"]),
    fixed("menu-accept", "Menu select", Game, &["Enter"]),
    fixed("menu-back", "Menu back", Game, &["Escape"]),
    fixed("", "Fullscreen / window", Game, &["Alt-Enter"]),
    fixed("", "Exit to desktop", Game, &["Alt-F4"]),
];

/// Stock keys that stay live whatever the profile says, so the player can
/// always reach the menu, change window mode and quit.
pub const PROTECTED_KEYS: [&str; 3] = ["Escape", "Alt-Enter", "Alt-F4"];

/// Menu-only rows use the keyboard's built-in navigation; their stock keys are
/// not flight shortcuts and are never disabled by remapping.
pub fn menu_only(entry: &Entry) -> bool {
    entry.action.starts_with("menu-")
}

/// Human name of a keyboard shortcut in `Input::key` naming.
pub fn key_label(key: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = key;
    for (prefix, name) in [
        ("Ctrl-", "Ctrl"),
        ("Alt-", "Alt"),
        ("Shift-", "Shift"),
        ("Super-", "Super"),
    ] {
        if let Some(tail) = rest.strip_prefix(prefix)
            && !tail.is_empty()
        {
            parts.push(name.to_owned());
            rest = tail;
        }
    }
    parts.push(match rest {
        "ArrowUp" => "Up".into(),
        "ArrowDown" => "Down".into(),
        "ArrowLeft" => "Left".into(),
        "ArrowRight" => "Right".into(),
        "PageUp" => "Page Up".into(),
        "PageDown" => "Page Down".into(),
        "Backspace" => "Backspace".into(),
        "Escape" => "Esc".into(),
        "'" => "Apostrophe".into(),
        "\\" => "Backslash".into(),
        "," => "Comma".into(),
        "-" => "Minus".into(),
        "=" => "Equals".into(),
        "." => "Period".into(),
        other if other.len() == 1 => other.to_ascii_uppercase(),
        other => other.into(),
    });
    parts.join("+")
}

pub fn mouse_label(control: &str) -> String {
    match control {
        "button:right" => "Right button".into(),
        "button:middle" => "Middle button".into(),
        "button:back" => "Back button".into(),
        "button:forward" => "Forward button".into(),
        "wheel:up" => "Wheel up".into(),
        "wheel:down" => "Wheel down".into(),
        other => other.into(),
    }
}

/// Standard Linux gamepad controls with Xbox names. Other platforms and
/// devices fall back to generic names in `control_label`.
fn gamepad_name(control: &str) -> Option<&'static str> {
    Some(match control {
        "button:304" => "A",
        "button:305" => "B",
        "button:307" => "X",
        "button:308" => "Y",
        "button:310" => "LB",
        "button:311" => "RB",
        "button:312" => "LT (click)",
        "button:313" => "RT (click)",
        "button:314" => "View",
        "button:315" => "Menu",
        "button:316" => "Xbox",
        "button:317" => "Left stick press",
        "button:318" => "Right stick press",
        "axis:0" => "Left stick X",
        "axis:1" => "Left stick Y",
        "axis:2" => "LT",
        "axis:3" => "Right stick X",
        "axis:4" => "Right stick Y",
        "axis:5" => "RT",
        "axis:16" => "D-pad X",
        "axis:17" => "D-pad Y",
        "axis:16=-1" => "D-pad left",
        "axis:16=1" => "D-pad right",
        "axis:17=-1" => "D-pad up",
        "axis:17=1" => "D-pad down",
        _ => return None,
    })
}

fn linux_axis(code: u32) -> String {
    match code {
        0 => "X axis".into(),
        1 => "Y axis".into(),
        2 => "Z axis".into(),
        3 => "X rotation".into(),
        4 => "Y rotation".into(),
        5 => "Z rotation".into(),
        6 => "Throttle axis".into(),
        7 => "Rudder axis".into(),
        8 => "Wheel axis".into(),
        9 => "Gas axis".into(),
        10 => "Brake axis".into(),
        16..=23 => format!(
            "Hat {} {}",
            (code - 16) / 2 + 1,
            if code.is_multiple_of(2) { "X" } else { "Y" }
        ),
        n => format!("Axis {n}"),
    }
}

fn decode_hex(hex: &str) -> Option<String> {
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}

/// One physical control or virtual button token, without modifiers.
fn token_label(token: &str, gamepad: bool) -> String {
    if gamepad && let Some(name) = gamepad_name(token) {
        return name.into();
    }
    let base = tore_input::token_base(token);
    let suffix = &token[base.len()..];
    let name = if gamepad && let Some(name) = gamepad_name(base) {
        name.to_owned()
    } else if let Some(code) = base.strip_prefix("button:") {
        match code.parse::<u32>() {
            // Linux joystick buttons start at BTN_TRIGGER (288).
            Ok(n @ 288..=303) => format!("Button {}", n - 287),
            Ok(n @ 704..=743) => format!("Button {}", n - 704 + 17),
            Ok(n) if cfg!(not(target_os = "linux")) || n < 256 => format!("Button {}", n + 1),
            Ok(n) => format!("Button {n}"),
            Err(_) => base.into(),
        }
    } else if let Some(code) = base.strip_prefix("axis:") {
        match code.parse::<u32>() {
            Ok(n) if cfg!(target_os = "linux") => linux_axis(n),
            Ok(n) => format!("Axis {}", n + 1),
            Err(_) => base.into(),
        }
    } else if let Some(n) = base.strip_prefix("switch:") {
        format!("Hat {}", n.parse::<u32>().map_or(0, |n| n + 1))
    } else if let Some(n) = base.strip_prefix("relative:") {
        format!("Dial {n}")
    } else if let Some(n) = base.strip_prefix("element:") {
        format!("Element {n}")
    } else if let Some((_, hex)) = base.split_once(':')
        && base.starts_with("gc-")
    {
        decode_hex(hex).unwrap_or_else(|| base.into())
    } else {
        base.into()
    };
    match suffix {
        "" => name,
        "=-1" if name.ends_with(" X") => format!("{} left", name.trim_end_matches(" X")),
        "=1" if name.ends_with(" X") => format!("{} right", name.trim_end_matches(" X")),
        "=-1" if name.ends_with(" Y") => format!("{} up", name.trim_end_matches(" Y")),
        "=1" if name.ends_with(" Y") => format!("{} down", name.trim_end_matches(" Y")),
        s if s.starts_with('>') => format!("{name} (pressed)"),
        s if s.starts_with('<') => format!("{name} (pulled)"),
        s => format!("{name} {s}"),
    }
}

/// Display name of a binding's control on a device, including modifiers and
/// the hat position a `position=N` binding reads.
pub fn control_label(device: &str, control: &str, mode: Mode, gamepad: bool) -> String {
    match device {
        "keyboard" => return key_label(control),
        "mouse" => return mouse_label(control),
        _ => {}
    }
    let (mods, base) = tore_input::chord_parts(control);
    let base = match mode {
        Mode::Position(n) => format!("{base}={n}"),
        _ => base.to_owned(),
    };
    let mut parts: Vec<String> = mods.iter().map(|m| token_label(m, gamepad)).collect();
    parts.push(token_label(&base, gamepad));
    parts.join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_bindable_entry_parses_and_labels_are_unique() {
        let mut labels = std::collections::BTreeSet::new();
        for entry in ENTRIES {
            assert!(labels.insert(entry.label), "{}", entry.label);
            if entry.bindable() {
                assert!(entry.parsed().is_some(), "{}", entry.action);
            }
        }
    }
    #[test]
    fn stock_keys_are_not_shared_between_flight_entries() {
        let mut seen = std::collections::BTreeMap::new();
        for entry in ENTRIES.iter().filter(|e| !menu_only(e)) {
            for key in entry.keys {
                if let Some(other) = seen.insert(*key, entry.label) {
                    // The menu key opens and backs out of the same menu.
                    assert_eq!(*key, "Escape", "{key}: {other} and {}", entry.label);
                }
            }
        }
    }
    fn cell(values: Vec<String>) -> String {
        if values.is_empty() {
            "-".into()
        } else {
            values.join(" or ").replace('|', "\\|")
        }
    }
    /// The generated part of `docs/CONTROLS.md`.
    fn controls_markdown() -> String {
        let pad = crate::input::gamepad_defaults(&crate::controls_editor::preview_device());
        let mut out = String::new();
        for group in Group::ALL {
            out.push_str(&format!(
                "\n### {}\n\n| Action | Keyboard | Mouse | Gamepad (Xbox) |\n| --- | --- | --- | --- |\n",
                group.title()
            ));
            for entry in ENTRIES.iter().filter(|e| e.group == group) {
                let keys = entry.keys.iter().map(|k| key_label(k)).collect();
                let mut mouse: Vec<String> = entry.mouse.iter().map(|m| mouse_label(m)).collect();
                if matches!(entry.kind, Kind::Axis) && entry.action.starts_with("look-") {
                    mouse.push("Hold right button and drag".into());
                }
                let buttons = pad
                    .bindings
                    .iter()
                    .filter(|b| entry.bindable() && entry.matches(b))
                    .map(|b| control_label(&b.device, &b.control, b.mode, true))
                    .collect();
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    entry.label,
                    cell(keys),
                    cell(mouse),
                    cell(buttons)
                ));
            }
        }
        out
    }
    /// `docs/CONTROLS.md` is generated from this catalog and the gamepad
    /// defaults. Regenerate with
    /// `TORE_UPDATE_CONTROLS_DOC=1 cargo test -p tore-app controls_doc`.
    #[test]
    fn controls_doc_matches_the_catalog() {
        const START: &str = "<!-- controls-table:start -->\n";
        const END: &str = "<!-- controls-table:end -->";
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONTROLS.md");
        // Windows checkouts may convert the doc to CRLF line endings.
        let text = std::fs::read_to_string(&path)
            .expect("docs/CONTROLS.md")
            .replace("\r\n", "\n");
        let (head, rest) = text.split_once(START).expect("start marker");
        let (current, tail) = rest.split_once(END).expect("end marker");
        let generated = controls_markdown() + "\n";
        if std::env::var_os("TORE_UPDATE_CONTROLS_DOC").is_some() {
            std::fs::write(&path, format!("{head}{START}{generated}{END}{tail}")).unwrap();
            return;
        }
        assert!(
            current == generated,
            "docs/CONTROLS.md is out of date; run TORE_UPDATE_CONTROLS_DOC=1 cargo test -p tore-app controls_doc"
        );
    }
    #[test]
    fn labels_use_xbox_names_and_hat_directions() {
        assert_eq!(key_label("Ctrl-Shift-Tab"), "Ctrl+Shift+Tab");
        assert_eq!(key_label("Shift-/"), "Shift+/");
        assert_eq!(
            control_label("pad", "button:314+button:311", Mode::HoldState, true),
            "View + RB"
        );
        assert_eq!(
            control_label("pad", "axis:16", Mode::Position(-1), true),
            "D-pad left"
        );
        assert_eq!(
            control_label("pad", "axis:16=1+button:304", Mode::Press, true),
            "D-pad right + A"
        );
        assert_eq!(
            control_label("pad", "axis:5>0", Mode::Press, true),
            "RT (pressed)"
        );
    }
}
