#!/usr/bin/env python3
"""Smoke test for the fire_seq_search_server HTTP API.

Hits /server_info once, then either runs a single query passed on the
command line (`tests/test_endpoints.py cruise`) or drops into an interactive
loop. Renders each hit with its title, score, summary, and top_snippet.
Summaries are generated asynchronously by the server's background
summarizer; pages that don't have one yet show as "(pending)".

`/ask` (streamed RAG): `tests/test_endpoints.py --ask "what is softmax?"` POSTs
the question, consumes the SSE stream, and prints sources, the streamed
answer, and the server-side citation-validation summary.
"""
import json
import sys
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:3030"
TIMEOUT = 30
ASK_TIMEOUT = 600


def http_get(path):
    req = urllib.request.Request(BASE + path)
    with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
        return json.loads(resp.read())


def show_server_info():
    print(f"GET {BASE}/server_info")
    info = http_get("/server_info")
    print(json.dumps(info, indent=2, ensure_ascii=False))
    idx = info.get("indexer")
    if idx and idx.get("in_flight"):
        print(
            f"\n[!] indexer is still running "
            f"({idx['indexed_notes']}/{idx['total_notes']} notes, "
            f"{idx['indexed_chunks']} chunks) — results may be partial.\n"
        )
    print()


def run_query(term):
    encoded = urllib.parse.quote(term, safe="")
    print(f"\nGET {BASE}/query/{encoded}")
    hits = http_get(f"/query/{encoded}")
    pending = sum(1 for h in hits if h.get("summary_status") != "ok")
    note = f" ({pending} summary pending)" if pending else ""
    print(f"  {len(hits)} hit(s){note}\n")
    return hits


def render_hit(i, hit):
    print(f"  [{i}] {hit['title']}   score={hit['score']:.3f}   chunk_id={hit['chunk_id']}")
    print(f"      {hit['logseq_uri']}")
    status = hit.get("summary_status", "ok")
    summary = hit.get("summary")
    if summary:
        print(f"      summary   : {summary}")
    elif status == "failed":
        print("      summary   : (failed to summarize)")
    else:
        print("      summary   : (pending — re-run query in a moment)")
    print(f"      top_snippet : {hit['top_snippet']}")
    print()


def query_and_render(term):
    try:
        hits = run_query(term)
    except Exception as e:
        print(f"[error] /query failed: {e}", file=sys.stderr)
        return
    for i, hit in enumerate(hits, 1):
        render_hit(i, hit)


def iter_sse(resp):
    """Yield (event, data) pairs from an SSE response body. `data` is the
    concatenation of all `data:` lines in the event, JSON-decoded if possible."""
    event = "message"
    data_lines = []
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
            continue
        if line.startswith(":"):
            continue  # comment / keep-alive
        if line.startswith("event:"):
            event = line[len("event:"):].strip()
        elif line.startswith("data:"):
            data_lines.append(line[len("data:"):].lstrip())


def run_ask(question):
    body = json.dumps({"question": question}).encode("utf-8")
    req = urllib.request.Request(
        BASE + "/ask",
        data=body,
        headers={"Content-Type": "application/json", "Accept": "text/event-stream"},
        method="POST",
    )
    print(f"\nPOST {BASE}/ask  {question!r}\n")
    answered = None
    with urllib.request.urlopen(req, timeout=ASK_TIMEOUT) as resp:
        for event, data in iter_sse(resp):
            if event == "meta":
                srcs = data.get("sources", []) if isinstance(data, dict) else []
                print(f"  sources ({len(srcs)}):")
                for s in srcs:
                    print(
                        f"    [{s['idx']}] {s['title']}   score={s['score']:.3f}"
                        f"   summary={s.get('summary_status')}"
                    )
                    print(f"        {s['logseq_uri']}")
                print("\n  answer:\n", end="")
                if not srcs:
                    pass
            elif event == "delta":
                sys.stdout.write(data.get("text", "") if isinstance(data, dict) else str(data))
                sys.stdout.flush()
            elif event == "done":
                answered = data.get("answered") if isinstance(data, dict) else None
                print(f"\n\n  done: {json.dumps(data, ensure_ascii=False)}")
            elif event == "error":
                msg = data.get("message") if isinstance(data, dict) else data
                print(f"\n  [error] {msg}", file=sys.stderr)
    return answered


def main():
    try:
        show_server_info()
    except Exception as e:
        print(f"[fatal] /server_info failed: {e}", file=sys.stderr)
        return 1

    if len(sys.argv) > 1 and sys.argv[1] == "--ask":
        if len(sys.argv) < 3:
            print("usage: tests/test_endpoints.py --ask <question>", file=sys.stderr)
            return 2
        try:
            run_ask(" ".join(sys.argv[2:]))
        except Exception as e:
            print(f"[error] /ask failed: {e}", file=sys.stderr)
            return 1
        return 0

    if len(sys.argv) > 1:
        # Treat all args as a single query phrase so unquoted multi-word
        # terms like `tests/test_endpoints.py air ticket` work as expected.
        query_and_render(" ".join(sys.argv[1:]))
        return 0

    while True:
        try:
            term = input("keyword (empty to quit) > ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            return 0
        if not term:
            return 0
        query_and_render(term)


if __name__ == "__main__":
    sys.exit(main())
