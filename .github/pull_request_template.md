<!--
  Keep this short. A PR nobody can review in two minutes is not a small PR.

  The one rule: EVIDENCE, NOT ASSERTIONS. "Tests pass" is not evidence. The command
  you ran and its output is.
-->

## What this changes

<!-- One or two sentences. What behaviour is different after this merges? -->

## Why

<!-- The problem. Link the task in docs/tasks.md if there is one. -->

## How it was verified

<!--
  Paste the real command and its real output. Not a summary of it.

  For a pure change:      cargo test --workspace        (and the pass count)
  For an AX behaviour:    the parity harness diff against the Python reference
  For a UI change:        a screenshot or a measured number
-->

```
$ cargo test --workspace
```

## Checklist

- [ ] CI is green (all three jobs: lint, test, docs)
- [ ] `docs/tasks.md` updated if this closes or opens a task
- [ ] `docs/memory.md` updated if this settled a decision or hit a dead end worth recording
- [ ] No secret, key, or personal path added to any file, including docs
- [ ] If this changes a **documented invariant** in `docs/rules.md`, that file is updated in the same PR

## Risk

<!--
  What could this break, and what is the rollback? If the answer is "nothing",
  say why you are confident.
-->
