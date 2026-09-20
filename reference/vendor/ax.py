"""Desktop observation and execution for Jev.

The decision layer is NOT reimplemented here. `jev_ultrafast.model.choose()` supplies
Jev's operation + element choice from the candidates built below, exactly as it does
for the browser. This module only does the two jobs the browser layer used to do:

  OBSERVE  macOS Accessibility tree -> the same `actions` list `model.action_space()` eats
  EXECUTE  Jev's chosen index       -> AXPress / AXValue write on that exact element

The mapping to jev-ultrafast's model, for reference:

    browser (CDP)                     desktop (AX)
    ------------------------------    ------------------------------------------
    button "Change ticket type"  <->  AXButton      "Save"
    combobox "Where from?"       <->  AXComboBox    "Where from?" value="San Francisco"
    textbox  "Where to?"         <->  AXTextField   "Where to?"   value=""
    kind=click|fill|select       <->  derived from AXRole + which actions the element exposes
    node (DOM backend node id)   <->  a per-walk integer handle we keep the AXUIElement under

Two rules are load-bearing, both taken from the accessibility contract:

  1. An element gets an index only if it is ACTIONABLE and ENABLED. An element you can
     see but not use must never become a target, or Jev is invited to pick something
     that cannot be executed.
  2. A messaging timeout is set on the app element. You cannot cancel a blocked
     AXUIElementCopyAttributeValue, so that timeout is the only thing stopping one
     unresponsive app from pinning the walk forever.
"""

from __future__ import annotations

import hashlib
import time

import ApplicationServices as AX
import Quartz

# ── tunables ───────────────────────────────────────────────────────────────
MAX_DEPTH = 25          # cua's default; deep enough for real apps
MAX_NODES = 2000        # bounds worst-case walk cost
MESSAGING_TIMEOUT = 2.0  # seconds, per element
TEXT_BUDGET = 6000      # characters of visible text handed to the model

# Roles that are clickable.
CLICK_ROLES = {
    "AXButton", "AXMenuItem", "AXMenuBarItem", "AXLink", "AXCheckBox", "AXRadioButton",
    "AXTab", "AXDisclosureTriangle", "AXPopUpButton", "AXMenuButton", "AXDockItem",
    "AXCell", "AXRow", "AXIncrementor", "AXSlider", "AXHandle", "AXImage",
}

# Roles whose AXValue can be written directly. Probing settability is only done for
# these, because the check costs an extra round trip per element.
SETTABLE_ROLES = {
    "AXTextField", "AXTextArea", "AXComboBox", "AXSearchField",
    "AXSlider", "AXStepper", "AXCheckBox", "AXRadioButton",
}

# Roles that accept typed text. A rich-text or web-backed editor often reports
# AXValue as NOT settable even though it is perfectly typeable, so these stay
# addressable and the executor falls back to focusing the element and sending
# real keystrokes. Without this the main editing surface gets no index at all.
EDITABLE_ROLES = {"AXTextField", "AXTextArea", "AXSearchField", "AXComboBox"}

# Roles that offer a fixed list of choices.
CHOICE_ROLES = {"AXPopUpButton", "AXMenuButton", "AXComboBox"}

KIND_BY_ROLE = {
    "AXTextField": "fill", "AXTextArea": "fill", "AXSearchField": "fill",
    "AXComboBox": "fill",
    "AXPopUpButton": "select", "AXMenuButton": "select",
}


# ── small AX helpers ───────────────────────────────────────────────────────

def _attr(el, name):
    """One attribute read. Returns None on any failure or explicit null."""
    try:
        err, value = AX.AXUIElementCopyAttributeValue(el, name, None)
    except Exception:
        return None
    if err != 0 or value is None:
        return None
    try:
        if value == Quartz.kCFNull:
            return None
    except Exception:
        pass
    return value


def _text(el, name):
    v = _attr(el, name)
    if v is None:
        return ""
    if isinstance(v, str):
        return v
    if isinstance(v, (int, float, bool)):
        return str(v)
    return ""


def _children(el):
    """All children in one call. Falls back to the single-value accessor."""
    try:
        err, values = AX.AXUIElementCopyAttributeValues(el, "AXChildren", 0, 999, None)
        if err == 0 and values is not None:
            return list(values)
    except Exception:
        pass
    v = _attr(el, "AXChildren")
    return list(v) if isinstance(v, (list, tuple)) else []


def _actions(el):
    try:
        err, names = AX.AXUIElementCopyActionNames(el, None)
        if err == 0 and names is not None:
            return [str(n) for n in names]
    except Exception:
        pass
    return []


def _settable(el, attr_name="AXValue"):
    try:
        err, ok = AX.AXUIElementIsAttributeSettable(el, attr_name, None)
        return err == 0 and bool(ok)
    except Exception:
        return False


def _rect(el):
    """(x, y, w, h) in screen points, or None. Two extra round trips, so only asked
    for elements that already earned an index."""
    try:
        err, pos = AX.AXUIElementCopyAttributeValue(el, "AXPosition", None)
        if err != 0 or pos is None:
            return None
        err, size = AX.AXUIElementCopyAttributeValue(el, "AXSize", None)
        if err != 0 or size is None:
            return None
        # AXValueGetValue lives in ApplicationServices, not Quartz, and its out-param
        # is returned by pyobjc as a (ok, value) pair.
        ok_p, point = AX.AXValueGetValue(pos, AX.kAXValueCGPointType, None)
        ok_s, ext = AX.AXValueGetValue(size, AX.kAXValueCGSizeType, None)
        if not ok_p or not ok_s:
            return None
        return (float(point.x), float(point.y), float(ext.width), float(ext.height))
    except Exception:
        return None


def _screen_bounds():
    """Main display bounds in points: (w, h). One call per walk."""
    try:
        b = Quartz.CGDisplayBounds(Quartz.CGMainDisplayID())
        return float(b.size.width), float(b.size.height)
    except Exception:
        return None


def _on_screen(rect, bounds) -> bool:
    """True only for an element that is really being displayed.

    A closed menu reports no rect at all, hidden tab content sits off-screen, and
    collapsed rows are degenerate. All three would otherwise earn an index and
    invite Jev to target something it cannot use.
    """
    if rect is None:
        return False
    x, y, w, h = rect
    if w < 1 or h < 1:
        return False
    if not bounds:
        return True
    sw, sh = bounds
    return not (x + w <= 0 or y + h <= 0 or x >= sw or y >= sh)


# ── app resolution ─────────────────────────────────────────────────────────

def _focused_app():
    """The system-wide focused application element, or None."""
    try:
        system = AX.AXUIElementCreateSystemWide()
        app = _attr(system, "AXFocusedApplication")
        if app is not None:
            return app
    except Exception:
        pass
    return None


def running_apps() -> list[tuple[str, int]]:
    """(name, pid) for every app with a UI, frontmost first."""
    out = []
    try:
        for proc in Quartz.NSWorkspace.sharedWorkspace().runningApplications():
            if proc.activationPolicy() != 0:      # 0 = regular, has a Dock icon
                continue
            out.append((str(proc.localizedName()), int(proc.processIdentifier())))
    except Exception:
        pass
    front = Quartz.NSWorkspace.sharedWorkspace().frontmostApplication()
    if front:
        front_name = str(front.localizedName())
        out.sort(key=lambda pair: pair[0] != front_name)
    return out


def _app_element(app: str | None):
    """Resolve an app name (or the focused app) to (element, label, pid)."""
    if app:
        for name, pid in running_apps():
            if name.lower() == app.lower() or app.lower() in name.lower():
                el = AX.AXUIElementCreateApplication(pid)
                AX.AXUIElementSetMessagingTimeout(el, MESSAGING_TIMEOUT)
                return el, name, pid
        raise LookupError(f"no running app matches '{app}'")
    el = _focused_app()
    if el is None:
        raise LookupError("no focused application")
    try:
        pid = AX.AXUIElementGetPid(el, None)[1]
    except Exception:
        pid = -1
    label = ""
    for name, cand in running_apps():
        if cand == pid:
            label = name
            break
    AX.AXUIElementSetMessagingTimeout(el, MESSAGING_TIMEOUT)
    return el, (label or "focused app"), pid


# ── OBSERVE ────────────────────────────────────────────────────────────────

def observe(app: str | None = None, max_nodes: int = MAX_NODES) -> dict:
    """Walk the app's accessibility tree into the table Jev chooses from.

    Returns the state dict `jev_ultrafast.model.choose()` expects, plus:
      nodes   handle -> AXUIElement, so a chosen index can be executed
      fingerprint  hash of the table, to tell whether the screen actually changed
    """
    started = time.perf_counter()
    root, label, pid = _app_element(app)
    bounds = _screen_bounds()

    # Background windows are not in AXChildren, so windows are seeded explicitly.
    roots = list(_children(root))
    windows = _attr(root, "AXWindows")
    if isinstance(windows, (list, tuple)):
        for w in windows:
            if w not in roots:
                roots.append(w)

    actions: list[dict] = []
    nodes: dict[str, object] = {}
    text_parts: list[str] = []
    handle = 0
    visited = 0
    hidden = 0
    truncated = False

    stack = [(el, 0) for el in reversed(roots)]
    while stack:
        el, depth = stack.pop()
        if visited >= max_nodes:
            truncated = True
            break
        visited += 1
        visited += 1
        if depth > MAX_DEPTH:
            continue

        role = _text(el, "AXRole") or "AXUnknown"
        names = _actions(el)
        title = _text(el, "AXTitle")
        # AXTitle and AXDescription stay separate on purpose: Calculator's digits are
        # AXTitle="" with AXDescription="2".
        desc = _text(el, "AXDescription")

        settable = False
        if role in SETTABLE_ROLES:
            settable = _settable(el)
        # Typeable even when AXValue refuses the write: focusable field roles get
        # keystrokes instead, so the main editor is never missing from the table.
        typeable = role in EDITABLE_ROLES

        clickable = "AXPress" in names or "AXConfirm" in names
        addressable = (clickable or settable or typeable) and role != "AXUnknown"
        enabled = _attr(el, "AXEnabled")
        if addressable and enabled is False:
            addressable = False
        if addressable:
            # Only now is the rect read: it costs two round trips, so it is asked
            # for elements that already cleared actionability and enablement.
            rect = _rect(el)
            if not _on_screen(rect, bounds):
                addressable = False
                hidden += 1
        else:
            rect = None

        if addressable:
            value = _text(el, "AXValue") or _text(el, "AXPlaceholderValue")
            label_text = title or desc or value or role.replace("AX", "")
            kind = KIND_BY_ROLE.get(role)
            if kind is None:
                if typeable:
                    kind = "fill"
                else:
                    kind = "fill" if (settable and not clickable) else "click"
            if kind == "fill" and not settable:
                entry_typeable = True   # executor must type, not write AXValue
            else:
                entry_typeable = False
            handle += 1
            node_id = str(handle)
            entry = {
                "id": f"n{handle}",
                "node": node_id,
                "kind": kind,
                "label": label_text[:120],
                "role": role,
            }
            entry["rect"] = rect   # reused by execute(), so no second read
            if entry_typeable:
                entry["typeable"] = True
            if value:
                entry["value"] = value[:200]
            if kind == "select":
                entry["current_value"] = value[:120]
                options = []
                for child in _children(el)[:40]:
                    if _text(child, "AXRole") == "AXMenuItem":
                        options.append(_text(child, "AXTitle") or _text(child, "AXValue"))
                if not options:
                    # A closed popup keeps its items under AXChildren of the menu,
                    # which is often empty until pressed. Say so rather than lie.
                    options = []
                entry["options"] = [o for o in options if o]
            if "AXSelected" in names or role in ("AXTab", "AXRow", "AXCheckBox", "AXRadioButton"):
                sel = _attr(el, "AXSelected")
                if sel is not None:
                    entry["selected"] = bool(sel)
                val = _attr(el, "AXValue")
                if role in ("AXCheckBox", "AXRadioButton") and val is not None:
                    entry["checked"] = bool(val) if isinstance(val, (int, bool)) else None
            nodes[node_id] = el
            actions.append(entry)
        else:
            # Non-addressable but informative nodes still contribute context text.
            if role == "AXStaticText":
                t = title or desc or _text(el, "AXValue")
                if t:
                    text_parts.append(t)
            elif title and role in ("AXHeading", "AXGroup", "AXWindow"):
                text_parts.append(title)

        for child in reversed(_children(el)):
            stack.append((child, depth + 1))

    window_title = ""
    if isinstance(windows, (list, tuple)) and windows:
        window_title = _text(windows[0], "AXTitle")
    elif roots:
        window_title = _text(roots[0], "AXTitle")

    text = " · ".join(text_parts)[:TEXT_BUDGET]
    fingerprint = hashlib.sha256(
        ("|".join(f"{a['kind']}:{a['label']}:{a.get('value','')}" for a in actions)).encode()
    ).hexdigest()[:16]

    return {
        "url": f"app://{label}",
        "title": window_title or label,
        "text": text,
        "actions": actions,
        "nodes": nodes,
        "fingerprint": fingerprint,
        "app": label,
        "pid": pid,
        "elements_seen": visited,
        "hidden": hidden,
        "truncated": truncated,
        "elapsed_ms": round((time.perf_counter() - started) * 1000),
    }


# ── EXECUTE ────────────────────────────────────────────────────────────────

def _focus(pid: int) -> None:
    """Bring the target app forward. AX actions work on background windows, but
    synthetic keystrokes need the app frontmost to receive them."""
    try:
        app = Quartz.NSRunningApplication.runningApplicationWithProcessIdentifier_(pid)
        if app is not None:
            app.activateWithOptions_(1 << 1)
    except Exception:
        pass
def _key(event_type, keycode):
    ev = Quartz.CGEventCreateKeyboardEvent(None, keycode, event_type == Quartz.kCGEventKeyDown)
    Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)


def _select_all():
    """Cmd+A, so typing replaces instead of appending."""
    cmd = Quartz.kCGEventFlagMaskCommand
    for down in (True, False):
        ev = Quartz.CGEventCreateKeyboardEvent(None, 0, down)   # 0 = 'a'
        Quartz.CGEventSetFlags(ev, cmd)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
    time.sleep(0.05)


def _type_text(text: str) -> None:
    """Type literal text as unicode keyboard events.

    Used when AXValue refuses the write (rich text, web-backed editors) and when a
    secure input field blocks ordinary keycodes. Unicode strings go through the
    event itself, so no keyboard layout mapping is involved.
    """
    for chunk in (text[i:i + 20] for i in range(0, len(text), 20)):
        down = Quartz.CGEventCreateKeyboardEvent(None, 0, True)
        Quartz.CGEventKeyboardSetUnicodeString(down, len(chunk), chunk)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, down)
        time.sleep(0.01)
    try:
        Quartz.NSRunningApplication.runningApplicationWithProcessIdentifier_(pid).activateWithOptions_(1 << 1)
    except Exception:
        pass


def execute(action: dict, state: dict, text: str | None = None) -> dict:
    """Carry out Jev's choice on the exact element it picked.

    `action` is the entry from `state['actions']` that the chosen index resolved to.
    Nothing here invents coordinates or guesses targets.
    """
    node_id = str(action.get("node"))
    el = state["nodes"].get(node_id)
    if el is None:
        return {"ok": False, "detail": "the chosen element is no longer in this observation"}

    _focus(state.get("pid", -1))
    kind = action.get("kind")
    label = action.get("label", "")

    try:
        if kind == "fill":
            if text is None:
                return {"ok": False, "detail": "fill needs a value"}
            # AX writes are not keyboard events, so a secure input field does not
            # block them. Try that first; it is atomic and leaves no partial state.
            err = AX.AXUIElementSetAttributeValue(el, "AXValue", text)
            if err == 0:
                return {"ok": True, "detail": f"set '{label}' to {text!r} via AXValue"}
            # Refused. Focus the element and type for real. This is the path rich
            # text and web-backed editors need; they report AXValue as unwritable.
            focused = AX.AXUIElementSetAttributeValue(el, "AXFocused", True)
            if focused != 0:
                AX.AXUIElementPerformAction(el, "AXPress")
            time.sleep(0.12)
            _select_all()
            _type_text(text)
            return {
                "ok": True,
                "detail": f"typed {text!r} into '{label}' via keystrokes (AXValue refused, err {err})",
            }

            err = AX.AXUIElementSetAttributeValue(el, "AXValue", text)
            if err != 0:
                return {"ok": False, "detail": f"AXValue write failed (AX err {err})"}
            return {"ok": True, "detail": f"set '{label}' to {text!r}"}

        if kind == "select":
            wanted = (text or "").strip().lower()
            for child in _children(el):
                if _text(child, "AXRole") != "AXMenuItem":
                    continue
                option = (_text(child, "AXTitle") or _text(child, "AXValue")).strip().lower()
                if option and option == wanted:
                    err = AX.AXUIElementPerformAction(child, "AXPress")
                    if err == 0:
                        return {"ok": True, "detail": f"selected {option!r} in '{label}'"}
            # Nothing matched by name; fall back to the press itself so the menu opens
            # and the next observation can see the real options.
            if AX.AXUIElementPerformAction(el, "AXPress") == 0:
                return {"ok": True, "detail": f"opened '{label}' (no option matched {text!r} yet)"}
            return {"ok": False, "detail": f"could not select in '{label}'"}

        err = AX.AXUIElementPerformAction(el, "AXPress")
        if err == 0:
            return {"ok": True, "detail": f"pressed '{label}'"}
        # Some rows respond only to a click on the element itself.
        rect = _rect(el)
        if rect:
            x, y, w, h = rect
            point = Quartz.CGPointMake(x + w / 2, y + h / 2)
            for down, up in ((Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp),):
                for ev_type in (down, up):
                    ev = Quartz.CGEventCreateMouseEvent(None, ev_type, point, Quartz.kCGMouseButtonLeft)
                    Quartz.CGEventPost(Quartz.kCGHIDEventTap, ev)
            return {"ok": True, "detail": f"clicked '{label}' at ({x + w / 2:.0f}, {y + h / 2:.0f})"}
        return {"ok": False, "detail": f"AXPress failed (AX err {err}) and no rect available"}
    except Exception as exc:  # noqa: BLE001
        return {"ok": False, "detail": f"{type(exc).__name__}: {exc}"}


def describe(state: dict) -> str:
    """The indexed element table, rendered for a human or a model prompt."""
    lines = [f"{state['app']} — {state['title']}  ({state['elapsed_ms']}ms, "
             f"{len(state['actions'])} addressable of {state['elements_seen']} nodes)"]
    for i, a in enumerate(state["actions"], 1):
        role = a["role"].replace("AX", "").lower()
        val = f' ="{a["value"]}"' if a.get("value") else ""
        checked = ""
        if a.get("checked") is not None:
            checked = " [x]" if a["checked"] else " [ ]"
        if a.get("selected"):
            checked += " *selected*"
        lines.append(f"  [{i}] {role:14s} {a['label']}{val}{checked}")
    return "\n".join(lines)
