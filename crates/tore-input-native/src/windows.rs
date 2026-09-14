//! Windows.Gaming.Input raw controllers; no redistributable or gamepad-only limit.
use super::*;
use ::windows::{
    Gaming::Input::{GameControllerSwitchPosition, Gamepad, GamepadVibration, RawGameController},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
};
use std::{collections::BTreeSet, io};
struct OpenDevice {
    raw: RawGameController,
    gamepad: Option<Gamepad>,
    device: Device,
}
impl Drop for OpenDevice {
    fn drop(&mut self) {
        // Removal/read failure must also stop an effect, before releasing the endpoint.
        if let Some(gamepad) = &self.gamepad {
            let _ = gamepad.SetVibration(GamepadVibration::default());
        }
    }
}
pub struct Platform {
    devices: BTreeMap<String, OpenDevice>,
    scan: Instant,
}
fn error(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
fn reading(raw: &RawGameController) -> io::Result<Vec<Control>> {
    let axes = raw.AxisCount().map_err(error)?;
    let buttons = raw.ButtonCount().map_err(error)?;
    let switches = raw.SwitchCount().map_err(error)?;
    if !(0..=128).contains(&axes) || !(0..=1024).contains(&buttons) || !(0..=64).contains(&switches)
    {
        return Err(error("controller exceeds capability bounds"));
    }
    let mut a = vec![0.; axes as usize];
    let mut b = vec![false; buttons as usize];
    let mut s = vec![GameControllerSwitchPosition::Center; switches as usize];
    raw.GetCurrentReading(&mut b, &mut s, &mut a)
        .map_err(error)?;
    let mut result = Vec::new();
    for (i, value) in a.into_iter().enumerate() {
        result.push(Control {
            id: format!("axis:{i}"),
            kind: Kind::Axis,
            min: 0.,
            max: 1.,
            value,
        });
    }
    for (i, value) in b.into_iter().enumerate() {
        result.push(Control {
            id: format!("button:{i}"),
            kind: Kind::Button,
            min: 0.,
            max: 1.,
            value: f64::from(value),
        });
    }
    for (i, value) in s.into_iter().enumerate() {
        result.push(Control {
            id: format!("switch:{i}"),
            kind: Kind::Position,
            min: 0.,
            max: 8.,
            value: value.0 as f64,
        });
    }
    Ok(result)
}
impl Platform {
    pub fn new() -> io::Result<Self> {
        // SAFETY: initialize a fresh, dedicated worker thread; balanced in Drop on the same thread.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(error)?;
        Ok(Self {
            devices: BTreeMap::new(),
            scan: Instant::now() - Duration::from_secs(2),
        })
    }
    pub fn poll(&mut self) -> Vec<Notification> {
        let mut out = vec![];
        if self.scan.elapsed() >= Duration::from_secs(1) {
            self.scan = Instant::now();
            match RawGameController::RawGameControllers() {
                Ok(list) => {
                    let mut present = BTreeSet::new();
                    for raw in list.into_iter().take(64) {
                        let Ok(identity) = raw.NonRoamableId() else {
                            continue;
                        };
                        let id = format!("windows-{}", encode(&identity.to_string()));
                        present.insert(id.clone());
                        if self.devices.contains_key(&id) {
                            continue;
                        }
                        match reading(&raw) {
                            Ok(controls) => {
                                let gamepad = Gamepad::FromGameController(&raw).ok();
                                let device = Device {
                                    id: id.clone(),
                                    name: raw
                                        .DisplayName()
                                        .map(|n| n.to_string())
                                        .unwrap_or_else(|_| "Controller".into()),
                                    controls,
                                    rumble: gamepad.is_some(),
                                };
                                out.push(Notification::Connected(device.clone()));
                                self.devices.insert(
                                    id,
                                    OpenDevice {
                                        raw,
                                        gamepad,
                                        device,
                                    },
                                );
                            }
                            Err(e) => out.push(Notification::Warning(e.to_string())),
                        }
                    }
                    self.devices.retain(|id, _| {
                        if present.contains(id) {
                            true
                        } else {
                            out.push(Notification::Disconnected(id.clone()));
                            false
                        }
                    });
                }
                Err(e) => out.push(Notification::Warning(e.to_string())),
            }
        }
        let mut lost = vec![];
        for (id, d) in &mut self.devices {
            match reading(&d.raw) {
                Ok(controls) => {
                    for (new, old) in controls.iter().zip(&d.device.controls) {
                        if new.value != old.value {
                            out.push(Notification::Input(Event {
                                device: id.clone(),
                                control: new.id.clone(),
                                value: new.value,
                                baseline: false,
                            }));
                        }
                    }
                    d.device.controls = controls;
                }
                Err(_) => lost.push(id.clone()),
            }
        }
        for id in lost {
            self.devices.remove(&id);
            out.push(Notification::Disconnected(id));
        }
        out
    }
    pub fn rumble(
        &mut self,
        id: &str,
        strong: f64,
        weak: f64,
        _duration: Duration,
    ) -> io::Result<()> {
        let gamepad = self
            .devices
            .get(id)
            .and_then(|d| d.gamepad.as_ref())
            .ok_or_else(|| error("device has no exposed gamepad rumble"))?;
        gamepad
            .SetVibration(GamepadVibration {
                LeftMotor: strong,
                RightMotor: weak,
                LeftTrigger: 0.,
                RightTrigger: 0.,
            })
            .map_err(error)
    }
    pub fn stop_device(&mut self, id: &str) {
        if let Some(g) = self.devices.get(id).and_then(|d| d.gamepad.as_ref()) {
            let _ = g.SetVibration(GamepadVibration::default());
        }
    }
    pub fn stop(&mut self) {
        for d in self.devices.values() {
            if let Some(g) = &d.gamepad {
                let _ = g.SetVibration(GamepadVibration::default());
            }
        }
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        self.stop();
        self.devices.clear();
        // SAFETY: balances this thread's successful RoInitialize after releasing COM objects.
        unsafe {
            RoUninitialize();
        }
    }
}
