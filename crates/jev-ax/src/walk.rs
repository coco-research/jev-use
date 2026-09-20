//! The walk: one observation of an app's accessibility tree (task T4).
//!
//! This is `observe()` ported from the pinned reference (`reference/vendor/ax.py:239-389`).
//! It is the most load-bearing piece of the crate: the decision layer picks an index out of
//! what this returns, the executor resolves that index back to an element, and the repeat
//! guard (rule R5) compares its fingerprint. A walk that is subtly wrong is therefore not
//! one bug in one place - it is a wrong candidate list, a wrong hash, and an "operation
//! failed" that was really a bad observation.
//!
//! # What one observation is
//!
//! A bounded depth-first walk over the app's tree. Each node that **earns an index** becomes
//! a [`crate::Element`]; each node that does not may still contribute text, because the table is
//! how a screen's words reach the model. "Earns an index" is rule R1: actionable AND
//! enabled AND on screen. Everything else is context.
//!
//! # The four traps, each with the reason it is a trap
//!
//! 1. **An absent `AXEnabled` is not a disabled element.** A read that fails - `-25205`
//!    `AttributeUnsupported` is a normal answer for an attribute an element does not have -
//!    yields `None`, and `None` must leave the element addressable. Measured on this
//!    machine: the system-wide element reports no `AXEnabled` at all (see the
//!    `an_absent_attribute_is_none_rather_than_a_default` test in `attrs.rs`). Reading a
//!    failed probe as "disabled" drops most of a real screen, and it drops it silently.
//! 2. **Windows are seeded explicitly, and compared by `CFEqual`.** Background windows are
//!    absent from `AXChildren` (rule R3), so `AXWindows` is appended - and the de-duplication
//!    must use Core Foundation equality, never pointer identity. Two reads can hand back two
//!    different `AXUIElementRef`s for the same window, so an identity check de-duplicates
//!    nothing and every window is walked twice. `AXUIElement`'s `PartialEq` in `objc2` is
//!    `CFEqual`, which is why it is used here and not `ptr::eq`.
//! 3. **The rect is asked for last.** `AXPosition` and `AXSize` are two round trips, so they
//!    are only asked for an element that already passed actionability and enablement. Asking
//!    first makes every walk several times slower and buys nothing.
//! 4. **The node counter counts one per node.** The reference increments it *twice* for the
//!    same node (`ax.py:272-273`), so its `elements_seen` is double the nodes it walked and
//!    its effective node cap is half of `MAX_NODES`. That is preserved in the reference and
//!    fixed here, deliberately and with the harness encoding it:
//!    `scripts/parity --live` asserts `ref.elements_seen == 2 * port.nodes_seen`. Keeping
//!    the doubled counter would have reproduced a measurement bug in new code; halving the
//!    cap silently would have been a divergence nobody could see.
//!
//! # What is pure, and why it is public
//!
//! Everything that does not need an `AXUIElement` is outside the `macos` module below: the
//! role-to-kind decision, the label and text truncation, the addressability decision, the
//! root union and the child-push order. Those are public so that CI on ubuntu - which can
//! never compile a line of the accessibility path - still tests them, the way `app.rs` does
//! with its name matcher. The parts that need the API are gated and covered by the local
//! pre-push hook and the self-hosted runner. See `.metagpt/GATE.json`.
//!
//! # The one place this refuses where the reference shrugs
//!
//! If the app's own `AXRole` cannot be read, `observe` returns [`ObserveError`](crate::walk::ObserveError) instead of
//! an empty table. The reference skips the check entirely, so an app that answers nothing
//! (measured: PI-Desktop returns `-25204` to *every* attribute - `docs/memory.md` D8) yields
//! `elements_seen: 0`, which reads as "this app has no UI". Rule R10 says name the cause.

use crate::{Kind, Rect};

/// The reference's `observe()` keys one element by these caps (`ax.py:325`, `ax.py:332`).
///
/// Both are **characters**, not bytes, because Python slices strings. See [`truncate`].
const LABEL_LIMIT: usize = 120;

/// How the parts of the screen's text are joined (`ax.py:371`).
///
/// Not a space: the separator is part of the observable contract, and a fixture diff compares
/// the joined text.
const TEXT_SEPARATOR: &str = " \u{b7} ";

/// The role reported for a node whose `AXRole` could not be read (`ax.py:277`).
const UNKNOWN_ROLE: &str = "AXUnknown";

/// Roles whose `AXValue` can be written directly.
///
/// Probing settability costs a round trip per element, so it is only done for these roles
/// (`ax.py:51-56`). Public because rule R4's executor needs the same list to know when a
/// write is even worth trying.
pub const SETTABLE_ROLES: &[&str] = &[
    "AXTextField",
    "AXTextArea",
    "AXComboBox",
    "AXSearchField",
    "AXSlider",
    "AXStepper",
    "AXCheckBox",
    "AXRadioButton",
];

/// Roles that accept typed text (`ax.py:58-62`).
///
/// A rich-text or web-backed editor often reports `AXValue` as not settable even though it is
/// perfectly typeable, so these stay addressable and the executor falls back to focusing the
/// element and sending keystrokes. Without this the main editing surface gets no index at all.
pub const EDITABLE_ROLES: &[&str] = &["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"];

/// The roles whose interaction kind is fixed (`ax.py:67-71`).
///
/// Everything else is decided from what the element can actually do, which is rule R1's
/// point: the role suggests, the action names decide.
pub const KIND_BY_ROLE: &[(&str, Kind)] = &[
    ("AXTextField", Kind::Fill),
    ("AXTextArea", Kind::Fill),
    ("AXSearchField", Kind::Fill),
    ("AXComboBox", Kind::Fill),
    ("AXPopUpButton", Kind::Select),
    ("AXMenuButton", Kind::Select),
];

/// Why an observation could not be made.
///
/// Rule R10: every variant names the real cause. There is deliberately no variant meaning
/// "an empty screen": a walk that found nothing addressable is a legitimate [`ElementTable`](crate::ElementTable)
/// with no elements, and an empty table produced by a permission error must never be able to
/// look like one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveError {
    /// The app could not be resolved to exactly one running process.
    App(crate::app::ResolveError),
    /// Nothing was named and no app is focused.
    NoFocusedApp,
    /// This process has no Accessibility grant (`-25211`, `APIDisabled`).
    ///
    /// System Settings -> Privacy & Security -> Accessibility, then add whatever runs this
    /// binary. Every read would return nothing without it, which is exactly why it is an
    /// error rather than an empty tree.
    NotPermitted,
    /// The app element could not take its messaging timeout.
    ///
    /// Rule R2: a blocked `AXUIElementCopyAttributeValue` cannot be cancelled, so that
    /// timeout is the only thing standing between an unresponsive app and a thread pinned
    /// forever. An element without one is worse than no element at all.
    NoMessagingTimeout,
    /// The app did not answer a read.
    ///
    /// `-25204` (`CannotComplete`) is the usual code, and it means the app is busy or
    /// unresponsive rather than that the attribute is missing (`-25205`).
    AppNotAnswering {
        /// Which attribute went unanswered.
        attribute: &'static str,
        /// The raw `AXError` code.
        code: i32,
    },
}

impl std::fmt::Display for ObserveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::App(e) => write!(f, "{e}"),
            Self::NoFocusedApp => write!(
                f,
                "no app is focused: name one explicitly, or bring one to the front"
            ),
            Self::NotPermitted => write!(
                f,
                "accessibility is not granted to this process (AXError -25211): System \
                 Settings -> Privacy & Security -> Accessibility"
            ),
            Self::NoMessagingTimeout => write!(
                f,
                "could not set the accessibility messaging timeout (rule R2) on this app"
            ),
            Self::AppNotAnswering { attribute, code } => write!(
                f,
                "the app did not answer {attribute} (AXError {code}); it is busy, unresponsive, \
                 or not an app"
            ),
        }
    }
}

impl std::error::Error for ObserveError {}

impl From<crate::app::ResolveError> for ObserveError {
    fn from(e: crate::app::ResolveError) -> Self {
        Self::App(e)
    }
}

// ── the pure decisions ─────────────────────────────────────────────────────
//
// Each one is a step of the reference's loop lifted out where it can be tested without an
// accessibility grant, which no CI runner has.

/// Keep the first `max` **characters** of `text`.
///
/// Characters, not bytes. The reference slices a Python string, and Python counts code
/// points, so `label[:120]` on a label of CJK or accented text keeps 120 characters.
/// Slicing UTF-8 at byte offset 120 would land mid-character, and every label on a screen
/// would report a difference against the same label in Python.
#[must_use]
pub fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// The label an element is known by: `AXTitle`, else `AXDescription`, else the value, else
/// the role without its `AX` prefix, truncated to 120 characters (`ax.py:308`, `ax.py:325`).
///
/// The order matters and was measured: Calculator's digits have `AXTitle=""` with
/// `AXDescription="2"`, so a walk that collapsed the two would label every digit "" and the
/// screen would be unusable. Dropping to the role is the last resort, and it is why a plain
/// `AXButton` still has a label rather than an empty string.
#[must_use]
pub fn label_of(title: &str, desc: &str, value: &str, role: &str) -> String {
    let first = if !title.is_empty() {
        title
    } else if !desc.is_empty() {
        desc
    } else if !value.is_empty() {
        value
    } else {
        // `str.replace` replaces every occurrence, and so does this - the reference writes
        // `role.replace("AX", "")`, not `removeprefix`.
        return truncate(&role.replace("AX", ""), LABEL_LIMIT);
    };
    truncate(first, LABEL_LIMIT)
}

/// What kind of interaction a node supports (`ax.py:309-314`).
///
/// A role in [`KIND_BY_ROLE`] decides outright. Otherwise: a typeable role is a `fill`, a
/// settable node that is not clickable is a `fill` (it is a value, not a button), and
/// anything else is a `click`.
#[must_use]
pub fn kind_of(role: &str, typeable: bool, settable: bool, clickable: bool) -> Kind {
    if let Some((_, kind)) = KIND_BY_ROLE.iter().find(|(r, _)| *r == role) {
        return *kind;
    }
    // The reference's two middle branches both end in `fill` - a role that accepts typed text
    // and a settable element that is not clickable are both a value rather than a button.
    // Written as one condition so that clippy does not have to be silenced to read them.
    if typeable || (settable && !clickable) {
        Kind::Fill
    } else {
        Kind::Click
    }
}

/// True only for a rectangle that is really being displayed (`ax.py:163-178`).
///
/// A closed menu reports no rect at all. Hidden tab content sits off-screen, and collapsed
/// rows are degenerate. All three would otherwise earn an index and invite a decision that
/// cannot be executed. `None` bounds means the screen size could not be read, and the
/// reference's answer there is to accept anything with a real size. See [`Rect::is_on_screen`].
#[must_use]
pub fn on_screen(rect: Option<Rect>, bounds: Option<(f64, f64)>) -> bool {
    match rect {
        Some(r) => r.is_on_screen(bounds),
        None => false,
    }
}

/// Whether a node earns an index, given what has already been read about it
/// (`ax.py:291-295`).
///
/// This is rule R1's first half - actionable and enabled - deliberately without the rect,
/// which is read afterwards and tested separately. The on-screen half is [`on_screen`], and
/// the two together are what [`Element::is_addressable`](crate::Element::is_addressable) checks after the fact.
///
/// `enabled` is an `Option` for a reason: `Some(false)` refuses the node, and `None` - the
/// answer to a probe that failed - does not. See trap 1 in the module docs.
#[must_use]
pub fn earns_index(
    clickable: bool,
    settable: bool,
    typeable: bool,
    role: &str,
    enabled: Option<bool>,
) -> bool {
    let actionable = (clickable || settable || typeable) && role != UNKNOWN_ROLE;
    actionable && enabled != Some(false)
}

/// The walk's roots: the app's children, then any window that is not already among them
/// (`ax.py:250-256`, rule R3).
///
/// Order is the contract: children first, windows appended, so a walk visits what
/// `AXChildren` reports before it visits anything seeded. `T`'s equality must be the
/// accessibility API's own (Core Foundation's `CFEqual`, which `AXUIElement`'s `PartialEq`
/// is). Pointer identity would make this de-duplication do nothing, because two reads of the
/// same window can be two different references.
#[must_use]
pub fn union_roots<T: PartialEq + Clone>(children: &[T], windows: &[T]) -> Vec<T> {
    let mut out = children.to_vec();
    for w in windows {
        if !out.contains(w) {
            out.push(w.clone());
        }
    }
    out
}

/// The items to push next: `items` reversed, each paired with the depth to walk it at.
///
/// Reversed because the walk pops from the end of its stack: pushing `[a, b, c]` in reverse
/// makes `a` pop first, so the walk stays in the order the app reported its children -
/// `AXChildren` order, never sorted and never filtered. The roots go on at depth 0 and a
/// node's children at `depth + 1`; the reference does the same with two `reversed()`
/// comprehensions (`ax.py:266`, `ax.py:362-363`).
///
/// Borrowed rather than cloned: in the walk `T` is a live `AXUIElement` and cloning it is a
/// retain, so the caller decides when that happens.
pub fn descend<T>(items: &[T], depth: usize) -> impl DoubleEndedIterator<Item = (&T, usize)> + '_ {
    items.iter().rev().map(move |item| (item, depth))
}

/// The screen's text: every contribution joined with the reference's separator, cut to the
/// character budget (`ax.py:371`).
#[must_use]
pub fn join_text(parts: &[String], budget: usize) -> String {
    truncate(&parts.join(TEXT_SEPARATOR), budget)
}

// ── macOS ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use std::ptr::{self, NonNull};
    use std::time::Instant;

    use objc2_application_services::{AXError, AXUIElement};
    use objc2_core_foundation::{
        CFArray, CFBoolean, CFNull, CFNumber, CFRetained, CFString, CFType,
    };

    use super::{
        descend, earns_index, join_text, kind_of, label_of, on_screen, truncate, union_roots,
        ObserveError, EDITABLE_ROLES, LABEL_LIMIT, SETTABLE_ROLES, UNKNOWN_ROLE,
    };
    use crate::{app, attrs, Element, ElementTable, Kind, Limits};

    /// How much of `AXValue` a table keeps (`ax.py:332`). The reference truncates *before*
    /// hashing, so this is part of the fingerprint's input, not a display choice.
    const VALUE_LIMIT: usize = 200;

    /// How many children one bulk read asks for.
    ///
    /// The reference passes `999` to `AXUIElementCopyAttributeValues` and never asks whether
    /// the answer was cut short (`ax.py:106`), so it is a silent cap on any element with a
    /// thousand children. Kept, because a difference here changes the walk on an enormous
    /// list view, and rule R8 says the reference decides.
    const MAX_CHILDREN: isize = 999;

    /// How many children of a popup are examined for its options (`ax.py:336`).
    const MAX_OPTIONS: usize = 40;

    /// `CGRect`'s C layout: origin then size, four `f64`s.
    ///
    /// Declared rather than pulled from another crate: two functions are the whole need, and
    /// a new framework binding would be a dependency to keep in step for two calls.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    /// `CGSize`'s C layout.
    #[repr(C)]
    struct CGSize {
        width: f64,
        height: f64,
    }

    /// `CGRect`'s C layout. The origin is part of the ABI and is never read: the on-screen
    /// test clips against the screen's size, not its origin.
    #[repr(C)]
    struct CGRect {
        #[allow(dead_code)]
        origin: CGPoint,
        size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }

    /// The main display's size in points, read once per walk (`ax.py:154-160`).
    ///
    /// The main display is the one with the menu bar on it. `NSScreen::mainScreen` is a
    /// different question - it answers with the screen holding the key window - so on a
    /// two-monitor machine it would silently clip against the wrong screen. The reference
    /// asks Core Graphics, and so does this.
    fn screen_bounds() -> Option<(f64, f64)> {
        // SAFETY: both are pure functions of the current display configuration, and the
        // return value is a plain C struct.
        let bounds = unsafe { CGDisplayBounds(CGMainDisplayID()) };
        Some((bounds.size.width, bounds.size.height))
    }

    /// One attribute read with the reference's `_attr` semantics: a failure, an absent
    /// attribute and an explicit null are all `None` (`ax.py:76-89`).
    ///
    /// `attrs` deliberately speaks in scalars; the walk needs the raw value twice - to tell
    /// an element from an array, and to reproduce Python's truthiness - so the typed read
    /// happens here rather than being forced through a string.
    fn attribute(el: &AXUIElement, name: &str) -> Option<CFRetained<CFType>> {
        // SAFETY: `el` is live, which is this module's whole contract.
        let value = unsafe { attrs::raw(el, name) }.ok()?;
        if value.downcast_ref::<CFNull>().is_some() {
            return None;
        }
        Some(value)
    }

    /// Python truthiness of a raw value: `bool(_attr(...))` (`ax.py:347`).
    ///
    /// A boolean answers for itself, a number is false only at zero, and anything else is
    /// present and therefore true - which is exactly what `bool()` does to a Python object.
    fn truthy(value: &CFType) -> bool {
        if let Some(b) = value.downcast_ref::<CFBoolean>() {
            return b.as_bool();
        }
        if let Some(n) = value.downcast_ref::<CFNumber>() {
            if let Some(i) = n.as_i64() {
                return i != 0;
            }
            if let Some(f) = n.as_f64() {
                return f != 0.0;
            }
        }
        true
    }

    /// A checkbox or radio button's state, or `None` when the value is not a number.
    ///
    /// The reference writes `bool(val) if isinstance(val, (int, bool)) else None`
    /// (`ax.py:350`): a value that arrives as anything else leaves `checked` unset rather
    /// than becoming a confident `false`.
    fn checked_of(value: &CFType) -> Option<bool> {
        if let Some(b) = value.downcast_ref::<CFBoolean>() {
            return Some(b.as_bool());
        }
        value
            .downcast_ref::<CFNumber>()
            .and_then(|n| n.as_i64())
            .map(|i| i != 0)
    }

    /// Re-type an array-valued attribute read as elements.
    ///
    /// `copy_attribute_values` hands back a borrowed pointer for each index, so every one is
    /// retained: the array is dropped when this function returns and the elements must
    /// outlive it.
    fn elements_of(array: &CFArray) -> Vec<CFRetained<AXUIElement>> {
        let mut out = Vec::with_capacity(array.count().max(0) as usize);
        for i in 0..array.count() {
            // SAFETY: `i` is within `count`.
            let item = unsafe { array.value_at_index(i) };
            if item.is_null() {
                continue;
            }
            // SAFETY: `AXChildren` and `AXWindows` hold AXUIElement references; retaining a
            // borrowed one keeps it alive past the array's drop.
            out.push(unsafe {
                CFRetained::retain(NonNull::new_unchecked(item as *mut AXUIElement))
            });
        }
        out
    }

    /// One bulk attribute read, or `None` (`ax.py:106-109`).
    fn bulk_array(el: &AXUIElement, name: &str, max: isize) -> Option<CFRetained<CFArray>> {
        let key = CFString::from_str(name);
        let mut out: *const CFArray = ptr::null();
        // SAFETY: `slot` points at a live, properly aligned local; the callee only writes it.
        let slot: NonNull<*const CFArray> = unsafe { NonNull::new_unchecked(&mut out) };
        // SAFETY: `el` is live per this module's contract.
        let err = unsafe { el.copy_attribute_values(&key, 0, max, slot) };
        if err.0 != 0 || out.is_null() {
            return None;
        }
        // SAFETY: non-null checked, and a Copy function returned a +1 reference.
        Some(unsafe { CFRetained::from_raw(NonNull::new_unchecked(out as *mut CFArray)) })
    }

    /// Every child of `el`, in `AXChildren` order.
    ///
    /// The bulk read first, the single-value read as the fallback, and a non-list answer as
    /// an empty list - the reference's three cases in the same order (`ax.py:103-112`). The
    /// fallback is not decoration: some elements refuse the indexed read and answer the
    /// single-value one, and losing them would quietly halve a subtree.
    fn children_of(el: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
        if let Some(array) = bulk_array(el, attrs::name::CHILDREN, MAX_CHILDREN) {
            return elements_of(&array);
        }
        match attribute(el, attrs::name::CHILDREN).and_then(|v| v.downcast::<CFArray>().ok()) {
            Some(array) => elements_of(&array),
            None => Vec::new(),
        }
    }

    /// The app's windows, or an empty list when `AXWindows` is absent or is not a list.
    fn windows_of(el: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
        attribute(el, attrs::name::WINDOWS)
            .and_then(|v| v.downcast::<CFArray>().ok())
            .map(|array| elements_of(&array))
            .unwrap_or_default()
    }

    /// The choices a `select` offers, from its own children (`ax.py:335-343`).
    ///
    /// A closed popup keeps its items somewhere the tree does not reach, so an empty answer
    /// is normal and is reported as such rather than filled in with a guess.
    fn options_of(children: &[CFRetained<AXUIElement>]) -> Vec<String> {
        let mut out = Vec::new();
        for child in children.iter().take(MAX_OPTIONS) {
            // SAFETY: live elements, per this module's contract.
            if unsafe { attrs::role(child) } != "AXMenuItem" {
                continue;
            }
            // SAFETY: as above.
            let title = unsafe { attrs::title(child) };
            // SAFETY: as above.
            let text = if title.is_empty() {
                unsafe { attrs::value(child) }
            } else {
                title
            };
            if !text.is_empty() {
                out.push(text);
            }
        }
        out
    }

    /// `AXSelected` and, for a checkbox or radio button, `AXValue` as a state
    /// (`ax.py:344-350`).
    ///
    /// Only asked for when it can mean something: the element exposes an `AXSelected` action,
    /// or its role carries selection. Both reads cost a round trip each.
    fn selection_of(
        el: &AXUIElement,
        names: &[String],
        role: &str,
    ) -> (Option<bool>, Option<bool>) {
        let watches_selection = names.iter().any(|n| n == "AXSelected")
            || matches!(role, "AXTab" | "AXRow" | "AXCheckBox" | "AXRadioButton");
        if !watches_selection {
            return (None, None);
        }
        let selected = attribute(el, attrs::name::SELECTED).map(|v| truthy(&v));
        let checked = if matches!(role, "AXCheckBox" | "AXRadioButton") {
            attribute(el, attrs::name::VALUE).and_then(|v| checked_of(&v))
        } else {
            None
        };
        (selected, checked)
    }

    /// `Some(text)` unless it is empty, which is how the reference decides whether a key is
    /// worth emitting at all (`ax.py:331`).
    fn non_empty(text: String) -> Option<String> {
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// The pid behind an element, or `None`.
    fn pid_of(el: &AXUIElement) -> Option<i32> {
        let mut pid: i32 = 0;
        // SAFETY: `slot` points at a live local; the callee only writes it.
        let slot: NonNull<i32> = unsafe { NonNull::new_unchecked(&mut pid) };
        // SAFETY: `el` is live.
        let err = unsafe { el.pid(slot) };
        (err.0 == 0).then_some(pid)
    }

    /// The focused application's element, or `None`.
    ///
    /// Read from the system-wide element rather than from `NSWorkspace`'s frontmost app: they
    /// can disagree, and the reference asks the accessibility API (`ax.py:183-192`).
    fn focused_element() -> Option<CFRetained<AXUIElement>> {
        // SAFETY: the system-wide element is always available.
        let system = unsafe { AXUIElement::new_system_wide() };
        attribute(&system, "AXFocusedApplication")?
            .downcast::<AXUIElement>()
            .ok()
    }

    /// One honest read before walking: may this process look at all?
    ///
    /// Every attribute read would answer `-25211` (`APIDisabled`) without a grant, and every
    /// read silently becomes an empty string - which is how an unpermitted walk turns into a
    /// confident "this app has no UI" (rule R10). See [`ObserveError::NotPermitted`].
    fn check_readable(el: &AXUIElement) -> Result<(), ObserveError> {
        // SAFETY: `el` is live.
        match unsafe { attrs::raw(el, attrs::name::ROLE) } {
            Ok(_) => Ok(()),
            Err(code) if code == AXError::APIDisabled.0 => Err(ObserveError::NotPermitted),
            Err(code) => Err(ObserveError::AppNotAnswering {
                attribute: attrs::name::ROLE,
                code,
            }),
        }
    }

    /// The walk's entry point: an app element that already carries its messaging timeout
    /// (rule R2), and the name to report it under.
    fn root_element(
        app: Option<&str>,
        limits: Limits,
    ) -> Result<(CFRetained<AXUIElement>, String), ObserveError> {
        match app {
            Some(query) => {
                let found = app::resolve(query)?;
                // SAFETY: the pid came from a live process enumeration.
                let el = unsafe { app::app_element(found.pid, limits) }
                    .ok_or(ObserveError::NoMessagingTimeout)?;
                check_readable(&el)?;
                Ok((el, found.name))
            }
            None => {
                let focused = focused_element().ok_or(ObserveError::NoFocusedApp)?;
                let pid = pid_of(&focused).ok_or(ObserveError::NoFocusedApp)?;
                // SAFETY: the pid of a focused element belongs to a live process.
                let el = unsafe { app::app_element(pid, limits) }
                    .ok_or(ObserveError::NoMessagingTimeout)?;
                check_readable(&el)?;
                let label = app::running_apps()
                    .iter()
                    .find(|a| a.pid == pid)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "focused app".to_string());
                Ok((el, label))
            }
        }
    }

    /// Walk an app's accessibility tree into the table a decision is made from.
    ///
    /// `app` is a name resolved by [`crate::app::resolve`], or `None` for whatever is
    /// focused. `limits` carries the caps (rule R6) and the messaging timeout (rule R2).
    ///
    /// The walk is depth-first, pre-order, in the order the app reports its children. Nodes
    /// deeper than `max_depth` are skipped **without descending** - the subtree goes with
    /// them, exactly as the reference's `continue` does (`ax.py:274-275`). The node cap sets
    /// `truncated` so a caller knows the tree is incomplete rather than assuming it saw
    /// everything.
    ///
    /// # Errors
    /// [`ObserveError`], which names the cause: an app that could not be resolved, no
    /// focused app, no Accessibility grant, no messaging timeout, or an app that did not
    /// answer.
    pub fn observe(app: Option<&str>, limits: Limits) -> Result<ElementTable, ObserveError> {
        let started = Instant::now();
        let (root, label) = root_element(app, limits)?;
        let bounds = screen_bounds();

        // Rule R3: background windows are absent from AXChildren, so AXWindows is seeded
        // explicitly - after the children, de-duplicated with CFEqual by `union_roots`.
        let children = children_of(&root);
        let windows = windows_of(&root);
        let roots = union_roots(&children, &windows);

        let mut elements: Vec<Element> = Vec::new();
        let mut text_parts: Vec<String> = Vec::new();
        let mut handle: u32 = 0;
        let mut nodes_seen: usize = 0;
        let mut hidden: usize = 0;
        let mut truncated = false;

        let mut stack: Vec<(CFRetained<AXUIElement>, usize)> = Vec::new();
        for (el, depth) in descend(&roots, 0) {
            stack.push((el.clone(), depth));
        }

        while let Some((el, depth)) = stack.pop() {
            if nodes_seen >= limits.max_nodes {
                truncated = true;
                break;
            }
            // One increment per node. The reference increments twice (`ax.py:272-273`); see
            // trap 4 in the module docs for why that is not copied.
            nodes_seen += 1;

            if depth > limits.max_depth {
                // Skip the node AND its subtree: the reference `continue`s here, so deeper
                // nodes are never even counted.
                continue;
            }

            // SAFETY: every element here is live: it came from a live app element's children.
            let role = match unsafe { attrs::role(&el) } {
                empty if empty.is_empty() => UNKNOWN_ROLE.to_string(),
                role => role,
            };
            // SAFETY: live element, as above.
            let names = unsafe { attrs::action_names(&el) };
            // SAFETY: as above.
            let title = unsafe { attrs::title(&el) };
            // `AXTitle` and `AXDescription` are read separately on purpose: Calculator's
            // digits are `AXTitle=""` with `AXDescription="2"`.
            // SAFETY: as above.
            let desc = unsafe { attrs::description(&el) };

            // Settability is a round trip, so only the roles where it changes the answer are
            // probed (the reference's SETTABLE_ROLES).
            // SAFETY: live element, as above.
            let settable = SETTABLE_ROLES.contains(&role.as_str())
                && unsafe { attrs::is_settable(&el, attrs::name::VALUE) };
            let typeable = EDITABLE_ROLES.contains(&role.as_str());
            let clickable = names.iter().any(|n| n == "AXPress" || n == "AXConfirm");
            // SAFETY: live element, as above.
            let mut addressable = earns_index(clickable, settable, typeable, &role, unsafe {
                attrs::enabled(&el)
            });

            // Read last: two more round trips, asked only of a node that has already cleared
            // actionability and enablement.
            let rect = if addressable {
                // SAFETY: live element, as above.
                let rect = unsafe { attrs::rect(&el) };
                if on_screen(rect, bounds) {
                    rect
                } else {
                    // Otherwise addressable, but not displayed. `hidden` counts exactly these
                    // nodes, so the drop rate by the on-screen test alone stays visible.
                    addressable = false;
                    hidden += 1;
                    None
                }
            } else {
                None
            };

            // `AXChildren` is read once per node and used for both the options list and the
            // descent. (The reference reads it twice on a `select` node - `ax.py:336` and
            // `ax.py:362` - which cannot produce a different answer unless the tree changes
            // inside one node's processing, and costs a round trip.)
            let mut children: Option<Vec<CFRetained<AXUIElement>>> = None;

            if addressable {
                // SAFETY: live element, as above.
                let read = unsafe { attrs::value(&el) };
                // SAFETY: as above.
                let value = if read.is_empty() {
                    unsafe { attrs::placeholder_value(&el) }
                } else {
                    read
                };
                let label_text = label_of(&title, &desc, &value, &role);
                let kind = kind_of(&role, typeable, settable, clickable);
                handle += 1;

                let (options, current_value) = if kind == Kind::Select {
                    let kids = children.get_or_insert_with(|| children_of(&el));
                    (options_of(kids), non_empty(truncate(&value, LABEL_LIMIT)))
                } else {
                    (Vec::new(), None)
                };
                let (selected, checked) = selection_of(&el, &names, &role);

                elements.push(Element {
                    node: handle,
                    kind,
                    label: label_text,
                    role: role.clone(),
                    value: non_empty(truncate(&value, VALUE_LIMIT)),
                    rect,
                    settable,
                    options,
                    current_value,
                    selected,
                    checked,
                });
            } else if role == "AXStaticText" {
                // A node that is not addressable still says something about the screen, and
                // the text is how the model gets those words.
                // SAFETY: live element, as above.
                let contribution = if !title.is_empty() {
                    title
                } else if !desc.is_empty() {
                    desc
                } else {
                    // SAFETY: live element, as above.
                    unsafe { attrs::value(&el) }
                };
                if !contribution.is_empty() {
                    text_parts.push(contribution);
                }
            } else if !title.is_empty()
                && matches!(role.as_str(), "AXHeading" | "AXGroup" | "AXWindow")
            {
                text_parts.push(title);
            }

            let kids = children.unwrap_or_else(|| children_of(&el));
            for (child, child_depth) in descend(&kids, depth + 1) {
                stack.push((child.clone(), child_depth));
            }
        }

        let text = join_text(&text_parts, limits.text_budget);

        // The window's title, else the first root's, else the app's own name (`ax.py:365-369`).
        // A `window_title` for window[0] is asked for even when it is empty, because the
        // reference then falls back to the app label rather than to the first root.
        // SAFETY: live elements, per this module's contract.
        let window_title = if !windows.is_empty() {
            unsafe { attrs::title(&windows[0]) }
        } else if !roots.is_empty() {
            unsafe { attrs::title(&roots[0]) }
        } else {
            String::new()
        };
        let title = if window_title.is_empty() {
            label.clone()
        } else {
            window_title
        };

        Ok(ElementTable {
            app: label,
            title,
            text,
            elements,
            nodes_seen,
            hidden,
            truncated,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }
}

#[cfg(target_os = "macos")]
pub use macos::observe;

// Tests unwrap freely on purpose: a panic here IS the failure report.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── the pure decisions: these run on ubuntu CI ─────────────────────────

    #[test]
    fn the_role_tables_are_the_references_verbatim() {
        // Asserted rather than trusted, because these three lists ARE the walk's behaviour.
        // A tidy-up that "simplifies" one of them changes which elements are probed, which
        // are typeable and which are selects - and the parity diff would report it as twelve
        // unrelated element differences. Source: ax.py:53-71.
        assert_eq!(
            SETTABLE_ROLES,
            &[
                "AXTextField",
                "AXTextArea",
                "AXComboBox",
                "AXSearchField",
                "AXSlider",
                "AXStepper",
                "AXCheckBox",
                "AXRadioButton",
            ]
        );
        assert_eq!(
            EDITABLE_ROLES,
            &["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"]
        );
        assert_eq!(
            KIND_BY_ROLE,
            &[
                ("AXTextField", Kind::Fill),
                ("AXTextArea", Kind::Fill),
                ("AXSearchField", Kind::Fill),
                ("AXComboBox", Kind::Fill),
                ("AXPopUpButton", Kind::Select),
                ("AXMenuButton", Kind::Select),
            ]
        );
    }

    #[test]
    fn the_kind_map_decides_before_anything_else_does() {
        // A popup that also reports AXPress is still a select: the map wins, because a
        // popup's meaning is "choose one of these", not "press me".
        assert_eq!(kind_of("AXPopUpButton", false, false, true), Kind::Select);
        assert_eq!(kind_of("AXComboBox", true, false, true), Kind::Fill);
        // Not in the map: a field that can be typed into is a fill even though nothing about
        // its value is writable.
        assert_eq!(kind_of("AXSomeField", true, false, false), Kind::Fill);
        // A slider is settable and not clickable: a value, not a button.
        assert_eq!(kind_of("AXSlider", false, true, false), Kind::Fill);
        // Settable AND clickable stays a click - pressing it is the interaction.
        assert_eq!(kind_of("AXButton", false, true, true), Kind::Click);
        // Nothing special: a plain button.
        assert_eq!(kind_of("AXButton", false, false, true), Kind::Click);
        // The reference's map covers every typeable role, so the typeable branch is
        // unreachable with the two tables above. Kept and driven directly here, because it
        // is the reference's decision rule and it is what would catch a widened editable set.
        assert_eq!(kind_of("AXMadeUpEditable", true, true, true), Kind::Fill);
    }

    #[test]
    fn an_absent_enabled_attribute_leaves_the_element_addressable() {
        // Trap 1. `None` is what a failed probe answers, and it is not `false`.
        assert!(earns_index(true, false, false, "AXButton", None));
        assert!(earns_index(false, true, false, "AXTextField", None));
        assert!(earns_index(false, false, true, "AXTextField", None));
    }

    #[test]
    fn a_disabled_element_is_not_addressable() {
        assert!(!earns_index(true, false, false, "AXButton", Some(false)));
        assert!(!earns_index(false, true, false, "AXTextField", Some(false)));
    }

    #[test]
    fn an_unknown_role_never_earns_an_index() {
        // The role stands in for "we could not read what this is". A node we cannot identify
        // must not become a target, however action-ready it looks.
        assert!(!earns_index(true, false, false, "AXUnknown", None));
        assert!(!earns_index(false, true, true, "AXUnknown", Some(true)));
    }

    #[test]
    fn nothing_actionable_never_earns_an_index() {
        // A static text node on a perfectly visible part of the screen.
        assert!(!earns_index(
            false,
            false,
            false,
            "AXStaticText",
            Some(true)
        ));
    }

    #[test]
    fn labels_fall_back_in_the_references_order() {
        assert_eq!(label_of("Save", "desc", "value", "AXButton"), "Save");
        assert_eq!(label_of("", "2", "value", "AXButton"), "2");
        assert_eq!(label_of("", "", "typed text", "AXTextField"), "typed text");
        // The last resort strips "AX" everywhere, not just once, because the reference uses
        // str.replace - a crafted role shows the difference.
        assert_eq!(label_of("", "", "", "AXButton"), "Button");
        assert_eq!(label_of("", "", "", "AXTextAXField"), "TextField");
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // 130 CJK characters are 390 bytes. A byte slice would produce 40 characters that
        // are not even valid UTF-8, and every label in a translated app would differ.
        let label: String = "日".repeat(130);
        assert_eq!(label.len(), 390, "the fixture is multi-byte");
        let cut = truncate(&label, 120);
        assert_eq!(cut.chars().count(), 120);
        assert_eq!(cut.len(), 360);
        // Under the cap, nothing is touched.
        assert_eq!(truncate("Save", 120), "Save");
        assert_eq!(truncate("", 120), "");
    }

    #[test]
    fn a_missing_or_degenerate_rect_is_not_on_screen() {
        // The closed-menu case: no position at all.
        assert!(!on_screen(None, Some((1440.0, 900.0))));
        assert!(!on_screen(
            Some(Rect {
                x: 10.0,
                y: 10.0,
                w: 0.0,
                h: 40.0
            }),
            Some((1440.0, 900.0))
        ));
        assert!(!on_screen(
            Some(Rect {
                x: 2000.0,
                y: 100.0,
                w: 100.0,
                h: 40.0
            }),
            Some((1440.0, 900.0))
        ));
        assert!(on_screen(
            Some(Rect {
                x: 100.0,
                y: 100.0,
                w: 100.0,
                h: 40.0
            }),
            Some((1440.0, 900.0))
        ));
    }

    #[test]
    fn windows_come_after_children_and_only_once() {
        // Rule R3. The union is by equality, which for AXUIElement is CFEqual - the fixture
        // here uses strings, and the ordering rule is what is under test.
        let children = vec!["menu bar", "window 1"];
        let windows = vec!["window 1", "window 2"];
        assert_eq!(
            union_roots(&children, &windows),
            vec!["menu bar", "window 1", "window 2"],
            "children first, then unseen windows, in the order given"
        );
        // A window already among the children must not be walked twice: every element under
        // it would be emitted twice with two different handles.
        assert_eq!(union_roots(&["w"], &["w"]), vec!["w"]);
        assert_eq!(union_roots::<&str>(&[], &["w"]), vec!["w"]);
        assert!(union_roots::<&str>(&[], &[]).is_empty());
    }

    #[test]
    fn children_are_pushed_in_reverse_so_they_pop_in_order() {
        let mut stack: Vec<(&str, usize)> = descend(&["first", "second", "third"], 1)
            .map(|(item, depth)| (*item, depth))
            .collect();
        assert_eq!(
            stack.pop(),
            Some(("first", 1)),
            "the first child must come off the stack first"
        );
        assert_eq!(stack.pop(), Some(("second", 1)));
        assert_eq!(stack.pop(), Some(("third", 1)));
        assert_eq!(stack.pop(), None);
        // The depth travels with the item: roots are 0, children are depth + 1.
        assert_eq!(
            descend(&["root"], 0)
                .map(|(item, depth)| (*item, depth))
                .collect::<Vec<_>>(),
            vec![("root", 0)]
        );
    }

    /// A node in a synthetic tree: a name and its children.
    struct Node(&'static str, Vec<Node>);

    /// Walk a synthetic tree with the same stack discipline the real walk uses.
    fn preorder(roots: &[Node]) -> Vec<(&'static str, usize)> {
        let mut out = Vec::new();
        let mut stack: Vec<(&Node, usize)> = descend(roots, 0).collect();
        while let Some((node, depth)) = stack.pop() {
            out.push((node.0, depth));
            for (child, child_depth) in descend(&node.1, depth + 1) {
                stack.push((child, child_depth));
            }
        }
        out
    }

    #[test]
    fn the_walk_is_depth_first_pre_order_in_reported_order() {
        // The order elements are emitted in IS the screen's reading order, and the parity
        // harness compares it element by element. Never sorted: the menu bar is first
        // because Finder reports it first, and a node's subtree is finished before its next
        // sibling - which is why a menu's own item can appear before the menu next to it.
        let tree = vec![
            Node(
                "menu bar",
                vec![
                    Node("apple", vec![]),
                    Node("file", vec![Node("new window", vec![])]),
                ],
            ),
            Node("window", vec![Node("toolbar", vec![])]),
        ];
        assert_eq!(
            preorder(&tree),
            vec![
                ("menu bar", 0),
                ("apple", 1),
                ("file", 1),
                ("new window", 2),
                // Both roots are depth 0: roots are seeded at the top, not below each other,
                // and the window is a sibling of the menu bar rather than a child of it.
                ("window", 0),
                ("toolbar", 1),
            ]
        );
    }

    #[test]
    fn text_is_joined_with_the_references_separator() {
        let parts = vec!["Hello".to_string(), "world".to_string()];
        assert_eq!(join_text(&parts, 6000), "Hello \u{b7} world");
        assert_eq!(join_text(&[], 6000), "");
        // The budget is characters, and it is applied after joining - the separator counts.
        assert_eq!(join_text(&parts, 5), "Hello");
        assert_eq!(join_text(&parts, 7), "Hello \u{b7}");
    }

    // ── live: macOS, an Accessibility grant, and Finder ────────────────────
    //
    // Tolerant of window state on purpose - Finder may have nothing open - and strict about
    // the invariants that must hold on any screen.

    #[cfg(target_os = "macos")]
    mod live {
        use crate::app::ResolveError;
        use crate::walk::{observe, ObserveError};
        use crate::Limits;

        #[test]
        fn a_finder_walk_starts_at_the_menu_bar_and_walks_in_order() {
            let table =
                observe(Some("Finder"), Limits::default()).expect("Finder is always running");

            assert!(!table.elements.is_empty(), "a Finder walk found nothing");
            assert!(!table.app.is_empty(), "the walk has no app name");
            assert!(
                !table.title.is_empty(),
                "the title falls back to the app name, so it is never empty"
            );
            assert_eq!(
                table.elements[0].role, "AXMenuBarItem",
                "a Finder walk starts at the menu bar, because that is the app's first child"
            );
            // Not the first eight: the walk finishes the File menu's subtree before the next
            // menu bar item, which is where a window-derived button appears.
            let menubar = table
                .elements
                .iter()
                .take(6)
                .filter(|e| e.role == "AXMenuBarItem")
                .count();
            assert!(
                menubar >= 3,
                "only {menubar} menu bar items in the first six elements, out of {}",
                table.elements.len()
            );

            for (i, e) in table.elements.iter().enumerate() {
                assert_eq!(
                    e.node as usize,
                    i + 1,
                    "handles are 1-based and assigned in walk order"
                );
                assert!(e.role.starts_with("AX"), "role {:?}", e.role);
                assert!(!e.label.is_empty(), "element {} has no label", e.node);
                let rect = e.rect.expect("an addressable element has a rect");
                assert!(
                    rect.w >= 1.0 && rect.h >= 1.0,
                    "element {} has a degenerate rect {rect:?}",
                    e.node
                );
            }

            // The on-screen test is what separates an addressable element from a visible
            // one, so a Finder walk always rejects something: closed menu items, hidden tab
            // content, collapsed rows.
            assert!(
                table.hidden > 0,
                "nothing was rejected by the on-screen test"
            );
            assert!(!table.truncated, "Finder's tree is far inside the node cap");
        }

        #[test]
        fn the_counter_counts_one_per_node_and_the_cap_truncates() {
            // The agreed divergence, asserted where it is visible: the reference reports
            // twice the nodes it walked (`ax.py:272-273`) and this port reports once. A
            // regression to the reference's double increment shows up here immediately: with
            // a cap of 5 it would stop after three nodes and report 6.
            let full =
                observe(Some("Finder"), Limits::default()).expect("Finder is always running");
            assert!(
                full.nodes_seen > full.elements.len(),
                "{} nodes and {} elements: most nodes are never addressable",
                full.nodes_seen,
                full.elements.len()
            );

            let capped = observe(
                Some("Finder"),
                Limits {
                    max_nodes: 5,
                    ..Limits::default()
                },
            )
            .expect("Finder is always running");
            assert_eq!(capped.nodes_seen, 5, "the cap must count nodes, once each");
            assert!(capped.truncated, "hitting the cap must be reported");
            assert!(
                capped.elements.len() <= 5,
                "an element was emitted from a node that was never counted"
            );
        }

        #[test]
        fn an_unknown_app_is_an_error_not_an_empty_screen() {
            // Rule R10: "no such app" must never be expressible as an app with no UI.
            match observe(Some("NoSuchAppZzz"), Limits::default()) {
                Err(ObserveError::App(ResolveError::NoMatch(q))) => assert_eq!(q, "NoSuchAppZzz"),
                other => panic!("expected a resolve error, got {other:?}"),
            }
        }

        #[test]
        fn the_focused_app_walks_or_says_there_is_none() {
            // What is frontmost during a test run is not predictable, and on this machine one
            // real answer is `AppNotAnswering`: PI-Desktop returns -25204 to every attribute
            // (docs/memory.md D8). All three outcomes are honest; what is never allowed is an
            // empty table pretending to be a screen.
            match observe(
                None,
                Limits {
                    max_nodes: 8,
                    ..Limits::default()
                },
            ) {
                Ok(table) => {
                    assert!(!table.app.is_empty(), "a walk with no app name");
                    assert!(table.nodes_seen > 0, "the walk visited nothing");
                }
                Err(ObserveError::NoFocusedApp) => {}
                Err(ObserveError::AppNotAnswering { attribute, code }) => {
                    assert_eq!(attribute, "AXRole");
                    assert_ne!(code, 0, "an error code of 0 is not a failure");
                }
                Err(other) => panic!("unexpected failure: {other}"),
            }
        }
    }
}
