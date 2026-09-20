//! What the voice hook decides. No I/O, no process spawning, no clock - so the decision
//! is testable on any platform, including CI, while the acting half stays on macOS.
//!
//! # The problem this solves
//!
//! CoCo Voice's `paste_method = "external_script"` **replaces typing**: every transcript
//! arrives at our script as `argv[1]` instead of being pasted into the focused field. So
//! the hook has to answer one question per utterance, in about a second:
//!
//! ```text
//! "Emma, open a new tab in Chrome"   -> a command: hand it to Jev
//! "Dear team, following up on ..."   -> dictation: type it into the field
//! ```
//!
//! Getting this wrong is not symmetric. A command typed into a field is visible and
//! harmless; dictation handed to Jev disappears into an agent that may act on it. So the
//! default mode is [`Mode::Prefix`], which is exact rather than clever: without the
//! trigger word there is no command, and every utterance is dictation. [`Mode::Classify`]
//! exists for people who want to skip the trigger, and it is honest about being a
//! heuristic - see [`decide`].
//!
//! # Why not a model call
//!
//! The hook runs **synchronously on the paste path**: CoCo Voice waits for this process
//! to exit before it finishes handling the utterance. A model call is 250-900 ms on a
//! good day (docs/memory.md, "Numbers worth keeping") and would put a network round trip
//! between speaking and seeing text. [`decide`] is a string match, which is microseconds.

/// How an utterance is routed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A command only when the trigger word opens the utterance. Exact, so it cannot eat
    /// dictation. **The default.**
    Prefix,
    /// The trigger opens a command, and otherwise an imperative opening verb is treated
    /// as a command too. Convenient, and a heuristic.
    Classify,
    /// Everything is typed. For a session where nothing should ever reach Jev.
    Dictation,
    /// Everything is a command.
    Command,
}

impl Mode {
    /// The mode used when nothing says otherwise.
    pub const DEFAULT: Self = Self::Prefix;

    /// Parse a mode name, as accepted on the command line or in the wrapper script.
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
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Prefix => "prefix",
            Self::Classify => "classify",
            Self::Dictation => "dictation",
            Self::Command => "command",
        };
        f.write_str(s)
    }
}

/// The word that turns an utterance into a command in [`Mode::Prefix`].
///
/// "emma" because that is the name of the voice front end in `docs/prd.md`, so the
/// spoken form and the thing doing the work are the same word.
pub const DEFAULT_TRIGGER: &str = "emma";

/// Verb that opens an imperative utterance in [`Mode::Classify`].
///
/// Closed set, and deliberately short. Every verb here is something Jev is asked to do
/// through a UI; anything not listed falls through to dictation, which is the safe side
/// of the asymmetry described in the module docs.
const IMPERATIVE_VERBS: &[&str] = &[
    "add", "click", "close", "copy", "create", "delete", "drag", "find", "focus", "go", "hide",
    "launch", "maximize", "minimize", "move", "mute", "open", "paste", "pause", "play", "press",
    "quit", "resize", "run", "save", "scroll", "search", "select", "send", "set", "show", "start",
    "stop", "switch", "take", "tap", "turn", "unmute",
];

/// A long utterance is prose, however it opens.
///
/// "open the door for a second" is dictation; "open a new tab and search for flights to
/// Delhi next month" is a command. Word count separates them without a model.
const MAX_COMMAND_WORDS: usize = 12;

/// What to do with one utterance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Hand this to Jev.
    Command(String),
    /// Type this into the focused field.
    Dictation(String),
    /// Do nothing: there was no speech, or the trigger stood alone.
    Ignore,
}

/// Decide how to route one utterance.
///
/// `text` is the transcript exactly as it arrived, `trigger` is the word that opens a
/// command. Matching is case-insensitive and ignores punctuation after the trigger, so
/// "Emma, open a new tab" and "emma open a new tab" are the same command.
///
/// # In [`Mode::Classify`]
///
/// The trigger still works, and when it is absent, an utterance is a command if it opens
/// with a verb from the closed imperative list (`IMPERATIVE_VERBS` in the source), is at
/// most 12 words, and is not a question. That last test matters:
/// moment it is spoken, not a command to open something. The rule is still a heuristic -
/// "send it now" is three words opening with a verb and it is probably dictation - which
/// is exactly why it is not the default.
#[must_use]
pub fn decide(text: &str, mode: Mode, trigger: &str) -> Intent {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Intent::Ignore;
    }

    match mode {
        Mode::Dictation => Intent::Dictation(trimmed.to_string()),
        Mode::Command => Intent::Command(trimmed.to_string()),
        Mode::Prefix | Mode::Classify => {
            if let Some(rest) = strip_trigger(trimmed, trigger) {
                let rest = rest.trim();
                // A bare "Emma" is a mis-fire or a hesitation, not a goal.
                return if rest.is_empty() {
                    Intent::Ignore
                } else {
                    Intent::Command(rest.to_string())
                };
            }
            if mode == Mode::Classify && looks_imperative(trimmed) {
                Intent::Command(trimmed.to_string())
            } else {
                Intent::Dictation(trimmed.to_string())
            }
        }
    }
}

/// Remove a leading trigger word, if the utterance opens with one.
///
/// Returns what follows the trigger, so "Emma, open a tab" yields "open a tab". An
/// optional "hey" is allowed in front, because that is how a wake word is said out loud.
fn strip_trigger<'a>(text: &'a str, trigger: &str) -> Option<&'a str> {
    let trigger = trigger.trim();
    if trigger.is_empty() {
        return None;
    }

    let first = bare(nth_word(text, 0)?);
    if first.eq_ignore_ascii_case(trigger) {
        return Some(rest_from_word(text, 1));
    }
    if first.eq_ignore_ascii_case("hey") && bare(nth_word(text, 1)?).eq_ignore_ascii_case(trigger) {
        return Some(rest_from_word(text, 2));
    }
    None
}

/// Everything from the `n`th word onwards, or an empty slice when there is no such word.
fn rest_from_word(text: &str, n: usize) -> &str {
    match word_start(text, n) {
        Some(i) => &text[i..],
        None => &text[text.len()..],
    }
}

/// The `n`th whitespace-delimited word, punctuation included.
fn nth_word(text: &str, n: usize) -> Option<&str> {
    text.split_whitespace().nth(n)
}

/// The byte index where the `n`th word starts.
fn word_start(text: &str, n: usize) -> Option<usize> {
    let mut seen = 0;
    let mut in_word = false;
    for (i, ch) in text.char_indices() {
        if ch.is_whitespace() {
            in_word = false;
        } else if !in_word {
            if seen == n {
                return Some(i);
            }
            seen += 1;
            in_word = true;
        }
    }
    None
}

/// A word with the punctuation the transcript attached to it removed.
///
/// "Emma," and "EMMA!" are both the trigger. Speech-to-text adds punctuation freely, so
/// comparing raw tokens would make the trigger miss about as often as it hit.
fn bare(word: &str) -> &str {
    word.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Does this open like an imperative, and stay short enough to be one?
fn looks_imperative(text: &str) -> bool {
    if text.trim_end().ends_with('?') {
        return false;
    }
    if text.split_whitespace().count() > MAX_COMMAND_WORDS {
        return false;
    }
    let Some(first) = text.split_whitespace().next() else {
        return false;
    };
    let word = first
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();
    IMPERATIVE_VERBS.contains(&word.as_str())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn d(text: &str) -> Intent {
        decide(text, Mode::Prefix, DEFAULT_TRIGGER)
    }

    fn c(text: &str) -> Intent {
        decide(text, Mode::Classify, DEFAULT_TRIGGER)
    }

    // ── shared behaviour ───────────────────────────────────────────────────

    #[test]
    fn nothing_spoken_is_not_a_decision() {
        // Silence, or the trigger on its own, must not become a goal for Jev.
        assert_eq!(d(""), Intent::Ignore);
        assert_eq!(d("   \n"), Intent::Ignore);
        assert_eq!(d("Emma"), Intent::Ignore);
        assert_eq!(d("emma,"), Intent::Ignore);
        assert_eq!(d("hey emma"), Intent::Ignore);
    }

    #[test]
    fn every_mode_leaves_a_non_empty_utterance_somewhere() {
        for mode in [Mode::Prefix, Mode::Classify, Mode::Dictation, Mode::Command] {
            for text in ["hello", "Emma do it", "open a tab"] {
                assert_ne!(
                    decide(text, mode, DEFAULT_TRIGGER),
                    Intent::Ignore,
                    "{mode} dropped {text:?}"
                );
            }
        }
    }

    // ── prefix mode: the default ───────────────────────────────────────────

    #[test]
    fn the_trigger_opens_a_command() {
        assert_eq!(
            d("Emma open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
        assert_eq!(
            d("emma open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
        assert_eq!(
            d("Emma, open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
        assert_eq!(
            d("EMMA! open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
        assert_eq!(
            d("hey Emma, open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
    }

    #[test]
    fn the_trigger_only_counts_at_the_start() {
        // Saying the name mid-sentence is not a wake word.
        assert_eq!(
            d("tell emma I said hello"),
            Intent::Dictation("tell emma I said hello".to_string())
        );
        assert_eq!(
            d("I asked Emma about it"),
            Intent::Dictation("I asked Emma about it".to_string())
        );
    }

    #[test]
    fn a_word_that_merely_starts_like_the_trigger_is_not_the_trigger() {
        assert_eq!(
            d("emmy open a tab"),
            Intent::Dictation("emmy open a tab".to_string())
        );
        assert_eq!(
            d("emmaline open a tab"),
            Intent::Dictation("emmaline open a tab".to_string())
        );
    }

    #[test]
    fn a_custom_trigger_is_honoured() {
        assert_eq!(
            decide("Jev open Safari", Mode::Prefix, "jev"),
            Intent::Command("open Safari".to_string())
        );
        // ...and the default word stops being special when it is not the trigger.
        assert_eq!(
            decide("Emma open Safari", Mode::Prefix, "jev"),
            Intent::Dictation("Emma open Safari".to_string())
        );
    }

    // ── the three floor modes ──────────────────────────────────────────────

    #[test]
    fn dictation_mode_types_everything_including_the_trigger() {
        assert_eq!(
            decide("Emma open a tab", Mode::Dictation, DEFAULT_TRIGGER),
            Intent::Dictation("Emma open a tab".to_string())
        );
    }

    #[test]
    fn command_mode_sends_everything_to_jev() {
        assert_eq!(
            decide("Dear team", Mode::Command, DEFAULT_TRIGGER),
            Intent::Command("Dear team".to_string())
        );
        // The trigger is not needed, and is not left in the goal.
        assert_eq!(
            decide("Emma open a tab", Mode::Command, DEFAULT_TRIGGER),
            Intent::Command("Emma open a tab".to_string())
        );
    }

    // ── classify mode, and its honest limits ───────────────────────────────

    #[test]
    fn classify_sends_an_imperative_to_jev_without_the_trigger() {
        assert_eq!(
            c("open a new tab"),
            Intent::Command("open a new tab".to_string())
        );
        assert_eq!(
            c("search Google Flights for flights to Delhi"),
            Intent::Command("search Google Flights for flights to Delhi".to_string())
        );
        assert_eq!(
            c("click the third row"),
            Intent::Command("click the third row".to_string())
        );
    }

    #[test]
    fn classify_leaves_prose_alone() {
        assert_eq!(
            c("Dear team, following up on the invoice"),
            Intent::Dictation("Dear team, following up on the invoice".to_string())
        );
        assert_eq!(
            c("the build finished at six"),
            Intent::Dictation("the build finished at six".to_string())
        );
    }

    #[test]
    fn a_question_is_never_a_command() {
        // "what did you open?" opens with a verb only if you squint at the wrong word;
        // "open?" does not. Questions are speech, and speech is dictated.
        assert_eq!(
            c("show me what happened?"),
            Intent::Dictation("show me what happened?".to_string())
        );
        assert_eq!(
            c("find that file?"),
            Intent::Dictation("find that file?".to_string())
        );
    }

    #[test]
    fn a_long_imperative_is_prose() {
        let long = "open the paragraph with the number three and then keep reading aloud \
                    because this is dictation and not a goal at all";
        assert!(long.split_whitespace().count() > MAX_COMMAND_WORDS);
        assert_eq!(c(long), Intent::Dictation(long.to_string()));
    }

    #[test]
    fn classify_can_be_fooled_and_that_is_why_it_is_not_the_default() {
        // Three words, opens with a verb, and it is dictation. The rule cannot tell, so
        // this test records the known cost instead of implying the rule is exact.
        assert_eq!(c("send it now"), Intent::Command("send it now".to_string()));
    }

    #[test]
    fn classify_still_honours_the_trigger_with_a_long_or_questioning_goal() {
        // The trigger overrides every heuristic: it is an explicit instruction.
        let long = "Emma open the file and then find the paragraph mentioning the invoice \
                    and replace every occurrence of the old name with the new one";
        assert!(long.split_whitespace().count() > MAX_COMMAND_WORDS);
        assert!(matches!(c(long), Intent::Command(_)));
        assert!(matches!(c("Emma, should we open it?"), Intent::Command(_)));
    }

    #[test]
    fn mode_names_round_trip() {
        for mode in [Mode::Prefix, Mode::Classify, Mode::Dictation, Mode::Command] {
            assert_eq!(Mode::parse(&mode.to_string()), Some(mode));
        }
        assert_eq!(Mode::parse(" CLASSIFY "), Some(Mode::Classify));
        assert_eq!(Mode::parse("clever"), None);
        assert_eq!(Mode::DEFAULT, Mode::Prefix);
    }
}
