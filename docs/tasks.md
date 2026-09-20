# Tasks

The live board. Every PR updates this file.

**Format:** `- [ ] T<n> <what> - <why it matters>` , then the PR number when done.

---

## Now

- [ ] **T2 - App resolution and activation** (`ax/app.rs`) - app name to a running process,
  and bring it forward. Blocks every real walk.
  - `AXUIElement::new_application(pid)` is confirmed working by the T1 spike, and
    `AXUIElement::pid()` exists. Activation needs `objc2-app-kit` (`NSRunningApplication`).

## Next

- [ ] **T3 - Typed attribute reads** (`ax/attrs.rs`) - `AXRole`, `AXTitle`, `AXDescription`,
  `AXValue`, `AXEnabled`, `AXPosition`, `AXSize`. One function each, tested against a live
  element.
  - Write against `AXUIElement::copy_attribute_value` and `AXValue::value`, and return
    `CFRetained`, not `Retained`. Four gotchas are listed in `docs/architecture.md`
    section 5 so they do not each cost a compile cycle.
- [ ] **T4 - The tree walk** (`ax/walk.rs`) - bounded DFS, seeding windows explicitly (R3),
  applying the addressability rule (R1), returning an `ElementTable`.
- [ ] **T5 - The parity harness** - run the Python reference and the Rust port on the same
  app, diff the tables. This is what makes the port verifiable (R8).
- [ ] **T6 - Execution** (`ax/execute.rs`) - `AXPress`, `AXValue` write, keystroke fallback
  (R4). Tested on a real button.
- [ ] **T7 - `jev-use-rs` CLI** - `--apps`, `--table --app <App>`, matching the existing
  CLI's flags so muscle memory transfers.

## Then

- [ ] **T8 - Jev client** (`jev/`) - the model call, questions, strict validation.
- [ ] **T9 - Agent loop** (`emma-core`) - observe, choose, execute, with the repeat guard
  (R5).
- [ ] **T10 - v0.1 Emma** - voice to answer on project status. Group A. **No Jev, no AX.**
- [ ] **T11 - Capability registry** - name to fulfilment path.

## Blocked

- [ ] **T12 - Tauri shell** - blocked on T4 (needs a real observation to display).
- [ ] **T13 - Voice capture** - blocked on the STT engine decision (PRD open question 1).

## Decided not to do

- Proactive monitoring / watch mode. Deferred to a daemon, not v1. (PRD group E)
- Any SwiftUI / Liquid Glass native build. Needs Xcode 26, absent. Revisit if installed.
- Vendoring `accessibility-sys` as an AX fallback. **Dropped** - the T1 spike proved the
  native `objc2-application-services` works.
- Multi-user anything.
- A plugin system. The registry is data, not a framework.

---

## Done

- [x] **T1 - SPIKE: does `objc2-application-services` expose the four AX entry points?**
  **YES.** All four exist and were called against five live apps.
  - Proof: `AXUIElementSetMessagingTimeout(2.0)` returned `AXError(0)`;
    `copy_attribute_value("AXRole")` returned `AXApplication`;
    `AXValue::value(CGPoint)` returned `true -> (0, 900)`.
  - Found: the `HIServices` feature is required; the crate returns `CFRetained`, not
    `Retained`; `CFArray::value_at_index` returns a bare pointer.
  - Bonus: PI-Desktop reproducibly returns `CannotComplete (-25204)`, which is exactly the
    hang rule R2 protects against.
  - Full evidence: `docs/memory.md` D8. **Spike crate deleted** - its value was the answer.
