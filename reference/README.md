# reference/

This directory is **not** the code. It is a pointer to the working system this repo is
porting from, plus the notes needed to trust it.

## What lives outside this repo

| Thing | Path | Why it is not vendored |
| --- | --- | --- |
| Python desktop driver | `~/code/jev-computeruse/` | It is the *reference implementation*. The parity harness imports it by path and compares against a **pinned, vendored copy** (below), so the two cannot drift silently. |
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


## The spec is pinned, and vendored (`PINNED.json`, `vendor/ax.py`)

The reference lived in a directory with **no version control**, which makes it one `rm` away
from unreproducible. So its identity is recorded and a copy is committed:

```
sha256  ecef65ff2ed12c9598a5a12379cc3c60756e004b2df1c7652ceab6425c3918fa
bytes   21269   (ax.py, mtime 2026-09-19 21:24:35)
```

`reference/vendor/ax.py` is **byte-identical** to that file - verified with `cmp`, and every
mode of the harness re-verifies both hashes before it observes anything. If the reference
changes, the harness refuses to run and says *the specification moved* rather than reporting
the port as broken. `PINNED.json` also records the pyobjc version, because observation
behaviour depends on the binding and the binding is not in the script's hash.

## The numbers above are evidence with a date, not targets

Measured today, same machine, minutes apart, same app: Finder reported **12**, then **15**,
then **12** addressable elements. A window's selection and the frontmost window change what
is on screen, so a count is a fact about a moment. That is why the harness compares against a
**capture** that records its own state rather than against a number in a table.

## `scripts/parity` - the three modes

```
scripts/parity --capture <app>     freeze what the reference sees, with its state
scripts/parity --check <fixture>   re-observe and diff: did the screen or the spec change?
scripts/parity --live <app>        reference vs port, back to back (needs T7's CLI)
```

Exit codes: `0` same, `1` different, `2` could not run. A skipped check never reports as a
pass.

### Fixtures in `fixtures/`

| File | App | What it captures |
| --- | --- | --- |
| `finder.json` | Finder | 12 addressable, 878 counted nodes, fingerprint `8cd813583638dbd4` |
| `pi-desktop.json` | PI-Desktop | 10 addressable, 436 counted (Electron) |
| `terminal.json` | Terminal | **0 addressable**, fingerprint `e3b0c44298fc1c14` |

The Terminal capture is the empty case and it is not a mistake: Terminal was running with no
window open, so the reference observed nothing. Its fingerprint is exactly `sha256("")[:16]`,
which is the same value the Rust golden-vector test asserts for an empty table
(`crates/jev-ax/src/lib.rs`, `fingerprint_matches_the_reference_vectors`). Two
implementations, one hash, checked from both ends.
