<div align="center">

<img src="docs/readme/hero-dark.png" alt="jev-use — Coco Research company mark, repo standard" width="840"/>

# jev-use

**Coco Research · Repo Standard · playbook home**

`company mark · standing instructions · not a product app`

![license](https://img.shields.io/badge/license-Apache--2.0-5b6169?labelColor=0a0a0a&style=flat-square)
![status](https://img.shields.io/badge/status-pre--alpha-5b6169?labelColor=0a0a0a&style=flat-square)
![role](https://img.shields.io/badge/role-repo%20standard-5b6169?labelColor=0a0a0a&style=flat-square)

</div>

> **Repo Standard / playbook home for Coco Research agents and humans.**
> Hero mark = Coco Research company mark (`assets/logo.svg` interlocked C-arcs). **Not** a shipped Mac product. Pre-alpha.

<img src="docs/readme/local-first.png" alt="Repo Standard → Playbook → macOS tools" width="100%"/>

## What this repo is

| Layer | What you get |
| --- | --- |
| **Repo Standard** | Standing instructions every agent/human follows — [`docs/repo-playbook.html`](docs/repo-playbook.html) |
| **Playbook** | How we work: PR size, evidence, gates, hooks, docs authority |
| **macOS tools (pre-alpha)** | Local experiments beside the standard — not a product surface |

## Start here

1. [`docs/repo-playbook.html`](docs/repo-playbook.html) — the Repo Standard (SoT).
2. [`AGENTS.md`](AGENTS.md) — before any agent edit.
3. [`CONTRIBUTING.md`](CONTRIBUTING.md) — if you are a person.
4. AX / voice workstream only (owner-gated draft): [`docs/prd.md`](docs/prd.md).

## Layout

```
docs/repo-playbook.html   Repo Standard (SoT)
docs/                     architecture, rules, design, tasks, memory, prd (draft)
crates/jev-ax/            Accessibility port target — pre-alpha, not a product
crates/jev-voice/         voice hook path — pre-alpha, not a product
.metagpt/                 STATE.md, GATE.json, interview.md
reference/                pointer to the working Python system
.github/                  CI, PR template
docs/readme/              SpaceX/dark README assets (Coco company mark)
```

## Working on this

```bash
cargo test --workspace
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

**Every change is a small PR with CI green.** No direct pushes to `main`.

## Keys: bring your own

**This repo contains no keys, and nothing in it will ever need ours.** Every component
reads credentials from your machine, in the form you choose:

| What | Where it looks | Override |
| --- | --- | --- |
| `scripts/usage-check` | Providers **you list** in `~/.config/jev-use/usage-check.json` (template: `scripts/usage-check.example.json`). Ships with none. | `JEV_USAGE_CONFIG`, `JEV_KEYS_FILE`, `JEV_AUTH_FILE`, `JEV_USAGE_DB` |
| `scripts/voice-hook` | Nothing. It reads no keys and calls no API; it decides, then hands a goal to *your* driver. | `JEV_USE_BIN`, `JEV_VOICE_MODE`, `JEV_VOICE_TRIGGER` |
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
| `scripts/install-voice-hook.sh` | Builds and installs the voice hook, then prints what to put in CoCo Voice's settings. |
| `scripts/voice-hook` | The file CoCo Voice runs per transcript. Reads its config from the environment. |
| `scripts/repo-check` | Audits a repo against the standing standard: required files, credentials, machine paths, docs. |
| `scripts/usage-check` | Where tokens and money went, from your own providers and the local turn log. |

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

