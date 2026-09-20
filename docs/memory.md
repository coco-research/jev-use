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

**Why:** the flagship use case - *"what is the status on Coco Code"* - was answered in
about **50 ms** from SQLite plus one git call. No AX walk, no screenshot, no vision model,
no Jev. Making that an LLM call would be slower and worse.

**Consequence:** v0.1 needs neither Jev nor Accessibility permission.

### D4 - The decision layer is imported, never forked (2026-09-19)

**Chosen:** `~/code/jev-ultrafast/` stays pristine; its `model.py` and `questions.py` are
imported by path.

**Why:** `jev_ultrafast/model.py` is transport-agnostic - it asks one question and
validates the answer against the candidates offered. Only its browser layer is
CDP-specific. Forking it would let the two drift.

### D5 - Repo is private (2026-09-19)

**Why:** the documents describe machine layout, project names, the routing policy, and
key names. Other `coco-research` repos are public; this one should not be until scrubbed.

### D6 - CI runs three jobs, and lint fails fast (2026-09-19)

**Chosen:** separate `lint` (fmt + clippy), `test`, `docs` jobs, on a macOS runner.

**Why:** formatting feedback should arrive in about a minute, not after a full build. And
`cargo doc` is a job because the crate is where the design lives - a doc comment that does
not compile is a broken contract.

### D7 - The pre-push hook is the gate; CI is the backstop (2026-09-20)

**Chosen:** `.githooks/pre-push` is the gate. It runs fmt, clippy, test, docs, a secret
scan, and (once it exists) the parity harness. The CI workflow is kept ready but its
triggers are **manual**, because it cannot run.

**Why, found the hard way:** the very first push to this repo failed in CI with
*"The job was not started because an Actions budget is preventing further use."*

**My first diagnosis was wrong, and the correction matters.** I assumed it was the
macOS multiplier, since `coco-research/coco` and `coco-connect` run CI fine - but those
are **public**, and public repos get unlimited Actions minutes. I switched the runner to
`ubuntu-latest` at 1x, pushed again, and it **still failed with the same message.** So
the budget is exhausted outright, not merely expensive. Do not re-litigate this by
swapping runners; it will not help.

**The deeper reason this split is right design, not just a workaround:** a GitHub-hosted
macOS runner *cannot test the accessibility code at all.* It has no logged-in GUI session
and no Accessibility permission, so every `AXUIElement` call fails there regardless of
runner. The behaviour this project cares about is only testable on this machine.

**So the split is deliberate:**

| Where | Tests | Cost |
| --- | --- | --- |
| Pre-push hook (local) | Everything, including AX against real apps | free, ~1.5 s warm |
| CI (ubuntu, manual) | Pure logic, types, fmt, lints, docs | **currently refused - budget exhausted** |

The crate is kept free of macOS-only dependencies **on purpose**, so the logic stays
testable in CI. macOS-only code goes in a module gated by `cfg(target_os = "macos")` and
is covered locally.

**Install the hook once per clone:** `git config core.hooksPath .githooks`

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
