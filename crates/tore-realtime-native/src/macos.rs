//! The Mach thread policy and the NSProcessInfo activity. Ports, pointers and
//! Objective-C objects never escape this module.
use crate::TimeConstraint;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2::{class, msg_send, msg_send_id};
use objc2_foundation::NSString;
use std::ffi::c_void;
use std::time::Duration;

/// `thread_time_constraint_policy` from `<mach/thread_policy.h>`, in Mach
/// absolute time units.
#[repr(C)]
struct ThreadTimeConstraintPolicy {
    period: u32,
    computation: u32,
    constraint: u32,
    /// `boolean_t`, 32 bits on both architectures.
    preemptible: u32,
}

#[repr(C)]
#[derive(Default)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

const THREAD_TIME_CONSTRAINT_POLICY: u32 = 2;
/// The policy's size in `integer_t`s.
const THREAD_TIME_CONSTRAINT_POLICY_COUNT: u32 = 4;
const KERN_SUCCESS: i32 = 0;

// libSystem, which every macOS program links.
unsafe extern "C" {
    fn pthread_self() -> *mut c_void;
    fn pthread_mach_thread_np(thread: *mut c_void) -> u32;
    fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    fn thread_policy_set(thread: u32, flavor: u32, policy: *mut i32, count: u32) -> i32;
}

/// Mach absolute time units in `duration`, given the timebase (nanoseconds
/// = units * numer / denom).
fn units(duration: Duration, timebase: &MachTimebaseInfo) -> Result<u32, String> {
    let units = duration.as_nanos() * u128::from(timebase.denom) / u128::from(timebase.numer);
    u32::try_from(units).map_err(|_| format!("{duration:?} is too long for a Mach policy"))
}

pub fn set_time_constraint(policy: TimeConstraint) -> Result<(), String> {
    let mut timebase = MachTimebaseInfo::default();
    // SAFETY: the pointer is to a live, writable struct of the C layout.
    let status = unsafe { mach_timebase_info(&mut timebase) };
    if status != KERN_SUCCESS || timebase.numer == 0 || timebase.denom == 0 {
        return Err(format!("mach_timebase_info failed ({status})"));
    }
    let mut info = ThreadTimeConstraintPolicy {
        period: units(policy.period, &timebase)?,
        computation: units(policy.computation, &timebase)?,
        constraint: units(policy.constraint, &timebase)?,
        preemptible: 1,
    };
    // SAFETY: pthread_self names the calling thread, which outlives the call;
    // pthread_mach_thread_np returns its port without adding a reference, so
    // there is nothing to release. thread_policy_set reads exactly COUNT
    // integers from the live, C-layout policy and keeps no pointer to it.
    let status = unsafe {
        let thread = pthread_mach_thread_np(pthread_self());
        thread_policy_set(
            thread,
            THREAD_TIME_CONSTRAINT_POLICY,
            (&raw mut info).cast::<i32>(),
            THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        )
    };
    if status == KERN_SUCCESS {
        Ok(())
    } else {
        Err(format!("thread_policy_set refused it ({status})"))
    }
}

/// `NSActivityUserInitiated | NSActivityLatencyCritical` from
/// `<Foundation/NSProcessInfo.h>`. User-initiated is 0x00FFFFFF with
/// `NSActivityIdleSystemSleepDisabled` (1 << 20); latency-critical is
/// 0xFF00000000.
const ACTIVITY_OPTIONS: u64 = 0x00FF_FFFF | (1 << 20) | 0xFF_0000_0000;

/// A begun activity; dropping it ends it.
pub struct Activity {
    process: Retained<AnyObject>,
    token: Retained<AnyObject>,
}

impl Activity {
    pub fn begin(reason: &str) -> Result<Self, String> {
        autoreleasepool(|_| {
            let reason = NSString::from_str(reason);
            // SAFETY: +[NSProcessInfo processInfo] takes no arguments and
            // returns the shared object (retained here), which is thread
            // safe; beginActivityWithOptions:reason: takes an
            // NSActivityOptions (uint64_t) and a non-nil NSString, and
            // returns the activity's token, retained here until the end.
            unsafe {
                let process: Option<Retained<AnyObject>> =
                    msg_send_id![class!(NSProcessInfo), processInfo];
                let process = process.ok_or_else(|| "NSProcessInfo is unavailable".to_owned())?;
                let token: Option<Retained<AnyObject>> = msg_send_id![
                    &*process,
                    beginActivityWithOptions: ACTIVITY_OPTIONS,
                    reason: &*reason
                ];
                let token = token.ok_or_else(|| "NSProcessInfo refused the activity".to_owned())?;
                Ok(Self { process, token })
            }
        })
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        autoreleasepool(|_| {
            // SAFETY: endActivity: takes the token beginActivity returned,
            // which is still retained, and returns nothing.
            unsafe {
                let _: () = msg_send![&*self.process, endActivity: &*self.token];
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_become_mach_units_on_either_timebase() {
        let tick = Duration::from_secs(1) / 120;
        // Intel: one unit a nanosecond.
        let intel = MachTimebaseInfo { numer: 1, denom: 1 };
        assert_eq!(units(tick, &intel), Ok(8_333_333));
        // Apple silicon: 24 MHz, 125/3 ns a unit.
        let apple = MachTimebaseInfo {
            numer: 125,
            denom: 3,
        };
        assert_eq!(units(tick, &apple), Ok(199_999));
        assert!(units(Duration::from_secs(1_000), &intel).is_err());
    }
}
