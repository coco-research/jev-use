Next action: start T4, the tree walk (`crates/jev-ax/src/walk.rs`) - and write T5, the parity
harness, as its acceptance test. Read `docs/handoff.md` first; it is the entry point now.

**Current stage:** `build` in progress, unblocked. `prd`, `arch`, `plan` approved.
**Last action:** T14 landed - the macOS accessibility path now runs in CI on the self-hosted
runner `coco-mac-jevu`, guarded so a fork PR can never reach this machine. 18 PRs merged,
58 tests, `main` green, nothing open, nothing half-written.

**Owner split (do not cross):** this repo and its code are yours. Model selection and
routing (`~/.zcode/litellm_config.yaml`, the LiteLLM launchd job, `~/.pi/agent/pinned-models.json`,
`~/.secrets/ai-keys.env`, KeyDeck) belong to a different agent. Read and probe; do not edit.

---

## What exists right now

```
jev-use/
  crates/jev-ax/        the port target. macOS Accessibility only.
    src/lib.rs          Element, Kind, Rect, Limits, ElementTable, fingerprint
    src/attrs.rs        typed AX attribute reads over one `raw` that keeps the error code
    src/app.rs          app resolution + activation (exact-or-unique, never fuzzy)
    src/walk.rs         NOT WRITTEN - this is T4
    src/execute.rs      NOT WRITTEN - this is T6
    examples/           list_apps (live check for T2), check_access (may this process look?)
  crates/jev-config/    one settings file, `jev-config` menu, `doctor`, precedence rules
  crates/jev-voice/     the voice hook: decide() is pure, main.rs acts
  scripts/              voice-hook (the wrapper the voice app runs), install-voice-hook.sh,
                        repo-check, usage-check, usage-check.example.json
  docs/                 prd, architecture, rules, tasks, memory, design, handoff, guides
  .metagpt/             this file, GATE.json, interview.md
  reference/            points at the working Python implementation this is ported from
  .github/              CI: lint, test, docs on ubuntu; `macos` on the self-hosted runner
```

58 tests (32 ax, 12 config, 14 voice). CI green.

## What was verified before committing (and how)

- **T2 live:** 11 apps found, pid/name set byte-identical to the Python reference. `"e"`
  matches 10 apps and errors; `"co"` matches 2 and errors; `"a"` resolves to Terminal.
- **T3 live:** the system-wide element reports `AXSystemWide`; `AXPosition` on it returns
  exactly `-25205`; Finder's menu bar reads in one attribute and its items report `AXPress`.
- **T15 live on this machine:** the hook pasted a transcript byte-identical into a scratch
  TextEdit document in 0.24-0.33 s, through the wrapper, under `env -i` with a GUI-like PATH;
  a backgrounded run returned to the app in 6-15 ms and logged its exit 2 s later.
- **T14:** the runner picked up its first job 4 seconds after the workflow was pushed.
- **T17:** the menu was driven through a real pty; `doctor` reports "everything in place".

## The one thing that could still invalidate the plan

The port's value rests on **parity** (R8), and parity is not yet measured for the walk -
the piece being built now. T5 is the answer: run both implementations on the same app and
diff the tables. If they cannot be made to agree, the port is a rewrite and the plan
changes. Write T5 before or with T4, not after.

## Next three steps

1. Read `docs/handoff.md` and `docs/rules.md`, then `cargo test --workspace` and
   `jev-config doctor` to confirm the machine matches what the docs claim.
2. Grant the runner Accessibility (System Settings -> Privacy & Security ->
   `~/actions-runner-jevu/bin/Runner.Listener`), re-run the `macos` job, and watch the live
   AX tests execute for the first time.
3. T4 + T5 together: the tree walk, and the harness that proves it matches the reference.
