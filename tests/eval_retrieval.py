#!/usr/bin/env python3
"""Tiny retrieval + /ask eval set.

Two checks, both run against a live server:

1. Retrieval — hits /query for each (query, expected_titles) pair and
   checks whether any expected page title appears in the top-K results,
   and (if the entry names a `max_rank`) whether it appears that high.
2. /ask grounding — POSTs each question to /ask, reads the `meta` event,
   and checks that an expected source page was retrieved AND that the
   `done` event reports `answered: true` with no invalid citations.

Designed to be re-run after every change so regressions are visible
instead of vibes-based. Edit EVAL_SET / ASK_SET with queries you
remember the answers to; each value is a list of acceptable page titles
(any match counts as a pass).

The built-in sets below are the author's Logseq corpus. `--set FILE`
loads a portable set instead — that's how the Obsidian smoke test grades
ranking against the AstroWiki vault (tests/astro_wiki_eval.json):

    {"top_k": 5,
     "retrieval": {"oort cloud": {"expect": ["Oort Cloud"], "max_rank": 1},
                   "loose query": ["Any Of", "These Titles"]},
     "ask":       {"what is X?": ["X"]}}

A retrieval entry may be a bare list (in top-K anywhere = pass) or an
object with `max_rank` — landing inside top-K but below `max_rank` is a
WARN, not a failure: it's a ranking slip worth seeing, not a broken index.
"""
import argparse
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


def parse_entry(value):
    """Normalise a retrieval entry to (expected_titles, max_rank)."""
    if isinstance(value, dict):
        return value["expect"], value.get("max_rank", TOP_K)
    return value, TOP_K


def run_retrieval_eval():
    passed = failed = warned = 0
    print(f"retrieval: {len(EVAL_SET)} queries against {BASE} (top-{TOP_K})\n")
    for query, value in EVAL_SET.items():
        expected, max_rank = parse_entry(value)
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
        if match_idx is None:
            print(f"  FAIL   {query!r}: expected one of {expected}")
            print(f"          top-{TOP_K}: {top_titles}")
            failed += 1
            continue
        h, rank = hits[match_idx], match_idx + 1
        where = f"{h['title']!r} at rank {rank} (score {h['score']:.3f})"
        if rank <= max_rank:
            print(f"  PASS   {query!r}: {where}")
            passed += 1
        else:
            # Retrieved, but something else outranked it — a ranking slip.
            # Report the pages that beat it; that's the actionable part.
            print(f"  WARN   {query!r}: {where}, expected rank <= {max_rank}")
            print(f"          outranked by: {top_titles[:rank - 1]}")
            warned += 1
            passed += 1
    print(f"\nretrieval: {passed}/{passed + failed} passed"
          f"{f' ({warned} rank warn)' if warned else ''}\n")
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


def load_set(path):
    """Replace the built-in Logseq sets with a portable one from JSON."""
    global EVAL_SET, ASK_SET, TOP_K
    with open(path) as f:
        spec = json.load(f)
    EVAL_SET = spec.get("retrieval", {})
    ASK_SET = spec.get("ask", {})
    TOP_K = spec.get("top_k", TOP_K)


def main():
    global BASE
    ap_ = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap_.add_argument("--set", metavar="FILE",
                     help="JSON eval set (default: the built-in Logseq one)")
    ap_.add_argument("--base", default=BASE, help=f"server base URL (default: {BASE})")
    ap_.add_argument("--retrieval-only", action="store_true",
                     help="skip /ask (fast; no chat backend needed)")
    args = ap_.parse_args()

    BASE = args.base.rstrip("/")
    if args.set:
        load_set(args.set)

    rp, rf = run_retrieval_eval()
    ap, af = (0, 0) if args.retrieval_only else run_ask_eval()
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
