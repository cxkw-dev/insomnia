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

    pub fn label(self) -> &'static str {
        match self {
            Kind::Display => "display",
            Kind::Idle => "system",
            Kind::System => "ac lock",
            Kind::Disk => "disk",
        }
    }
}

pub struct Assertion {
    id: IOPMAssertionID,
    pub kind: Kind,
}

pub fn create(kind: Kind) -> Result<Assertion, String> {
    let ty = CString::new(kind.assertion_type()).unwrap();
    let name = CString::new(format!("insomnia: keeping {} awake", kind.label())).unwrap();
    unsafe {
        let cf_ty =
            CFStringCreateWithCString(std::ptr::null(), ty.as_ptr(), KCF_STRING_ENCODING_UTF8);
        let cf_name =
            CFStringCreateWithCString(std::ptr::null(), name.as_ptr(), KCF_STRING_ENCODING_UTF8);
        // NULL on allocation failure — and CFRelease(NULL) is an immediate crash.
        if cf_ty.is_null() || cf_name.is_null() {
            if !cf_ty.is_null() {
                CFRelease(cf_ty);
            }
            if !cf_name.is_null() {
                CFRelease(cf_name);
            }
            return Err("could not allocate CoreFoundation strings".into());
        }
        let mut id: IOPMAssertionID = 0;
        let ret = IOPMAssertionCreateWithName(cf_ty, K_IOPM_ASSERTION_LEVEL_ON, cf_name, &mut id);
        CFRelease(cf_ty);
        CFRelease(cf_name);
        if ret == 0 {
            Ok(Assertion { id, kind })
        } else {
            Err(format!(
                "could not create {} assertion (IOReturn {ret:#x})",
                kind.label()
            ))
        }
    }
}

impl Drop for Assertion {
    // The kernel also releases assertions automatically when the process exits,
    // so even SIGKILL can't leave the display pinned awake.
    fn drop(&mut self) {
        unsafe {
            IOPMAssertionRelease(self.id);
        }
    }
}

#[derive(Clone, Debug)]
pub struct PowerStatus {
    pub on_ac: bool,
    pub percent: Option<u8>,
}

impl PowerStatus {
    pub fn poll() -> Self {
        match Command::new("pmset").args(["-g", "batt"]).output() {
            Ok(out) => parse_pmset(&String::from_utf8_lossy(&out.stdout)),
            Err(_) => PowerStatus {
                on_ac: true,
                percent: None,
            },
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
