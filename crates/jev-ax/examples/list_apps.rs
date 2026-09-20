//! List the apps this machine can be driven in, and resolve a name to one of them.
//!
//! This is the live check for T2. It runs the macOS path, which no CI runner can
//! execute: an ubuntu runner has no AppKit and no window server.
//!
//! ```text
//! cargo run --example list_apps
//! cargo run --example list_apps -- chrome
//! ```
//!
//! Compare with the Python reference, which is the parity target:
//!
//! ```text
//! jev-use --apps
//! ```

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("this example needs macOS: AppKit is not available elsewhere");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
fn main() {
    use jev_ax::app::{frontmost, match_app, resolve, running_apps};

    let apps = running_apps();

    println!("{} app(s) with a UI, frontmost first\n", apps.len());
    for a in &apps {
        println!(
            "  {:>7}  {}{}",
            a.pid,
            a.name,
            if a.frontmost { "   <- frontmost" } else { "" }
        );
    }

    match frontmost(&apps) {
        Some(f) => println!("\nfrontmost: {} ({})", f.name, f.pid),
        None => println!("\nfrontmost: none reported"),
    }

    // Resolution, the way a caller would use it.
    let query = std::env::args().nth(1);
    match query {
        Some(q) => {
            println!("\nresolve({q:?}):");
            match resolve(&q) {
                Ok(a) => println!("  -> {} ({})", a.name, a.pid),
                Err(e) => println!("  -> error: {e}"),
            }
            // And the pure matcher against the same list, to show the two agree.
            match match_app(&q, &apps) {
                Ok(a) => println!("  match_app -> {}", a.name),
                Err(e) => println!("  match_app -> {e}"),
            }
        }
        None => {
            // Exercise the ambiguous path against whatever is actually running, so a
            // real collision is visible rather than hypothetical.
            for probe in ["e", "co", "a"] {
                match match_app(probe, &apps) {
                    Ok(a) => println!("\nresolve({probe:?}) -> {}", a.name),
                    Err(e) => println!("\nresolve({probe:?}) -> {e}"),
                }
            }
        }
    }
}
