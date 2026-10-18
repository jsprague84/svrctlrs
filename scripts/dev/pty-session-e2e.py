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
import asyncio, json, os, time, urllib.request, http.cookiejar
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
    # 1. open a shell, run a marker command, note the last seq, then drop the socket (no close)
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 100, "rows": 30, "binary": BINARY}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        conn = [m for m in fr if m["type"] == "connected"]
        assert conn, fr
        session_id = conn[0]["session_id"]
        print("session", session_id)
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
