Next action: run T1, the objc2 spike. One hour, then report the answer.

**Current stage:** `index` done, `interview` done, `prd` drafted, `arch` drafted, `plan`
drafted. All three are written but **awaiting owner review** - so the gate is `prd`.
`build` is not started and is blocked behind the spike.

**Last action:** created the repo, scaffolded the Rust workspace, wrote the six baseline
documents, went public under Apache-2.0 after scrubbing machine-specific content, and
confirmed CI green on a real GitHub runner (run 35485404565: lint, test, docs all pass).

---

## What exists right now

```
jev-use/
  crates/jev-ax/       Rust stub: Element, Kind, Rect, Limits, ElementTable, fingerprint
                       9 tests, clippy clean, fmt clean
  docs/                prd, architecture, rules, design, tasks, memory
  .metagpt/            this file, GATE.json, interview.md
  reference/           points at the working Python implementation
  .github/             CI (3 jobs), PR template
```

## Why the crate is not empty

`crates/jev-ax/src/lib.rs` holds the real types the port needs, not placeholders:
`Element`, `Kind`, `Rect`, `Limits`, `ElementTable`, and the fingerprint used by the
repeat guard. The `is_addressable` and `is_on_screen` predicates are implemented and
tested, because they encode rule R1 - the single most important predicate in the port.

Starting with them means CI is green from commit one and every later PR has something to
test against.

## What was verified before committing

| Check | Result |
| --- | --- |
| `cargo test --workspace` | **9 passed, 0 failed** |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |

## The one thing that could invalidate the plan

**T1.** If `objc2-application-services` does not expose `AXUIElementCopyAttributeValue`,
`AXUIElementCopyActionNames`, `AXUIElementSetMessagingTimeout`, and `AXValueGetValue`, the
dependency choice changes - vendor `accessibility-sys` or write the FFI directly. The
architecture survives either way; the first PR does not.

Everything else in `docs/architecture.md` rests on that answer, which is why the spike is
task one and time-boxed to an hour.

## Next three steps

1. **T1** - the spike. Report yes / partial / no.
2. Owner reviews `docs/prd.md` and `docs/rules.md`. Rules are the ones that matter; they
   are the invariants with measurements behind them.
3. **T2 + T3** in parallel - app resolution, and typed attribute reads. Both are small
   and neither depends on the other.
