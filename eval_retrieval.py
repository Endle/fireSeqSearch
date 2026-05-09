#!/usr/bin/env python3
"""Tiny retrieval eval set.

Hits /query for each (query, expected_titles) pair and checks whether
any of the expected page titles appears in the top-K results. Designed
to be re-run after every change so retrieval regressions are visible
instead of vibes-based.

Edit EVAL_SET below with queries you remember the answers to. Each
entry maps a query string to a list of acceptable page titles (any
match counts as a pass).
"""
import json
import sys
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:3030"
TOP_K = 5
TIMEOUT = 30


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


def fetch_query(term):
    encoded = urllib.parse.quote(term, safe="")
    with urllib.request.urlopen(
        f"{BASE}/query/{encoded}", timeout=TIMEOUT
    ) as resp:
        return json.loads(resp.read())


def main():
    passed = 0
    failed = 0
    print(f"running {len(EVAL_SET)} queries against {BASE} (top-{TOP_K})\n")
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
    total = passed + failed
    print(f"\n{passed}/{total} passed", end="")
    if failed:
        print(f" ({failed} fail)")
        return 1
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())
