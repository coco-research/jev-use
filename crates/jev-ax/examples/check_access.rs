//! Check that this process can actually read the accessibility tree, and say why not when
//! it cannot.
//!
//! This is the check a CI runner needs before it can claim to have tested the macOS path:
//! the code compiling is not the same as the code being allowed to look. Accessibility is
//! granted per process by TCC, so a runner that is not in
//! System Settings -> Privacy & Security -> Accessibility will compile every test and
//! fail every live read.
//!
//! ```text
//! cargo run --example check_access
//! ```
//!
//! Exit codes: 0 readable, 1 not readable (with the reason), 2 wrong platform.

// Examples are separate crates, so the library's `allow(unsafe_code)` does not reach here.
// Same reason as there: unsafe is the interface to the accessibility API, and the reason is
// documented at each call site rather than silenced wholesale.
#![allow(unsafe_code)]

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this example needs macOS: accessibility is not a thing elsewhere");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
fn main() {
    std::process::exit(mac::run());
}

/// The whole check, so that the non-macOS `main` above does not have to pretend to compile
/// code that only exists here.
#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;

    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::{CFRetained, CFType};

    use jev_ax::attrs;

    pub fn run() -> i32 {
        println!("accessibility check");
        println!("  pid             {}", std::process::id());

        // The system-wide element always exists. Its role is the cheapest honest read:
        // `AXSystemWide` means reads work at all, an empty string means they do not.
        let wide = unsafe { AXUIElement::new_system_wide() };
        // SAFETY: the system-wide element is always live.
        let wide_role = unsafe { attrs::role(&wide) };
        // SAFETY: as above.
        let wide_err = unsafe { attrs::raw(&wide, attrs::name::ROLE) }.err();
        println!("  system-wide     role {wide_role:?} error {wide_err:?}");

        // A real app element: what the walk needs, and what permission gates.
        let mut app_ok = false;
        let mut detail = String::from("Finder is not running");
        if let Some(pid) = pid_of("Finder") {
            // SAFETY: the pid came from a running process.
            let app = unsafe { AXUIElement::new_application(pid) };
            // SAFETY: live element.
            let app_role = unsafe { attrs::role(&app) };
            // SAFETY: live element.
            let menu = unsafe { attrs::raw(&app, "AXMenuBar") };
            let menu_role = menu
                .as_deref()
                // SAFETY: AXMenuBar is an element-valued attribute, re-typed below.
                .map(|v| unsafe { attrs::role(&as_element(v)) })
                .unwrap_or_default();
            // SAFETY: live element.
            let err = unsafe { attrs::raw(&app, attrs::name::ROLE) }.err();
            app_ok = app_role == "AXApplication" && menu_role == "AXMenuBar";
            detail = format!("role {app_role:?} menu bar {menu_role:?} error {err:?}");
        }
        println!("  Finder          {detail}");

        if !wide_role.is_empty() && app_ok {
            println!();
            println!("  readable: live accessibility tests can run in this process");
            return 0;
        }

        println!();
        println!("  NOT readable. The API answered, but not with what a live test needs.");
        println!("  On macOS that is almost always one of two things:");
        println!("    1. this process is not trusted. System Settings -> Privacy & Security");
        println!("       -> Accessibility -> add the binary that runs it (in CI, the runner:");
        println!("       bin/Runner.Listener in the runner's directory)");
        println!("    2. the app is not answering: raise the messaging timeout (rule R2)");
        1
    }

    fn pid_of(name: &str) -> Option<i32> {
        let out = std::process::Command::new("pgrep")
            .args(["-x", name])
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()
    }

    /// Re-type an element-valued attribute read. T4 grows a real accessor for this.
    fn as_element(v: &CFType) -> CFRetained<AXUIElement> {
        // SAFETY: only called on element-valued attributes, which are +1 AXUIElement
        // references.
        unsafe {
            let p = v as *const CFType as *mut AXUIElement;
            CFRetained::retain(NonNull::new_unchecked(p))
        }
    }
}
