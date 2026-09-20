# Architecture

**Status:** draft
**Last updated:** 2026-09-19

---

## 1. The shape

```
                    +---------------------------+
   voice  ------->  |  trigger + capture        |   global hotkey, mic
                    +-------------+-------------+
                                  |
                                  v
                    +---------------------------+
                    |  intent                   |   text -> {which capability, args}
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    |  capability registry      |   purpose-built paths, ~50 ms
                    +-------------+-------------+
                                  |
                    +-------------v-------------+
                    |  JEV fallback             |   AX observation + action
                    +---------------------------+
```

Five subsystems. Each independently testable. Only the last one is what this repo is
currently building.

## 2. The layer boundary that matters

`crates/jev-ax` has exactly two jobs:

1. **OBSERVE** the accessibility tree into an indexed element table.
2. **EXECUTE** the chosen element with an accessibility action.

It **knows nothing about Jev.** The Jev client knows nothing about AX. They meet one
layer up, in the agent loop. That is why the port is testable without a model, and the
model is testable without a screen.

```
jev-ax      ElementTable, Kind, Rect, Limits          (no network, no model)
jev         choose() -> (operation, element index)     (no AX, no screen)
emma-core   the loop: observe -> choose -> execute      (the only place they meet)
```

## 3. The port target

`crates/jev-ax` is a **port of a working system**, not a rewrite. The reference is
`~/code/jev-computeruse/jev_desktop/ax.py`, which is measured and running today.

The port target is **parity**, not similarity:

```
python jev_desktop --table --app Terminal --json   ->  20 addressable of 878 nodes
cargo run -p jev-ax -- --table --app Terminal      ->  must match exactly
```

Same count, same order, same roles, labels, values, fingerprint. Divergence is a port
bug, caught mechanically.

## 4. Crate layout

```
crates/
  jev-ax/                 the port target. macOS Accessibility only.
    src/lib.rs            Element, Kind, Rect, Limits, ElementTable, fingerprint
    src/attrs.rs          (planned) typed AX attribute reads
    src/walk.rs           (planned) tree walk with caps and the on-screen test
    src/execute.rs        (planned) AXPress, AXValue write, keystroke fallback
    src/app.rs            (planned) app resolution and activation
  emma-core/              (planned) the loop: observe -> choose -> execute
  jev/                    (planned) the model client, router, questions, validation
apps/
  desktop/                (planned) Tauri shell, glass UI
```

## 5. Dependencies

Verified on this machine, 2026-09-20:

| Need | Crate | Version |
| --- | --- | --- |
| AX API (`AXUIElement`, `AXValue`) | `objc2-application-services` | 0.3.2 |
| App list, activation | `objc2-app-kit` | 0.3.2 |
| Mouse + keyboard events | `core-graphics` | 0.25.0 |
| ObjC runtime | `objc2` | 0.6.4 |

**Settled - T1 spike, 2026-09-20.** All four entry points exist and were **called
successfully against five live apps**, not merely referenced. The dependency choice is
final; the `accessibility-sys` fallback is dropped. See `docs/memory.md` D8 for the raw
evidence.

The crate's method names differ from the C function names, and these are the ones to
write against:

| C name | Rust name |
| --- | --- |
| `AXUIElementCopyAttributeValue` | `AXUIElement::copy_attribute_value` |
| `AXUIElementCopyActionNames` | `AXUIElement::copy_action_names` |
| `AXUIElementSetMessagingTimeout` | `AXUIElement::set_messaging_timeout` |
| `AXValueGetValue` | `AXValue::value` |

Four facts the spike established that the design could not have guessed, each of which
will otherwise cost a compile cycle:

1. The **`HIServices`** feature is required, or the entire AX module is gated out.
2. It returns **`CFRetained<T>`**, not `objc2::rc::Retained<T>`, and `from_raw` takes a
   `NonNull`.
3. **`CFArray::value_at_index` returns a bare `*const c_void`**, so null checks are ours.
4. **`AXError` is a newtype over `i32`** with PascalCase associated consts.

`trycua/cua` keeps a production Rust AX implementation that is a useful reference for the
calls, though it pins `objc2 0.5` and is not drop-in reusable.

## 6. UI

**Tauri**, not SwiftUI. Reason: Liquid Glass needs Xcode 26 and the macOS 26 SDK, and
this machine has neither (SDK 11.3, Swift 5.4).

Tauri uses WKWebView, so CSS `backdrop-filter: blur()` blurs the **real desktop behind
the window**. That is genuine translucency, not a flat fake. What is lost is Apple's
automatic specular highlights and morphing transitions - both approximable in CSS.

macOS 26.5.2 renders Liquid Glass natively, so a future SwiftUI path stays open if Xcode
is ever installed. The decision is reversible.

See `docs/design.md`.

## 7. Data

| Store | Holds | Location |
| --- | --- | --- |
| Capability registry | name -> how to fulfil it | local file; format is an open question |
| Task list | what is in flight | `docs/tasks.md` for now, DB later |
| History | what was asked, what happened | local SQLite |
| Settings | hotkey, voice, model | local file |

**No secrets in this repo, ever.** The app reads keys from a local secret store that lives
outside this repository, by path, and never copies them anywhere.

## 8. Error handling

The rule throughout: **fail loudly and honestly, never silently substitute.**

- A missing capability falls through to Jev. It does not guess.
- A blocked AX call times out. It does not hang the walk.
- An element that is not addressable is not offered. Jev is never invited to pick
  something that cannot be executed.
- A repeat with an unchanged screen stops the loop. It does not spin.

## 9. What is deliberately not here

- No plugin system. The registry is data, not a framework.
- No cross-platform abstraction. macOS only until that is wrong.
- No abstraction over Jev. There is one decision layer and it is imported, not wrapped.
