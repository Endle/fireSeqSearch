#!/usr/bin/env python3
"""Interactive smoke test for the fire_seq_search_server HTTP API.

Hits /server_info once, then loops: read a keyword, call /query, render
each hit with its title, score, summary, and top_chunk. Summaries are
generated asynchronously by the server's background summarizer; pages
that don't have one yet show as "(pending)".
"""
import json
import sys
import urllib.parse
import urllib.request

BASE = "http://127.0.0.1:3030"
TIMEOUT = 30


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
    print(f"      top_chunk : {hit['top_chunk']}")
    print()


def main():
    try:
        show_server_info()
    except Exception as e:
        print(f"[fatal] /server_info failed: {e}", file=sys.stderr)
        return 1

    while True:
        try:
            term = input("keyword (empty to quit) > ").strip()
        except (EOFError, KeyboardInterrupt):
            print()
            return 0
        if not term:
            return 0

        try:
            hits = run_query(term)
        except Exception as e:
            print(f"[error] /query failed: {e}", file=sys.stderr)
            continue

        for i, hit in enumerate(hits, 1):
            render_hit(i, hit)


if __name__ == "__main__":
    sys.exit(main())
