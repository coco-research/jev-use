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

---

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
