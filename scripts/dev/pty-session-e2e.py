#!/usr/bin/env python3
"""End-to-end check of persistent PTY sessions against a running server.

`scripts/dev/run-e2e.sh` starts the server and runs this twice. By hand, it needs a server
started with a short TTL and an SSH target the given key can log in to:

    DATABASE_URL=sqlite:/tmp/e2e.db ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
    ENCRYPTION_KEY=$(openssl rand -hex 32) SESSION_SECURE=false SESSION_DETACH_TTL_SECS=3 \
    target/debug/server --addr 127.0.0.1:18080 &
    E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 \
    python3 scripts/dev/pty-session-e2e.py        # needs the `websockets` package

Exercises: shell, dropped socket, attach with last_seq (gap replay), live input after
reattach, full redraw from seq 0, `closed` on exit, `session_gone`, TTL expiry, and session
recording (opt-in per server, output only, invisible to another user), and agent tokens
(a shell through a ticket, audit attribution, presets, revocation), approvals (a held
delete run once the owner approves), and the operations diagnose, exec and onboard. Output
frames
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

# The editor's tagged save on real OpenSSH: ETag on a small file, If-Match writes, a stale tag
# is refused. The file has a quote and a space in its name for the editor tab below.
edit_path = f"{home['path']}/svrctlrs e2e it's-{RUN}.txt"
edit_marker = f"E2E-EDIT-{RUN}"
st, body, _ = api_raw("PUT", f"/api/v1/servers/{sid}/files/content{qp(edit_path)}", b"draft\r\n", {"Content-Type": "application/octet-stream"})
assert st == 200, st
st, body, hdrs = api_raw("GET", f"/api/v1/servers/{sid}/files/content{qp(edit_path)}&view=true")
tag = hdrs.get("etag")
assert body == b"draft\r\n" and tag and tag.startswith('"'), (body, hdrs)
st, body, _ = api_raw("PUT", f"/api/v1/servers/{sid}/files/content{qp(edit_path)}", f"{edit_marker}\r\n".encode(), {"Content-Type": "application/octet-stream", "If-Match": tag})
saved = json.loads(body)
assert saved["message"] == "Saved" and saved["etag"] != tag, saved
try:
    api_raw("PUT", f"/api/v1/servers/{sid}/files/content{qp(edit_path)}", b"stale", {"Content-Type": "application/octet-stream", "If-Match": tag})
    raise AssertionError("stale If-Match accepted")
except urllib.error.HTTPError as e:
    assert e.code == 412, e.code
st, body, _ = api_raw("GET", f"/api/v1/servers/{sid}/files/content{qp(edit_path)}")
assert body == f"{edit_marker}\r\n".encode(), body
print("sftp ETag / If-Match save ok")

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

    # 0c. "Edit in terminal": the server's own editor on that file, after the startup command
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 100, "rows": 30, "binary": BINARY, "edit": edit_path}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        fr = await recv_until(ws, lambda m: m["type"] == "output" and edit_marker in m["data"] and "printf" not in m["data"], 20)
        assert fr and edit_marker in fr[-1]["data"], "the editor did not show the file"
        await asyncio.sleep(0.5)
        await ws.send(json.dumps({"type": "input", "data": ":q!\r"}))
        await ws.send(json.dumps({"type": "input", "data": "echo BACK-$((20+3))\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "BACK-23" in m["data"] and "echo" not in m["data"], 15)
        assert fr, "no shell after quitting the editor"
        await ws.send(json.dumps({"type": "input", "data": "exit\n"}))
        await recv_until(ws, lambda m: m["type"] == "closed", 15)
        # A relative path is refused, and no shell opens
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 80, "rows": 24, "binary": BINARY, "edit": "relative.txt"}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 15)
        assert fr[-1]["type"] == "error", fr
    api("DELETE", f"/api/v1/servers/{sid}/files{qp(edit_path)}")
    print("edit in terminal ok")

    # 1. open a shell, run a marker command, note the last seq, then drop the socket (no close)
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 100, "rows": 30, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        conn = [m for m in fr if m["type"] == "connected"]
        assert conn, fr
        assert conn[0].get("recording") is False, conn[0]
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

    # 7. Recording: opt-in per server, what the terminal shows (never keystrokes), the owner's only
    rec_srv = api("POST", "/api/v1/servers", {"name": f"e2e recorded {RUN}", "hostname": SSH_HOST, "port": 22,
        "username": SSH_USER, "credential_id": cid, "record_sessions": True})
    rec_sid = rec_srv["id"] if "id" in rec_srv else rec_srv["data"]["id"]
    assert api("GET", f"/api/v1/servers/{rec_sid}")["server"]["record_sessions"] is True
    trust_first_contact(rec_sid)
    secret = f"hunter2-e2e-{RUN}"
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": rec_sid, "cols": 100, "rows": 30, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        conn = fr[-1]
        assert conn["type"] == "connected" and conn.get("recording") is True, conn
        assert "recorded" in conn["data"], conn["data"]
        live = [x for x in api("GET", "/api/v1/sessions")["sessions"] if x["id"] == conn["session_id"]]
        assert live and live[0]["recorded"] is True, live
        await ws.send(json.dumps({"type": "input", "data": "echo REC-$((9*9))\n"}))
        await recv_until(ws, lambda m: m["type"] == "output" and "REC-81" in m["data"] and not m["data"].startswith("echo"), 15)
        await ws.send(json.dumps({"type": "resize", "cols": 120, "rows": 40}))
        # A password at a prompt that does not echo never reaches the recording
        await ws.send(json.dumps({"type": "input", "data": "stty -echo; read -r SECRET; stty echo; echo READ-DONE-$((2*2))\n"}))
        await asyncio.sleep(0.8)
        await ws.send(json.dumps({"type": "input", "data": secret + "\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "READ-DONE-4" in m["data"] and "echo" not in m["data"], 15)
        assert fr, "the read did not finish"
        await ws.send(json.dumps({"type": "input", "data": "exit\n"}))
        await recv_until(ws, lambda m: m["type"] == "closed", 15)
    rec = None
    for _ in range(50):
        recs = api("GET", f"/api/v1/recordings?server_id={rec_sid}")["recordings"]
        if recs and recs[0]["status"] != "recording":
            rec = recs[0]
            break
        await asyncio.sleep(0.2)
    assert rec and rec["status"] == "complete" and rec["size_bytes"] > 0 and rec["duration_ms"] > 0, recs
    assert rec["session_id"] == conn["session_id"] and rec["server_name"] == f"e2e recorded {RUN}", rec
    st, body, rh = api_raw("GET", f"/api/v1/recordings/{rec['id']}/cast")
    assert st == 200 and rh.get("content-type") == "application/x-asciicast", (st, rh)
    cast = body.decode()
    assert len(body) == rec["size_bytes"], (len(body), rec["size_bytes"])
    lines = cast.splitlines()
    header = json.loads(lines[0])
    assert header["version"] == 2 and (header["width"], header["height"]) == (100, 30), header
    events = [json.loads(line) for line in lines[1:]]
    shown = "".join(e[2] for e in events if e[1] == "o")
    assert "REC-81" in shown and "READ-DONE-4" in shown, shown[-400:]
    assert ["r", "120x40"] in [e[1:] for e in events], [e for e in events if e[1] != "o"]
    assert secret not in cast, "a password typed at a silent prompt was recorded"
    st, _, rh = api_raw("GET", f"/api/v1/recordings/{rec['id']}/cast?download=true")
    assert "attachment" in rh.get("content-disposition", ""), rh
    print("recording ok:", rec["size_bytes"], "bytes,", len(events), "events, no keystrokes")

    # Another account sees nothing of it
    other = f"e2e-other-{RUN}"
    api("POST", "/api/v1/users", {"username": other, "password": "another-long-password"})
    other_jar = http.cookiejar.CookieJar()
    other_opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(other_jar))
    def other_api(method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(BASE + path, data=data, method=method,
            headers={"Content-Type": "application/json", "Origin": BASE})
        with other_opener.open(req) as r:
            return json.loads(r.read() or b"null")
    other_api("POST", "/api/v1/auth/login", {"username": other, "password": "another-long-password"})
    assert other_api("GET", "/api/v1/recordings")["recordings"] == []
    for method, path in [("GET", f"/api/v1/recordings/{rec['id']}"), ("GET", f"/api/v1/recordings/{rec['id']}/cast"),
                         ("DELETE", f"/api/v1/recordings/{rec['id']}")]:
        try:
            other_api(method, path)
            raise AssertionError(f"{method} {path} worked for another user")
        except urllib.error.HTTPError as e:
            assert e.code == 404, (method, path, e.code)
    print("recording invisible to another user ok")
    api("DELETE", f"/api/v1/recordings/{rec['id']}")
    try:
        api("GET", f"/api/v1/recordings/{rec['id']}")
        raise AssertionError("deleted recording still there")
    except urllib.error.HTTPError as e:
        assert e.code == 404, e.code
    print("recording deleted ok")

    # Agent tokens: a full-admin token opens a shell on the real host through a WebSocket
    # ticket (no cookie, no Origin, as an agent would), its audit row names it, a read-only
    # token gets no terminal, and revoking the token ends the shell it started
    agent = f"e2e agent {RUN}"
    made = api("POST", "/api/v1/tokens", {"name": agent, "preset": "full_admin", "expires_in_days": 30})
    secret, tid = made["secret"], made["token"]["id"]
    reader = api("POST", "/api/v1/tokens", {"name": f"e2e reader {RUN}", "preset": "read_only", "expires_in_days": None})["secret"]

    def token_call(token, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(BASE + path, data=data, method=method,
            headers={"Content-Type": "application/json", "Authorization": f"Bearer {token}"})
        try:
            with urllib.request.urlopen(req) as r:
                return r.status, json.loads(r.read() or b"null")
        except urllib.error.HTTPError as e:
            return e.code, json.loads(e.read() or b"null")

    st, _ = token_call(reader, "GET", "/api/v1/servers")
    assert st == 200, st
    st, body = token_call(reader, "POST", "/api/v1/auth/ws-ticket")
    assert st == 403 and body["error"]["code"] == "TOKEN_SCOPE", (st, body)
    st, body = token_call(reader, "GET", f"/api/v1/servers/{sid}/files")
    assert st == 403, st

    # Behind the edge's token router an agent sends its token on the handshake itself (no
    # ticket, no cookie), and a token never reaches the password login
    async with websockets.connect(WS, additional_headers={"Authorization": f"Bearer {secret}"}) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 80, "rows": 24, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        header_session = fr[-1]["session_id"]
    api("DELETE", f"/api/v1/sessions/{header_session}")
    st, body = token_call(secret, "POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
    assert st == 400 and body["error"]["code"] == "BAD_REQUEST", (st, body)
    print("agent token: shell with the token on the handshake, login carrying a token refused ok")

    st, body = token_call(secret, "POST", "/api/v1/auth/ws-ticket")
    assert st == 200, (st, body)
    async with websockets.connect(f"{WS}?ticket={body['ticket']}") as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 80, "rows": 24, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        token_session = fr[-1]["session_id"]
        await ws.send(json.dumps({"type": "input", "data": "echo TOKEN-$((70+7))\n"}))
        fr = await recv_until(ws, lambda m: m["type"] == "output" and "TOKEN-77" in m["data"] and not m["data"].startswith("echo"), 15)
        assert fr and "TOKEN-77" in fr[-1]["data"], "no output in the token's shell"
        api("POST", f"/api/v1/tokens/{tid}/revoke")
        fr = await recv_until(ws, lambda m: m["type"] == "error", 10)
        assert fr and fr[-1].get("code") == "token_revoked", fr[-3:]
    st, body = token_call(secret, "GET", "/api/v1/servers")
    assert st == 401 and body["error"]["code"] == "TOKEN_REVOKED", (st, body)
    await asyncio.sleep(0.5)
    live = [x["id"] for x in api("GET", "/api/v1/sessions")["sessions"]]
    assert token_session not in live, "the revoked token's shell is still running"
    entries = api("GET", "/api/v1/audit?limit=50")["entries"]
    assert any(e["action"] == "session.pty_opened" and e.get("token_name") == agent for e in entries), \
        "no audit row names the token"
    print("agent token: shell via ticket, audit attribution, read-only refused, revoke ends shell ok")

    # Approvals: a token set to ask first gets 202 for a delete; nothing happens until the
    # owner approves, then the server makes the call as the token
    asker = api("POST", "/api/v1/tokens", {"name": f"e2e asker {RUN}", "preset": "servers_keys",
                                            "expires_in_days": 30, "require_approval": True})["secret"]
    doomed = api("POST", "/api/v1/servers", {"name": f"e2e doomed {RUN}", "hostname": "192.0.2.1", "port": 22})
    doomed_id = doomed["id"] if "id" in doomed else doomed["data"]["id"]
    st, body = token_call(asker, "DELETE", f"/api/v1/servers/{doomed_id}")
    assert st == 202, (st, body)
    aid = body["approval"]["id"]
    assert "/approvals?id=" in body["approval"]["url"], body
    st, _ = token_call(asker, "POST", f"/api/v1/approvals/{aid}/approve")
    assert st == 403, st
    api("GET", f"/api/v1/servers/{doomed_id}")  # still there
    decided = api("POST", f"/api/v1/approvals/{aid}/approve")
    assert decided["status"] == "done", decided
    st, body = token_call(asker, "GET", f"/api/v1/approvals/{aid}?wait=1")
    assert st == 200 and body["status"] == "done", (st, body)
    try:
        api("GET", f"/api/v1/servers/{doomed_id}")
        raise AssertionError("the approved delete did not happen")
    except urllib.error.HTTPError as e:
        assert e.code == 404, e.code
    print("approval: held, refused to the token, approved by the owner, done ok")

    # Operations from svrctlrs itself, against the real host. rotate-key and onboarding with a
    # new key are left to the loopback tests: they would edit authorized_keys on the target.
    d = api("POST", f"/api/v1/servers/{sid}/diagnose")
    assert d["ok"] is True, d
    steps = {st["step"]: st["status"] for st in d["hops"][-1]["steps"]}
    assert steps == {"dns": "ok", "tcp": "ok", "handshake": "ok", "host_key": "ok", "login": "ok"}, steps
    closed = api("POST", "/api/v1/servers", {"name": f"e2e closed {RUN}", "hostname": SSH_HOST, "port": 1,
                                            "username": SSH_USER, "credential_id": cid})
    closed_id = closed["id"] if "id" in closed else closed["data"]["id"]
    d = api("POST", f"/api/v1/servers/{closed_id}/diagnose")
    tcp = next(st for st in d["hops"][0]["steps"] if st["step"] == "tcp")
    assert d["ok"] is False and tcp["status"] == "failed" and tcp["fix"], d
    hop_d = api("POST", f"/api/v1/servers/{hop_sid}/diagnose")
    assert hop_d["ok"] is True and len(hop_d["hops"]) == 2, hop_d
    print("diagnose ok:", d["verdict"][:70])
    out = api("POST", f"/api/v1/servers/{sid}/exec", {"command": "echo $((6*7)); echo oops >&2; exit 4"})
    assert out["stdout"] == "42\n" and out["stderr"] == "oops\n" and out["exit_code"] == 4, out
    out = api("POST", f"/api/v1/servers/{sid}/exec", {"command": "sleep 5", "timeout_secs": 1})
    assert out["timed_out"] is True and out["exit_code"] is None, out
    print("exec ok: stdout, stderr, exit status, timeout")
    req = {"name": f"e2e onboard {RUN}", "hostname": SSH_HOST, "username": SSH_USER,
           "credential_id": cid, "new_key": False}
    first = api("POST", "/api/v1/servers/onboard", req)
    waiting = next(st for st in first["steps"] if st["step"] == "host_key")
    assert first["done"] is False and waiting["status"] == "waiting", first
    fp = waiting["detail"].split()[-1]
    done = api("POST", "/api/v1/servers/onboard", {**req, "fingerprint": fp})
    assert done["done"] is True, done
    print("onboard ok (no new key):", [st["step"] + "=" + st["status"] for st in done["steps"]])
    print("ALL E2E CHECKS PASSED")

asyncio.run(main())
