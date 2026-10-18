#!/usr/bin/env python3
"""End-to-end check of persistent PTY sessions against a running server.

Needs a server started with a short TTL and an SSH target the given key can log in to:

    DATABASE_URL=sqlite:/tmp/e2e.db ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
    ENCRYPTION_KEY=$(openssl rand -hex 32) SESSION_SECURE=false SESSION_DETACH_TTL_SECS=3 \
    target/debug/server --addr 127.0.0.1:18080 &
    E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 \
    python3 scripts/dev/pty-session-e2e.py        # needs the `websockets` package

Exercises: shell, dropped socket, attach with last_seq (gap replay), live input after
reattach, full redraw from seq 0, `closed` on exit, `session_gone`, TTL expiry. Output frames
are requested in binary form (E2E_BINARY=0 for the legacy JSON `output` frames) and a
multibyte marker checks that bytes reach the client intact.
"""
import asyncio, json, os, time, urllib.request, urllib.error, http.cookiejar
import websockets

BASE = os.environ.get("E2E_URL", "http://127.0.0.1:18080")
WS = BASE.replace("http", "ws", 1) + "/ws/terminal/pty"
USER = os.environ.get("E2E_USER", "admin")
PASSWORD = os.environ.get("E2E_PASSWORD", "e2e-password-123456")
SSH_HOST = os.environ["E2E_SSH_HOST"]
SSH_USER = os.environ.get("E2E_SSH_USER", "root")
SSH_KEY = os.path.expanduser(os.environ.get("E2E_SSH_KEY", "~/.ssh/id_ed25519"))
TTL = float(os.environ.get("E2E_TTL_SECS", "3"))
BINARY = os.environ.get("E2E_BINARY", "1") == "1"
# Names are unique so the script can run repeatedly against one database
RUN = os.environ.get("E2E_RUN", str(int(time.time())))
# Box-drawing and an emoji: 3- and 4-byte UTF-8, the kind a per-chunk decode would mangle
UNICODE_MARKER = "╔═╗ ✓ 🦀 ü"

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))

def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method,
        headers={"Content-Type": "application/json", "Origin": BASE})
    with opener.open(req) as r:
        return json.loads(r.read() or b"null")

api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
cookie = "; ".join(f"{c.name}={c.value}" for c in cj)
key = open(SSH_KEY).read()
cred = api("POST", "/api/v1/credentials", {"name": f"e2e key {RUN}", "credential_type": "ssh_key", "value": key, "username": SSH_USER})
cid = cred["id"] if "id" in cred else cred["data"]["id"]
srv = api("POST", "/api/v1/servers", {"name": f"e2e target {RUN}", "hostname": SSH_HOST, "port": 22, "username": SSH_USER, "credential_id": cid})
sid = srv["id"] if "id" in srv else srv["data"]["id"]
print("server", sid, "credential", cid)

# First contact: the connection test records the host key and refuses until it is confirmed
test = api("POST", f"/api/v1/servers/{sid}/test")
assert test.get("success") is False and test.get("code") == "HOST_KEY_UNCONFIRMED", test
challenge = test["details"]
assert challenge["fingerprint"].startswith("SHA256:") and challenge["key_type"], challenge
keys = api("GET", f"/api/v1/servers/{sid}/host-keys")["host_keys"]
assert len(keys) == 1 and keys[0]["trusted"] is False and keys[0]["fingerprint"] == challenge["fingerprint"], keys
# SFTP is refused the same way, with the fingerprint in the error details
try:
    api("GET", f"/api/v1/servers/{sid}/files")
    raise AssertionError("files listed before the host key was confirmed")
except urllib.error.HTTPError as e:
    body = json.loads(e.read())
    assert e.code == 428 and body["error"]["code"] == "HOST_KEY_UNCONFIRMED", (e.code, body)
    assert body["error"]["details"]["fingerprint"] == challenge["fingerprint"], body
# A wrong fingerprint is not trusted
try:
    api("POST", f"/api/v1/servers/{sid}/host-keys/trust", {"fingerprint": "SHA256:not-the-one"})
    raise AssertionError("wrong fingerprint accepted")
except urllib.error.HTTPError as e:
    assert e.code == 404, e.code
trusted = api("POST", f"/api/v1/servers/{sid}/host-keys/trust", {"fingerprint": challenge["fingerprint"]})["host_key"]
assert trusted["trusted"] is True and trusted["trusted_at"], trusted
print("host key confirmed:", challenge["key_type"], challenge["fingerprint"])

# With the key trusted, the test goes through the same SSH client as the shell (key auth)
test = api("POST", f"/api/v1/servers/{sid}/test")
assert test.get("success") is True, test
print("connection test ok:", test.get("message") or test)

def trust_first_contact(server_id):
    t = api("POST", f"/api/v1/servers/{server_id}/test")
    assert t.get("code") == "HOST_KEY_UNCONFIRMED", t
    assert t["details"]["fingerprint"] == challenge["fingerprint"], t["details"]
    api("POST", f"/api/v1/servers/{server_id}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]})

# A wrong password must reach the server and come back as a password failure, not a crash
bad = api("POST", "/api/v1/credentials", {"name": f"e2e bad password {RUN}", "credential_type": "password", "value": "definitely-wrong", "username": SSH_USER})
bad_id = bad["id"] if "id" in bad else bad["data"]["id"]
bad_srv = api("POST", "/api/v1/servers", {"name": f"e2e bad target {RUN}", "hostname": SSH_HOST, "port": 22, "username": SSH_USER, "credential_id": bad_id})
bad_sid = bad_srv["id"] if "id" in bad_srv else bad_srv["data"]["id"]
trust_first_contact(bad_sid)
bad_test = api("POST", f"/api/v1/servers/{bad_sid}/test")
assert bad_test.get("success") is False and "assword" in json.dumps(bad_test), bad_test
print("wrong password refused ok")

# Jump host: reach the same box through the trusted server's SSH connection (direct-tcpip).
# Its host key is seen afresh for the new server row, so first contact asks again.
hop = api("POST", "/api/v1/servers", {"name": f"e2e via {RUN}", "hostname": "127.0.0.1", "port": 22, "username": SSH_USER, "credential_id": cid, "via_server_id": sid})
hop_sid = hop["id"] if "id" in hop else hop["data"]["id"]
assert api("GET", f"/api/v1/servers/{hop_sid}")["server"]["via_server_id"] == sid
t = api("POST", f"/api/v1/servers/{hop_sid}/test")
assert t.get("code") == "HOST_KEY_UNCONFIRMED" and t["details"]["server_id"] == hop_sid, t
api("POST", f"/api/v1/servers/{hop_sid}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]})
t = api("POST", f"/api/v1/servers/{hop_sid}/test")
assert t.get("success") is True, t
print("jump host ok:", t["message"], t["latency_ms"], "ms")
listing = api("GET", f"/api/v1/servers/{hop_sid}/files")
assert listing["path"].startswith("/") and isinstance(listing["entries"], list), listing
print("sftp through jump host ok")
# loops and self-references are refused
for bad_via in (hop_sid,):
    try:
        api("PUT", f"/api/v1/servers/{sid}", {"via_server_id": bad_via})
        raise AssertionError("jump loop accepted")
    except urllib.error.HTTPError as e:
        assert e.code == 400, e.code
try:
    api("PUT", f"/api/v1/servers/{hop_sid}", {"via_server_id": hop_sid})
    raise AssertionError("self jump accepted")
except urllib.error.HTTPError as e:
    assert e.code == 400, e.code
print("jump loop refused ok")

# Tags: normalised on write, searchable in the UI; replaced as a whole on update
api("PUT", f"/api/v1/servers/{sid}", {"tags": [" Prod", "prod", "e2e "]})
assert api("GET", f"/api/v1/servers/{sid}")["server"]["tags"] == ["e2e", "Prod"], api("GET", f"/api/v1/servers/{sid}")["server"]["tags"]
print("tags ok")

# ssh_config import: Host blocks become servers; ProxyJump resolves to a known server;
# patterns, Match blocks and existing names are skipped
cfg = f"""
Host *
  ServerAliveInterval 30
Host e2e-imported-{RUN}
  HostName 127.0.0.1
  User {SSH_USER}
  ProxyJump e2e target {RUN}
Host e2e-imported-2-{RUN} e2e-imported-3-{RUN}
  HostName=%h.lab
  Port 2222
Host e2e-imported-{RUN}
  HostName dup.example
Match host x
  User y
"""
imp = api("POST", "/api/v1/servers/import", {"config": cfg, "credential_id": cid, "tags": ["imported"]})
names = [c["name"] for c in imp["created"]]
assert names == [f"e2e-imported-{RUN}", f"e2e-imported-2-{RUN}", f"e2e-imported-3-{RUN}"], imp
skipped = {s_["name"]: s_["reason"] for s_ in imp["skipped"]}
assert "*" in skipped and "Match host x" in skipped and skipped.get(f"e2e-imported-{RUN}") == "a server with that name exists", skipped
imported = api("GET", f"/api/v1/servers/{imp['created'][0]['id']}")["server"]
assert imported["via_server_id"] == sid and imported["tags"] == ["imported"] and imported["username"] == SSH_USER, imported
third = api("GET", f"/api/v1/servers/{imp['created'][2]['id']}")["server"]
assert third["hostname"] == f"e2e-imported-3-{RUN}.lab" and third["port"] == 2222, third
assert any("connects through" in n for n in imp["notes"]), imp["notes"]
# the imported server behind the jump host works once its key is confirmed
imp_sid = imp["created"][0]["id"]
t = api("POST", f"/api/v1/servers/{imp_sid}/test")
assert t.get("code") == "HOST_KEY_UNCONFIRMED", t
api("POST", f"/api/v1/servers/{imp_sid}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]})
assert api("POST", f"/api/v1/servers/{imp_sid}/test").get("success") is True
try:
    api("POST", "/api/v1/servers/import", {"config": "Host *\n  Port 22\n"})
    raise AssertionError("empty import accepted")
except urllib.error.HTTPError as e:
    assert e.code == 400, e.code
print("ssh_config import ok:", len(names), "created,", len(skipped), "skipped")

# Startup command: typed into every new shell on the server; must be one line
try:
    api("PUT", f"/api/v1/servers/{sid}", {"startup_command": "echo a\necho b"})
    raise AssertionError("multi-line startup command accepted")
except urllib.error.HTTPError as e:
    assert e.code == 400, e.code
api("PUT", f"/api/v1/servers/{sid}", {"startup_command": " echo STARTUP-$((7*6)) "})
assert api("GET", f"/api/v1/servers/{sid}")["server"]["startup_command"] == "echo STARTUP-$((7*6))"
print("startup command set")

# A shell on a server whose key is not confirmed yet gets a structured refusal
fresh_srv = api("POST", "/api/v1/servers", {"name": f"e2e unconfirmed {RUN}", "hostname": SSH_HOST, "port": 22, "username": SSH_USER, "credential_id": cid})
fresh_sid = fresh_srv["id"] if "id" in fresh_srv else fresh_srv["data"]["id"]

# SFTP file browser: list home, make a folder, upload, download back, rename, delete, rmdir
import urllib.parse
def api_raw(method, path, data=None, headers=None):
    req = urllib.request.Request(BASE + path, data=data, method=method, headers={"Origin": BASE, **(headers or {})})
    with opener.open(req) as r:
        return r.status, r.read(), {k.lower(): v for k, v in r.headers.items()}

home = api("GET", f"/api/v1/servers/{sid}/files")
assert home["path"].startswith("/") and isinstance(home["entries"], list), home
print("sftp home:", home["path"], len(home["entries"]), "entries")
folder = f"{home['path']}/svrctlrs-e2e-{RUN}"
api("POST", f"/api/v1/servers/{sid}/files/mkdir", {"path": folder})
payload = ("héllo ✓ 🦀\n" * 2000).encode()
qp = lambda p: "?path=" + urllib.parse.quote(p, safe="")
st, _, _ = api_raw("PUT", f"/api/v1/servers/{sid}/files/content{qp(folder + '/up.txt')}", payload, {"Content-Type": "application/octet-stream"})
assert st == 200, st
st, body, hdrs = api_raw("GET", f"/api/v1/servers/{sid}/files/content{qp(folder + '/up.txt')}")
assert body == payload, (len(body), len(payload))
assert "attachment" in hdrs.get("content-disposition", ""), hdrs
listing = api("GET", f"/api/v1/servers/{sid}/files{qp(folder)}")
names = {e["name"]: e for e in listing["entries"]}
assert names["up.txt"]["kind"] == "file" and names["up.txt"]["size"] == len(payload), names
assert listing["parent"] == home["path"], listing["parent"]
api("POST", f"/api/v1/servers/{sid}/files/rename", {"from": folder + "/up.txt", "to": folder + "/renamed.txt"})
try:
    api("POST", f"/api/v1/servers/{sid}/files/rename", {"from": folder + "/renamed.txt", "to": folder + "/renamed.txt"})
    raise AssertionError("rename onto itself accepted")
except urllib.error.HTTPError as e:
    assert e.code == 409, e.code
try:
    api("DELETE", f"/api/v1/servers/{sid}/files{qp(folder)}")
    raise AssertionError("non-empty folder deleted")
except urllib.error.HTTPError as e:
    assert e.code == 409, e.code
api("DELETE", f"/api/v1/servers/{sid}/files{qp(folder + '/renamed.txt')}")
api("DELETE", f"/api/v1/servers/{sid}/files{qp(folder)}")
try:
    api("GET", f"/api/v1/servers/{sid}/files{qp(folder)}")
    raise AssertionError("deleted folder still listed")
except urllib.error.HTTPError as e:
    assert e.code == 404, e.code
try:
    api("GET", f"/api/v1/servers/{sid}/files?path=relative")
    raise AssertionError("relative path accepted")
except urllib.error.HTTPError as e:
    assert e.code == 400, e.code
print("sftp mkdir/upload/download/rename/delete ok")

# Local servers can no longer be created
try:
    api("POST", "/api/v1/servers", {"name": f"e2e local {RUN}", "is_local": True, "port": 22})
    raise AssertionError("local server was accepted")
except urllib.error.HTTPError as e:
    assert e.code == 400, e.code
    print("local server refused ok")

hdrs = {"Cookie": cookie, "Origin": BASE}

async def recv_until(ws, pred, timeout=15):
    frames = []
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=end - time.time())
        except asyncio.TimeoutError:
            break
        if isinstance(raw, (bytes, bytearray)):
            assert BINARY, "binary frame although binary was not requested"
            m = {"type": "output", "seq": int.from_bytes(raw[:8], "big"), "data": raw[8:].decode("utf-8", "replace")}
        else:
            m = json.loads(raw)
            assert not (BINARY and m["type"] == "output" and "seq" in m), "JSON output although binary was requested"
        frames.append(m)
        if pred(m):
            break
    return frames

async def main():
    # 0. a shell on an unconfirmed server is refused with the fingerprint to confirm
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": fresh_sid, "cols": 80, "rows": 24, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        err = fr[-1]
        assert err["type"] == "error" and err.get("code") == "host_key_unconfirmed", fr
        assert err["details"]["fingerprint"] == challenge["fingerprint"], err
        print("shell refused until confirmed ok")
        # confirm it and the same socket opens a shell
        api("POST", f"/api/v1/servers/{fresh_sid}/host-keys/trust", {"fingerprint": err["details"]["fingerprint"]})
        await ws.send(json.dumps({"type": "shell", "server_id": fresh_sid, "cols": 80, "rows": 24, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        await ws.send(json.dumps({"type": "input", "data": "exit\n"}))
        await recv_until(ws, lambda m: m["type"] == "closed", 15)
        print("shell after confirmation ok")

    # 0b. a shell through the jump host
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": hop_sid, "cols": 80, "rows": 24, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        await ws.send(json.dumps({"type": "input", "data": "echo VIA-$((60+6))\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "VIA-66" in m["data"] and not m["data"].startswith("echo"), 15)
        assert fr, "no output through the jump host"
        await ws.send(json.dumps({"type": "input", "data": "exit\n"}))
        await recv_until(ws, lambda m: m["type"] == "closed", 15)
        print("shell through jump host ok")

    # 1. open a shell, run a marker command, note the last seq, then drop the socket (no close)
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 100, "rows": 30, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        conn = [m for m in fr if m["type"] == "connected"]
        assert conn, fr
        session_id = conn[0]["session_id"]
        print("session", session_id)
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "STARTUP-42" in m["data"] and "echo" not in m["data"], 15)
        assert fr and "STARTUP-42" in fr[-1]["data"], "startup command did not run"
        print("startup command ran ok")
        await ws.send(json.dumps({"type": "input", "data": "echo MARKER-ONE-$((40+2))\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "MARKER-ONE-42" in m["data"] and not m["data"].startswith("echo"), 15)
        seqs = [m["seq"] for m in fr if m["type"] == "output"]
        assert seqs and all(seqs[i] < seqs[i+1] for i in range(len(seqs)-1)), seqs
        last_seq = seqs[-1]
        print("saw marker one at seq", last_seq)
        # output produced while nobody is attached
        await ws.send(json.dumps({"type": "input", "data": "echo MARKER-TWO-$((50+2))\n"}))
        await asyncio.sleep(1.0)
        # drop without close/detach
    print("socket dropped; sessions:", api("GET", "/api/v1/sessions"))
    await asyncio.sleep(1.0)

    # 2. reattach with last_seq: expect replay of the gap containing MARKER-TWO-52, then live output
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "attach", "session_id": session_id, "last_seq": last_seq, "cols": 100, "rows": 30, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] == "replay_end", 15)
        kinds = [m["type"] for m in fr]
        assert kinds[0] == "connected" and fr[0]["session_id"] == session_id, fr[:2]
        assert all("seq" in m for m in fr if m["type"] == "output"), "unnumbered output on attach"
        assert "replay_start" in kinds and "replay_end" in kinds, kinds
        replay = "".join(m["data"] for m in fr if m["type"] == "output")
        assert "MARKER-TWO-52" in replay, replay
        assert all(m["seq"] > last_seq for m in fr if m["type"] == "output")
        print("replayed gap ok:", len([m for m in fr if m["type"] == "output"]), "frames")
        await ws.send(json.dumps({"type": "input", "data": "echo MARKER-THREE\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "MARKER-THREE" in m["data"] and not m["data"].startswith("echo"), 15)
        assert fr, "no live output after reattach"
        print("live output after reattach ok")
        await ws.send(json.dumps({"type": "input", "data": f"printf '%s\\n' '{UNICODE_MARKER}'\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and UNICODE_MARKER in m["data"] and "printf" not in m["data"], 15)
        assert fr, "multibyte marker did not arrive intact"
        print("multibyte output intact:", UNICODE_MARKER)
        # 3. attach again on a stale seq far behind → fine (ring has all); then window lost from 0 = full redraw
        await ws.send(json.dumps({"type": "detach"}))
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "attach", "session_id": session_id, "last_seq": 0, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] == "replay_end", 15)
        full = "".join(m["data"] for m in fr if m["type"] == "output")
        assert "MARKER-ONE-42" in full and "MARKER-THREE" in full, full[:300]
        print("full redraw from seq 0 ok")
        # 4. exit the shell → closed
        await ws.send(json.dumps({"type": "input", "data": "exit\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "closed", 15)
        assert fr and fr[-1]["type"] == "closed", fr[-3:]
        print("closed delivered:", fr[-1]["data"])
    # 5. attaching to the dead session → session_gone
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "attach", "session_id": session_id, "last_seq": 0, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] == "error", 10)
        assert fr and fr[-1].get("code") == "session_gone", fr
        print("session_gone ok; sessions now:", api("GET", "/api/v1/sessions")["sessions"])
        # and a shell on the same socket still works
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 80, "rows": 24}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        sid2 = fr[-1]["session_id"]
        print("fresh shell after session_gone ok", sid2)
    # 6. TTL: detached session expires (server started with SESSION_DETACH_TTL_SECS=TTL)
    await asyncio.sleep(TTL + 1.5)
    left = api("GET", "/api/v1/sessions")["sessions"]
    assert all(s["id"] != sid2 for s in left), left
    print("TTL expiry ok; remaining:", left)
    print("ALL E2E CHECKS PASSED")

asyncio.run(main())
