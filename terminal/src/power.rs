//! IOKit power assertions (what keeps the Mac awake) and battery status.

use std::ffi::{c_char, c_void, CString};
use std::process::Command;

type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type IOReturn = i32;
type IOPMAssertionID = u32;

const KCF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_IOPM_ASSERTION_LEVEL_ON: u32 = 255;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFRelease(cf: *const c_void);
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOPMAssertionCreateWithName(
        assertion_type: CFStringRef,
        assertion_level: u32,
        assertion_name: CFStringRef,
        assertion_id: *mut IOPMAssertionID,
    ) -> IOReturn;
    fn IOPMAssertionRelease(assertion_id: IOPMAssertionID) -> IOReturn;
}

/// An owned CFString, released on drop.
struct CfString(CFStringRef);

impl CfString {
    fn new(s: &str) -> Option<Self> {
        let c = CString::new(s).ok()?;
        // SAFETY: `c` is a valid NUL-terminated UTF-8 string for the call's
        // duration; a NULL allocator means the default one.
        let r = unsafe {
            CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), KCF_STRING_ENCODING_UTF8)
        };
        // NULL on allocation failure — and CFRelease(NULL) would crash.
        (!r.is_null()).then_some(Self(r))
    }
}

impl Drop for CfString {
    fn drop(&mut self) {
        // SAFETY: created by CFStringCreateWithCString (+1 retain), non-null,
        // and released exactly once here.
        unsafe { CFRelease(self.0) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Display,
    Idle,
    System,
    Disk,
}

impl Kind {
    fn assertion_type(self) -> &'static str {
        match self {
            Kind::Display => "PreventUserIdleDisplaySleep",
            Kind::Idle => "PreventUserIdleSystemSleep",
            Kind::System => "PreventSystemSleep",
            Kind::Disk => "PreventDiskIdle",
        }
    }

    /// What the user sees held. `Idle` keeps the *system* up while idle;
    /// `System` blocks sleep outright, which macOS only honors on AC power —
    /// hence "ac lock".
    pub fn label(self) -> &'static str {
        match self {
            Kind::Display => "display",
            Kind::Idle => "system",
            Kind::System => "ac lock",
            Kind::Disk => "disk",
        }
    }
}

/// A held power assertion. Dropping it releases the hold, so bind it to a
/// variable that lives as long as the Mac should stay awake.
#[must_use = "the assertion is released as soon as it is dropped"]
pub struct Assertion {
    id: IOPMAssertionID,
}

pub fn create(kind: Kind) -> Result<Assertion, String> {
    let ty = CfString::new(kind.assertion_type());
    let name = CfString::new(&format!("insomnia: keeping {} awake", kind.label()));
    let (Some(ty), Some(name)) = (ty, name) else {
        return Err("could not allocate CoreFoundation strings".into());
    };
    let mut id: IOPMAssertionID = 0;
    // SAFETY: both CFStrings are valid for the call and `id` is a valid out
    // pointer; IOKit copies what it keeps.
    let ret =
        unsafe { IOPMAssertionCreateWithName(ty.0, K_IOPM_ASSERTION_LEVEL_ON, name.0, &mut id) };
    if ret == 0 {
        Ok(Assertion { id })
    } else {
        Err(format!(
            "could not create {} assertion (IOReturn {ret:#x})",
            kind.label()
        ))
    }
}

impl Drop for Assertion {
    // The kernel also releases assertions automatically when the process exits,
    // so even SIGKILL can't leave the display pinned awake.
    fn drop(&mut self) {
        // SAFETY: `id` came from a successful IOPMAssertionCreateWithName and
        // is released exactly once.
        unsafe {
            IOPMAssertionRelease(self.id);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerStatus {
    pub on_ac: bool,
    pub percent: Option<u8>,
}

/// Below this, a Mac on battery reads as low.
pub const LOW_BATTERY_PCT: u8 = 20;

impl Default for PowerStatus {
    /// Unknown reads as AC power: better than a false "battery" warning
    /// while the first `pmset` poll is in flight or when it can't run.
    fn default() -> Self {
        Self {
            on_ac: true,
            percent: None,
        }
    }
}

impl PowerStatus {
    pub fn poll() -> Self {
        match Command::new("pmset").args(["-g", "batt"]).output() {
            Ok(out) if out.status.success() => parse_pmset(&String::from_utf8_lossy(&out.stdout)),
            _ => Self::default(),
        }
    }

    pub fn low(&self) -> bool {
        !self.on_ac && self.percent.is_some_and(|p| p < LOW_BATTERY_PCT)
    }

    /// `AC POWER · BATTERY 100%`, `BATTERY 64%`, …
    pub fn label(&self) -> String {
        match (self.on_ac, self.percent) {
            (true, Some(p)) => format!("AC POWER · BATTERY {p}%"),
            (true, None) => "AC POWER".into(),
            (false, Some(p)) => format!("BATTERY {p}%"),
            (false, None) => "BATTERY".into(),
        }
    }
}

fn parse_pmset(s: &str) -> PowerStatus {
    let on_ac = s.contains("AC Power");
    let percent = s.find('%').and_then(|i| {
        let digits: Vec<char> = s[..i]
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        digits
            .into_iter()
            .rev()
            .collect::<String>()
            .parse::<u8>()
            .ok()
    });
    PowerStatus { on_ac, percent }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmset_output_parses() {
        let ac = parse_pmset(
            "Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged; 0:00 remaining",
        );
        assert_eq!(
            ac,
            PowerStatus {
                on_ac: true,
                percent: Some(100)
            }
        );
        let batt =
            parse_pmset("Now drawing from 'Battery Power'\n -InternalBattery-0\t12%; discharging");
        assert!(batt.low());
        assert_eq!(batt.label(), "BATTERY 12%");
    }
}
