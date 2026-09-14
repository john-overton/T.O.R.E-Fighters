//! evdev input and FF_RUMBLE. No exclusive grabs or system permission changes.
use super::*;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::PathBuf,
};
fn request(dir: u32, nr: u32, size: usize) -> libc::c_ulong {
    ((dir << 30) | ((size as u32) << 16) | (u32::from(b'E') << 8) | nr) as _
}
fn ioctl<T>(file: &File, nr: u32, value: &mut T) -> io::Result<()> {
    // SAFETY: the request encodes exactly T's allocation size; caller selects a reviewed evdev request.
    let r = unsafe {
        libc::ioctl(
            file.as_raw_fd(),
            request(2, nr, std::mem::size_of::<T>()),
            value as *mut T,
        )
    };
    if r < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn bits(file: &File, kind: u32) -> io::Result<[u8; 128]> {
    let mut b = [0; 128];
    ioctl(file, 0x20 + kind, &mut b)?;
    Ok(b)
}
fn has(bits: &[u8], n: usize) -> bool {
    bits.get(n / 8).is_some_and(|b| b & (1 << (n % 8)) != 0)
}
fn text(file: &File, nr: u32) -> String {
    let mut b = [0u8; 256];
    if ioctl(file, nr, &mut b).is_err() {
        return String::new();
    }
    String::from_utf8_lossy(&b[..b.iter().position(|b| *b == 0).unwrap_or(b.len())]).into()
}
fn abs(file: &File, code: u16) -> io::Result<libc::input_absinfo> {
    // SAFETY: this integer-only C struct has a valid all-zero representation.
    let mut value = unsafe { std::mem::zeroed() };
    ioctl(file, 0x40 + code as u32, &mut value)?;
    Ok(value)
}
struct OpenDevice {
    file: File,
    device: Device,
    pending: Vec<Event>,
    dropped: bool,
    effect: i16,
}
pub struct Platform {
    devices: BTreeMap<PathBuf, OpenDevice>,
    scan: Instant,
    warned: BTreeSet<String>,
}
use std::collections::BTreeSet;
impl Platform {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            devices: BTreeMap::new(),
            scan: Instant::now() - Duration::from_secs(2),
            warned: BTreeSet::new(),
        })
    }
    fn open(path: &PathBuf) -> io::Result<OpenDevice> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .or_else(|_| {
                OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                    .open(path)
            })?;
        let keys = bits(&file, 1)?;
        let axes = bits(&file, 3)?;
        let relative = bits(&file, 2)?;
        // Consumer keyboards often advertise a bogus joystick collection. Keep their
        // keyboard/media interfaces on winit; never enumerate their key stream here.
        if !controller(&keys, &axes) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "not a controller",
            ));
        }
        let mut identity = [0u16; 4];
        ioctl(&file, 2, &mut identity)?;
        let name = text(&file, 6);
        let mut serial = text(&file, 8);
        if serial.is_empty() {
            let sys = PathBuf::from("/sys/class/input")
                .join(path.file_name().unwrap())
                .join("device");
            if let Ok(real) = fs::canonicalize(sys) {
                for parent in real.ancestors() {
                    if parent.join("idVendor").exists() && parent.join("idProduct").exists() {
                        if let Ok(value) = fs::read_to_string(parent.join("serial")) {
                            serial = value.trim().into();
                        }
                        break;
                    }
                }
            }
        }
        let phys = text(&file, 7);
        let mut unique = if serial.is_empty() {
            phys.clone()
        } else {
            format!("{serial}/{}", phys.rsplit('/').next().unwrap_or("input0"))
        };
        if unique.is_empty() {
            unique = format!("session:{}", path.display());
        }
        let id = format!(
            "linux-{:04x}-{:04x}-{}",
            identity[1],
            identity[2],
            encode(&unique)
        );
        let mut controls = vec![];
        let mut down = [0u8; 128];
        ioctl(&file, 0x18, &mut down)?;
        for code in 0..768 {
            if has(&keys, code) {
                controls.push(Control {
                    id: format!("button:{code}"),
                    kind: Kind::Button,
                    min: 0.,
                    max: 1.,
                    value: f64::from(has(&down, code)),
                });
            }
        }
        for code in 0..64 {
            if has(&axes, code) {
                let a = abs(&file, code as u16)?;
                if a.maximum > a.minimum {
                    controls.push(Control {
                        id: format!("axis:{code}"),
                        kind: Kind::Axis,
                        min: a.minimum as f64,
                        max: a.maximum as f64,
                        value: a.value as f64,
                    });
                }
            }
        }
        for code in 0..16 {
            if has(&relative, code) {
                controls.push(Control {
                    id: format!("relative:{code}"),
                    kind: Kind::Relative,
                    min: -32.,
                    max: 32.,
                    value: 0.,
                });
            }
        }
        let rumble = bits(&file, 0x15).is_ok_and(|b| has(&b, 0x50));
        Ok(OpenDevice {
            file,
            device: Device {
                id,
                name,
                controls,
                rumble,
            },
            pending: vec![],
            dropped: false,
            effect: -1,
        })
    }
    pub fn poll(&mut self) -> Vec<Notification> {
        let mut out = vec![];
        if self.scan.elapsed() >= Duration::from_secs(1) {
            self.scan = Instant::now();
            let paths: BTreeSet<_> = fs::read_dir("/dev/input")
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with("event"))
                .map(|e| e.path())
                .take(256)
                .collect();
            self.devices.retain(|path, d| {
                if paths.contains(path) {
                    true
                } else {
                    out.push(Notification::Disconnected(d.device.id.clone()));
                    false
                }
            });
            for path in paths {
                if self.devices.contains_key(&path) {
                    continue;
                }
                let sys = PathBuf::from("/sys/class/input")
                    .join(path.file_name().unwrap())
                    .join("device/capabilities");
                if let (Ok(keys), Ok(axes)) = (
                    fs::read_to_string(sys.join("key")),
                    fs::read_to_string(sys.join("abs")),
                ) && !controller(&capability_bits(&keys), &capability_bits(&axes))
                {
                    continue;
                }
                match Self::open(&path) {
                    Ok(d) => {
                        out.push(Notification::Connected(d.device.clone()));
                        self.devices.insert(path, d);
                    }
                    Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                        let message = format!(
                            "{}: permission denied; controller access requires the desktop session's device ACL",
                            path.display()
                        );
                        if self.warned.insert(message.clone()) {
                            out.push(Notification::Warning(message));
                        }
                    }
                    _ => {}
                }
            }
            // A HID receiver can exist without a usable evdev controller (mode/driver/link).
            if !self
                .devices
                .values()
                .any(|d| d.device.name.to_ascii_lowercase().contains("8bitdo"))
                && let Ok(entries) = fs::read_dir("/dev/input/by-id")
            {
                for e in entries.filter_map(Result::ok) {
                    let n = e.file_name().to_string_lossy().into_owned();
                    if n.to_ascii_lowercase().contains("8bitdo") {
                        let message = format!(
                            "{n} detected, but no readable evdev controller. Check controller power, receiver link and connection mode; no driver or system setting was changed."
                        );
                        if self.warned.insert(message.clone()) {
                            out.push(Notification::Warning(message));
                        }
                    }
                }
            }
        }
        let mut lost = vec![];
        for (path, d) in &mut self.devices {
            for _ in 0..256 {
                let mut bytes = [0u8; std::mem::size_of::<libc::input_event>()];
                match d.file.read(&mut bytes) {
                    Ok(0) => {
                        lost.push(path.clone());
                        break;
                    }
                    Ok(n) if n == bytes.len() => {}
                    Ok(_) => {
                        lost.push(path.clone());
                        break;
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        lost.push(path.clone());
                        break;
                    }
                }
                // SAFETY: exact input_event bytes copied to an aligned integer-only value.
                let event =
                    unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<libc::input_event>()) };
                if event.type_ == 0 {
                    if event.code == 3 {
                        d.dropped = true;
                        d.pending.clear();
                    }
                    if event.code == 0 {
                        if d.dropped {
                            // SYN_DROPPED invalidates all held contributions before resnapshot.
                            out.push(Notification::Disconnected(d.device.id.clone()));
                            let mut down = [0u8; 128];
                            if ioctl(&d.file, 0x18, &mut down).is_err() {
                                lost.push(path.clone());
                                break;
                            }
                            let mut complete = true;
                            for c in &mut d.device.controls {
                                let code =
                                    c.id.split_once(':')
                                        .and_then(|(_, n)| n.parse::<u16>().ok())
                                        .unwrap_or(0);
                                match c.kind {
                                    Kind::Button => c.value = f64::from(has(&down, code as usize)),
                                    Kind::Axis => match abs(&d.file, code) {
                                        Ok(a) => c.value = a.value as f64,
                                        Err(_) => {
                                            complete = false;
                                            break;
                                        }
                                    },
                                    _ => c.value = 0.,
                                }
                            }
                            if !complete {
                                lost.push(path.clone());
                                break;
                            }
                            out.push(Notification::Connected(d.device.clone()));
                            d.dropped = false;
                        } else {
                            out.extend(d.pending.drain(..).map(Notification::Input));
                        }
                    }
                    continue;
                }
                if d.dropped || (event.type_ == 1 && event.value == 2) {
                    continue;
                }
                let prefix = match event.type_ {
                    1 => "button",
                    2 => "relative",
                    3 => "axis",
                    _ => continue,
                };
                let control = format!("{prefix}:{}", event.code);
                if d.device.controls.iter().any(|c| c.id == control) {
                    d.pending.push(Event {
                        device: d.device.id.clone(),
                        control,
                        value: event.value as f64,
                        baseline: false,
                    });
                    if d.pending.len() > 1024 {
                        d.pending.clear();
                        d.dropped = true;
                    }
                }
            }
        }
        for path in lost {
            if let Some(d) = self.devices.remove(&path) {
                out.push(Notification::Disconnected(d.device.id));
            }
        }
        out
    }
    pub fn rumble(
        &mut self,
        id: &str,
        strong: f64,
        weak: f64,
        duration: Duration,
    ) -> io::Result<()> {
        let d = self
            .devices
            .values_mut()
            .find(|d| d.device.id == id)
            .ok_or_else(|| io::Error::other("device disconnected"))?;
        if !d.device.rumble {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "device has no FF_RUMBLE",
            ));
        }
        // SAFETY: integer fields/union storage are all valid when zeroed.
        let mut effect: libc::ff_effect = unsafe { std::mem::zeroed() };
        effect.type_ = 0x50;
        effect.id = d.effect;
        effect.replay.length = duration.as_millis().min(2000) as u16;
        let r = libc::ff_rumble_effect {
            strong_magnitude: (strong * 65535.) as u16,
            weak_magnitude: (weak * 65535.) as u16,
        };
        // SAFETY: rumble is the selected effect type; union storage is aligned and large enough.
        unsafe {
            std::ptr::write(effect.u.as_mut_ptr().cast::<libc::ff_rumble_effect>(), r);
        }
        // SAFETY: EVIOCSFF reads/writes exactly the supplied ff_effect.
        if unsafe {
            libc::ioctl(
                d.file.as_raw_fd(),
                request(1, 0x80, std::mem::size_of::<libc::ff_effect>()),
                &mut effect,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        d.effect = effect.id;
        write_effect(&mut d.file, d.effect, 1)
    }
    pub fn stop_device(&mut self, id: &str) {
        if let Some(d) = self.devices.values_mut().find(|d| d.device.id == id)
            && d.effect >= 0
        {
            let _ = write_effect(&mut d.file, d.effect, 0);
        }
    }
    pub fn stop(&mut self) {
        for d in self.devices.values_mut() {
            if d.effect >= 0 {
                let _ = write_effect(&mut d.file, d.effect, 0);
            }
        }
    }
}
fn write_effect(file: &mut File, id: i16, value: i32) -> io::Result<()> {
    // SAFETY: zero is valid for timeval and integer fields.
    let mut e: libc::input_event = unsafe { std::mem::zeroed() };
    e.type_ = 0x15;
    e.code = id as u16;
    e.value = value;
    // SAFETY: borrow exact initialized repr(C) input_event bytes for the write.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&e as *const libc::input_event).cast::<u8>(),
            std::mem::size_of_val(&e),
        )
    };
    file.write_all(bytes)
}

fn controller(keys: &[u8], axes: &[u8]) -> bool {
    !has(keys, 272)
        && !has(keys, 330)
        && !has(axes, 47)
        && !(1..256).any(|n| has(keys, n))
        && ((0x120..0x140).any(|n| has(keys, n))
            || (0x100..0x110).any(|n| has(keys, n))
            || (0..9).any(|n| has(axes, n)))
}
fn capability_bits(text: &str) -> [u8; 128] {
    let mut result = [0u8; 128];
    for (i, word) in text.split_whitespace().rev().enumerate() {
        if let Ok(word) = usize::from_str_radix(word, 16) {
            for bit in 0..usize::BITS as usize {
                let n = i * usize::BITS as usize + bit;
                if n / 8 < result.len() && word & (1usize << bit) != 0 {
                    result[n / 8] |= 1 << (n % 8);
                }
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_collections_are_not_controllers() {
        let mut k = [0u8; 128];
        let mut a = [0u8; 128];
        k[304 / 8] |= 1 << (304 % 8);
        a[0] = 3;
        assert!(controller(&k, &a));
        k[28 / 8] |= 1 << (28 % 8);
        assert!(!controller(&k, &a));
        assert_eq!(capability_bits("3")[0], 3);
    }
}
