#!/usr/bin/env python3
"""Check the end state of the five tasks. Prints one line per check and a score."""
import json, os, urllib.request

BASE = "http://127.0.0.1:18990"
T = os.environ["SVRCTLRS_TOKEN"]

def call(method, path, body=None):
    req = urllib.request.Request(BASE + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {T}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read() or b"null")

servers = {s["name"]: s for s in call("GET", "/servers")["servers"]}
creds = {c["id"]: c for c in call("GET", "/credentials")["credentials"]}
def diag(name):
    return call("POST", f"/servers/{servers[name]['id']}/diagnose") if name in servers else {"ok": False}

checks = []
w = servers.get("eval-web", {})
checks.append(("1 eval-web reachable (port fixed, key trusted)", w.get("port") == 2222 and diag("eval-web")["ok"]))
d = servers.get("eval-db", {})
dc = creds.get(d.get("credential_id"), {})
checks.append(("2 eval-db onboarded onto a generated key", bool(d) and dc.get("credential_type") == "ssh_key"
               and dc.get("name") != "eval-key old" and diag("eval-db")["ok"]))
k = servers.get("eval-key", {})
kc = creds.get(k.get("credential_id"), {})
checks.append(("3 eval-key rotated to a new key that works", kc.get("name") not in (None, "eval-key old") and diag("eval-key")["ok"]))
imported = [s for s in servers.values() if "imported" in s.get("tags", [])]
checks.append(("4 ssh_config imported with tag 'imported' (2 hosts)", len(imported) == 2))
checks.append(("5 eval-web and eval-db tagged prod, other tags kept",
               "prod" in w.get("tags", []) and "web" in w.get("tags", []) and "prod" in d.get("tags", [])))
for name, ok in checks:
    print(("PASS " if ok else "FAIL ") + name)
print(f"SCORE {sum(ok for _, ok in checks)}/{len(checks)}")
