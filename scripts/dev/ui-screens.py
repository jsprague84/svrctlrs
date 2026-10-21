#!/usr/bin/env python3
"""Screenshots of every screen at phone and desktop size, dark and light, for UI review and
for the README/USER-GUIDE images. Seeds a throwaway server (run-ui-screens.sh starts one) with
servers, tags, credentials, quick commands and a second user.

E2E_SSH_HOST etc. are optional: with them, one server ("lab-01") is real and the terminal shots
show a live shell; without, the terminal shots show the empty state. The real address never
reaches a picture: each shot first rewrites it on the page to 192.0.2.10 (a documentation
address), since the images are published.

    E2E_OUT=/tmp/shots python3 scripts/dev/ui-screens.py
"""
import base64, hmac, json, os, struct, sys, tempfile, time, urllib.request, urllib.error, http.cookiejar
from playwright.sync_api import sync_playwright

BASE = os.environ.get("E2E_URL", "http://127.0.0.1:18082")
def totp_now(secret):
    """The code an authenticator app shows for this base32 secret (RFC 6238, SHA-1, 6 digits)"""
    key = base64.b32decode(secret + "=" * (-len(secret) % 8))
    digest = hmac.new(key, struct.pack(">Q", int(time.time()) // 30), "sha1").digest()
    offset = digest[-1] & 15
    return "%06d" % ((struct.unpack(">I", digest[offset:offset + 4])[0] & 0x7FFFFFFF) % 1000000)


USER = os.environ.get("E2E_USER", "admin")
PASSWORD = os.environ.get("E2E_PASSWORD", "e2e-password-123456")
SSH_HOST = os.environ.get("E2E_SSH_HOST")
SSH_USER = os.environ.get("E2E_SSH_USER", "root")
SSH_KEY = os.path.expanduser(os.environ.get("E2E_SSH_KEY", "~/.ssh/id_ed25519"))
CHROMIUM = os.environ.get("CHROMIUM") or None
OUT = os.environ.get("E2E_OUT") or tempfile.mkdtemp(prefix="svrctlrs-screens-")
ONLY = set(filter(None, os.environ.get("SCREENS", "").split(",")))
LIVE = "lab-01"           # the real server's name in the pictures
SHOWN_HOST = "192.0.2.10"  # RFC 5737, shown instead of SSH_HOST
os.makedirs(OUT, exist_ok=True)

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


# ── Seed ────────────────────────────────────────────────────────────────
api("POST", "/api/v1/auth/login", {"username": USER, "password": PASSWORD})
existing = api("GET", "/api/v1/servers")
if not (existing.get("servers") or existing.get("data")):
    pw = ident(api("POST", "/api/v1/credentials", {"name": "ops password", "credential_type": "password",
        "value": "not-a-real-password", "username": "ops", "description": "Fallback login for the lab"}))
    key_cred = None
    if SSH_HOST and os.path.exists(SSH_KEY):
        key_cred = ident(api("POST", "/api/v1/credentials", {"name": "laptop ed25519", "credential_type": "ssh_key",
            "value": open(SSH_KEY).read(), "username": SSH_USER}))
    api("POST", "/api/v1/credentials/generate-key", {"name": "svrctlrs deploy key"})
    api("POST", "/api/v1/credentials/generate-key", {"name": "spare key"})
    servers = [
        ("web-01", "10.0.10.21", ["prod", "web"], pw, None),
        ("web-02", "10.0.10.22", ["prod", "web"], pw, None),
        ("db-primary", "10.0.20.5", ["prod", "db"], pw, None),
        ("pi-garage", "192.168.1.40", ["home"], pw, "cd ~/sensors && ls"),
        ("nas", "192.168.1.10", ["home", "storage"], pw, None),
    ]
    if SSH_HOST and key_cred:
        servers.insert(0, (LIVE, SSH_HOST, ["lab"], key_cred, None))
    for name, host, tags, cred, startup in servers:
        body = {"name": name, "hostname": host, "port": 22, "username": SSH_USER if name == LIVE else "ops",
                "credential_id": cred, "tags": tags, "record_sessions": name == LIVE}
        if startup:
            body["startup_command"] = startup
        api("POST", "/api/v1/servers", body)
    for name, cmd in [("Disk usage", "df -h"), ("Follow syslog", "journalctl -f"), ("Top memory", "ps aux --sort=-%mem | head")]:
        try:
            api("POST", "/api/v1/quick-commands", {"name": name, "command": cmd})
        except urllib.error.HTTPError:
            pass
    try:
        api("POST", "/api/v1/users", {"username": "alex", "password": "a-long-enough-password"})
    except urllib.error.HTTPError:
        pass

# ── Shots ───────────────────────────────────────────────────────────────
VIEWPORTS = {
    "phone": dict(device="Pixel 7"),
    "desktop": dict(viewport={"width": 1440, "height": 900}, device_scale_factor=1),
}
shots = []
failures = []
tiny = {}
# Visible controls smaller than 40 px either way on a phone; a ::before hit margin counts.
# Skipped: xterm internals, backdrops, and the extra-keys row (24 keys across a phone screen
# are 38x36 by design; taller keys would cost terminal rows).
SMALL_TARGETS = """
(() => {
  const out = [];
  for (const el of document.querySelectorAll('button, a[href], [role=menuitem], select, input:not([type=hidden])')) {
    if (el.closest('.xterm') || el.closest('[aria-label="Extra keys"]')) continue;
    if (el.tabIndex < 0 && el.getAttribute('aria-label')?.startsWith('Close')) continue;
    if (!el.checkVisibility({ visibilityProperty: true, opacityProperty: true })) continue;
    const r = el.getBoundingClientRect();
    if (!r.width || !r.height || r.bottom < 0 || r.top > innerHeight) continue;
    const st = getComputedStyle(el, '::before');
    const grow = st.content !== 'none' && st.position === 'absolute' ? -2 * parseFloat(st.top || '0') : 0;
    const w = r.width + grow, h = r.height + grow;
    if (Math.min(w, h) < 40) out.push(`${(el.getAttribute('aria-label') || el.textContent || el.tagName).trim().slice(0, 24)} ${Math.round(w)}x${Math.round(h)}`);
  }
  return out;
})()
"""

# Every text node and <option> that shows the real address shows SHOWN_HOST instead
MASK_HOST = """
(([real, shown]) => {
  const walk = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n; (n = walk.nextNode()); )
    if (n.nodeValue.includes(real)) n.nodeValue = n.nodeValue.replaceAll(real, shown);
})
"""


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (f" — {detail}" if detail and not ok else ""), flush=True)
    if not ok:
        failures.append(name)


def want(name):
    return not ONLY or any(o in name for o in ONLY)


with sync_playwright() as p:
    # DOM renderer: xterm's WebGL renderer under Playwright's device-scale emulation draws the
    # phone's text at 2.6x (an emulation artefact; real phones are fine)
    browser = p.chromium.launch(executable_path=CHROMIUM, args=["--disable-webgl", "--disable-3d-apis"])
    for vp_name, vp in VIEWPORTS.items():
        for theme in ("dark", "light"):
            opts = dict(p.devices[vp["device"]]) if "device" in vp else {k: v for k, v in vp.items()}
            ctx = browser.new_context(**opts, base_url=BASE)
            ctx.add_init_script(f"localStorage.setItem('svrctlrs-theme', '{theme}');"
                                "localStorage.setItem('svrctlrs-gesture-hint-seen', '1');")
            r = ctx.request.post(BASE + "/api/v1/auth/login", data={"username": USER, "password": PASSWORD},
                headers={"Origin": BASE})
            assert r.ok, r.status
            page = ctx.new_page()

            def shot(name, full=False):
                fname = f"{vp_name}-{theme}-{name}.png"
                if not want(fname):
                    return
                page.wait_for_timeout(400)
                if SSH_HOST:
                    page.evaluate(MASK_HOST, [SSH_HOST, SHOWN_HOST])
                    leaked = page.evaluate("h => document.body.innerText.includes(h)", SSH_HOST)
                    check(f"{fname} does not show the real address", not leaked)
                page.screenshot(path=os.path.join(OUT, fname), full_page=full)
                # Horizontal overflow is a bug at any width
                overflow = page.evaluate("document.documentElement.scrollWidth - document.documentElement.clientWidth")
                shots.append((fname, overflow))
                if vp_name == "phone":
                    small = page.evaluate(SMALL_TARGETS)
                    if small:
                        tiny[fname] = small

            def go(path):
                page.goto(path)
                page.wait_for_load_state("networkidle")
                page.wait_for_timeout(500)

            go("/")
            shot("terminal-empty")
            go("/servers")
            shot("servers", full=True)
            add = page.get_by_role("button", name="Add Server")
            if add.count():
                add.first.click()
                shot("servers-add")
                if vp_name == "desktop" and theme == "dark":
                    # Tab stays inside the dialog; Escape closes it and focus goes back
                    for _ in range(30):
                        page.keyboard.press("Tab")
                    inside = page.evaluate("!!document.activeElement.closest('[role=dialog]')")
                    check("Tab stays inside the modal", inside)
                    page.keyboard.press("Escape")
                    page.wait_for_timeout(300)
                    check("Escape closes the modal", page.get_by_role("dialog").count() == 0)
                else:
                    page.keyboard.press("Escape")
            if vp_name == "desktop" and theme == "dark":
                # The key picker keeps whatever is picked (it used to snap back to the first key)
                page.get_by_role("button", name="Authorize an SSH key on this server").first.click()
                picker = page.get_by_role("dialog").get_by_label("Key to authorize")
                picker.wait_for(timeout=5000)
                options = picker.evaluate("s => [...s.options].map(o => o.value).filter(Boolean)")
                if len(options) > 1:
                    picker.select_option(options[1])
                    page.wait_for_timeout(400)
                    check("authorize-key picker keeps the second key", picker.input_value() == options[1],
                          f"{picker.input_value()} != {options[1]}")
                else:
                    check("authorize-key picker has two keys to pick from", False, str(options))
                page.keyboard.press("Escape")
            if vp_name == "phone":
                menu = page.get_by_role("button", name="Actions for db-primary")
                if menu.count():
                    menu.first.click()
                    shot("servers-menu")
                    page.keyboard.press("Escape")
                    page.wait_for_timeout(200)
                    check("Escape closes the row menu", page.get_by_role("menu").count() == 0)
            # Diagnose: the live server passes every step; without one, an unreachable lab
            # address fails at TCP with a fix
            target = LIVE if SSH_HOST else "pi-garage"
            go("/servers")
            if vp_name == "phone":
                page.get_by_role("button", name=f"Actions for {target}").first.click()
                page.get_by_role("menuitem", name="Diagnose").click()
            else:
                page.locator("li", has_text=target).get_by_role("button", name="Diagnose the connection").first.click()
            dialog = page.get_by_role("dialog", name=f"Diagnose {target}")
            dialog.wait_for(timeout=5000)
            dialog.get_by_text("Connecting step by step").wait_for(state="detached", timeout=25000)
            verdict = dialog.get_by_role("status").first.inner_text()
            if SSH_HOST:
                # A fresh database has not confirmed lab-01's key yet: that is the right verdict too
                check(f"diagnose passes on {target}, or stops at its unconfirmed key ({vp_name}-{theme})",
                      "is reachable" in verdict or "host key is not trusted" in verdict, verdict)
            else:
                check(f"diagnose names the failure ({vp_name}-{theme})", "cannot be reached" in verdict, verdict)
            shot("diagnose")
            page.keyboard.press("Escape")
            go("/credentials")
            shot("credentials", full=True)
            for tab in ["general", "notifications", "commands", "access", "audit", "about"]:
                go(f"/settings?tab={tab}")
                shot(f"settings-{tab}", full=True)
            # An agent token: created in the dialog, shown once, works outside the browser,
            # revoked in one tap
            agent = f"agent-{vp_name}-{theme}"
            go("/settings?tab=access")
            page.get_by_role("button", name="New token").click()
            dialog = page.get_by_role("dialog", name="New token")
            dialog.get_by_label("Name").fill(agent)
            dialog.get_by_role("radio", name="Full admin").click()
            dialog.get_by_role("radio", name="Never").click()
            check("full admin + never warns", dialog.get_by_text("never expires opens a shell").count() == 1)
            shot("token-new")
            dialog.get_by_role("button", name="Create token").click()
            created = page.get_by_role("dialog", name="Token created")
            created.wait_for(timeout=5000)
            secret = created.get_by_label("The new token").input_value()
            check("the new token is shown once, whole", secret.startswith("svr_") and len(secret) == 44, secret[:6])
            shot("token-created")
            created.get_by_role("button", name="Done").click()

            def token_status(path="/api/v1/servers"):
                req = urllib.request.Request(BASE + path, headers={"Authorization": f"Bearer {secret}"})
                try:
                    with urllib.request.urlopen(req) as r:
                        return r.status
                except urllib.error.HTTPError as e:
                    return e.code

            check("the token reads the server list", token_status() == 200)
            check("the token cannot list tokens", token_status("/api/v1/tokens") == 403)
            check("the list shows no secret", secret not in page.content())
            page.get_by_role("button", name=f"Revoke token {agent}").click()
            page.get_by_role("button", name=f"Remove token {agent}").wait_for(timeout=5000)
            check("a revoked token is refused", token_status() == 401)
            shot("settings-access-tokens", full=True)
            # Two-factor sign-in: on through the dialog (password, QR, a code), ten recovery
            # codes shown once, then off again so the next context's password login still works
            signin = page.get_by_test_id("sign-in-totp")
            signin.get_by_role("button", name="Turn on").click()
            dialog = page.get_by_role("dialog", name="Turn on two-factor sign-in")
            dialog.get_by_label("Current password").fill(PASSWORD)
            page.get_by_role("button", name="Continue").click()
            qr = dialog.get_by_role("img", name="QR code for the authenticator app")
            qr.wait_for(timeout=5000)
            page.wait_for_timeout(300)
            check("the QR code renders", qr.evaluate("i => i.complete && i.naturalWidth > 0"))
            key = dialog.locator("code").inner_text().strip()
            shot("totp-scan")
            dialog.get_by_label("Code from the app").fill(totp_now(key))
            page.get_by_role("button", name="Turn on", exact=True).last.click()
            codes = page.get_by_test_id("recovery-codes")
            codes.wait_for(timeout=5000)
            check("ten recovery codes are shown", codes.locator("li").count() == 10)
            shot("totp-codes")
            page.get_by_role("button", name="Done").click()
            page.wait_for_timeout(500)
            check("two-factor shows on", signin.get_by_text("on", exact=True).count() == 1)
            r = ctx.request.post(BASE + "/api/v1/auth/login", data={"username": USER, "password": PASSWORD},
                headers={"Origin": BASE})
            check("a password alone is no longer enough", r.status == 401 and r.json().get("code") == "TOTP_REQUIRED",
                  str(r.status))
            shot("settings-access-signin", full=True)
            signin.get_by_role("button", name="Turn off").click()
            dialog = page.get_by_role("dialog", name="Turn off two-factor sign-in")
            dialog.get_by_label("Current password").fill(PASSWORD)
            page.get_by_role("button", name="Turn off").last.click()
            try:
                signin.get_by_role("button", name="Turn on").wait_for(timeout=5000)
                off = True
            except Exception:
                off = False
            check("two-factor is off again", off)
            # Approval: a token set to ask first tries to delete a server; the link opens the
            # request, and Deny leaves the server alone
            asker = api("POST", "/api/v1/tokens", {"name": f"asker-{vp_name}-{theme}", "preset": "servers_keys",
                                                   "expires_in_days": 30, "require_approval": True})["secret"]
            nas = next(x for x in api("GET", "/api/v1/servers")["servers"] if x["name"] == "nas")
            req = urllib.request.Request(BASE + f"/api/v1/servers/{nas['id']}", method="DELETE",
                headers={"Authorization": f"Bearer {asker}"})
            with urllib.request.urlopen(req) as r:
                held = json.loads(r.read())
                check("a gated delete is held (202)", r.status == 202, str(r.status))
            go(f"/approvals?id={held['approval']['id']}")
            check("the link opens the request",
                  page.get_by_role("heading", name="Delete server nas").count() == 1)
            shot("approval")
            page.get_by_role("region", name="Requested action").get_by_role("button", name="Deny").click()
            page.wait_for_timeout(800)
            check("denied, and the server is still there",
                  any(x["name"] == "nas" for x in api("GET", "/api/v1/servers")["servers"]))
            shot("approvals", full=True)
            if SSH_HOST and theme == "dark":
                # A live shell on the real target, for the README and USER-GUIDE pictures
                go("/")
                page.locator(f"text={LIVE} >> visible=true").first.click()
                trust = page.get_by_role("button", name="Trust and connect")
                try:
                    trust.wait_for(timeout=8000)
                    trust.click()
                except Exception:
                    pass  # already trusted in an earlier context
                page.wait_for_timeout(4000)
                page.keyboard.type("clear; uname -sr; df -h --output=target,size,used,avail,pcent / /boot 2>/dev/null; ls -C -w $COLUMNS --color=always /etc | head -14")
                page.keyboard.press("Enter")
                page.wait_for_timeout(2500)
                shot("terminal-live")
                if vp_name == "desktop":
                    picked = page.locator("select").first.evaluate("s => s.options[s.selectedIndex].text")
                    check("toolbar picker shows the active tab's server", picked.startswith(LIVE), picked)
                    page.get_by_label("Files on this server").first.click()
                    page.wait_for_timeout(2500)
                    # The filesystem root, not someone's home directory, in published pictures
                    page.get_by_role("dialog", name="Files").locator("nav button").first.click()
                    page.wait_for_timeout(2000)
                    shot("terminal-files")
            # The live shells above are recorded (lab-01 records sessions)
            go("/recordings")
            shot("recordings", full=True)
            play = page.get_by_role("button", name="Play the recording").first
            if play.count():
                play.click()
                page.wait_for_timeout(3000)
                shot("recording-player")
                if vp_name == "desktop" and theme == "dark":
                    shown = page.locator(".recording-frame .xterm-rows").inner_text()
                    check("the player shows the recorded shell", "$" in shown, shown[:80])
                page.keyboard.press("Escape")
                page.wait_for_timeout(300)
                check(f"Escape closes the player ({vp_name}-{theme})",
                      page.get_by_role("dialog", name="Recording of").count() == 0)
            if vp_name == "phone":
                go("/")
                menu = page.get_by_label("Open menu")
                if menu.count():
                    menu.click()
                    shot("sidebar")
                    page.keyboard.press("Escape")
                go("/")
                page.get_by_label("New terminal").click()
                shot("sheet-servers")
            if vp_name == "desktop":
                # Logout lands on the server's login page (the one that offers an identity
                # provider), not the SPA's password-only route, and does not bounce to a provider
                go("/")
                page.get_by_role("button", name="Logout").click()
                page.wait_for_url("**/auth/login?signed_out=1", timeout=5000)
                check(f"logout shows the server's login page ({theme})",
                      page.locator('form[action="/auth/login"]').count() == 1)
            ctx.close()
    # Login page, logged out
    for vp_name, vp in VIEWPORTS.items():
        opts = dict(p.devices[vp["device"]]) if "device" in vp else {k: v for k, v in vp.items()}
        ctx = browser.new_context(**opts, base_url=BASE)
        page = ctx.new_page()
        page.goto("/auth/login")
        page.wait_for_timeout(800)
        fname = f"{vp_name}-dark-login.png"
        if want(fname):
            page.screenshot(path=os.path.join(OUT, fname))
            shots.append((fname, 0))
        ctx.close()
    browser.close()

for name, overflow in shots:
    print(("H-SCROLL " if overflow > 0 else "ok       ") + name + (f" (+{overflow}px)" if overflow > 0 else ""))
for name, items in tiny.items():
    print(f"small targets {name}: {len(items)}: " + "; ".join(items[:12]))
print("screens:", OUT)
if tiny:
    failures.append(f"tap targets under 40 px on {len(tiny)} phone screens")
overflowing = [n for n, o in shots if o > 0]
if overflowing:
    failures.append(f"horizontal scroll on {len(overflowing)} screens")
print("FAILED: " + ", ".join(failures) if failures else "ALL UI CHECKS PASSED")
sys.exit(1 if failures else 0)
