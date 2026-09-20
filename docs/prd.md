# PRD - jev-use

**Status:** draft, awaiting owner approval
**Owner:** Rijul
**Last updated:** 2026-09-19

---

## 1. One sentence

A voice-driven control layer for macOS: press a key, speak a goal, and the machine does
it across any app - with Jev as the universal fallback for anything that has no
purpose-built path.

## 2. The problem, precisely

Three problems that get mistaken for one:

### P1 - The model is the router
Every agent run re-decides whether to use Jev. Today that decision is a paragraph in a
config file, and the model can ignore it and reach for Playwright instead. There is no
way to make it deterministic with prose.

**Fix:** wire the pipeline. A hotkey is a fact, not a suggestion.

### P2 - State is scattered
"What is Coco Code doing" means opening an app, finding a thread, reading it. The answer
usually already exists in a database or a git repo.

**Fix:** a capability registry. Query the source directly.

### P3 - No single control surface
Browser, editor, terminal, files, git - separate contexts, no shared entry point.

**Fix:** one hotkey, one input, spoken.

## 3. Non-goals for v1

Explicitly **not** building:

- Multi-user. Single user, single machine.
- A cloud service. Everything local.
- Mobile or web. macOS only.
- A general agent framework. This solves one person's workflow well.
- Wake-word or always-listening. Push-to-talk only, for privacy and battery.
- Proactive monitoring ("tell me when CI breaks"). That is a daemon, not a hotkey app.
  Deferred, not refused.

## 4. The core idea

**A capability registry with a voice front door, where Jev is the fallback.**

```
voice -> intent -> does a purpose-built capability exist?
                        |
                  yes --+-- no
                   |        |
           run it (50 ms)   +--> JEV (0.5-2 s/step, any app, any screen)
```

Most requests are queries, not agent tasks. Measured: *"what is the status on Coco Code"*
answered in about 50 ms from SQLite plus one git call - no AX walk, no screenshot, no
vision model, no Jev.

**Jev's job is the long tail.** Anything with no built-in path goes through it. That is
what makes this a platform rather than a collection of scripts, and it is why the port
matters.

## 5. Use cases, ranked

### Group A - READ: answer from data (~50 ms). No Jev.
| You say | Source | Status |
| --- | --- | --- |
| "Status on Coco Code" | project DB + git | **proven** |
| "What did I work on yesterday?" | last-opened timestamps | **proven**, data already exists |
| "Any uncommitted work in Hermes?" | git status | **proven** |
| "What is running right now?" | process list | trivial |

Highest value, lowest effort. **This is v0.1.**

### Group B - LAUNCH: open or focus (~200 ms)
"Open Coco Code" - "Switch to Terminal" - "Show me the Hermes folder"

Resolve a name to a path, launch or focus. Jev only if the app is not in the registry.

### Group C - ACT: drive an app's own controls (0.5-2 s/step, Jev + AX)
"New tab in Chrome" - "New note in Notes" - "Click Settings in PI Desktop"

**Proven working.** Native apps expose their controls; Electron exposes a real tree.

### Group D - WEB: the page inside a browser (needs CDP)
"Search Hacker News for X" - "Open the top story"

**Requires `jev-browse`.** The page content is invisible to the accessibility tree.
Measured: Chrome exposes 1246 nodes, 17 usable, and **zero** page text. For a consumer
app this means bundling a browser or launching Chrome with a flag - a real scope
decision, not a detail.

### Group E - WATCH: proactive (deferred)
"Tell me when Hermes finishes." Buildable, but it is a daemon that runs without you.
Out of v1.

## 6. The measured constraints

Real numbers from this machine, 2026-09-19. **The PRD states these so the product does
not lie about latency.**

| Thing | Number |
| --- | --- |
| AX observation per step | 200-1100 ms |
| Jev decision | 250-900 ms |
| **One desktop step, end to end** | **0.5-2 s** |
| A 5-step task | **5-15 s** |
| Structured query (Group A) | **~50 ms** |
| Chrome page text visible to AX | **zero** |
| Terminal usable elements | 20 of 878 |
| This machine | M1, 16 GB |

**Consequence:** a model cannot run locally. DeepSeek must be an API call.

## 7. Phasing

| Version | Scope | Needs Jev? | Needs AX permission? | Needs the Rust port? |
| --- | --- | --- | --- | --- |
| **v0.1** | Group A - voice to answer about your projects | No | No | No |
| **v0.2** | Group B - launch and focus | No | No | No |
| **v0.3** | Group C - act via AX | Yes | Yes | Yes |
| **v0.4** | Group D - web via CDP | Yes | No | No |
| **v1.0** | Task list, history, the glass UI | - | - | - |

v0.1 is a localhost page with a microphone. It ships in about a day and is genuinely
useful on its own.

## 8. Success criteria

- v0.1: asking three different project-status questions by voice gives correct answers
- v0.3: "new tab in Chrome" works ten times in a row without a misfire
- Parity: the Rust port matches the Python reference element-for-element on four apps
- Never: an agent in this repo uses Playwright

## 9. Open questions

1. Which speech-to-text engine - Apple's on-device, or `whisper-rs`?
2. Push-to-talk key: Fn, or a modifier combination? Fn is not reliably capturable.
3. Does the app bundle a browser for Group D, or drive the existing Chrome?
4. How does the user grant Accessibility permission without a signed build, given an
   unsigned app loses that grant on every rebuild?
