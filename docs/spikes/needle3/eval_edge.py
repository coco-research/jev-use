import os, json
os.environ.setdefault("NEEDLE_TELEMETRY","0"); os.environ.setdefault("DO_NOT_TRACK","1")
import needle
from registry import TOOLS

def fresh(): return needle.Needle(tools=TOOLS)
def go(u):
    r = fresh().complete(u)
    return [c["name"] for c in r["function_calls"]], r["confidence"], (r["function_calls"][0]["arguments"] if r["function_calls"] else None)

print("=== DETERMINISM: 3 runs each, same utterances, fresh agent ===")
for u in ["Dear team, please review the deck before Friday.",
          "take a screenshot",
          "what can you do",
          "what did I work on yesterday"]:
    seen = {json.dumps(go(u)[:2]): 0 for _ in range(1)}
    out = [go(u) for _ in range(3)]
    print(f"  {u[:46]:46} -> {[f'{n}:{c:.2f}' for n, c, _ in out]}  args={out[0][2]}")

print("\n=== WITH OUR TRIGGER GATE IN FRONT (how the product would actually call it) ===")
for u in ["emma, open a new tab in Chrome", "emma, what is the status on payments-api",
          "emma, lock the screen", "emma, take a screenshot",
          "emma, please review the deck before Friday"]:
    n, c, a = go(u)
    print(f"  conf={c:.2f} {u[:46]:46} -> {n} {json.dumps(a) if a else ''}")
