//! App resolution and activation: a name to a running process, and bring it forward.
//!
//! This is task T2. It exists because every walk needs an entry point: you cannot
//! observe an app you cannot name, and you cannot type into an app that is not
//! frontmost.
//!
//! # The layering that makes this testable
//!
//! Only the *matching* logic is interesting, and matching is pure. So it lives in
//! [`match_app`](crate::app::match_app), which takes a list and returns a decision — no Cocoa, no AX, and it
//! runs in CI on any platform. The macOS calls that *produce* that list live behind
//! `cfg(target_os = "macos")` and are covered by the local pre-push hook.
//!
//! That split is deliberate. See `.metagpt/GATE.json` for the known CI gap.

/// A running application that has a UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    /// Localised display name, e.g. `Google Chrome`.
    pub name: String,
    /// Process identifier.
    pub pid: i32,
    /// Whether this was the frontmost app when enumerated.
    pub frontmost: bool,
}

/// Why a name could not be turned into exactly one app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// Nothing matched.
    NoMatch(String),
    /// Several matched equally well. Listed frontmost first.
    ///
    /// This is an error rather than a silent pick because choosing for the caller is
    /// exactly the kind of quiet substitution that rule R10 forbids. Offer the list;
    /// let the caller be specific.
    Ambiguous {
        /// The query that was too broad.
        query: String,
        /// Every candidate, frontmost first.
        candidates: Vec<String>,
    },
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatch(q) => write!(f, "no running app matches {q:?}"),
            Self::Ambiguous { query, candidates } => write!(
                f,
                "{:?} matches {} apps: {}. Be more specific.",
                query,
                candidates.len(),
                candidates.join(", ")
            ),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Turn a name into exactly one app.
///
/// Three rules, in order:
///
/// 1. **Exact match wins**, case-insensitively. `Terminal` beats `Terminal Foo`.
/// 2. **A unique partial match wins.** `chrome` finds `Google Chrome` when that is the
///    only app containing it.
/// 3. **Several partial matches is an error**, not a guess. The candidate list is
///    returned frontmost-first so the caller can pick.
///
/// Matching is case-insensitive because nobody wants to type `Google Chrome` exactly.
/// It is not fuzzy because a fuzzy match that resolves to the wrong app is worse than
/// an error — you would be driving the wrong program.
pub fn match_app<'a>(query: &str, apps: &'a [App]) -> Result<&'a App, ResolveError> {
    let q = query.trim();
    if q.is_empty() {
        return Err(ResolveError::NoMatch(query.to_string()));
    }
    let ql = q.to_lowercase();

    // 1. exact, case-insensitive
    if let Some(app) = apps.iter().find(|a| a.name.to_lowercase() == ql) {
        return Ok(app);
    }

    // 2. unique partial
    let partial: Vec<&App> = apps
        .iter()
        .filter(|a| a.name.to_lowercase().contains(&ql))
        .collect();
    match partial.len() {
        0 => Err(ResolveError::NoMatch(q.to_string())),
        1 => Ok(partial[0]),
        _ => Err(ResolveError::Ambiguous {
            query: q.to_string(),
            candidates: partial.iter().map(|a| a.name.clone()).collect(),
        }),
    }
}

/// The frontmost app, if any.
pub fn frontmost(apps: &[App]) -> Option<&App> {
    apps.iter().find(|a| a.frontmost)
}

/// Order apps so the frontmost comes first, then by name.
///
/// Deterministic on purpose: an unstable order would make [`match_app`] return
/// different results for the same screen, which is a bug that looks like flakiness.
pub fn sort_apps(apps: &mut [App]) {
    apps.sort_by(|a, b| {
        b.frontmost
            .cmp(&a.frontmost)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

// ── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use super::{sort_apps, App};
    use crate::Limits;
    use objc2_app_kit::{
        NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication,
        NSWorkspace,
    };
    use objc2_application_services::AXUIElement;
    use objc2_core_foundation::CFRetained;

    /// Every running app that has a UI, frontmost first.
    ///
    /// `ActivationPolicy::Regular` is what filters out background processes, menu-bar
    /// helpers and daemons: an app only appears here if it has a Dock icon and can
    /// therefore be observed and driven.
    ///
    /// Safe because objc2 marks every call used here as safe. Marking this `unsafe`
    /// would be noise, and noise around `unsafe` is how a real one gets missed.
    pub fn running_apps() -> Vec<App> {
        let ws = NSWorkspace::sharedWorkspace();
        let front_pid = ws
            .frontmostApplication()
            .map(|a| a.processIdentifier())
            .unwrap_or(0);

        let mut out = Vec::new();
        for app in ws.runningApplications().iter() {
            if app.activationPolicy() != NSApplicationActivationPolicy::Regular {
                continue;
            }
            let pid = app.processIdentifier();
            let name = app
                .localizedName()
                .map(|n| n.to_string())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            out.push(App {
                name,
                pid,
                frontmost: pid == front_pid,
            });
        }
        sort_apps(&mut out);
        out
    }

    /// The AX element for an app, with the messaging timeout already applied.
    ///
    /// The timeout is set HERE, at construction, rather than left to the caller. Rule
    /// R2 says it is mandatory and must be set before any walk; making it impossible to
    /// obtain an element without it is how that rule is enforced in practice rather
    /// than in prose.
    ///
    /// Returns `None` if the timeout could not be set, because an element without a
    /// timeout can hang forever and is worse than no element at all.
    ///
    /// # Safety
    /// The caller must ensure `pid` refers to a live process. The AX API reaches into
    /// another process; a stale pid is not detected here.
    pub unsafe fn app_element(pid: i32, limits: Limits) -> Option<CFRetained<AXUIElement>> {
        // SAFETY: documented constructor, returns a +1 reference. The pid is the
        // caller's responsibility per the contract above.
        let el = unsafe { AXUIElement::new_application(pid) };
        // SAFETY: `el` is live for the duration of this call.
        let err = unsafe { el.set_messaging_timeout(limits.messaging_timeout_secs as f32) };
        if err.0 != 0 {
            return None;
        }
        Some(el)
    }

    /// Bring an app to the front. Returns whether the activation was accepted.
    ///
    /// # Why the options are zero
    ///
    /// The Python driver this crate ports calls `activateWithOptions(1 << 1)`, which is
    /// `NSApplicationActivateIgnoringOtherApps`. On macOS 14 and later that flag is
    /// **deprecated and has no effect** - the SDK says so in as many words. The driver
    /// has therefore been relying on plain activation while appearing to ask for
    /// something stronger. Passing zero says what actually happens.
    ///
    /// Safe: both calls are marked safe by objc2.
    pub fn focus(pid: i32) -> bool {
        match NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
            Some(app) => app.activateWithOptions(NSApplicationActivationOptions(0)),
            None => false,
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{app_element, focus, running_apps};

/// Resolve a name to a running app, or explain why it could not.
///
/// macOS only: it needs the process list.
#[cfg(target_os = "macos")]
pub fn resolve(query: &str) -> Result<App, ResolveError> {
    let apps = macos::running_apps();
    match_app(query, &apps).cloned()
}

// Tests unwrap freely on purpose: a panic here IS the failure report, and threading
// Result through every assertion would obscure what is being asserted.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn apps(names: &[&str]) -> Vec<App> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| App {
                name: (*n).to_string(),
                pid: 100 + i as i32,
                frontmost: i == 0,
            })
            .collect()
    }

    #[test]
    fn exact_match_beats_a_longer_partial() {
        // "Terminal" must not resolve to "Terminal Foo" just because Foo came first.
        let list = apps(&["Terminal Foo", "Terminal"]);
        let hit = match_app("Terminal", &list).unwrap();
        assert_eq!(hit.name, "Terminal");
    }

    #[test]
    fn exact_match_is_case_insensitive() {
        let list = apps(&["Terminal"]);
        assert_eq!(match_app("terminal", &list).unwrap().name, "Terminal");
        assert_eq!(match_app("TERMINAL", &list).unwrap().name, "Terminal");
    }

    #[test]
    fn a_unique_partial_match_resolves() {
        let list = apps(&["Finder", "Google Chrome", "Terminal"]);
        assert_eq!(match_app("chrome", &list).unwrap().name, "Google Chrome");
        assert_eq!(match_app("goog", &list).unwrap().name, "Google Chrome");
    }

    #[test]
    fn several_partial_matches_is_an_error_not_a_guess() {
        // The whole point: do not silently pick one.
        let list = apps(&["Google Chrome", "Chrome Canary"]);
        let err = match_app("chrome", &list).unwrap_err();
        match err {
            ResolveError::Ambiguous { query, candidates } => {
                assert_eq!(query, "chrome");
                assert_eq!(candidates, vec!["Google Chrome", "Chrome Canary"]);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn ambiguous_lists_candidates_in_the_order_given() {
        // So the caller sees the frontmost first and can pick the obvious one.
        let mut list = apps(&["Chrome Canary", "Google Chrome"]);
        list[0].frontmost = false;
        list[1].frontmost = true;
        sort_apps(&mut list);
        let err = match_app("chrome", &list).unwrap_err();
        if let ResolveError::Ambiguous { candidates, .. } = err {
            assert_eq!(
                candidates[0], "Google Chrome",
                "frontmost must lead the list"
            );
        } else {
            panic!("expected Ambiguous");
        }
    }

    #[test]
    fn no_match_is_reported_with_the_query() {
        let list = apps(&["Finder", "Terminal"]);
        match match_app("Photoshop", &list).unwrap_err() {
            ResolveError::NoMatch(q) => assert_eq!(q, "Photoshop"),
            other => panic!("expected NoMatch, got {other:?}"),
        }
    }

    #[test]
    fn whitespace_and_empty_queries_are_rejected() {
        let list = apps(&["Finder"]);
        assert!(match_app("", &list).is_err());
        assert!(match_app("   ", &list).is_err());
        // A query with padding still matches, because people paste with spaces.
        assert!(match_app("  Finder  ", &list).is_ok());
    }

    #[test]
    fn an_exact_match_wins_even_when_several_partials_exist() {
        // "Chrome" is an exact app name AND a partial of "Chrome Canary".
        let list = apps(&["Chrome Canary", "Chrome"]);
        assert_eq!(match_app("Chrome", &list).unwrap().name, "Chrome");
    }

    #[test]
    fn sorting_is_deterministic_and_frontmost_leads() {
        let mut list = vec![
            App {
                name: "Bravo".into(),
                pid: 3,
                frontmost: false,
            },
            App {
                name: "alpha".into(),
                pid: 2,
                frontmost: false,
            },
            App {
                name: "Zulu".into(),
                pid: 1,
                frontmost: true,
            },
        ];
        sort_apps(&mut list);
        let names: Vec<&str> = list.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Zulu", "alpha", "Bravo"],
            "frontmost first, then case-insensitive name"
        );

        // Same input, same output. An unstable sort would make resolution flaky.
        let mut again = vec![
            App {
                name: "Zulu".into(),
                pid: 1,
                frontmost: true,
            },
            App {
                name: "Bravo".into(),
                pid: 3,
                frontmost: false,
            },
            App {
                name: "alpha".into(),
                pid: 2,
                frontmost: false,
            },
        ];
        sort_apps(&mut again);
        assert_eq!(again, list);
    }

    #[test]
    fn frontmost_finds_the_flagged_app() {
        let list = apps(&["Finder", "Terminal"]);
        assert_eq!(frontmost(&list).unwrap().name, "Finder");
        let none = apps(&[]);
        assert!(frontmost(&none).is_none());
    }
}
