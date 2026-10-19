#!/usr/bin/env python3
"""Backpressure check: what a large burst of shell output costs the server, and what a client
that stops reading does to it. `scripts/dev/run-e2e.sh backpressure` starts a debug server and
runs this with SERVER_PID set (the server's memory is read from /proc).

Three passes on one real SSH target, each printing the server's resident memory:
  fast   — read a burst of BURST_MB as fast as possible; every byte must arrive, in order.
  stall  — start the same burst, stop reading for STALL_SECS (the socket's buffers fill and
           the server's sends block), then read again: memory must stay bounded, output the
           ring no longer holds is reported as a lost window followed by a redraw from the
           ring, and the session must still answer afterwards.
  ctrl-c — `yes` until Ctrl-C; how long until the prompt is back.
Exits 1 if memory grows past LIMIT_MB over the starting point, or a pass fails.
"""
import asyncio, json, os, re, time, urllib.request, http.cookiejar
import websockets

BASE = os.environ.get("E2E_URL", "http://127.0.0.1:18080")
WS = BASE.replace("http", "ws", 1) + "/ws/terminal/pty"
USER = os.environ.get("E2E_USER", "admin")
PASSWORD = os.environ.get("E2E_PASSWORD", "e2e-password-123456")
SSH_HOST = os.environ["E2E_SSH_HOST"]
SSH_USER = os.environ.get("E2E_SSH_USER", "root")
SSH_KEY = os.path.expanduser(os.environ.get("E2E_SSH_KEY", "~/.ssh/id_ed25519"))
PID = int(os.environ["SERVER_PID"])
BURST_MB = int(os.environ.get("BURST_MB", "200"))
STALL_SECS = float(os.environ.get("STALL_SECS", "15"))
LIMIT_MB = float(os.environ.get("LIMIT_MB", "64"))
RUN = str(int(time.time()))

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method,
        headers={"Content-Type": "application/json", "Origin": BASE})
    with opener.open(req) as r:
        return json.loads(r.read() or b"null")


def ident(obj):
    return obj.get("id") or obj["data"]["id"]


def rss_mb():
    with open(f"/proc/{PID}/status") as f:
        for line in f:
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024
    return 0.0


api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
cookie = "; ".join(f"{c.name}={c.value}" for c in cj)
cid = ident(api("POST", "/api/v1/credentials", {"name": f"bp key {RUN}", "credential_type": "ssh_key",
    "value": open(SSH_KEY).read(), "username": SSH_USER}))
sid = ident(api("POST", "/api/v1/servers", {"name": f"bp target {RUN}", "hostname": SSH_HOST, "port": 22,
    "username": SSH_USER, "credential_id": cid}))
t = api("POST", f"/api/v1/servers/{sid}/test")
if t.get("code") == "HOST_KEY_UNCONFIRMED":
    api("POST", f"/api/v1/servers/{sid}/host-keys/trust", {"fingerprint": t["details"]["fingerprint"]})

failures = []
baseline = rss_mb()
peak = baseline


def sample(label=""):
    global peak
    m = rss_mb()
    peak = max(peak, m)
    if label:
        print(f"  rss {m:7.1f} MB  {label}", flush=True)
    return m


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
    if not ok:
        failures.append(name)


class Session:
    def __init__(self, ws):
        self.ws = ws
        self.bytes = 0
        self.last_seq = 0
        self.gaps = 0
        self.lost = 0
        self.xs_after_lost = 0  # x's received since the last lost-window notice
        self.tail = b""

    async def send(self, data):
        await self.ws.send(json.dumps({"type": "input", "data": data}))

    async def read_until(self, needle: bytes, timeout: float, count_x=False):
        """Read frames until `needle` shows up in the output. Returns the x's seen."""
        xs = 0
        end = time.monotonic() + timeout
        last_sample = 0.0
        while True:
            left = end - time.monotonic()
            if left <= 0:
                raise TimeoutError(needle)
            msg = await asyncio.wait_for(self.ws.recv(), left)
            if isinstance(msg, bytes):
                seq = int.from_bytes(msg[:8], "big")
                if seq != self.last_seq + 1 and self.last_seq:
                    self.gaps += 1
                self.last_seq = seq
                data = msg[8:]
                self.bytes += len(data)
                n = data.count(b"x")
                self.xs_after_lost += n
                if count_x:
                    xs += n
                self.tail = (self.tail + data)[-4096:]
                if needle in self.tail:
                    return xs
            else:
                res = json.loads(msg)
                if res["type"] == "replay_window_lost":
                    self.lost += 1
                    self.xs_after_lost = 0
                elif res["type"] == "closed":
                    raise RuntimeError("session closed: " + res.get("data", ""))
            if time.monotonic() - last_sample > 0.5:
                sample()
                last_sample = time.monotonic()


async def open_shell():
    ws = await websockets.connect(WS, additional_headers={"Cookie": cookie, "Origin": BASE},
                                  max_size=None)
    await ws.send(json.dumps({"type": "shell", "server_id": sid, "cols": 200, "rows": 50, "binary": True}))
    s = Session(ws)
    while True:
        res = json.loads(m) if isinstance(m := await asyncio.wait_for(ws.recv(), 20), str) else None
        if res and res["type"] == "connected":
            break
        if res and res["type"] == "error":
            raise RuntimeError(res["data"])
    await s.send("echo READY-$((40+2))\n")
    await s.read_until(b"READY-42", 20)
    return s


# A burst of exactly BURST_MB MiB of 'x' in 200-byte lines, then a marker the echo cannot fake
BURST = f"head -c {BURST_MB * 1024 * 1024} /dev/zero | tr '\\0' x | fold -w 200; echo; echo END-$((6*7))\n"


async def main():
    print(f"server pid {PID}, baseline rss {baseline:.1f} MB, burst {BURST_MB} MiB")

    # ── fast reader
    s = await open_shell()
    sample("shell open")
    t0 = time.monotonic()
    await s.send(BURST)
    xs = await s.read_until(b"END-42", 600, count_x=True)
    dt = time.monotonic() - t0
    m = sample(f"fast: {s.bytes / 1e6:.0f} MB in {dt:.1f} s ({s.bytes / 1e6 / dt:.0f} MB/s)")
    check("fast reader gets every byte", xs >= BURST_MB * 1024 * 1024, f"{xs} x's")
    check("fast reader sees no gaps or lost windows", s.gaps == 0 and s.lost == 0, f"gaps {s.gaps}, lost {s.lost}")
    await s.ws.close()

    # ── stalled reader: stop reading mid-burst; TCP pushes back on the server's sends
    s = await open_shell()
    await s.send(BURST)
    await s.read_until(b"xxxxxxxxxx", 20)
    print(f"  stalling for {STALL_SECS:.0f} s")
    end = time.monotonic() + STALL_SECS
    stall_peak = 0.0
    while time.monotonic() < end:
        stall_peak = max(stall_peak, sample())
        await asyncio.sleep(0.5)
    sample(f"after the stall (peak during it {stall_peak:.1f} MB)")
    t0 = time.monotonic()
    xs = await s.read_until(b"END-42", 600, count_x=True)
    sample(f"stall: drained in {time.monotonic() - t0:.1f} s; gaps {s.gaps}, lost windows {s.lost}, "
           f"{xs / 1e6:.0f} M of {BURST_MB * 1.048576:.0f} M x's delivered")
    if s.lost:
        # The ring (2 MiB) is redrawn after the notice, so the screen is not left blank
        check("a lost window is followed by a redraw from the ring", s.xs_after_lost >= 1_000_000,
              f"{s.xs_after_lost} x's after the notice")
    await s.send("echo ALIVE-$((50+5))\n")
    try:
        await s.read_until(b"ALIVE-55", 20)
        check("session answers after the stall", True)
    except Exception as e:
        check("session answers after the stall", False, repr(e))
    await s.ws.close()

    # ── Ctrl-C under a flood
    s = await open_shell()
    await s.send("yes\n")
    await s.read_until(b"y\r\ny\r\ny\r\n", 20)
    await asyncio.sleep(2)
    t0 = time.monotonic()
    await s.send("\x03")
    await s.send("echo STOPPED-$((7*11))\n")
    try:
        await s.read_until(b"STOPPED-77", 120)
        check("Ctrl-C stops a flood", True, f"prompt back after {time.monotonic() - t0:.2f} s")
    except Exception as e:
        check("Ctrl-C stops a flood", False, repr(e))
    await s.ws.close()

    await asyncio.sleep(1)
    sample("end")
    growth = peak - baseline
    check(f"server memory stays within {LIMIT_MB:.0f} MB of the baseline", growth <= LIMIT_MB,
          f"baseline {baseline:.1f} MB, peak {peak:.1f} MB (+{growth:.1f})")


asyncio.run(main())
print("FAILED: " + ", ".join(failures) if failures else "BACKPRESSURE CHECK PASSED")
raise SystemExit(1 if failures else 0)
