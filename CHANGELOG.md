# Changelog

All notable changes to SvrCtlRS. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
the release workflow uses the matching section as the release notes.

## [Unreleased]

### Added
- Key setup without leaving the app: **Generate SSH key** (ed25519, private half stored
  encrypted, public half shown), **Public key** on any key credential, and **Authorize key** on
  a server, which installs the public key in `~/.ssh/authorized_keys` over a one-time password
  or an existing credential, verifies the key logs in, and switches the server to it
  (`POST /credentials/generate-key`, `POST /servers/{id}/authorize-key`).
- Host-key management: `GET/DELETE /api/v1/servers/{id}/host-keys` and a **Keys** modal on
  the Servers page showing `SHA256:` fingerprints, with **Forget all keys** for a server whose
  key legitimately changed. The mismatch error in the terminal points there.
- Accounts: `/api/v1/users` (list, create, delete — never yourself or the last account) and
  `PUT /api/v1/users/me/password`; a **Users** tab in Settings with a change-password form.
- `svrctl backup --out DIR [--keep N]`: a consistent `VACUUM INTO` copy of the database plus
  the key file (when the key is not in the environment), with retention; restore steps are in
  the deployment guide. `svrctl --token` is now sent as a Bearer header; the dead
  `status metrics` command is gone.
- Audit log: logins (successful, failed, rate-limited), logouts, interactive sessions,
  command-mode commands with exit codes, and changes to servers, credentials and accounts are
  recorded with the client address (`GET /api/v1/audit`, Settings → Audit); entries are kept
  for 180 days and never contain secrets.

### Security
- The HTML login and logout routes refuse cross-site requests (403), closing the last open
  item on the known-weaknesses list.

### Changed
- Health monitor: servers are probed concurrently instead of one 5-second timeout after
  another, and the result is recorded on the server (`last_seen_at`, `last_error`), so the
  Servers page reflects what the monitor knows; a server found down at the very first check
  is logged rather than silently ignored.
- Terminal tabs follow the WAI-ARIA pattern: arrow keys, Home and End move between tabs.
- Reconnect delays carry ±15 % jitter; the command palette's "Manage servers" navigates
  in-app instead of reloading the page.
- `LOG_FORMAT=json` switches the server to JSON log lines; the image builds both binaries in
  one cargo invocation.
- In-range dependency updates (Svelte 5.57, SvelteKit 2.70, Vite 8.3, Tailwind 4.3, Tauri JS
  2.12); unused Tauri JS plugin packages removed.
- `GET /api/v1/health` pings the database and answers 503 (`status: degraded`) when it is
  unreachable, so the container healthcheck reflects a broken volume.
- Accessibility: icon buttons carry labels, layout buttons report `aria-pressed`, toasts are an
  `aria-live` status region, and the command palette is a proper combobox/listbox.
- UI tooling: ESLint, Prettier and Vitest with the first unit tests; `engines.node >= 22.12`.
  The whole UI tree is Prettier-formatted (a formatting-only diff).

### Fixed
- Saving an SSH key credential with the *public* key (`id_ed25519.pub`), a `.pub` path or a
  path from your own machine used to be accepted and then fail at connect time with
  "No such file or directory". The form and the API now refuse these and say which file to
  use; a connection that still hits a bad value explains it instead of echoing an OS error.
- The native apps' server-URL screen normalises what you type (defaults to https, strips
  trailing slashes), refuses plain `http://` on Android — release builds cannot use it — and
  warns about it on desktop.
- Form labels are associated with their controls throughout the Settings and login forms;
  `svelte-check` is at zero warnings.
- Desktop: the tray icon had no image and was invisible on most desktops, leaving its
  Show/Quit menu unreachable after the window was closed; it now uses the application icon,
  and when no tray can be created closing the window quits instead of hiding it.

## [3.0.0] - 2026-10-05

First release of SvrCtlRS as a terminal/SSH management tool. The 1.x/2.x line (a plugin-based
monitoring platform with an HTMX UI) was retired in March 2026; this version shares nothing
with it except the repository and the SQLite migration history.

### Security
- The server refuses to start when the credential encryption key cannot be initialised
  (invalid or empty `ENCRYPTION_KEY`, unreadable key file) instead of falling back to storing
  credentials in plaintext. The database layer likewise refuses to store or return a secret
  without a key.
- On start-up, any credential rows still stored in plaintext by an earlier run are encrypted
  in one transaction; the count is logged.
- WebSocket upgrades refuse a cross-site `Origin` (403): the origin must be the request's own
  host or one of `ALLOWED_ORIGINS`. Closes cross-site WebSocket hijacking of a logged-in
  browser session.
- Token authentication (`Authorization: Bearer`, `?token=`) now resolves the session through
  the store, so expired sessions are refused, and takes the user id from the session record.
  The client-supplied `X-User-Id` header and `user_id` query parameter are ignored.
- Request tracing logs a redacted URI; the WebSocket token no longer appears in logs.
- Host-key verification compares the presented key against every key stored for the server,
  so a different key *algorithm* is a mismatch rather than a first contact; both connectors
  refuse the connection on a database error instead of accepting the key.
- `POST /api/v1/servers/{id}/test` verifies the host key before authenticating and supports
  password credentials; the legacy unverified SSH module is gone.
- Terminal profiles and quick commands: `get`, `update` and `delete` are scoped to the calling
  user and answer 404 for another user's id; setting a default profile no longer clears other
  users' defaults.
- First tests in the `server` crate: auth middleware, origin rules, URI redaction, host-key
  decisions; database tests for per-user scoping.
- The web UI no longer stores the session id in `localStorage`; the HttpOnly cookie is its
  only credential. Only the desktop and Android apps keep a token, and no client sends a
  user id any more.
- Login attempts are rate limited: 10 failures per 15 minutes per client address and per
  username, answered with 429 and `Retry-After`. New passwords must be at least 12
  characters (this applies to `ADMIN_PASSWORD` on a first start). The session id is cycled
  on login.
- Credential metadata — the SSH key passphrase — is encrypted at rest like the value
  (migration 022 adds `metadata_encrypted`); legacy plaintext metadata is encrypted at
  start-up.
- SSH stack upgraded: russh 0.51 → 0.64, async-ssh2-tokio 0.8 → 0.13, clearing
  RUSTSEC-2026-0154 and RUSTSEC-2026-0153 (unbounded allocation reachable by a malicious SSH
  server). A server presenting a host *certificate* is now pinned by the certificate's key.

### Added
- Interactive PTY terminals over WebSocket (russh + xterm.js 6 with WebGL), tabs, four
  layouts, output search, session restore across navigation, reconnect with buffer restore.
- Command mode: run a single command on a server and capture output and exit code.
- Server profiles, credentials encrypted at rest (AES-256-GCM), SSH host-key trust on first use.
- Terminal profiles (saved layouts with per-pane servers) and a default profile loaded at start.
- Quick commands with categories, launched from the command palette (`Ctrl+K`).
- Settings UI with General, Notifications, Quick Commands and About tabs.
- Push notifications (rstify/Gotify/ntfy) for health checks and command completion.
- Mobile layout: drawer sidebar, status bar with tab dots, swipe navigation, extra-keys row,
  safe-area and on-screen-keyboard handling.
- Tauri 2 desktop app for Linux (`.deb`, `.rpm`, AppImage, Flatpak) with tray icon, and a Tauri
  Android app.
- `svrctl` CLI used as the container healthcheck; `/api/v1/health` reports version and the
  build revision.
- Forgejo Actions CI/CD on the dev-space runners: lint, audit, test and frontend checks on
  every branch; image publishing and auto-deploy from `master`; releases with APK and desktop
  packages from `v*` tags.

- SSH key passphrases: a passphrase field on SSH key credentials in the UI and API
  (`passphrase`), used by both the interactive and the command connector.
- `POST /api/v1/auth/ws-ticket`: one-time, 30-second tickets that native clients use to
  authenticate WebSocket upgrades instead of putting their session token in the URL. The
  `?token=` form is deprecated and will be removed after the next app release.

### Fixed
- "Paste key" credentials never worked: both connectors treated the stored key material as a
  file path. Key material is now used directly; paths still work.
- The Notifications and General settings forms opened blank because the settings store was
  never loaded; they now load it first.
- A terminal session that dropped with an error never auto-reconnected (the error status
  masked the "was connected" check); it now reconnects with the full backoff schedule.
### Changed
- Versions unified at 3.0.0 across the Cargo workspace, the UI and the Tauri apps.
- Container image pinned to Rust 1.98, built without OpenSSL, stamped with
  `org.opencontainers.image.revision`.
- Reference compose uses the Forgejo registry and the edge-auth standard middlewares.
- An empty `ALLOWED_ORIGINS` is treated as unset (it used to switch the session cookie to
  `SameSite=None`).

### Removed
- GitHub Actions workflows and the ghcr.io image; GitHub is no longer used.
- Legacy migration repair scripts and the empty root `package.json`.
- Job-era documentation moved to `docs-archived/job-era-2025-12/`.

### Known issues
See `docs/SECURITY.md`, "Known weaknesses" (remaining: no CSRF token on the HTML login
and logout forms; the deprecated `?token=` WebSocket parameter) and `docs/ROADMAP.md`.

## Earlier history

- `v2.1.0`–`v2.2.4` (2025-11): the retired monitoring platform (Dioxus, then HTMX + Askama).
- `v1.0.0` (2025-11-25): first stable release of that platform.
- `v0.1.0`/`v0.1.1` (2026-03): pre-release tags of the terminal rewrite; superseded by 3.0.0.
