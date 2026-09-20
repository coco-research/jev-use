# Rules

**The invariants. These do not change without the owner's explicit approval.**

Every rule here has a **measurement** behind it. They are not stylistic preferences; each
one exists because breaking it caused a real, observed failure. If a task appears to
require breaking one, stop and ask.

Any PR that changes a rule in this file must say so in its title.

---

## R1 - An element earns an index only if it is actionable AND enabled AND on screen

**Enforced by:** `Element::is_addressable`, tested in `crates/jev-ax/src/lib.rs`

**Measured:** Terminal reports **20 addressable elements of 878 nodes**. Chrome **17 of
1246**. Drop the on-screen test and you get 207 candidates, roughly 150 of them invisible
menu items.

A closed menu reports no rect at all. Hidden tab content sits off-screen. Collapsed rows
are degenerate. All three must be rejected.

**Why it matters:** an element you can see but cannot use must never become a target, or
Jev is invited to pick something that cannot be executed - and the failure looks like a
bad decision rather than a bad candidate list.

---

## R2 - The messaging timeout is mandatory, and it is set before the walk

**Enforced by:** `Limits::messaging_timeout_secs`, default 2.0

**Measured:** you cannot cancel a blocked `AXUIElementCopyAttributeValue`.

**Why it matters:** that timeout is the *only* thing stopping one unresponsive app from
pinning a worker thread forever. There is no cancellation path. Removing it does not
degrade performance, it hangs the process.

---

## R3 - Windows are seeded explicitly, not discovered through children

**Enforced by:** the walk unions `AXChildren` with `AXWindows`

**Measured:** background windows are absent from `AXChildren`.

**Why it matters:** miss this and a backgrounded app returns an empty tree, which reads
as "this app has no UI" rather than "we asked the wrong question". Every debugging hour
spent on that is wasted.

---

## R4 - AXValue write first, keystrokes only as fallback

**Enforced by:** `Element::settable`; the executor tries the write, then types

**Measured:** macOS blocks synthetic keystrokes into secure input fields; AX writes are
not keyboard events, so they still land.

**Why it matters:** the write is atomic and leaves no partial state. Typing is a fallback
for rich-text and web-backed editors that report `AXValue` as unwritable - not the
default. Reversing the order makes every plain text field slower and less reliable.

---

## R5 - The repeat guard requires BOTH a repeated operation AND an unchanged screen

**Enforced by:** the agent loop, comparing `ElementTable::fingerprint` between steps

**Measured:** clicking "new tab" three times legitimately opens three tabs.

**Why it matters:** keying on the operation alone makes a working action look like a loop
and stops a good run. Keying on the screen alone misses a true spin. Both halves are
required.

---

## R6 - The walk is bounded

**Enforced by:** `Limits` - depth 25, nodes 2000, text 6000 chars

**Measured:** an Electron app produced a 10k+ node tree that blows the context window.

**Why it matters:** the caps bound worst-case cost. `truncated: true` is reported so a
caller knows the tree is incomplete rather than assuming it saw everything.

---

## R7 - A secret never reaches a log, an error message, or argv

**Enforced by:** review, and the PR checklist

**Why it matters:** keys live in `~/.secrets/ai-keys.env` and `~/.pi/agent/auth.json`.
The app **reads them by path and never copies them**. Nothing in this repo stores,
prints, or transmits a key. The existing gateway returns `x-litellm-model-api-base` on
every response, so a fallback is visible rather than silent.

---

## R8 - Parity with the reference is the specification

**Enforced by:** the parity harness, run in PRs that touch observation

**Measured:** the Python driver is running and gives known numbers (see `reference/`).

**Why it matters:** this is a port. "It looks better" is not a reason to diverge from a
working reference. If the Rust output differs, either it is a bug or the difference is
deliberate and documented in the same PR.

---

## R9 - Playwright does not exist in this project

**Enforced by:** `jev-check`; the Jev policy in `AGENTS.md`

**Measured:** a Playwright `fullPage` screenshot injected 920 KB of image into a 750 K
token context and broke a session with a `400 unsupported image` error.

**Why it matters:** browser and desktop work goes through `jev-browse` and `jev-use`.
On this machine the ban is policy; in this repo it is also a correctness rule.

---

## R10 - Fail loudly. Never silently substitute.

**Enforced by:** review

**Examples:**
- A missing capability falls through to Jev. It does not guess.
- A class with one route fails honestly rather than drifting to a different model.
- A provider that is down reports the error. It does not try a different provider.

**Why it matters:** every silent substitution becomes an hour of debugging later.
