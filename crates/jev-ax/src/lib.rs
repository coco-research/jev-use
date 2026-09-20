//! macOS Accessibility observation and execution, shaped for Jev's decision layer.
//!
//! This crate is the Rust port of the proven Python driver at
//! `~/code/jev-computeruse/jev_desktop/ax.py`. The port target is **parity**, not
//! similarity: same elements, same order, same fingerprint, on the same apps.
//!
//! # The contract
//!
//! The decision layer (`jev_ultrafast/model.py`) is transport-agnostic. It asks one
//! question - which operation, which element index - and validates the answer against
//! the candidates offered. So this crate has exactly two jobs:
//!
//! 1. OBSERVE the accessibility tree into an indexed [`ElementTable`].
//! 2. EXECUTE the chosen element with an accessibility action.
//!
//! It knows nothing about Jev. `jev` knows nothing about AX. They meet one layer up.
//!
//! # The addressability rule
//!
//! An element earns an index only if it is **actionable AND enabled AND on screen**.
//! Python measured this: Terminal reports 20 addressable elements out of 878 nodes,
//! Chrome 17 of 1246. Drop the on-screen test and you get 207 candidates, roughly 150
//! of them invisible menu items. See [`Element::is_addressable`].

#![warn(missing_docs)]
// This crate wraps Objective-C and Core Foundation. `unsafe` is the interface, not an
// oversight, so the workspace's `unsafe_code = warn` is allowed here and the reason is
// documented at each call site instead. Removing this would produce a warning on every
// FFI call, which trains a reader to ignore the one that matters.
#![allow(unsafe_code)]

/// App resolution and activation. See the module docs for why the matching logic is
/// pure while the enumeration is macOS-only.
pub mod app;

/// Which kind of interaction an element supports.
///
/// Maps onto the operations Jev chooses from: `fill` becomes `TYPE_TEXT`, `select`
/// becomes `SELECT`, `click` becomes `CLICK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Click, press, toggle. Anything with an `AXPress` action.
    Click,
    /// A text field whose value can be read and written.
    Fill,
    /// A popup or menu offering a fixed list of choices.
    Select,
}

/// One addressable element in the accessibility tree.
// `Eq` is deliberately absent: `Rect` holds `f64`, which is only `PartialEq`.
// Exact equality is the wrong comparison for geometry anyway.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// Stable handle for the life of one observation. The executor resolves the
    /// chosen index back to an accessibility element through this.
    pub node: u32,
    /// What kind of interaction this element supports.
    pub kind: Kind,
    /// Human-readable name: `AXTitle`, else `AXDescription`, else the current value.
    pub label: String,
    /// Raw accessibility role, e.g. `AXButton`. Kept verbatim for parity checks.
    pub role: String,
    /// Current value, if any.
    pub value: Option<String>,
    /// Screen rectangle in points, if the element is displayed.
    pub rect: Option<Rect>,
    /// Whether `AXValue` accepted a write. `false` means type real keystrokes instead.
    pub settable: bool,
}

/// A screen rectangle in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

impl Rect {
    /// True when this rectangle is genuinely displayed on a `bounds`-sized screen.
    ///
    /// A closed menu reports no rect at all. Hidden tab content sits off-screen.
    /// Collapsed rows are degenerate. All three must be rejected, or Jev is invited
    /// to target something it cannot use.
    pub fn is_on_screen(&self, bounds: Option<(f64, f64)>) -> bool {
        if self.w < 1.0 || self.h < 1.0 {
            return false;
        }
        match bounds {
            None => true,
            Some((sw, sh)) => {
                !(self.x + self.w <= 0.0 || self.y + self.h <= 0.0 || self.x >= sw || self.y >= sh)
            }
        }
    }

    /// Centre point, used by the coordinate-click fallback when `AXPress` fails.
    pub fn centre(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

impl Element {
    /// Whether this element can be acted on: it has an action or accepts a value,
    /// it is enabled, and it is on screen.
    ///
    /// This is the single most important predicate in the crate. Python's numbers
    /// depend on it, and the parity harness compares against them.
    pub fn is_addressable(&self, enabled: bool, bounds: Option<(f64, f64)>) -> bool {
        if !enabled {
            return false;
        }
        let interactive = self.kind == Kind::Fill || self.rect.is_some();
        if !interactive {
            return false;
        }
        match self.rect {
            Some(r) => r.is_on_screen(bounds),
            None => false,
        }
    }
}

/// One complete observation of an app: the numbered table plus its context.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementTable {
    /// Resolved app name, e.g. `Terminal`.
    pub app: String,
    /// Window title, or the app name when there is no window title.
    pub title: String,
    /// Visible text, trimmed to the model's text budget.
    pub text: String,
    /// The addressable elements, in reading order.
    pub elements: Vec<Element>,
    /// Total nodes walked, before filtering. Reported so the drop rate is visible.
    pub nodes_seen: usize,
    /// Nodes rejected specifically by the on-screen test.
    pub hidden: usize,
    /// True when the walk hit the node cap and the tree is incomplete.
    pub truncated: bool,
    /// Observation cost in milliseconds.
    pub elapsed_ms: u64,
}

impl ElementTable {
    /// A hash of the element table, used to tell whether the screen actually moved.
    ///
    /// The agent loop compares this between steps. Three identical operations with an
    /// unchanged fingerprint is a loop; three identical operations that each move the
    /// screen is legitimate progress (clicking "new tab" three times opens three tabs).
    pub fn fingerprint(&self) -> String {
        let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
        for e in &self.elements {
            let row = format!(
                "{}:{}:{}",
                match e.kind {
                    Kind::Click => "click",
                    Kind::Fill => "fill",
                    Kind::Select => "select",
                },
                e.label,
                e.value.as_deref().unwrap_or("")
            );
            for b in row.as_bytes() {
                acc ^= u64::from(*b);
                acc = acc.wrapping_mul(0x100_0000_01b3);
            }
        }
        format!("{acc:016x}")
    }
}

/// Node and depth caps for one walk.
///
/// Python uses 25 / 2000 / 6000. The caps exist so one enormous tree cannot blow the
/// context window; a 10k-node Electron app was the original motivation.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum tree depth to descend.
    pub max_depth: usize,
    /// Maximum nodes to visit before truncating.
    pub max_nodes: usize,
    /// Maximum characters of visible text to keep.
    pub text_budget: usize,
    /// Per-element accessibility messaging timeout, in seconds.
    ///
    /// You cannot cancel a blocked `AXUIElementCopyAttributeValue`. This timeout is
    /// the only thing stopping an unresponsive app from pinning the walk forever.
    pub messaging_timeout_secs: f64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 25,
            max_nodes: 2_000,
            text_budget: 6_000,
            messaging_timeout_secs: 2.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(kind: Kind, label: &str, rect: Option<Rect>) -> Element {
        Element {
            node: 1,
            kind,
            label: label.to_string(),
            role: "AXButton".to_string(),
            value: None,
            rect,
            settable: false,
        }
    }

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn offscreen_rect_is_rejected() {
        let bounds = Some((1440.0, 900.0));
        assert!(
            !rect(-500.0, 100.0, 100.0, 40.0).is_on_screen(bounds),
            "left of screen"
        );
        assert!(
            !rect(100.0, -500.0, 100.0, 40.0).is_on_screen(bounds),
            "above screen"
        );
        assert!(
            !rect(2000.0, 100.0, 100.0, 40.0).is_on_screen(bounds),
            "right of screen"
        );
        assert!(
            !rect(100.0, 1200.0, 100.0, 40.0).is_on_screen(bounds),
            "below screen"
        );
    }

    #[test]
    fn degenerate_rect_is_rejected() {
        let bounds = Some((1440.0, 900.0));
        assert!(
            !rect(10.0, 10.0, 0.0, 40.0).is_on_screen(bounds),
            "zero width"
        );
        assert!(
            !rect(10.0, 10.0, 100.0, 0.0).is_on_screen(bounds),
            "zero height"
        );
    }

    #[test]
    fn visible_rect_is_accepted() {
        assert!(rect(100.0, 100.0, 200.0, 40.0).is_on_screen(Some((1440.0, 900.0))));
        assert!(
            rect(100.0, 100.0, 200.0, 40.0).is_on_screen(None),
            "no bounds means no clipping test"
        );
    }

    #[test]
    fn an_element_with_no_rect_is_never_addressable() {
        // This is the closed-menu case: it reports no position at all.
        let e = el(Kind::Click, "About This Mac", None);
        assert!(!e.is_addressable(true, Some((1440.0, 900.0))));
    }

    #[test]
    fn a_disabled_element_is_never_addressable() {
        let e = el(Kind::Click, "Save", Some(rect(10.0, 10.0, 80.0, 30.0)));
        assert!(!e.is_addressable(false, Some((1440.0, 900.0))), "disabled");
        assert!(
            e.is_addressable(true, Some((1440.0, 900.0))),
            "same element, enabled"
        );
    }

    #[test]
    fn fingerprint_tracks_content_not_order_only() {
        let mk = |label: &str| ElementTable {
            app: "Terminal".into(),
            title: "Terminal".into(),
            text: String::new(),
            elements: vec![el(Kind::Click, label, Some(rect(0.0, 0.0, 10.0, 10.0)))],
            nodes_seen: 1,
            hidden: 0,
            truncated: false,
            elapsed_ms: 1,
        };
        assert_eq!(
            mk("New tab").fingerprint(),
            mk("New tab").fingerprint(),
            "stable"
        );
        assert_ne!(
            mk("New tab").fingerprint(),
            mk("New tab ").fingerprint(),
            "content matters"
        );
    }

    #[test]
    fn fingerprint_ignores_volatile_fields() {
        // Two observations of an unchanged screen must hash the same even though
        // timing and node counts differ, or the loop would think the screen moved.
        let mut a = ElementTable {
            app: "Terminal".into(),
            title: "Terminal".into(),
            text: "same".into(),
            elements: vec![el(Kind::Click, "New tab", Some(rect(0.0, 0.0, 10.0, 10.0)))],
            nodes_seen: 878,
            hidden: 156,
            truncated: false,
            elapsed_ms: 470,
        };
        let mut b = a.clone();
        b.elapsed_ms = 901;
        b.nodes_seen = 900;
        b.hidden = 1;
        b.text = "different text, still same elements".into();
        assert_eq!(
            a.fingerprint(),
            b.fingerprint(),
            "volatile fields must not affect the hash"
        );
        a.elements.push(el(
            Kind::Click,
            "Close tab",
            Some(rect(0.0, 0.0, 10.0, 10.0)),
        ));
        assert_ne!(
            a.fingerprint(),
            b.fingerprint(),
            "a new element must change it"
        );
    }

    #[test]
    fn default_limits_match_the_python_reference() {
        let l = Limits::default();
        assert_eq!(l.max_depth, 25);
        assert_eq!(l.max_nodes, 2_000);
        assert_eq!(l.text_budget, 6_000);
        assert!((l.messaging_timeout_secs - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn centre_is_the_midpoint() {
        let (x, y) = rect(100.0, 200.0, 40.0, 20.0).centre();
        assert!((x - 120.0).abs() < f64::EPSILON);
        assert!((y - 210.0).abs() < f64::EPSILON);
    }
}
