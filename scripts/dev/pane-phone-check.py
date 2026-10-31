#!/usr/bin/env python3
"""Phone check of the terminal pane: a Pixel 7 emulation in Chromium with real touch events
(CDP Input.dispatchTouchEvent) against a running server and a real SSH target.

`scripts/dev/run-phone-check.sh` starts a throwaway server and runs this. It needs Python with
`playwright` and a Chromium (CHROMIUM=/path, else Playwright's own). WebGL is disabled so xterm
uses its DOM renderer and the screen's text can be read back.

Covers: first-contact host-key dialog → trust → shell, typing, tap-to-move-cursor, long-press
copy, a second tab and a swipe between tabs (which brings up no keyboard), that no hidden pane
resizes its shell, CTL from the extra-keys row (Ctrl+C interrupts `sleep`), the Paste key (two
lines as one bracketed paste that waits for Enter; a refused clipboard shows a toast and sends
nothing), a vertical drag in a program that reads the mouse (wheel notches, no arrows, the
keyboard let go until a tap), predictive echo over a slowed link (typed text drawn ahead of
the echo and cleared after it, nothing drawn at a `read -s` prompt), reload → resume with the
screen intact, the home-screen shortcut (manifest lists the server,
`/?connect=<id>` shows its tab), terminal preferences following the account (a change on
the phone is on the server and in a fresh desktop browser), and quick-command variables
(`{{name}}` from the server, `{{what:hello}}` asked for in a dialog with a live preview),
and the file browser's viewer: a tap opens a file instead of downloading it, text and an image
shown, a small CRLF file edited and saved with its line endings, a save refused after the file
changed behind the editor, and "Edit in terminal" opening vi on it in a new tab.
Screenshots land in E2E_OUT (default: a temp dir).
"""
import json, os, struct, sys, tempfile, time, urllib.parse, urllib.request, http.cookiejar, zlib
from playwright.sync_api import sync_playwright

# The app follows the OS theme on a first visit: THEME=light runs the whole check in light mode
THEME = os.environ.get("THEME", "dark")
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
srv = api("POST", "/api/v1/servers", {"name": NAME, "hostname": SSH_HOST, "port": 22,
    "username": SSH_USER, "credential_id": cid})
SID = srv.get("id") or srv["data"]["id"]


def file_uri(path, view=False):
    return (f"/api/v1/servers/{SID}/files/content?path=" + urllib.parse.quote(path, safe="")
            + ("&view=true" if view else ""))


def put_file(path, data):
    req = urllib.request.Request(BASE + file_uri(path), data=data, method="PUT",
        headers={"Content-Type": "application/octet-stream", "Origin": BASE})
    with opener.open(req) as r:
        return json.loads(r.read())


def get_file(path):
    req = urllib.request.Request(BASE + file_uri(path), headers={"Origin": BASE})
    with opener.open(req) as r:
        return r.read()


def tiny_png():
    """A 2×2 red PNG, built by hand"""
    def chunk(kind, data):
        c = kind + data
        return struct.pack(">I", len(data)) + c + struct.pack(">I", zlib.crc32(c))
    raw = b"".join(b"\x00" + b"\xff\x00\x00" * 2 for _ in range(2))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))

failures = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""), flush=True)
    if not ok:
        failures.append(name)


with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=CHROMIUM, args=["--disable-webgl", "--disable-3d-apis"])
    ctx = browser.new_context(**p.devices["Pixel 7"], base_url=BASE, color_scheme=THEME)
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
    # The keyboard was down (no soft keyboard here): the swipe was for looking and brings none up
    check("a swipe brings up no keyboard",
        not page.evaluate("!!document.activeElement?.closest('.xterm')"))

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

    # The Paste key types the clipboard like a native paste: two lines go in as one bracketed
    # paste (bash asks for it), so neither runs before Enter; then both run.
    def paste_key():
        k = page.get_by_role("button", name="Paste", exact=True).first.bounding_box()
        tap(k["x"] + k["width"] / 2, k["y"] + k["height"] / 2)

    page.evaluate("navigator.clipboard.writeText('echo paste-$((6*7))\\necho paste-two')")
    before_paste = len(sent)
    paste_key()
    page.wait_for_timeout(1000)
    inputs = [json.loads(f).get("data", "") for f in sent[before_paste:] if '"input"' in f]
    check("Paste sends the clipboard as one bracketed paste",
        "\x1b[200~echo paste-$((6*7))\recho paste-two\x1b[201~" in inputs, repr(inputs[:3]))
    check("nothing pasted runs before Enter", "\npaste-42" not in screen(), screen()[-200:])
    check("the Paste key keeps the terminal focused",
        page.evaluate("!!document.activeElement?.closest('.xterm')"))
    page.keyboard.press("Enter")
    check("both pasted lines run on Enter", wait_screen("\npaste-42") and wait_screen("\npaste-two"),
        screen()[-300:])
    # The browser refuses: a toast says so and nothing is sent
    ctx.clear_permissions()
    before_paste = len(sent)
    paste_key()
    page.wait_for_timeout(1000)
    check("a refused clipboard says so", page.get_by_text("would not share the clipboard").count() > 0)
    check("and sends nothing", not [f for f in sent[before_paste:] if '"input"' in f], str(sent[before_paste:][:3]))
    ctx.grant_permissions(["clipboard-read", "clipboard-write"], origin=BASE)
    page.screenshot(path=f"{OUT}/paste.png")

    # A program that reads the mouse (herdr, tmux with mouse on) gets wheel notches from a
    # vertical drag, never arrow keys (herdr hands those to its shell); the drag lets go of the
    # keyboard so the whole screen is there to read, and a tap brings it back
    def drag(dy):
        b = shell_box()
        x, y0 = b["x"] + b["width"] / 2, b["y"] + b["height"] * (0.8 if dy < 0 else 0.2)
        touch("touchStart", x, y0)
        for i in range(1, 13):
            touch("touchMove", x, y0 + dy * i / 12)
            time.sleep(0.016)
        touch("touchEnd")
        page.wait_for_timeout(300)

    page.keyboard.type("printf '\\033[?1049h\\033[?1002h\\033[?1006h'; stty -echo -icanon; timeout 6 cat >/dev/null;"
                       " stty echo icanon; printf '\\033[?1006l\\033[?1002l\\033[?1049l'; echo mouse-$((2*3))-done")
    page.keyboard.press("Enter")
    page.wait_for_timeout(1200)
    before_wheel = len(sent)
    drag(-300)  # finger up: further down
    check("a scroll lets go of the keyboard", not page.evaluate("!!document.activeElement?.closest('.xterm')"))
    drag(300)
    wheel = "".join(json.loads(f).get("data", "") for f in sent[before_wheel:] if '"input"' in f)
    check("a drag sends wheel-down notches to a mouse program", "\x1b[<65;" in wheel, repr(wheel[:80]))
    check("and wheel-up notches back", "\x1b[<64;" in wheel, repr(wheel[:80]))
    check("and no arrow keys", not any(a in wheel for a in ("\x1b[A", "\x1b[B", "\x1bOA", "\x1bOB")),
        repr(wheel[:80]))
    box = shell_box()
    tap(box["x"] + box["width"] / 2, box["y"] + box["height"] * 0.85)
    page.wait_for_timeout(400)
    check("a tap brings the keyboard back", page.evaluate("!!document.activeElement?.closest('.xterm')"))
    check("the mouse program ends cleanly", wait_screen("mouse-6-done", 10), screen()[-200:])

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
    desk = browser.new_context(base_url=BASE, viewport={"width": 1280, "height": 800}, color_scheme=THEME)
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

    # Quick-command variables: {{name}} comes from the tab's server, {{what}} is asked for
    api("POST", "/api/v1/quick-commands", {"name": f"greet {RUN}",
        "command": "echo qc-{{name}}-{{what:hello}}"})
    page.get_by_role("button", name="Command palette").click()
    page.get_by_placeholder("Type a command or action...").fill(f"greet {RUN}")
    page.wait_for_timeout(300)
    page.keyboard.press("Enter")
    fill = page.get_by_role("dialog", name="Fill in the command")
    fill.wait_for(timeout=5000)
    preview = fill.get_by_test_id("snippet-preview")
    check("the dialog previews the command with the server's name and the default",
          preview.inner_text() == f"echo qc-{NAME}-hello", preview.inner_text())
    fill.get_by_label("what").fill("world")
    check("the preview follows the answer", preview.inner_text() == f"echo qc-{NAME}-world",
          preview.inner_text())
    page.screenshot(path=f"{OUT}/snippet-dialog.png")
    fill.get_by_role("button", name="Run").click()
    check("the filled-in command runs in the shell", wait_screen(f"\nqc-{NAME}-world"), screen()[-200:])
    check("no horizontal scroll with the dialog's text", page.evaluate(
        "document.documentElement.scrollWidth <= window.innerWidth"))

    # ── Files: a tap opens the viewer; downloads only on the Download button ──────────────
    downloads = []
    page.on("download", lambda d: downloads.append(d.suggested_filename))
    home = api("GET", f"/api/v1/servers/{SID}/files")["path"]
    folder = f"{home}/svrctlrs-phone-{RUN}"
    api("POST", f"/api/v1/servers/{SID}/files/mkdir", {"path": folder})
    notes = f"{folder}/notes.txt"
    put_file(notes, b"phone-notes-first\r\nsecond line\r\n")
    put_file(f"{folder}/pic.png", tiny_png())

    page.get_by_role("button", name="Command palette").click()
    page.get_by_placeholder("Type a command or action...").fill("Browse Files")
    page.wait_for_timeout(300)
    page.keyboard.press("Enter")
    page.get_by_title(f"Open svrctlrs-phone-{RUN}").first.wait_for(timeout=15000)
    page.get_by_title(f"Open svrctlrs-phone-{RUN}").first.tap()
    page.get_by_title("Open notes.txt").wait_for(timeout=15000)
    page.get_by_title("Open notes.txt").tap()
    viewer = page.get_by_role("dialog", name="notes.txt")
    viewer.wait_for(timeout=15000)
    page.wait_for_function("() => document.querySelector('pre')?.textContent.includes('phone-notes-first')", timeout=15000)
    page.wait_for_timeout(500)
    check("a tap on a file opens the viewer, not a download", not downloads, str(downloads))
    check("the viewer says what the file is", "CRLF" in viewer.inner_text(), viewer.inner_text()[-200:])
    check("the viewer fits the phone", page.evaluate("document.documentElement.scrollWidth <= window.innerWidth"))
    page.screenshot(path=f"{OUT}/viewer-text.png")

    viewer.get_by_role("button", name="Edit here").tap()
    editor = viewer.get_by_label("Contents of notes.txt")
    editor.wait_for(timeout=5000)
    editor.fill("phone-notes-first\nedited on the phone\n")
    page.screenshot(path=f"{OUT}/viewer-edit.png")
    viewer.get_by_role("button", name="Save").tap()
    page.wait_for_timeout(2500)
    check("a saved edit keeps the file's CRLF line endings",
          get_file(notes) == b"phone-notes-first\r\nedited on the phone\r\n", repr(get_file(notes)))

    # Changed behind the editor's back: the save is refused and nothing is overwritten
    viewer.get_by_role("button", name="Edit here").tap()
    editor.fill("mine\n")
    put_file(notes, b"changed-elsewhere\r\n")
    viewer.get_by_role("button", name="Save").tap()
    page.wait_for_timeout(2500)
    check("a save after the file changed is refused",
          "changed on the server" in viewer.inner_text() and get_file(notes) == b"changed-elsewhere\r\n",
          viewer.inner_text()[:300])
    page.screenshot(path=f"{OUT}/viewer-conflict.png")
    viewer.get_by_role("button", name="Reload the file").tap()
    page.get_by_role("dialog", name="Reload and lose your edits?").get_by_role("button", name="Reload").tap()
    page.wait_for_function("() => document.querySelector('pre')?.textContent.includes('changed-elsewhere')", timeout=15000)
    check("reload shows the file as it is now", True)

    viewer.get_by_role("button", name="Close notes.txt").tap()
    page.get_by_title("Open pic.png").tap()
    img = page.get_by_role("dialog", name="pic.png").get_by_role("img", name="pic.png")
    img.wait_for(timeout=15000)
    page.wait_for_timeout(500)
    check("an image shows in the viewer", page.evaluate(
        "() => document.querySelector('img[alt=\"pic.png\"]')?.naturalWidth === 2"))
    page.screenshot(path=f"{OUT}/viewer-image.png")
    page.get_by_role("dialog", name="pic.png").get_by_role("button", name="Download pic.png").tap()
    page.wait_for_timeout(1500)
    check("Download in the viewer saves the file", downloads == ["pic.png"], str(downloads))
    page.get_by_role("dialog", name="pic.png").get_by_role("button", name="Close pic.png").tap()

    # Edit in terminal: a new tab on the server runs its editor (vi here) on the file
    tabs_before = len(page.evaluate("JSON.parse(sessionStorage.getItem('svrctlrs-terminal-tabs')).tabs"))
    page.get_by_title("Open notes.txt").tap()
    viewer.wait_for(timeout=15000)
    viewer.get_by_role("button", name="Edit in terminal").tap()
    page.wait_for_timeout(1000)
    saved_tabs = page.evaluate("JSON.parse(sessionStorage.getItem('svrctlrs-terminal-tabs'))")
    active = next(t for t in saved_tabs["tabs"] if t["id"] == saved_tabs["activeTabId"])
    check("Edit in terminal opens a new tab named after the file",
          len(saved_tabs["tabs"]) == tabs_before + 1 and active["label"] == "notes.txt", str(active))
    check("the editor shows the file in the new tab", wait_screen("changed-elsewhere", 20), screen()[-300:])
    page.screenshot(path=f"{OUT}/edit-in-terminal.png")
    page.keyboard.type(":q!")
    page.keyboard.press("Enter")
    page.keyboard.type("echo after-editor-$((40+2))")
    page.keyboard.press("Enter")
    check("quitting the editor leaves the shell", wait_screen("\nafter-editor-42"), screen()[-300:])
    shell_sent = [json.loads(f) for f in sent if '"type":"shell"' in f.replace(" ", "")]
    check("the edit went with that one shell request only",
          sum(1 for m in shell_sent if m.get("edit") == notes) == 1, str(shell_sent)[-300:])
    for path in (notes, f"{folder}/pic.png", folder):
        api("DELETE", f"/api/v1/servers/{SID}/files?path=" + urllib.parse.quote(path, safe=""))

    # The refused save's 412 is logged by the browser as a failed load; it was meant
    errors = [l for l in logs if l.startswith(("error", "pageerror")) and "status of 412" not in l]
    check("no console errors", not errors, "\n".join(errors[:5]))
    browser.close()

print("screenshots:", OUT)
print("ALL PHONE CHECKS PASSED" if not failures else f"FAILED: {failures}")
sys.exit(1 if failures else 0)
