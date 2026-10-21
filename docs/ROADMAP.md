# Roadmap

**Updated 2026-10-21.** The first part is the live plan: where the app stands against similar
projects, what comes next and in which order, what is parked, and what is deliberately out of
scope. The second part is the history of the 2026-10 hardening passes, kept because each row
names the evidence for a fix.

## Where it stands (review of 2026-10-18)

**3.1.0 released 2026-10-18**: host-key confirmation, keepalives and jump hosts, tags and
`ssh_config` import, startup command, file browser, typed errors, migration baseline (Next
items 1–6 below, struck through). **3.2.0 released 2026-10-19**: the pane split, the UI pass,
the in-process SSH test server, session recording, predictive echo and preferences per
account (items 7–12). **3.3.0 released 2026-10-19**: quick-command variables, `svrctl
reset-password`, `linux/arm64` images (items 13–14), and agent access: tokens with presets and
approvals for the deletes that cannot be undone (items 15–16).
**3.4.0 released 2026-10-19**: the OpenAPI document and typed responses, diagnose, exec,
onboard and rotate-key, and `svrctl` as the agent CLI with its skill and a Hermes eval
(items 17–19): agent access is done. **After 3.4.0** (2026-10-19): the password login closed
to the internet (item 20), sign-in with OIDC and TOTP in the binary (item 21), the desktop app signing in through the
browser (item 22), shells kept across a restart (item 23), backups inside the app (item
24) and a file browser that opens files before it downloads them (item 25); item 26 is next.

**Niche.** SvrCtlRS is the *small* one: a self-hosted SSH terminal manager for one person's
servers, phone-first, with shells that survive the phone. One Rust binary, one SQLite file, an
audit log, encrypted credentials, host-key pinning, an SFTP browser, and nothing else. It is
not a server-management panel and not a bastion.

| Project | What it is | Stack · licence | Has, that we lack | We have, that it lacks or does differently |
|---|---|---|---|---|
| [Termix](https://github.com/Termix-SSH/Termix) | Plugin-based "self-hosted Termius": SSH, RDP/VNC, files, tunnels, Docker, metrics, fleets, automations, SSO | TypeScript, Node, Electron · Apache-2.0 | Host folders/tags, import/export, sharing with roles, passkeys/TOTP/SSO, tunnels, snippets across hosts, session recording and live sharing, 30 languages, native mobile apps | One binary and one file to run; a deliberately small surface; server-owned sessions with replay |
| [Nexterm](https://github.com/gnmyt/Nexterm) | All-in-one panel: SSH/SFTP/RDP/VNC, snippets, organisations, Docker and Proxmox management, recordings, AI assistant | Node (server + engine), MIT | 2FA, OIDC, organisations, recordings, Proxmox/Docker apps | Same as above; no engine sidecar; audit log |
| [Sshwifty](https://github.com/nirui/sshwifty) | Stateless web SSH/Telnet client, presets, maintenance-only | Go · AGPL-3.0 | Fingerprint confirmation page on first contact; works with zero server state | Saved servers and encrypted credentials, persistent sessions, files, audit |
| [Warpgate](https://github.com/warp-tech/warpgate) | Clientless bastion/PAM: SSH, HTTPS, databases, RDP; recording and replay, TOTP/OIDC, tickets, per-user targets | Rust, russh, Svelte 5 · Apache-2.0 | Session recording and replay, brute-force lockout that survives restarts, per-user target scoping, OpenAPI-generated SDKs | We are a client with a terminal UI, not a proxy for native clients; phone ergonomics |
| [XPipe](https://github.com/xpipe-io/xpipe) | Desktop connection hub over your local ssh/docker/kubectl; file browser with local editors, scripts, git-synced vault | Java · open-core | `~/.ssh/config` import, scripts in the remote PATH, tunnels with one click | Runs on a server, so the phone works; nothing installed on the client |
| [sshx](https://github.com/ekzhang/sshx) | Collaborative shared terminal with predictive local echo | Rust, Svelte · MIT | Predictive echo for high-latency links | Private, saved servers; not a sharing tool |

**What the comparison says.** Every peer that is a *terminal manager* has these and we do
not: host grouping, `ssh_config` import, jump hosts, SSH keepalive, session recording, and a
first-contact fingerprint prompt. None of those grow the product's purpose; they are the
expected shape of an SSH client. Everything else the peers add (remote desktop, container and
hypervisor management, fleets, automations, metrics, plugins, AI) is the panel business we left
in 2.x and should not return. Two lessons from Warpgate's stack, which is ours: record sessions
per user and scope their playback to that user (its 2026 CVE leaked live recordings between
users), and keep the login limiter's state in the database so a restart does not reset it.

## Next (in order)

1. ~~**Migration squash and schema hygiene.**~~ Done 2026-10-18: `100_baseline.sql` for
   fresh installs; the legacy chain (`000`–`024`, frozen) finishes and is marked as baseline
   for existing databases; `is_local`, the OS-fact columns, `servers.metadata` and
   `terminal_profiles.quick_commands` left the model (ARCHITECTURE.md, "Data model").
2. ~~**First-contact host-key confirmation.**~~ Done 2026-10-18: the first key is recorded
   unconfirmed; shell, test, SFTP and authorize-key refuse with the fingerprint until the user
   trusts it (`POST /servers/{id}/host-keys/trust`); `SECURITY.md` "SSH host keys".
3. ~~**SSH keepalive and jump hosts.**~~ Done 2026-10-18: keepalives on every connection
   (`SSH_KEEPALIVE_SECS`, default 15 s, three unanswered close it); `servers.via_server_id`
   tunnels the target through the jump host's connection (`direct-tcpip`, up to four hops,
   loops refused, host keys confirmed on both). E2E covers shell and SFTP through a hop.
4. ~~**Server organisation and import.**~~ Done 2026-10-18: `servers.tags` (JSON array,
   normalised) with filter chips on the Servers page, the sidebar and the phone sheet, and
   palette/search matching; `POST /servers/import` parses `ssh_config` `Host` blocks
   (`server/src/ssh_config.rs`, tested) with `ProxyJump` → jump host.
5. ~~**Per-server startup command.**~~ Done 2026-10-18: `servers.startup_command`, one line
   typed as the shell's first input (`cd dir && …` covers the directory, so no second column).
6. ~~**Terminal polish.**~~ Done 2026-10-18: OSC title → tab name, bell (flash, vibration,
   tab marker), grapheme-cluster widths, `COLORTERM` env request, zlib compression offered.
7. ~~**Split `TerminalPane.svelte`.**~~ Done 2026-10-18: the WebSocket session (connect,
   attach, replay, ping/pong watchdog, reconnect, host-key challenge, persistence) is
   `ui/src/lib/terminal/ptySession.ts` with 35 Vitest tests; the pane keeps xterm, gestures
   and overlays (1,232 → 800 lines). The phone check (`scripts/dev/pane-phone-check.py`,
   Pixel 7 emulation with CDP touch against hermes) passed long-press copy, swipe between
   tabs, tap-to-move-cursor and CTL from the extra keys, and found that hidden panes resized
   their shell to 10×3 (fixed). Recording markers, hand-off and predictive echo land in that
   module.
8. ~~**UI pass: polish and consistency.**~~ Done 2026-10-19: phone tap targets of 44 px
   everywhere (checked by `scripts/dev/ui-screens.py`, which also fails on horizontal scroll),
   the Servers row as Connect + a ⋯ menu on narrow screens, Settings tabs as a phone grid, one
   `StateBlock` for loading/empty/error, an `overlay` attachment giving every dialog, sheet and
   panel Escape, focus return and (when modal) a focus trap, toasts above the extra-keys row,
   the running revision on About, light/dark text colours at WCAG AA (tested from `app.css`),
   home-screen shortcuts to recent servers (server-built manifest, `/?connect=<id>`), an empty
   terminal that lists servers, and screenshots in the README and USER-GUIDE. Found on the way:
   the credentials list called every SSH key a file path, and the toolbar picker ignored the
   active tab (both fixed).
9. ~~**In-process SSH server for tests.**~~ Done 2026-10-19: `server/src/test_support/` runs
   russh's server side on a loopback port; `ssh.rs` tests first-contact refusal and trust, a
   changed key (refused before the password is sent), password and key logins, and a jump
   host through `direct-tcpip` with a loop; `terminal_pty.rs` tests the PTY driver (split into
   `open_shell` + `spawn_pty_driver`) for PTY/env/startup command, input, resize, exit
   status, close and a shell through a jump host. CI runs them; disabling the host-key check
   makes three fail. The e2e against hermes stays for OpenSSH, SFTP and the WebSocket.
10. ~~**Session recording.**~~ Done 2026-10-19: `servers.record_sessions` (opt-in); a recorder
   follows the session like a socket (`sessions.rs` `Follower`) and writes asciicast v2
   (output and resizes, never input) to `RECORDINGS_DIR/<user>/`, encrypted per chunk with
   the credential key; owner-only API (404 for anyone else, tested per route and in the e2e),
   `recording.retention_days` with an hourly sweep, a 100 MB cap, download as `.cast`, and an
   in-app player (`/recordings`); REC mark on recorded panes. Backpressure measured first
   (`scripts/dev/run-e2e.sh backpressure`): the send side does not grow (a client that stops
   reading during a 200 MiB burst costs the server about 15 MB; the PTY driver never waits),
   so no flow control; a client that fell behind the ring was left blank and is now redrawn.
11. ~~**Predictive local echo**~~ Done 2026-10-19: `ui/src/lib/terminal/localEcho.ts`,
    driven by `ptySession.ts`; predictions are confirmed against the screen (cell holds the
    character and the cursor has passed it), drawn only in an epoch the shell has echoed in
    (non-text keys, unexplained output, mismatches and a 2 s timeout start a new one), never in
    the alternate screen or at a password prompt, and in `auto` only above 30 ms of measured
    echo latency. Drawn as xterm decorations, never written into the terminal. Preference
    Predictive Echo (on slow links / always / off). 33 Vitest tests; the phone check slows the
    link and types at a `read -s` prompt. The one gap (echo turned off silently after
    type-ahead) is in `SECURITY.md`.
12. ~~**Terminal preferences per account.**~~ Done 2026-10-19: a per-user table
    (`106_user_settings.sql`; the `settings` table is global) behind
    `GET/PUT /users/me/preferences/terminal`; localStorage stays as the start-up and offline
    copy. `ui/src/lib/terminal/prefsSync.ts` (Vitest) debounces saves, never lets the server
    overwrite an unsaved change and uploads a browser's own preferences to an empty account;
    the phone check changes one on the phone and finds it in a fresh desktop browser.
13. ~~**Quick-command variables**~~ Done 2026-10-19: `{{host}}`, `{{user}}`, `{{port}}`,
    `{{name}}` from the tab's server; any other `{{word}}` (`{{word:default}}`) asked for in a
    dialog with a live preview (`ui/src/lib/utils/snippet.ts`, Vitest; the phone check runs
    one). Only plain identifiers are variables and Go template keywords are reserved, so
    `docker --format '{{.Names}}'` and `{{range}}…{{end}}` stay as written.
14. **Release maturity.** Done 2026-10-19 except the last bullet:
   ~~`linux/arm64` image~~ (cross-compiled on the amd64 runner, no QEMU: every `RUN` on
   `$BUILDPLATFORM`, the runtime stage only copies; smoke-tested under qemu-user),
   ~~`svrctl reset-password`~~ (prompt, `--password-stdin` or `--generate`; ends that account's
   sessions; audited), ~~`CONTRIBUTING.md` and issue templates~~, ~~screenshots in the README~~
   (item 8), ~~a compose example without the dev-space edge~~ (`docker-compose.standalone.yml`).
   Found on the way: `ADMIN_PASSWORD` without `ADMIN_USERNAME` seeded nobody (fixed).
   **Still open:** the published OpenAPI document, now item 17.

### Agent access (decided 2026-10-19, owner)

Agents such as Hermes (the owner's always-on agent) and Claude Code administer svrctlrs from
outside: add and fix servers, keys and host keys, and diagnose why a server cannot be reached.
There is still no assistant inside the app. Research behind the shape (October 2026): for an
agent that has a shell, a CLI plus an Agent Skill (`SKILL.md`, the open standard Hermes, Claude
Code and Codex read) is cheaper and more reliable than a wide MCP tool list, and Hermes's own
docs call its skill+CLI GitHub integration more capable than the GitHub MCP server. MCP is
moving toward a few task-level tools or code against a typed API, and its 2026-07-28 revision
reworked the transport, so it waits until a client without a shell needs it. Agents already
reach servers over their own SSH; svrctlrs is not meant to be a second door to servers, but a
token may open terminals and files when that helps a diagnosis. Phone and desktop both get
compact screens for everything below.

15. ~~**Agent tokens.**~~ Done 2026-10-19: `svr_` + 40 alphanumerics, SHA-256 stored,
    shown once (`107_api_tokens.sql`, `server/src/api_tokens.rs`, `routes/api/tokens.rs`);
    presets *full admin* / *servers & keys* / *read-only* decided in one table by method and
    path before any handler, deny by default, and none reaches token or account management;
    lifetime 30/90/365 days or never; last use recorded; revoke in one tap, which also closes a
    terminal opened with the token. The auth layer resolves a token to its owner as
    `CurrentUser`, every audit row it causes carries `token_id`/`token_name` (a task-local the
    middleware sets, carried past the WebSocket upgrade), tickets carry the token, and tokens
    are refused in URLs. Settings → Access (Users renamed; phone grid stays 3×2): cards on the
    phone, a table on the desktop, a create dialog with a copy-once display. Tests: the preset
    table route by route, valid/expired/revoked/unknown/malformed/in-URL, 403 outside the
    preset, audit attribution, and through the real router (`server/src/app.rs`, extracted
    from `main.rs`) a full-admin token opening a shell on the loopback SSH server via a ticket,
    then revoked; the e2e does the same against OpenSSH on hermes and the UI sweep creates,
    uses and revokes one per viewport.
16. ~~**Approvals, rare and per token.**~~ Done 2026-10-19: `require_approval` per token
    (default on for new tokens, a toggle in the list); the auth layer holds exactly
    `DELETE /servers/{id}`, `DELETE /credentials/{id}` and `DELETE /servers/{id}/host-keys`
    (`server/src/approvals.rs`, `108_approvals.sql`) and answers `202` with the request and a
    link, re-asking returns the same one; links go out by push (Gotify/ntfy), webhook (n8n),
    Matrix (client-server API) or not at all, with a test button. The owner approves or denies
    on `/approvals` signed in; no preset reaches the decision routes. On approval the call is
    replayed through the API router as the token (still active, preset still covering it),
    once, before the hour is up; the result is stored and a waiting caller
    (`GET /approvals/{id}?wait=25`) hears it at once. Tests through the real router: held,
    self-approval refused, waiter woken, delete done and audited under the token, deny, a
    revoked token (410, nothing runs), webhook and Matrix delivery against a local listener.
    Found on the way: deleting a missing server answered 200 (now 404).
17. ~~**Typed responses and the OpenAPI document (utoipa).**~~ Done 2026-10-19: every
    handler returns a `svrctlrs-api` type (`api/`, `Serialize + ToSchema`; one module per
    commit) and is registered with `routes!()` on a utoipa `OpenApiRouter`, so routing and
    documentation are one line; `GET /api/v1/openapi.json` (every preset may read it) and
    `docs/openapi.json` with a drift test; 67 operations. Exact JSON compatibility is checked,
    not hoped for: `server/testdata/api-shapes.json` holds the keys and types of ~90 responses
    recorded from the old handlers through the real router, and stayed unchanged through every
    module. A test ties the token presets to the document. Found on the way: settings took
    `value_type: number`, which the table refuses (now `integer`); deleting a missing
    credential answered 200 (now 404); several audit rows had empty target names; the loopback
    SSH test server learned `exec`. `svrctl` adopts the types with item 19.
18. ~~**Task-level operations.**~~ Done 2026-10-19 (`routes/api/ops.rs`, outside the request
    timeout): `diagnose` (`diagnose.rs`: DNS → TCP, or the jump host's tunnel → handshake → host
    key → login, per hop, with the same handler and login as `ssh::connect`, a verdict and a fix
    per failed step, the server's status updated like a test), `exec` (one command, 1–600 s,
    1 MiB per stream, audited with the command, full-admin tokens only), `onboard` (create or
    resume, trust the host key only with the caller's fingerprint, check the login, generate a
    key and switch to it; stops with `next` when it needs you) and `rotate-key` (check, generate,
    install with the old key, prove, switch, remove the old public key). Mutations take
    `dry_run`; requests name stored credentials, never carry one. The Servers page has Diagnose
    (a step list with fixes, phone and desktop) and Rotate key. Tests against the loopback SSH
    server, which learned exec and applies the install/removal commands, so a rotated-away key
    is proved refused; the e2e runs diagnose, exec and onboard against OpenSSH on hermes.
19. ~~**`svrctl` as the agent CLI, a `SKILL.md`, and an eval.**~~ Done 2026-10-19: `svrctl`
    (`server/src/bin/svrctl/`) covers servers, credentials, host keys, diagnose (with
    `--credential` to test one without changing the server), exec, onboard, rotate-key,
    import, tag, approvals and audit; JSON when piped, errors with hints, stable exit codes
    (3 = a human must act), never a prompt, `--wait` for approvals, examples in `--help`,
    the server's own `svrctlrs-api` types; `server/tests/svrctl.rs` runs both binaries.
    `skills/svrctlrs/SKILL.md` (~950 tokens) is installed in Hermes default and local-qwen
    (`hermes skills install jsprague84/svrctlrs/svrctlrs`, after rewording two lines its
    scanner blocked) and linked for Claude Code. The eval (`scripts/eval/hermes/`): both
    profiles passed all five tasks (why can't I reach X + fix, onboard, rotate, import, tag)
    with every host key compared out of band; the transcripts led to `diagnose --credential`,
    a rotate-key hint that no longer invites deleting the old credential, and a "do only
    what was asked" rule in the skill. MCP stays under "Maybe later".

### After agent access (decided 2026-10-19, owner)

A look at a real week's use (phone and desktop) next to `SECURITY.md` found one hole and six
gaps; the owner set the order. Small and light stays the rule: nothing here adds a service
beside the binary, and what an identity provider does stays optional.

20. ~~**Password login only behind the passkey.**~~ Done 2026-10-19: the reference compose let
    all of `/api/` and `/ws/` skip the edge passkey for the native apps, so a password alone
    signed in from the internet (the login route answered from outside; SECURITY known
    weakness 13). Every browser path is behind the passkey now; `svrctlrs-token` passes only
    requests carrying `Bearer svr_…` (never the login) and `svrctlrs-health` the health check.
    The auth layer judges a token request by the token alone (a cookie beside it counted
    before), refuses two `Authorization` headers and a login carrying one (tests, e2e). The
    web app reloads when an API call is redirected to the edge's sign-in, and a refused
    terminal socket asks the API, so an expired passkey sign-in brings the prompt back.
21. ~~**Sign-in: OIDC and TOTP, both optional, both inside the binary.**~~ Done 2026-10-21:
    OpenID Connect with any provider (code flow with PKCE, `state`, `nonce`; the ID token
    checked in full with the RSA/ECDSA/Ed25519 code russh already builds; accounts linked by
    issuer and subject, never created or matched by e-mail), optional TOTP with ten recovery
    codes for password sign-in, the login limiter's failures in the database (migration 109),
    `PASSWORD_LOGIN=off` with break-glass by environment flag and `svrctl reset-password
    [--clear-totp]`, and Settings → Access → Sign-in. One new crate (`qrcode`, SVG only). The
    reference deployment signs in with Pocket ID in the app (its own client, `owner` group,
    `scripts/deploy/pocket-id-client.sh`) with password sign-in off, so tinyauth left
    SvrCtlRS's path and one headers-only router serves browsers, apps and agent tokens. Agent
    tokens work exactly as before (tested with password sign-in off; outside check
    2026-10-21).
22. ~~**Keep the desktop app**~~ Done 2026-10-21 (owner tested on the t490, NixOS + niri:
    sign-in through the browser with Pocket ID, vim `:split` and Ctrl+W j, still signed in after
    a restart). Sign-in through the system browser with svrctlrs as the authority (`/auth/app`,
    a one-time code on a `127.0.0.1` port, PKCE; works without a provider), a 30-day session in
    the OS keyring instead of `localStorage` (SECURITY known weakness 4), `?token=` removed,
    `ALLOWED_ORIGINS` no longer relaxing the browser cookie, Bearer logout, a tray that is
    optional, and a Nix flake for NixOS (the AppImage forces X11).
23. ~~**Shells that outlive a deploy and a pocket.**~~ Done 2026-10-21: a per-account keep
    time for detached shells (15 min / 1 h / 8 h in the Sessions panel, `PUT /sessions/keep`,
    capped by `SESSION_DETACH_TTL_MAX_SECS`, applied to shells already waiting) and an opt-in
    per-server **Keep shells in tmux** (migration 110): the shell runs in `tmux new-session -A
    -s svrctlrs-<id>` on the server, a restart or a dropped SSH connection releases the session
    instead of ending it, and the client's reconnect attaches a new connection to the same tmux
    session; kept shells end like any other and take their tmux session with them (no tmux: a
    plain shell and a note). Tested against a loopback server that acts like tmux and against
    real tmux on hermes.
24. ~~**Backups inside the app.**~~ Done 2026-10-21: the server copies its database every
    night (`backup.hour`, 03:00 UTC) into `BACKUP_DIR` (default `data/backups`, on the data
    volume, so the VM snapshot carries finished copies) and keeps `backup.keep` (14; 0 = off),
    one attempt per daily slot, made up minutes after a start that missed it, a failed night
    notified once (`server/src/backups.rs`, migration 111 `backup_runs`). The copy
    (`database/src/backup.rs`, shared with `svrctl backup`) is `VACUUM INTO` a hidden file then
    renamed, 0600 in a 0700 folder, with the sign-ins removed and vacuumed away (checked on the
    bytes); never the key. About shows the last copy and error with **Back up now**
    (`GET`/`POST /backups`, full admin to write); General sets keep and hour. `svrctl restore`
    checks integrity, schema and that every secret decrypts with the running key, stages the
    copy, and the next start swaps it in keeping `before-restore-<time>/`; `--check` only
    looks. Tested end to end with both binaries (backup, change, wrong key refused, restore,
    restart, data back, old session gone). Production's first copy ran two minutes after the
    deploy, and `restore --check` on it read all six credentials with the production key.
25. ~~**File browser: open before download, and edit small files.**~~ Done 2026-10-21: a tap
    opens the viewer (text up to 1 MiB, images up to 10 MiB through a `blob:` URL; Download
    is a button); text up to 256 KiB, UTF-8, one kind of line ending opens in a plain
    textarea that keeps CRLF and a BOM. No new routes: `GET /files/content` tags small files
    with their SHA-256 (`ETag`), `PUT` with `If-Match` re-reads and refuses a changed file
    (412), writing in place. **Edit in terminal** sends `edit` with the tab's first `shell`
    request; the server types `sh -c '${VISUAL:-${EDITOR:-vi}} "$1"' edit '<path>'`, quoted
    for sh and fish (tested through both). Desktop drag and drop uploads. The loopback SSH
    server now speaks SFTP, so every file route is tested in CI; the e2e opens vim on hermes
    through `edit`; the phone check edits, conflicts, views an image and edits in a terminal
    tab (it caught the CSP blocking `blob:` images).
26. **Signed-in devices.** The account's browser and app sessions (device, address, last use)
    with sign-out per device, so a lost phone does not need a password reset that ends every
    session.

## Maybe later

- **Port forwarding** (owner, 2026-10-18): a VPN into the home LAN covers the need; a web
  preview through the server would also need wildcard DNS/TLS at the edge. Revisit only if
  there is a recurring case a VPN cannot serve.
- **Roles, and passkeys built into the app** (owner, 2026-10-18): single-user today. Roles
  are required before the app is offered to other people. (OIDC and TOTP are item 21, the
  session list item 26.)
- 2FA on the *target* via keyboard-interactive auth if a server needs it.
- A **Paste** key on the phone's extra-keys row (pasting relies on the keyboard's clipboard
  button today), and fullscreen with Keyboard Lock so Ctrl+W reaches vim in a Chromium
  browser, if the desktop app (item 22) falls short.
- **MCP server for agent access** (owner, 2026-10-19): only when a client without a shell
  (Claude Desktop, a phone app, Open WebUI) needs it. Then a few task-level tools mirroring
  items 18–19 over Streamable HTTP on the current spec, bearer token, each tool marked
  read-only or destructive — not one tool per endpoint.

## Out of scope (deliberately)

Remote desktop (RDP/VNC/Telnet); Docker, Proxmox or Kubernetes management; host metrics
dashboards and alerting beyond up/down notifications (the retired 1.x/2.x product); fleets or
"run on every host"; automations and schedulers; a plugin system; an AI assistant inside the
app (outside agents administer it through tokens and `svrctl`, items 15–19); multiple
database engines; collaborative session sharing; **SSH agent forwarding** (the server would
hold the agent, so a compromised server could sign with it). If one of these is wanted,
Termix or Nexterm already do it well.

## History

### P0 — Security (details in `SECURITY.md`, "Known weaknesses")

| # | Change | Where |
|---|---|---|
| ~~1~~ | Done 2026-10-05: WebSocket upgrades require `Origin` = request host / `X-Forwarded-Host` or an `ALLOWED_ORIGINS` entry (403 otherwise); tested | `auth::websocket_origin_allowed`, both WS handlers |
| ~~2~~ | Done 2026-10-05: token auth resolves the session through the store (expiry-filtered) and takes `user_id` from the record; `X-User-Id`/`?user_id=` ignored; `CurrentUser` extractor replaces per-handler session reads; session id cycled on login; tested | `server/src/routes/ui/auth.rs` |
| ~~3~~ | Done 2026-10-05: one-time 30 s WebSocket tickets (`POST /api/v1/auth/ws-ticket`), redeemed by the middleware on upgrades only; native clients use them with a token fallback for old servers; `?token=` stays accepted (deprecated) for old app builds — remove after the next APK rollout | `server/src/ws_tickets.rs`, `auth.rs`, `TerminalPane.svelte` |
| ~~4~~ | Done 2026-10-05 for the browser (cookie only; no token in `localStorage`). Native apps: move the token to the platform keystore (P1) | `ui/src/routes/auth/login/+page.svelte`, `ui/src/lib/api/client.ts` |
| ~~5~~ | Done 2026-10-05: `server/src/host_keys.rs` evaluates against every stored key; both connectors fail closed; tested | `host_keys.rs`, `terminal.rs`, `terminal_pty.rs` |
| ~~6~~ | Done 2026-10-05: connection test uses the command connector (TOFU first, password credentials supported); `ssh.rs` deleted | `server/src/routes/api/servers.rs` |
| ~~7~~ | Done 2026-10-05: profiles and quick commands scoped per user with 404 on foreign ids; tested | `terminal_profiles.rs`, `quick_commands.rs` |
| ~~8~~ | Done 2026-10-05: metadata encrypted with its own flag (migration 022), start-up pass for legacy rows, redacted `Debug`; passphrases are now settable from the UI/API and used by both connectors. Also fixed: pasted key material was treated as a file path, so "Paste key" credentials could never connect | `database/migrations/022`, `server/src/ssh_keys.rs`, `credentials.rs` |
| ~~9~~ | Done 2026-10-05: the server refuses to start when encryption cannot initialise; the database layer refuses plaintext. (Production had run in the fallback since 2026-09-14 because the compose passed `ENCRYPTION_KEY=` empty.) | `server/src/main.rs`, `database/src/queries/credentials.rs` |
| ~~9a~~ | Done 2026-10-05: start-up encrypts every `encrypted = 0` row in one transaction and logs the count | `encrypt_plaintext_credentials` |
| ~~10~~ | Done 2026-10-05: sliding-window limiter (10 failures / 15 min) per client address and per username on both login routes, 429 + `Retry-After`; 12-character minimum for new passwords; session id cycled on login | `server/src/login_limiter.rs`, `auth.rs`, `users.rs` |
| ~~11~~ | Done 2026-10-11: auth middleware covered end to end (session cookie, Bearer, query token, one-time WS tickets, exempt routes and their lookalikes, OPTIONS preflight, unauthenticated 401-vs-redirect); TOFU accept/mismatch in `host_keys::evaluate`; the WS Origin guard in `websocket_origin_allowed` (both socket handlers call it and 403 a bad origin before upgrade) | `server/src/routes/ui/auth.rs`, `host_keys.rs` |
| ~~12~~ | Done 2026-10-05: russh 0.51 → 0.64 and async-ssh2-tokio 0.8 → 0.13 (the latter removed 2026-10-18 with command mode); `russh-keys` dropped for `russh::keys`; host certificates are pinned by their embedded key. RUSTSEC-2026-0154/-0153 cleared; only `rsa` RUSTSEC-2023-0071 (Marvin, transitive via `ssh-key`, no fix) remains ignored. First live PTY/CMD connection after deploy was the behavioral check; since 2026-10-19 the loopback SSH tests cover it in CI | `Cargo.toml`, `host_keys.rs`, `ci.yaml` |

### P1 — First-class app

| Area | Gap | Proposal |
|---|---|---|
| ~~Settings UI~~ | Done 2026-10-05: both settings forms load the store before reading it | — |
| Reconnect | Done 2026-10-05: reconnects after an error with all six backoff steps and ±15 % jitter | `TerminalPane.svelte` |
| Session persistence | Done 2026-10-10: PTYs live in a server-side `SessionRegistry`; a dropped socket detaches (TTL `SESSION_DETACH_TTL_SECS`, default 15 min) and a reconnect `attach`es with replay from a per-session ring (4096 frames / 2 MiB); terminal tabs survive a reload. Resume list done 2026-10-11: toolbar **Sessions** panel / phone sheet view / palette *Resume Session* list `GET /api/v1/sessions`, **Resume** opens a tab attached to a lost shell, **End** calls `DELETE /api/v1/sessions/{id}` (`SessionRegistry::close_session`, tested). Binary frames done 2026-10-18: raw PTY bytes in the ring, `binary: true` on `shell`/`attach` → 8-byte `seq` + bytes per WebSocket message; fixes multibyte characters split across SSH reads; e2e script covers it | `server/src/sessions.rs`, `terminal_pty.rs`, `TerminalPane.svelte`, `SessionList.svelte`, `state/sessions.svelte.ts` |
| Terminal navigation | Done 2026-10-10: leader key (`Ctrl+Space`, configurable) with tmux-style bindings and an on-screen cheat sheet (`ui/src/lib/utils/leader.ts`); the web app is installable (manifest, icons, a service worker that caches only the hashed shell) | `TerminalView.svelte`, `ui/src/service-worker.ts` |
| Users | Done 2026-10-05: `/api/v1/users` list/create/delete (not self, not last) and `PUT /users/me/password`; Settings → Users tab. Still open: a `role` column so not every account is an administrator | `server/src/routes/api/users.rs` |
| Key setup | Done 2026-10-05: generate an ed25519 key in the app, show any key's public half, and **Authorize key** on a server (ssh-copy-id over a one-time password or an existing credential, verified, then switched over) | `ssh_keys.rs`, `servers.rs` |
| Host keys | Done 2026-10-05: `GET/DELETE /api/v1/servers/{id}/host-keys` with `SHA256:` fingerprints; Keys modal on the Servers page | `servers.rs`, `host_keys::fingerprint` |
| Audit log | Done 2026-10-05: `audit_log` (migration 023) written fire-and-forget for logins/failures/rate limits, logout, sessions, commands (+exit code), server/credential/user changes, host keys forgotten; `GET /api/v1/audit` with cursor paging; Settings → Audit; 180-day prune at start-up | `server/src/audit.rs`, `queries/audit.rs` |
| Backups | Done 2026-10-05: `svrctl backup --out DIR --keep N` (`VACUUM INTO`, key file copied when not in the environment, retention); restore documented in DEPLOYMENT.md | `server/src/bin/svrctl.rs` |
| Health | Done 2026-10-05: `/api/v1/health` pings the database and answers 503 `degraded` when it fails, so the compose healthcheck and the deploy script notice | `routes/api/mod.rs` |
| Observability | Done 2026-10-05: `LOG_FORMAT=json`. Done 2026-10-18: per-request UUID in the `x-request-id` response header and the `request_id` span field (client-sent ids dropped; tested) | `main.rs`, `server/src/request_id.rs` |
| Desktop | Done 2026-10-05 (unverified until the next desktop CI build): tray uses the bundled icon; close hides to the tray only when a tray was created, otherwise it quits. The tray menu already had Show/Quit | `ui/src-tauri/src/lib.rs` |
| Identity | Decided 2026-10-10 (owner): `cc.jsnode.svrctlrs` everywhere. Flatpak dropped (owner does not use it; AppImage/.deb/.rpm remain). Android workflow parked — the app is unused and due for a rethink; `gen/android` must be regenerated for the new identifier if it returns | `tauri.conf.json`, `.forgejo/workflows-parked/` |
| Android | Parked 2026-10-10: the Tauri app's terminal input was unreliable and the owner uses the web app on the phone. Direction under evaluation: a PWA (installable web app) first; native only if that falls short | `.forgejo/workflows-parked/android.yaml` |
| Frontend tooling | Done 2026-10-05: ESLint (flat config, Svelte + TS), Prettier, Vitest with the first unit tests, `engines.node >= 22.12`; `lint` and `test` run in CI | `ui/eslint.config.js`, `ui/.prettierrc` |
| a11y | Done 2026-10-05: svelte-check at 0 warnings, icon buttons labelled, layout buttons `aria-pressed`, toast region `role=status aria-live`, palette is a `combobox` over a `listbox` with `aria-activedescendant`. Arrow/Home/End keys move between terminal tabs with a roving tabindex (2026-10-05) | — |
| Docs | no screenshots | add to README and USER-GUIDE once the UI settles |

### P2 — Hygiene

- Done 2026-10-18: command mode removed (owner: never used it; a shell is more flexible), and
  with it the second SSH stack and `async-ssh2-tokio`. `server/src/ssh.rs` is the one client
  (russh) for the shell, the connection test and "Authorize key"; password credentials now
  work for interactive shells too. Local execution is gone (SECURITY #7 closed).
- Done 2026-10-18: the never-read TOML `Config`, `AppState.config`, the `--config` flag,
  `config/example.toml` and the `toml` dependency are gone. So are `core::types` (`Server`, `ServerStatus`,
  `MetricValue`), monitoring-era types nothing used.
- Done 2026-10-18: `core::Error` has kind-based variants (`NotFound`, `Conflict`,
  `InvalidInput`, `Database(anyhow)` with its cause chain, `Encryption`, `Config`,
  `Notification`, `Other(anyhow)`); `ApiError` implements `IntoResponse` and `From` for both
  `core::Error` and `anyhow::Error`, so handlers use `?` instead of per-call `map_err`
  closures. Lookups return `NotFound` for a missing row instead of a database error.
- Done 2026-10-18: the database crate root no longer glob-re-exports `models::*` and
  `queries::*` (which collided on their module names); callers import by module path.
- Removed 2026-10-05: `Header.svelte`, `ServerSelector.svelte`, `api/index.ts`,
  `platform/biometrics.ts` and the unused `plugin-http`/`plugin-os`/`plugin-biometric` JS
  packages (the Rust plugins stay registered). Still: unused exports in state modules.
- Squash migrations `000`–`015` into one baseline for fresh installs (keep the old files for
  existing databases behind a version check).
- Remove the global `npm i -g @tauri-apps/cli` from both Tauri CI images (the devDependency is
  used); confirm `flatpak-builder` runs unprivileged on the runner or move it out.
- Done 2026-10-05/06: all in-range JS updates; SvelteKit 3 + adapter-static 4 + TypeScript 6
  (`$lib` imports renamed to the `#lib` subpath; tsconfig extends `$app/tsconfig`). Tauri JS pinned to ~2.10 because the Rust
  crate's newest 2.x is 2.10.3 and the CLI refuses a newer JS package (Android build broke).
  `@lucide/svelte` 1.x replaces the deprecated `lucide-svelte` (10-10). Left: TypeScript 7
  (typescript-eslint supports <6.1 today).
- Tag hygiene: `v0.1.0` is newer than `v0.1.1`; both are superseded by `v3.0.0`. Leave them (a
  tag delete breaks nothing but history) and never reuse pre-3.0 numbers.
- GitHub (`github.com/jsprague84/svrctlrs`) is a Forgejo push mirror of `master`; Forgejo is
  the source of truth for issues, CI and releases.

### Done in the 2026-10-04 pass

- CI/CD moved fully to Forgejo on the standard runners; GitHub workflows removed; image
  published to the Forgejo registry with revision labels; master auto-deploys to docker-vm;
  tags produce releases with notes from `CHANGELOG.md`.
- Versions unified at 3.0.0 across Cargo, UI and Tauri; About page reads it from `package.json`.
- Empty `ALLOWED_ORIGINS` no longer switches the cookie to `SameSite=None`.
- `/api/v1/health` reports the build revision.
- Image: pinned Rust, no OpenSSL/curl/sqlite3, revision label; `.dockerignore` excludes
  `node_modules`, builds and the Tauri tree.
- Compose: Forgejo image, `secure-auth-chain@file` for the UI and headers-only for API/WS,
  no Docker group id.
- Docs rewritten for the current product; the retired job-era and rewrite-planning docs were
  removed (they remain in git history).
