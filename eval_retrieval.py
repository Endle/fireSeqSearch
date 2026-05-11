#!/usr/bin/env python3
"""Tiny retrieval + /ask eval set.

Two checks, both run against a live server:

1. Retrieval — hits /query for each (query, expected_titles) pair and
   checks whether any expected page title appears in the top-K results.
2. /ask grounding — POSTs each question to /ask, reads the `meta` event,
   and checks that an expected source page was retrieved AND that the
   `done` event reports `answered: true` with no invalid citations.

Designed to be re-run after every change so regressions are visible
instead of vibes-based. Edit EVAL_SET / ASK_SET with queries you
remember the answers to; each value is a list of acceptable page titles
(any match counts as a pass).
"""
import json
import sys
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:3030"
TOP_K = 5
TIMEOUT = 30
ASK_TIMEOUT = 600


# Personal eval set. Add more entries as you find queries that should
# always work. Each value is a list — a hit on ANY of the listed page
# titles passes that query.
EVAL_SET = {
    "softmax": ["Softmax"],
    "coffee machine": [
        "Home In Canada/Electronics/CoffeeMachine",
        "Home In Canada___Electronics___CoffeeMachine",
    ],
    "cruise": ["Travel/Cruise", "Travel___Cruise"],
    # add more pairs here
}

# /ask eval: question -> acceptable source page titles. A pass requires
# (a) one of these titles in the streamed `meta` sources, (b) the `done`
# event reporting answered=true, and (c) no invalid citations.
ASK_SET = {
    "what is softmax?": ["Softmax"],
    "what coffee machine do I have at home?": [
        "Home In Canada/Electronics/CoffeeMachine",
        "Home In Canada___Electronics___CoffeeMachine",
    ],
    # add more pairs here
}


def fetch_query(term):
    encoded = urllib.parse.quote(term, safe="")
    with urllib.request.urlopen(
        f"{BASE}/query/{encoded}", timeout=TIMEOUT
    ) as resp:
        return json.loads(resp.read())


def _iter_sse(resp):
    event, data_lines = "message", []
    for raw in resp:
        line = raw.decode("utf-8", "replace").rstrip("\n").rstrip("\r")
        if line == "":
            if data_lines:
                payload = "\n".join(data_lines)
                try:
                    payload = json.loads(payload)
                except json.JSONDecodeError:
                    pass
                yield event, payload
            event, data_lines = "message", []
        elif line.startswith(":"):
            continue
        elif line.startswith("event:"):
            event = line[len("event:"):].strip()
        elif line.startswith("data:"):
            data_lines.append(line[len("data:"):].lstrip())


def fetch_ask(question):
    """Return (source_titles, done_dict). done_dict is {} if no done event."""
    body = json.dumps({"question": question}).encode("utf-8")
    req = urllib.request.Request(
        f"{BASE}/ask",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    titles, done = [], {}
    with urllib.request.urlopen(req, timeout=ASK_TIMEOUT) as resp:
        for event, data in _iter_sse(resp):
            if event == "meta" and isinstance(data, dict):
                titles = [s["title"] for s in data.get("sources", [])]
            elif event == "done" and isinstance(data, dict):
                done = data
            elif event == "error":
                raise RuntimeError(data.get("message") if isinstance(data, dict) else data)
    return titles, done


def run_retrieval_eval():
    passed = failed = 0
    print(f"retrieval: {len(EVAL_SET)} queries against {BASE} (top-{TOP_K})\n")
    for query, expected in EVAL_SET.items():
        try:
            hits = fetch_query(query)
        except Exception as e:
            print(f"  ERROR  {query!r}: {e}")
            failed += 1
            continue
        if not hits:
            print(f"  FAIL   {query!r}: 0 results returned")
            failed += 1
            continue
        top_titles = [h["title"] for h in hits[:TOP_K]]
        match_idx = next(
            (i for i, t in enumerate(top_titles) if t in expected), None
        )
        if match_idx is not None:
            h = hits[match_idx]
            print(
                f"  PASS   {query!r}: {h['title']!r} at rank {match_idx + 1} "
                f"(score {h['score']:.3f})"
            )
            passed += 1
        else:
            print(f"  FAIL   {query!r}: expected one of {expected}")
            print(f"          top-{TOP_K}: {top_titles}")
            failed += 1
    print(f"\nretrieval: {passed}/{passed + failed} passed\n")
    return passed, failed


def run_ask_eval():
    passed = failed = 0
    print(f"/ask: {len(ASK_SET)} questions against {BASE}\n")
    for question, expected in ASK_SET.items():
        try:
            titles, done = fetch_ask(question)
        except Exception as e:
            print(f"  ERROR  {question!r}: {e}")
            failed += 1
            continue
        match = next((t for t in titles if t in expected), None)
        invalid = done.get("invalid") or []
        answered = bool(done.get("answered"))
        if match and answered and not invalid:
            print(
                f"  PASS   {question!r}: source {match!r}, "
                f"cited {done.get('cited')}, {done.get('chars')} chars"
            )
            passed += 1
        else:
            reasons = []
            if not match:
                reasons.append(f"no expected source in {titles}")
            if not answered:
                reasons.append("answered=false")
            if invalid:
                reasons.append(f"invalid citations {invalid}")
            print(f"  FAIL   {question!r}: {'; '.join(reasons)}")
            failed += 1
    print(f"\n/ask: {passed}/{passed + failed} passed\n")
    return passed, failed


def main():
    rp, rf = run_retrieval_eval()
    ap, af = run_ask_eval()
    passed, failed = rp + ap, rf + af
    total = passed + failed
    print(f"total: {passed}/{total} passed", end="")
    if failed:
        print(f" ({failed} fail)")
        return 1
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
