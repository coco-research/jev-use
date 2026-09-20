# AGENTS.md - read this before touching anything

**Taking this repo over? Read `docs/handoff.md` first.** It is the entry point a new agent
starts from: what exists, what is in flight, what is open, what to verify, and which side
of the ownership boundary this repo is on.

Rules for any agent working in this repo. They are not suggestions.

## 0. Before this file: the standard

`docs/repo-playbook.html` is the **source of truth for the standard itself** — the standing
rules, the gate, and the documents every repo carries. `scripts/repo-check` is the tool that
enforces it, and the playbook declares the checks that tool runs, so the two cannot drift
apart unnoticed.

Run it before you claim anything is done:

```bash
python3 scripts/repo-check            # must say COMPLIANT
```

## 0. The five documents, and when each is authoritative

| File | Answers | Update when |
| --- | --- | --- |
| `docs/prd.md` | What and why | Scope changes |
| `docs/architecture.md` | How the parts fit | Structure changes |
| `docs/rules.md` | The laws. Invariants that must not break | **Only with owner approval** |
| `docs/design.md` | What it looks and feels like | UI work |
| `docs/tasks.md` | What is being worked on now | Every PR |
| `docs/memory.md` | What was decided and what failed | Every PR that settles something |
| `.metagpt/STATE.md` | Where we are, next action | Every session end |

If two documents disagree, the earlier one in that table wins. Never silently pick one -
fix the disagreement in the same PR.

## 1. Jev drives the machine. Playwright does not exist here.

```
jev-use "goal" --app <App>        drive any Mac app (Accessibility tree)
jev-browse <url> "goal"           drive a web page (CDP)
jev-check                         audit that this is still true
```

**Playwright is banned.** Not for screenshots, not for driving a page, not as a fallback.
This includes `agent-browser` and `plugin_pi_browser_*`.

Why it matters *in this repo specifically*: a Playwright `fullPage` screenshot once
injected a 920 KB image into a 750 K-token context and broke a session with a 400 error
that took an hour to diagnose. See `docs/memory.md`.

## 2. The PR contract

- Every change is a **small PR**. No direct pushes to `main`.
- After clone: `git config core.hooksPath .githooks` so `.githooks/pre-push` runs (see `CONTRIBUTING.md`). Bypass only with cause, documented in the PR.
- **CI green before review.** Three jobs: `fmt + clippy`, `test`, `docs`.
- **Evidence, not assertions.** Paste the command and its real output.
- One concern per PR. If you touched two things, that is two PRs.
- The PR template has the checklist. Fill it in honestly, including the Risk section.

## 3. The parity rule

`crates/jev-ax` is a **port**, not a rewrite. It has a working reference:

```
python jev_desktop --table --app Terminal --json   ->  20 addressable of 878 nodes
```

Any PR changing observation output must show the parity harness result and explain the
divergence. "It looks better" is not a reason; the reference is the specification.

## 4. Never commit

- Any API key, token, or secret. Not in code, not in docs, not in a test fixture.
- Absolute personal paths in shipped code (`/Users/<name>/...`). Docs may name a path as
  an example; code must not hardcode one.
- Build output. `target/` is ignored and must stay ignored.

## 5. When you are unsure

`docs/rules.md` holds the invariants, each with the measurement that justifies it. If a
task appears to require breaking one, **stop and ask** - do not decide alone. Those
numbers came from real runs and are not stylistic.

## 6. Recording what you learned

`docs/memory.md` is not a diary. It records **decisions with reasons** and **dead ends
worth not repeating**. If you spent an hour discovering that a hypothesis was wrong, that
sentence saves the next agent the hour.
