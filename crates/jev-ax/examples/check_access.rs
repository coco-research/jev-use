// Examples are separate crates, so the library's `allow(unsafe_code)` does not reach them.
// Same reason as there: unsafe is the interface to the accessibility API, not an oversight,
// and the reason is documented at each call site instead of being silenced wholesale.
#![allow(unsafe_code)]

//! Check that this process can actually read the accessibility tree, and say why not when
//! it cannot.
//!
//! This is the check a CI runner needs before it can claim to have tested the macOS path:
//! the code compiling is not the same as the code being allowed to look. Accessibility is
//! granted per process by TCC, so a self-hosted runner that is not in
//! System Settings -> Privacy & Security -> Accessibility will compile every test and
//! fail every live read with `APIDisabled (-25211)`.
//!
//! ```text
//! cargo run --example check_access
//! ```
//!
//! Exit codes: 0 readable, 1 not readable (with the reason), 2 wrong platform.

use objc2_application_services::AXUIElement;

use jev_ax::attrs;

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("this example needs macOS: accessibility is not a thing elsewhere");
        std::process::exit(2);
    }

    #[cfg(target_os = "macos")]
    {
        println!("accessibility check");
        println!("  pid        {}", std::process::id());

        // The system-wide element always exists. Its role is the cheapest honest read:
        // `AXSystemWide` means reads work, an empty string means they do not.
        let wide = unsafe { AXUIElement::new_system_wide() };
        // SAFETY: the system-wide element is always live.
        let role = unsafe { attrs::role(&wide) };
        let wide_err = unsafe { attrs::raw(&wide, attrs::name::ROLE) }.err();
        println!("  system-wide role {role:?} (error {wide_err:?})");

        // A real app element, which is what the walk needs and what permission gates.
        let mut app_ok = false;
        let mut app_detail = String::from("Finder is not running");
        if let Some(pid) = pid_of("Finder") {
            // SAFETY: the pid came from a running process.
            let app = unsafe { AXUIElement::new_application(pid) };
            // SAFETY: live element.
            let app_role = unsafe { attrs::role(&app) };
            // SAFETY: live element.
            let menu = unsafe { attrs::raw(&app, "AXMenuBar") };
            let menu_role = menu
                .as_ref()
                // SAFETY: live element.
                .map(|v| unsafe { attrs::role(&as_element(v)) })
                .unwrap_or_default();
            // SAFETY: live element.
            let actions = unsafe { attrs::action_names(&app) };
            app_ok = app_role == "AXApplication";
            app_detail = format!(
                "role {app_role:?} menu bar {menu_role:?} actions {actions:?} error {:?}",
                unsafe { attrs::raw(&app, attrs::name::ROLE) }.err()
            );
        }
        println!("  Finder     {app_detail}");

        if !role.is_empty() && app_ok {
            println!("\n  readable: live accessibility tests can run in this process");
            return;
        }

        println!("\n  NOT readable.");
        println!("  the accessibility API answered, but not with what a live test needs.");
        println!("  On macOS this is almost always one of two things:");
        println!("    1. this process is not trusted: System Settings -> Privacy & Security");
        println!("       -> Accessibility, add the binary that is running (in CI, the runner)");
        println!("    2. the app is not answering yet: try again, or raise the messaging");
        println!("       timeout (see rule R2)");
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
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
#[cfg(target_os = "macos")]
fn as_element(v: &objc2_core_foundation::CFType) -> objc2_core_foundation::CFRetained<AXUIElement> {
    use std::ptr::NonNull;
    // SAFETY: only called on element-valued attributes, which are +1 AXUIElement
    // references.
    unsafe {
        let p = v as *const objc2_core_foundation::CFType as *mut AXUIElement;
        objc2_core_foundation::CFRetained::retain(NonNull::new_unchecked(p))
    }
}
