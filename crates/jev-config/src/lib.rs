//! One settings file for everything, and the rules for reading it.
//!
//! # Why this exists
//!
//! Configuration used to be spread over a wrapper script, four environment variables and
//! a separate JSON file for one of the tools. That works for the person who wrote it and
//! nobody else: there was no place to look, no way to see what was in effect, and no way
//! to change a value without knowing which of the three mechanisms owned it.
//!
//! So: **one file**, `~/.config/jev-use/settings.json`, one command to read and write it,
//! and a defined precedence for everything:
//!
//! ```text
//! built-in default   <   settings.json   <   environment variable   <   command-line flag
//! ```
//!
//! Later wins. Every layer can be seen with `jev-config show --sources`, so "why is it
//! doing that" has an answer that does not require reading source.
//!
//! # Editing it
//!
//! By hand, or with `jev-config set voice.trigger computer`. Both are first-class: the
//! file is meant to be readable and the command is meant to be safe. Nothing here writes
//! a value it cannot read back, and every write is validated first.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file this module reads and writes when nothing says otherwise.
pub const SETTINGS_ENV: &str = "JEV_SETTINGS_FILE";

/// The wake word used when nothing is configured.
///
/// "emma" because that is the name of the voice front end in `docs/prd.md`, so the
/// spoken form and the thing doing the work are the same word.
pub const DEFAULT_TRIGGER: &str = "emma";

/// Environment variables that override individual settings, so scripts and shells can
/// still win without rewriting the file.
pub const ENV_KEYS: &[(&str, &str)] = &[
    ("voice.mode", "JEV_VOICE_MODE"),
    ("voice.trigger", "JEV_VOICE_TRIGGER"),
    ("voice.hook_bin", "JEV_VOICE_HOOK_BIN"),
    ("voice.driver", "JEV_USE_BIN"),
    ("voice.log", "JEV_VOICE_LOG"),
    ("usage.db", "JEV_USAGE_DB"),
    ("usage.keys_file", "JEV_KEYS_FILE"),
    ("usage.auth_file", "JEV_AUTH_FILE"),
    ("usage.state", "JEV_USAGE_STATE"),
];

/// How an utterance is routed. Lives here rather than in the voice crate so that
/// validating a settings file does not require the code that acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// A command only when the trigger word opens the utterance. Exact.
    #[default]
    Prefix,
    /// The trigger, or an imperative opening verb. Convenient, and a heuristic.
    Classify,
    /// Everything is typed.
    Dictation,
    /// Everything is a command.
    Command,
}

impl Mode {
    /// Every valid value, for a menu or a completion.
    pub const ALL: [Self; 4] = [Self::Prefix, Self::Classify, Self::Dictation, Self::Command];

    /// Parse a value, as typed on a command line.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "prefix" => Some(Self::Prefix),
            "classify" => Some(Self::Classify),
            "dictation" => Some(Self::Dictation),
            "command" => Some(Self::Command),
            _ => None,
        }
    }

    /// One line a human can act on, for the menu.
    #[must_use]
    pub fn describe(self) -> &'static str {
        match self {
            Self::Prefix => "only \"<trigger> ...\" runs; everything else is typed",
            Self::Classify => "the trigger, or an opening verb like \"open\"; a heuristic",
            Self::Dictation => "never hand anything over; type everything",
            Self::Command => "hand everything over",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Prefix => "prefix",
            Self::Classify => "classify",
            Self::Dictation => "dictation",
            Self::Command => "command",
        };
        f.write_str(s)
    }
}

/// Everything the voice hook needs to decide and act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Voice {
    /// How an utterance is routed.
    pub mode: Mode,
    /// The word that turns an utterance into a command.
    pub trigger: String,
    /// The hook binary CoCo Voice runs. `~` is expanded.
    pub hook_bin: PathBuf,
    /// What a command is handed to. A bare name is looked up on `PATH`.
    pub driver: PathBuf,
    /// Where the hook writes its log.
    pub log: PathBuf,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            trigger: DEFAULT_TRIGGER.to_string(),
            hook_bin: PathBuf::from("~/.local/bin/jev-voice-hook"),
            driver: PathBuf::from("jev-use"),
            log: PathBuf::from("~/Library/Logs/jev-voice-hook.log"),
        }
    }
}

/// A wallet to probe. Keys are never stored here - only the name of the thing that has
/// one, which is the whole point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    /// Shown in the report.
    pub name: String,
    /// Balance endpoint.
    pub url: String,
    /// Environment variable holding the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_env: Option<String>,
    /// Entry in the auth file holding the key, for providers authenticated that way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_file_key: Option<String>,
    /// Field (or candidate fields) holding the balance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_field: Option<FieldList>,
    /// The API returns a granted/used pair rather than a single balance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub credits: bool,
    /// Currency label.
    #[serde(default = "usd")]
    pub unit: String,
    /// One wallet behind several keys: reported once, not once per key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Free text for the report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn usd() -> String {
    "USD".to_string()
}

/// A field name, or several candidates tried in order (vendors disagree).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldList {
    /// One field.
    One(String),
    /// Candidates, first present wins.
    Many(Vec<String>),
}

impl FieldList {
    /// The candidates, in order.
    #[must_use]
    pub fn candidates(&self) -> Vec<&str> {
        match self {
            Self::One(s) => vec![s.as_str()],
            Self::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// A LiteLLM-compatible gateway, if there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gateway {
    /// Base URL, without a trailing slash.
    pub base_url: String,
    /// Environment variable holding the master key.
    pub key_env: String,
    /// Model ids to probe.
    #[serde(default)]
    pub models: Vec<String>,
}

/// Where usage-check reads its inputs, and what it probes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Usage {
    /// Turn log to count tokens from.
    pub db: PathBuf,
    /// Env-style file holding provider keys.
    pub keys_file: PathBuf,
    /// Auth file holding OAuth-style provider keys.
    pub auth_file: PathBuf,
    /// Wallet snapshots, for spend deltas.
    pub state: PathBuf,
    /// Wallets to probe. Empty means probe nothing, and say so.
    pub providers: Vec<Provider>,
    /// Optional gateway health probe.
    pub gateway: Option<Gateway>,
}

impl Default for Usage {
    fn default() -> Self {
        Self {
            db: PathBuf::from("~/.pi-desktop/pi.sqlite"),
            keys_file: PathBuf::from("~/.secrets/ai-keys.env"),
            auth_file: PathBuf::from("~/.pi/agent/auth.json"),
            state: PathBuf::from("~/.pi/agent/usage-history.json"),
            providers: Vec::new(),
            gateway: None,
        }
    }
}

/// The whole settings file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Schema version, so a future change can migrate instead of guess.
    pub version: u32,
    /// The voice hook.
    pub voice: Voice,
    /// The usage reporter.
    pub usage: Usage,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            voice: Voice::default(),
            usage: Usage::default(),
        }
    }
}

/// Where a value came from. Printed by `show --sources`, because "why is it doing that"
/// should not require reading source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Not set anywhere: the built-in default is in effect.
    Default,
    /// Set in the settings file.
    File,
    /// Set by an environment variable in this process.
    Env,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Default => "default",
            Self::File => "settings.json",
            Self::Env => "environment",
        })
    }
}

/// Where the settings file is.
///
/// `JEV_SETTINGS_FILE` wins, then `$XDG_CONFIG_HOME/jev-use/`, then `~/.config/jev-use/`.
#[must_use]
pub fn settings_path() -> PathBuf {
    if let Ok(explicit) = std::env::var(SETTINGS_ENV) {
        if !explicit.trim().is_empty() {
            return expand_tilde(Path::new(&explicit));
        }
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map_or_else(|| home().join(".config"), PathBuf::from);
    base.join("jev-use").join("settings.json")
}

/// The user's home directory, as far as this process can tell.
#[must_use]
pub fn home() -> PathBuf {
    std::env::var("HOME").map_or_else(|_| PathBuf::from("/tmp"), PathBuf::from)
}

/// Expand a leading `~` to `$HOME`. Any other `~` is left alone, because only a leading
/// one is a path to somebody's home.
#[must_use]
pub fn expand_tilde(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let rest = s
        .strip_prefix("~/")
        .or_else(|| if s == "~" { Some("") } else { None });
    match rest {
        Some(rest) => home().join(rest),
        None => p.to_path_buf(),
    }
}

/// Expand `~` in every path in the file. Called once, on load, so nothing downstream has
/// to remember to.
#[must_use]
pub fn expand(mut s: Settings) -> Settings {
    s.voice.hook_bin = expand_tilde(&s.voice.hook_bin);
    s.voice.log = expand_tilde(&s.voice.log);
    if s.voice.driver.components().count() > 1 {
        s.voice.driver = expand_tilde(&s.voice.driver);
    }
    s.usage.db = expand_tilde(&s.usage.db);
    s.usage.keys_file = expand_tilde(&s.usage.keys_file);
    s.usage.auth_file = expand_tilde(&s.usage.auth_file);
    s.usage.state = expand_tilde(&s.usage.state);
    s
}

/// Why a settings file could not be used.
#[derive(Debug)]
pub enum LoadError {
    /// The file exists and could not be read.
    Io(std::io::Error),
    /// The file exists and is not valid settings.
    Parse(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Parse(e) => write!(f, "invalid JSON: {e}"),
        }
    }
}

impl Settings {
    /// Read the file at `path`. A missing file is not an error: it means defaults.
    ///
    /// A file that exists but does not parse **is** an error, loudly. Silently falling
    /// back to defaults would mean a typo in a settings file looks like settings that
    /// were ignored, which is the kind of bug that costs an afternoon.
    pub fn load_from(path: &Path) -> Result<Self, LoadError> {
        match std::fs::read_to_string(path) {
            // Deliberately NOT expanded: what is on disk is what a write must put back. A
            // settings file that rewrites "~" as /Users/someone stops being portable the
            // first time a setting is changed. [`Settings::effective`] does the expansion,
            // and only for callers that act.
            Ok(text) => {
                serde_json::from_str::<Self>(&text).map_err(|e| LoadError::Parse(e.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(LoadError::Io(e)),
        }
    }

    /// Read the file `settings_path` points at.
    pub fn load() -> Result<Self, LoadError> {
        Self::load_from(&settings_path())
    }

    /// Fold in `~` expansion and the environment. This is what anything that *acts* on
    /// the settings wants; writing never does it, so the file stays as the user wrote it.
    #[must_use]
    pub fn effective(self) -> Self {
        expand(self).with_env()
    }

    /// Write the file, creating its directory. Pretty, because a human reads it.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        text.push('\n');
        std::fs::write(path, text)
    }

    /// Every setting that can be addressed by name, in a stable order.
    #[must_use]
    pub fn keys() -> Vec<&'static str> {
        vec![
            "voice.mode",
            "voice.trigger",
            "voice.hook_bin",
            "voice.driver",
            "voice.log",
            "usage.db",
            "usage.keys_file",
            "usage.auth_file",
            "usage.state",
        ]
    }

    /// The value of a dotted key, as text.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        let s = match key {
            "voice.mode" => self.voice.mode.to_string(),
            "voice.trigger" => self.voice.trigger.clone(),
            "voice.hook_bin" => self.voice.hook_bin.display().to_string(),
            "voice.driver" => self.voice.driver.display().to_string(),
            "voice.log" => self.voice.log.display().to_string(),
            "usage.db" => self.usage.db.display().to_string(),
            "usage.keys_file" => self.usage.keys_file.display().to_string(),
            "usage.auth_file" => self.usage.auth_file.display().to_string(),
            "usage.state" => self.usage.state.display().to_string(),
            _ => return None,
        };
        Some(s)
    }

    /// Set a dotted key, validating the value. Nothing is written unless this returns
    /// `Ok`, so a bad value can never reach the file.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let v = value.trim();
        match key {
            "voice.mode" => {
                self.voice.mode = Mode::parse(v).ok_or_else(|| {
                    let all: Vec<String> = Mode::ALL.iter().map(ToString::to_string).collect();
                    format!("mode must be one of: {}", all.join(", "))
                })?;
            }
            "voice.trigger" => {
                if v.is_empty() {
                    return Err(
                        "the trigger cannot be empty (use mode=dictation to disable commands)"
                            .into(),
                    );
                }
                if v.split_whitespace().count() != 1 {
                    return Err("the trigger must be a single word".into());
                }
                self.voice.trigger = v.to_string();
            }
            "voice.hook_bin" => self.voice.hook_bin = PathBuf::from(v),
            "voice.driver" => self.voice.driver = PathBuf::from(v),
            "voice.log" => self.voice.log = PathBuf::from(v),
            "usage.db" => self.usage.db = PathBuf::from(v),
            "usage.keys_file" => self.usage.keys_file = PathBuf::from(v),
            "usage.auth_file" => self.usage.auth_file = PathBuf::from(v),
            "usage.state" => self.usage.state = PathBuf::from(v),
            other => return Err(format!("unknown setting {other:?} (try `jev-config keys`)")),
        }
        Ok(())
    }

    /// Where each key's value comes from in this process.
    #[must_use]
    pub fn sources(&self, file: &Settings) -> BTreeMap<&'static str, Source> {
        let defaults = Settings::default();
        let mut out = BTreeMap::new();
        for key in Self::keys() {
            let from_env = ENV_KEYS
                .iter()
                .find(|(k, _)| *k == key)
                .and_then(|(_, var)| std::env::var(var).ok())
                .filter(|v| !v.trim().is_empty());
            let source = if from_env.is_some() {
                Source::Env
            } else if file.get(key) != defaults.get(key) {
                Source::File
            } else {
                Source::Default
            };
            out.insert(key, source);
        }
        out
    }

    /// Fold the environment into these settings. Called by anything that acts.
    #[must_use]
    pub fn with_env(mut self) -> Self {
        for (key, var) in ENV_KEYS {
            if let Ok(v) = std::env::var(var) {
                if !v.trim().is_empty() {
                    let _ = self.set(key, &v);
                }
            }
        }
        self
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_file_is_defaults_not_an_error() {
        let s = Settings::load_from(Path::new("/nonexistent/settings.json")).unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.voice.mode, Mode::Prefix);
        assert_eq!(s.voice.trigger, "emma");
    }

    #[test]
    fn a_file_may_set_one_field_and_inherit_the_rest() {
        // The whole point of the merge: a settings file is a diff from the defaults,
        // not a document you must complete.
        let dir = std::env::temp_dir().join(format!("jevcfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("partial.json");
        std::fs::write(&p, r#"{"voice":{"trigger":"computer"}}"#).unwrap();
        let s = Settings::load_from(&p).unwrap();
        assert_eq!(s.voice.trigger, "computer");
        assert_eq!(s.voice.mode, Mode::Prefix, "unset fields keep the default");
        assert_eq!(s.usage.providers.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_file_is_loud() {
        // Falling back to defaults here would make a typo look like settings that were
        // silently ignored.
        let dir = std::env::temp_dir().join(format!("jevcfg-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("broken.json");
        std::fs::write(&p, "{ this is not json").unwrap();
        assert!(matches!(Settings::load_from(&p), Err(LoadError::Parse(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_key_in_the_file_is_rejected() {
        // deny_unknown_fields: a misspelled key is a typo, not a feature. Better to fail
        // than to leave the user wondering why their setting did nothing.
        let dir = std::env::temp_dir().join(format!("jevcfg-unk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("unknown.json");
        std::fs::write(&p, r#"{"voice":{"triger":"emma"}}"#).unwrap();
        assert!(matches!(Settings::load_from(&p), Err(LoadError::Parse(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tilde_expands_only_at_the_front() {
        let h = home();
        assert_eq!(expand_tilde(Path::new("~/.config/x")), h.join(".config/x"));
        assert_eq!(expand_tilde(Path::new("~")), h);
        // A ~ in the middle is part of the name, not a home directory.
        assert_eq!(
            expand_tilde(Path::new("/opt/a~b/c")),
            PathBuf::from("/opt/a~b/c")
        );
    }

    #[test]
    fn a_bare_driver_name_is_left_alone_so_path_lookup_still_works() {
        let s = expand(Settings {
            voice: Voice {
                driver: PathBuf::from("jev-use"),
                ..Voice::default()
            },
            ..Settings::default()
        });
        assert_eq!(s.voice.driver, PathBuf::from("jev-use"));

        let s = expand(Settings {
            voice: Voice {
                driver: PathBuf::from("~/bin/jev-use"),
                ..Voice::default()
            },
            ..Settings::default()
        });
        assert!(s.voice.driver.is_absolute());
    }

    #[test]
    fn every_key_can_be_read_back_after_being_set() {
        let mut s = Settings::default();
        for key in Settings::keys() {
            let value = match key {
                "voice.mode" => "classify",
                "voice.trigger" => "computer",
                _ => "/tmp/somewhere",
            };
            s.set(key, value).expect(key);
            assert!(s.get(key).is_some(), "{key} did not read back");
        }
    }

    #[test]
    fn a_bad_value_is_refused_before_it_reaches_the_file() {
        let mut s = Settings::default();
        let before = s.clone();
        assert!(s.set("voice.mode", "clever").is_err());
        assert!(s.set("voice.trigger", "").is_err());
        assert!(s.set("voice.trigger", "two words").is_err());
        assert!(s.set("voice.nonsense", "x").is_err());
        assert_eq!(s, before, "a refused value must not change anything");
    }

    #[test]
    fn modes_round_trip_through_text() {
        for m in Mode::ALL {
            assert_eq!(Mode::parse(&m.to_string()), Some(m));
        }
        assert_eq!(Mode::parse(" PREFIX "), Some(Mode::Prefix));
        assert_eq!(Mode::parse("clever"), None);
        assert_eq!(Mode::default(), Mode::Prefix);
    }

    #[test]
    fn a_round_trip_through_the_file_keeps_everything() {
        let dir = std::env::temp_dir().join(format!("jevcfg-rt-{}", std::process::id()));
        let p = dir.join("settings.json");
        let mut s = Settings::default();
        s.set("voice.trigger", "computer").unwrap();
        s.set("voice.mode", "classify").unwrap();
        s.usage.providers.push(Provider {
            name: "Example".into(),
            url: "https://example.test/balance".into(),
            key_env: Some("EXAMPLE_KEY".into()),
            auth_file_key: None,
            balance_field: Some(FieldList::Many(vec!["balance".into(), "remaining".into()])),
            credits: false,
            unit: "USD".into(),
            group: Some("example".into()),
            note: None,
        });
        s.usage.gateway = Some(Gateway {
            base_url: "http://127.0.0.1:4010".into(),
            key_env: "GATEWAY_KEY".into(),
            models: vec!["a".into()],
        });
        s.save_to(&p).unwrap();

        let back = Settings::load_from(&p).unwrap();
        assert_eq!(back, s, "a saved file must load back identically");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn balance_candidates_keep_their_order() {
        let one = FieldList::One("balance".into());
        assert_eq!(one.candidates(), vec!["balance"]);
        let many = FieldList::Many(vec!["balance".into(), "remaining".into()]);
        assert_eq!(many.candidates(), vec!["balance", "remaining"]);
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let mut s = Settings::default();
        s.set("voice.trigger", "from_file").unwrap();
        // Set through the same path the binary uses, without touching global env in a way
        // other tests depend on: the mapping is what is under test here.
        assert_eq!(s.get("voice.trigger").unwrap(), "from_file");
        s.set("voice.trigger", "from_env").unwrap();
        assert_eq!(s.get("voice.trigger").unwrap(), "from_env");
    }
}
