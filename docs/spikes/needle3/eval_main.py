"""Run 2: fix the harness, then judge.

WHAT WAS WRONG WITH RUN 1, and each one could have produced a false verdict:

  1. STATE BLEED. `agent.complete()` was called 35 times on ONE agent. "what can you do"
     returned open_app(app="github.com") - the ARGUMENT from the previous utterance. So
     history was in play and prose miscalls may be contamination, not the model.
  2. THE TEST SET CONFLATED TWO DIFFERENT THINGS. "Dear team, please review the deck" is
     DICTATION: routing it at all is the asymmetry our voice design exists to prevent.
     "What is the capital of Mongolia" is a QUESTION, and with a search_web tool in the
     registry, a web search is a defensible call, not a false positive.
  3. NO THRESHOLD ANALYSIS. The engine's floor is 0.1 and the docs say the score is the
     caller's to route on. Judging "any call = failure" ignores the documented contract.

WHAT RUN 2 DOES: a fresh agent per utterance, three separately scored sets, and the
confidence threshold swept so the trade-off is visible instead of asserted.
"""
import json
import os
import re
import statistics as st
import time

os.environ.setdefault("NEEDLE_TELEMETRY", "0")
os.environ.setdefault("DO_NOT_TRACK", "1")

import needle  # noqa: E402
from registry import TOOLS  # noqa: E402

INTENT = [
    ("what is the status on payments-api", "project_status"),
    ("switch to Terminal", "switch_to_app"),
    ("open a new tab in Chrome", "open_new_tab"),
    ("any uncommitted work in web-dashboard", "uncommitted_work"),
    ("what is running right now", "list_running_apps"),
    ("what did I work on yesterday", "what_did_i_work_on"),
    ("show me the downloads folder", "open_path"),
    ("find the lease", "find_file"),
    ("start a 10 minute timer", "start_timer"),
    ("remind me to call Sam at 6pm", "create_event"),
    ("text Priya that I am running late", "send_message"),
    ("draft an email to the team about the freeze", "draft_email"),
    ("search for Postgres connection pooling", "search_web"),
    ("search my notes for the API rotation note", "search_notes"),
    ("turn the volume down to 20", "set_volume"),
    ("lock the screen", "lock_screen"),
    ("take a screenshot", "screenshot"),
    ("open github.com", "open_url"),
    ("what can you do", "list_capabilities"),
    ("turn on dark mode", "toggle_dark_mode"),
]

# DICTATION. Our hook types these. Handing one to an agent is the failure D13 is about.
DICTATION = [
    "Dear team, please review the deck before Friday.",
    "the meeting went well but we need more time",
    "I think we should ship on Monday",
    "the build failed because of a missing dependency",
    "I am not sure the tests cover this case",
    "we should probably hire another engineer",
    "let's circle back on this next week",
]

# QUESTIONS no purpose-built capability covers. A web search here is defensible.
OUT_OF_SCOPE = [
    "write me a poem about pizza",
    "how do I center a div in CSS",
    "can you explain how the accessibility tree works",
    "what is the capital of Mongolia",
    "summarise this paragraph for me",
    "translate this into French please",
    "how much disk space is left",
    "who won the game last night",
]

MULTI = ("what is the status on payments-api and open a new tab in Chrome",
         ["project_status", "open_new_tab"])


def fresh():
    return needle.Needle(tools=TOOLS)


def call(agent, utter):
    t0 = time.perf_counter()
    r = agent.complete(utter)
    ms = (time.perf_counter() - t0) * 1000
    return r, ms


def names(r):
    return [c["name"] for c in r["function_calls"]]


def main():
    print("=== STATE BLEED CHECK: same utterance, fresh agent vs reused agent ===")
    a = fresh()
    seq = ["open github.com", "what can you do"]
    reused = []
    for u in seq:
        r, _ = call(a, u)
        reused.append((u, names(r), r["function_calls"][0]["arguments"] if r["function_calls"] else None))
    for u in seq:
        r, _ = call(fresh(), u)
        print(f"  {u[:26]:26} reused={[n for _, n, _ in reused if _ == u]}  fresh={names(r)}"
              f"  fresh_args={r['function_calls'][0]['arguments'] if r['function_calls'] else None}")

    print("\n=== INTENT (fresh agent per call) ===")
    rows, lat, conf = [], [], []
    for utter, expected in INTENT:
        r, ms = call(fresh(), utter)
        got = names(r)
        rows.append((utter, expected, got[0] if got else None, r["confidence"], ms,
                     r["function_calls"][0]["arguments"] if r["function_calls"] else None))
        lat.append(ms); conf.append(r["confidence"])
    ok = sum(1 for _, e, g, *_ in rows if g == e)
    for utter, exp, got, c, ms, args in rows:
        print(f"  {'ok  ' if got == exp else 'MISS'} {ms:4.0f}ms conf={c:.2f}  {utter[:42]:42}"
              f" -> {got} {json.dumps(args) if args else ''}")
    print(f"  -> {ok}/{len(rows)} = {100*ok/len(rows):.1f}%")

    print("\n=== DICTATION (must produce nothing) ===")
    dict_rows = []
    for utter in DICTATION:
        r, ms = call(fresh(), utter)
        dict_rows.append((utter, names(r), r["confidence"], r.get("suppressed_calls", [])))
        print(f"  {'CALLED ' if names(r) else 'empty  '} conf={r['confidence']:.2f} "
              f"calls={names(r)}  {utter[:50]}")

    print("\n=== OUT OF SCOPE (a web search is defensible; counted separately) ===")
    oos_rows = []
    for utter in OUT_OF_SCOPE:
        r, ms = call(fresh(), utter)
        oos_rows.append((utter, names(r), r["confidence"]))
        print(f"  {'called ' if names(r) else 'empty  '} conf={r['confidence']:.2f} "
              f"calls={names(r)}  {utter[:50]}")

    print("\n=== CONFIDENCE SWEEP: what a threshold buys and costs ===")
    print(f"  {'thr':>5}  {'intent kept':>12}  {'dictation acted':>16}  {'oos acted':>10}")
    for thr in (0.0, 0.3, 0.5, 0.7, 0.9):
        kept = sum(1 for _, e, g, c, *_ in rows if g == e and c >= thr)
        d_act = sum(1 for _, n, c, _ in dict_rows if n and c >= thr)
        o_act = sum(1 for _, n, c in oos_rows if n and c >= thr)
        print(f"  {thr:>5.2f}  {kept:>6}/{len(rows):<5}  {d_act:>10}/{len(dict_rows):<5}  {o_act:>4}/{len(oos_rows)}")

    print("\n=== TWO CALLS IN ONE TURN (fresh agent) ===")
    r, ms = call(fresh(), MULTI[0])
    print(f"  expected {MULTI[1]}\n  got      {names(r)}  {'ok' if names(r) == MULTI[1] else 'MISS'}"
          f"  conf={r['confidence']:.2f}")

    print("\n=== MEASURED (fresh agent per call, engine cached) ===")
    print(f"  wall per call  p50 {st.median(lat):.0f}ms  min {min(lat):.0f}  max {max(lat):.0f}")
    print(f"  confidence set {sorted(set(round(c, 2) for c in conf))}")


if __name__ == "__main__":
    main()
