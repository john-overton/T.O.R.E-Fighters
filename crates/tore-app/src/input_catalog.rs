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
    Replay,
    Communication,
    Game,
}
impl Group {
    pub const ALL: [Group; 8] = [
        Group::Flight,
        Group::Systems,
        Group::Weapons,
        Group::Sensors,
        Group::View,
        Group::Replay,
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
            Group::Replay => "Replay drone",
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
    /// An absolute throttle, nozzle, nacelle or collective lever.
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
    pub fn replay_only(&self) -> bool {
        self.group == Group::Replay
    }
    /// Replay assignments have their own desktop context. Removing W here
    /// must not remove a flight action on W, or vice versa.
    pub fn device<'a>(&self, device: &'a str) -> &'a str {
        match (self.replay_only(), device) {
            (true, "keyboard") => "replay-keyboard",
            (true, "mouse") => "replay-mouse",
            _ => device,
        }
    }
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
    cmd(
        "drone-cycle",
        "Drone: cycle flight/follow/free",
        Replay,
        &["`"],
    ),
    cmd("drone-follow", "Drone: follow aircraft", Replay, &[]),
    cmd("drone-free", "Drone: free camera", Replay, &[]),
    e("drone-forward", "Drone: move forward", Replay, Hold, &["w"]),
    e(
        "drone-backward",
        "Drone: move backward",
        Replay,
        Hold,
        &["s"],
    ),
    e("drone-left", "Drone: move left", Replay, Hold, &["a"]),
    e("drone-right", "Drone: move right", Replay, Hold, &["d"]),
    e("drone-up", "Drone: move up", Replay, Hold, &["e"]),
    e("drone-down", "Drone: move down", Replay, Hold, &["q"]),
    e(
        "drone-boost",
        "Drone: four times faster",
        Replay,
        Hold,
        &["Shift"],
    ),
    Entry {
        mouse: &["button:right"],
        ..e(
            "drone-look",
            "Drone: hold to look with mouse",
            Replay,
            Hold,
            &[],
        )
    },
    mouse(
        "drone-faster",
        "Drone: increase movement speed",
        Replay,
        &[],
        &["wheel:up"],
    ),
    mouse(
        "drone-slower",
        "Drone: decrease movement speed",
        Replay,
        &[],
        &["wheel:down"],
    ),
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
    e("yaw", "Rudder left", Flight, Direction(-1.), &["End", "z"]),
    e(
        "yaw",
        "Rudder right",
        Flight,
        Direction(1.),
        &["PageDown", "x"],
    ),
    e(
        "vector-pitch",
        "Nozzle pitch (VTOL) lever",
        Flight,
        Lever,
        &[],
    ),
    e(
        "vector-pitch-rate",
        "Nozzle pitch (VTOL) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "vector-pitch-rate",
        "Nozzle pitch (VTOL): decrease",
        Flight,
        Direction(-1.),
        &["Ctrl-ArrowUp"],
    ),
    e(
        "vector-pitch-rate",
        "Nozzle pitch (VTOL): increase",
        Flight,
        Direction(1.),
        &["Ctrl-ArrowDown"],
    ),
    e("vector-yaw", "Nozzle yaw (VTOL) lever", Flight, Axis, &[]),
    e(
        "vector-yaw-rate",
        "Nozzle yaw (VTOL) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "vector-yaw-rate",
        "Nozzle yaw (VTOL): decrease",
        Flight,
        Direction(-1.),
        &["Ctrl-ArrowLeft"],
    ),
    e(
        "vector-yaw-rate",
        "Nozzle yaw (VTOL): increase",
        Flight,
        Direction(1.),
        &["Ctrl-ArrowRight"],
    ),
    e(
        "conversion",
        "Nacelle conversion (V-22) lever",
        Flight,
        Lever,
        &[],
    ),
    e(
        "conversion-rate",
        "Nacelle conversion (V-22) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "conversion-rate",
        "Nacelle conversion (V-22): decrease",
        Flight,
        Direction(-1.),
        &["Ctrl-PageUp"],
    ),
    e(
        "conversion-rate",
        "Nacelle conversion (V-22): increase",
        Flight,
        Direction(1.),
        &["Ctrl-PageDown"],
    ),
    e(
        "collective",
        "Collective (helicopters / V-22) lever",
        Flight,
        Lever,
        &[],
    ),
    e(
        "collective-rate",
        "Collective (helicopters / V-22) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "collective-rate",
        "Collective (helicopters / V-22): decrease",
        Flight,
        Direction(-1.),
        &["Ctrl-End"],
    ),
    e(
        "collective-rate",
        "Collective (helicopters / V-22): increase",
        Flight,
        Direction(1.),
        &["Ctrl-Home"],
    ),
    cmd(
        "neutral-vector",
        "Nozzles/nacelles: forward neutral",
        Flight,
        &["0"],
    ),
    cmd(
        "nozzle-step-up",
        "Nozzles up (aft) 10 degrees (AV-8, Yak-141)",
        Flight,
        &["z"],
    ),
    cmd(
        "nozzle-step-down",
        "Nozzles down 10 degrees (AV-8, Yak-141)",
        Flight,
        &["x"],
    ),
    cmd(
        "nozzle-preset-forward",
        "Nozzles to 0, or braking stop to vertical (AV-8, Yak-141)",
        Flight,
        &["Shift-z"],
    ),
    cmd(
        "nozzle-preset-vertical",
        "Nozzles vertical, again to the braking stop (AV-8, Yak-141)",
        Flight,
        &["Shift-x"],
    ),
    e(
        "trim-pitch-rate",
        "Cyclic trim fore/aft (helicopters / V-22) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "trim-pitch-rate",
        "Cyclic trim forward (helicopters / V-22)",
        Flight,
        Direction(-1.),
        &["Ctrl-ArrowUp"],
    ),
    e(
        "trim-pitch-rate",
        "Cyclic trim aft (helicopters / V-22)",
        Flight,
        Direction(1.),
        &["Ctrl-ArrowDown"],
    ),
    e(
        "trim-roll-rate",
        "Cyclic trim left/right (helicopters / V-22) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "trim-roll-rate",
        "Cyclic trim left (helicopters / V-22)",
        Flight,
        Direction(-1.),
        &["Ctrl-ArrowLeft"],
    ),
    e(
        "trim-roll-rate",
        "Cyclic trim right (helicopters / V-22)",
        Flight,
        Direction(1.),
        &["Ctrl-ArrowRight"],
    ),
    e(
        "trim-pedal-rate",
        "Pedal trim (helicopters / V-22) rate axis",
        Flight,
        Axis,
        &[],
    ),
    e(
        "trim-pedal-rate",
        "Pedal trim left (helicopters / V-22)",
        Flight,
        Direction(-1.),
        &[],
    ),
    e(
        "trim-pedal-rate",
        "Pedal trim right (helicopters / V-22)",
        Flight,
        Direction(1.),
        &[],
    ),
    cmd(
        "trim-set",
        "Trim set / force trim release (helicopters / V-22)",
        Flight,
        &[],
    ),
    cmd(
        "trim-centre",
        "Trim to centre (helicopters)",
        Flight,
        &["0"],
    ),
    cmd(
        "stability-level",
        "Stability level: Off, Damper, Attitude (VTOL)",
        Flight,
        &["Ctrl-Shift-a"],
    ),
    cmd(
        "stability-level=off",
        "Stability level Off (VTOL)",
        Flight,
        &[],
    ),
    cmd(
        "stability-level=damper",
        "Stability level Damper (VTOL)",
        Flight,
        &[],
    ),
    cmd(
        "stability-level=attitude",
        "Stability level Attitude (VTOL)",
        Flight,
        &[],
    ),
    e(
        "throttle",
        "Throttle / engine power lever",
        Flight,
        Lever,
        &[],
    ),
    e("throttle-rate", "Throttle rate (axis)", Flight, Axis, &[]),
    e("throttle-rate", "Throttle up", Flight, Direction(1.), &[]),
    e(
        "throttle-rate",
        "Throttle down",
        Flight,
        Direction(-1.),
        &[],
    ),
    cmd("throttle-preset=0", "Throttle idle", Flight, &["1"]),
    cmd("throttle-preset=0.25", "Throttle 25%", Flight, &["2"]),
    cmd("throttle-preset=0.5", "Throttle 50%", Flight, &["3"]),
    cmd("throttle-preset=0.75", "Throttle 75%", Flight, &["4"]),
    cmd("throttle-preset=1", "Throttle 100%", Flight, &["5"]),
    cmd(
        "throttle-preset=burner",
        "Throttle afterburner",
        Flight,
        &["6"],
    ),
    cmd("throttle-step=-0.05", "Throttle down 5%", Flight, &["7"]),
    cmd("throttle-step=0.05", "Throttle up 5%", Flight, &["8"]),
    cmd("burner", "Afterburner", Flight, &["Shift-b"]),
    cmd("autopilot", "Autopilot (heading/altitude)", Flight, &["a"]),
    cmd(
        "waypoint-autopilot",
        "Waypoint autopilot",
        Flight,
        &["Ctrl-a"],
    ),
    cmd(
        "hover-hold",
        "Hover hold autopilot (helicopters / V-22)",
        Flight,
        &["Ctrl-Alt-a"],
    ),
    cmd("eject", "Eject (press twice)", Systems, &["Shift-e"]),
    cmd("gear", "Landing gear", Systems, &["g"]),
    cmd("flaps", "Flaps", Systems, &["f"]),
    cmd("airbrake", "Airbrake / wheel brakes", Systems, &["b"]),
    cmd("hook", "Tailhook", Systems, &["h"]),
    cmd("engine", "Engine on/off", Systems, &["e"]),
    cmd("bay", "Weapon bays (F-22)", Systems, &["o"]),
    cmd("damage-report", "Damage report", Systems, &["d"]),
    e("fire", "Fire / release weapon", Weapons, Hold, &["Space"]),
    cmd("weapon-next", "Next weapon / NAV", Weapons, &["]"]),
    cmd("weapon-previous", "Previous weapon / NAV", Weapons, &["["]),
    cmd(
        "weapon-group-next",
        "Next gun candidate (AC-130)",
        Weapons,
        &["Ctrl-7"],
    ),
    cmd(
        "weapon-group-toggle",
        "Link/unlink candidate gun (AC-130)",
        Weapons,
        &["Ctrl-8"],
    ),
    cmd(
        "sight-designate",
        "Gunsight: designate under crosshair (AC-130)",
        Weapons,
        &["\\"],
    ),
    cmd(
        "sight-pin",
        "Gunsight: pin ground point (AC-130)",
        Weapons,
        &["Shift-\\"],
    ),
    cmd(
        "sight-zoom-in",
        "Gunsight: zoom in (AC-130)",
        Weapons,
        &["Shift-'"],
    ),
    cmd(
        "sight-zoom-out",
        "Gunsight: zoom out (AC-130)",
        Weapons,
        &["Shift-;"],
    ),
    e(
        "sight-x",
        "Gunsight: slew left/right (AC-130)",
        Weapons,
        Axis,
        &[],
    ),
    e(
        "sight-y",
        "Gunsight: slew up/down (AC-130)",
        Weapons,
        Axis,
        &[],
    ),
    e(
        "sight-left",
        "Gunsight: slew left (AC-130)",
        Weapons,
        Hold,
        &["Alt-ArrowLeft"],
    ),
    e(
        "sight-right",
        "Gunsight: slew right (AC-130)",
        Weapons,
        Hold,
        &["Alt-ArrowRight"],
    ),
    e(
        "sight-up",
        "Gunsight: slew up (AC-130)",
        Weapons,
        Hold,
        &["Alt-ArrowUp"],
    ),
    e(
        "sight-down",
        "Gunsight: slew down (AC-130)",
        Weapons,
        Hold,
        &["Alt-ArrowDown"],
    ),
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
    cmd(
        "clear-designation",
        "Clear designation",
        Weapons,
        &[";", "l"],
    ),
    cmd(
        "weapon-seeker-mode",
        "Seeker mode (bore/cued)",
        Weapons,
        &[],
    ),
    cmd("chaff", "Release chaff", Weapons, &["Insert"]),
    cmd("flare", "Release flare", Weapons, &["Delete"]),
    cmd(
        "jettison",
        "Jettison selected stores",
        Weapons,
        &["Shift-k"],
    ),
    cmd(
        "range-target",
        "Reset range target",
        Weapons,
        &["Ctrl-Shift-\\"],
    ),
    cmd("key:u", "IFF squawk on the target", Weapons, &["u"]),
    cmd(
        "incoming",
        "Incoming missile (range)",
        Weapons,
        &["Ctrl-Shift-i"],
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
    cmd("sensor-channel", "Cycle sensor channel", Sensors, &["m"]),
    cmd("sensor-infrared", "Infrared channel", Sensors, &["i"]),
    cmd("sensor-history", "Contact history", Sensors, &["y"]),
    cmd("range-down", "Scope range down", Sensors, &["."]),
    cmd("range-up", "Scope range up", Sensors, &[","]),
    cmd("airport-nav", "NAV / ILS mode", Sensors, &["n"]),
    cmd("waypoint-next", "Next waypoint", Sensors, &["w"]),
    cmd(
        "waypoint-previous",
        "Previous waypoint",
        Sensors,
        &["Shift-w"],
    ),
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
    cmd("view-track", "Track current target", View, &["F4"]),
    cmd("view-threat", "Player to inbound missile", View, &["F5"]),
    cmd("view-wing", "Player to wingman", View, &["F6"]),
    cmd("view-target", "Player to target", View, &["F7"]),
    cmd("view-target-player", "Target to player", View, &["F8"]),
    cmd("view-fly-by", "Fixed fly-by view", View, &["F9"]),
    cmd("view-missile", "Missile to its target", View, &["F12"]),
    cmd("store-view", "Save and open Other View", View, &["v"]),
    cmd(
        "view-target-track",
        "Target-relative tracking (Alt-F4 exits)",
        View,
        &[],
    ),
    cmd(
        "key:Ctrl-F1",
        "Last missile: forward view",
        View,
        &["Ctrl-F1"],
    ),
    cmd("key:Ctrl-F2", "Last missile: back view", View, &["Ctrl-F2"]),
    cmd("key:Ctrl-F3", "Last missile: up view", View, &["Ctrl-F3"]),
    cmd(
        "key:Ctrl-F4",
        "Last missile: tracking view",
        View,
        &["Ctrl-F4"],
    ),
    cmd(
        "key:Ctrl-F5",
        "Last missile: threat view",
        View,
        &["Ctrl-F5"],
    ),
    cmd(
        "key:Ctrl-F6",
        "Last missile: wingman view",
        View,
        &["Ctrl-F6"],
    ),
    cmd(
        "key:Ctrl-F7",
        "Last missile: to target view",
        View,
        &["Ctrl-F7"],
    ),
    cmd(
        "key:Ctrl-F8",
        "Last missile: target to reference view",
        View,
        &["Ctrl-F8"],
    ),
    cmd(
        "key:Ctrl-F9",
        "Last missile: fly-by view",
        View,
        &["Ctrl-F9"],
    ),
    cmd(
        "key:Ctrl-F10",
        "Last missile: external view",
        View,
        &["Ctrl-F10"],
    ),
    cmd(
        "key:Ctrl-F12",
        "Last missile: missile to target view",
        View,
        &["Ctrl-F12"],
    ),
    cmd("key:Alt-F1", "Target: forward view", View, &["Alt-F1"]),
    cmd("key:Alt-F2", "Target: back view", View, &["Alt-F2"]),
    cmd("key:Alt-F3", "Target: up view", View, &["Alt-F3"]),
    cmd("key:Alt-F5", "Target: threat view", View, &["Alt-F5"]),
    cmd("key:Alt-F6", "Target: wingman view", View, &["Alt-F6"]),
    cmd("key:Alt-F7", "Target: to target view", View, &["Alt-F7"]),
    cmd(
        "key:Alt-F8",
        "Target: target to reference view",
        View,
        &["Alt-F8"],
    ),
    cmd("key:Alt-F9", "Target: fly-by view", View, &["Alt-F9"]),
    cmd("key:Alt-F10", "Target: external view", View, &["Alt-F10"]),
    cmd(
        "key:Alt-F12",
        "Target: missile to target view",
        View,
        &["Alt-F12"],
    ),
    e("look-x", "Look left/right", View, Axis, &[]),
    e(
        "look-x",
        "Look left",
        View,
        Direction(-1.),
        &["Shift-ArrowLeft"],
    ),
    e(
        "look-x",
        "Look right",
        View,
        Direction(1.),
        &["Shift-ArrowRight"],
    ),
    e("look-y", "Look up/down", View, Axis, &[]),
    e("look-y", "Look up", View, Direction(1.), &["Shift-ArrowUp"]),
    e(
        "look-y",
        "Look down",
        View,
        Direction(-1.),
        &["Shift-ArrowDown"],
    ),
    e("head-yaw", "Head tracker yaw", View, Head, &[]),
    e("head-pitch", "Head tracker pitch", View, Head, &[]),
    cmd("center-look", "Center view", View, &["Numpad5", "Shift-/"]),
    mouse("zoom-in", "Zoom in", View, &["="], &["wheel:up"]),
    mouse("zoom-out", "Zoom out", View, &["-"], &["wheel:down"]),
    cmd("cockpit", "Cockpit art", View, &["Backspace"]),
    cmd("hud", "HUD", View, &["Shift-u"]),
    cmd("key:Shift-[", "Dim HUD", View, &["Shift-["]),
    cmd("key:Shift-]", "Brighten HUD", View, &["Shift-]"]),
    cmd("key:Shift-m", "Live map", View, &["Shift-m"]),
    cmd("key:Ctrl-t", "Show target info", View, &["Ctrl-t"]),
    cmd("key:Alt-1", "Wing: fly straight", Communication, &["Alt-1"]),
    cmd("key:Alt-2", "Wing: break left", Communication, &["Alt-2"]),
    cmd("key:Alt-3", "Wing: break right", Communication, &["Alt-3"]),
    cmd("key:Alt-4", "Wing: break low", Communication, &["Alt-4"]),
    cmd("key:Alt-5", "Wing: break high", Communication, &["Alt-5"]),
    cmd(
        "key:Alt-6",
        "Wing: approach target left",
        Communication,
        &["Alt-6"],
    ),
    cmd(
        "key:Alt-7",
        "Wing: approach target right",
        Communication,
        &["Alt-7"],
    ),
    cmd(
        "key:Alt-8",
        "Wing: approach target low",
        Communication,
        &["Alt-8"],
    ),
    cmd(
        "key:Alt-9",
        "Wing: approach target high",
        Communication,
        &["Alt-9"],
    ),
    cmd(
        "key:Alt-e",
        "Wing: engage my target",
        Communication,
        &["Alt-e"],
    ),
    cmd(
        "key:Alt-r",
        "Wing: engage from formation",
        Communication,
        &["Alt-r"],
    ),
    cmd(
        "key:Alt-a",
        "Wing: sort (a different bandit for each wingman)",
        Communication,
        &["Alt-a"],
    ),
    cmd(
        "key:Alt-w",
        "Wing: attack on contact",
        Communication,
        &["Alt-w"],
    ),
    cmd("key:Alt-p", "Wing: protect me", Communication, &["Alt-p"]),
    cmd("key:Alt-d", "Wing: disengage", Communication, &["Alt-d"]),
    cmd("key:Alt-b", "Wing: bug out", Communication, &["Alt-b"]),
    cmd(
        "key:Alt-t",
        "Wing: next formation",
        Communication,
        &["Alt-t"],
    ),
    cmd(
        "key:Alt-c",
        "Wing: loose/medium control",
        Communication,
        &["Alt-c"],
    ),
    cmd("key:Alt-h", "Wing: spacing", Communication, &["Alt-h"]),
    cmd("key:Alt-v", "Wing: stacking", Communication, &["Alt-v"]),
    cmd(
        "key:Alt-l",
        "Wing: land at selected airport",
        Communication,
        &["Alt-l"],
    ),
    cmd("key:Alt-s", "Radio silence", Communication, &["Alt-s"]),
    cmd(
        "key:Alt-n",
        "Monitor the battle net (the other flights' contact reports and attack calls)",
        Communication,
        &["Alt-n"],
    ),
    cmd(
        "key:Alt-0",
        "Address whole flight",
        Communication,
        &["Alt-0"],
    ),
    cmd(
        "key:Alt-Shift-1",
        "Address wingman 1",
        Communication,
        &["Alt-Shift-1"],
    ),
    cmd(
        "key:Alt-Shift-2",
        "Address wingman 2",
        Communication,
        &["Alt-Shift-2"],
    ),
    cmd(
        "key:Alt-Shift-3",
        "Address wingman 3",
        Communication,
        &["Alt-Shift-3"],
    ),
    cmd(
        "key:Alt-Shift-4",
        "Address wingman 4",
        Communication,
        &["Alt-Shift-4"],
    ),
    cmd(
        "key:Alt-Shift-e",
        "Reply to the flight: Engaging",
        Communication,
        &["Alt-Shift-e"],
    ),
    cmd(
        "key:Alt-Shift-w",
        "Reply to the flight: Winchester",
        Communication,
        &["Alt-Shift-w"],
    ),
    cmd(
        "key:Alt-Shift-b",
        "Reply to the flight: Bingo fuel",
        Communication,
        &["Alt-Shift-b"],
    ),
    cmd(
        "key:Alt-Shift-h",
        "Request help from the flight",
        Communication,
        &["Alt-Shift-h"],
    ),
    fixed("", "Chat line (network games only)", Communication, &["`"]),
    cmd("airport-next", "Next airport", Communication, &["Shift-n"]),
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
        &["Ctrl-Shift-r"],
    ),
    cmd(
        "airport-cancel",
        "Cancel approach",
        Communication,
        &["Ctrl-Shift-c"],
    ),
    fixed("menu", "Flight menu / back", Game, &["Escape"]),
    cmd("pause", "Pause", Game, &["Ctrl-p"]),
    cmd("key:c", "Time compression", Game, &["c"]),
    cmd("key:Shift-c", "Slow motion", Game, &["Shift-c"]),
    cmd("end-flight", "End mission", Game, &["Ctrl-q"]),
    cmd("key:Ctrl-v", "Valkyries music", Game, &["Ctrl-v"]),
    cmd("key:k", "Score board (network games)", Game, &["k"]),
    cmd("bookmark", "Mark replay moment", Game, &["Ctrl-b"]),
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

/// The kinds of aircraft whose stock keys differ (VTOL overhaul, design
/// 5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Airframe {
    /// Every aircraft without powered lift.
    Fixed,
    /// The AV-8 and Yak-141.
    VectorJet,
    /// The V-22.
    Tiltrotor,
    /// The AH-64, Mi-24 and CH-47.
    Helicopter,
}
impl Airframe {
    #[cfg(test)]
    pub const ALL: [Airframe; 4] = [
        Airframe::Fixed,
        Airframe::VectorJet,
        Airframe::Tiltrotor,
        Airframe::Helicopter,
    ];
    /// The kind of aircraft whose powered-lift levers are `vectoring`
    /// (nozzles), `conversion` (nacelles) and `collective`.
    pub fn of(vectoring: bool, conversion: bool, collective: bool) -> Self {
        match (vectoring, conversion, collective) {
            (true, ..) => Self::VectorJet,
            (_, true, _) => Self::Tiltrotor,
            (_, _, true) => Self::Helicopter,
            _ => Self::Fixed,
        }
    }
    #[cfg(test)]
    fn name(self) -> &'static str {
        match self {
            Self::Fixed => "fixed-wing",
            Self::VectorJet => "AV-8 / Yak-141",
            Self::Tiltrotor => "V-22",
            Self::Helicopter => "helicopters",
        }
    }
}
/// A set of [`Airframe`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Airframes(u8);
impl Airframes {
    pub const FIXED: Self = Self(1);
    pub const VECTOR_JET: Self = Self(2);
    pub const HELICOPTER: Self = Self(8);
    pub const ALL: Self = Self(15);
    pub const ROTORCRAFT: Self = Self(4 | 8);
    pub const POWERED_LIFT: Self = Self(2 | 4 | 8);
    pub const fn but(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
    pub fn contains(self, airframe: Airframe) -> bool {
        self.0 & (1 << airframe as u8) != 0
    }
    #[cfg(test)]
    fn disjoint(self, other: Self) -> bool {
        self.0 & other.0 == 0
    }
    /// Where the key acts, for the controls document: empty for every
    /// aircraft.
    #[cfg(test)]
    pub fn describe(self) -> String {
        let named = |set: Self| -> Vec<&str> {
            Airframe::ALL
                .into_iter()
                .filter(|a| set.contains(*a))
                .map(Airframe::name)
                .collect()
        };
        match Self::ALL.but(self) {
            Self(0) => String::new(),
            missing if named(missing).len() == 1 => format!("not on {}", named(missing)[0]),
            _ => format!("{} only", named(self).join(" and ")),
        }
    }
}

/// Stock keys whose action depends on the aircraft flown (VTOL overhaul,
/// design 5.6): the same key may name one action on some aircraft and
/// another on the rest, never two on the same aircraft. Every stock key not
/// listed here acts on every aircraft. A player's own bindings always act.
/// The aircraft-dependent meanings are opinionated, decided by John on
/// 2026-10-08 (design decisions 4 and 5).
pub const CONTEXTUAL: &[(&str, &str, Airframes)] = &[
    // Z and X: rudder, except on the vectoring jets, where they are the
    // retail nozzle keys.
    ("z", "yaw", Airframes::ALL.but(Airframes::VECTOR_JET)),
    ("x", "yaw", Airframes::ALL.but(Airframes::VECTOR_JET)),
    ("z", "nozzle-step-up", Airframes::VECTOR_JET),
    ("x", "nozzle-step-down", Airframes::VECTOR_JET),
    ("Shift-z", "nozzle-preset-forward", Airframes::VECTOR_JET),
    ("Shift-x", "nozzle-preset-vertical", Airframes::VECTOR_JET),
    // Ctrl+arrows: nozzle slew on the jets (and, doing nothing, on every
    // other aircraft as before), cyclic trim on the rotorcraft.
    (
        "Ctrl-ArrowUp",
        "vector-pitch-rate",
        Airframes::ALL.but(Airframes::ROTORCRAFT),
    ),
    (
        "Ctrl-ArrowDown",
        "vector-pitch-rate",
        Airframes::ALL.but(Airframes::ROTORCRAFT),
    ),
    ("Ctrl-ArrowLeft", "vector-yaw-rate", Airframes::FIXED),
    ("Ctrl-ArrowRight", "vector-yaw-rate", Airframes::FIXED),
    ("Ctrl-ArrowUp", "trim-pitch-rate", Airframes::ROTORCRAFT),
    ("Ctrl-ArrowDown", "trim-pitch-rate", Airframes::ROTORCRAFT),
    ("Ctrl-ArrowLeft", "trim-roll-rate", Airframes::ROTORCRAFT),
    ("Ctrl-ArrowRight", "trim-roll-rate", Airframes::ROTORCRAFT),
    // 0: nozzles and nacelles forward, except on the helicopters.
    (
        "0",
        "neutral-vector",
        Airframes::ALL.but(Airframes::HELICOPTER),
    ),
    ("0", "trim-centre", Airframes::HELICOPTER),
    ("Ctrl-Shift-a", "stability-level", Airframes::POWERED_LIFT),
];

/// The aircraft on which stock `key` performs `action`.
pub fn stock_airframes(key: &str, action: &str) -> Airframes {
    CONTEXTUAL
        .iter()
        .find(|(k, a, _)| *k == key && *a == action)
        .map_or(Airframes::ALL, |(.., set)| *set)
}

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
        "`" => "Backquote (~)".into(),
        "\\" => "Backslash".into(),
        "," => "Comma".into(),
        ";" => "Semicolon".into(),
        "Numpad5" => "Keypad 5".into(),
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
        "keyboard" | "replay-keyboard" => return key_label(control),
        "mouse" | "replay-mouse" => return mouse_label(control),
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
    /// The clash check (design 5.6): a stock key names one action on any
    /// one aircraft. Two entries share a key only through
    /// [`CONTEXTUAL`], on aircraft that do not overlap.
    #[test]
    fn stock_keys_are_not_shared_between_flight_entries() {
        for airframe in Airframe::ALL {
            let mut seen = std::collections::BTreeMap::new();
            for entry in ENTRIES.iter().filter(|e| !menu_only(e)) {
                for key in entry.keys {
                    if !stock_airframes(key, entry.action).contains(airframe) {
                        continue;
                    }
                    if let Some(other) = seen.insert((entry.replay_only(), *key), entry.label) {
                        // The menu key opens and backs out of the same menu.
                        assert_eq!(
                            *key, "Escape",
                            "{key} on {airframe:?}: {other} and {}",
                            entry.label
                        );
                    }
                }
            }
        }
        // Every contextual key is a stock key of its entry, and each key's
        // meanings cover disjoint aircraft.
        for (index, (key, action, set)) in CONTEXTUAL.iter().enumerate() {
            assert!(
                ENTRIES
                    .iter()
                    .any(|e| e.action == *action && e.keys.contains(key)),
                "{key} {action}"
            );
            for (other_key, other_action, other) in &CONTEXTUAL[index + 1..] {
                if other_key == key {
                    assert!(set.disjoint(*other), "{key}: {action} and {other_action}");
                }
            }
        }
    }
    /// The design's clash check of the new chords: Shift+Z, Shift+X,
    /// Ctrl+Alt+A and Ctrl+Shift+A were free; Ctrl+A and Alt+A stay
    /// distinct, Shift+A (AWACS radar link) stays free.
    #[test]
    fn the_vtol_chords_are_their_own() {
        let owners = |key: &str| -> Vec<&str> {
            ENTRIES
                .iter()
                .filter(|e| e.keys.contains(&key))
                .map(|e| e.action)
                .collect()
        };
        assert_eq!(owners("Ctrl-Alt-a"), ["hover-hold"]);
        assert_eq!(owners("Ctrl-Shift-a"), ["stability-level"]);
        assert_eq!(owners("Shift-z"), ["nozzle-preset-forward"]);
        assert_eq!(owners("Shift-x"), ["nozzle-preset-vertical"]);
        assert_eq!(owners("Ctrl-a"), ["waypoint-autopilot"]);
        assert_eq!(owners("Alt-a"), ["key:Alt-a"]);
        assert!(owners("Shift-a").is_empty());
        assert!(owners("Ctrl-z").is_empty() && owners("Ctrl-x").is_empty());
        assert_eq!(
            Airframes::ALL.but(Airframes::VECTOR_JET).describe(),
            "not on AV-8 / Yak-141"
        );
        assert_eq!(
            Airframes::ROTORCRAFT.describe(),
            "V-22 and helicopters only"
        );
        assert_eq!(Airframes::ALL.describe(), "");
    }
    #[test]
    fn the_phase_two_keys_are_listed_with_their_stock_keys() {
        for (action, key) in [
            ("key:u", "u"),
            ("key:Ctrl-t", "Ctrl-t"),
            ("key:k", "k"),
            ("key:Alt-Shift-e", "Alt-Shift-e"),
            ("key:Alt-Shift-w", "Alt-Shift-w"),
            ("key:Alt-Shift-b", "Alt-Shift-b"),
            ("key:Alt-Shift-h", "Alt-Shift-h"),
        ] {
            let entry = ENTRIES
                .iter()
                .find(|entry| entry.action == action)
                .unwrap_or_else(|| panic!("{action} is not in the catalog"));
            assert_eq!(entry.keys, [key], "{action}");
            assert!(entry.bindable() && entry.parsed().is_some(), "{action}");
        }
        // Alt+A is the data link's sort (slice G3c) and Alt+N monitors the
        // battle net (slice G8); the rest of stage G's keys stay free.
        for (action, key) in [("key:Alt-a", "Alt-a"), ("key:Alt-n", "Alt-n")] {
            assert_eq!(
                ENTRIES
                    .iter()
                    .find(|entry| entry.action == action)
                    .map(|entry| entry.keys),
                Some(&[key][..])
            );
        }
        for key in ["Alt-Shift-a", "Alt-Shift-n"] {
            assert!(
                ENTRIES.iter().all(|entry| !entry.keys.contains(&key)),
                "{key} is taken"
            );
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
                let keys = entry
                    .keys
                    .iter()
                    .map(|k| match stock_airframes(k, entry.action).describe() {
                        place if place.is_empty() => key_label(k),
                        place => format!("{} ({place})", key_label(k)),
                    })
                    .collect();
                let mut mouse: Vec<String> = entry.mouse.iter().map(|m| mouse_label(m)).collect();
                if matches!(entry.kind, Kind::Axis) && entry.action.starts_with("look-") {
                    mouse.push("Hold right button and drag".into());
                }
                let buttons = pad
                    .bindings
                    .iter()
                    .filter(|b| entry.bindable() && entry.matches(b))
                    .map(|b| {
                        let label = control_label(&b.device, &b.control, b.mode, true);
                        match b.mode {
                            Mode::Tap => format!("{label} (tap)"),
                            Mode::Long => format!("{label} (hold half a second)"),
                            _ => label,
                        }
                    })
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
    /// The gunsight's keys (John, 2026-10-09): each is its entry's only
    /// stock owner, none is a retail key (retail's Backslash is the IR/laser
    /// designate this key now does on the AC-130), and the live-fire range
    /// reset moved to Ctrl+Shift+Backslash to make room.
    #[test]
    fn the_gunsight_keys_are_their_own() {
        let owners = |key: &str| -> Vec<&str> {
            ENTRIES
                .iter()
                .filter(|e| e.keys.contains(&key))
                .map(|e| e.action)
                .collect()
        };
        assert_eq!(owners("\\"), ["sight-designate"]);
        assert_eq!(owners("Shift-\\"), ["sight-pin"]);
        assert_eq!(owners("Ctrl-Shift-\\"), ["range-target"]);
        assert_eq!(owners("Shift-'"), ["sight-zoom-in"]);
        assert_eq!(owners("Shift-;"), ["sight-zoom-out"]);
        for (key, action) in [
            ("Alt-ArrowLeft", "sight-left"),
            ("Alt-ArrowRight", "sight-right"),
            ("Alt-ArrowUp", "sight-up"),
            ("Alt-ArrowDown", "sight-down"),
        ] {
            assert_eq!(owners(key), [action], "{key}");
        }
        // The keys they sit beside keep their own meaning: the plain
        // apostrophe and semicolon still pick and clear a target, Shift+L is
        // still the landing request, and no other Alt+arrow is stock.
        assert_eq!(owners("'"), ["designate-visual"]);
        assert_eq!(owners(";"), ["clear-designation"]);
        assert_eq!(owners("l"), ["clear-designation"]);
        assert!(
            ENTRIES
                .iter()
                .flat_map(|e| e.keys)
                .filter(|k| k.starts_with("Alt-Arrow"))
                .all(|k| owners(k).len() == 1)
        );
        // None of them is a protected desktop-style key, and every gunsight
        // action is bindable and parses, so the controls screen can rebind it.
        for entry in ENTRIES.iter().filter(|e| e.action.starts_with("sight-")) {
            assert!(
                entry.bindable() && entry.parsed().is_some(),
                "{}",
                entry.label
            );
            assert!(!entry.keys.iter().any(|k| PROTECTED_KEYS.contains(k)));
            assert_eq!(entry.group, Weapons);
        }
        // The slew has an axis row for sticks and four hold rows for keys and
        // buttons; the captured control of each decides how it is bound.
        let kinds = |action: &str| -> Vec<Kind> {
            ENTRIES
                .iter()
                .filter(|e| e.action == action)
                .map(|e| e.kind)
                .collect()
        };
        assert_eq!(kinds("sight-x"), [Kind::Axis]);
        assert_eq!(kinds("sight-y"), [Kind::Axis]);
        assert_eq!(kinds("sight-left"), [Kind::Hold]);
    }
    #[test]
    fn scope_range_keys_follow_the_manual() {
        // Manual pp. 21, 94, 97: comma increases the radar and RWR range,
        // period decreases it.
        let keys = |action: &str| {
            ENTRIES
                .iter()
                .find(|entry| entry.action == action)
                .map(|entry| entry.keys.to_vec())
        };
        assert_eq!(keys("range-up"), Some(vec![","]));
        assert_eq!(keys("range-down"), Some(vec!["."]));
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
