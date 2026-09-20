# spike: Needle 3 on the action path

**Date:** 2026-09-20. **Question:** should a small local intent model (Cactus Needle 3,
8-29 MB, Apache-2.0) replace or precede the frontier model as the router for voice commands?

**Answer:** not on the action path, on this registry, today. The finding is recorded as
**X6** in `docs/memory.md`.

This directory exists so the measurement can be repeated rather than believed. It is a
record, not a maintained tool: nothing in the repo imports it and nothing runs it.

## What is here

| File | What it is |
| --- | --- |
| `registry.py` | Our 22 capabilities, expressed as Needle tools the way its tool guide asks for: one tool per action, names users say, formats in descriptions, and three deliberately confusable pairs (`switch_to_app` / `open_app`, `take_note` / `search_notes`, `search_web` / `search_notes`) |
| `eval_main.py` | The measurement: a **fresh agent per utterance**, three separately scored sets (intent, dictation, out-of-scope), a confidence threshold sweep, and a 40-line keyword matcher as the baseline |
| `eval_edge.py` | Determinism (same input three times) and the trigger-prefix case, which is how our voice hook would actually call it |

## How to run it

```sh
python3 -m venv /tmp/needle-venv
/tmp/needle-venv/bin/pip install cactus-needle      # 3.0.3 at the time; pin it
cd docs/spikes/needle3
/tmp/needle-venv/bin/python eval_main.py
/tmp/needle-venv/bin/python eval_edge.py
```

Telemetry is on by default in their binary. Both scripts set `NEEDLE_TELEMETRY=0` and
`DO_NOT_TRACK=1` before importing, so the run is offline in that sense.

## What it found, in one line each

- Intent accuracy **17/20 (85%)** against a keyword matcher's **16/20 (80%)**, on our registry.
- Every dictation sentence produced a call, at confidence **0.97 to 1.00**.
- Raising the act threshold to **0.90** removes **none** of those and costs 3 intent points.
- **78 ms** p50, 110 MB RAM, 673 tok/s decode, deterministic across repeats.
- The trigger word leaks into arguments: `"emma, take a screenshot"` -> `take_note(text="emma")`.

## What is deliberately not here

Run 1 of this spike, which called `complete()` 35 times on **one** agent. State carried
between turns (an argument from the previous utterance reappeared), so its numbers are not
trustworthy. The corrected method is `eval_main.py`. The mistake is recorded because it is
the kind that produces a confident wrong verdict.

## When to revisit

When a fine-tune on our own registry, with dictation sentences as explicit negatives, clears
zero false positives on a 100-utterance dictation set. Their own published lift is 18 to 36
points on the target distribution, so the base-model numbers here are a floor, not a ceiling.
Two cautions from their documentation: a tuned archive reports `confidence` as `None` (the
calibration head is not updated by a fine-tune), and non-English calls measured at 0.0
confidence.
