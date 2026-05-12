#!/usr/bin/env python3
"""Automated protocol/invariant tests for the /ask endpoint.

Runs against a live fire_seq_search_server (default 127.0.0.1:3030) and
asserts the SSE wire contract and the server-side guarantees, rather than
the *content* of answers (which is model-dependent and lives in
eval_retrieval.py's ASK_SET). Exits non-zero if any check fails.

Checked:
  - event ordering: meta first, then delta*, then exactly one done; no error
  - meta carries a non-empty, well-formed source list
  - the streamed answer (concatenated deltas) is non-empty
  - done.cited ⊆ retrieved source indices; done.answered reflects cited
  - done.invalid is empty for a well-grounded question (a non-empty value
    means the model cited a source that wasn't retrieved — a real signal)
  - meta.confidence is "high"|"low" and done.confidence agrees with it
  - the `k` request parameter caps the number of sources
  - an empty/blank question yields a single `error` event and nothing else
  - only known event names appear

Usage: ./test_ask.py            # uses GROUNDED_QUESTION below
       ./test_ask.py "<q>"      # also runs the grounded checks on <q>
"""
import json
import sys
import urllib.request

BASE = "http://127.0.0.1:3030"
ASK_TIMEOUT = 600
KNOWN_EVENTS = {"meta", "delta", "done", "error"}

# A question your notebook can answer well (one strong source page). Override
# on the command line if "Softmax" isn't in your notes.
GROUNDED_QUESTION = "what is softmax?"


# --- tiny test harness ---------------------------------------------------
_failures = []


def check(name, cond, detail=""):
    status = "PASS" if cond else "FAIL"
    line = f"  {status}  {name}"
    if detail:
        line += f"  — {detail}"
    print(line)
    if not cond:
        _failures.append(name)
    return cond


# --- SSE client ----------------------------------------------------------
def ask(question, k=None):
    """POST /ask and return the list of (event, data) pairs in arrival order.
    `data` is JSON-decoded when possible."""
    payload = {"question": question}
    if k is not None:
        payload["k"] = k
    req = urllib.request.Request(
        BASE + "/ask",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json", "Accept": "text/event-stream"},
        method="POST",
    )
    events = []
    event, data_lines = "message", []
    with urllib.request.urlopen(req, timeout=ASK_TIMEOUT) as resp:
        for raw in resp:
            line = raw.decode("utf-8", "replace").rstrip("\n").rstrip("\r")
            if line == "":
                if data_lines:
                    body = "\n".join(data_lines)
                    try:
                        body = json.loads(body)
                    except json.JSONDecodeError:
                        pass
                    events.append((event, body))
                event, data_lines = "message", []
            elif line.startswith(":"):
                continue
            elif line.startswith("event:"):
                event = line[len("event:"):].strip()
            elif line.startswith("data:"):
                data_lines.append(line[len("data:"):].lstrip())
    return events


# --- checks --------------------------------------------------------------
def check_server_up():
    try:
        with urllib.request.urlopen(BASE + "/server_info", timeout=10) as resp:
            json.loads(resp.read())
        return True
    except Exception as e:
        print(f"[fatal] server not reachable at {BASE}: {e}", file=sys.stderr)
        return False


def run_grounded_checks(question):
    print(f"\n# grounded question: {question!r}")
    events = ask(question)
    names = [e for e, _ in events]

    check("only known event names", set(names) <= KNOWN_EVENTS, str(set(names)))
    check("no error event", "error" not in names)
    if not names or names[0] != "meta":
        check("first event is meta", False, str(names[:1]))
        return  # nothing else is meaningful
    check("first event is meta", True)

    meta = events[0][1]
    sources = meta.get("sources", []) if isinstance(meta, dict) else []
    check("meta has >=1 source", len(sources) >= 1, f"{len(sources)} sources")
    check(
        "source records well-formed",
        all(
            isinstance(s, dict)
            and isinstance(s.get("idx"), int)
            and isinstance(s.get("title"), str)
            and isinstance(s.get("logseq_uri"), str)
            for s in sources
        ),
    )
    check(
        "source idx are 1..N",
        [s["idx"] for s in sources] == list(range(1, len(sources) + 1)),
        str([s.get("idx") for s in sources]),
    )

    check("exactly one done event", names.count("done") == 1, f"{names.count('done')}")
    check("done is last event", names[-1] == "done", str(names[-1]))
    check("has >=1 delta event", names.count("delta") >= 1, f"{names.count('delta')}")

    answer = "".join(
        d.get("text", "") for e, d in events if e == "delta" and isinstance(d, dict)
    )
    check("streamed answer is non-empty", len(answer.strip()) > 0, f"{len(answer)} chars")

    done = next((d for e, d in events if e == "done"), {})
    valid_idx = set(range(1, len(sources) + 1))
    cited = set(done.get("cited", []))
    invalid = list(done.get("invalid", []))
    check("done.cited ⊆ retrieved indices", cited <= valid_idx, f"cited={sorted(cited)} valid={sorted(valid_idx)}")
    check(
        "done.answered matches cited",
        bool(done.get("answered")) == (len(cited) > 0),
        f"answered={done.get('answered')} cited={sorted(cited)}",
    )
    check("done.chars ≈ streamed length", done.get("chars") == len(answer), f"{done.get('chars')} vs {len(answer)}")
    meta_conf = meta.get("confidence")
    done_conf = done.get("confidence")
    check("meta.confidence is high|low", meta_conf in ("high", "low"), f"{meta_conf!r}")
    check("done.confidence matches meta", done_conf == meta_conf, f"meta={meta_conf!r} done={done_conf!r}")
    # Soft-ish: a well-grounded question should not provoke phantom citations.
    check("done.invalid is empty", invalid == [], f"invalid={invalid}")
    # For a question the corpus answers, we expect an actual answer with cites.
    check("question was answered with a citation", bool(done.get("answered")) and len(cited) >= 1,
          f"answered={done.get('answered')} cited={sorted(cited)}")


def run_k_param_check():
    print("\n# k parameter")
    events = ask(GROUNDED_QUESTION, k=2)
    meta = events[0][1] if events and events[0][0] == "meta" else {}
    sources = meta.get("sources", []) if isinstance(meta, dict) else []
    check("k=2 caps sources at 2", len(sources) <= 2, f"{len(sources)} sources")


def run_empty_question_check():
    print("\n# empty question")
    events = ask("   ")
    names = [e for e, _ in events]
    check("only an error event", names == ["error"], str(names))
    if names == ["error"]:
        msg = events[0][1]
        check(
            "error carries a message",
            isinstance(msg, dict) and isinstance(msg.get("message"), str) and msg["message"],
            str(msg),
        )


def main():
    if not check_server_up():
        return 2

    questions = [GROUNDED_QUESTION]
    if len(sys.argv) > 1:
        q = " ".join(sys.argv[1:])
        if q != GROUNDED_QUESTION:
            questions.append(q)

    for q in questions:
        run_grounded_checks(q)
    run_k_param_check()
    run_empty_question_check()

    print()
    if _failures:
        print(f"{len(_failures)} check(s) failed: {_failures}")
        return 1
    print("all checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
