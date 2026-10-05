# Changelog

All notable changes to SvrCtlRS. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
the release workflow uses the matching section as the release notes.

## [Unreleased]

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
- First tests in the `server` crate: auth middleware, origin rules, URI redaction.

## [3.0.0] - 2026-10-04

First release of SvrCtlRS as a terminal/SSH management tool. The 1.x/2.x line (a plugin-based
monitoring platform with an HTMX UI) was retired in March 2026; this version shares nothing
with it except the repository and the SQLite migration history.

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
See `docs/SECURITY.md`, "Known weaknesses", and `docs/ROADMAP.md` P0.

## Earlier history

- `v2.1.0`–`v2.2.4` (2025-11): the retired monitoring platform (Dioxus, then HTMX + Askama).
- `v1.0.0` (2025-11-25): first stable release of that platform.
- `v0.1.0`/`v0.1.1` (2026-03): pre-release tags of the terminal rewrite; superseded by 3.0.0.
