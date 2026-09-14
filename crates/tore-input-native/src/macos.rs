//! Generic IOKit HID queues plus Apple GameController input/haptics on macOS 11+.
mod game_controller;
use super::*;
use game_controller::GameControllers;
use std::{
    ffi::{c_char, c_void},
    io, ptr,
};
type Ref = *const c_void;
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Ref);
    fn CFStringCreateWithCString(allocator: Ref, text: *const c_char, encoding: u32) -> Ref;
    fn CFStringGetCString(value: Ref, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
    fn CFNumberCreate(allocator: Ref, kind: isize, value: Ref) -> Ref;
    fn CFNumberGetValue(number: Ref, kind: isize, value: *mut c_void) -> bool;
    fn CFDictionaryCreateMutable(
        allocator: Ref,
        capacity: isize,
        key_callbacks: Ref,
        value_callbacks: Ref,
    ) -> Ref;
    fn CFDictionarySetValue(dictionary: Ref, key: Ref, value: Ref);
    fn CFArrayCreate(allocator: Ref, values: *const Ref, count: isize, callbacks: Ref) -> Ref;
    fn CFArrayGetCount(array: Ref) -> isize;
    fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    fn CFSetGetCount(set: Ref) -> isize;
    fn CFSetGetValues(set: Ref, values: *mut Ref);
    fn CFRunLoopGetCurrent() -> Ref;
    fn CFRunLoopRunInMode(mode: Ref, seconds: f64, return_after_source: bool) -> i32;
    static kCFRunLoopDefaultMode: Ref;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    static kCFTypeArrayCallBacks: u8;
}
#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOHIDManagerCreate(allocator: Ref, options: u32) -> Ref;
    fn IOHIDManagerSetDeviceMatchingMultiple(manager: Ref, multiple: Ref);
    fn IOHIDManagerScheduleWithRunLoop(manager: Ref, runloop: Ref, mode: Ref);
    fn IOHIDManagerUnscheduleFromRunLoop(manager: Ref, runloop: Ref, mode: Ref);
    fn IOHIDManagerOpen(manager: Ref, options: u32) -> i32;
    fn IOHIDManagerClose(manager: Ref, options: u32) -> i32;
    fn IOHIDManagerCopyDevices(manager: Ref) -> Ref;
    fn IOHIDDeviceGetProperty(device: Ref, key: Ref) -> Ref;
    fn IOHIDDeviceCopyMatchingElements(device: Ref, matching: Ref, options: u32) -> Ref;
    fn IOHIDElementGetType(element: Ref) -> u32;
    fn IOHIDElementGetUsagePage(element: Ref) -> u32;
    fn IOHIDElementGetUsage(element: Ref) -> u32;
    fn IOHIDElementGetReportSize(element: Ref) -> u32;
    fn IOHIDElementGetCookie(element: Ref) -> u32;
    fn IOHIDElementGetLogicalMin(element: Ref) -> isize;
    fn IOHIDElementGetLogicalMax(element: Ref) -> isize;
    fn IOHIDElementIsRelative(element: Ref) -> bool;
    fn IOHIDQueueCreate(allocator: Ref, device: Ref, depth: isize, options: u32) -> Ref;
    fn IOHIDQueueAddElement(queue: Ref, element: Ref);
    fn IOHIDQueueStart(queue: Ref);
    fn IOHIDQueueStop(queue: Ref);
    fn IOHIDQueueCopyNextValueWithTimeout(queue: Ref, timeout: f64) -> Ref;
    fn IOHIDValueGetElement(value: Ref) -> Ref;
    fn IOHIDValueGetIntegerValue(value: Ref) -> isize;
    fn IOHIDDeviceGetValue(device: Ref, element: Ref, value: *mut Ref) -> i32;
}
struct Owned(Ref);
impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: each Owned wraps exactly one Create/Copy reference.
            unsafe {
                CFRelease(self.0);
            }
        }
    }
}
fn string(text: &str) -> Owned {
    let text = std::ffi::CString::new(text).expect("static HID key");
    // SAFETY: terminated string and default allocator; returned reference owned.
    Owned(unsafe { CFStringCreateWithCString(ptr::null(), text.as_ptr(), 0x08000100) })
}
fn property(device: Ref, key: &str) -> Ref {
    let key = string(key);
    // SAFETY: live device and CFString property key; borrowed result.
    unsafe { IOHIDDeviceGetProperty(device, key.0) }
}
fn number(device: Ref, key: &str) -> i64 {
    let p = property(device, key);
    let mut n = 0i64;
    if !p.is_null() {
        // SAFETY: these documented numeric HID properties are CFNumbers; SInt64 storage.
        unsafe {
            CFNumberGetValue(p, 4, (&mut n as *mut i64).cast());
        }
    }
    n
}
fn label(device: Ref, key: &str) -> String {
    let p = property(device, key);
    let mut bytes = [0u8; 512];
    if !p.is_null() {
        // SAFETY: these documented text properties are CFStrings; bounded writable buffer.
        unsafe {
            CFStringGetCString(p, bytes.as_mut_ptr().cast(), 512, 0x08000100);
        }
    }
    String::from_utf8_lossy(&bytes[..bytes.iter().position(|v| *v == 0).unwrap_or(512)]).into()
}
struct OpenDevice {
    queue: Owned,
    elements: Owned,
    device: Device,
}
impl Drop for OpenDevice {
    fn drop(&mut self) {
        // SAFETY: queue still owned and live until after this drop body.
        unsafe {
            IOHIDQueueStop(self.queue.0);
        }
    }
}
pub struct Platform {
    gamepads: GameControllers,
    devices: BTreeMap<usize, OpenDevice>,
    manager: Owned,
    scan: Instant,
}
impl Platform {
    pub fn new() -> io::Result<Self> {
        // SAFETY: all objects created on this worker and used/released on the same thread.
        unsafe {
            let manager = Owned(IOHIDManagerCreate(ptr::null(), 0));
            if manager.0.is_null() {
                return Err(io::Error::other("IOHIDManager allocation failed"));
            }
            let mut dictionaries = vec![];
            for usage in [4i32, 5, 8] {
                // Generic Desktop joystick/gamepad/multi-axis.
                let d = Owned(CFDictionaryCreateMutable(
                    ptr::null(),
                    2,
                    (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
                    (&raw const kCFTypeDictionaryValueCallBacks).cast(),
                ));
                if d.0.is_null() {
                    return Err(io::Error::other("HID matching allocation failed"));
                }
                for (key, value) in [("DeviceUsagePage", 1i32), ("DeviceUsage", usage)] {
                    let k = string(key);
                    let v = Owned(CFNumberCreate(
                        ptr::null(),
                        3,
                        (&value as *const i32).cast(),
                    ));
                    CFDictionarySetValue(d.0, k.0, v.0);
                }
                dictionaries.push(d);
            }
            let refs: Vec<_> = dictionaries.iter().map(|d| d.0).collect();
            let matching = Owned(CFArrayCreate(
                ptr::null(),
                refs.as_ptr(),
                refs.len() as isize,
                (&raw const kCFTypeArrayCallBacks).cast(),
            ));
            IOHIDManagerSetDeviceMatchingMultiple(manager.0, matching.0);
            let status = IOHIDManagerOpen(manager.0, 0);
            if status != 0 {
                return Err(io::Error::other(format!("IOHIDManagerOpen: {status:#x}")));
            }
            IOHIDManagerScheduleWithRunLoop(
                manager.0,
                CFRunLoopGetCurrent(),
                kCFRunLoopDefaultMode,
            );
            Ok(Self {
                gamepads: GameControllers::new(),
                devices: BTreeMap::new(),
                manager,
                scan: Instant::now() - Duration::from_secs(2),
            })
        }
    }
    fn open(device: Ref) -> io::Result<OpenDevice> {
        // SAFETY: live device from retained manager set; all Copy objects owned locally.
        unsafe {
            let elements = Owned(IOHIDDeviceCopyMatchingElements(device, ptr::null(), 0));
            if elements.0.is_null() {
                return Err(io::Error::other("HID elements unavailable"));
            }
            let count = CFArrayGetCount(elements.0);
            if !(0..=4096).contains(&count) {
                return Err(io::Error::other("HID element limit exceeded"));
            }
            let queue = Owned(IOHIDQueueCreate(ptr::null(), device, 4096, 0));
            if queue.0.is_null() {
                return Err(io::Error::other("HID queue allocation failed"));
            }
            let mut controls = vec![];
            for i in 0..count {
                let e = CFArrayGetValueAtIndex(elements.0, i);
                let kind = IOHIDElementGetType(e);
                if !(1..=3).contains(&kind) || IOHIDElementGetReportSize(e) > 32 {
                    continue;
                }
                let min = IOHIDElementGetLogicalMin(e) as f64;
                let max = IOHIDElementGetLogicalMax(e) as f64;
                let mut value = ptr::null();
                if IOHIDDeviceGetValue(device, e, &mut value) != 0 || value.is_null() {
                    continue;
                }
                controls.push(Control {
                    id: format!("element:{}", IOHIDElementGetCookie(e)),
                    kind: if IOHIDElementGetUsagePage(e) == 1 && IOHIDElementGetUsage(e) == 0x39 {
                        Kind::Position
                    } else if IOHIDElementIsRelative(e) {
                        Kind::Relative
                    } else if kind == 2 {
                        Kind::Button
                    } else {
                        Kind::Axis
                    },
                    min,
                    max,
                    value: IOHIDValueGetIntegerValue(value) as f64,
                });
                IOHIDQueueAddElement(queue.0, e);
            }
            IOHIDQueueStart(queue.0);
            let serial = label(device, "SerialNumber");
            let id = format!(
                "macos-{:04x}-{:04x}-{}",
                number(device, "VendorID"),
                number(device, "ProductID"),
                if serial.is_empty() {
                    format!("location-{:x}", number(device, "LocationID"))
                } else {
                    encode(&serial)
                }
            );
            Ok(OpenDevice {
                queue,
                elements,
                device: Device {
                    id,
                    name: label(device, "Product"),
                    controls,
                    rumble: false,
                },
            })
        }
    }
    pub fn poll(&mut self) -> Vec<Notification> {
        let mut out = self.gamepads.poll();
        // SAFETY: nonblocking run-loop service on the manager's owning worker.
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0., true);
        }
        if self.scan.elapsed() >= Duration::from_secs(1) {
            self.scan = Instant::now();
            // SAFETY: copied set kept alive through device enumeration.
            unsafe {
                let set = Owned(IOHIDManagerCopyDevices(self.manager.0));
                let mut values = vec![];
                if !set.0.is_null() {
                    let n = CFSetGetCount(set.0);
                    if (0..=64).contains(&n) {
                        values.resize(n as usize, ptr::null());
                        CFSetGetValues(set.0, values.as_mut_ptr());
                    }
                }
                values.retain(|device| !GameControllers::handles(*device));
                self.devices.retain(|id, d| {
                    if values.iter().any(|p| *p as usize == *id) {
                        true
                    } else {
                        out.push(Notification::Disconnected(d.device.id.clone()));
                        false
                    }
                });
                for device in values {
                    if self.devices.contains_key(&(device as usize)) {
                        continue;
                    }
                    match Self::open(device) {
                        Ok(d) => {
                            out.push(Notification::Connected(d.device.clone()));
                            self.devices.insert(device as usize, d);
                        }
                        Err(e) => out.push(Notification::Warning(e.to_string())),
                    }
                }
            }
        }
        for d in self.devices.values_mut() {
            // Keep element array alive for the queue's complete lifetime.
            let _ = &d.elements;
            for _ in 0..512 {
                // SAFETY: live queue; timeout zero never waits for a hardware report.
                let value = Owned(unsafe { IOHIDQueueCopyNextValueWithTimeout(d.queue.0, 0.) });
                if value.0.is_null() {
                    break;
                }
                // SAFETY: borrowed element belongs to this retained HID value.
                let (cookie, n) = unsafe {
                    (
                        IOHIDElementGetCookie(IOHIDValueGetElement(value.0)),
                        IOHIDValueGetIntegerValue(value.0),
                    )
                };
                out.push(Notification::Input(Event {
                    device: d.device.id.clone(),
                    control: format!("element:{cookie}"),
                    value: n as f64,
                    baseline: false,
                }));
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
        self.gamepads.rumble(id, strong, weak, duration)
    }
    pub fn stop_device(&mut self, id: &str) {
        self.gamepads.stop_device(id);
    }
    pub fn stop(&mut self) {
        self.gamepads.stop();
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        self.stop();
        self.devices.clear();
        // SAFETY: unschedule/close on owning run loop before releasing the manager.
        unsafe {
            IOHIDManagerUnscheduleFromRunLoop(
                self.manager.0,
                CFRunLoopGetCurrent(),
                kCFRunLoopDefaultMode,
            );
            IOHIDManagerClose(self.manager.0, 0);
        }
    }
}
