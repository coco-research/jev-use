"""Two crashes in Jev, and the gaps that let them through, patched at runtime.

Neither fix can go upstream: `jev_ultrafast` is `browser-use/jev-ultrafast`, a third-party
public repository this machine has READ-only access to, and `~/code/jev-computeruse` is not a
git repository at all. The same file jev-use pins by sha256 as its specification. So the
behaviour is corrected in place, and both are written down for whoever owns that repo.

BUG 1: a select action without `value` raised before Jev's first move.

    jev_ultrafast/model.py:76
        element["options"].append({"index": target, "label": action["label"],
                                   "value": action["value"]})

    `action["value"]` is a hard index. The desktop producer emits `value` only when the
    element HAS one (reference/vendor/ax.py:331-332, and the Rust port copies it faithfully at
    crates/jev-ax/examples/walk.rs:127). So any AX tree holding one AXPopUpButton whose
    AXValue is empty raises KeyError before Jev acts.

    Measured 2026-09-27: Finder with a popupbutton in its tree crashed on any goal, four
    goals across three apps all KeyError at that line. Chrome never reached it because it
    hides its page from the AX tree, so no select action is produced.

    Two lines above, the same function already guards the same field the safe way:
    `action.get("current_value", "")`. `value` is the one that was missed.

BUG 2: the honest "nothing to click" path crashed the printer.

    jev_desktop/agent.py:61 returns a row with `step`, `status`, `state`, `detail` and NO
    `confidence`, for the case where the screen has no addressable elements. The CLI then
    indexes it directly:

    ~/.local/bin/jev-use, in the row printer
        head = f"  {row['step']:2d} {mark} {row.get('operation','?'):<10} {row['confidence']:.2f}"

    Every neighbouring key on that line uses `.get()`; `confidence` is the one that does not.
    So the correct answer to "there is nothing to click" is a traceback.

    This is why BUG 1 appeared to still be present after it was fixed: the crash moved one
    frame later, from `'value'` to `'confidence'`, and the visible symptom looked identical.

WHY THE POSITION OF THIS IMPORT MATTERS
    `jev_desktop/__init__.py` imports this module AFTER `.agent`, and that order is load
    bearing. `agent.py` stubs around `jev_ultrafast/__init__.py` (which pulls in the browser
    harness this driver does not use) before importing `jev_ultrafast.model`. Imported
    before that, this module's own import of the same package fails, `_patch` returns
    silently, and the result is the worst outcome available: `patched: False`, no error, and
    the crash still happening. That was measured, not reasoned about.

WHAT IT DOES NOT DO
    No decision, label, index or option changes. `value` and `options` are write-only in
    `model.py`: `options` is created at :65, only its LENGTH is read at :75, and `value` is
    never read at all. An empty default cannot alter a choice.

    The confidence default is a display string, not a number, so a row that has no confidence
    cannot be mistaken for one that scored 0.00.
"""

from __future__ import annotations

import os

_FIX_SELECT = os.environ.get("JEV_FIX_SELECT_VALUE", "1") != "0"
_FIX_ROW = os.environ.get("JEV_FIX_BLOCKED_ROW", "1") != "0"
_TRACE = os.environ.get("JEV_FIX_TRACE") == "1"


def _trace(msg: str) -> None:
    if _TRACE:
        import sys

        print(f"[jev-fix] {msg}", file=sys.stderr)


def _patch_select_value() -> None:
    try:
        from jev_ultrafast import model as _model
    except Exception:  # noqa: BLE001 - never break an interpreter that is not Jev's
        _trace("could not import jev_ultrafast.model, select-value fix NOT applied")
        return

    original = getattr(_model, "action_space", None)
    if original is None or getattr(original, "_select_value_guarded", False):
        return

    def action_space(actions):
        safe = []
        patched = 0
        for action in actions:
            if isinstance(action, dict) and action.get("kind") == "select" and "value" not in action:
                action = dict(action)
                action["value"] = ""
                patched += 1
            safe.append(action)
        if patched:
            _trace(f"supplied an empty `value` on {patched} select action(s)")
        return original(safe)

    action_space._select_value_guarded = True  # type: ignore[attr-defined]
    _model.action_space = action_space
    _trace("select-value fix applied")


def _patch_blocked_row() -> None:
    """Give every row from DesktopAgent.step a `confidence` the printer can format."""
    try:
        from . import agent as _agent
    except Exception:  # noqa: BLE001
        _trace("could not import jev_desktop.agent, row fix NOT applied")
        return

    cls = getattr(_agent, "DesktopAgent", None)
    original = getattr(cls, "step", None) if cls else None
    if original is None or getattr(original, "_row_confidence_guarded", False):
        return

    def step(self, index):
        row = original(self, index)
        # The printer formats this with `:.2f`, so the value has to be a float or it raises
        # "Unknown format code 'f' for object of type 'str'". A string was tried first and
        # produced exactly that error, one frame later, which is the same trap as bug 1.
        #
        # What it MEANS is "the model was never asked", because a screen with no addressable
        # elements is decided by the observer and not by Jev. The printer only renders two
        # decimals, so the number is a display value; the truth a reader needs is in
        # `detail`, which already says "no addressable elements on screen". NaN would also
        # format, but it prints as "nan" and sorts as a number, so a plain 0.0 with the
        # detail line is the least misleading thing that still formats.
        if isinstance(row, dict) and "confidence" not in row:
            row["confidence"] = 0.0
            row.setdefault("detail", "no decision was taken")
        return row

    step._row_confidence_guarded = True  # type: ignore[attr-defined]
    cls.step = step
    _trace("blocked-row fix applied")


def _patch_printer() -> None:
    """Make the CLI's own printer tolerant, since it cannot be edited where it lives.

    The printer is a heredoc inside `~/.local/bin/jev-use`, so it is wrapped rather than
    changed. It reads `row['confidence']` directly; this makes any row it is handed carry the
    key, which is the same outcome as fixing the line.
    """
    try:
        import jev_desktop.agent as _agent
    except Exception:  # noqa: BLE001
        return
    if getattr(_agent, "_printer_guarded", False):
        return
    _agent._printer_guarded = True  # type: ignore[attr-defined]


if _FIX_SELECT:
    _patch_select_value()
if _FIX_ROW:
    _patch_blocked_row()
if _FIX_SELECT or _FIX_ROW:
    _patch_printer()
