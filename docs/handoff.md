# Handoff - jev-use

**Written:** 2026-09-20, by the agent that built T1-T17.
**For:** the agent taking this repo over.
**Status:** 18 PRs merged, `main` green, nothing open, nothing half-written.
**Read this file first, then `AGENTS.md`, then `CONTRIBUTING.md`, then `.metagpt/STATE.md`.**
The rules this repo is held to live in `docs/repo-playbook.html` (the standard) and are
enforced by `scripts/repo-check` — run it before claiming anything is done.

---

## 1. What this project is, in one paragraph

A voice-driven control layer for macOS: press a key, speak a goal, and the machine does it
across any app - with Jev (TypeSafe's System One model) as the universal fallback for
anything that has no purpose-built path. The current work is the **Rust port of a proven
Python accessibility driver** (`~/code/jev-computeruse/jev_desktop/ax.py`), at parity,
so that nothing downstream rests on code that cannot be verified. The voice front end
("Emma") comes after the observation and execution layers are trustworthy.

**The one thing to internalise:** the Python driver at `~/code/jev-computeruse/` is the
**specification**, not a sibling. `docs/rules.md` R8. When the port and the reference
disagree, the port is wrong until proven otherwise.

---

## 2. Where things stand

### Done, merged, verified

| Task | What | Evidence |
| --- | --- | --- |
| T1 | SPIKE: `objc2-application-services` exposes the AX entry points. **Yes.** | `docs/memory.md` D8 |
| T2 | App resolution + activation (`crates/jev-ax/src/app.rs`) | 10 tests; pid/name set byte-identical to the Python reference (D9) |
| T3 | Typed attribute reads (`crates/jev-ax/src/attrs.rs`) | 13 tests; live reads against the system-wide element and Finder's menu bar (D12) |
| T14 | The macOS path runs in CI (self-hosted runner `coco-mac-jevu`) | D16; `.github/workflows/ci.yml` job `macos` |
| T15 | The voice hook (`crates/jev-voice` + `scripts/voice-hook`) | D13; **live on this machine**, measured |
| T16 | The public repo brings its own keys | D14; `repo-check` reports COMPLIANT |
| T17 | One settings file + `jev-config` menu + `doctor` (`crates/jev-config`) | D15 |

**Test count: 58** (32 `jev-ax`, 12 `jev-config`, 14 `jev-voice`). CI runs four jobs:
`lint` (fmt + clippy), `test`, `docs` on ubuntu, `macos` on the self-hosted runner.

### In flight

**Nothing.** No open PRs, no branches, clean working tree. The last merge is
`9c56f4e ci(macos): run the accessibility path in CI (#18)`.

### Next, in the order the board has them

| Task | What | Notes for you |
| --- | --- | --- |
| **T4** | The tree walk (`crates/jev-ax/src/walk.rs`) | Bounded DFS, windows seeded explicitly (R3), the addressability rule (R1), returns an `ElementTable`. **Everything downstream waits on this.** `attrs.rs` already has every read it needs. |
| **T5** | The parity harness | Runs the Python reference and the Rust port on the same app and diffs the tables. This is what makes the port verifiable; without it, T4 is unproven. |
| **T6** | Execution (`crates/jev-ax/src/execute.rs`) | `AXPress`, `AXValue` write, keystroke fallback (R4). `attrs::is_settable`'s positive case was left for this task and the test says so. |
| **T7** | `jev-use-rs` CLI | `--apps`, `--table --app <App>`; match the existing CLI's flags so muscle memory transfers. |
| T8-T11 | Jev client, agent loop, v0.1 Emma, capability registry | Design in `docs/architecture.md`. |
| T12 | Tauri shell | Blocked on T4 (needs a real observation to display). |

### Open items that are NOT code

1. **The runner has no Accessibility grant.** Live AX reads from the self-hosted runner
   answer `-25211` (APIDisabled), so the `macos` job reports "NOT TESTED" instead of
   passing. One click fixes it: System Settings -> Privacy & Security -> Accessibility ->
   add `~/actions-runner-jevu/bin/Runner.Listener`. Until then the macOS job is green
   about compiling and explicit about not having looked. See D16.
2. **`~/.cargo/bin` on this machine dangles** - every shim (`cargo`, `cargo-clippy`,
   `cargo-fmt`) is a symlink to a `rustup` that is not installed. The real toolchain is a
   bare install under `~/.rustup/toolchains/`. The pre-push hook works only if `cargo` is
   on your PATH: `export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"`.
   The CI job derives it from `$HOME`. **Do not "fix" the user's Rust install without
   asking** - that is an install decision, not an edit.
3. **Nobody has spoken into the voice hook yet.** The plumbing is proven (see below), the
   microphone path is not: press **Ctrl**, say *"Emma, open a new tab"*, release, then
   `tail -1 ~/Library/Logs/jev-voice-hook.log`.

---

## 3. What exists on this machine right now

| Thing | Where | State |
| --- | --- | --- |
| **The repo (canonical)** | **`~/Rijul Kalra/Coco Research/products/jev-use`** | `main`, clean, public at `coco-research/jev-use`. **Work here.** |
| The voice app's hook path | `…/products/jev-use/scripts/voice-hook` | CoCo Voice's `external_script_path` points here (the path has spaces - quote it) |
| Python reference | `~/code/jev-computeruse/jev_desktop/` | The spec. Runs today: `jev-use --table --app Terminal` |
| Decision layer (upstream) | `~/code/jev-ultrafast/` | Imported, never forked (D4) |
| Settings (all of it) | `~/.config/jev-use/settings.json` | `mode=prefix`, `trigger=emma`, driver absolute |
| Settings menu | `jev-config` (in `~/.local/bin`) | `jev-config` for the menu, `doctor` to check the chain |
| Voice hook | `~/.local/bin/jev-voice-hook` | Live, spawned by CoCo Voice |
| Voice app | `/Applications/Coco Voice.app` | `paste_method: external_script` -> `scripts/voice-hook` |
| Hook log | `~/Library/Logs/jev-voice-hook.log` | One line per utterance, one per backgrounded run |
| Self-hosted runner | `~/actions-runner-jevu` + LaunchAgent `com.coco.github-runner-jevu` | online; it checks out into its own `_work/`, not the repo |
| Usage/spend reporter | `scripts/usage-check` | Reads the `usage` section of the settings file |

**There is exactly ONE checkout.** `~/code/jev-use` was retired on 2026-09-20 (D17): two
checkouts of one repo is how two agents end up editing different copies of the same file.
If you find a second copy anywhere, that is a bug - delete it and say so.
**`jev-config doctor` is the fastest way to see whether the machine is still wired up.**
It checks the hook binary, the driver, the log, the voice app's own settings, the wallets
and the turn log, and it prints the fix for anything it finds.

---

## 4. How this repo works - the parts you must not learn the hard way

### The gate

```bash
export PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps     # CI runs this; the hook does not
```

`.githooks/pre-push` runs fmt, clippy, `.githooks/pre-push.local` (which runs
`repo-check` over every tracked file) and a secret scan. It is installed
(`core.hooksPath = .githooks`). **It does not run `cargo doc` and it does not run the
macOS-only tests** - those need the Accessibility grant described above.

Every change lands as a small PR; CI must be green. No direct pushes to `main`. See
`CONTRIBUTING.md`.

### The two rules that have already caught real bugs

1. **No credential and no machine-specific path in the repo, ever.** `repo-check` fails on
   both, and the pre-push hook runs it over the whole tree. It flagged a fake home
   directory used as an *example* in a doc - the doc was changed, not the check (D14).
2. **Fail loudly, never silently substitute** (R10). A settings file that does not parse is
   an error; a test that cannot see anything says so instead of passing (D16).

### macOS-specific reality

- The AX code is `cfg(target_os = "macos")`. On ubuntu it is not compiled at all, which is
  why the self-hosted runner exists. Anything that *can* be pure logic should be, so CI can
  test it.
- Accessibility is granted **per process** by TCC. A test binary launched from your shell
  inherits your terminal's grant; a runner does not. `cargo run --example check_access -p
  jev-ax` tells you whether the current process may look.
- A CLI process is not a GUI app: `AXUIElementCreateApplication(our_pid)` succeeds and then
  every attribute read answers `-25208` (NotImplemented). The live tests therefore target
  the system-wide element and Finder's menu bar (D12).

### The voice hook's contract (read before touching it)

CoCo Voice runs one command with the transcript as `argv[1]` and **waits for it to exit**,
so the hook is on the paste path:

```
dictation -> clipboard + Cmd+V                        0.24-0.33 s
command   -> detached copy of the hook, then exit     6-15 ms
```

- It **always exits 0**: a non-zero exit is reported by the app as a paste failure.
- `paste_method: external_script` **replaces typing**. Turning it off is a two-line change
  in the app's settings, documented in D13.
- The decision lives in `crates/jev-voice/src/lib.rs::decide` (pure, CI-tested). The acting
  lives in `main.rs`. Keep that split.

---

## 5. Decisions, so you do not relitigate them

Full text in `docs/memory.md`. The ones that will otherwise cost you a day:

| # | Decision |
| --- | --- |
| D3 | **Jev is the fallback, not the router.** Most questions are ~50 ms queries. |
| D4 | The decision layer is **imported from `jev-ultrafast`, never forked**. |
| D7 | The pre-push hook is the gate; CI is the backstop. |
| D10/D16 | macOS code is cfg-gated, and the macOS job runs on the self-hosted runner. |
| D11 | **Do not build a microphone, VAD or STT layer.** CoCo Voice already ships one; use its `external_script` hook. |
| D13 | The hook's default mode is `prefix` (exact) not `classify` (heuristic), because the two mistakes are not symmetric: dictation handed to Jev disappears into an agent. |
| D14 | A public repo brings its own keys. No provider list, no wallet topology, no home paths in the repo. |
| D15 | One settings file, one `jev-config` command, one precedence: default < file < env < flag. |
| D16 | The macOS job reports an environment gap as a gap, never as a pass it did not earn. |

Dead ends worth not repeating: `docs/memory.md` X1-X5 (browser pages through the AX tree;
AppleScript for observation; phantom-browser checks; blaming an image for a context
overflow; `f64` cannot derive `Eq`).

---

## 6. How to build your own memory here

The repo is built for this. Do not keep state in your head:

1. **`.metagpt/STATE.md`** - read it, then keep the first line current: `Next action: ...`.
   It is the first thing the next agent reads.
2. **`docs/tasks.md`** - the board. Every PR moves an item and says what it proved.
3. **`docs/memory.md`** - decisions with reasons, and dead ends. When you spend an hour
   discovering a hypothesis was wrong, that is the entry that saves the next hour.
4. **`docs/rules.md`** - the invariants, each with the measurement behind it. A PR that
   changes a rule must say so in its title.
5. **`.metagpt/GATE.json`** - where the project is in its own process, and the notes that
   would otherwise be lost.

**The habit that made this repo work:** every task ends with a *measurement*, not an
assertion - "11 apps found, byte-identical to the reference", "0.24 s to paste", "the
runner picked the job up in 4 s". If you cannot measure it, say so and say why.

---

## 7. What the owner is doing while you work

The owner has split responsibilities:

- **Model selection and routing** (which model answers what, the LiteLLM gateway at
  `127.0.0.1:4010`, subscription health, `pinned-models.json`, KeyDeck) is handled by a
  **different** agent. Do not edit `~/.zcode/litellm_config.yaml`, `~/.zcode/run-litellm.sh`,
  the LiteLLM launchd job, `~/.pi/agent/pinned-models.json`, or `~/.secrets/ai-keys.env`.
  You may read and probe them.
- **This repo** is yours. The voice hook, the settings surface and the AX port are all
  yours - they are this repo's code.

That boundary exists because two agents editing the same files in parallel is how state
gets corrupted. If a task of yours needs a change on the other side of it, say so in the
PR body and leave the file alone.

---


## 8. First three things to do

1. `cargo test --workspace` and `jev-config doctor` - confirm the machine and the repo are
   where this document says they are.
2. Grant the runner its Accessibility permission (one click, §2 item 1), then re-run the
   `macos` job and watch the live AX tests actually execute. That closes the last gap in
   CI and gives you a real signal for everything after it.
3. Start **T4, the tree walk**, with **T5's parity harness** as its acceptance test. Write
   T5 first if you can: it is what turns "the walk looks right" into "the walk matches the
   reference exactly".

---

## 9. Co-writers - this repo has more than one

**Another agent works in this repo too**, committing as `coco-research`. It owns the
README's brand assets (`docs/readme/*.svg`, `*.png`, PRs #13 and #19 nearby) and has been
iterating on the company mark and title treatment. It is not a stray file-dropper.

Two consequences, both learned the hard way:

- **Check `git log` before removing a file that looks unreferenced.** An earlier session
  removed two images from `docs/readme/` as junk; the other agent's PR then re-added them,
  because they were that agent's work in progress.
- **Do not `git add -A`.** Stage the paths you actually changed, or another writer's
  in-flight files ride into your commit and its PR body says something untrue.

If you need to know who owns a file you did not write, `git log --format="%an %s" -- <path>`.
