# Two crashes in the desktop driver, and the patch that gets past them

Kept here so the measurement can be repeated instead of believed, the same reason
`docs/spikes/needle3/` exists.

Written 2026-09-27. The findings are `docs/memory.md` **X7**. This directory holds the
working patch and the two commands that prove it.

## What was wrong

`jev-use` could not complete a single desktop goal. Not one app, not one goal: **any** goal,
in **any** app whose accessibility tree holds a popupbutton or a menubutton. Two crashes, one
behind the other.

**Crash 1, upstream.** `jev_ultrafast/model.py:76` indexes `action["value"]` directly. The
desktop producer emits `value` only when the element has one (`reference/vendor/ax.py:331-332`,
and `crates/jev-ax/examples/walk.rs:127` copies that faithfully). An `AXPopUpButton` with an
empty `AXValue` raises `KeyError: 'value'` before Jev is asked anything.

**Crash 2, ours.** `jev_desktop/agent.py:61` returns a row with no `confidence` for the case
where the screen has no addressable elements, and the printer in `bin/jev-use` reads
`row['confidence']` while every neighbouring key on that line uses `.get()`.

## Reproduce

```bash
# Crash 1, upstream: any app with a popupbutton in its tree.
JEV_FIX_SELECT_VALUE=0 jev-use "click the Applications item in the sidebar" --app Finder
#   KeyError: 'value'   at jev_ultrafast/model.py:76

# Crash 2, ours: with the first patched, this is what comes next.
# Turn only the second fix off and the first stays on.
JEV_FIX_BLOCKED_ROW=0 jev-use "collapse the sidebar" --app "PI-Desktop"
#   KeyError: 'confidence'
```

## Prove the patch

```bash
JEV_FIX_TRACE=1 jev-use "click the Applications item in the sidebar" --app Finder
#   [jev-fix] select-value fix applied
#   [jev-fix] blocked-row fix applied
#      1 ■ ?          0.00          0ms obs + 0ms jev
#          no addressable elements on screen
#   stopped: blocked after 1 step(s)
```

It completes, and it says the honest thing instead of raising.

## What is here

`select_value_fix.py` is the patch, dropped into `~/code/jev-computeruse/jev_desktop/` as
`_select_value_fix.py` and imported from that package's `__init__.py`. Self-contained,
env-gated, no dependency on anything in this repo.

**The import order is load-bearing and was measured, not reasoned about.** It must come
**after** `.agent` in `jev_desktop/__init__.py`, because `agent.py` stubs around
`jev_ultrafast/__init__.py` before importing `jev_ultrafast.model`. Imported first, the patch's
own import fails, `_patch` returns silently, and the result is the worst outcome available:
`patched: False`, no error, and the crash still happening.

## Where the real fix goes

Neither patch belongs in `jev-ultrafast`: it is `browser-use/jev-ultrafast`, third-party, with
**READ**-only access, and it is the file `reference/PINNED.json` pins by **sha256** with
`scripts/parity` checking both copies before it runs. Editing it would break the pin.

Both real fixes are one line each, in files **this** repository owns or will own:

| Crash | File | Change |
| --- | --- | --- |
| 1 | `jev_ultrafast/model.py:76` (upstream) | `value`: `action["value"]` to `action.get("value", "")` |
| 2 | `bin/jev-use`, the row printer | `row['confidence']` to `row.get('confidence', 0.0)` |

Until they land, the desktop half of the computer-use policy cannot drive anything, and
`jev-use` is not a fallback for the browser path.
