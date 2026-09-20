//! `jev-config` - the settings menu, and the four commands scripts need.
//!
//! Run with no arguments it is a menu: numbered rows, current value, one key to change.
//! Run with a subcommand it is pipeable, for an install script or an agent.
//!
//! ```text
//! jev-config                 the menu (or `show` when not attached to a terminal)
//! jev-config show [--sources] where every value came from
//! jev-config get <key>       one value, for a script
//! jev-config set <key> <v>   validated write; refuses anything it cannot read back
//! jev-config keys            every settable key
//! jev-config path            which file is in effect
//! jev-config init            write the file with defaults, if it is missing
//! jev-config edit            open it in $EDITOR
//! jev-config doctor          check the whole chain, and say what is missing
//! ```
//!
//! Exit codes: 0 fine, 1 something needs attention, 2 the check itself failed.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use jev_config::{settings_path, Mode, Settings};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        None => {
            if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
                menu()
            } else {
                // Piped or non-interactive: never block waiting for a keypress that will
                // not come. Print the state instead.
                show(false)
            }
        }
        Some("show") => show(args.iter().any(|a| a == "--sources")),
        Some("get") => get(args.get(1)),
        Some("set") => set(args.get(1), args.get(2)),
        Some("keys") => keys(),
        Some("path") => {
            println!("{}", settings_path().display());
            0
        }
        Some("init") => init(),
        Some("edit") => edit(),
        Some("doctor") => doctor(),
        Some("-h" | "--help" | "help") => {
            print_help();
            0
        }
        Some(other) => {
            eprintln!("jev-config: unknown command {other:?} (try `jev-config --help`)");
            2
        }
    };
    std::process::exit(code);
}

/// Load, reporting a broken file the way it deserves: loudly, and as a failure.
fn load() -> Settings {
    match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("jev-config: {}: {e}", settings_path().display());
            eprintln!("           fix the file, or move it aside to start from defaults");
            std::process::exit(2);
        }
    }
}

fn show(sources: bool) -> i32 {
    let path = settings_path();
    let file = load();
    let effective = file.clone().effective();

    println!("jev-config");
    println!("  file        {}", path.display());
    if !path.exists() {
        println!("              (does not exist yet - defaults are in effect, `init` writes it)");
    }
    println!();

    let source_of = |key: &str| -> String {
        if !sources {
            return String::new();
        }
        let mut map = effective.sources(&file);
        let s = map.remove(key).unwrap_or(jev_config::Source::Default);
        let var = jev_config::ENV_KEYS
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or("");
        match s {
            jev_config::Source::Env => format!("   <- {var}"),
            other => format!("   <- {other}"),
        }
    };

    println!("voice");
    println!(
        "  mode        {}{}",
        effective.voice.mode,
        source_of("voice.mode")
    );
    println!("              {}", effective.voice.mode.describe());
    println!(
        "  trigger     {}{}",
        effective.voice.trigger,
        source_of("voice.trigger")
    );
    println!(
        "  driver      {}{}",
        effective.voice.driver.display(),
        source_of("voice.driver")
    );
    println!(
        "  hook binary {}{}",
        effective.voice.hook_bin.display(),
        source_of("voice.hook_bin")
    );
    println!(
        "  log         {}{}",
        effective.voice.log.display(),
        source_of("voice.log")
    );
    println!();
    println!("usage");
    println!(
        "  turn log    {}{}",
        effective.usage.db.display(),
        source_of("usage.db")
    );
    println!(
        "  keys file   {}{}",
        effective.usage.keys_file.display(),
        source_of("usage.keys_file")
    );
    println!(
        "  auth file   {}{}",
        effective.usage.auth_file.display(),
        source_of("usage.auth_file")
    );
    println!(
        "  state       {}{}",
        effective.usage.state.display(),
        source_of("usage.state")
    );
    println!(
        "  providers   {}",
        if effective.usage.providers.is_empty() {
            "none configured".to_string()
        } else {
            let names: Vec<&str> = effective
                .usage
                .providers
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            names.join(", ")
        }
    );
    println!(
        "  gateway     {}",
        effective
            .usage
            .gateway
            .as_ref()
            .map_or("none configured".to_string(), |g| format!(
                "{} ({} model(s))",
                g.base_url,
                g.models.len()
            ))
    );
    0
}

fn get(key: Option<&String>) -> i32 {
    let Some(key) = key else {
        eprintln!("usage: jev-config get <key>   (see `jev-config keys`)");
        return 2;
    };
    let s = load().effective();
    match s.get(key) {
        Some(v) => {
            println!("{v}");
            0
        }
        None => {
            eprintln!("jev-config: unknown key {key:?} (see `jev-config keys`)");
            2
        }
    }
}

fn set(key: Option<&String>, value: Option<&String>) -> i32 {
    let (Some(key), Some(value)) = (key, value) else {
        eprintln!("usage: jev-config set <key> <value>   (see `jev-config keys`)");
        return 2;
    };
    let path = settings_path();
    let mut s = load();
    if let Err(e) = s.set(key, value) {
        eprintln!("jev-config: {e}");
        return 2;
    }
    match s.save_to(&path) {
        Ok(()) => {
            println!("{} = {}", key, s.get(key).unwrap_or_default());
            println!("  written to {}", path.display());
            0
        }
        Err(e) => {
            eprintln!("jev-config: cannot write {}: {e}", path.display());
            2
        }
    }
}

fn keys() -> i32 {
    for k in Settings::keys() {
        println!("{k}");
    }
    println!("usage.providers");
    println!("usage.gateway");
    0
}

fn init() -> i32 {
    let path = settings_path();
    if path.exists() {
        println!("already exists: {}", path.display());
        println!("  (`jev-config show` to read it, `jev-config edit` to change it)");
        return 0;
    }
    match Settings::default().save_to(&path) {
        Ok(()) => {
            println!("wrote {}", path.display());
            println!("  every value in it is the default; change what you need:");
            println!("    jev-config set voice.trigger computer");
            println!("  then check the install:");
            println!("    jev-config doctor");
            0
        }
        Err(e) => {
            eprintln!("jev-config: cannot write {}: {e}", path.display());
            2
        }
    }
}

fn edit() -> i32 {
    let path = settings_path();
    if !path.exists() {
        let _ = Settings::default().save_to(&path);
    }
    let editor = std::env::var("EDITOR").or_else(|_| std::env::var("VISUAL"));
    let Ok(editor) = editor else {
        println!("{}", path.display());
        eprintln!("  $EDITOR is not set, so here is the path instead");
        return 1;
    };
    match Command::new(editor).arg(&path).status() {
        Ok(st) if st.success() => 0,
        Ok(st) => {
            eprintln!("editor exited with {st}");
            1
        }
        Err(e) => {
            eprintln!("cannot run $EDITOR ({e}); the file is {}", path.display());
            1
        }
    }
}

// ── doctor ─────────────────────────────────────────────────────────────────

/// One finding: what, whether it is fine, and what to do about it if not.
enum Level {
    Ok,
    Warn,
    Fail,
}

/// Read an env-style file: `NAME=value` per line, `#` comments, blanks ignored.
///
/// The same shape `usage-check` reads, so doctor and the tool agree about whether a key
/// exists - a check that disagrees with the thing it checks is worse than no check.
fn read_env_file(p: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(p) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            Some((
                k.trim().to_string(),
                v.trim().trim_matches(['"', '\'']).to_string(),
            ))
        })
        .collect()
}

/// Is this directory on the PATH a GUI-launched app gets?
///
/// macOS gives a launched application `/usr/bin:/bin:/usr/sbin:/sbin`. A tool installed
/// into `~/.local/bin` is on the user's shell PATH and invisible to that app - which is
/// the quiet failure this check exists to catch, because everything works when tested
/// from a terminal.
fn gui_path_contains(dir: &Path) -> bool {
    const GUI_PATH: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
    GUI_PATH.iter().any(|d| Path::new(d) == dir)
}

fn doctor() -> i32 {
    let mut fails = 0;
    let mut warns = 0;
    let mut line = |level: Level, name: &str, detail: String| {
        let (tag, note) = match level {
            Level::Ok => ("ok  ", ""),
            Level::Warn => ("warn", "  (not fatal)"),
            Level::Fail => ("FAIL", ""),
        };
        match level {
            Level::Ok => {}
            Level::Warn => warns += 1,
            Level::Fail => fails += 1,
        }
        println!("  {tag}  {name:24} {detail}{note}");
    };

    println!("jev-config doctor");

    // 1. the file itself
    let path = settings_path();
    match Settings::load_from(&path) {
        Ok(_) if path.exists() => line(Level::Ok, "settings file", path.display().to_string()),
        Ok(_) => line(
            Level::Warn,
            "settings file",
            format!("{} does not exist; defaults are in effect", path.display()),
        ),
        Err(e) => {
            line(Level::Fail, "settings file", format!("{e}"));
            println!("\n  a settings file that does not parse cannot be ignored safely");
            return 2;
        }
    }

    let s = load().effective();

    // 2. the hook binary
    let hook = &s.voice.hook_bin;
    if is_executable(hook) {
        line(Level::Ok, "hook binary", hook.display().to_string());
    } else {
        line(
            Level::Fail,
            "hook binary",
            format!("{} missing or not executable", hook.display()),
        );
        println!("        fix: scripts/install-voice-hook.sh");
    }

    // 3. the driver
    let driver = &s.voice.driver;
    if driver.components().count() > 1 {
        if is_executable(driver) {
            line(Level::Ok, "driver", driver.display().to_string());
        } else {
            line(
                Level::Fail,
                "driver",
                format!("{} missing or not executable", driver.display()),
            );
        }
    } else if let Some(found) = which(driver) {
        // A bare name resolves through PATH, and PATH is exactly what changes when a GUI
        // app spawns the hook: macOS hands a launched app /usr/bin:/bin:/usr/sbin:/sbin,
        // not the login shell's PATH. So "found on PATH" here can still mean "not found"
        // in the process that actually runs it.
        if gui_path_contains(found.parent().unwrap_or(Path::new("/"))) {
            line(
                Level::Ok,
                "driver",
                format!("{} (on PATH)", driver.display()),
            );
        } else {
            line(
                Level::Warn,
                "driver",
                format!(
                    "{} resolves to {}, which is not on the PATH a GUI app gets",
                    driver.display(),
                    found.display()
                ),
            );
            println!(
                "        the hook is spawned by the voice app, not by your shell, so it may\n\
                 \x20       not find it. fix: jev-config set voice.driver {}",
                found.display()
            );
        }
    } else {
        line(
            Level::Warn,
            "driver",
            format!(
                "{} is not on PATH yet; commands will have nowhere to go",
                driver.display()
            ),
        );
        println!("        fix: jev-config set voice.driver /path/to/your/driver");
    }

    // 4. the log
    let log_dir = s.voice.log.parent().unwrap_or(Path::new("/tmp"));
    if log_dir.is_dir() {
        line(Level::Ok, "log directory", log_dir.display().to_string());
    } else {
        line(
            Level::Warn,
            "log directory",
            format!("{} does not exist", log_dir.display()),
        );
        println!("        the hook creates it on first use");
    }

    // 5. CoCo Voice, if it is on this machine
    let cov = voice_app_settings();
    match cov {
        Some(p) if p.exists() => match coco_state(&p) {
            Some((method, script)) => {
                if method == "external_script" {
                    let wanted = script.clone().unwrap_or_default();
                    if wanted.contains("voice-hook") {
                        line(
                            Level::Ok,
                            "CoCo Voice",
                            format!("paste_method=external_script -> {wanted}"),
                        );
                    } else {
                        line(
                            Level::Warn,
                            "CoCo Voice",
                            format!("external_script points at {wanted}"),
                        );
                        println!("        fix: point external_script_path at scripts/voice-hook");
                    }
                } else {
                    line(
                        Level::Warn,
                        "CoCo Voice",
                        format!("paste_method={method} (typing is still native)"),
                    );
                    println!(
                        "        to route speech through the hook: set paste_method=external_script"
                    );
                }
            }
            None => line(
                Level::Warn,
                "CoCo Voice",
                format!("{} has no paste_method", p.display()),
            ),
        },
        _ => line(
            Level::Warn,
            "CoCo Voice",
            "not installed; the hook has nothing to feed it".to_string(),
        ),
    }

    // 6. the usage side: does every configured provider have a way to get a key?
    if s.usage.providers.is_empty() {
        line(
            Level::Warn,
            "wallets",
            "none configured; `usage-check --live` will probe nothing".to_string(),
        );
    } else {
        let auth = read_json(&s.usage.auth_file);
        // Keys can live in the environment OR in the env-style file the settings name.
        // Checking only the environment would report "no key" for the common case where
        // everything is in that file.
        let keys_file = read_env_file(&s.usage.keys_file);
        let mut missing = Vec::new();
        for p in &s.usage.providers {
            let from_env = |var: &str| {
                std::env::var(var).is_ok_and(|v| !v.trim().is_empty())
                    || keys_file
                        .iter()
                        .any(|(k, v)| k == var && !v.trim().is_empty())
            };
            let has_key = match (&p.key_env, &p.auth_file_key) {
                (Some(var), _) => from_env(var),
                (_, Some(entry)) => auth
                    .as_ref()
                    .and_then(|v| v.get(entry))
                    .is_some_and(|v| !v.is_null()),
                _ => false,
            };
            if !has_key {
                missing.push(p.name.clone());
            }
        }
        if missing.is_empty() {
            line(
                Level::Ok,
                "wallets",
                format!("{} configured, all with a key", s.usage.providers.len()),
            );
        } else {
            line(
                Level::Warn,
                "wallets",
                format!("no key for: {}", missing.join(", ")),
            );
            println!(
                "        they will report \"no key\" rather than a balance, which is intended"
            );
        }
    }

    // 7. the turn log
    if s.usage.db.exists() {
        line(Level::Ok, "turn log", s.usage.db.display().to_string());
    } else {
        line(
            Level::Warn,
            "turn log",
            format!(
                "{} not found (token counts will be empty)",
                s.usage.db.display()
            ),
        );
    }

    println!();
    if fails > 0 {
        println!("  {fails} thing(s) must be fixed, {warns} warning(s)");
        return 2;
    }
    if warns > 0 {
        println!("  nothing broken, {warns} thing(s) worth a look");
        return 1;
    }
    println!("  everything in place");
    0
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn which(name: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|c| is_executable(c))
}

fn read_json(p: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// CoCo Voice's settings file, if that app is on this machine.
fn voice_app_settings() -> Option<PathBuf> {
    let home = jev_config::home();
    let p = home.join("Library/Application Support/com.cocoresearch.cocovoice/settings_store.json");
    Some(p)
}

/// The two fields that decide whether speech reaches the hook.
fn coco_state(p: &Path) -> Option<(String, Option<String>)> {
    let doc = read_json(p)?;
    let holder = doc
        .get("settings")
        .filter(|v| v.is_object())
        .unwrap_or(&doc);
    let method = holder.get("paste_method")?.as_str()?.to_string();
    let script = holder
        .get("external_script_path")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    Some((method, script))
}

// ── the menu ───────────────────────────────────────────────────────────────

/// Rows in the menu, in order. One place that defines what "everything" means.
const ROWS: &[(&str, &str)] = &[
    ("voice.mode", "how an utterance is routed"),
    ("voice.trigger", "the wake word"),
    ("voice.driver", "what a command is handed to"),
    ("voice.hook_bin", "the hook binary"),
    ("voice.log", "where the hook logs"),
    ("usage.db", "turn log to count tokens from"),
    ("usage.keys_file", "file holding provider keys"),
    ("usage.auth_file", "file holding OAuth keys"),
    ("usage.state", "wallet snapshots"),
];

fn menu() -> i32 {
    loop {
        let path = settings_path();
        let file = load();
        let s = file.clone().effective();

        println!("\njev-use settings");
        println!(
            "  {}{}",
            path.display(),
            if path.exists() {
                ""
            } else {
                "   (not created yet)"
            }
        );
        println!();
        for (i, (key, what)) in ROWS.iter().enumerate() {
            let value = s.get(key).unwrap_or_default();
            let shown = if *key == "voice.mode" {
                format!("{value:9}  {}", s.voice.mode.describe())
            } else {
                format!("{value:9}  {what}")
            };
            println!("  {:>2}  {:<17} {}", i + 1, key, shown);
        }
        println!(
            "  {:>2}  {:<17} {}",
            10,
            "usage.providers",
            if s.usage.providers.is_empty() {
                "none configured".to_string()
            } else {
                format!("{}", s.usage.providers.len())
            }
        );
        println!(
            "  {:>2}  {:<17} {}",
            11,
            "usage.gateway",
            s.usage
                .gateway
                .as_ref()
                .map_or("none configured".to_string(), |g| g.base_url.clone())
        );
        println!();
        println!("   s  show everything, with where each value came from");
        println!("   d  doctor - check the whole chain");
        println!("   i  write the file (init)");
        println!("   q  quit");
        print!("\nchange a setting by number: ");
        let _ = std::io::stdout().flush();

        let Some(input) = read_line() else { return 0 };
        match input.trim() {
            "" | "q" | "quit" | "exit" => return 0,
            "s" => {
                show(true);
            }
            "d" => {
                doctor();
            }
            "i" => {
                init();
            }
            n => match n.parse::<usize>() {
                Ok(i) if (1..=ROWS.len()).contains(&i) => {
                    change(&file, ROWS[i - 1].0, ROWS[i - 1].1);
                }
                Ok(10) => println!(
                    "\n  providers are a list, so they live in the file: {}\n  \
                     (template: scripts/usage-check.example.json)\n",
                    path.display()
                ),
                Ok(11) => println!(
                    "\n  the gateway is a nested object, so it lives in the file: {}\n",
                    path.display()
                ),
                _ => println!("  not a number I know: {n:?}"),
            },
        }
    }
}

/// Ask for one value and write it, or offer the valid values when the key has a closed
/// set - a menu offering an invalid answer is a menu that wastes a trip.
fn change(file: &Settings, key: &str, what: &str) {
    let current = file.get(key).unwrap_or_default();

    if key == "voice.mode" {
        println!("\n  {key} - {what}");
        for (i, m) in Mode::ALL.iter().enumerate() {
            let mark = if *m == file.voice.mode { "*" } else { " " };
            println!("   {mark} {}  {:<10} {}", i + 1, m, m.describe());
        }
        print!("  pick 1-4, or enter to keep {current}: ");
        let _ = std::io::stdout().flush();
        let Some(v) = read_line() else { return };
        let v = v.trim();
        if v.is_empty() {
            return;
        }
        match v.parse::<usize>().ok().and_then(|i| Mode::ALL.get(i - 1)) {
            Some(m) => write_setting(key, &m.to_string()),
            None => {
                if Mode::parse(v).is_some() {
                    write_setting(key, v);
                } else {
                    println!("  not one of the four; nothing changed");
                }
            }
        }
        return;
    }

    print!("\n  {key} - {what}\n  now {current:?}\n  new value (enter keeps it): ");
    let _ = std::io::stdout().flush();
    let Some(v) = read_line() else { return };
    let v = v.trim();
    if v.is_empty() {
        return;
    }
    write_setting(key, v);
}

fn write_setting(key: &str, value: &str) {
    let path = settings_path();
    let mut s = match Settings::load() {
        Ok(s) => s,
        Err(e) => {
            println!("  cannot read {}: {e}", path.display());
            return;
        }
    };
    if let Err(e) = s.set(key, value) {
        println!("  refused: {e}");
        return;
    }
    match s.save_to(&path) {
        Ok(()) => println!("  {key} = {}", s.get(key).unwrap_or_default()),
        Err(e) => println!("  cannot write {}: {e}", path.display()),
    }
}

/// One line from the user, or `None` at end of input.
fn read_line() -> Option<String> {
    let mut buf = String::new();
    match std::io::stdin().read_line(&mut buf) {
        Ok(0) => None,
        Ok(_) => Some(buf),
        Err(_) => None,
    }
}

fn print_help() {
    println!(
        "jev-config - one file for every setting, and the command that manages it\n\
         \n\
         usage: jev-config [command]\n\
         \n\
         commands:\n\
           (none)              the settings menu, when attached to a terminal\n\
           show [--sources]    every setting, and where each value came from\n\
           get <key>           one value\n\
           set <key> <value>   change one value (validated before it is written)\n\
           keys                the settable keys\n\
           path                which file is in effect\n\
           init                write the file with defaults, if it is missing\n\
           edit                open the file in $EDITOR\n\
           doctor              check the hook, the driver, the app and the wallets\n\
         \n\
         the file lives at $JEV_SETTINGS_FILE, else $XDG_CONFIG_HOME/jev-use/settings.json,\n\
         else ~/.config/jev-use/settings.json\n\
         \n\
         precedence: default < settings file < environment variable < command-line flag"
    );
}
