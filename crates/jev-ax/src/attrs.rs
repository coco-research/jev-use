//! Typed attribute reads. This is the walk's shopping list.
//!
//! Every accessibility read goes through [`raw`], which is the only place `AXError` is
//! interpreted. Everything above it returns a plain Rust type, so `walk.rs` never has to
//! know that `-25205` (`AttributeUnsupported`) is a normal answer while `-25204`
//! (`CannotComplete`) means the app is not responding at all.
//!
//! # Why the primitives live here rather than in the walk
//!
//! The walk's job is tree traversal and the addressability rule. If attribute plucking
//! lived beside it, the traversal logic would be buried in FFI calls and neither part
//! would be readable on its own. This module knows how to read one element; it knows
//! nothing about trees.
//!
//! # Parity
//!
//! [`as_text`] reproduces what the Python driver produces from pyobjc, including `"True"`
//! rather than `"true"` for a boolean and `"2880.0"` rather than `"2880"` for a whole
//! float. That is deliberate: rule R8 says the reference is the specification, and a
//! parity diff should show zero differences rather than a list of near-misses.

use std::ptr::{self, NonNull};

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFArray, CFBoolean, CFNumber, CFRetained, CFString, CFType};

use crate::Rect;

/// Attribute names, so a typo becomes a compile error rather than an empty string.
///
/// These are the subset the walk uses. Adding one here is cheap; passing a string literal
/// and getting `""` back from a misspelling is not.
pub mod name {
    /// The element's role, e.g. `AXButton`.
    pub const ROLE: &str = "AXRole";
    /// The accessible title. Often empty on custom-drawn controls.
    pub const TITLE: &str = "AXTitle";
    /// Fallback label when there is no title: Calculator's digits are `AXTitle=""` with
    /// `AXDescription="2"`, which is why the two are read separately.
    pub const DESCRIPTION: &str = "AXDescription";
    /// The current value.
    pub const VALUE: &str = "AXValue";
    /// Shown in an empty field.
    pub const PLACEHOLDER: &str = "AXPlaceholderValue";
    /// Whether the element accepts interaction.
    pub const ENABLED: &str = "AXEnabled";
    /// Top-left corner in screen points, as an `AXValue`.
    pub const POSITION: &str = "AXPosition";
    /// Width and height in screen points, as an `AXValue`.
    pub const SIZE: &str = "AXSize";
    /// Children of this element.
    pub const CHILDREN: &str = "AXChildren";
    /// Windows owned by the app, which are NOT always in `AXChildren`.
    pub const WINDOWS: &str = "AXWindows";
    /// Whether the element is currently selected.
    pub const SELECTED: &str = "AXSelected";
}

/// Read one attribute, raw.
///
/// Follows Core Foundation's Create Rule, so the returned value is owned and freed on
/// drop.
///
/// Returns the `AXError` code on failure rather than collapsing every failure to `None`,
/// because the caller sometimes needs to tell the two apart:
///
/// | Code | Constant | Means |
/// | --- | --- | --- |
/// | `-25205` | `AttributeUnsupported` | normal - an app element genuinely has no `AXPosition` |
/// | `-25204` | `CannotComplete` | the app did not answer in time |
/// | `-25211` | `APIDisabled` | this process lacks Accessibility permission |
///
/// A caller that treats all three the same concludes "this app has no UI" when the real
/// answer is "we are not allowed to look" or "the app is busy".
///
/// # Safety
/// `el` must be a live `AXUIElement`.
pub unsafe fn raw(el: &AXUIElement, attribute: &str) -> Result<CFRetained<CFType>, i32> {
    let key = CFString::from_str(attribute);
    let mut out: *const CFType = ptr::null();
    // SAFETY: `slot` points at a live, properly aligned local; the callee only writes it.
    let slot: NonNull<*const CFType> = unsafe { NonNull::new_unchecked(&mut out) };
    // SAFETY: `el` is live per this function's contract.
    let err = unsafe { el.copy_attribute_value(&key, slot) };
    if err.0 != 0 {
        return Err(err.0);
    }
    if out.is_null() {
        return Err(0);
    }
    // SAFETY: non-null checked, and CopyAttributeValue returned a +1 reference.
    Ok(unsafe { CFRetained::from_raw(NonNull::new_unchecked(out as *mut CFType)) })
}

/// Best-effort string view of any Core Foundation value.
///
/// Covers the three shapes the accessibility API returns for the attributes this crate
/// reads: a string, a number, and a boolean. Anything else - an array, an `AXValue`
/// holding geometry - becomes an empty string, because rendering it as text would be a
/// guess.
///
/// The boolean case prints `"True"`/`"False"` to match the Python reference. See the
/// module docs.
#[must_use]
pub fn as_text(v: &CFType) -> String {
    if let Some(s) = v.downcast_ref::<CFString>() {
        return s.to_string();
    }
    if let Some(b) = v.downcast_ref::<CFBoolean>() {
        return if b.as_bool() {
            "True".to_string()
        } else {
            "False".to_string()
        };
    }
    if let Some(n) = v.downcast_ref::<CFNumber>() {
        // Integers first: a count of 3 should print "3", not "3.0".
        if let Some(i) = n.as_i64() {
            return i.to_string();
        }
        if let Some(f) = n.as_f64() {
            return python_float(f);
        }
    }
    String::new()
}

/// Format an `f64` the way Python's `str()` does.
///
/// Python prints `str(2880.0)` as `"2880.0"`; Rust's default `Display` prints `"2880"`.
/// The difference is invisible until a parity diff flags every float in the element
/// table, so it is handled here once rather than at every call site.
#[must_use]
pub fn python_float(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// Read an attribute and coerce it to text. Empty string when absent or unreadable.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn text(el: &AXUIElement, attribute: &str) -> String {
    // SAFETY: forwarded from this function's contract.
    match unsafe { raw(el, attribute) } {
        Ok(v) => as_text(&v),
        Err(_) => String::new(),
    }
}

/// The element's role, e.g. `AXButton`. Empty when unreadable.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn role(el: &AXUIElement) -> String {
    unsafe { text(el, name::ROLE) }
}

/// The accessible title, or empty when the element has none.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn title(el: &AXUIElement) -> String {
    unsafe { text(el, name::TITLE) }
}

/// The accessible description, kept separate from the title on purpose.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn description(el: &AXUIElement) -> String {
    unsafe { text(el, name::DESCRIPTION) }
}

/// The current value, or empty.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn value(el: &AXUIElement) -> String {
    unsafe { text(el, name::VALUE) }
}

/// The placeholder shown in an empty field, or empty.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn placeholder_value(el: &AXUIElement) -> String {
    unsafe { text(el, name::PLACEHOLDER) }
}

/// Whether the element is enabled, or `None` when the attribute is absent.
///
/// `None` is not `false`. An element with no `AXEnabled` attribute is not a disabled
/// element, and the walk must not treat it as one.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn enabled(el: &AXUIElement) -> Option<bool> {
    // SAFETY: forwarded from this function's contract.
    let v = unsafe { raw(el, name::ENABLED) }.ok()?;
    if let Some(b) = v.downcast_ref::<CFBoolean>() {
        return Some(b.as_bool());
    }
    // Some elements report enabled as a number rather than a boolean.
    v.downcast_ref::<CFNumber>()
        .and_then(|n| n.as_i64())
        .map(|i| i != 0)
}

/// Read a geometry attribute (`AXPosition` or `AXSize`).
///
/// # Safety
/// `el` must be a live `AXUIElement`, and `kind` must match the attribute: `CGPoint` for
/// `AXPosition`, `CGSize` for `AXSize`. Passing the wrong kind returns `None` rather than
/// garbage, because `AXValueGetValue` validates the type before writing.
unsafe fn geometry(el: &AXUIElement, attribute: &str, kind: AXValueType) -> Option<(f64, f64)> {
    // SAFETY: forwarded from this function's contract.
    let v = unsafe { raw(el, attribute) }.ok()?;
    let av = v.downcast_ref::<AXValue>()?;
    // Both CGPoint and CGSize are two contiguous f64s, so one buffer serves either.
    let mut buf = [0.0f64; 2];
    // SAFETY: `buf` is two contiguous f64s, which is the layout AXValueGetValue writes for
    // a CGPoint or a CGSize. The callee validates `kind` before writing.
    let ok = unsafe {
        av.value(
            kind,
            NonNull::new_unchecked(buf.as_mut_ptr().cast::<std::ffi::c_void>()),
        )
    };
    if ok {
        Some((buf[0], buf[1]))
    } else {
        None
    }
}

/// The element's top-left corner in screen points, or `None`.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn position(el: &AXUIElement) -> Option<(f64, f64)> {
    unsafe { geometry(el, name::POSITION, AXValueType::CGPoint) }
}

/// The element's width and height in screen points, or `None`.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn size(el: &AXUIElement) -> Option<(f64, f64)> {
    unsafe { geometry(el, name::SIZE, AXValueType::CGSize) }
}

/// The element's screen rectangle, or `None` when either half is unreadable.
///
/// Two round trips, so the walk only asks for this once an element has already earned an
/// index. Rule R1 depends on the result: a half-read rect must not become a whole one.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn rect(el: &AXUIElement) -> Option<Rect> {
    // SAFETY: forwarded from this function's contract.
    let (x, y) = unsafe { position(el) }?;
    // SAFETY: as above.
    let (w, h) = unsafe { size(el) }?;
    Some(Rect { x, y, w, h })
}

/// The actions this element supports, e.g. `["AXPress", "AXShowMenu"]`.
///
/// An element is clickable iff this contains `AXPress`. That is how the walk decides,
/// rather than by guessing from the role.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn action_names(el: &AXUIElement) -> Vec<String> {
    let mut arr: *const CFArray = ptr::null();
    // SAFETY: `slot` points at a live local; the callee only writes it.
    let slot: NonNull<*const CFArray> = unsafe { NonNull::new_unchecked(&mut arr) };
    // SAFETY: `el` is live per this function's contract.
    let err = unsafe { el.copy_action_names(slot) };
    if err.0 != 0 || arr.is_null() {
        return Vec::new();
    }
    // SAFETY: non-null checked; copy_action_names returned a +1 reference, so owning it
    // here balances that and frees on drop.
    let owned: CFRetained<CFArray> =
        unsafe { CFRetained::from_raw(NonNull::new_unchecked(arr as *mut CFArray)) };

    let mut out = Vec::with_capacity(owned.count() as usize);
    for i in 0..owned.count() {
        // SAFETY: index is within count.
        let item = unsafe { owned.value_at_index(i) };
        if item.is_null() {
            continue;
        }
        // SAFETY: copy_action_names returns an array of CFStrings.
        let s = unsafe { &*(item as *const CFString) };
        out.push(s.to_string());
    }
    out
}

/// Whether writing `attribute` on this element would succeed.
///
/// Used to tell an editable field from a read-only one. Probing costs a round trip, so
/// the walk only asks for roles where it matters.
///
/// # Safety
/// `el` must be a live `AXUIElement`.
#[must_use]
pub unsafe fn is_settable(el: &AXUIElement, attribute: &str) -> bool {
    let key = CFString::from_str(attribute);
    let mut out: u8 = 0;
    // SAFETY: `slot` points at a live local; the callee only writes it.
    let slot: NonNull<u8> = unsafe { NonNull::new_unchecked(&mut out) };
    // SAFETY: `el` is live per this function's contract.
    let err = unsafe { el.is_attribute_settable(&key, slot) };
    err.0 == 0 && out != 0
}

// ── tests ──────────────────────────────────────────────────────────────────
//
// The coercion tests need CoreFoundation; the live ones need Accessibility permission and
// Finder, which is always running. Both run on macOS only, which means CI cannot execute
// them - the known gap in `.metagpt/GATE.json`. The pre-push hook covers them locally;
// nothing covers them remotely yet.
//
// Tests unwrap freely on purpose: a panic here IS the failure report.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── coercion ───────────────────────────────────────────────────────────

    #[test]
    fn a_string_coerces_to_itself() {
        let s = CFString::from_str("AXButton");
        let t: &CFType = &s;
        assert_eq!(as_text(t), "AXButton");
    }

    #[test]
    fn a_boolean_prints_python_style() {
        // "True", not "true". Deliberate - module docs, rule R8.
        let t: &CFType = CFBoolean::new(true);
        let f: &CFType = CFBoolean::new(false);
        assert_eq!(as_text(t), "True");
        assert_eq!(as_text(f), "False");
    }

    #[test]
    fn an_integer_prints_without_a_decimal_point() {
        // A count of 3 must be "3", not "3.0". Integers are checked first for this reason.
        let n = CFNumber::new_i64(3);
        let t: &CFType = &n;
        assert_eq!(as_text(t), "3");
    }

    #[test]
    fn a_whole_float_keeps_pythons_trailing_zero() {
        // Python prints str(2880.0) as "2880.0"; Rust's Display prints "2880". Without
        // this, a parity diff flags every float in the element table.
        assert_eq!(python_float(2880.0), "2880.0");
        assert_eq!(python_float(0.5), "0.5");
        assert_eq!(python_float(-12.0), "-12.0");
    }

    #[test]
    fn a_non_scalar_coerces_to_empty_rather_than_a_guess() {
        // An array rendered as text would be invented. Empty is the honest answer.
        // SAFETY: an empty array with default callbacks; nothing is read from it.
        let empty = unsafe { CFArray::new(None, std::ptr::null_mut(), 0, std::ptr::null()) }
            .expect("an empty CFArray should be creatable");
        let t: &CFType = &empty;
        assert_eq!(as_text(t), "");
    }

    /// The system-wide element: always present, needs no app open and no Accessibility
    /// permission, which makes it the one live target every test can rely on.
    fn system_wide() -> CFRetained<AXUIElement> {
        // SAFETY: the system-wide element is always available.
        unsafe { AXUIElement::new_system_wide() }
    }

    /// The Finder app element, or `None` when Finder is not running.
    ///
    /// Finder cannot be quit and it owns the menu bar that is always on screen, so it is a
    /// live target that needs nothing to be launched first. The lookup uses `pgrep` rather
    /// than this crate's own app list, so that testing `attrs` does not depend on `app`.
    fn finder() -> Option<CFRetained<AXUIElement>> {
        let out = std::process::Command::new("pgrep")
            .args(["-x", "Finder"])
            .output()
            .ok()?;
        let pid: i32 = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()?
            .trim()
            .parse()
            .ok()?;
        // SAFETY: the pid came from a live process.
        Some(unsafe { AXUIElement::new_application(pid) })
    }

    /// Re-type an element-valued attribute read.
    ///
    /// `AXMenuBar`, `AXWindows` and `AXChildren` hand back elements. T4 grows a real
    /// accessor for that; the tests do it locally so that T3 needs no API it does not own.
    fn as_element(v: &CFType) -> CFRetained<AXUIElement> {
        // SAFETY: only called on element-valued attributes, which are +1 AXUIElement
        // references.
        unsafe {
            let p = v as *const CFType as *mut AXUIElement;
            CFRetained::retain(NonNull::new_unchecked(p))
        }
    }

    /// One element out of an array of elements, such as `AXChildren`.
    fn element_at(arr: &CFArray, i: isize) -> CFRetained<AXUIElement> {
        // SAFETY: `i` is within `count` at every call site, and these arrays hold
        // AXUIElements.
        unsafe {
            let p = arr.value_at_index(i);
            CFRetained::retain(NonNull::new_unchecked(p as *mut AXUIElement))
        }
    }

    #[test]
    fn the_system_wide_element_reports_its_role() {
        let wide = system_wide();
        // SAFETY: live element.
        let got = unsafe { role(&wide) };
        assert_eq!(
            got, "AXSystemWide",
            "an empty role means the read failed, not that the element has no role - check \
             that Accessibility permission is granted to whatever runs the tests"
        );
    }

    #[test]
    fn an_absent_attribute_is_none_rather_than_a_default() {
        // The system-wide element has no geometry, so these reads all come back
        // AttributeUnsupported. `None` must not become `false` or `(0, 0)`: (0, 0) is a
        // valid on-screen coordinate, so a defaulted rect would place an element on screen
        // that is not there. Rule R1 depends on this.
        let wide = system_wide();
        // SAFETY: live element.
        unsafe {
            assert_eq!(enabled(&wide), None, "an absent AXEnabled became a value");
            assert_eq!(position(&wide), None, "an absent AXPosition became (0, 0)");
            assert_eq!(size(&wide), None, "an absent AXSize became a value");
            assert_eq!(rect(&wide), None, "half a rect became a whole one");
        }
    }

    #[test]
    fn an_unsupported_attribute_keeps_its_error_code() {
        // This is what `raw` exists for, and why it returns `Result<_, i32>` rather than
        // `Option`. -25205 (AttributeUnsupported) and -25204 (CannotComplete) both mean
        // "no value", but one says the element has no such attribute while the other says
        // the app never answered. Callers act differently on each.
        let wide = system_wide();
        // SAFETY: live element.
        let got = unsafe { raw(&wide, name::POSITION) };
        assert_eq!(
            got.err(),
            Some(-25205),
            "AttributeUnsupported should reach the caller verbatim"
        );
    }

    #[test]
    fn a_menu_bar_is_reachable_in_one_read() {
        // Element-valued attributes are how the walk seeds windows and menus. Finder owns
        // the menu bar that is always on screen, so this needs nothing open first.
        let app = finder().expect("Finder should be running; it cannot be quit");
        // SAFETY: live element.
        let bar = unsafe { raw(&app, "AXMenuBar") }.expect("Finder exposes AXMenuBar");
        // SAFETY: live element.
        assert_eq!(unsafe { role(&as_element(&bar)) }, "AXMenuBar");
    }

    #[test]
    fn a_menu_bar_reports_the_actions_it_supports() {
        // Were action_names to return an empty Vec silently, the walk would conclude that
        // every element on screen is unclickable - a failure that looks like an app with
        // no controls rather than like a bug.
        let app = finder().expect("Finder should be running; it cannot be quit");
        // SAFETY: live element.
        let bar = as_element(&unsafe { raw(&app, "AXMenuBar") }.expect("Finder exposes AXMenuBar"));
        // SAFETY: live element.
        let actions = unsafe { action_names(&bar) };
        assert!(
            !actions.is_empty(),
            "the menu bar reported no actions; action_names is not reading the array"
        );
    }

    #[test]
    fn a_menu_bar_item_is_pressable() {
        // "Clickable" means "AXPress is in the action list" (rule R1), so one element that
        // really is pressable has to be seen reporting it, and every menu bar item is.
        // The full walk is T4; this only proves the read.
        let app = finder().expect("Finder should be running; it cannot be quit");
        // SAFETY: live element.
        let bar = as_element(&unsafe { raw(&app, "AXMenuBar") }.expect("Finder exposes AXMenuBar"));
        // SAFETY: live element.
        let items = unsafe { raw(&bar, name::CHILDREN) }.expect("a menu bar has children");
        // SAFETY: AXChildren holds elements.
        let items = unsafe { &*(CFRetained::as_ptr(&items).as_ptr() as *const CFArray) };
        assert!(items.count() > 0, "the menu bar reported no items");

        let mut pressable = 0;
        for i in 0..items.count() {
            let item = element_at(items, i);
            // SAFETY: live element.
            if unsafe { action_names(&item) }
                .iter()
                .any(|a| a == "AXPress")
            {
                pressable += 1;
            }
        }
        assert!(
            pressable > 0,
            "no menu bar item reported AXPress, out of {} items",
            items.count()
        );
    }

    #[test]
    fn settable_is_false_for_a_read_only_attribute() {
        // The positive case needs a writable attribute on an element we control, which is
        // T6's write path. What matters here is that probing an unwritable attribute does
        // not claim it can be written.
        let wide = system_wide();
        // SAFETY: live element.
        assert!(
            !unsafe { is_settable(&wide, name::ROLE) },
            "AXRole is not writable"
        );
    }

    #[test]
    fn a_bogus_attribute_name_is_an_error_not_a_panic() {
        let wide = system_wide();
        // SAFETY: live element, deliberately bogus attribute.
        assert!(
            unsafe { raw(&wide, "AXThisAttributeDoesNotExist") }.is_err(),
            "a bogus attribute returned a value"
        );
        // SAFETY: live element, deliberately bogus attribute.
        assert_eq!(unsafe { text(&wide, "AXThisAttributeDoesNotExist") }, "");
    }
}
