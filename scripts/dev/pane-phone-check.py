#!/usr/bin/env python3
"""Phone check of the terminal pane: a Pixel 7 emulation in Chromium with real touch events
(CDP Input.dispatchTouchEvent) against a running server and a real SSH target.

`scripts/dev/run-phone-check.sh` starts a throwaway server and runs this. It needs Python with
`playwright` and a Chromium (CHROMIUM=/path, else Playwright's own). WebGL is disabled so xterm
uses its DOM renderer and the screen's text can be read back.

Covers: first-contact host-key dialog → trust → shell, typing, tap-to-move-cursor, long-press
copy, a second tab and a swipe between tabs, that no hidden pane resizes its shell, CTL from
the extra-keys row (Ctrl+C interrupts `sleep`), predictive echo over a slowed link (typed text
drawn ahead of the echo and cleared after it, nothing drawn at a `read -s` prompt), reload →
resume with the screen intact, the home-screen shortcut (manifest lists the server,
`/?connect=<id>` shows its tab), and terminal preferences following the account (a change on
the phone is on the server and in a fresh desktop browser).
Screenshots land in E2E_OUT (default: a temp dir).
"""
import json, os, sys, tempfile, time, urllib.request, http.cookiejar
from playwright.sync_api import sync_playwright

BASE = os.environ.get("E2E_URL", "http://127.0.0.1:18081")
USER = os.environ.get("E2E_USER", "admin")
PASSWORD = os.environ.get("E2E_PASSWORD", "e2e-password-123456")
SSH_HOST = os.environ["E2E_SSH_HOST"]
SSH_USER = os.environ.get("E2E_SSH_USER", "root")
SSH_KEY = os.path.expanduser(os.environ.get("E2E_SSH_KEY", "~/.ssh/id_ed25519"))
CHROMIUM = os.environ.get("CHROMIUM") or None
OUT = os.environ.get("E2E_OUT") or tempfile.mkdtemp(prefix="svrctlrs-phone-")
RUN = str(int(time.time()))
NAME = f"phone target {RUN}"

cj = http.cookiejar.CookieJar()
opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(cj))


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, method=method,
        headers={"Content-Type": "application/json", "Origin": BASE})
    with opener.open(req) as r:
        return json.loads(r.read() or b"null")


api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
cred = api("POST", "/api/v1/credentials", {"name": f"phone key {RUN}", "credential_type": "ssh_key",
    "value": open(SSH_KEY).read(), "username": SSH_USER})
cid = cred.get("id") or cred["data"]["id"]
# Host key left unconfirmed on purpose: the pane must show the first-contact dialog
api("POST", "/api/v1/servers", {"name": NAME, "hostname": SSH_HOST, "port": 22,
    "username": SSH_USER, "credential_id": cid})

failures = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""), flush=True)
    if not ok:
        failures.append(name)


with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=CHROMIUM, args=["--disable-webgl", "--disable-3d-apis"])
    ctx = browser.new_context(**p.devices["Pixel 7"], base_url=BASE)
    ctx.grant_permissions(["clipboard-read", "clipboard-write"], origin=BASE)
    page = ctx.new_page()
    logs = []
    page.on("console", lambda m: logs.append(f"{m.type}: {m.text}"))
    page.on("pageerror", lambda e: logs.append(f"pageerror: {e}"))
    # A slow link on demand: `window.__lag` ms added to every message the page's sockets
    # receive (order kept). `window.__echoSeen` collects the text of every predicted-echo
    # decoration that was ever on screen.
    ctx.add_init_script("""(() => {
      window.__lag = 0;
      const d = Object.getOwnPropertyDescriptor(WebSocket.prototype, 'onmessage');
      Object.defineProperty(WebSocket.prototype, 'onmessage', {
        configurable: true,
        get() { return d.get.call(this); },
        set(fn) {
          d.set.call(this, fn && function (ev) {
            const lag = window.__lag;
            if (!lag) return fn.call(this, ev);
            setTimeout(() => fn.call(this, ev), lag);
          });
        }
      });
      window.__echoSeen = [];
      const watch = () => new MutationObserver(() => {
        for (const el of document.querySelectorAll('.local-echo')) window.__echoSeen.push(el.textContent);
      }).observe(document.body, { subtree: true, childList: true, characterData: true });
      if (document.body) watch(); else document.addEventListener('DOMContentLoaded', watch);
    })();""")
    cdp = ctx.new_cdp_session(page)
    sent = []
    page.on("websocket", lambda ws: ws.on("framesent", lambda f: sent.append(f) if isinstance(f, str) else None))

    def touch(kind, x=None, y=None):
        pts = [] if kind in ("touchEnd", "touchCancel") else [{"x": x, "y": y}]
        cdp.send("Input.dispatchTouchEvent", {"type": kind, "touchPoints": pts})

    def tap(x, y):
        touch("touchStart", x, y)
        time.sleep(0.05)
        touch("touchEnd")

    def shell_box():
        return page.locator(".terminal-shell:not(.hidden)").bounding_box()

    def screen():
        return page.inner_text(".terminal-shell:not(.hidden) .xterm-rows")

    def wait_screen(text, timeout=15):
        end = time.time() + timeout
        while time.time() < end:
            if text in screen():
                return True
            time.sleep(0.2)
        return False

    def active_tab():
        return page.evaluate("JSON.parse(sessionStorage.getItem('svrctlrs-terminal-tabs') || '{}').activeTabId")

    def open_terminal():
        page.get_by_label("New terminal").click()
        page.get_by_role("dialog").locator(f"text={NAME} >> visible=true").first.click()

    # The browser context shares the cookie of this login
    r = ctx.request.post(BASE + "/api/v1/auth/login", data={"username": USER, "password": PASSWORD},
        headers={"Origin": BASE})
    assert r.ok, r.status
    page.goto("/")
    page.wait_for_timeout(1500)

    open_terminal()
    dlg = page.get_by_role("dialog", name="Unknown host key")
    dlg.wait_for(timeout=15000)
    check("host-key dialog shows the fingerprint", "SHA256:" in dlg.inner_text())
    page.screenshot(path=f"{OUT}/hostkey.png")
    page.get_by_role("button", name="Trust and connect").click()
    check("shell after trusting the key", wait_screen(f"{SSH_USER}@", 20), screen()[-300:])
    page.wait_for_timeout(1000)

    # Single tap focuses (and would bring up the soft keyboard), then type
    box = shell_box()
    tap(box["x"] + box["width"] / 2, box["y"] + box["height"] * 0.85)
    page.wait_for_timeout(500)
    page.keyboard.type("echo 12345")
    check("typing reaches the shell", wait_screen("echo 12345"), screen()[-200:])
    page.wait_for_timeout(400)

    # Tap three cells left of the cursor: the shell's cursor moves there, X goes in before 3
    cur = page.locator(".terminal-shell:not(.hidden) .xterm-cursor").first.bounding_box()
    w = cur["width"]
    tap(cur["x"] - 3 * w + w / 2, cur["y"] + cur["height"] / 2)
    page.wait_for_timeout(600)  # past the double-tap window
    page.keyboard.type("X")
    page.keyboard.press("Enter")
    check("tap moves the cursor (12X345)", wait_screen("12X345"), screen()[-200:])

    # Long-press copies the word under the finger
    page.keyboard.type("echo longpressword")
    page.keyboard.press("Enter")
    wait_screen("longpressword\n")
    page.wait_for_timeout(400)
    rows = page.locator(".terminal-shell:not(.hidden) .xterm-rows > div")
    target = next((rows.nth(i).bounding_box() for i in range(rows.count())
                   if rows.nth(i).inner_text().strip() == "longpressword"), None)
    if target:
        touch("touchStart", target["x"] + w * 3, target["y"] + target["height"] / 2)
        time.sleep(0.7)
        touch("touchEnd")
        page.wait_for_timeout(500)
        clip = page.evaluate("navigator.clipboard.readText()")
        check("long-press copies the word", clip == "longpressword", repr(clip))
        check("copy toast shown", page.get_by_text("Copied “longpressword”").count() > 0)
    else:
        check("long-press target row found", False)

    # A second tab, then swipe right-to-left back to the first
    open_terminal()
    wait_screen(f"{SSH_USER}@", 20)
    page.wait_for_timeout(800)
    second = active_tab()
    box = shell_box()
    y = box["y"] + box["height"] / 2
    touch("touchStart", box["x"] + box["width"] - 40, y)
    for i in range(1, 11):
        touch("touchMove", box["x"] + box["width"] - 40 - i * 25, y + 2)
        time.sleep(0.016)
    touch("touchEnd")
    page.wait_for_timeout(600)
    check("swipe switches tabs", active_tab() != second)
    check("the swiped-to tab shows its own screen", "12X345" in screen(), screen()[-200:])
    tiny = [f for f in sent if '"resize"' in f and json.loads(f)["cols"] < 20]
    check("no hidden pane resizes its shell", not tiny, str(tiny[:3]))

    # CTL on the extra-keys row turns the next typed key into a control key
    tap(box["x"] + box["width"] / 2, box["y"] + box["height"] * 0.85)
    page.wait_for_timeout(500)
    page.keyboard.type("sleep 300")
    page.keyboard.press("Enter")
    page.wait_for_timeout(500)
    ctl = page.get_by_role("button", name="Control", exact=True).first.bounding_box()
    tap(ctl["x"] + ctl["width"] / 2, ctl["y"] + ctl["height"] / 2)
    page.keyboard.type("c")
    page.wait_for_timeout(500)
    page.keyboard.type("echo after-ctl-c")
    page.keyboard.press("Enter")
    check("CTL then c interrupts sleep", wait_screen("\nafter-ctl-c"), screen()[-300:])

    # Predictive echo on a slow link (250 ms on every message back): the default "auto" mode
    # measures the echo, then draws typed characters at once and clears them when they echo
    page.evaluate("window.__lag = 250")
    page.keyboard.type("echo predicted-echo", delay=150)
    page.screenshot(path=f"{OUT}/predictive-echo.png")
    seen = page.evaluate("window.__echoSeen.filter(Boolean)")
    check("predictive echo draws typed text ahead of the echo", len(seen) > 0)
    page.wait_for_timeout(1500)
    check("predictions cleared once echoed", page.locator(".local-echo").count() == 0)
    check("the echoed line is intact", "echo predicted-echo" in screen(), screen()[-200:])
    page.keyboard.press("Enter")
    wait_screen("\npredicted-echo")
    # A prompt that turns echo off and that the password pattern does not know: the epoch rule
    # alone must keep every typed character off the screen
    page.keyboard.type("read -s -p 'Key: ' v; echo \"[$v]\"", delay=40)
    page.keyboard.press("Enter")
    wait_screen("Key:")
    page.wait_for_timeout(1500)
    page.evaluate("window.__echoSeen = []")
    page.keyboard.type("hunter2 x", delay=150)
    page.wait_for_timeout(1500)
    leaked = page.evaluate("window.__echoSeen.filter(Boolean)")
    check("nothing predicted at a prompt with echo off", not leaked, str(leaked[:5]))
    page.keyboard.press("Enter")
    check("the hidden input reached the shell", wait_screen("[hunter2 x]"), screen()[-200:])
    page.evaluate("window.__lag = 0")
    page.wait_for_timeout(500)

    # Reload: the tab attaches to its server-side session with the screen intact
    page.reload()
    page.wait_for_timeout(4000)
    check("reload resumes with the screen", wait_screen("after-ctl-c", 15), screen()[-300:])
    page.keyboard.type("echo resumed-ok")
    page.keyboard.press("Enter")
    check("input after resume", wait_screen("\nresumed-ok"), screen()[-200:])
    page.screenshot(path=f"{OUT}/resumed.png")

    # The installed app's shortcuts name this server now that a shell was opened on it
    manifest = page.evaluate("fetch('/manifest.webmanifest', {credentials: 'include'}).then(r => r.json())")
    names = [sc.get("name") for sc in manifest.get("shortcuts", [])]
    check("manifest shortcuts list the server", NAME in names, str(names))
    shortcut = next((sc["url"] for sc in manifest.get("shortcuts", []) if sc.get("name") == NAME), None)
    if shortcut:
        # Launching the shortcut shows the tab already on that server instead of a new one
        before = len(page.evaluate("JSON.parse(sessionStorage.getItem('svrctlrs-terminal-tabs')).tabs"))
        page.goto(shortcut)
        page.wait_for_timeout(3000)
        after = page.evaluate("JSON.parse(sessionStorage.getItem('svrctlrs-terminal-tabs'))")
        active = next(t for t in after["tabs"] if t["id"] == after["activeTabId"])
        check("shortcut opens that server's tab", active["serverName"] == NAME and len(after["tabs"]) == before,
              f"{active['serverName']} tabs {before}->{len(after['tabs'])}")
        check("shortcut parameter dropped from the URL", "connect=" not in page.url, page.url)

    # Terminal preferences follow the account: change one on the phone, find it on the server
    # and in a desktop browser that never had it
    page.get_by_role("button", name="Terminal settings").click()
    panel = page.get_by_role("dialog", name="Terminal preferences")
    panel.get_by_label("Predictive Echo").select_option("off")
    page.wait_for_timeout(1500)  # past the save delay
    saved = api("GET", "/api/v1/users/me/preferences/terminal")["value"] or {}
    check("a preference changed on the phone is saved to the account",
          saved.get("predictiveEcho") == "off", str(saved)[:200])
    desk = browser.new_context(base_url=BASE, viewport={"width": 1280, "height": 800})
    r = desk.request.post(BASE + "/api/v1/auth/login", data={"username": USER, "password": PASSWORD},
        headers={"Origin": BASE})
    dpage = desk.new_page()
    dpage.goto("/")
    dpage.wait_for_timeout(2500)
    local = dpage.evaluate("JSON.parse(localStorage.getItem('svrctlrs-terminal-prefs') || '{}')")
    check("a fresh desktop browser picks up the account's preferences",
          local.get("predictiveEcho") == "off", str(local)[:200])
    desk.close()
    panel.get_by_label("Predictive Echo").select_option("auto")
    page.keyboard.press("Escape")
    page.wait_for_timeout(1500)
    saved = api("GET", "/api/v1/users/me/preferences/terminal")["value"] or {}
    check("and changing it back is saved too", saved.get("predictiveEcho") == "auto", str(saved)[:200])

    errors = [l for l in logs if l.startswith(("error", "pageerror"))]
    check("no console errors", not errors, "\n".join(errors[:5]))
    browser.close()

print("screenshots:", OUT)
print("ALL PHONE CHECKS PASSED" if not failures else f"FAILED: {failures}")
sys.exit(1 if failures else 0)
