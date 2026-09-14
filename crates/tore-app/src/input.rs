//! Desktop bridge: native samples and keyboard bindings resolve before simulation ticks.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};
use tore_input::{
    Action, Event, FeedbackEvent, FeedbackMixer, FeedbackUpdate, PilotCommand, PilotInput, Profile,
    Resolver,
};
use tore_input_native::{Backend, Device, Kind, Notification};
const DEFAULTS: &str = "tore-input 1\n\
bind keyboard flight-pitch pitch axis -1 0 1 0 1 1 100\n\
bind keyboard flight-roll roll axis -1 0 1 0 1 1 100\n\
bind keyboard flight-yaw yaw axis -1 0 1 0 1 1 100\n\
bind keyboard flight-throttle throttle-rate axis -1 0 1 0 1 1 100\n\
bind keyboard flight-look-x look-x axis -1 0 1 0 1 1 100\n\
bind keyboard flight-look-y look-y axis -1 0 1 0 1 1 100\n";
pub struct Input {
    pub resolver: Resolver,
    automatic: bool,
    pub observed: Vec<Event>,
    profile_path: Option<std::path::PathBuf>,
    pub devices: BTreeMap<String, Device>,
    backend: Backend,
    feedback_mixer: FeedbackMixer,
    feedback_targets: BTreeSet<String>,
    feedback_pending: Option<FeedbackUpdate>,
    commands: Vec<PilotCommand>,
    key_claims: BTreeMap<String, String>,
    key_values: BTreeMap<String, f64>,
    context: (bool, bool),
    pub next_poll: Instant,
}
impl Input {
    pub fn new(path: Option<&Path>, native: bool) -> Result<Self, String> {
        let mut profile = Profile::parse(DEFAULTS)?;
        profile.gamepad_defaults = path.is_none();
        if let Some(path) = path {
            let mut text = String::new();
            std::fs::File::open(path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .take(256 * 1024 + 1)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            let custom = Profile::parse(&text)?;
            profile.aliases = custom.aliases;
            profile.rumble = custom.rumble;
            profile.gamepad_defaults = custom.gamepad_defaults;
            profile.bindings.extend(custom.bindings);
        }
        let mut resolver = Resolver::new(profile);
        for control in [
            "flight-pitch",
            "flight-roll",
            "flight-yaw",
            "flight-throttle",
            "flight-look-x",
            "flight-look-y",
        ] {
            resolver.event(Event {
                device: "keyboard".into(),
                control: control.into(),
                value: 0.,
                baseline: true,
            });
        }
        let automatic = resolver.profile.gamepad_defaults;
        Ok(Self {
            resolver,
            automatic,
            observed: vec![],
            profile_path: path.map(Path::to_path_buf),
            devices: BTreeMap::new(),
            backend: if native {
                Backend::start()
            } else {
                Backend::disabled()
            },
            feedback_mixer: FeedbackMixer::default(),
            feedback_targets: BTreeSet::new(),
            feedback_pending: None,
            commands: vec![],
            key_claims: BTreeMap::new(),
            key_values: BTreeMap::new(),
            context: (false, true),
            next_poll: Instant::now(),
        })
    }
    pub fn settings_profile(&self) -> Profile {
        let mut profile = self.resolver.profile.clone();
        profile
            .bindings
            .retain(|b| !(b.device == "keyboard" && b.control.starts_with("flight-")));
        profile
    }
    pub fn save_settings(&mut self, profile: &Profile) -> Result<(), String> {
        if profile.bindings.iter().any(|b| b.control == "unassigned") {
            return Err("Capture or select an input for every new binding".into());
        }
        let text = profile.to_text()?;
        let path = match &self.profile_path {
            Some(p) => p.clone(),
            None => crate::assets::data_directory()
                .map_err(|e| e.to_string())?
                .join("input-v1.conf"),
        };
        crate::preferences::write(&path, &text)
            .map_err(|e| format!("Could not save controls: {e}"))?;
        self.stop();
        let mut complete = Profile::parse(DEFAULTS)?;
        complete.aliases = profile.aliases.clone();
        complete.rumble = profile.rumble;
        complete.gamepad_defaults = profile.gamepad_defaults;
        complete.bindings.extend(profile.bindings.clone());
        self.resolver = Resolver::new(complete);
        self.resolver.context(self.context.0, self.context.1);
        for control in [
            "flight-pitch",
            "flight-roll",
            "flight-yaw",
            "flight-throttle",
            "flight-look-x",
            "flight-look-y",
        ] {
            self.resolver.event(Event {
                device: "keyboard".into(),
                control: control.into(),
                value: 0.,
                baseline: true,
            });
        }
        for d in self.devices.values() {
            for c in &d.controls {
                self.resolver.event(normalize(
                    d,
                    Event {
                        device: d.id.clone(),
                        control: c.id.clone(),
                        value: c.value,
                        baseline: true,
                    },
                ));
            }
        }
        self.key_claims.clear();
        self.key_values.clear();
        self.automatic = profile.gamepad_defaults;
        self.profile_path = Some(path);
        Ok(())
    }
    pub fn context(&mut self, paused: bool, focused: bool) {
        if self.context != (paused, focused) {
            self.context = (paused, focused);
            self.commands.clear();
            self.resolver.context(paused, focused);
            self.feedback_mixer.clear();
            self.feedback_pending = None;
            self.feedback_targets.clear();
            self.backend.stop();
        }
    }
    pub fn stop(&mut self) {
        self.feedback_mixer.clear();
        self.feedback_pending = None;
        self.feedback_targets.clear();
        self.backend.stop();
        self.commands.clear();
    }
    pub fn queue(&mut self, command: PilotCommand) {
        if self.context.0 || !self.context.1 {
            return;
        }
        if matches!(
            command,
            PilotCommand::Throttle(_) | PilotCommand::AdjustThrottle(_)
        ) {
            self.resolver.override_throttle();
        }
        if self.commands.len() < 256 {
            self.commands.push(command);
        }
    }
    pub fn claimed(&self, key: &str) -> bool {
        self.key_claims.contains_key(key)
    }
    pub fn key(
        &mut self,
        key: &str,
        pressed: bool,
        modifiers: winit::keyboard::ModifiersState,
    ) -> bool {
        if !pressed {
            if let Some(control) = self.key_claims.remove(key) {
                self.key_value(&control, 0.);
                return true;
            }
            return false;
        }
        if self.key_claims.contains_key(key) {
            return true;
        }
        let control = format!(
            "{}{}{}{}{}",
            if modifiers.control_key() { "Ctrl-" } else { "" },
            if modifiers.alt_key() { "Alt-" } else { "" },
            if modifiers.shift_key() { "Shift-" } else { "" },
            if modifiers.super_key() { "Super-" } else { "" },
            key
        );
        if self.resolver.bound("keyboard", &control) {
            self.key_claims.insert(key.into(), control.clone());
            self.key_value(&control, 1.);
            return true;
        }
        false
    }
    fn key_value(&mut self, control: &str, value: f64) {
        if !self.key_values.contains_key(control) {
            self.resolver.event(Event {
                device: "keyboard".into(),
                control: control.into(),
                value: 0.,
                baseline: true,
            });
        }
        if self.key_values.insert(control.into(), value) != Some(value) {
            self.resolver.event(Event {
                device: "keyboard".into(),
                control: control.into(),
                value,
                baseline: false,
            });
        }
    }
    pub fn poll(&mut self) -> (Vec<Action>, bool, Vec<String>) {
        self.observed.clear();
        self.next_poll = Instant::now() + Duration::from_millis(8);
        let mut lost = false;
        let mut warnings = vec![];
        for notification in self.backend.drain() {
            match notification {
                Notification::Connected(d) => {
                    if self.automatic
                        && !self
                            .resolver
                            .profile
                            .bindings
                            .iter()
                            .any(|b| b.device == d.id)
                    {
                        let defaults = gamepad_defaults(&d);
                        if self.resolver.profile.bindings.len() + defaults.len() <= 1024 {
                            if !defaults.is_empty() {
                                eprintln!(
                                    "Input: standard Linux gamepad bindings enabled; see docs/INPUT.md"
                                );
                            }
                            self.resolver.profile.bindings.extend(defaults);
                        }
                    }
                    if self.devices.contains_key(&d.id) {
                        lost |= self.resolver.disconnect(&d.id);
                    }
                    for c in &d.controls {
                        self.resolver.event(normalize(
                            &d,
                            Event {
                                device: d.id.clone(),
                                control: c.id.clone(),
                                value: c.value,
                                baseline: true,
                            },
                        ));
                    }
                    eprintln!("Input connected: {} ({})", d.name, d.id);
                    self.devices.insert(d.id.clone(), d);
                }
                Notification::Disconnected(id) => {
                    lost |= self.resolver.disconnect(&id);
                    self.devices.remove(&id);
                    self.feedback_targets.remove(&id);
                    eprintln!("Input disconnected: {id}");
                }
                Notification::Input(event) => {
                    self.observed.push(event.clone());
                    if let Some(d) = self.devices.get_mut(&event.device) {
                        if let Some(c) = d.controls.iter_mut().find(|c| c.id == event.control) {
                            c.value = event.value;
                        }
                        self.resolver.event(normalize(d, event));
                    }
                }
                Notification::Warning(message) => warnings.push(message),
                Notification::Feedback(id, Err(error)) => {
                    if let Some(d) = self.devices.get_mut(&id) {
                        d.rumble = false;
                    }
                    self.feedback_targets.remove(&id);
                    self.backend.stop();
                    warnings.push(format!(
                        "feedback disabled until reconnect for {id}: {error}"
                    ))
                }
                Notification::Feedback(_, Ok(())) => {}
                Notification::Overflow => {
                    self.feedback_mixer.clear();
                    self.feedback_pending = None;
                    self.feedback_targets.clear();
                    for id in self.devices.keys() {
                        self.resolver.disconnect(id);
                    }
                    self.devices.clear();
                    lost = true;
                    warnings
                        .push("Input queue overflow: controls released; resume explicitly".into());
                }
            }
        }
        if self.resolver.take_overflow() {
            lost = true;
            warnings.push("Invalid or overflowing input stream: resume explicitly".into());
        }
        let actions = self
            .resolver
            .drain()
            .into_iter()
            .map(|(_, action)| action)
            .collect();
        (actions, lost, warnings)
    }
    pub fn frame(&mut self, keys: &BTreeSet<String>, throttle: f64) -> (PilotInput, [f32; 2]) {
        let held = |key: &str| f64::from(keys.contains(key));
        for (control, value) in [
            ("flight-pitch", held("ArrowDown") - held("ArrowUp")),
            ("flight-roll", held("ArrowRight") - held("ArrowLeft")),
            ("flight-yaw", held("x") - held("z")),
            ("flight-throttle", held("PageUp") - held("PageDown")),
            (
                "flight-look-x",
                held("LookArrowRight") - held("LookArrowLeft"),
            ),
            ("flight-look-y", held("LookArrowUp") - held("LookArrowDown")),
        ] {
            self.key_value(control, value);
        }
        // Resolve pickup against the setting that ordered presets will establish this tick.
        let target = self
            .commands
            .iter()
            .fold(throttle, |value, command| match command {
                PilotCommand::Throttle(v) => *v,
                PilotCommand::AdjustThrottle(v) => (value + v).clamp(0., 1.),
                _ => value,
            });
        let (mut frame, look) = self.resolver.frame(target);
        let mut commands = std::mem::take(&mut self.commands);
        commands.append(&mut frame.commands);
        frame.commands = commands;
        (frame, look)
    }
    pub fn feedback(&mut self, event: FeedbackEvent) {
        if !self.resolver.profile.rumble || self.context.0 || !self.context.1 {
            return;
        }
        let targets: BTreeSet<_> = self
            .devices
            .values()
            .filter(|d| {
                d.rumble
                    && d.controls
                        .iter()
                        .any(|c| self.resolver.flight_bound(&d.id, &c.id))
            })
            .map(|d| d.id.clone())
            .collect();
        if !targets.is_empty() && self.feedback_mixer.event(event) {
            self.feedback_targets.extend(targets);
        }
    }
    pub fn afterburner_feedback(&mut self, active: bool) {
        let active = active && self.resolver.profile.rumble && !self.context.0 && self.context.1;
        if active && !self.feedback_mixer.afterburner() {
            for d in self.devices.values() {
                if d.rumble
                    && d.controls
                        .iter()
                        .any(|c| self.resolver.flight_bound(&d.id, &c.id))
                {
                    self.feedback_targets.insert(d.id.clone());
                }
            }
        }
        self.feedback_mixer.set_afterburner(active);
    }
    /// Called once after each authoritative flight tick; never during render interpolation.
    pub fn feedback_tick(&mut self) {
        if !self.resolver.profile.rumble || self.context.0 || !self.context.1 {
            return;
        }
        // A pulse waiting behind catch-up ticks must not regain its original lifetime.
        if let Some(FeedbackUpdate::Pulse { duration, .. }) = &mut self.feedback_pending {
            *duration = duration.saturating_sub(Duration::from_nanos(8_333_334));
            if duration.is_zero() {
                self.feedback_pending = Some(FeedbackUpdate::Stop);
            }
        }
        if let Some(update) = self.feedback_mixer.tick() {
            self.feedback_pending = Some(update);
        }
    }
    /// Coalesce fixed-tick catch-up before calling the native worker.
    pub fn feedback_flush(&mut self) {
        match self.feedback_pending.take() {
            Some(FeedbackUpdate::Pulse {
                strong,
                weak,
                duration,
            }) => {
                for id in &self.feedback_targets {
                    if self.devices.contains_key(id) {
                        let _ = self.backend.rumble(id, strong, weak, duration);
                    }
                }
            }
            Some(FeedbackUpdate::Stop) => {
                self.backend.stop();
            }
            None => {}
        }
    }
}
fn normalize(device: &Device, mut event: Event) -> Event {
    if let Some(control) = device.controls.iter().find(|c| c.id == event.control)
        && matches!(control.kind, Kind::Axis)
        && control.max > control.min
    {
        event.value =
            ((event.value - control.min) / (control.max - control.min) * 2. - 1.).clamp(-1., 1.);
    }
    event
}
pub fn diagnostics(
    seconds: u64,
    write_profile: Option<&Path>,
    rumble: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..=300).contains(&seconds) {
        return Err("input monitor duration must be 1..300 seconds".into());
    }
    let backend = Backend::start();
    let start = Instant::now();
    let mut devices = BTreeMap::new();
    println!("Input diagnostics: {seconds}s; native raw values, no retail media or GPU required");
    while start.elapsed() < Duration::from_secs(seconds) {
        for event in backend.drain() {
            match event {
                Notification::Connected(d) => {
                    println!("device {} name={:?} rumble={}", d.id, d.name, d.rumble);
                    for c in &d.controls {
                        println!(
                            "  {} {:?} min={} max={} value={}",
                            c.id, c.kind, c.min, c.max, c.value
                        );
                    }
                    devices.insert(d.id.clone(), d);
                }
                Notification::Feedback(id, result) => {
                    result.map_err(|e| format!("feedback {id}: {e}"))?;
                    println!(
                        "Rumble request accepted by native API: {id}; physical response requires user verification"
                    );
                }
                Notification::Input(e) => println!(
                    "{:.3} {} {} {}",
                    start.elapsed().as_secs_f64(),
                    e.device,
                    e.control,
                    e.value
                ),
                Notification::Disconnected(id) => {
                    devices.remove(&id);
                    println!("disconnected {id}");
                }
                Notification::Warning(w) => eprintln!("Input: {w}"),
                Notification::Overflow => {
                    devices.clear();
                    eprintln!("Input overflow; rebuilding baselines");
                }
            }
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    if devices.is_empty() {
        println!("No readable controller input devices found.");
    }
    if let Some(selector) = rumble {
        let id = rumble_target(&devices, selector)?;
        backend.rumble(id, 0.2, 0.2, Duration::from_millis(200))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut accepted = None;
        while Instant::now() < deadline {
            for event in backend.drain() {
                match event {
                    Notification::Feedback(device, result) if device == id => {
                        result.map_err(|e| format!("feedback {id}: {e}"))?;
                        accepted = Some(Instant::now());
                        println!(
                            "Rumble request accepted by native API: {id}; physical response requires user verification"
                        );
                    }
                    Notification::Disconnected(device) if device == id => {
                        return Err("rumble device disconnected during test".into());
                    }
                    Notification::Overflow => {
                        return Err("input overflow interrupted rumble test".into());
                    }
                    Notification::Warning(w) => eprintln!("Input: {w}"),
                    _ => {}
                }
            }
            if accepted.is_some_and(|at| at.elapsed() >= Duration::from_millis(220)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(8));
        }
        if accepted.is_none() {
            return Err("native rumble request timed out".into());
        }
    }
    backend.stop();
    if let Some(path) = write_profile {
        // Explicit create-new prevents accidentally overwriting user calibrations.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        writeln!(
            file,
            "tore-input 1\nrumble off\n# See docs/INPUT.md. Axis inputs are normalized to -1..1 before calibration."
        )?;
        for (i, d) in devices.values().enumerate() {
            let binding_device = if d.id.starts_with("macos-gc-session-") {
                writeln!(
                    file,
                    "\n# {}\n# Apple gamepad IDs are session-only. These commented * bindings share\n# named controls across all Apple gamepads; enable only the desired actions.",
                    d.name.replace(['\n', '\r'], " ")
                )?;
                "*".to_owned()
            } else {
                writeln!(
                    file,
                    "\n# {}\nalias device{} {}",
                    d.name.replace(['\n', '\r'], " "),
                    i + 1,
                    d.id
                )?;
                format!("device{}", i + 1)
            };
            for line in gamepad_text(d).lines().skip(1) {
                writeln!(
                    file,
                    "{}",
                    line.replacen(
                        &format!("bind {} ", d.id),
                        &format!("bind device{} ", i + 1),
                        1
                    )
                )?;
            }
            for c in &d.controls {
                let (action, mode) = match c.kind {
                    Kind::Axis => ("roll", "axis"),
                    Kind::Relative => ("throttle-rate", "delta"),
                    Kind::Position => ("instrument-next", "position=1"),
                    Kind::Button => ("gear", "press"),
                };
                writeln!(file, "# bind {binding_device} {} {} {}", c.id, action, mode)?;
            }
        }
        file.sync_all()?;
        println!(
            "Wrote {} (standard Linux gamepad defaults active; other suggestions commented)",
            path.display()
        );
    }
    Ok(())
}

/// `only` never chooses the first controller from an ambiguous device list.
fn rumble_target<'a>(
    devices: &'a BTreeMap<String, Device>,
    selector: &str,
) -> Result<&'a str, String> {
    if selector == "only" {
        let mut capable = devices.values().filter(|d| d.rumble);
        let device = capable.next().ok_or("no rumble-capable controller found")?;
        if capable.next().is_some() {
            return Err("multiple rumble-capable controllers found; supply an exact device ID or connect just the controller to test".into());
        }
        Ok(&device.id)
    } else {
        let device = devices
            .get(selector)
            .ok_or("requested rumble device was not found")?;
        if !device.rumble {
            return Err("requested controller exposes no rumble capability".into());
        }
        Ok(&device.id)
    }
}

fn gamepad_defaults(device: &Device) -> Vec<tore_input::Binding> {
    Profile::parse(&gamepad_text(device))
        .expect("static gamepad profile")
        .bindings
}
fn gamepad_text(device: &Device) -> String {
    if !device.id.starts_with("linux-")
        || ![
            "axis:0",
            "axis:1",
            "axis:2",
            "axis:3",
            "axis:4",
            "axis:5",
            "button:304",
            "button:305",
        ]
        .iter()
        .all(|id| device.controls.iter().any(|c| &c.id == id))
    {
        return "tore-input 1\n".into();
    }
    let defaults = [
        ("axis:0", "roll", "axis", 1.),
        ("axis:1", "pitch", "axis", 1.),
        ("axis:3", "look-x", "axis", 1.),
        ("axis:4", "look-y", "axis", -1.),
        ("axis:2", "yaw", "trigger-negative", 1.),
        ("axis:5", "yaw", "trigger-positive", 1.),
        ("button:310", "throttle-rate", "negative", 1.),
        ("button:311", "throttle-rate", "positive", 1.),
        ("button:304", "gear", "press", 1.),
        ("button:305", "airbrake", "press", 1.),
        ("button:307", "flaps", "press", 1.),
        ("button:308", "burner", "press", 1.),
        ("button:315", "pause", "press", 1.),
        ("button:316", "menu", "press", 1.),
        ("button:317", "view-front", "press", 1.),
        ("button:318", "center-look", "press", 1.),
        ("axis:16", "instrument-previous", "position=-1", 1.),
        ("axis:16", "instrument-next", "position=1", 1.),
        ("axis:17", "control-1", "position=-1", 1.),
        ("axis:17", "control-2", "position=1", 1.),
        ("axis:16", "menu-left", "position=-1", 1.),
        ("axis:16", "menu-right", "position=1", 1.),
        ("axis:17", "menu-up", "position=-1", 1.),
        ("axis:17", "menu-down", "position=1", 1.),
        ("button:304", "menu-accept", "press", 1.),
        ("button:305", "menu-back", "press", 1.),
    ];
    let mut text = String::from("tore-input 1\n");
    for (control, action, mode, scale) in defaults {
        if device.controls.iter().any(|c| c.id == control) {
            text.push_str(&format!(
                "bind {} {control} {action} {mode} -1 0 1 0.1 1 {scale} 10\n",
                device.id
            ));
        }
    }
    for (control, action, mode) in [
        ("button:311", "fire", "hold"),
        ("button:310", "weapon-next", "press"),
        ("button:304", "designate", "press"),
        ("button:305", "clear-designation", "press"),
        ("button:307", "master-arm", "press"),
        ("button:308", "jammer", "press"),
        ("button:317", "radar", "press"),
        ("button:318", "jettison", "press"),
        ("axis:16", "damage-class", "position=-1"),
        ("axis:16", "fail-station", "position=1"),
        ("axis:17", "range-target", "position=-1"),
        ("axis:17", "damage-player", "position=1"),
        ("button:315", "target-jammer", "press"),
        ("button:316", "incoming", "press"),
    ] {
        if device.controls.iter().any(|c| c.id == "button:314")
            && device.controls.iter().any(|c| c.id == control)
        {
            text.push_str(&format!(
                "bind {} button:314+{control} {action} {mode}\n",
                device.id
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::ModifiersState as M;
    fn input(profile: &str) -> Input {
        let mut input = Input::new(None, false).unwrap();
        let profile = Profile::parse(&format!("tore-input 1\n{profile}")).unwrap();
        input.resolver.profile.aliases.extend(profile.aliases);
        input.resolver.profile.bindings.extend(profile.bindings);
        input
    }
    #[test]
    fn settings_save_reload_and_invalid_edit_preserve_previous_file() {
        let dir = std::env::temp_dir().join(format!(
            "tore-controls-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("input-v1.conf");
        let mut input = Input::new(None, false).unwrap();
        input.profile_path = Some(path.clone());
        let p=Profile::parse("tore-input 1\nrumble on\ngamepad-defaults on\nalias stick synthetic\nbind stick b gear switch\nbind keyboard Ctrl-g gear press").unwrap();
        input.context(true, true);
        input.save_settings(&p).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(input.automatic);
        let reloaded = Input::new(Some(&path), false).unwrap();
        assert_eq!(reloaded.settings_profile().to_text().unwrap(), saved);
        let mut bad = p.clone();
        bad.bindings[0].calibration.max = bad.bindings[0].calibration.min;
        assert!(input.save_settings(&bad).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        assert!(input.resolver.profile.rumble);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn feedback_uses_assigned_idle_devices_and_respects_context() {
        let mut i =
            input("alias pad pad-id\nbind pad x roll axis\nbind ui b instrument-next press");
        for (id, control, kind) in [
            ("pad-id", "x", Kind::Axis),
            ("ui", "b", Kind::Button),
            ("unassigned", "x", Kind::Axis),
        ] {
            i.devices.insert(
                id.into(),
                Device {
                    id: id.into(),
                    name: id.into(),
                    rumble: true,
                    controls: vec![tore_input_native::Control {
                        id: control.into(),
                        kind,
                        min: -1.,
                        max: 1.,
                        value: 0.,
                    }],
                },
            );
        }
        i.feedback(FeedbackEvent::AfterburnerEngaged);
        assert!(i.feedback_targets.is_empty()); // Opt-in required.
        i.resolver.profile.rumble = true;
        i.feedback(FeedbackEvent::AfterburnerEngaged);
        assert_eq!(i.feedback_targets, BTreeSet::from(["pad-id".into()]));
        assert!(matches!(
            i.feedback_mixer.tick(),
            Some(FeedbackUpdate::Pulse { .. })
        ));
        i.feedback(FeedbackEvent::BombReleased);
        for _ in 0..40 {
            i.feedback_tick();
        }
        assert_eq!(i.feedback_pending, Some(FeedbackUpdate::Stop));
        i.context(true, true);
        assert!(i.feedback_targets.is_empty());
        i.feedback(FeedbackEvent::GunFired);
        i.context(false, true);
        assert_eq!(i.feedback_mixer.tick(), None);
        i.context(false, false);
        i.feedback(FeedbackEvent::Crash);
        assert!(i.feedback_targets.is_empty());
    }
    #[test]
    fn rumble_selection_requires_unique_capable_device() {
        let mut devices = BTreeMap::new();
        assert!(rumble_target(&devices, "only").is_err());
        for (id, rumble) in [("box", false), ("pad-a", true)] {
            devices.insert(
                id.into(),
                Device {
                    id: id.into(),
                    name: id.into(),
                    controls: vec![],
                    rumble,
                },
            );
        }
        assert_eq!(rumble_target(&devices, "only").unwrap(), "pad-a");
        assert!(rumble_target(&devices, "box").is_err());
        assert!(rumble_target(&devices, "missing").is_err());
        devices.insert(
            "pad-b".into(),
            Device {
                id: "pad-b".into(),
                name: "same model".into(),
                controls: vec![],
                rumble: true,
            },
        );
        assert!(rumble_target(&devices, "only").is_err());
        assert_eq!(rumble_target(&devices, "pad-b").unwrap(), "pad-b");
    }
    #[test]
    fn custom_modifier_binding_keeps_release_owner() {
        let mut i = input("bind keyboard Ctrl-g gear press\nbind keyboard g flaps press");
        assert!(i.key("g", true, M::CONTROL));
        assert_eq!(i.resolver.drain().len(), 1);
        assert!(i.key("g", true, M::empty()));
        assert!(i.resolver.drain().is_empty());
        assert!(i.key("g", false, M::empty()));
        assert!(i.key("g", true, M::empty()));
        assert_eq!(
            i.resolver.drain()[0].1,
            Action::Pilot(PilotCommand::Toggle(tore_input::Switch::Flaps))
        );
    }
    #[test]
    fn pending_commands_are_consumed_once_and_dropped_on_pause() {
        let mut i = input("");
        let keys = BTreeSet::new();
        i.queue(PilotCommand::Throttle(0.3));
        assert_eq!(i.frame(&keys, 0.7).0.commands.len(), 1);
        assert!(i.frame(&keys, 0.3).0.commands.is_empty());
        i.queue(PilotCommand::Throttle(0.1));
        i.context(true, true);
        i.context(false, true);
        assert!(i.frame(&keys, 0.3).0.commands.is_empty());
    }
    #[test]
    fn throttle_preset_releases_axis_that_matches_the_previous_setting() {
        let mut i = input("bind hotas t throttle unit");
        i.resolver.event(Event {
            device: "hotas".into(),
            control: "t".into(),
            value: 0.4,
            baseline: true,
        });
        assert_eq!(i.frame(&BTreeSet::new(), 0.7).0.throttle, Some(0.7));
        i.queue(PilotCommand::Throttle(0.1));
        assert_eq!(i.frame(&BTreeSet::new(), 0.7).0.throttle, None);
        assert_eq!(i.frame(&BTreeSet::new(), 0.1).0.throttle, None);
    }
    #[test]
    fn keyboard_pitch_and_look_keep_original_signs() {
        let mut i = input("");
        let keys = BTreeSet::from(["ArrowDown".into(), "LookArrowLeft".into()]);
        let (p, look) = i.frame(&keys, 0.7);
        assert_eq!(p.pitch, 1.);
        assert_eq!(look, [-1., 0.]);
        let (p, look) = i.frame(&BTreeSet::new(), 0.7);
        assert_eq!(p.pitch, 0.);
        assert_eq!(look, [0., 0.]);
    }

    #[test]
    fn every_default_combat_combo_emits_only_its_intended_action() {
        let ids = [
            "axis:0",
            "axis:1",
            "axis:2",
            "axis:3",
            "axis:4",
            "axis:5",
            "axis:16",
            "axis:17",
            "button:304",
            "button:305",
            "button:307",
            "button:308",
            "button:310",
            "button:311",
            "button:314",
            "button:315",
            "button:316",
            "button:317",
            "button:318",
        ];
        let d = Device {
            id: "linux-synthetic".into(),
            name: "synthetic".into(),
            rumble: false,
            controls: ids
                .iter()
                .map(|id| tore_input_native::Control {
                    id: (*id).into(),
                    kind: if id.starts_with("button") {
                        Kind::Button
                    } else {
                        Kind::Axis
                    },
                    min: -1.,
                    max: 1.,
                    value: 0.,
                })
                .collect(),
        };
        let profile = Profile::parse(&gamepad_text(&d)).unwrap();
        let chords: Vec<_> = profile
            .bindings
            .iter()
            .filter(|b| b.control.contains('+'))
            .cloned()
            .collect();
        assert_eq!(chords.len(), 14);
        for binding in chords {
            let mut r = Resolver::new(profile.clone());
            for id in ids {
                r.event(Event {
                    device: d.id.clone(),
                    control: id.into(),
                    value: 0.,
                    baseline: true,
                });
            }
            r.event(Event {
                device: d.id.clone(),
                control: "button:314".into(),
                value: 1.,
                baseline: false,
            });
            let (_, control) = binding.control.split_once('+').unwrap();
            r.event(Event {
                device: d.id.clone(),
                control: control.into(),
                value: if let tore_input::Mode::Position(n) = binding.mode {
                    f64::from(n)
                } else {
                    1.
                },
                baseline: false,
            });
            let actions = r.drain();
            if binding.action == Action::Ui("fire".into()) {
                assert!(r.held("fire"));
                assert!(actions.is_empty());
            } else {
                assert_eq!(
                    actions,
                    vec![(d.id.clone(), binding.action.clone())],
                    "{}",
                    binding.control
                );
            }
            assert_eq!(r.frame(0.5).0.throttle_rate, 0.);
        }
    }
}
