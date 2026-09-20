# Memory

**Not a diary.** Decisions with their reasons, and dead ends worth not repeating. If an
agent spent an hour proving a hypothesis wrong, that sentence saves the next hour.

Add to this on every PR that settles something.

---

## Decisions

### D1 - Rust from scratch, port the driver first (2026-09-19)

**Chosen:** port `jev_desktop` to Rust before building any of Emma.

**Why:** the port is verifiable because a working reference exists. That safety net is
rare and worth taking before adding new surface. The alternative - build the app in
Python, port later - defers the risky part to the moment the project is most interesting,
which is when it gets skipped.

**Rejected:** Tauri UI first; Python prototype first.

### D2 - Tauri, not SwiftUI (2026-09-19)

**Chosen:** Tauri, with CSS glass.

**Why:** measured. `.glassEffect()` **does not compile** on this machine -
`swiftc -typecheck` returns *"value of type 'Text' has no member 'glassEffect'"*.
Requires Xcode 26 and the macOS 26 SDK; this machine has SDK **11.3** and Swift **5.4**,
and Xcode needs ~40 GB against 29 GB free.

WKWebView's `backdrop-filter: blur()` blurs the **real desktop** behind the window, so
this is genuine translucency, not a fake. Lost: Apple's automatic specular highlights and
morphing transitions.

**Reversible:** macOS 26.5.2 renders Liquid Glass natively, so a SwiftUI path stays open
if Xcode is installed later.

### D3 - Jev is the fallback, not the primary (2026-09-19)

**Chosen:** a capability registry first; Jev for anything with no built-in path.

**Why:** the flagship use case - *"what is the status on payments-api"* - was answered in
about **50 ms** from SQLite plus one git call. No AX walk, no screenshot, no vision model,
no Jev. Making that an LLM call would be slower and worse.

**Consequence:** v0.1 needs neither Jev nor Accessibility permission.

### D4 - The decision layer is imported, never forked (2026-09-19)

**Chosen:** `~/code/jev-ultrafast/` stays pristine; its `model.py` and `questions.py` are
imported by path.

**Why:** `jev_ultrafast/model.py` is transport-agnostic - it asks one question and
validates the answer against the candidates offered. Only its browser layer is
CDP-specific. Forking it would let the two drift.

### D5 - Repo went public, because CI is free that way (2026-09-20)

**Superseded D5 (2026-09-19): "Repo is private."** It was private for one day, on the
reasoning that the documents describe machine layout and project names. That reasoning was
half right, and the cost turned out to be larger than the benefit.

**Why it flipped:**

1. **GitHub Actions is free and unlimited for public repositories.** The private repo
   could not run CI at all - the account's budget was exhausted (see D7). Going public
   restores CI immediately, on any runner including macOS.
2. **The sensitive parts were scrubable in about twenty minutes.** Private project names
   became generic examples, secret-store paths were genericised, and an absolute path
   became relative. That is a small one-time cost against an ongoing CI budget problem.
3. **A project like this is worth other people finding.** A Rust accessibility driver
   paired with a fast selection model is unusual, and the measured numbers in
   `reference/README.md` are the kind of thing that saves someone else a week.

**What did not need scrubbing:** the account name, because it is in the repository URL
regardless, and the machine specification (an M1 with 16 GB), because that is the
performance baseline a contributor needs to interpret the numbers.

**Licence:** Apache-2.0. Chosen over MIT for the explicit patent grant, at some cost in
familiarity. This matters because the project depends on other people's models and
drivers, and a public repo with no licence is legally untouchable.

### D6 - CI runs three jobs, and lint fails fast (2026-09-19)

**Chosen:** separate `lint` (fmt + clippy), `test`, `docs` jobs, on a macOS runner.

**Why:** formatting feedback should arrive in about a minute, not after a full build. And
`cargo doc` is a job because the crate is where the design lives - a doc comment that does
not compile is a broken contract.

### D7 - The pre-push hook is the gate; CI is the backstop (2026-09-20)

**Chosen:** `.githooks/pre-push` is the gate: fmt, clippy, test, docs, a secret scan, and
(once it exists) the parity harness. CI runs the same checks on `ubuntu-latest` as a
backstop, and it is green as of `d6c4201` once the repo went public.

**Why, found the hard way:** the very first push to this repo failed in CI with
*"The job was not started because an Actions budget is preventing further use."*

**My first diagnosis was wrong, and the correction matters.** I assumed it was the
macOS multiplier, since other repos in the same account run CI fine - but those are
**public**, and public repos get unlimited Actions minutes. I switched the runner to
`ubuntu-latest` at 1x, pushed again, and it **still failed with the same message.** So
the private budget was exhausted outright, not merely expensive. That is what forced the
public flip, and the moment it went public CI ran green.

**The deeper reason this split is right design, not just a workaround:** a GitHub-hosted
macOS runner *cannot test the accessibility code at all.* It has no logged-in GUI session
and no Accessibility permission, so every `AXUIElement` call fails there regardless of
runner. The behaviour this project cares about is only testable on this machine.

**So the split is deliberate:**

| Where | Tests | Cost |
| --- | --- | --- |
| Pre-push hook (local) | Everything, including AX against real apps | free, ~1.5 s warm |
| CI (ubuntu) | Pure logic, types, fmt, lints, docs | free - public repos are unlimited |

The crate is kept free of macOS-only dependencies **on purpose**, so the logic stays
testable in CI. macOS-only code goes in a module gated by `cfg(target_os = "macos")` and
is covered locally.

**Install the hook once per clone:** `git config core.hooksPath .githooks`

### D8 - T1 answered: objc2-application-services works. Build on it (2026-09-20)

**The question was:** does `objc2-application-services` expose the four AX entry points
the port needs, and can they actually be **called**, not merely referenced?

**Answer: yes to both.** Verified by compiling and running a live probe against five real
apps, not by reading documentation.

| What we needed | What the crate calls it |
| --- | --- |
| `AXUIElementCopyAttributeValue` | `AXUIElement::copy_attribute_value` |
| `AXUIElementCopyActionNames` | `AXUIElement::copy_action_names` |
| `AXUIElementSetMessagingTimeout` | `AXUIElement::set_messaging_timeout` |
| `AXValueGetValue` | `AXValue::value` |

**Runtime proof, one line each.** The raw C names still exist but are `#[deprecated]` with
the message *"renamed to `AXUIElement::copy_attribute_value`"*, so the method form is the
intended API, not a wrapper we are inventing.

```
AXUIElement::new_application(pid)        created
AXUIElementSetMessagingTimeout(2.0)      AXError(0)            <- rule R2 works
AXUIElementCopyAttributeValue("AXRole")  Some("AXApplication")
AXUIElementCopyActionNames               AXError(0)
AXValueGetValue(CGPoint)                 true -> (0, 900)      <- rule R1's input
```

**Things the spike settled that the design could not have guessed:**

1. **The crate needs the `HIServices` feature.** Without it the whole AX module is gated
   out and nothing compiles. `AXError` is a separate feature.
2. **It returns `CFRetained<T>`, not `objc2::rc::Retained<T>`.** Different type, different
   `from_raw` signature (takes `NonNull`). Easy to get wrong; costs a compile cycle.
3. **`CFArray::value_at_index` returns a bare `*const c_void`**, not an `Option`. Null
   checks are the caller's job.
4. **`AXError` is a newtype over `i32`** with PascalCase associated consts, compared as
   `err.0 == 0`.

**A real-world condition appeared during the spike, and it justifies rule R2.** PI-Desktop
returned `AXError(-25204)` = `CannotComplete` on **every** attribute, reproducibly across
attempts, while Finder, Terminal, Notes and Chrome all answered normally. Without the 2 s
messaging timeout that call does not fail - it **hangs**, and the walk never returns.

Error codes worth knowing, confirmed against the crate's own constants:

| Code | Name | Meaning here |
| --- | --- | --- |
| `-25204` | `CannotComplete` | app did not answer in time. R2 catches it. |
| `-25205` | `AttributeUnsupported` | normal: an app element has no `AXPosition` |
| `-25211` | `APIDisabled` | the process lacks Accessibility permission |

**Consequence:** `docs/architecture.md` section 5 is no longer "planned / unknown". The
dependency choice is settled, the method names are known, and T2 and T3 can be written
against a real API instead of an assumed one. The fallback option (vendoring
`accessibility-sys`) is dropped.

**The spike crate was deleted.** Its value was the answer, not the code; the real
implementation arrives in T2/T3 with proper tests. Recorded here so the next session does
not re-probe.


### D9 - T2 done: app resolution, and one deliberate divergence from Python (2026-09-20)

**Built:** `crates/jev-ax/src/app.rs` - a name to a running process, and bring it forward.
10 new tests, 19 total, all green. Verified against the live machine: **11 apps, and the
pid/name set is byte-identical to the Python reference.**

**The divergence, recorded because rule R8 requires it.** The two implementations return
the same apps but in a different order after the frontmost one:

| | Order after frontmost |
| --- | --- |
| Python | whatever `NSWorkspace` hands back |
| Rust | sorted alphabetically, case-insensitively |

**Why the change:** `NSWorkspace` order is not guaranteed stable between calls, so the
same screen could resolve a query to a different app on a different run. That is a bug
that presents as flakiness, which is the worst kind to debug. Sorting makes resolution
deterministic. The observable behaviour a caller depends on - which app a name resolves
to, and whether it is ambiguous - is unchanged.

**Two bugs found in the Python reference while porting:**

1. **`activateWithOptions(1 << 1)` is a no-op.** That bit is
   `NSApplicationActivateIgnoringOtherApps`, and the macOS SDK marks it
   `#[deprecated = "ignoringOtherApps is deprecated in macOS 14 and will have no effect."]`
   The driver calls it in two places (`ax.py:400`, `ax.py:431`), so it has been asking for
   something the OS ignores while silently getting plain activation. It works - for a
   different reason than the code says. The port passes `0` and says so.

2. **The two `apikey.fan` keys share one wallet** (found during the usage work, recorded
   here because it affects any balance reporting).

**Matching is exact-or-unique, never fuzzy.** `match_app` tries an exact
case-insensitive match, then a unique partial one. Several partial matches is an
**error** listing the candidates, not a pick, because choosing silently is exactly the
substitution rule R10 forbids. Live proof from this machine:

```
resolve("e")   -> 10 matches -> error, lists all ten
resolve("co")  -> CoCo Hermes, ZCode -> error
resolve("a")   -> Terminal -> resolves
```

### D10 - macOS code is cfg-gated, and that is a real CI gap (2026-09-20)

The AX and AppKit dependencies are `[target.'cfg(target_os = "macos")'.dependencies]`, so
an ubuntu runner never compiles them. That is deliberate: it keeps the pure logic
(Rect, Element, Limits, fingerprint, `match_app`) testable in CI on any platform.

**The cost, stated plainly:** CI cannot verify a single line of the macOS path. A typo in
`running_apps` would pass CI and fail at runtime. The mitigation is the local pre-push
hook, plus the `list_apps` example, which is the live check.

**And it is not only the tests.** `cargo doc` cannot see a module that is gated out
either, so a broken intra-doc link in `attrs.rs` **passed CI on PR #9** and failed the
moment the same command ran on this machine. Reproduce what CI would do if it could:

```
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The pre-push hook does not run it, which is why the failure was found by hand rather than
by the gate. Cheap fix if the hook is worth extending: one more `gate` line.

**The real fix is available and not yet used:** a self-hosted runner is already online on
this machine (`coco-mac-local`, registered for coco, coco-connect, coco-hermes and
coco-m0). Registering it for this repo would let CI run the macOS path, because a
self-hosted runner has a GUI session and Accessibility permission. That is a task, not a
theory.

**Dependency knowledge worth not rediscovering.** Each of these cost a compile cycle:

1. `libc` is a required feature of `objc2-app-kit`. Without it,
   `NSRunningApplication::processIdentifier` does not exist, and the error reads as
   "no method named processIdentifier" - which looks like a wrong type, not a missing feature.
2. `NSEnumerator` is required on `objc2-foundation` for **any** array iteration:
   `iter()` and every `IntoIterator` impl are gated behind it.
3. `objc2-core-foundation` has no `CFType` feature. `CFType` is always exported; the
   features are per-type (`CFString`, `CFArray`).
4. `default-features = false` on `objc2-app-kit` is worth keeping - the defaults pull in
   every AppKit binding - but it means `libc` and `bitflags` must be named explicitly.

### D11 - The voice layer already exists. Use CoCo Voice, do not build one (2026-09-20)

**Decision:** jev-use does **not** build its own microphone, VAD, or speech-to-text.
`coco-research/Coco-Voice` already ships that, fully offline, and exposes a clean hook.

This closes PRD open question 1 ("which speech-to-text engine - Apple's on-device, or
`whisper-rs`?"). The answer was already made and tested by that project.

**What was verified on this machine, not read:**

```
$ /Applications/Coco Voice.app/Contents/MacOS/coco-voice --transcribe-file <wav> --json
{"audio_secs":20.25, "best_ms":3049, "rtf":6.64, "bound_backend":"MTL0",
 "model":"handy-computer/cohere-transcribe-03-2026-gguf/...Q5_K_M.gguf",
 "text":"Also, note that the wording was never that it doesn't work with no model. ..."}
```

**20.25 s of audio, transcribed in 3.05 s. RTF 6.64x realtime**, on the M1 GPU via Metal.
Model load 1.63 s. Installing nothing: the app and its model were already present.

**The integration point, which already exists and needs no change to that repo:**

```rust
// Coco-Voice, src-tauri/src/clipboard.rs:505
/// Pastes text by invoking an external script.
/// The script receives the text to paste as a single argument.
fn paste_via_external_script(text: &str, script_path: &str) -> Result<(), String>
```

Setting `paste_method = "external_script"` and `external_script_path = <our script>` routes
**every transcript** to us as `argv[1]`. Their app keeps the hotkey, the microphone, the
VAD (Silero), the model management, and the Apple Intelligence post-processing.

**The chain:**

```
Ctrl (their push-to-talk binding, already set)
  -> mic -> VAD -> Cohere Transcribe (6.6x realtime) -> post-process
  -> external script  argv[1] = transcript
       -> jev-use: classify the intent -> answer directly, or hand it to Jev
```

**The trade-off to design around, and it is the important one:** `external_script`
**replaces** typing. Choosing it means dictation into the focused field stops working
unless the hook itself does the typing.

So the hook must decide, per utterance: is this a **command** ("open a new tab") or
**dictation** ("Dear team, ...")? Commands go to Jev. Dictation gets typed. That decision
is exactly what `jev-classify` already does for lanes, and it is fast enough to sit on
this path.

**Also a constraint:** the script runs synchronously on the paste path, so a slow hook is
a slow paste. Anything that cannot finish in about a second must be fired off in the
background rather than awaited.

**State on this machine, recorded so nobody re-checks it:**

| Thing | Where |
| --- | --- |
| The app | `/Applications/Coco Voice.app`, v0.9.4, running |
| Models | `~/.cache/huggingface/hub/models--handy-computer--*` (32 GB cache) |
| Cohere Transcribe Q5_K_M | **1.6 GB, already downloaded** |
| Push-to-talk | `push_to_talk: true`, bound to **Ctrl** |
| VAD | enabled (Silero) |
| Post-processing | enabled, Apple Intelligence, prompt `coco_formatted` |
| 81 models available | Parakeet, Whisper, Voxtral, Qwen3-ASR, Canary, Granite |

**One caveat about their app's own models directory:** `~/Library/Application
Support/com.cocoresearch.cocovoice/models/` is **empty**, and the models actually live in
the HuggingFace cache. That is not a fault - but it means "is a model installed" cannot be
answered by looking at the app's directory.

**Alternatives considered and rejected:**

- *Depend on their crate as a library.* It is a Tauri app crate; importing it drags in
  Tauri and ~100 dependencies to get at one module.
- *Reuse `transcribe-rs` / `transcribe-cpp` directly.* Duplicates the audio plumbing,
  VAD, model management and post-processing that already work, to gain nothing.
- *Ask them for a socket or headless mode.* Not needed: the script hook exists today.

### D12 - T3 done: what a live AX element actually reports, measured (2026-09-20)

Four facts that the first draft of the tests got wrong, all measured with
`crates/jev-ax/src/attrs.rs` against running apps:

| Target | `AXRole` | `AXEnabled` | `AXPosition` | Actions |
| --- | --- | --- | --- | --- |
| System-wide element | `AXSystemWide` | `-25205` unsupported | `-25205` | **none** |
| Our own test binary (a CLI process) | `-25208` | `-25208` | `-25208` | `-25208` |
| A real app element (Finder) | `AXApplication` | **`False`** | `(0, 900)` | **none** |
| Finder's menu bar | `AXMenuBar` | `True` | `(0, 0)` | `AXCancel` |
| A menu bar item | `AXMenuBarItem` | not measured | real rect | `AXCancel`, **`AXPress`**, `AXPick` |

1. **A CLI process is not an app.** `AXUIElementCreateApplication(our_pid)` succeeds and
   echoes the pid back, but every attribute read returns `-25208` (`NotImplemented`).
   So "point the tests at ourselves" does not work: there is no UI element to read.
2. **An app element is not enabled and has no actions.** `enabled: False`, `0` actions -
   the first draft asserted the opposite and the failure was the test's fault, not the
   code's. Do not assume an app element behaves like a control.
3. **The system-wide element is the one target that always exists** and needs no
   permission beyond the Accessibility grant, which makes it the right subject for the
   error-code and absent-attribute tests.
4. **`AXMenuBar` is one attribute read away from any app element**, gives an element with
   actions, and its children report `AXPress`. That is a live, always-available proof of
   the clickability rule, with nothing to launch first.

**Dependency note:** `as_text` needs `CFNumber`, so the `CFNumber` feature has to be named
on `objc2-core-foundation` alongside `CFString` and `CFArray`. Without it, numbers coerce
to an empty string - which would look like an element with no value rather than like a
missing feature.

**Still open from this task:** `is_settable`'s true case cannot be proven from an element
we control until T6 writes a value. The test says so instead of pretending otherwise.

### D13 - T15 done: the voice hook, and the decision that is the hard part (2026-09-20)

**The plumbing was trivial and the decision was not.** CoCo Voice's `external_script`
hook runs one command with the transcript as `argv[1]` and **waits for it to exit**, so
the hook is on the paste path and has about a second. Everything that costs time - a Jev
run - is backgrounded as a detached copy of the hook; everything on the path is a string
match.

**Measured, real binary through the real wrapper:**

| Path | Time |
| --- | --- |
| Decision only (`--dry-run`) | **8 ms** |
| Dictation, including the paste | **0.24-0.33 s** |
| Command hand-off (parent returns) | **6-15 ms** |

**Proof, not assertion:** a scratch TextEdit document received
`Hello from the voice hook: it typed this, didn't it?` byte-identical; the detached child
ran a stub Jev for 2.0 s and logged `exit=Some(0)` after the parent had already returned.

**The decision rule, and why `prefix` is the default.** `external_script` REPLACES typing,
so the hook must decide command or dictation, and the two mistakes are not equal: a
*command* typed into a field is visible and harmless, while *dictation* handed to Jev
disappears into an agent that may act on it. So the default is the exact rule - without the
trigger word ("emma"), everything is dictation - and the heuristic (`classify`: imperative
verb, under 12 words, not a question) is opt-in. Its known false positive, "send it now",
is recorded in a test rather than papered over.

**Three details that would have cost an hour each:**

1. **The child must not inherit the app's pipes.** CoCo Voice closes stdout/stderr as soon
   as it moves on, so a backgrounded Jev run writing to them dies of SIGPIPE partway. The
   child's stdout and stderr go to the log file instead.
2. **`pbcopy` first, `osascript` second.** The keystroke has to come after the clipboard
   write, or Cmd+V pastes the previous contents.
3. **Exit 0 unconditionally.** A non-zero exit is reported by the app as a paste failure,
   which is worse than anything the hook could have done. An unknown flag fails closed -
   logged, nothing pasted - rather than pasting the flag as text.

**Known side effects, stated rather than hidden:**

- The hook overwrites the clipboard on the dictation path. That is what CoCo Voice's own
  `ctrl_v` method does too, and it is why they have a `ClipboardHandling` setting.
- Push-to-talk is bound to **Ctrl**, so if Ctrl is still physically held when the paste
  chord is posted, the accelerator can read as Ctrl+Cmd+V. Unconfirmed: it needs a human
  at the keyboard. Move the binding to Fn or Caps if it shows up.
- On this machine `strip = "symbols"` in the release profile warns: `rust-objcopy` cannot
  find `libLLVM.dylib` in the toolchain. The build is fine; the binary is just unstripped.

**Switched on, and verified as far as a machine can verify it:** the owner approved the
flip, so Coco Voice now runs `paste_method: external_script` with
`external_script_path: …/products/jev-use/scripts/voice-hook` (at the time of writing,
`~/code/jev-use/…`; the repo moved on 2026-09-20 - see D17). The app's own
log confirms it loaded the new settings, not just that the file says so:

```
[..][coco_voice_lib::settings][DEBUG] Loaded settings: AppSettings { ..
  paste_method: ExternalScript, ..
  external_script_path: Some("…/products/jev-use/scripts/voice-hook") }
```

Before the flip, every real dictation in that log read `Using paste method: CtrlV` - which
is what stops being true now.

**The environment is the part worth having tested.** A GUI app does not hand its child a
login shell's environment, so the hook was run under `env -i HOME="$HOME"
PATH=/usr/bin:/bin:/usr/sbin:/sbin"`, through the wrapper, into a scratch TextEdit
document: **exit 0, 0.28 s, byte-identical text.** That rules out the failure mode this
kind of hook usually dies of, a `pbcopy` or `osascript` that is simply not on the PATH.

**Rollback, if dictation ever feels wrong**, in one go - quit, two settings, relaunch:

```
osascript -e 'quit app "Coco Voice"'
python3 - <<'PY'
import json, pathlib
p = pathlib.Path.home()/'Library/Application Support/com.cocoresearch.cocovoice/settings_store.json'
d = json.loads(p.read_text()); s = d.get('settings', d)
### D14 - A public repo brings its own keys. Ours stay ours (2026-09-20)

**The rule, stated by the owner:** *"what we are pushing to the public repo should allow
others to use their own keys, not ours."* Not a security incident - an audit that found
the repo usable only on this machine.

**And the rule was already written down.** `AGENTS.md` section 4 says *never commit* an
API key, token or secret, and never an absolute personal path in shipped code. So this
was not a rule anybody forgot. It was a rule with **nothing able to see a violation**:
the voice hook and `usage-check` were written by an agent that had read the rule, and
both still shipped this machine's assumptions. That is the finding worth keeping - a
rule enforced by memory is a rule that holds until the memory is busy.

**Audit first, and the audit is the evidence:**

| Check | Result |
| --- | --- |
| Key-shaped strings in the working tree | **none** (the only hit was a placeholder that names itself as one) |
| Key-shaped strings across all 29 commits, every blob | **none** |
| `repo-check` credentials scan | pass, 34 files |
| `repo-check` machine paths | **GAP: 3 occurrences in 2 files** - a home directory baked into docs |
| Vendor/account assumptions in code | `usage-check` read our provider list, our wallet topology and our gateway from hardcoded constants |

**What changed, and the principle behind each:**

1. **`usage-check` ships with NO providers.** It lists none, names none and knows none.
   The operator names theirs in `~/.config/jev-use/usage-check.json` (template:
   `scripts/usage-check.example.json`), and the tool reports `none configured` - not
   `none reachable` - when that file is absent, because those are different problems and
   only one of them is alarming. Every path is an env var with a default:
   `JEV_USAGE_CONFIG`, `JEV_USAGE_DB`, `JEV_KEYS_FILE`, `JEV_AUTH_FILE`, `JEV_USAGE_STATE`.
2. **Machine-specific settings live outside the repo.** This machine's providers moved to
   `~/.config/jev-use/usage-check.json`, and the output was diffed against the old
   hardcoded behaviour: same balances, same wallet grouping, same route health. A fork
   now behaves like a fresh install and asks for keys instead of inheriting ours.
3. **`scripts/voice-hook` reads its config from the environment** (`JEV_VOICE_HOOK_BIN`,
   `JEV_USE_BIN`, `JEV_VOICE_MODE`, `JEV_VOICE_TRIGGER`), and
   `scripts/install-voice-hook.sh` installs it into *your* bin directory and prints the
   settings for *your* machine. The hook reads no keys at all - whatever the driver does
   with a goal is the driver's business.
4. **The README states both rules** and the gate enforces them: no credential ever, in
   code, docs, examples or fixtures; no machine-specific path ever.
5. **`.githooks/pre-push.local` runs `repo-check`** - the hook's documented extension
   point - so the whole tracked tree is scanned on every push. That is a different check
   from the shared hook's staged-diff scan: a key committed three commits ago is invisible
   to a diff scan and obvious to this one. `repo-check` now reports `COMPLIANT  every
   check passed`, up from one gap.

**One thing worth knowing about the two scans.** Neither alone is enough. The shared hook
scans the staged diff and so catches a key as it is typed; `repo-check` scans tracked
files and so catches one already in history. Both run on every push now.

**Deliberately left alone, and why:**

- `docs/memory.md` still names vendors in its decision log, because that is what a
  decision log is for; names are not credentials. It is also the one public place where
  this machine's topology is described. Say the word and it gets scrubbed.
- `docs/prd.md` names the project owner. A public repo may name its owner.
- The rule is stated in the README rather than added to `docs/rules.md`. That file says a
  rule change needs the owner's explicit approval, so promoting this to **R11** is a
  decision to make on purpose, not in passing.

s['paste_method'] = 'ctrl_v'; s['external_script_path'] = None
p.write_text(json.dumps(d, indent=2))
PY
open -a "Coco Voice"
```

A timestamped backup of the original file is next to it (`settings_store.json.bak-*`).

**Still unproven, and only a human can close it:** that the app spawns the hook when
speech arrives. Nothing can exercise the microphone path without recording ambient audio,
which is not an agent's call to make. The 5-second test: press **Ctrl**, say *"Emma, open
a new tab"*, release - then `tail -1 ~/Library/Logs/jev-voice-hook.log` should show the
utterance and a `backgrounded pid` line.


### D15 - One settings file, one menu, and the bug the doctor found (2026-09-20)

**The problem with "it works if you know how":** configuration was spread over a wrapper
script, five environment variables and one tool's own JSON file. Nothing was wrong with
any single piece, and there was nowhere to look. No place showing what is in effect, no
way to change a value without knowing which of the three mechanisms owns it, and no way
for somebody who just cloned this to set it up.

**So: one file, one command, one menu.** `~/.config/jev-use/settings.json` holds
everything, `crates/jev-config` reads and writes it, and the precedence is stated once:

```
built-in default  <  settings file  <  environment variable  <  command-line flag
```

| Command | For |
| --- | --- |
| `jev-config` | The menu: numbered rows, current value, one key to change |
| `jev-config show --sources` | Every value, and which layer won |
| `jev-config set <key> <value>` | Validated write; refuses anything it cannot read back |
| `jev-config doctor` | The whole chain: hook, driver, log, voice app, wallets, turn log |

**The doctor found a real bug, on this machine, the first time it ran.** The driver was
configured as the bare name `jev-use`, which resolves through `PATH`. The hook, however,
is spawned by the voice app - and macOS gives a launched application
`/usr/bin:/bin:/usr/sbin:/sbin`, not a login shell's `PATH`. `~/.local/bin` is not in it.
Every voice command would have failed with `cannot run jev-use`, after a transcript that
looked like it had arrived perfectly. Fixed by storing the absolute path, and `doctor` now
warns about the pattern in general rather than about that one instance.

**Two decisions worth keeping:**

1. **`~` is expanded when used and never when written.** The first cut expanded paths on
   load, so the first `set` rewrote `~/.local/bin/...` as an absolute path under one
   machine's home directory, and the file stopped being portable. Load now returns exactly what is on disk, `effective()`
   applies expansion and the environment, and only callers that act ask for it.
2. **A broken settings file does not break dictation.** The hook is on the paste path, so
   a file that does not parse is logged and defaults are used for that run. The loud
   complaint belongs to `jev-config show` and `doctor`, which nobody's typing depends on.

**What the interface does on purpose:** the menu refuses values it cannot validate
(`voice.mode` offers the four that exist, `voice.trigger` rejects a multi-word wake
"word"), re-reads the file after every change so the redraw cannot lie, and prints
`not created yet` rather than inventing a file. With no terminal attached it degrades to
`show` instead of blocking on a keypress that will never come.

**Cost, stated:** one more crate (`serde`, `serde_json`) and about 1 100 lines with tests.
The alternative was documented environment variables, which is what this replaced.


### D16 - T14 done: the macOS path runs in CI now, and what it found (2026-09-20)

**The gap this closes** has been on the board since T1: the accessibility code is
`cfg(target_os = "macos")`, an ubuntu runner never compiles it, and a GitHub-hosted macOS
runner cannot test it either - no logged-in GUI session, no accessibility grant. So the
local pre-push hook was the only thing that ever ran it, which is not a gate.

**Registered `coco-mac-jevu`**: a self-hosted runner for this repo, on this Mac, as a
LaunchAgent so it survives reboots (one runner directory per repo, the pattern the other
four runners on this machine already follow). It picked up its first job **four seconds**
after the workflow was pushed.

**SECURITY, recorded because it is easy to undo by accident.** This repository is public,
and a self-hosted runner executes whatever a workflow asks it to, on a real machine, with
a real logged-in user. A fork pull request must never reach it. The job carries:

```yaml
if: github.event_name != 'pull_request'
    || github.event.pull_request.head.repo.full_name == github.repository
```

Removing that to "make forks work" hands anyone who can open a pull request a shell on
this Mac. The workflow comment says so in the same words.

**What the job found on its first honest run: two environment bugs and one real limit.**

1. **`cargo: command not found`**, while `cargo --version` worked in a terminal. A
   launchd-launched runner does not hand its steps the user's PATH.
2. **Then it still could not find cargo, with the right PATH.** `~/.cargo/bin` is a
   directory of symlinks to `rustup`, and `rustup` is not installed on this machine - so
   `cargo`, `cargo-clippy`, `cargo-fmt` and `rust-analyzer` all dangle. The Rust that
   actually runs is a bare toolchain under `~/.rustup/toolchains/`. The job now derives
   that directory from `$HOME` (no architecture, no home directory in a public workflow)
   and prefers `stable` over whatever `ls | head -1` returns, which was 1.85.0.

   Worth knowing outside CI: `~/.profile` sources `$HOME/.cargo/env`, which puts that
   dangling directory on PATH. Anything launched with that environment finds no cargo.
   Left alone deliberately - it is the user's Rust install, and repairing it is an install
   decision, not an edit.
3. **The runner has no accessibility grant.** The probe answered the question it was built
   for:

   ```
   system-wide     role "AXSystemWide" error None          <- reads work at all
   Finder          role ""             error Some(-25211)  <- APIDisabled
   ```

   That is a TCC grant made by a human in System Settings, so it is an **environment** gap
   and not a code failure - the line this repo already draws in the pre-push hook ("a
   missing tool is an environment problem, not a code problem"). The permission step
   therefore reports instead of failing, the live tests run only when the grant exists, and
   when it does not the run says so twice: a warning annotation and a step summary reading
   **"macOS path: NOT TESTED on this run"**. Nothing claims a pass it did not earn, and
   nothing pretends the code is broken.

**Why the honesty matters more than a green tick:** a skipped test that reports as a pass
is worse than no test, because it buys confidence it did not earn. This job is allowed to
be green about compiling and explicit about not having looked.

**One open item, for a human:** grant `$HOME/actions-runner-jevu/bin/Runner.Listener` in
System Settings -> Privacy & Security -> Accessibility. The moment it is granted, the same
job starts running the live AX tests on real apps, with no change to the workflow.

### D17 - One checkout, in products/ (2026-09-20)

**The repo moved from `~/code/jev-use` to
`~/Rijul Kalra/Coco Research/products/jev-use`, and the old directory is gone.** Not a
tidy-up: the owner asked for the products folder to be the one place work lives, and the
handoff had already flagged the reason - **two checkouts of one repo is how two agents end
up editing different copies of the same file.**

**What the move actually required** (this is the list to check if a path ever moves again):

| Dependency | Action |
| --- | --- |
| CoCo Voice `external_script_path` | Repointed, then **proved** - the app's own log shows the new path loaded, and a real paste landed byte-identical through the wrapper under a GUI-like minimal environment (`env -i`, `PATH=/usr/bin:/bin`) in 0.17 s |
| `~/.pi/agent/AGENTS.md` | Pointer to `docs/handoff.md` repointed |
| `~/code/jev-use` | Removed (moved to the Trash, not deleted - recoverable if something was missed) |
| The self-hosted runner | Nothing to do: it checks out into `actions-runner-jevu/_work/`, not the repo |

**Not required, and worth knowing:** `voice.driver` in the settings points at
`~/.local/bin/jev-use`, which is the *Python reference*, not this repo. The hook binary is
`~/.local/bin/jev-voice-hook`. So the move touched **one** machine-level path, not three.

**Two things the move surfaced, both worth keeping:**

1. **A path with spaces is fine but must be quoted.** The app runs the script through
   `Command::new(path)` - no shell - so spaces are harmless there; every script of ours that
   references it must quote it.
2. **The end-to-end paste test failed twice after the move, for two different reasons
+   that were both the TEST's fault, not the product's.** Getting this right took three
+   runs, and the distinction is the useful part:
+
+   - Failure 1: an empty document and no paste. The test created a TextEdit document and
+     pasted 0.17 s later, sometimes before the document was ready. The hook had run and
+     logged correctly; the clipboard *did* receive the text, and a standalone Cmd+V *did*
+     land, which is how the halves were separated.
+   - Failure 2: the text was there, with junk around it. A leftover document from an
+     earlier debug keystroke was still open, so an exact-match assertion reported a
+     failure that looked like a broken hook.
+
+   The harness now **asserts its own starting state** - every document closed, the front
+   one empty - before it measures anything. Three consecutive runs then matched at 0.15-0.16 s.
+   A test that cannot describe its starting state will eventually report a failure that is
+   worse than useless: a false one, about code that works.

## Dead ends - do not repeat these

### X1 - Do not try to read a browser page through the accessibility tree (2026-09-19)

Measured on Google Chrome: **1246 nodes walked, 17 usable, and the page text came back
completely empty.** Chromium walls off web content unless accessibility is force-enabled.

The toolbar is reachable - new tab, back, reload, the URL bar. **The page is not.** Use
`jev-browse` (CDP) for page content. Do not spend time on `AXManualAccessibility`; it was
refused by both Chrome and Finder on this machine.

### X2 - AppleScript is too slow to observe a UI (2026-09-19)

`osascript` returned **3 elements for PI-Desktop and took ~1200 ms per walk.** The native
`pyobjc` path returns **229 elements in ~300 ms**. Do not reach for AppleScript to
enumerate a tree.

### X3 - A naive Playwright-process check reports phantom browsers (2026-09-19)

`ps | grep playwright_chromiumdev_profile` matches **its own command line**, and the word
"chromium" appears in it, so the check reports a browser that does not exist. Require a
real `.app/Contents/` binary path in the match.

### X4 - Do not trust a `400 unsupported image` to mean the image is bad (2026-09-19)

A session failed repeatedly with `.messages[634].image[0] ... unsupported image`. Every
image in that session was **valid** - correct CRCs, IEND present, decodable by macOS. All
of them, including the largest, were **accepted by the provider when sent directly**.

The error path was the tell: the app reported `.messages[634].image[0]` while a direct API
call reports `input[0].content[1].image_url`. **Different paths mean a different layer** -
a Vercel AI SDK envelope wrapping an OpenAI message. The image was a symptom; the real
problem was a 9 320-message, ~750 K-token session.

Lesson: when an error names a specific index, check whether you are even looking at the
same array layout the app builds. Two compactions had shifted the indices.

### X5 - `f64` cannot derive `Eq` (2026-09-19)

`Rect` holds `f64`, so `Element` cannot derive `Eq`. It derives `PartialEq` only. Caught
by CI on the first commit, which is the system working.

---

## Numbers worth keeping

Recorded 2026-09-19, M1 / 16 GB / macOS 26.5.2. **These are what "parity" means.**

| App | Addressable | Nodes seen | Observation |
| --- | --- | --- | --- |
| Terminal | 20 | 878 | ~470 ms |
| Finder | 11 | 916 | ~200 ms |
| PI-Desktop (Electron) | 229 | 2000 (capped) | ~300 ms |
| Google Chrome (toolbar) | 17 | 1246 | ~380 ms |

| Jev decision | 250-900 ms |
| --- | --- |
| One desktop step, end to end | 0.5-2 s |
| Structured query (Group A) | ~50 ms |

Known-good end-to-end runs:

- `jev-use "open a new tab" --app "Google Chrome"` - CLICK, confidence **1.00**, tabs 2 to 3
- `jev-use "open a new tab" --app Terminal` - CLICK, confidence **0.94**, three steps, 2576 ms
