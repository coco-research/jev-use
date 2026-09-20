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
| 2 | **State is scattered across ~8 apps.** "What is payments-api doing" means opening an app, finding a thread, reading it. | Minutes per question, by hand |
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

Most questions are queries, not agent tasks. "Status on payments-api" is a SQLite read that
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

## Settings

One file, one command, and a menu for the times you do not want to remember a key name.

```bash
jev-config                  # the menu: numbered rows, current value, one key to change
jev-config doctor           # check the hook, the driver, the voice app and your wallets
jev-config set voice.trigger computer
jev-config show --sources   # every value, and whether it came from default/file/env
```

The file is `$JEV_SETTINGS_FILE`, else `$XDG_CONFIG_HOME/jev-use/settings.json`, else
`~/.config/jev-use/settings.json`. It is plain JSON you can edit by hand; a file that does
not parse is reported loudly rather than quietly ignored, and a misspelled key is an error
rather than a setting that silently does nothing.

```json
{
  "voice": {
    "mode": "prefix",              // prefix | classify | dictation | command
    "trigger": "emma",             // the wake word
    "driver": "/path/to/your/driver",
    "hook_bin": "~/.local/bin/jev-voice-hook",
    "log": "~/Library/Logs/jev-voice-hook.log"
  },
  "usage": {
    "db": "~/.pi-desktop/pi.sqlite",
    "keys_file": "~/.secrets/ai-keys.env",
    "auth_file": "~/.pi/agent/auth.json",
    "state": "~/.pi/agent/usage-history.json",
    "providers": [],               // your wallets; see usage-check.example.json
    "gateway": null                // optional LiteLLM-compatible /health probe
  }
}
```

**Precedence:** built-in default < settings file < environment variable < command-line flag.
`jev-config show --sources` prints which one won for every value, so "why is it doing
that" has an answer.

Two things the tool is deliberately strict about:

- **`~` stays `~`.** Values are expanded when used and never when written, so the file
  does not turn into a pile of absolute paths the first time you change one setting.
- **A bare driver name is flagged.** macOS gives a launched app
  `/usr/bin:/bin:/usr/sbin:/sbin`, not your shell's `PATH`, so a driver in
  `~/.local/bin` resolves in your terminal and not in the process that actually runs it.
  `jev-config doctor` says so and prints the absolute path to use.

## Keys: bring your own

**This repo contains no keys, and nothing in it will ever need ours.** Every component
reads credentials from your machine, in the form you choose:

| What | Where it looks | Override |
| --- | --- | --- |
| `scripts/usage-check` | Providers **you list** in the `usage` section of the settings file (`jev-config`), or from its own older file. Ships with none. | `JEV_SETTINGS_FILE`, `JEV_USAGE_CONFIG`, `JEV_KEYS_FILE`, `JEV_AUTH_FILE`, `JEV_USAGE_DB` |
| `scripts/voice-hook` | Nothing. It reads no keys and calls no API; it decides, then hands a goal to *your* driver - both from the settings file. | `JEV_SETTINGS_FILE`, `JEV_VOICE_HOOK_BIN`, `JEV_USE_BIN`, `JEV_VOICE_MODE`, `JEV_VOICE_TRIGGER` |
| `crates/jev-ax` | Nothing. Accessibility reads, no network at all. | - |
| The driver that answers a goal | Whatever you already use. This repo ships none yet (T7). | - |

Two rules follow, and the gate enforces them:

1. **No credential is ever committed**, in code, docs, examples or test fixtures. The
   pre-push hook scans for key shapes, and `repo-check` scans every tracked file.
2. **No machine-specific path is committed.** `$HOME`-relative or an environment variable
   with a default - never someone's home directory baked in. `repo-check` fails on those.

Config lives outside the repo (`~/.config/jev-use/`), which is why a fork behaves like a
fresh install and asks for your keys instead of inheriting ours.

## Scripts

| Script | What it does |
| --- | --- |
| `scripts/install-voice-hook.sh` | Builds and installs the hook and `jev-config`, writes a settings file, then prints what to put in CoCo Voice's settings. |
| `scripts/voice-hook` | The file CoCo Voice runs per transcript. Forwards it; all configuration is in the settings file. |
| `scripts/repo-check` | Audits a repo against the standing standard: required files, credentials, machine paths, docs. |
| `scripts/usage-check` | Where tokens and money went, from your own providers and the local turn log. |

## Licence

Apache-2.0. Chosen over MIT for the explicit patent grant, which matters for a project
that depends on other people's models and drivers. See `LICENSE`.

## Relationship to other projects

To avoid any confusion about what is official:

- **Jev** is TypeSafe's System One model. It is not ours, and this repo does not wrap or
  fork it.
- **`browser-use/jev-ultrafast`** is the upstream project this repo's browser counterpart
  comes from. The decision layer is **imported from it, never modified**. This repo is
  not an official Browser Use or TypeSafe project.
- **What is ours** is `crates/jev-ax`: the macOS Accessibility observation and execution
  layer, written in Rust, shaped so that decision layer can drive the desktop instead of
  a browser.
