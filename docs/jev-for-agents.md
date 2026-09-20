---
name: jev
description: >-
  Drive this Mac — any app or a browser page — using Jev, a fast selection model.
  Use when a task requires clicking, typing, navigating, or reading a screen:
  "open X in Chrome", "click the Settings button in <app>", "fill that form",
  "open a new tab", "navigate to <url> and do <thing>", "drive <app>". Covers the
  five commands, what Jev can and cannot see, measured limits, and the hard rules
  (Playwright is banned here). Read this before attempting any GUI automation.
---

# Jev — how to drive this Mac

Jev is a **selection model**, not a chatbot. Given a numbered list of things on screen
and a goal, it returns **one operation and one element index** in about 400 ms. Code
then executes that choice. It never writes prose, never reasons step by step, and has
no memory between calls.

Everything below is measured on this machine. Numbers are real, not estimates.

---

## 0. The one thing to get right

**Jev picks. Your code executes. They are separate jobs.**

```
goal ──> [ observe the screen ] ──> Jev picks operation + index ──> code executes
             ~200–1100 ms                  ~250–900 ms              exact element
```

Do not ask Jev to "plan". Do not expect it to explain. Ask it *which one*, then act.

---

## 1. The five commands

| Command | Use it for |
| --- | --- |
| `jev-use "goal" --app <App>` | Drive any macOS app: click, type, menus, dialogs |
| `jev-browse <url> "goal"` | Drive a **browser page** (the web content itself) |
| `jev-use --table --app <App>` | Print the element table. **No model call. Free, instant.** |
| `jev-use --apps` | List apps that have a UI |
| `jev-check` | Audit that everything is installed and working |

### Choosing between `jev-use` and `jev-browse`

**This is the most common mistake.**

| You want to | Use |
| --- | --- |
| Click Chrome's own Back / Reload / New tab / address bar | `jev-use` |
| Open a URL | `open <url>` — a shell command, not Jev |
| Interact with what is **inside** a web page (links, forms, results) | `jev-browse` |
| Drive Notes, Terminal, Finder, Preview, any native app | `jev-use` |

**Chrome is two programs in one window.** Its toolbar is reachable by `jev-use`. The web
page inside is **not** — measured: 1246 nodes walked, 17 usable, and the page text came
back **completely empty**. The page needs `jev-browse`, which reads the DOM instead.

---

## 2. Before you drive anything: look first

`jev-use --table --app <App>` costs nothing and answers "can Jev even see this app?" in
under a second. **Always do this before attempting a goal in an unfamiliar app.**

```bash
jev-use --table --app Terminal
```

```
Terminal — rijulkalra — -zsh — 80×24  (465ms, 20 addressable of 878 nodes)
  [1] radiobutton    ~ — -zsh ="True" [x]
  [2] button         Close tab
  [3] button         new tab
  [4] button         Button
  ...
```

Twenty addressable elements out of 878 nodes. The rest were rejected — hidden, disabled,
or off screen. **That filtering is the point:** an element you can see but cannot use must
never become a target.

---

## 3. Hard rules — do not violate these

### 3.1 Playwright is banned on this machine

Not for screenshots, not for driving a page, **not as a fallback**.

`playwright`, `agent-browser`, and `plugin_pi_browser_*` are not tools for agent work
here. If a task needs GUI automation, it is `jev-use` or `jev-browse`.

Why it matters concretely: a Playwright `fullPage` screenshot once injected 920 KB of
image into a 750 K-token context and broke a session with an error that took an hour to
diagnose. The page content was invisible in the accessibility tree, and it chose to read
the pixels instead.

### 3.2 Never touch these repos

| Repo | Why |
| --- | --- |
| `cocoteams` | **AGPL-3.0, reference-only, frozen.** The product is rebuilt clean-room at `cocoteams-ng`. Work done here cannot be moved across the licence. |
| `jev-ultrafast` | Not ours — `browser-use/jev-ultrafast`. Read-only reference. |
| `OpenMAIC` | Not ours — `THU-MAIC/OpenMAIC`. |
| `dsh`, `hermes-agent`, `goose`, `twenty`, `Stirling-PDF`, `raganything` | Forks of other projects. |

### 3.3 Astra requires a human yes, every time

`gpt-6-astra` is expensive review-only. Never select it as a default, never reach for it
because something else failed. Ask first.

### 3.4 A sub-0.70 confidence decision is a guess

Below 0.70 the lane is not trustworthy. Say so rather than acting on it.

---

## 4. What Jev can see, precisely

Jev does not see pixels. It sees an **element table** built from the macOS Accessibility
tree — role, name, current value, and whether the element is actually usable.

```
Google Chrome — 1246 nodes walked, 17 usable, 385 thrown away as hidden
  [13] button     Back
  [14] button     Reload this page
  [15] button     Search or type URL
  [16] button     Bookmark this tab
  [17] button     New tab
```

Given this plus a goal, Jev answers something like `CLICK [17]`. Code presses element 17.
Then the whole thing repeats with a fresh table, because the screen just changed.

### Measured element counts

| App | Addressable | Nodes walked | Observation time |
| --- | --- | --- | --- |
| Terminal | 20 | 878 | ~470 ms |
| Finder | 11 | 916 | ~200 ms |
| PI-Desktop (Electron) | 229 | 2000 (capped) | ~300 ms |
| Google Chrome (toolbar only) | 17 | 1246 | ~380 ms |

---

## 5. Measured limits — do not promise anything better

| Thing | Reality |
| --- | --- |
| One Jev decision | **250–900 ms** |
| One desktop step, end to end | **0.5–2 s** |
| A 5-step task | **5–15 s** |
| A structured query (see below) | **~50 ms** |
| Browser page, via `jev-browse` | ~400 ms per decision |

**Desktop is not as fast as the browser.** The browser path is ~400 ms per step; the
desktop path is 0.5–2 s because it must walk the accessibility tree. Do not promise
browser numbers for desktop work.

### What does not work

- **Chromium apps with accessibility off** expose almost nothing. PI-Desktop exposes 229
  elements; a Chromium app with a11y disabled returns a near-empty tree.
- **An app with no open window** exposes only its menu bar. That is correct, not a bug.
  Verify with `--table` before blaming the loop.
- **Very deep trees truncate** at 2000 nodes / depth 25. A long message list will cut off.
- **Secure input fields** block synthetic keystrokes. Direct value writes still work, and
  the executor tries that first — but a password field is still not a good target.

---

## 6. Not everything needs Jev

**Check this before reaching for GUI automation.** Most "status" questions are queries,
not agent tasks.

| Task | Right tool | Time |
| --- | --- | --- |
| "What is the status of project X?" | read its database / `git status` | **~50 ms** |
| "What did I work on yesterday?" | query stored timestamps | **~50 ms** |
| "Any uncommitted work in repo Y?" | `git -C <repo> status --porcelain` | **~50 ms** |
| "Open app Z" | `open -a "<App>"` | instant |
| "Open a URL" | `open <url>` | instant |
| "Click the Settings button in app Z" | **`jev-use`** | 0.5–2 s |
| "Search for X on site Y" | **`jev-browse`** | ~400 ms/step |

Driving a GUI to read a value that a database already holds is slower and less reliable.
**Prefer the direct read. Use Jev for what has no direct path.**

---

## 7. Worked examples

### Open a new tab in Chrome

```bash
jev-use "open a new tab" --app "Google Chrome" --max-steps 2
```

```
goal:  open a new tab
app:   Google Chrome
route: openrouter

   1 · CLICK      1.00  New tab        363ms obs + 850ms jev
   2 · CLICK      1.00  New tab        642ms obs + 284ms jev
stopped: ok after 2 step(s), 2258ms total
```

Verified: Chrome's tab count went 2 → 3.

### Open a new Terminal tab

```bash
jev-use "open a new tab" --app Terminal
```

Jev chose `[4] AXButton "new tab"`, confidence 0.94, three steps, 2576 ms.

### Look without spending anything

```bash
jev-use --table --app "Google Chrome"
```

No model call. Free. Always start here for a new app.

### Check the install is healthy

```bash
jev-check
```

```
pass  policy in AGENTS.md          Playwright banned; commands named in the first 3000 chars
pass  accessibility permission     granted
pass  Jev route                    openrouter (key present)
pass  desktop driver (jev-use)     6 apps visible, e.g. Terminal
pass  orphaned browsers            none running
```

---

## 8. Troubleshooting

### Nothing works at all

```bash
jev-check
```

Read the failures in order. The usual causes:

| Symptom | Cause | Fix |
| --- | --- | --- |
| `jev-use --apps` errors | Accessibility permission missing | Grant it in System Settings → Privacy & Security → Accessibility |
| Jev route fails | No key on either route | Check `~/keys/typesafe.txt` or the OpenRouter entry in `~/.pi/agent/auth.json` |
| App in `--apps` but empty table | No window open, or a11y disabled | Open a window first |

### Jev picks the same thing repeatedly and nothing happens

That is the loop guard doing its job. Three identical operations **with an unchanged
screen** stops the run. Three identical clicks that each change the screen are legitimate
— clicking "new tab" three times opens three tabs.

If it stops, the target is probably not doing anything. Re-run `--table` and look at what
is actually there.

### An app returns errors on every attribute

`AXError(-25204)` = `CannotComplete` means the app did not answer in time. PI-Desktop does
this reproducibly while other apps answer normally. It is the app being busy, not Jev
failing. Retry, or use a different app.

Other codes worth recognising:

| Code | Name | Meaning |
| --- | --- | --- |
| `-25204` | `CannotComplete` | app did not answer in time |
| `-25205` | `AttributeUnsupported` | normal — e.g. an app element has no position |
| `-25211` | `APIDisabled` | the process lacks Accessibility permission |

---

## 9. How the pieces fit

```
  voice ──▶ intent ──▶ does a purpose-built capability exist?
                            │
                      yes ──┴── no
                       │        │
               run it (50 ms)   └──▶ JEV (0.5–2 s/step, any app, any screen)
```

**Jev is the fallback, not the router.** Most requests are queries with a direct answer.
Jev exists for the long tail — anything with no built-in path. That is what makes it a
platform rather than a pile of scripts.

The wider plan is a voice-driven control layer for macOS: press a key, speak a goal, it
happens. Jev is the hands. Everything is local; nothing leaves the machine except the
model call itself.

---

## 10. The short version

1. **Jev answers one question: which button.** Ask it that; do not ask it to plan.
2. **Look first.** `jev-use --table --app <App>` is free and answers "can it even see
   this?".
3. **Chrome's toolbar is Jev-able. The web page is not** — that needs `jev-browse`.
4. **Most status questions are database reads, not GUI tasks.** Check before automating.
5. **Playwright is banned. Never fall back to it.**
6. **Never touch `cocoteams`, `jev-ultrafast`, or `OpenMAIC`.**
7. **Astra needs a human yes.** Every time.
8. **Desktop is 0.5–2 s per step.** Do not promise browser speed.
