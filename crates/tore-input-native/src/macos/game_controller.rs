//! macOS 11+ gamepads: input and haptics share the exact retained GCController.
//! No matching by display name, enumeration order, or private HID identifiers.
use super::*;
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::{
    AnyThread, ClassType, Encoding, RefEncode, msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{Bool, ProtocolObject},
    sel,
};
use objc2_core_haptics::{
    CHHapticEngine, CHHapticEvent, CHHapticEventParameter, CHHapticEventParameterIDHapticIntensity,
    CHHapticEventParameterIDHapticSharpness, CHHapticEventTypeHapticContinuous, CHHapticPattern,
    CHHapticPatternPlayer,
};
use objc2_foundation::{NSArray, NSString};
use objc2_game_controller::{
    GCController, GCDevice, GCHapticsLocalityDefault, GCHapticsLocalityLeftHandle,
    GCHapticsLocalityRightHandle,
};

// IOHIDDeviceRef's Objective-C encoding, for the one API missing from the bindings.
#[repr(C)]
struct HidDevice {
    _opaque: [u8; 0],
}
// SAFETY: this opaque type is only used behind a borrowed IOHIDDeviceRef pointer.
unsafe impl RefEncode for HidDevice {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__IOHIDDevice", &[]));
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

struct Channel {
    engine: Retained<CHHapticEngine>,
    player: Option<Retained<ProtocolObject<dyn CHHapticPatternPlayer>>>,
}
impl Channel {
    fn play(&mut self, intensity: f64, sharpness: f32, duration: Duration) -> io::Result<()> {
        self.stop();
        if intensity == 0. {
            return Ok(());
        }
        // SAFETY: controller-created engine, retained objects, finite bounded parameters;
        // all effect operations remain on the input worker. No audio resource is used.
        unsafe {
            self.engine.startAndReturnError().map_err(error)?;
            let intensity = CHHapticEventParameter::initWithParameterID_value(
                CHHapticEventParameter::alloc(),
                CHHapticEventParameterIDHapticIntensity,
                intensity as f32,
            );
            let sharpness = CHHapticEventParameter::initWithParameterID_value(
                CHHapticEventParameter::alloc(),
                CHHapticEventParameterIDHapticSharpness,
                sharpness,
            );
            let event = CHHapticEvent::initWithEventType_parameters_relativeTime_duration(
                CHHapticEvent::alloc(),
                CHHapticEventTypeHapticContinuous,
                &NSArray::from_retained_slice(&[intensity, sharpness]),
                0.,
                duration.as_secs_f64(),
            );
            let pattern = CHHapticPattern::initWithEvents_parameters_error(
                CHHapticPattern::alloc(),
                &NSArray::from_retained_slice(&[event]),
                &NSArray::new(),
            )
            .map_err(error)?;
            let player = self
                .engine
                .createPlayerWithPattern_error(&pattern)
                .map_err(error)?;
            player.startAtTime_error(0.).map_err(error)?;
            self.player = Some(player);
        }
        Ok(())
    }
    fn stop(&mut self) {
        // SAFETY: the player belongs to our engine. Stopping never schedules a replay.
        unsafe {
            if let Some(player) = self.player.take() {
                let _ = player.stopAtTime_error(0.);
            }
        }
    }
}
impl Drop for Channel {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: documented nullable completion block; async stop without Rust captures.
        unsafe {
            self.engine.stopWithCompletionHandler(ptr::null_mut());
        }
    }
}
struct Haptics {
    channels: Vec<Channel>,
}
impl Haptics {
    fn new(controller: &GCController) -> io::Result<Self> {
        // SAFETY: macOS 11+ controller; this factory returns retained engines (ordinary
        // ObjC +0 method family). The generated 0.3.2 binding omits this macOS method.
        unsafe {
            let h = controller
                .haptics()
                .ok_or_else(|| error("controller exposes no haptics"))?;
            let localities = h.supportedLocalities();
            let both = localities.containsObject(GCHapticsLocalityLeftHandle)
                && localities.containsObject(GCHapticsLocalityRightHandle);
            let selected: &[&NSString] = if both {
                &[GCHapticsLocalityLeftHandle, GCHapticsLocalityRightHandle]
            } else {
                &[GCHapticsLocalityDefault]
            };
            let mut channels = vec![];
            for locality in selected {
                let engine: Option<Retained<CHHapticEngine>> =
                    msg_send![&*h, createEngineWithLocality: *locality];
                let engine = engine.ok_or_else(|| error("controller haptic engine unavailable"))?;
                engine.setPlaysHapticsOnly(true);
                engine.setAutoShutdownEnabled(true);
                channels.push(Channel {
                    engine,
                    player: None,
                });
            }
            Ok(Self { channels })
        }
    }
    fn play(&mut self, strong: f64, weak: f64, duration: Duration) -> io::Result<()> {
        let result = if self.channels.len() == 2 {
            self.channels[0]
                .play(strong, 0., duration)
                .and_then(|()| self.channels[1].play(weak, 1., duration))
        } else {
            // Default locality has no independent two-motor contract; bounded mono fallback.
            self.channels[0].play(strong.max(weak), 0.5, duration)
        };
        if result.is_err() {
            self.stop();
        }
        result
    }
    fn stop(&mut self) {
        for c in &mut self.channels {
            c.stop();
        }
    }
}
struct Gamepad {
    controller: Retained<GCController>,
    device: Device,
    haptics: Option<Haptics>,
}
pub(super) struct GameControllers {
    devices: BTreeMap<usize, Gamepad>,
    queue: DispatchRetained<DispatchQueue>,
    scan: Instant,
    session: u128,
    next_id: u64,
}
impl GameControllers {
    pub fn new() -> Self {
        Self {
            devices: BTreeMap::new(),
            queue: DispatchQueue::new("org.tore.controllers", None),
            scan: Instant::now() - Duration::from_secs(2),
            session: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            next_id: 0,
        }
    }
    pub fn handles(device: Ref) -> bool {
        if !objc2::available!(macos = 11.0) {
            return false;
        }
        // SAFETY: live IOHIDDeviceRef supplied by the HID manager. Capability query is
        // Apple's supported duplicate-suppression API, not a guessed device match.
        unsafe {
            if GCController::class()
                .class_method(sel!(supportsHIDDevice:))
                .is_none()
            {
                return false;
            }
            let supported: Bool =
                msg_send![GCController::class(), supportsHIDDevice: device.cast::<HidDevice>()];
            supported.as_bool()
        }
    }
    fn reading(controller: &GCController) -> io::Result<Vec<Control>> {
        // SAFETY: snapshot isolates one consistent native profile from OS updates;
        // dictionaries have documented NSString keys and typed input values.
        unsafe {
            let profile = controller.physicalInputProfile().capture();
            let axes = profile.axes();
            let buttons = profile.buttons();
            if axes.len() > 128 || buttons.len() > 1024 {
                return Err(error("GCController capability limit exceeded"));
            }
            let mut controls = vec![];
            for key in axes.keys() {
                let name = key.to_string();
                if name.len() > 256 {
                    return Err(error("GCController element name too long"));
                }
                if let Some(axis) = axes.objectForKey(&key) {
                    controls.push(Control {
                        id: format!("gc-axis:{}", encode(&name)),
                        kind: Kind::Axis,
                        min: -1.,
                        max: 1.,
                        value: axis.value() as f64,
                    });
                }
            }
            for key in buttons.keys() {
                let name = key.to_string();
                if name.len() > 256 {
                    return Err(error("GCController element name too long"));
                }
                if let Some(button) = buttons.objectForKey(&key) {
                    // Separate native pressed state from pressure: bindings get genuine
                    // digital edges, while trigger modes receive a normalized 0..1 axis.
                    controls.push(Control {
                        id: format!("gc-button:{}", encode(&name)),
                        kind: Kind::Button,
                        min: 0.,
                        max: 1.,
                        value: f64::from(button.isPressed()),
                    });
                    controls.push(Control {
                        id: format!("gc-pressure:{}", encode(&name)),
                        kind: Kind::Axis,
                        min: 0.,
                        max: 1.,
                        value: button.value() as f64,
                    });
                }
            }
            controls.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(controls)
        }
    }
    pub fn poll(&mut self) -> Vec<Notification> {
        autoreleasepool(|_| self.poll_inner())
    }
    fn poll_inner(&mut self) -> Vec<Notification> {
        let mut out = vec![];
        if !objc2::available!(macos = 11.0) {
            return out;
        }
        // SAFETY: native API retains endpoints. OS input handler dispatch is assigned
        // to a serial queue, so CLI diagnostics do not depend on an AppKit main loop.
        unsafe {
            if self.scan.elapsed() >= Duration::from_secs(1) {
                self.scan = Instant::now();
                let list = GCController::controllers();
                let mut present = std::collections::BTreeSet::new();
                for controller in list.iter().take(64) {
                    let key = Retained::as_ptr(&controller) as usize;
                    present.insert(key);
                    if self.devices.contains_key(&key) {
                        continue;
                    }
                    controller.setHandlerQueue(&self.queue);
                    match Self::reading(&controller) {
                        Ok(controls) => {
                            self.next_id += 1;
                            let device = Device {
                                id: format!("macos-gc-session-{:x}-{}", self.session, self.next_id),
                                name: controller
                                    .vendorName()
                                    .map(|n| n.to_string())
                                    .unwrap_or_else(|| "Apple game controller".into()),
                                controls,
                                rumble: controller.haptics().is_some(),
                            };
                            out.push(Notification::Connected(device.clone()));
                            self.devices.insert(
                                key,
                                Gamepad {
                                    controller,
                                    device,
                                    haptics: None,
                                },
                            );
                        }
                        Err(e) => out.push(Notification::Warning(e.to_string())),
                    }
                }
                self.devices.retain(|key, d| {
                    if present.contains(key) {
                        true
                    } else {
                        out.push(Notification::Disconnected(d.device.id.clone()));
                        false
                    }
                });
            }
        }
        self.devices
            .retain(|_, d| match Self::reading(&d.controller) {
                Ok(controls) => {
                    if controls.len() != d.device.controls.len()
                        || controls
                            .iter()
                            .zip(&d.device.controls)
                            .any(|(a, b)| a.id != b.id)
                    {
                        out.push(Notification::Disconnected(d.device.id.clone()));
                        return false; // Reopen with fresh capabilities/baselines next scan.
                    }
                    for c in &controls {
                        if d.device
                            .controls
                            .iter()
                            .find(|old| old.id == c.id)
                            .is_none_or(|old| old.value != c.value)
                        {
                            out.push(Notification::Input(Event {
                                device: d.device.id.clone(),
                                control: c.id.clone(),
                                value: c.value,
                                baseline: false,
                            }));
                        }
                    }
                    d.device.controls = controls;
                    true
                }
                Err(e) => {
                    out.push(Notification::Warning(e.to_string()));
                    out.push(Notification::Disconnected(d.device.id.clone()));
                    false
                }
            });
        out
    }
    pub fn rumble(
        &mut self,
        id: &str,
        strong: f64,
        weak: f64,
        duration: Duration,
    ) -> io::Result<()> {
        autoreleasepool(|_| {
            let d = self
                .devices
                .values_mut()
                .find(|d| d.device.id == id)
                .ok_or_else(|| error("device has no Apple GameController haptic endpoint"))?;
            if d.haptics.is_none() {
                d.haptics = Some(Haptics::new(&d.controller)?);
            }
            let result = d
                .haptics
                .as_mut()
                .expect("initialized haptics")
                .play(strong, weak, duration);
            // Recover on the next explicit request after native engine/reset errors.
            // Never retry/replay this failed pulse or an interrupted one automatically.
            if result.is_err() {
                d.haptics = None;
            }
            result
        })
    }
    pub fn stop_device(&mut self, id: &str) {
        autoreleasepool(|_| {
            if let Some(d) = self.devices.values_mut().find(|d| d.device.id == id) {
                d.haptics = None;
            }
        });
    }
    pub fn stop(&mut self) {
        autoreleasepool(|_| {
            for d in self.devices.values_mut() {
                d.haptics = None;
            }
        });
    }
}
