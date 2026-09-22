use crate::{PilotCommand, PilotInput, Switch};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    Pitch,
    Roll,
    Yaw,
    Throttle,
    ThrottleRate,
    LookX,
    LookY,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Axis(Axis),
    Pilot(PilotCommand),
    Ui(String),
}
impl Action {
    pub fn parse(s: &str) -> Result<Self, String> {
        if let Some(mut key) = s.strip_prefix("key:") {
            for prefix in ["Ctrl-", "Alt-", "Shift-"] {
                if let Some(rest) = key.strip_prefix(prefix) {
                    key = rest;
                }
            }
            if (key.len() == 1 && key.is_ascii())
                || matches!(
                    key,
                    "Escape"
                        | "Enter"
                        | "Space"
                        | "Tab"
                        | "Backspace"
                        | "ArrowUp"
                        | "ArrowDown"
                        | "ArrowLeft"
                        | "ArrowRight"
                        | "PageUp"
                        | "PageDown"
                )
                || key
                    .strip_prefix('F')
                    .and_then(|v| v.parse::<u8>().ok())
                    .is_some_and(|n| (1..=12).contains(&n))
            {
                return Ok(Self::Ui(s.into()));
            }
            return Err("invalid keyboard shortcut action".into());
        }
        let axis = match s {
            "pitch" => Some(Axis::Pitch),
            "roll" => Some(Axis::Roll),
            "yaw" => Some(Axis::Yaw),
            "throttle" => Some(Axis::Throttle),
            "throttle-rate" => Some(Axis::ThrottleRate),
            "look-x" => Some(Axis::LookX),
            "look-y" => Some(Axis::LookY),
            _ => None,
        };
        if let Some(axis) = axis {
            return Ok(Self::Axis(axis));
        }
        for (name, switch) in [
            ("gear", Switch::Gear),
            ("flaps", Switch::Flaps),
            ("airbrake", Switch::Airbrake),
            ("hook", Switch::Hook),
            ("bay", Switch::Bay),
            ("engine", Switch::Engine),
            ("burner", Switch::Burner),
            ("radar", Switch::Radar),
            ("jammer", Switch::Jammer),
            ("autopilot", Switch::Autopilot),
            ("waypoint-autopilot", Switch::WaypointAutopilot),
        ] {
            if s == name {
                return Ok(Self::Pilot(PilotCommand::Toggle(switch)));
            }
        }
        if let Some(value) = s.strip_prefix("throttle=") {
            let value: f64 = value.parse().map_err(|_| "invalid throttle")?;
            if !value.is_finite() || !(0. ..=1.).contains(&value) {
                return Err("invalid throttle".into());
            }
            return Ok(Self::Pilot(PilotCommand::Throttle(value)));
        }
        if matches!(
            s,
            "fire"
                | "weapon-seeker-mode"
                | "weapon-next"
                | "weapon-previous"
                | "designate"
                | "clear-designation"
                | "master-arm"
                | "jettison"
                | "range-target"
                | "damage-class"
                | "fail-station"
                | "damage-report"
                | "damage-player"
                | "target-jammer"
                | "incoming"
                | "pause"
                | "menu"
                | "end-flight"
                | "restart"
                | "view-front"
                | "view-back"
                | "view-up"
                | "view-external"
                | "center-look"
                | "instrument-next"
                | "instrument-previous"
                | "range-down"
                | "range-up"
                | "radar-mode"
                | "sensor-channel"
                | "sensor-infrared"
                | "sensor-history"
                | "airport-next"
                | "airport-nav"
                | "airport-request-landing"
                | "airport-repeat"
                | "airport-cancel"
                | "cockpit"
                | "hud"
                | "zoom-in"
                | "zoom-out"
                | "menu-up"
                | "menu-down"
                | "menu-left"
                | "menu-right"
                | "menu-accept"
                | "menu-back"
        ) {
            return Ok(Self::Ui(s.into()));
        }
        for (prefix, lo, hi) in [("page-", 0, 9), ("instrument-", 1, 6), ("control-", 1, 4)] {
            if s.strip_prefix(prefix)
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| n >= lo && n <= hi)
            {
                return Ok(Self::Ui(s.into()));
            }
        }
        if let Some(tail) = s.strip_prefix("instrument-")
            && let Some((slot, button)) = tail.split_once("-control-")
            && slot.parse::<u8>().is_ok_and(|n| (1..=6).contains(&n))
            && button.parse::<u8>().is_ok_and(|n| (1..=4).contains(&n))
        {
            return Ok(Self::Ui(s.into()));
        }
        Err(format!("unknown action {s}"))
    }
    fn allowed_paused(&self) -> bool {
        matches!(self, Self::Ui(s) if s == "pause" || s == "menu" || s.starts_with("menu-"))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    pub min: f64,
    pub center: f64,
    pub max: f64,
    pub deadzone: f64,
    pub curve: f64,
    pub scale: f64,
}
impl Default for Calibration {
    fn default() -> Self {
        Self {
            min: -1.,
            center: 0.,
            max: 1.,
            deadzone: 0.08,
            curve: 1.,
            scale: 1.,
        }
    }
}
impl Calibration {
    pub fn validate(&self) -> bool {
        [
            self.min,
            self.center,
            self.max,
            self.deadzone,
            self.curve,
            self.scale,
        ]
        .iter()
        .all(|v| v.is_finite())
            && (self.max - self.min).is_finite()
            && self.min < self.center
            && self.center < self.max
            && (0. ..0.95).contains(&self.deadzone)
            && (0.1..=5.).contains(&self.curve)
            && self.scale.abs() <= 1.
    }
    pub fn apply(&self, value: f64, unit: bool) -> f64 {
        if !value.is_finite() {
            return 0.;
        }
        if unit {
            let v = ((value - self.min) / (self.max - self.min)).clamp(0., 1.);
            return if self.scale < 0. { 1. - v } else { v };
        }
        let v = if value >= self.center {
            (value - self.center) / (self.max - self.center)
        } else {
            (value - self.center) / (self.center - self.min)
        }
        .clamp(-1., 1.);
        v.signum()
            * ((v.abs() - self.deadzone).max(0.) / (1. - self.deadzone)).powf(self.curve)
            * self.scale
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Axis,
    Unit,
    Hold(f64),
    Trigger(f64),
    HoldState,
    Press,
    Release,
    Switch,
    Follow,
    Position(i32),
    Delta,
}
#[derive(Clone, Debug)]
pub struct Binding {
    pub device: String,
    pub control: String,
    pub action: Action,
    pub mode: Mode,
    pub calibration: Calibration,
    pub priority: i32,
}
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub aliases: BTreeMap<String, String>,
    pub bindings: Vec<Binding>,
    pub rumble: bool,
    pub gamepad_defaults: bool,
}
impl Profile {
    /// Bounded versioned text; aliases and control names are whitespace-free identifiers.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 256 * 1024 {
            return Err("input profile exceeds 256 KiB".into());
        }
        let mut p = Self::default();
        let mut version = false;
        for (line, raw) in text.lines().enumerate() {
            let w: Vec<_> = raw
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .collect();
            if w.is_empty() {
                continue;
            }
            let result = (|| -> Result<(), String> {
                if !version {
                    if w == ["tore-input", "1"] {
                        version = true;
                        return Ok(());
                    }
                    return Err("expected tore-input 1".into());
                }
                match w.as_slice() {
                    ["alias", name, id] if p.aliases.len() < 64 => {
                        if p.aliases.insert((*name).into(), (*id).into()).is_some() {
                            return Err("duplicate alias".into());
                        }
                    }
                    ["gamepad-defaults", value] if matches!(*value, "on" | "off") => {
                        p.gamepad_defaults = *value == "on";
                    }
                    ["rumble", value] if matches!(*value, "on" | "off") => {
                        p.rumble = *value == "on"
                    }
                    ["bind", device, control, action, mode, rest @ ..]
                        if p.bindings.len() < 1024 =>
                    {
                        let action = Action::parse(action)?;
                        let mode = match *mode {
                            "axis" => Mode::Axis,
                            "unit" => Mode::Unit,
                            "positive" => Mode::Hold(1.),
                            "negative" => Mode::Hold(-1.),
                            "trigger-positive" => Mode::Trigger(1.),
                            "trigger-negative" => Mode::Trigger(-1.),
                            "hold" => Mode::HoldState,
                            "press" => Mode::Press,
                            "release" => Mode::Release,
                            "switch" => Mode::Switch,
                            "follow" => Mode::Follow,
                            "delta" => Mode::Delta,
                            m if m.starts_with("position=") => {
                                Mode::Position(m[9..].parse().map_err(|_| "bad position")?)
                            }
                            _ => return Err("unknown binding mode".into()),
                        };
                        let mut c = Calibration::default();
                        let mut priority = 10;
                        if !rest.is_empty() {
                            if rest.len() != 7 {
                                return Err(
                                    "expected min center max deadzone curve scale priority".into(),
                                );
                            }
                            let values: Result<Vec<f64>, _> =
                                rest[..6].iter().map(|s| s.parse()).collect();
                            let v = values.map_err(|_| "invalid calibration")?;
                            c = Calibration {
                                min: v[0],
                                center: v[1],
                                max: v[2],
                                deadzone: v[3],
                                curve: v[4],
                                scale: v[5],
                            };
                            priority = rest[6].parse().map_err(|_| "invalid priority")?;
                        }
                        if !c.validate() {
                            return Err("invalid calibration bounds".into());
                        }
                        let compatible = match mode {
                            Mode::Axis | Mode::Hold(_) | Mode::Trigger(_) => {
                                matches!(action, Action::Axis(a) if a != Axis::Throttle)
                            }
                            Mode::Unit => action == Action::Axis(Axis::Throttle),
                            Mode::HoldState if action == Action::Ui("fire".into()) => true,
                            Mode::Switch | Mode::Follow | Mode::HoldState => {
                                matches!(action, Action::Pilot(PilotCommand::Toggle(_)))
                            }
                            Mode::Delta => {
                                action == Action::Axis(Axis::ThrottleRate)
                                    || matches!(action, Action::Ui(_))
                            }
                            _ => !matches!(action, Action::Axis(_)),
                        };
                        if action == Action::Ui("fire".into()) && mode != Mode::HoldState {
                            return Err("fire requires hold behavior".into());
                        }
                        if control.contains('+')
                            && (device == &"keyboard"
                                || control.split('+').count() != 2
                                || control.split('+').any(str::is_empty)
                                || control.split_once('+').is_some_and(|(m, c)| m == c))
                        {
                            return Err("invalid two-control chord".into());
                        }
                        if !compatible {
                            return Err("mode incompatible with action".into());
                        }
                        p.bindings.push(Binding {
                            device: (*device).into(),
                            control: (*control).into(),
                            action,
                            mode,
                            calibration: c,
                            priority,
                        });
                    }
                    _ => return Err("invalid profile directive or limit exceeded".into()),
                }
                Ok(())
            })();
            result.map_err(|e| format!("input profile line {}: {e}", line + 1))?;
        }
        if !version {
            return Err("missing input profile version".into());
        }
        Ok(p)
    }
}
#[derive(Clone, Debug, Default)]
struct Contribution {
    raw: f64,
    value: f64,
    armed: bool,
    seen: bool,
    pickup: bool,
    previous: f64,
}
/// Physical baseline events initialize state without producing synthetic presses.
#[derive(Clone, Debug)]
pub struct Event {
    pub device: String,
    pub control: String,
    pub value: f64,
    pub baseline: bool,
}
#[derive(Default)]
pub struct Resolver {
    pub profile: Profile,
    states: BTreeMap<(usize, String), Contribution>,
    owners: BTreeMap<Axis, (usize, String)>,
    events: VecDeque<(String, Action)>,
    paused: bool,
    focused: bool,
    overflow: bool,
    held_switches: BTreeSet<Switch>,
    physical: BTreeMap<(String, String), f64>,
}
impl Resolver {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            focused: true,
            ..Self::default()
        }
    }
    pub fn bound(&self, device: &str, control: &str) -> bool {
        self.profile
            .bindings
            .iter()
            .any(|b| self.matches(b, device) && b.control == control)
    }
    fn matches(&self, binding: &Binding, device: &str) -> bool {
        binding.device == device
            || (binding.device == "*" && device != "keyboard")
            || self
                .profile
                .aliases
                .get(&binding.device)
                .is_some_and(|id| id == device)
    }
    /// Feedback eligibility includes assigned controls at rest, excluding UI-only boxes.
    pub fn flight_bound(&self, device: &str, control: &str) -> bool {
        self.profile.bindings.iter().any(|b| {
            self.matches(b, device)
                && b.control.rsplit('+').next() == Some(control)
                && (matches!(
                    b.action,
                    Action::Pilot(_)
                        | Action::Axis(
                            Axis::Pitch
                                | Axis::Roll
                                | Axis::Yaw
                                | Axis::Throttle
                                | Axis::ThrottleRate
                        )
                ) || b.action == Action::Ui("fire".into()))
        })
    }
    pub fn context(&mut self, paused: bool, focused: bool) {
        if self.paused == paused && self.focused == focused {
            return;
        }
        self.paused = paused;
        self.focused = focused;
        self.events.clear();
        self.owners.clear();
        for ((index, _), s) in &mut self.states {
            let binding = &self.profile.bindings[*index];
            let neutral = match binding.mode {
                Mode::Axis => binding.calibration.apply(s.raw, false).abs() <= 0.02,
                Mode::Unit => true,
                Mode::Trigger(_) => {
                    binding.calibration.apply(s.raw, true) <= binding.calibration.deadzone
                }
                Mode::Position(n) => s.raw != n as f64,
                _ => s.raw == 0.,
            };
            s.armed = focused && neutral;
            s.value = if focused && !paused && binding.mode == Mode::Unit {
                binding.calibration.apply(s.raw, true)
            } else {
                0.
            };
            s.previous = s.value;
            s.pickup = false;
        }
    }
    /// Modifier-first chords use `button-id+control-id`. Entering/leaving a
    /// layer baselines its controls, requiring neutral before a new action.
    pub fn event(&mut self, event: Event) {
        if event.device == "keyboard" || !event.value.is_finite() {
            self.resolve_event(event);
            return;
        }
        self.physical
            .insert((event.device.clone(), event.control.clone()), event.value);
        let chords: BTreeSet<_> = self
            .profile
            .bindings
            .iter()
            .filter(|b| self.matches(b, &event.device))
            .filter_map(|b| {
                b.control
                    .split_once('+')
                    .map(|(m, c)| (m.to_owned(), c.to_owned()))
            })
            .collect();
        let modifier_changed = chords.iter().any(|(m, _)| m == &event.control);
        if modifier_changed {
            let mut controls = BTreeSet::new();
            for (m, c) in &chords {
                if m != &event.control {
                    continue;
                }
                let raw = *self
                    .physical
                    .get(&(event.device.clone(), c.clone()))
                    .unwrap_or(&0.);
                self.resolve_event(Event {
                    device: event.device.clone(),
                    control: format!("{m}+{c}"),
                    value: if event.value != 0. { raw } else { 0. },
                    baseline: true,
                });
                controls.insert(c.clone());
            }
            for c in controls {
                let raw = *self
                    .physical
                    .get(&(event.device.clone(), c.clone()))
                    .unwrap_or(&0.);
                self.resolve_event(Event {
                    device: event.device.clone(),
                    control: c,
                    value: if event.value != 0. { 0. } else { raw },
                    baseline: true,
                });
            }
        }
        let mut consumed = false;
        for (m, c) in chords {
            if c == event.control
                && self
                    .physical
                    .get(&(event.device.clone(), m.clone()))
                    .is_some_and(|v| *v != 0.)
            {
                consumed = true;
                self.resolve_event(Event {
                    control: format!("{m}+{c}"),
                    ..event.clone()
                });
            }
        }
        if !consumed {
            self.resolve_event(event);
        }
    }
    pub fn held(&self, name: &str) -> bool {
        !self.paused
            && self.focused
            && self.states.iter().any(|((i, _), s)| {
                s.armed
                    && s.value != 0.
                    && self.profile.bindings[*i].mode == Mode::HoldState
                    && self.profile.bindings[*i].action == Action::Ui(name.into())
            })
    }
    fn resolve_event(&mut self, event: Event) {
        if !event.value.is_finite() {
            self.disconnect(&event.device);
            self.overflow = true;
            return;
        }
        for index in 0..self.profile.bindings.len() {
            let b = &self.profile.bindings[index];
            if !self.matches(b, &event.device) || b.control != event.control {
                continue;
            }
            let s = self
                .states
                .entry((index, event.device.clone()))
                .or_default();
            let previous = s.raw;
            let initial = event.baseline || !s.seen;
            s.raw = event.value;
            s.seen = true;
            let allowed = self.focused && (!self.paused || b.action.allowed_paused());
            let mut output = None;
            match b.mode {
                Mode::Trigger(sign) => {
                    let unit = b.calibration.apply(event.value, true);
                    let v = ((unit - b.calibration.deadzone).max(0.)
                        / (1. - b.calibration.deadzone))
                        .powf(b.calibration.curve)
                        * sign;
                    if initial || !allowed {
                        s.armed = v == 0.;
                        s.value = 0.;
                    }
                    if v == 0. {
                        s.armed = true;
                        s.value = 0.;
                    } else if allowed && s.armed {
                        s.value = v;
                    }
                }
                Mode::Axis | Mode::Unit => {
                    let v = b.calibration.apply(event.value, b.mode == Mode::Unit);
                    s.previous = if initial { v } else { s.value };
                    if initial {
                        s.armed = b.mode == Mode::Unit || v.abs() <= 0.02;
                        s.pickup = false;
                    }
                    if !allowed {
                        s.value = 0.;
                        s.armed = false;
                    } else {
                        if b.mode == Mode::Unit || v.abs() <= 0.02 {
                            s.armed = true;
                        }
                        s.value = if s.armed { v } else { 0. };
                    }
                }
                Mode::Hold(_) | Mode::HoldState => {
                    let sign = match b.mode {
                        Mode::Hold(sign) => sign,
                        _ => 1.,
                    };
                    if initial || !allowed {
                        s.armed = event.value == 0.;
                        s.value = 0.;
                    }
                    if event.value == 0. {
                        s.armed = true;
                        s.value = 0.;
                    } else if allowed && s.armed {
                        s.value = sign;
                        if b.mode == Mode::HoldState
                            && let Action::Pilot(PilotCommand::Toggle(switch)) = b.action
                        {
                            self.held_switches.insert(switch);
                        }
                    }
                }
                Mode::Switch | Mode::Follow => {
                    if allowed
                        && ((!initial && previous != event.value)
                            || (initial && b.mode == Mode::Follow))
                        && let Action::Pilot(PilotCommand::Toggle(switch)) = b.action
                    {
                        output = Some(Action::Pilot(PilotCommand::Set(switch, event.value != 0.)));
                    }
                }
                Mode::Delta => {
                    if allowed && !initial {
                        let count = (event.value as i32).clamp(-32, 32);
                        if b.action == Action::Axis(Axis::ThrottleRate) {
                            output = Some(Action::Pilot(PilotCommand::AdjustThrottle(
                                count as f64 * 0.01 * b.calibration.scale,
                            )));
                        } else {
                            // Sign selects this binding's action; use negative scale for counterclockwise.
                            if (count as f64 * b.calibration.scale) > 0. {
                                for _ in 0..count.unsigned_abs() {
                                    self.events
                                        .push_back((event.device.clone(), b.action.clone()));
                                }
                            }
                        }
                    }
                }
                Mode::Press | Mode::Release | Mode::Position(_) => {
                    let active = match b.mode {
                        Mode::Position(n) => event.value == n as f64,
                        _ => event.value != 0.,
                    };
                    let was_active = match b.mode {
                        Mode::Position(n) => previous == n as f64,
                        _ => previous != 0.,
                    };
                    if initial || !allowed {
                        s.armed = !active;
                    } else if s.armed
                        && active != was_active
                        && (if b.mode == Mode::Release {
                            !active
                        } else {
                            active
                        })
                    {
                        output = Some(b.action.clone());
                    }
                    if !active {
                        s.armed = true;
                    }
                }
            }
            if let Some(action) = output {
                self.events.push_back((event.device.clone(), action));
            }
        }
        if self.events.len() > 1024 {
            self.events.clear();
            self.overflow = true;
        }
    }
    pub fn take_overflow(&mut self) -> bool {
        std::mem::take(&mut self.overflow)
    }
    pub fn drain(&mut self) -> Vec<(String, Action)> {
        self.events.drain(..).collect()
    }
    pub fn disconnect(&mut self, device: &str) -> bool {
        let primary = self.owners.iter().any(|(axis, (_, d))| {
            d == device && matches!(axis, Axis::Pitch | Axis::Roll | Axis::Yaw | Axis::Throttle)
        });
        self.physical.retain(|(d, _), _| d != device);
        self.states.retain(|(_, d), _| d != device);
        self.owners.retain(|_, (_, d)| d != device);
        self.events.retain(|(d, _)| d != device);
        primary
    }
    pub fn override_throttle(&mut self) {
        self.owners.remove(&Axis::Throttle);
        for ((i, _), s) in &mut self.states {
            if self.profile.bindings[*i].action == Action::Axis(Axis::Throttle) {
                s.pickup = false;
                s.previous = s.value;
            }
        }
    }
    fn axis(&mut self, axis: Axis, throttle: f64) -> f64 {
        let mut candidates = Vec::new();
        let mut groups: BTreeMap<(String, i32), ((usize, String), f64)> = BTreeMap::new();
        for (key, s) in &mut self.states {
            let b = &self.profile.bindings[key.0];
            if b.action != Action::Axis(axis) || !s.armed {
                continue;
            }
            if matches!(b.mode, Mode::Hold(_) | Mode::Trigger(_)) {
                let group = groups
                    .entry((key.1.clone(), b.priority))
                    .or_insert((key.clone(), 0.));
                group.1 += s.value;
                continue;
            }
            if axis == Axis::Throttle {
                // Pickup only when close to the current commanded position.
                if (s.value - throttle).abs() <= 0.04
                    || (s.previous - throttle) * (s.value - throttle) < 0.
                {
                    s.pickup = true;
                }
                s.previous = s.value;
                if !s.pickup {
                    continue;
                }
            } else if s.value.abs() <= 0.001 {
                continue;
            }
            candidates.push((b.priority, key.clone(), s.value));
        }
        for ((_, priority), (key, value)) in groups {
            if value.abs() > 0.001 {
                candidates.push((priority, key, value.clamp(-1., 1.)));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        if let Some(owner) = self.owners.get(&axis)
            && let Some(c) = candidates.iter().find(|c| &c.1 == owner)
            && candidates.first().is_some_and(|first| first.0 == c.0)
        {
            return c.2;
        }
        if let Some((_, key, value)) = candidates.first() {
            self.owners.insert(axis, key.clone());
            *value
        } else {
            self.owners.remove(&axis);
            if axis == Axis::Throttle { throttle } else { 0. }
        }
    }
    pub fn frame(&mut self, throttle: f64) -> (PilotInput, [f32; 2]) {
        if self.paused || !self.focused {
            return (PilotInput::default(), [0.; 2]);
        }
        let throttle_rate = self.axis(Axis::ThrottleRate, throttle);
        if throttle_rate != 0. {
            self.override_throttle();
        }
        let t = if throttle_rate == 0. {
            self.axis(Axis::Throttle, throttle)
        } else {
            throttle
        };
        let mut commands = Vec::new();
        for switch in &self.held_switches {
            let active = self.states.iter().any(|((i, _), s)| {
                s.armed
                    && s.value != 0.
                    && self.profile.bindings[*i].mode == Mode::HoldState
                    && self.profile.bindings[*i].action
                        == Action::Pilot(PilotCommand::Toggle(*switch))
            });
            commands.push(PilotCommand::Set(*switch, active));
        }
        let mut follows: BTreeMap<Switch, (i32, PilotCommand)> = BTreeMap::new();
        for ((i, _), s) in &self.states {
            let b = &self.profile.bindings[*i];
            if b.mode == Mode::Follow
                && s.seen
                && let Action::Pilot(PilotCommand::Toggle(switch)) = b.action
            {
                let command = PilotCommand::Set(switch, s.raw != 0.);
                if follows
                    .get(&switch)
                    .is_none_or(|(priority, _)| b.priority > *priority)
                {
                    follows.insert(switch, (b.priority, command));
                }
            }
        }
        commands.extend(follows.values().map(|(_, c)| *c));
        let input = PilotInput {
            pitch: self.axis(Axis::Pitch, throttle),
            roll: self.axis(Axis::Roll, throttle),
            yaw: self.axis(Axis::Yaw, throttle),
            throttle_rate,
            throttle: self.owners.contains_key(&Axis::Throttle).then_some(t),
            commands,
        };
        let look = self.look();
        (input, look)
    }
    /// Presentation-only look resolution cannot acquire throttle/flight-axis ownership.
    pub fn look(&mut self) -> [f32; 2] {
        if self.paused || !self.focused {
            return [0.; 2];
        }
        [
            self.axis(Axis::LookX, 0.) as f32,
            self.axis(Axis::LookY, 0.) as f32,
        ]
    }
    pub fn active_devices(&self) -> BTreeSet<String> {
        self.owners.values().map(|(_, d)| d.clone()).collect()
    }
}
