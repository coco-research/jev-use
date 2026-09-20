# reference/

This directory is **not** the code. It is a pointer to the working system this repo is
porting from, plus the notes needed to trust it.

## What lives outside this repo

| Thing | Path | Why it is not vendored |
| --- | --- | --- |
| Python desktop driver | `~/code/jev-computeruse/` | It is the *reference implementation*. Copying it here would let the two drift silently. The parity harness calls it by path instead. |
| Jev decision layer | `~/code/jev-ultrafast/` | Upstream, kept pristine. Imported, never forked or modified. |
| The CLI shipped today | `~/.local/bin/jev-use` | Installed artifact, not source. |

## Why the reference matters

`jev_desktop` **already works and is measured**. That is what makes this port
verifiable rather than hopeful:

```
python jev_desktop --table --app Terminal --json   ->  20 addressable of 878 nodes
cargo run -p jev-ax -- --table --app Terminal      ->  must match, exactly
```

Same element count, same order, same roles, labels, values, and fingerprint. Any
divergence is a port bug, caught mechanically instead of by feel.

This is a luxury most rewrites do not get. Use it.

## Known-good measurements to port against

Recorded 2026-09-19 on this machine (M1, 16 GB, macOS 26.5.2):

| App | Addressable | Nodes seen | Observation |
| --- | --- | --- | --- |
| Terminal | 20 | 878 | ~470 ms |
| Finder | 11 | 916 | ~200 ms |
| PI-Desktop (Electron) | 229 | 2000 (capped) | ~300 ms |
| Google Chrome (toolbar only) | 17 | 1246 | ~380 ms |

A future PR that changes these numbers is changing behaviour, and must say so.
