//! The voice hook. CoCo Voice calls this with one transcript as `argv[1]` and waits for
//! it to exit, so this process is on the paste path: it must decide and act in about a
//! second, and it must always exit 0. A non-zero exit makes the *app* report a paste
//! failure, which is a worse outcome than anything this hook could do.
//!
//! # What it does with each decision
//!
//! ```text
//! dictation -> put the text on the clipboard, press Cmd+V          (about 0.2 s)
//! command   -> re-spawn ourselves detached, exit immediately        (about 1 ms)
//! ignore    -> exit                                                 (about 1 ms)
//! ```
//!
//! The command path must not be taken inline: a Jev run is seconds. The app would sit
//! there waiting, and the paste would look hung. So the parent spawns a detached copy of
//! itself in `--run-command` mode, writes one line to the log, and exits. The child keeps
//! running after the app has moved on.
//!
//! # Install
//!
//! `scripts/voice-hook` is the wrapper the app is pointed at. Point
//! `external_script_path` at it, and set `paste_method = "external_script"`. Read that
//! file before installing: turning it on **replaces** typing, so dictation breaks until
//! this hook is in place.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use jev_voice::{decide, Intent, Mode, DEFAULT_TRIGGER};

/// Where Jev lives on this machine. The Python reference, not the Rust port: the port is
/// still missing its tree walk (T4), so the reference is what can actually drive a Mac.
const JEV_USE: &str = "jev-use";

/// Fallback when `$HOME` is unset, which happens when a GUI app spawns us.
const FALLBACK_HOME: &str = "/tmp";

fn main() {
    // The hook's contract is "exit 0". Anything that reaches here as an error is logged
    // and swallowed, because the alternative is breaking the user's paste.
    let code = run();
    std::process::exit(code);
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut mode = Mode::DEFAULT;
    let mut trigger = DEFAULT_TRIGGER.to_string();
    let mut jev_use = JEV_USE.to_string();
    let mut dry_run = false;
    let mut run_command = false;
    let mut transcript: Option<String> = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return 0;
            }
            "--dry-run" => dry_run = true,
            "--run-command" => run_command = true,
            "--mode" => match it.next().and_then(|s| Mode::parse(s)) {
                Some(m) => mode = m,
                None => {
                    log_line(&format!("bad --mode value for {:?}", args));
                    return 0;
                }
            },
            "--trigger" => {
                if let Some(t) = it.next() {
                    trigger = t.clone();
                }
            }
            "--jev-use" => {
                if let Some(p) = it.next() {
                    jev_use = p.clone();
                }
            }
            other => {
                // Fail closed on an unrecognised flag rather than treating it as text: a
                // typo in the wrapper would otherwise be pasted into the user's field as
                // part of their dictation, which reads as the hook mishearing them.
                if other.starts_with("--") {
                    log_line(&format!("unknown flag {other:?}; doing nothing"));
                    return 0;
                }
                // CoCo Voice passes exactly one positional: the transcript.
                transcript = Some(match transcript.take() {
                    Some(prev) => format!("{prev} {other}"),
                    None => other.to_string(),
                });
            }
        }
    }

    let Some(text) = transcript else {
        log_line("called with no transcript");
        return 0;
    };
    // The detached child's job: run Jev, log the outcome, notify, exit. Nothing else.
    if run_command {
        return run_jev(&text, &jev_use);
    }

    let intent = decide(&text, mode, &trigger);
    log_line(&format!("mode={mode} in={text:?} out={intent:?}"));

    match intent {
        Intent::Ignore => 0,
        Intent::Dictation(text) => {
            if dry_run {
                println!("dictation: would paste {} chars", text.chars().count());
                return 0;
            }
            paste(&text)
        }
        Intent::Command(goal) => {
            if dry_run {
                println!("command: would hand to Jev: {goal:?}");
                return 0;
            }
            spawn_detached(&goal, mode, &trigger, &jev_use)
        }
    }
}

/// Re-spawn ourselves, detached, to run the goal. Returns immediately.
fn spawn_detached(goal: &str, mode: Mode, trigger: &str, jev_use: &str) -> i32 {
    let Ok(me) = std::env::current_exe() else {
        log_line("cannot find our own executable; cannot background the command");
        return 0;
    };

    // stdout and stderr go to the log file, so the child is free of the app's pipes: the
    // app closes them the moment it moves on, and a write to a closed pipe would kill
    // the child with SIGPIPE partway through a Jev run.
    let log = match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        Ok(f) => f,
        Err(e) => {
            log_line(&format!("cannot open the log for the child: {e}"));
            return 0;
        }
    };
    let err = log.try_clone();

    let spawned = Command::new(me)
        .arg("--run-command")
        .arg("--mode")
        .arg(mode.to_string())
        .arg("--trigger")
        .arg(trigger)
        .arg("--jev-use")
        .arg(jev_use)
        .arg(goal)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(match err {
            Ok(e) => Stdio::from(e),
            Err(_) => Stdio::null(),
        })
        .spawn();

    match spawned {
        Ok(child) => {
            log_line(&format!("backgrounded pid {} for {goal:?}", child.id()));
            0
        }
        Err(e) => {
            log_line(&format!("cannot spawn the background run: {e}"));
            0
        }
    }
}

/// The detached half: run Jev on the goal, log what it said, notify.
fn run_jev(goal: &str, jev_use: &str) -> i32 {
    let started = SystemTime::now();
    let out = Command::new(jev_use).arg(goal).output();

    let secs = started
        .elapsed()
        .map(|d| d.as_secs_f32())
        .unwrap_or_default();

    let message = match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let tail = stdout
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("no output")
                .trim()
                .to_string();
            log_line(&format!(
                "ran {jev_use} {goal:?} exit={:?} in {secs:.1}s last_line={tail:?}",
                o.status.code()
            ));
            let head: String = tail.chars().take(120).collect();
            if o.status.success() {
                format!("done in {secs:.1}s: {head}")
            } else {
                format!("failed ({:?}): {head}", o.status.code())
            }
        }
        Err(e) => {
            log_line(&format!("cannot run {jev_use}: {e}"));
            format!("cannot run {jev_use}")
        }
    };

    notify(&message);
    0
}

/// Put the text on the clipboard and press Cmd+V.
///
/// Two shells out to system tools, both taking argv rather than a command line, so the
/// transcript is never parsed as shell code. `pbcopy` first, `osascript` second: the
/// keystroke must come after the clipboard is written, or Cmd+V pastes the previous
/// contents.
fn paste(text: &str) -> i32 {
    let mut pbcopy = match Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
    {
        Ok(p) => p,
        Err(e) => {
            log_line(&format!("pbcopy failed: {e}"));
            return 0;
        }
    };
    if let Some(stdin) = pbcopy.stdin.as_mut() {
        if let Err(e) = stdin.write_all(text.as_bytes()) {
            log_line(&format!("writing to pbcopy failed: {e}"));
        }
    }
    // Dropping stdin is what tells pbcopy the text is complete.
    drop(pbcopy.stdin.take());
    let _ = pbcopy.wait();

    let key = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .output();
    if let Err(e) = key {
        log_line(&format!("osascript failed: {e}"));
    }
    0
}

/// Post a notification. Best effort: a missing notification must not fail a run.
fn notify(message: &str) {
    let _ = Command::new("osascript")
        .arg("-e")
        .arg("on run argv")
        .arg("-e")
        .arg("display notification (item 1 of argv) with title \"Jev\"")
        .arg("-e")
        .arg("end run")
        .arg(message)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();
}

fn log_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| FALLBACK_HOME.to_string());
    Path::new(&home).join("Library/Logs/jev-voice-hook.log")
}

/// Append one line, stamped in UTC. Never fails loudly: logging is for us, not the user.
fn log_line(line: &str) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "{stamp} {line}");
    }
}

fn print_help() {
    println!(
        "jev-voice-hook - decide whether a transcript is a command or dictation\n\
         \n\
         usage: jev-voice-hook [options] <transcript>\n\
         \n\
         options:\n\
           --mode <prefix|classify|dictation|command>  default: prefix\n\
           --trigger <word>                            default: {DEFAULT_TRIGGER}\n\
           --jev-use <path>                            default: {JEV_USE}\n\
           --dry-run                                   print the decision, do nothing\n\
           --run-command                               internal: the detached Jev runner\n\
           --help\n\
         \n\
         Always exits 0: CoCo Voice treats a non-zero exit as a paste failure."
    );
}
