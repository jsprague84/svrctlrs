#!/usr/bin/env python3
"""Seed the eval svrctlrs: credentials, two servers (one broken), a token. Prints the token."""
import json, os, sys, time, urllib.request, urllib.error

BASE = "http://127.0.0.1:18990"

def call(method, path, body=None, token=None):
    req = urllib.request.Request(BASE + "/api/v1" + path, method=method,
        data=json.dumps(body).encode() if body is not None else None,
        headers={"Content-Type": "application/json", **({"Authorization": f"Bearer {token}"} if token else {})})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, json.loads(r.read() or b"null")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"null")

for _ in range(60):
    try:
        if call("GET", "/health")[0] == 200: break
    except Exception: pass
    time.sleep(1)
_, login = call("POST", "/auth/login", {"username": "admin", "password": os.environ["EVAL_ADMIN_PASSWORD"]})
s = login["session_id"]
_, pw = call("POST", "/credentials", {"name": "eval password", "credential_type": "password",
             "value": os.environ["EVAL_SSH_PASSWORD"], "username": "eval"}, s)
_, old = call("POST", "/credentials", {"name": "eval-key old", "credential_type": "ssh_key",
              "value": open(os.environ["EVAL_OLD_KEY_FILE"]).read(), "username": "eval"}, s)
# eval-web: the wrong port, host key never confirmed (task 1)
_, web = call("POST", "/servers", {"name": "eval-web", "hostname": "eval-web", "port": 2223, "username": "eval",
              "credential_id": pw["id"], "tags": ["web"]}, s)
# eval-key: works today with the old key (task 3); its host key is confirmed here
_, key = call("POST", "/servers", {"name": "eval-key", "hostname": "eval-key", "port": 2222, "username": "eval",
              "credential_id": old["id"], "tags": ["infra"]}, s)
for _ in range(30):
    st, t = call("POST", f"/servers/{key['id']}/test", None, s)
    if t.get("code") == "HOST_KEY_UNCONFIRMED":
        call("POST", f"/servers/{key['id']}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]}, s)
    elif t.get("success"):
        break
    time.sleep(2)
else:
    sys.exit(f"eval-key never became reachable: {t}")
_, tok = call("POST", "/tokens", {"name": "eval-agent", "preset": "servers_keys", "expires_in_days": 1,
              "require_approval": False}, s)
print(tok["secret"])
