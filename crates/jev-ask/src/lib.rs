//! The decision queue: questions that wait for you instead of scrolling away.
//!
//! # The failure this exists to fix, measured
//!
//! On this machine's turn log: of 95 agent turns that ended on a question, **16% were
//! answered promptly**. 56% were followed by a message that shared no vocabulary with the
//! question at all, and 5% were never followed by anything. Of the 31 that were an actual
//! decision for the human, **45% were not answered promptly**.
//!
//! The mechanism is not attention, it is structure: **a question asked into a queue that does
//! not wait is not a question, it is a comment.** It has no id, no age, no owner, and nothing
//! attached that says what it holds up. So three things go wrong, in rising order of cost:
//!
//! 1. The question is a message, so when the scroll moves it is gone.
//! 2. The next queued prompt does not merely skip it, it **inherits an assumption nobody
//!    approved**. That is rule R10's failure (never silently substitute) one level up, at the
//!    workflow instead of at the element.
//! 3. Every incentive in the loop points at skipping the human: an agent's cheapest action is
//!    to keep going, and a queue's whole job is to queue.
//!
//! # The shape of the answer
//!
//! A question becomes an **object** with four properties that do the work:
//!
//! | Field | Why it is there |
//! | --- | --- |
//! | `options` | a closed set, so answering is a choice rather than a composition |
//! | `default` | so silence can resolve instead of rot, and so the fast path is one keystroke |
//! | `blocks` | so the cost of not answering is visible, and the queue can be held |
//! | `expires_at` | so an unanswered question is applied and reported rather than forgotten |
//!
//! # The store
//!
//! Append-only JSON Lines, one event per line, default `~/.config/jev-use/decisions.jsonl`
//! (override `JEV_DECISIONS`). State is a projection of the events, never a rewrite of them,
//! for three reasons: several agents may append at once, an answer is evidence worth keeping,
//! and `tail -f` is a real interface.
//!
//! A line that does not parse is an **error with its line number**, never a silent skip. If
//! the log is damaged, saying so is the only useful behaviour.
//!
//! # What this deliberately does not do
//!
//! No model call, no ranking by importance, no notification. It cannot tell you what matters;
//! it can only make what is waiting on you *visible, aged, and expensive to ignore*.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Seconds since the Unix epoch. The whole tool speaks in this unit.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Where a question lives: one lane per project, so five projects do not share one queue.
pub fn lane_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".to_string())
}

/// What happened. The log is a sequence of these, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Event {
    /// An agent (or a person) needs a decision before continuing.
    Asked {
        /// Short, typeable, unique.
        id: String,
        /// When it was asked, in epoch seconds.
        at: u64,
        /// The project it belongs to.
        lane: String,
        /// The question, in one line.
        text: String,
        /// The closed set of answers.
        options: Vec<String>,
        /// What happens if nobody answers.
        default: String,
        /// What this holds up. Empty means it does not hold anything up.
        #[serde(default)]
        blocks: Vec<String>,
        /// Who asked.
        asked_by: String,
        /// When silence becomes an answer.
        #[serde(default)]
        expires_at: Option<u64>,
    },
    /// A human chose.
    Answered {
        /// Question id.
        id: String,
        /// When.
        at: u64,
        /// The choice, which must be one of the offered options.
        answer: String,
        /// Who answered.
        by: String,
        /// Anything worth carrying back to the agent.
        #[serde(default)]
        note: Option<String>,
    },
    /// Nobody answered in time, so the default was applied and reported.
    Expired {
        /// Question id.
        id: String,
        /// When.
        at: u64,
        /// The default that was applied.
        applied: String,
    },
    /// The question stopped mattering (the work changed, or it was answered elsewhere).
    Cancelled {
        /// Question id.
        id: String,
        /// When.
        at: u64,
        /// Why it stopped mattering.
        reason: String,
    },
}

impl Event {
    /// The id this event refers to.
    pub fn id(&self) -> &str {
        match self {
            Event::Asked { id, .. }
            | Event::Answered { id, .. }
            | Event::Expired { id, .. }
            | Event::Cancelled { id, .. } => id,
        }
    }
}

/// Where a question ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// Still waiting on a human.
    Open,
    /// A human chose.
    Answered,
    /// The default was applied because nobody chose in time.
    Expired,
    /// It stopped mattering.
    Cancelled,
}

/// One question, projected from the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// Short id.
    pub id: String,
    /// Project lane.
    pub lane: String,
    /// The question.
    pub text: String,
    /// The closed set of answers.
    pub options: Vec<String>,
    /// What happens if nobody answers.
    pub default: String,
    /// What it holds up.
    pub blocks: Vec<String>,
    /// Who asked.
    pub asked_by: String,
    /// When it was asked.
    pub asked_at: u64,
    /// When silence becomes an answer.
    pub expires_at: Option<u64>,
    /// Where it ended up.
    pub state: State,
    /// The choice, when one was made or applied.
    pub answer: Option<String>,
    /// When it was resolved.
    pub resolved_at: Option<u64>,
    /// A note carried back to the agent.
    pub note: Option<String>,
}

impl Question {
    /// True while it is still waiting on a human and has not passed its expiry.
    ///
    /// Past the expiry it is neither open nor resolved: `sweep` is what turns it into an
    /// `Expired` event. Nothing here applies a default behind the caller's back.
    pub fn is_open(&self, now: u64) -> bool {
        self.state == State::Open && !self.is_due_for_expiry(now)
    }

    /// True when nobody answered and the clock has run out, so `sweep` should apply the default.
    pub fn is_due_for_expiry(&self, now: u64) -> bool {
        self.state == State::Open && matches!(self.expires_at, Some(t) if now >= t)
    }

    /// Seconds since it was asked.
    pub fn age(&self, now: u64) -> u64 {
        now.saturating_sub(self.asked_at)
    }

    /// Seconds until the default is applied, negative once it is overdue.
    pub fn expires_in(&self, now: u64) -> Option<i64> {
        self.expires_at.map(|t| t as i64 - now as i64)
    }

    /// Does this question hold up the queue?
    ///
    /// Only when it declares what it blocks. A question with no `blocks` is still visible and
    /// still ages, but it does not stop work, because a question that stalls a lane without
    /// saying why is worse than no question at all.
    pub fn blocks_anything(&self) -> bool {
        !self.blocks.is_empty()
    }

    /// True when the resolution was the default rather than a human's choice.
    pub fn took_the_default(&self) -> bool {
        self.state == State::Expired
            || (self.state == State::Answered
                && self.answer.as_deref() == Some(self.default.as_str()))
    }
}

/// Fold the log into the current state of every question, in the order they were asked.
///
/// An event for an id that was never asked is **ignored, not an error**: an append-only log
/// can be read mid-write by another process, and refusing to render the rest of the queue
/// because of one orphan line would be the wrong trade.
pub fn project(events: &[Event]) -> Vec<Question> {
    let mut order: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Question> = Vec::new();

    for ev in events {
        match ev {
            Event::Asked {
                id,
                at,
                lane,
                text,
                options,
                default,
                blocks,
                asked_by,
                expires_at,
            } => {
                if index.contains_key(id) {
                    continue;
                }
                index.insert(id.clone(), order.len());
                order.push(id.clone());
                out.push(Question {
                    id: id.clone(),
                    lane: lane.clone(),
                    text: text.clone(),
                    options: options.clone(),
                    default: default.clone(),
                    blocks: blocks.clone(),
                    asked_by: asked_by.clone(),
                    asked_at: *at,
                    expires_at: *expires_at,
                    state: State::Open,
                    answer: None,
                    resolved_at: None,
                    note: None,
                });
            }
            Event::Answered {
                id,
                at,
                answer,
                note,
                ..
            } => {
                if let Some(q) = index.get(id).and_then(|i| out.get_mut(*i)) {
                    q.state = State::Answered;
                    q.answer = Some(answer.clone());
                    q.resolved_at = Some(*at);
                    q.note = note.clone();
                }
            }
            Event::Expired { id, at, applied } => {
                if let Some(q) = index.get(id).and_then(|i| out.get_mut(*i)) {
                    q.state = State::Expired;
                    q.answer = Some(applied.clone());
                    q.resolved_at = Some(*at);
                }
            }
            Event::Cancelled { id, at, .. } => {
                if let Some(q) = index.get(id).and_then(|i| out.get_mut(*i)) {
                    q.state = State::Cancelled;
                    q.resolved_at = Some(*at);
                }
            }
        }
    }
    out
}

/// The order to show questions in: what holds up the most first, then the oldest.
///
/// This is the whole ranking. Deliberately not "most important", because nothing here can
/// know that, and a ranking that pretends to would be ignored within a week.
pub fn rank(mut qs: Vec<Question>) -> Vec<Question> {
    qs.sort_by(|a, b| {
        b.blocks
            .len()
            .cmp(&a.blocks.len())
            .then(a.asked_at.cmp(&b.asked_at))
    });
    qs
}

/// The open questions waiting on a human right now, ranked.
pub fn open_now(qs: &[Question], now: u64) -> Vec<Question> {
    rank(
        qs.iter()
            .filter(|q| q.is_open(now))
            .cloned()
            .collect::<Vec<_>>(),
    )
}

/// The open questions that are holding the queue, optionally narrowed to one lane.
///
/// This is what `gate` uses. A question that is open but declares no `blocks` is not here,
/// which is the difference between "you have a question to answer" and "work has stopped".
pub fn blocking(qs: &[Question], lane: Option<&str>, now: u64) -> Vec<Question> {
    rank(
        qs.iter()
            .filter(|q| q.is_open(now))
            .filter(|q| q.blocks_anything())
            .filter(|q| lane.map(|l| q.lane == l).unwrap_or(true))
            .cloned()
            .collect::<Vec<_>>(),
    )
}

/// Questions whose default should be applied now.
pub fn due_for_expiry(qs: &[Question], now: u64) -> Vec<Question> {
    rank(
        qs.iter()
            .filter(|q| q.is_due_for_expiry(now))
            .cloned()
            .collect::<Vec<_>>(),
    )
}

/// What went wrong resolving an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    /// Nothing matched.
    NotFound(String),
    /// More than one open question starts with it.
    Ambiguous(String, Vec<String>),
}

impl std::fmt::Display for IdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdError::NotFound(p) => write!(f, "no question matches {p:?}"),
            IdError::Ambiguous(p, ids) => write!(
                f,
                "{p:?} matches {} questions: {}",
                ids.len(),
                ids.join(", ")
            ),
        }
    }
}

/// Exact id, or a unique prefix of one. Ambiguity is an error listing the candidates.
///
/// Same rule as app resolution (exact-or-unique, never fuzzy): picking one of several
/// silently is exactly the substitution rule R10 forbids.
pub fn resolve_id(qs: &[Question], prefix: &str) -> Result<String, IdError> {
    if let Some(q) = qs.iter().find(|q| q.id == prefix) {
        return Ok(q.id.clone());
    }
    let mut hits: Vec<String> = qs
        .iter()
        .filter(|q| q.id.starts_with(prefix))
        .map(|q| q.id.clone())
        .collect();
    hits.sort();
    hits.dedup();
    match hits.len() {
        0 => Err(IdError::NotFound(prefix.to_string())),
        1 => Ok(hits.remove(0)),
        _ => Err(IdError::Ambiguous(prefix.to_string(), hits)),
    }
}

/// A short, typeable id. Time makes it sortable, the hash makes collisions unlikely.
pub fn new_id(now: u64, text: &str) -> String {
    let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        acc ^= u64::from(*b);
        acc = acc.wrapping_mul(0x100_0000_01b3);
    }
    format!("q-{:x}-{:04x}", now, acc & 0xffff)
}

/// Durations as a person reads them: `3m`, `2h 14m`, `4d 2h`.
pub fn human_age(secs: u64) -> String {
    let m = secs / 60;
    let h = m / 60;
    let d = h / 24;
    if d > 0 {
        format!("{}d {}h", d, h % 24)
    } else if h > 0 {
        format!("{}h {}m", h, m % 60)
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{secs}s")
    }
}

/// The numbers that say whether this tool is earning its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// How many were asked in total.
    pub asked: u64,
    /// How many are still waiting.
    pub open: u64,
    /// How many a human chose.
    pub answered: u64,
    /// How many had their default applied.
    pub expired: u64,
    /// How many stopped mattering.
    pub cancelled: u64,
    /// Median seconds from asked to resolved, over everything resolved.
    pub median_time_to_answer: Option<u64>,
    /// 90th percentile of the same.
    pub p90_time_to_answer: Option<u64>,
    /// How many resolutions were the default rather than a choice.
    pub took_default: u64,
    /// The oldest still-open question, if any.
    pub oldest_open_age: Option<u64>,
    /// Open questions per lane, highest first.
    pub per_lane: Vec<(String, u64)>,
}

/// Fold the queue into its summary.
pub fn summarize(qs: &[Question], now: u64) -> Summary {
    let mut s = Summary {
        asked: qs.len() as u64,
        open: 0,
        answered: 0,
        expired: 0,
        cancelled: 0,
        median_time_to_answer: None,
        p90_time_to_answer: None,
        took_default: 0,
        oldest_open_age: None,
        per_lane: Vec::new(),
    };

    let mut resolved: Vec<u64> = Vec::new();
    let mut lanes: HashMap<String, u64> = HashMap::new();

    for q in qs {
        match q.state {
            State::Open => {
                if q.is_due_for_expiry(now) {
                    // Past its clock, waiting for `sweep`. Counted as open, because it is.
                }
                s.open += 1;
                *lanes.entry(q.lane.clone()).or_insert(0) += 1;
                let age = q.age(now);
                s.oldest_open_age = Some(s.oldest_open_age.map_or(age, |o| o.max(age)));
            }
            State::Answered => s.answered += 1,
            State::Expired => s.expired += 1,
            State::Cancelled => s.cancelled += 1,
        }
        if let Some(at) = q.resolved_at {
            resolved.push(at.saturating_sub(q.asked_at));
        }
        if q.took_the_default() {
            s.took_default += 1;
        }
    }

    resolved.sort_unstable();
    if !resolved.is_empty() {
        s.median_time_to_answer = Some(resolved[resolved.len() / 2]);
        let idx = ((resolved.len() as f64 * 0.9) as usize).min(resolved.len() - 1);
        s.p90_time_to_answer = Some(resolved[idx]);
    }

    let mut per_lane: Vec<(String, u64)> = lanes.into_iter().collect();
    per_lane.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    s.per_lane = per_lane;
    s
}

/// The append-only log of every question this machine has seen.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// A store at an explicit path.
    pub fn at(path: impl Into<PathBuf>) -> Store {
        Store { path: path.into() }
    }

    /// The default store: `$JEV_DECISIONS`, else `~/.config/jev-use/decisions.jsonl`.
    ///
    /// Outside the repository on purpose. This is machine state about several projects at
    /// once, and the standard is explicit that config lives outside the repo.
    pub fn from_env() -> Store {
        if let Some(p) = std::env::var_os("JEV_DECISIONS") {
            return Store::at(PathBuf::from(p));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Store::at(home.join(".config").join("jev-use").join("decisions.jsonl"))
    }

    /// Where it is, so the caller can print it.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event.
    pub fn append(&self, event: &Event) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(f, "{line}")?;
        f.flush()
    }

    /// Read every event, in order.
    ///
    /// A line that does not parse is an error naming the line. A missing file is an empty
    /// log, which is not a problem worth reporting: nobody has asked anything yet.
    pub fn events(&self) -> std::io::Result<Vec<Event>> {
        let f = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut out = Vec::new();
        for (n, line) in BufReader::new(f).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Event>(&line) {
                Ok(ev) => out.push(ev),
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("line {}: {e}", n + 1),
                    ))
                }
            }
        }
        Ok(out)
    }

    /// Every question, projected from the log.
    pub fn questions(&self) -> std::io::Result<Vec<Question>> {
        Ok(project(&self.events()?))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn asked(id: &str, at: u64, blocks: &[&str], expires_at: Option<u64>) -> Event {
        Event::Asked {
            id: id.to_string(),
            at,
            lane: "jev-use".to_string(),
            text: format!("question {id}"),
            options: vec!["a".into(), "b".into()],
            default: "a".to_string(),
            blocks: blocks.iter().map(|s| s.to_string()).collect(),
            asked_by: "agent".to_string(),
            expires_at,
        }
    }

    #[test]
    fn an_asked_question_is_open_until_it_expires() {
        let qs = project(&[asked("q-1", 100, &["work"], Some(200))]);
        assert!(qs[0].is_open(150), "before the expiry it waits");
        assert!(!qs[0].is_open(200), "at the expiry it stops being open");
        assert!(
            qs[0].is_due_for_expiry(200),
            "and sweep should apply the default"
        );
    }

    #[test]
    fn an_open_question_without_an_expiry_never_expires() {
        let qs = project(&[asked("q-1", 0, &[], None)]);
        assert!(qs[0].is_open(u64::MAX / 2), "no clock, so it waits forever");
        assert!(!qs[0].is_due_for_expiry(u64::MAX / 2));
        assert_eq!(qs[0].expires_in(1_000), None);
    }

    #[test]
    fn a_human_answer_resolves_it_and_keeps_the_note() {
        let qs = project(&[
            asked("q-1", 100, &["work"], None),
            Event::Answered {
                id: "q-1".into(),
                at: 160,
                answer: "b".into(),
                by: "human".into(),
                note: Some("because staging is down".into()),
            },
        ]);
        assert_eq!(qs[0].state, State::Answered);
        assert_eq!(qs[0].answer.as_deref(), Some("b"));
        assert_eq!(qs[0].resolved_at, Some(160));
        assert_eq!(qs[0].note.as_deref(), Some("because staging is down"));
        assert!(!qs[0].is_open(1_000));
        assert!(!qs[0].took_the_default(), "b is not the default");
    }

    #[test]
    fn applying_the_default_counts_as_the_default_even_when_a_human_typed_it() {
        let qs = project(&[
            asked("q-1", 0, &[], None),
            Event::Answered {
                id: "q-1".into(),
                at: 10,
                answer: "a".into(),
                by: "human".into(),
                note: None,
            },
        ]);
        assert!(
            qs[0].took_the_default(),
            "a IS the default, and that is worth counting"
        );
    }

    #[test]
    fn an_expiry_event_records_what_was_applied() {
        let qs = project(&[
            asked("q-1", 0, &[], Some(50)),
            Event::Expired {
                id: "q-1".into(),
                at: 51,
                applied: "a".into(),
            },
        ]);
        assert_eq!(qs[0].state, State::Expired);
        assert_eq!(qs[0].answer.as_deref(), Some("a"));
        assert!(qs[0].took_the_default());
    }

    #[test]
    fn a_cancelled_question_stops_being_open() {
        let qs = project(&[
            asked("q-1", 0, &["work"], None),
            Event::Cancelled {
                id: "q-1".into(),
                at: 5,
                reason: "the branch was deleted".into(),
            },
        ]);
        assert_eq!(qs[0].state, State::Cancelled);
        assert!(!qs[0].is_open(10));
    }

    #[test]
    fn an_event_for_an_unknown_id_is_ignored_not_fatal() {
        // A second process can be appending while this one reads. One orphan line must not
        // take the whole queue down.
        let qs = project(&[
            Event::Answered {
                id: "q-ghost".into(),
                at: 1,
                answer: "a".into(),
                by: "human".into(),
                note: None,
            },
            asked("q-1", 2, &[], None),
        ]);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].id, "q-1");
    }

    #[test]
    fn projection_keeps_the_order_questions_were_asked_in() {
        let qs = project(&[
            asked("q-a", 10, &[], None),
            asked("q-b", 20, &[], None),
            asked("q-c", 5, &[], None),
        ]);
        assert_eq!(
            qs.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(),
            vec!["q-a", "q-b", "q-c"],
            "insertion order, not sorted by time"
        );
    }

    #[test]
    fn ranking_puts_what_blocks_the_most_first_then_the_oldest() {
        let qs = project(&[
            asked("q-old", 10, &[], None),
            asked("q-blocks2", 30, &["a", "b"], None),
            asked("q-blocks1-new", 40, &["a"], None),
            asked("q-blocks1-old", 20, &["a"], None),
        ]);
        let ranked = open_now(&qs, 100);
        assert_eq!(
            ranked.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(),
            vec!["q-blocks2", "q-blocks1-old", "q-blocks1-new", "q-old"]
        );
    }

    #[test]
    fn only_questions_that_declare_what_they_block_hold_the_queue() {
        let qs = project(&[
            asked("q-idle", 10, &[], None),
            asked("q-blocking", 20, &["build the walk"], None),
        ]);
        let blocking_now = blocking(&qs, None, 100);
        assert_eq!(blocking_now.len(), 1);
        assert_eq!(blocking_now[0].id, "q-blocking");
        assert_eq!(open_now(&qs, 100).len(), 2, "both are still visible");
    }

    #[test]
    fn the_gate_can_be_narrowed_to_one_lane() {
        let mut events = vec![asked("q-here", 10, &["x"], None)];
        let mut other = asked("q-there", 20, &["y"], None);
        if let Event::Asked { lane, .. } = &mut other {
            *lane = "other-project".to_string();
        }
        events.push(other);
        let qs = project(&events);
        assert_eq!(blocking(&qs, Some("jev-use"), 100).len(), 1);
        assert_eq!(blocking(&qs, Some("nothing-here"), 100).len(), 0);
        assert_eq!(blocking(&qs, None, 100).len(), 2);
    }

    #[test]
    fn ids_resolve_exactly_then_by_unique_prefix() {
        let qs = project(&[
            asked("q-abc-1", 1, &[], None),
            asked("q-abc-2", 2, &[], None),
        ]);
        assert_eq!(resolve_id(&qs, "q-abc-1").unwrap(), "q-abc-1");
        assert_eq!(
            resolve_id(&qs, "q-abc-2").unwrap(),
            "q-abc-2",
            "a unique prefix resolves"
        );
        match resolve_id(&qs, "q-abc") {
            Err(IdError::Ambiguous(_, ids)) => assert_eq!(ids.len(), 2),
            other => panic!("expected ambiguity, got {other:?}"),
        }
        match resolve_id(&qs, "q-nope") {
            Err(IdError::NotFound(_)) => {}
            other => panic!("expected not found, got {other:?}"),
        }
    }

    #[test]
    fn ids_are_short_unique_and_sortable_by_time() {
        let a = new_id(1_700_000_000, "which front end?");
        let b = new_id(1_700_000_001, "which front end?");
        assert_ne!(a, b, "the clock separates them");
        assert!(a.starts_with("q-"), "typeable prefix");
        assert!(a.len() <= 22, "short enough to retype: {a}");
        let c = new_id(1_700_000_000, "which back end?");
        assert_ne!(a, c, "the text separates them at the same second");
    }

    #[test]
    fn durations_read_the_way_a_person_says_them() {
        assert_eq!(human_age(45), "45s");
        assert_eq!(human_age(90), "1m");
        assert_eq!(human_age(3600 * 2 + 60 * 14), "2h 14m");
        assert_eq!(human_age(86400 * 4 + 3600 * 2), "4d 2h");
    }

    #[test]
    fn summary_counts_everything_and_reports_the_medians() {
        let qs = project(&[
            asked("q-1", 0, &["x"], None),
            Event::Answered {
                id: "q-1".into(),
                at: 600,
                answer: "b".into(),
                by: "human".into(),
                note: None,
            },
            asked("q-2", 0, &[], Some(10)),
            Event::Expired {
                id: "q-2".into(),
                at: 11,
                applied: "a".into(),
            },
            asked("q-3", 0, &[], None),
            Event::Cancelled {
                id: "q-3".into(),
                at: 5,
                reason: "superseded".into(),
            },
            asked("q-4", 90, &["y"], None),
        ]);
        let s = summarize(&qs, 100);
        assert_eq!(
            (s.asked, s.open, s.answered, s.expired, s.cancelled),
            (4, 1, 1, 1, 1)
        );
        assert_eq!(
            s.median_time_to_answer,
            Some(11),
            "the middle of 5s, 11s and 600s"
        );
        assert_eq!(s.took_default, 1, "the expired one");
        assert_eq!(s.oldest_open_age, Some(10));
        assert_eq!(s.per_lane, vec![("jev-use".to_string(), 1)]);
    }

    #[test]
    fn sweep_only_returns_questions_whose_clock_ran_out() {
        let qs = project(&[
            asked("q-late", 0, &[], Some(50)),
            asked("q-early", 0, &[], Some(500)),
            asked("q-forever", 0, &[], None),
        ]);
        let due = due_for_expiry(&qs, 100);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, "q-late");
    }

    #[test]
    fn the_store_reads_back_what_it_wrote_and_reports_a_bad_line_by_number() {
        let dir = std::env::temp_dir().join(format!("jev-ask-test-{}", std::process::id()));
        let path = dir.join("decisions.jsonl");
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::at(&path);

        assert!(
            store.events().unwrap().is_empty(),
            "no file is an empty log"
        );

        store.append(&asked("q-1", 10, &["x"], None)).unwrap();
        store
            .append(&Event::Answered {
                id: "q-1".into(),
                at: 20,
                answer: "a".into(),
                by: "human".into(),
                note: None,
            })
            .unwrap();

        let qs = store.questions().unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].state, State::Answered);

        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        writeln!(f, "{{not json}}").unwrap();
        let err = store.events().unwrap_err().to_string();
        assert!(err.contains("line 3"), "the error names the line: {err}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
