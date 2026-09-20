# Tasks

The live board. Every PR updates this file.

**Format:** `- [ ] T<n> <what> - <why it matters>` , then the PR number when done.

---

## Now

- [ ] **T1 - SPIKE: does `objc2-application-services` expose the four AX entry points?**
  - Needs: `AXUIElementCopyAttributeValue`, `AXUIElementCopyActionNames`,
    `AXUIElementSetMessagingTimeout`, `AXValueGetValue`
  - Why: **everything in this repo rests on this answer.** A 1-hour probe.
  - If yes: native crate. If partial: vendor `accessibility-sys 0.2.0`. If no: write the
    FFI ourselves, and re-plan.
  - Done when: a `spike/` branch with a scratch binary that compiles, links, and prints
    the AX role of one element from one running app.
  - Time-box: 1 hour. Report the answer, do not build past it.

## Next

- [ ] **T2 - App resolution and activation** (`ax/app.rs`) - app name to a running
  process, and bring it forward. Blocks every real walk.
- [ ] **T3 - Typed attribute reads** (`ax/attrs.rs`) - `AXRole`, `AXTitle`,
  `AXDescription`, `AXValue`, `AXEnabled`, `AXPosition`, `AXSize`. One function each,
  tested against a live element.
- [ ] **T4 - The tree walk** (`ax/walk.rs`) - bounded DFS, seeding windows explicitly
  (R3), applying the addressability rule (R1), returning an `ElementTable`.
- [ ] **T5 - The parity harness** - run the Python reference and the Rust port on the
  same app, diff the tables. This is what makes the port verifiable (R8).
- [ ] **T6 - Execution** (`ax/execute.rs`) - `AXPress`, `AXValue` write, keystroke
  fallback (R4). Tested on a real button.
- [ ] **T7 - `jev-use-rs` CLI** - `--apps`, `--table --app <App>`, matching the existing
  CLI's flags so muscle memory transfers.

## Then

- [ ] **T8 - Jev client** (`jev/`) - the model call, questions, strict validation.
- [ ] **T9 - Agent loop** (`emma-core`) - observe, choose, execute, with the repeat
  guard (R5).
- [ ] **T10 - v0.1 Emma** - voice to answer on project status. Group A. **No Jev, no AX.**
- [ ] **T11 - Capability registry** - name to fulfilment path.

## Blocked

- [ ] **T12 - Tauri shell** - blocked on T4 (needs a real observation to display).
- [ ] **T13 - Voice capture** - blocked on the STT engine decision (PRD open question 1).

## Decided not to do

- Proactive monitoring / watch mode. Deferred to a daemon, not v1. (PRD group E)
- Any SwiftUI / Liquid Glass native build. Needs Xcode 26, absent. Revisit if installed.
- Multi-user anything.
- A plugin system. The registry is data, not a framework.

---

## Done

*(nothing yet - the repo scaffold is the first commit)*
