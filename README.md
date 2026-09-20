# jev-use

**A voice-driven control layer for macOS.** Press a key, speak a goal, and the machine
does it - across any app, not just a browser.

Status: **pre-alpha.** The AX observation layer is being ported to Rust. Nothing here is
usable yet; the working system lives at `~/code/jev-computeruse` (see `reference/`).

## The problem

Three separate problems that look like one:

| # | Problem | Today's cost |
| --- | --- | --- |
| 1 | **The model is the router.** Every agent run re-decides whether to use Jev, and can choose Playwright instead. | Prose in a config file, hoping it is followed |
| 2 | **State is scattered across ~8 apps.** "What is Coco Code doing" means opening an app, finding a thread, reading it. | Minutes per question, by hand |
| 3 | **No single control surface.** Browser, editor, terminal, files, git - separate contexts. | Context-switch tax all day |

Only problem 1 needs Jev. The rest is queries and routing. That distinction is the whole
design: see `docs/prd.md`.

## The shape

```
voice -> intent -> does a purpose-built capability exist?
                       |
                 yes --+-- no
                  |        |
          run it (50 ms)   +--> JEV (0.5-2 s/step, any app, any screen)
```

Most questions are queries, not agent tasks. "Status on Coco Code" is a SQLite read that
takes about 50 ms. Making that an LLM call would be slower and worse. Jev's job is the
**long tail** - anything without a built-in.

## Layout

```
crates/jev-ax/     macOS Accessibility observation + execution (the port target)
docs/              prd, architecture, rules, design, tasks, memory
.metagpt/          STATE.md, GATE.json, interview.md - where we are and what gate we are at
reference/         points at the working Python system this is ported from
.github/           CI, PR template
```

## Working on this

Read `AGENTS.md` first if you are an agent. Read `CONTRIBUTING.md` if you are a person.
The short version:

```bash
cargo test --workspace          # the gate
cargo fmt --all                 # before every commit
cargo clippy --workspace --all-targets -- -D warnings
```

**Every change lands as a small PR with CI green.** No direct pushes to `main`. A PR
without a passing run is not reviewable - see `.github/pull_request_template.md`.

## Licence

UNLICENSED. Private. Not for distribution.
