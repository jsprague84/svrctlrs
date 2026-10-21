#!/usr/bin/env python3
"""End-to-end check of kept shells (tmux, ROADMAP 23) against real tmux on a real host.

`scripts/dev/run-e2e.sh kept` runs it in two phases around a restart of the server:

    start   a server with "Keep shells in tmux", a shell on it, a variable set in that shell;
            the session id and last seq go to $E2E_STATE
    (the runner stops the server with SIGTERM and starts it again on the same database)
    resume  the session is listed as waiting; attach reconnects it; the variable is still set
            (the same shell); closing it ends the tmux session on the host

The host needs tmux; the key must log in as E2E_SSH_USER. Needs the `websockets` package.
"""
import asyncio, json, os, re, subprocess, sys, time, urllib.request, http.cookiejar
import websockets

BASE = os.environ.get("E2E_URL", "http://127.0.0.1:18080")
WS = BASE.replace("http", "ws", 1) + "/ws/terminal/pty"
USER = os.environ.get("E2E_USER", "admin")
PASSWORD = os.environ.get("E2E_PASSWORD", "e2e-password-123456")
SSH_HOST = os.environ["E2E_SSH_HOST"]
SSH_USER = os.environ.get("E2E_SSH_USER", "root")
SSH_KEY = os.path.expanduser(os.environ.get("E2E_SSH_KEY", "~/.ssh/id_ed25519"))
STATE = os.environ["E2E_STATE"]
RUN = os.environ.get("E2E_RUN", str(int(time.time())))

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method,
        headers={"Content-Type": "application/json", "Origin": BASE})
    with opener.open(req) as r:
        return json.loads(r.read() or b"null")


def remote_tmux_sessions():
    out = subprocess.run(
        ["ssh", "-i", SSH_KEY, "-o", "IdentitiesOnly=yes", "-o", "BatchMode=yes",
         f"{SSH_USER}@{SSH_HOST}", "tmux ls -F '#{session_name}' 2>/dev/null || true"],
        capture_output=True, text=True, timeout=30)
    return [l.strip() for l in out.stdout.splitlines() if l.strip()]


async def recv_until(ws, pred, timeout=20):
    frames = []
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = await asyncio.wait_for(ws.recv(), timeout=end - time.time())
        except asyncio.TimeoutError:
            break
        if isinstance(raw, (bytes, bytearray)):
            m = {"type": "output", "seq": int.from_bytes(raw[:8], "big"),
                 "data": raw[8:].decode("utf-8", "replace")}
        else:
            m = json.loads(raw)
        frames.append(m)
        if pred(m):
            break
    return frames


def last_seq(frames, current):
    return max([current] + [f["seq"] for f in frames if f.get("type") == "output" and "seq" in f])


ANSI = re.compile(r"\x1b(\[[0-9;?>=]*[ -/]*[@-~]|\][^\x07\x1b]*(\x07|\x1b\\)|[()][A-Z0-9]|[=>78])")


def shown(frames):
    """What the frames printed, escape sequences removed: tmux redraws with cursor moves."""
    return ANSI.sub("", "".join(f["data"] for f in frames if f.get("type") == "output"))


async def until_shown(ws, text, timeout=20):
    frames = []
    end = time.time() + timeout
    while time.time() < end and text not in shown(frames):
        frames += await recv_until(ws, lambda m: m["type"] == "output", end - time.time())
    return frames


async def start():
    api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
    cred = api("POST", "/api/v1/credentials", {"name": f"kept key {RUN}", "credential_type": "ssh_key",
                                               "value": open(SSH_KEY).read(), "username": SSH_USER})
    cid = cred["id"] if "id" in cred else cred["data"]["id"]
    srv = api("POST", "/api/v1/servers", {"name": f"kept target {RUN}", "hostname": SSH_HOST, "port": 22,
                                          "username": SSH_USER, "credential_id": cid, "persistent_shell": True})
    sid = srv["id"] if "id" in srv else srv["data"]["id"]
    assert api("GET", f"/api/v1/servers/{sid}")["server"]["persistent_shell"] is True
    t = api("POST", f"/api/v1/servers/{sid}/test")
    api("POST", f"/api/v1/servers/{sid}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]})
    # The keep time is the account's own choice, within the server's maximum
    keep = api("PUT", "/api/v1/sessions/keep", {"detach_ttl_secs": 3600})
    assert keep == {"detach_ttl_secs": 3600, "detach_ttl_chosen": True}, keep

    hdrs = {"Cookie": "; ".join(f"{c.name}={c.value}" for c in cj), "Origin": BASE}
    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 100, "rows": 30, "binary": True}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected", fr
        session_id = fr[-1]["session_id"]
        assert f"svrctlrs-{session_id}" in remote_tmux_sessions(), remote_tmux_sessions()
        await ws.send(json.dumps({"type": "input", "data": f"KEPT_MARK=kept-{RUN}; echo set-$KEPT_MARK-done\n"}))
        fr = await until_shown(ws, f"set-kept-{RUN}-done")
        assert f"set-kept-{RUN}-done" in shown(fr), shown(fr)
        seq = last_seq(fr, 0)
    json.dump({"session_id": session_id, "last_seq": seq, "sid": sid, "run": RUN}, open(STATE, "w"))
    print(f"kept shell started in tmux: svrctlrs-{session_id} (seq {seq})")


async def resume():
    st = json.load(open(STATE))
    session_id = st["session_id"]
    run = st["run"]  # the marker the first phase set
    api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
    hdrs = {"Cookie": "; ".join(f"{c.name}={c.value}" for c in cj), "Origin": BASE}
    listed = api("GET", "/api/v1/sessions")
    mine = [s for s in listed["sessions"] if s["id"] == session_id]
    assert mine and mine[0]["persistent"] and mine[0]["waiting"], listed
    assert listed["detach_ttl_secs"] == 3600 and listed["detach_ttl_chosen"], listed
    assert f"svrctlrs-{session_id}" in remote_tmux_sessions(), "tmux kept the shell across the restart"
    print("kept shell waiting after the restart ok")

    async with websockets.connect(WS, additional_headers=hdrs) as ws:
        await ws.send(json.dumps({"type": "attach", "session_id": session_id, "last_seq": st["last_seq"],
                                  "cols": 100, "rows": 30, "binary": True}))
        fr = await recv_until(ws, lambda m: m["type"] in ("connected", "error"), 30)
        assert fr[-1]["type"] == "connected" and fr[-1]["session_id"] == session_id, fr
        await ws.send(json.dumps({"type": "input", "data": "echo still-$KEPT_MARK-here\n"}))
        fr = await until_shown(ws, f"still-kept-{run}-here")
        assert f"still-kept-{run}-here" in shown(fr), "the variable is gone: not the same shell\n" + repr(shown(fr)[-600:])
        assert all(f["seq"] > st["last_seq"] for f in fr if f.get("type") == "output" and "seq" in f), \
            "output numbered after what the client saw"
        print("reattached to the same shell after the restart ok")
        await ws.send(json.dumps({"type": "close"}))
    for _ in range(20):
        if f"svrctlrs-{session_id}" not in remote_tmux_sessions():
            break
        time.sleep(0.5)
    assert f"svrctlrs-{session_id}" not in remote_tmux_sessions(), "closing ends the tmux session"
    assert not [s for s in api("GET", "/api/v1/sessions")["sessions"] if s["id"] == session_id]
    api("PUT", "/api/v1/sessions/keep", {"detach_ttl_secs": None})
    print("closing a kept shell ends its tmux session ok")


asyncio.run({"start": start, "resume": resume}[sys.argv[1]]())
