#!/usr/bin/env python3
"""Compact view of a Hermes stream-json transcript: commands, short results, final answer, and
what the task cost: tool calls, bytes of tool output the model read, and tokens when Hermes
reports them."""
import json, sys

def short(s, n):
    s = s.replace("\n", " ⏎ ")
    return s if len(s) <= n else s[:n] + " …"

for path in sys.argv[1:]:
    print(f"### {path}")
    text, turns, read, usage = [], 0, 0, None
    for line in open(path):
        try:
            e = json.loads(line)
        except Exception:
            continue
        t = e.get("type")
        if t == "system":
            print(f"model: {e.get('model')}")
        elif t == "tool_use":
            turns += 1
            inp = e.get("input", {})
            print(f"  $ {short(inp.get('command') or json.dumps(inp), 300)}")
        elif t == "tool_result":
            out = e.get("output", "")
            read += len(out)
            try:
                o = json.loads(out)
                out = (o.get("output") or "") + (f" [exit {o.get('exit_code')}]" if o.get("exit_code") not in (0, None) else "")
            except Exception:
                pass
            print(f"    → {short(out, 220)}")
        elif t == "text":
            text.append(e.get("text", ""))
        if t == "result" and e.get("usage"):
            usage = e["usage"]
        if t in ("error", "result") and e.get("is_error"):
            print(f"  ! {short(json.dumps(e), 300)}")
    print(f"tool calls: {turns} · tool output read: {read} bytes"
          + (f" · tokens: {json.dumps(usage)}" if usage else ""))
    print("answer: " + short("".join(text).strip(), 1500))
    print()
