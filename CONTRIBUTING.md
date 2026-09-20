# Contributing

## The loop

```bash
git checkout main && git pull
git checkout -b <type>/<short-name>        # feat, fix, docs, chore, spike
# ... work ...
cargo fmt --all
cargo test --workspace
git commit -m "<type>(<scope>): <what changed>"
git push -u origin <branch>
gh pr create --fill
```

Wait for CI. Green, then review. Squash-merge.

## Branch naming

| Prefix | Use for |
| --- | --- |
| `feat/` | New capability |
| `fix/` | Broken behaviour |
| `spike/` | Time-boxed investigation, expected to be thrown away or rewritten |
| `docs/` | Documents only, no code |
| `chore/` | Tooling, CI, dependencies |

The `spike/` prefix is deliberate and honest. The first task in this repo is a spike: does
`objc2-application-services` expose the AX calls we need? That answer might invalidate the
approach, so it should not pretend to be a feature.

## Commit messages

`<type>(<scope>): <imperative summary>`

```
feat(jev-ax): walk the AX tree into an indexed element table
fix(jev-ax): reject elements with no rect as not addressable
spike(jev-ax): confirm objc2 exposes AXUIElementSetMessagingTimeout
docs(rules): record why the messaging timeout is mandatory
```

The scope is usually the crate. The summary completes the sentence *"This commit will..."*.

## PR size

A PR should be reviewable in about two minutes. If it is not, split it.

Good PRs from this repo's future:

- `feat(jev-ax): add Rect and the on-screen test` - one type, one predicate, tests
- `feat(jev-ax): read AXRole and AXTitle from one element` - one function
- `spike(jev-ax): probe objc2 for the four AX entry points` - one script, one finding

Bad PR: "implement the AX walker." That is five reviewable pieces wearing one coat.

## Definition of done

- [ ] CI green (lint, test, docs)
- [ ] Tests cover the new behaviour, not just the happy path
- [ ] `docs/tasks.md` reflects reality
- [ ] `docs/memory.md` has the decision, if a decision was made
- [ ] No secret, key, or personal absolute path added
- [ ] `docs/rules.md` updated in the same PR if an invariant changed

## Housekeeping, and why it matters here

This repo will be read by agents more often than by people. That changes the rules:

1. **Documents are load-bearing.** A stale doc is worse than a missing one, because an
   agent will act on it. A PR that makes a doc wrong must fix it.
2. **Small PRs, because context is finite.** A 400-line diff cannot be reviewed by an
   agent with a 200 K window and 12 other things loaded.
3. **Evidence over prose.** "Verified" is a claim. A pasted command and its output is a
   fact. Every PR template section exists to force the second.
4. **Dead ends are valuable.** `docs/memory.md` records what did not work. This is the
   cheapest thing in the repo and the most often skipped.
5. **One owner per fact.** Two documents must never both be authoritative for the same
   thing. See the table in `AGENTS.md`.

## Weekly and monthly

**Weekly** (5 minutes, `docs/tasks.md`):

- Move finished items to Done with the PR number
- Re-rank what is next
- Delete anything that has been "next" for three weeks and is not happening

**Monthly** (20 minutes):

- Re-read `docs/rules.md` against the code. Is each invariant still enforced by a test?
- Prune `docs/memory.md` of anything now obvious
- Check `docs/architecture.md` still describes the real tree (`find . -name '*.rs'`)
- Look at the CI time trend. If it passed five minutes, fix it.
