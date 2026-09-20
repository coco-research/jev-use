# Interview

The questions asked before any design work, and the answers as given. Recorded so the
plan can be traced back to what was actually wanted, not what was assumed.

---

## Q1 - What is this, in your words?

> "A personal assistant on the app. You know the concept of Jarvis. With Jev the agentic
> use cases for browser and computer are already solved. And AI with a model capable as
> you, we already have Jarvis. We just need a front end to monitor everything and
> basically a computer that can listen to you, converts your voice into actual usable
> content for the AI to ingest and takes actions on its behalf."

Recorded: **Emma is Jarvis.** Jev is the hands, a model is the mouth and brain, the user
is the voice.

## Q2 - What does the interaction feel like?

> "I should be able to just press the function key and just talk to you, or switch on my
> microphone and talk to you, and you operate my computer."

Recorded: push-to-talk on a single key. No wake word, no chat window.

## Q3 - Give me the flagship example.

> "I am working on 10 different projects - Coco, Hermes, NG, Coco Code. I can just say,
> hey Emma, what is the status on Coco Code? Can you open the particular app we were
> working on yesterday and check what it is."

**This answer reshaped the architecture.** It is a *read* task, not an action task. Built
and measured during the interview: the answer came from a SQLite query plus one git call
in about **50 ms**, with no Jev, no screen-reading, and no vision model.

## Q4 - Why an app rather than a skill?

> "I have to give special instructions and there are chances that the agents do not follow
> it. jev-use and jev-check are not slash commands. It is still up to the model to
> understand what they mean."

**Confirmed by measurement.** The Jev policy was at line 286 of a 350-line config file. It
was moved to line 7 the same day. That is the best a text file can do, and it still
depends on an agent reading, remembering, and choosing correctly.

A hotkey is a fact. A paragraph is a suggestion. **That is the product thesis.**

## Q5 - What must not be compromised?

> "All backend needs to be written in Rust. No compromises there."

Recorded, and honoured. When offered a faster path (Python prototype first, port later),
the owner chose the port first.

## Q6 - What had to be settled before design?

Three constraints were measured rather than assumed, and each changed the plan:

1. **Liquid Glass cannot be built here.** `swiftc -typecheck` on `.glassEffect()` fails.
   Needs Xcode 26 and the macOS 26 SDK; this machine has SDK 11.3, Swift 5.4, and 29 GB
   free against a ~40 GB requirement. Resolved by choosing Tauri, which gives real
   backdrop blur through WKWebView.
2. **Desktop is slower than browser.** One step is 0.5-2 s, not 400 ms. The PRD states
   this so the product does not promise otherwise.
3. **A browser page is invisible to the accessibility tree.** Chrome exposes its toolbar
   and nothing of the page. Group D needs CDP, which is a scope decision, not a detail.

## Q7 - What is explicitly not wanted?

- Playwright, for anything. ("My models or agents will not use Playwright anymore.")
- Always-listening wake word.
- A chat interface.

## Open questions carried forward

1. Speech-to-text engine: Apple on-device, or `whisper-rs`?
2. Hotkey: Fn is not reliably capturable. Which key?
3. Group D: bundle a browser, or drive the existing Chrome?
4. Accessibility permission without a signed build - an unsigned app loses the grant on
   every rebuild. This is a product problem, not a code problem.
