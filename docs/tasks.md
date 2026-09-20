# Tasks

The live board. Every PR updates this file.

**Format:** `- [ ] T<n> <what> - <why it matters>` , then the PR number when done.

---

## Now

- [ ] **T3 - Typed attribute reads** (`ax/attrs.rs`) - `AXRole`, `AXTitle`, `AXDescription`,
  `AXValue`, `AXEnabled`, `AXPosition`, `AXSize`. One function each, tested against a live
  element.
  - Write against `AXUIElement::copy_attribute_value` and `AXValue::value`, and return
    `CFRetained`, not `Retained`. The dependency gotchas are in `docs/memory.md` D10.
- [ ] **T15 - The voice hook** - a script that receives a transcript from CoCo Voice and
  decides: command (hand to Jev) or dictation (type it). Unblocked by D11.
  - Wires into `paste_method = "external_script"` + `external_script_path`. No change to
    that repo is needed.
  - **The hard part is the per-utterance decision**, not the plumbing: `external_script`
    REPLACES typing, so choosing it means dictation stops working unless the hook types.
  - Must return in about a second. Slower work goes to the background.
- [ ] **T14 - Register the self-hosted runner for this repo** - `coco-mac-local` is already
  online for four other repos. Registering it here would let CI run the macOS path, which
  an ubuntu runner cannot. See `docs/memory.md` D10.

## Next

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

- [x] **T2 - App resolution and activation** (`ax/app.rs`) - 10 new tests, 19 total.
  - **Verified live:** 11 apps found, and the pid/name set is **byte-identical to the Python
    reference** (`jev-use --apps`).
  - Matching is exact-or-unique, never fuzzy. Several partial matches is an error listing
    the candidates, not a pick (rule R10). Live proof: `"e"` matches 10 apps and errors;
    `"co"` matches 2 and errors; `"a"` resolves to Terminal.
  - **One deliberate divergence from Python**, recorded per rule R8: apps are sorted
    alphabetically after the frontmost one, because `NSWorkspace` order is not stable
    between calls and an unstable order makes resolution flaky. See `docs/memory.md` D9.
  - **Two bugs found in the Python reference:** `activateWithOptions(1 << 1)` is a no-op on
    macOS 14+ (the SDK deprecates that flag), and the driver calls it in two places.
  - Live check: `cargo run --example list_apps` in `crates/jev-ax`.
