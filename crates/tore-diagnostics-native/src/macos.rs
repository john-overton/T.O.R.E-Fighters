use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication};
use objc2_foundation::{MainThreadMarker, NSString};

pub fn show_error(title: &str, message: &str) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("NSAlert requires the main thread")?;
    autoreleasepool(|_| {
        // Initialize AppKit even when startup failed before winit existed.
        let application = NSApplication::sharedApplication(mtm);
        // Keep support for macOS versions before the newer activate API.
        #[allow(deprecated)]
        application.activateIgnoringOtherApps(true);
        // SAFETY: MainThreadMarker verifies AppKit thread affinity. The retained
        // alert and strings remain alive until its synchronous modal loop exits.
        // No delegates, callbacks, raw pointers or borrowed views are installed.
        unsafe {
            let alert = NSAlert::new(mtm);
            alert.setAlertStyle(NSAlertStyle::Critical);
            alert.setMessageText(&NSString::from_str(title));
            alert.setInformativeText(&NSString::from_str(message));
            alert.runModal();
        }
    });
    Ok(())
}
