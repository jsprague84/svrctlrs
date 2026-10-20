#!/usr/bin/env python3
"""Compact view of a Hermes stream-json transcript: commands, short results, final answer."""
import json, sys

def short(s, n):
    s = s.replace("\n", " ⏎ ")
    return s if len(s) <= n else s[:n] + " …"

for path in sys.argv[1:]:
    print(f"### {path}")
    text, turns = [], 0
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
            try:
                o = json.loads(out)
                out = (o.get("output") or "") + (f" [exit {o.get('exit_code')}]" if o.get("exit_code") not in (0, None) else "")
            except Exception:
                pass
            print(f"    → {short(out, 220)}")
        elif t == "text":
            text.append(e.get("text", ""))
        elif t in ("error", "result") and e.get("is_error"):
            print(f"  ! {short(json.dumps(e), 300)}")
    print(f"tool calls: {turns}")
    print("answer: " + short("".join(text).strip(), 1500))
    print()
