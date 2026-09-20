# Tasks

The live board. Every PR updates this file.

**Format:** `- [ ] T<n> <what> - <why it matters>` , then the PR number when done.

---

## Now

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

- [x] **T3 - Typed attribute reads** (`ax/attrs.rs`) - 13 new tests, 32 total. **PR #9.**
  - One reader each for role, title, description, value, placeholder, enabled, position,
    size, rect, actions and settability, over a shared `raw` and `as_text` pair. The walk
    now has everything it needs to fill an element; it does not have to touch FFI.
  - `raw` returns the `AXError` code instead of `Option`, because `-25205`
    (`AttributeUnsupported`) and `-25204` (`CannotComplete`) both mean "no value" while
    meaning opposite things about the app. Live proof: `AXPosition` on the system-wide
    element returns exactly `-25205`, asserted as a code, not just as `err()`.
  - **`None` never becomes a default.** An absent `AXEnabled` is `None`, not `false`; an
    absent `AXPosition` is `None`, not `(0, 0)`. Rule R1 rests on this, so it is asserted.
  - `as_text` prints `True`/`False` and `2880.0` the way Python does, so a parity diff
    against the reference shows real differences rather than formatting noise (rule R8).
  - **Live tests target the system-wide element and Finder's menu bar**, not this process.
    A CLI test binary is not a GUI app and AX answers `-25208` for it; a UI element is not
    reachable from one. Both chosen targets need nothing open. See `docs/memory.md` D12.
  - `is_settable`'s positive case needs a writable attribute on an element we control,
    which is T6's write path. Only the false case is asserted here, and that is stated in
    the test.

- [x] **T15 - The voice hook** - `crates/jev-voice` + `scripts/voice-hook`. 15 new tests,
  47 total.
  - `decide()` is pure: no I/O, no spawning, no clock, so CI tests the decision on any
    platform while the acting half stays on macOS.
  - **Mode `prefix` is the default, and that is a safety decision.** A command happens
    only when the utterance opens with the trigger ("emma"), so prose cannot become a
    command by accident. `classify` (verb heuristic, opt-in) exists for people who want
    to skip the trigger, and its known false positive - "send it now" - is a test, not a
    claim that the rule is exact. The asymmetry is why: a command typed into a field is
    harmless and visible, while dictation handed to Jev disappears into an agent.
  - **Measured, all on the real binary through the real wrapper:**

    | Path | Time |
    | --- | --- |
    | Decision only (`--dry-run`) | **8 ms** |
    | Dictation, including the paste | **0.24-0.33 s** |
    | Command hand-off (parent returns) | **6-15 ms** |

  - Dictation is proven end to end, not asserted: a scratch TextEdit document received
    `Hello from the voice hook: it typed this, didn't it?` **byte-identical**, and the
    document was closed without saving.
  - A command is backgrounded as a detached copy of the hook, so the app never waits for
    a Jev run. Proven with a stub Jev: parent back in 15 ms, child finished 2.0 s later
    and logged its exit code.
  - **Always exits 0**, including on a missing transcript, a bad `--mode`, and an unknown
    flag (which fails closed rather than pasting the flag as text). A non-zero exit is
    reported by CoCo Voice as a paste failure.
  - **Installed and switched on:** binary at `~/.local/bin/jev-voice-hook`; Coco Voice
    now runs `paste_method: external_script` pointed at `scripts/voice-hook`, and its own
    log confirms it loaded that. Verified under a GUI-like minimal environment
    (`env -i`, `PATH=/usr/bin:/bin`): exit 0, 0.28 s, text byte-identical. Rollback and
    the one remaining human-only check are in `docs/memory.md` D13.

- [x] **T16 - A public repo brings its own keys** - the audit found no credential anywhere
  (the working tree, or all 29 commits) but did find the repo usable only on this machine.
  - `scripts/usage-check` now ships with **no providers in it**: you list yours in
    `~/.config/jev-use/usage-check.json` (template provided), keys come from your
    environment or your auth file, and every path is an env var with a default. This
    machine's providers moved to that file, and the output was diffed against the old
    hardcoded behaviour - same balances, same grouping, same routes.
  - `scripts/voice-hook` takes `JEV_VOICE_HOOK_BIN`, `JEV_USE_BIN`, `JEV_VOICE_MODE` and
    `JEV_VOICE_TRIGGER`; `scripts/install-voice-hook.sh` installs into your bin directory
    and prints the settings for your machine. The hook reads no keys at all.
  - The README states the two rules, and `.githooks/pre-push.local` runs `repo-check` over
    every **tracked** file, so a credential or a personal path cannot be pushed. That is
    a different scan from the shared hook's staged diff, and both now run.
  - Result: `repo-check` went from one gap to `COMPLIANT every check passed`.
  - Open question for the owner: promote the rule to `docs/rules.md` **R11**? That file
    requires explicit approval for a rule change, so it is not an agent's call. See D14.

- [x] **T17 - Settings, in one place, with a menu** - `crates/jev-config` + `jev-config`.
  - One file (`~/.config/jev-use/settings.json`), one command, one precedence:
    default < file < env var < flag. `show --sources` prints which layer won, per value.
  - `jev-config` with no arguments is a menu: numbered rows, current value, one key to
    change, with closed sets offered rather than guessed at. Not attached to a terminal it
    degrades to `show` instead of blocking.
  - `doctor` checks hook, driver, log, voice app, wallets and turn log in one command.
    **It found a live bug immediately:** the driver was the bare name `jev-use`, which
    resolves on a shell `PATH` but not on the `/usr/bin:/bin:/usr/sbin:/sbin` a launched
    app gets - so every voice command would have failed silently after a perfect
    transcript. Fixed, and the pattern is now a check rather than an anecdote.
  - The wrapper script stopped being the configuration; it is one `exec` and a warning.
  - `~` is expanded when used, never when written, so the file stays portable. A settings
    file that does not parse is logged and ignored *for that run*: the hook is on the
    paste path and must not break dictation.
  - See `docs/memory.md` D15.
