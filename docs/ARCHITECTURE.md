# Architecture

SvrCtlRS 3.x is a single Rust server process that serves a SvelteKit single-page app and a
small REST + WebSocket API. The same SPA is packaged by Tauri 2 as a Linux desktop app and an
Android app; those talk to a remote server over the same API.

```
 browser / Tauri desktop / Tauri Android
        │  HTTPS: SPA assets, /api/v1/*        │  WSS: /ws/terminal/pty
        ▼                                       ▼
 ┌─────────────────────────── server (Axum 0.8) ───────────────────────────┐
 │ require_auth middleware ── sessions (tower-sessions, SQLite store)       │
 │ routes/api/*  servers · credentials · settings · profiles ·             │
 │               quick-commands · notifications · users · audit · health · │
 │               files (SFTP browse/transfer, outside the request timeout) │
 │ routes/terminal_pty.rs  interactive shell over WebSocket                 │
 │ ssh.rs                  the one SSH client (russh): host keys, auth, exec │
 │ ssh_config.rs           reads Host blocks of a pasted ssh_config (import)  │
 │ sessions.rs             server-owned PTY sessions, replay ring, TTL      │
 │ health_monitor.rs       periodic checks → notifications (rstify/Gotify) │
 │ ServeDir(SPA_DIR) fallback → index.html                                  │
 └────────────┬──────────────────────────────┬─────────────────────────────┘
              │ sqlx                          │ SSH
        SQLite (data/svrctlrs.db)        your servers (TOFU host keys)
```

## Workspace crates

| Crate | Purpose | Key files |
|---|---|---|
| `core` | AES-256-GCM credential encryption, error type, notification senders | `src/encryption.rs`, `src/error.rs` |
| `database` | sqlx models, query modules, embedded migrations | `src/models/*.rs`, `src/queries/*.rs`, `migrations/` |
| `server` | Axum application and the `svrctl` CLI; `src/test_support/` (tests only) holds a loopback SSH server and database fixtures | `src/main.rs`, `src/routes/**`, `src/bin/svrctl.rs` |

`ui/src-tauri` is a separate Cargo project (excluded from the workspace) so the server build
does not pull in Tauri's GTK/WebKit dependencies.

## Request flow

1. **Middleware** (outermost first): request id (`server/src/request_id.rs`: minted per
   request, client values dropped, echoed as `x-request-id`) → CORS → security headers (CSP,
   Referrer-Policy, X-Frame-Options, nosniff) → gzip → tracing (span carries `request_id`,
   method and a redacted URI) → session manager → `require_auth`.
   HTTP routes also sit behind a 30 s `TimeoutLayer`; the WebSocket and the SFTP file routes
   (`routes/api/files.rs`, streamed transfers) do not.
2. **`require_auth`** (`server/src/routes/ui/auth.rs`) exempts `/auth/login`, `/auth/logout`,
   `/api/v1/auth/login`, `/api/v1/health`, the static app shell, `/manifest.webmanifest` and
   `OPTIONS` (list in `SECURITY.md`). Everything else needs a session cookie, a Bearer token
   (the session id, looked up in the session store), or on a WebSocket upgrade a one-time
   `?ticket=`; the user always comes from the session record. Unauthenticated browser requests
   redirect to `/auth/login`; API requests get 401.
3. **API handlers** (`server/src/routes/api/*.rs`) validate input, call `database` queries,
   and return JSON. Errors use the `ApiError` envelope (see `API.md`): `ApiError` implements
   `IntoResponse` and converts from `svrctlrs_core::Error` by kind (`NotFound` → 404,
   `Conflict` → 409, `InvalidInput` → 400, anything else a logged 500), so handlers use `?`.
4. **The terminal WebSocket** carries a small JSON protocol (`API.md`, "WebSocket protocol");
   PTY output itself travels as binary frames (`seq` + raw bytes) so the terminal decodes the
   stream. PTY sessions live in `sessions.rs` and outlive their sockets. Every SSH connection
   — the shell, the connection test, SFTP, "Authorize key" — goes through `ssh.rs`: russh,
   host key verified during the handshake (first contact recorded and refused until the user
   confirms the fingerprint, mismatch refused, fail closed), password or key
   authentication with the credential's key first and the container's default keys after.
   A server with `via_server_id` is reached through the jump host's own connection (made the
   same way, recursively, at most four hops) over a `direct-tcpip` channel; every connection
   sends keepalives (`SSH_KEEPALIVE_SECS`).
5. **Static files**: `ServeDir(SPA_DIR)` with `index.html` fallback, so client-side routes
   work on reload.

## Authentication and credentials (summary; details in `SECURITY.md`)

- Users: `users` table, argon2 password hashes. The first start seeds an admin from
  `ADMIN_USERNAME`/`ADMIN_PASSWORD` if the table is empty.
- Sessions: tower-sessions with the SQLite store, 24 h inactivity expiry, `Secure` unless
  `SESSION_SECURE=false`, `SameSite=None` only when `ALLOWED_ORIGINS` is configured for
  non-browser clients.
- Credentials: `value` encrypted with AES-256-GCM (`core/src/encryption.rs`). The key comes
  from `ENCRYPTION_KEY` (64 hex chars), else `data/encryption.key`, else is generated.
- SSH host keys: `server_host_keys` table, one row per `(server_id, key_type)`; `trusted_at`
  NULL is the key seen on first contact, waiting for the user to confirm its fingerprint.

## Data model

| Table | Holds |
|---|---|
| `users` | username, argon2 hash |
| `tower_sessions` | session records (managed by the store) |
| `servers` | name, hostname, port, username (NULL = the credential's user, else root), `credential_id`, `via_server_id` (jump host; `ON DELETE SET NULL`), `tags` (JSON array), `startup_command`, `enabled`, `last_seen_at`, `last_error` |
| `credentials` | name, type (`password`, `ssh_key`, …), encrypted `value`, optional `username`, JSON `metadata`, `encrypted` flag |
| `server_host_keys` | `(server_id, key_type)` → public key, first/last seen, `trusted_at` (NULL = waiting for confirmation) |
| `terminal_profiles` | per-user layout + pane configs (`server_id` per pane), `is_default` |
| `quick_commands` | per-user snippets: name, command, optional `server_id`, category, sort order |
| `settings` | key/value with `value_type`; `notification.*` and `app.default_profile_id` are seeded |
| `audit_log` | who did what: user, client address, action, target, JSON details; pruned after 180 days |

Two migration chains are embedded (`database/src/lib.rs::run_migrations`). A fresh database
runs `database/migrations/`: `100_baseline.sql`, the clean schema above, and whatever follows
it. A database created before the baseline carries `_sqlx_migrations` rows below 100; it
finishes the frozen legacy chain in `database/migrations-legacy/` (`000`–`015` built and
dropped the plugin and job schemas, `016`–`023` grew the current product, `024` converges on
the baseline shape: retired local servers disabled, dead columns dropped) and is then marked
as having `100` applied, so later migrations apply to both kinds alike. Such a database keeps
one vestige, `servers.is_local`, because a CHECK constraint references it; nothing reads it.
Migrations are checksummed, so never edit one that has shipped.

## Frontend (`ui/`)

- **SvelteKit 5** with `ssr = false`, `adapter-static`, fallback `index.html`. Build output
  `ui/build` is what the server serves and what Tauri bundles.
- **Routes**: `/` (terminal, rendered by the layout so it survives navigation), `/servers`,
  `/credentials`, `/settings` (General, Notifications, Quick Commands, About), `/auth/login`.
- **State**: `ui/src/lib/state/*.svelte.ts` — `terminal` (tabs, slots, layouts, profile
  apply, `resumeSession`), `terminalPrefs` (xterm options, localStorage), `sessions` (the
  user's live server-side shells, for the resume list), `servers`, `credentials`, `profiles`,
  `quickCommands`, `settings`, `theme`, `toast`, `mobile`.
- **API client**: `ui/src/lib/api/client.ts` adds `Authorization: Bearer` when a token is
  stored (the native apps; the browser uses its cookie), redirects to login on 401.
- **Terminal session**: `ui/src/lib/terminal/ptySession.ts` is the WebSocket side of one pane,
  plain TypeScript with no xterm or Svelte in it: `shell` or `attach`, binary output frames and
  their `seq`, replay, the ping/pong watchdog, reconnect backoff, the host-key challenge, and
  the session record plus serialized screen in sessionStorage. The pane hands it a small
  `TerminalSurface` (write, reset, size, serialize) and reads a `SessionView` (status, banner,
  reconnect overlay, host key) back. Socket, timers, storage and the trust call are injectable,
  so `ptySession.test.ts` drives the whole protocol with fakes. `socketUrl.ts` builds the URL
  (cookie in the browser, one-time ticket for the native apps).
- **Terminal**: `components/terminal/TerminalPane.svelte` (xterm.js 6 and its addons, WebGL
  with DOM fallback, the touch gesture machine, the overlays; keystrokes go to the session
  while its socket is open), `TerminalView.svelte` (layouts, keyboard shortcuts, command palette), `TerminalTabs`,
  `SplitView`, `CommandPalette`, `TerminalPrefsPanel`, `ProfileManager`, `SessionsPanel` +
  `SessionList` (resume or end the shells still running on the server; a pane's session record
  in sessionStorage — `utils/sessionRecord.ts` — is what ties a tab to a session, so resuming a
  lost one is "write the record, open the tab, let the pane attach"), `FilesPanel` +
  `FileBrowser` (SFTP browser for the active tab's server; `state/files.svelte.ts` keeps the
  last directory per server).
- **Mobile**: `MobileStatusBar`, `TabDots`, `ExtraKeysRow`, swipe between tabs, visual-viewport
  keyboard detection (`platform/keyboard.svelte.ts`).
- **Platform**: `platform/index.ts` detects Tauri and stores the server URL; in the browser the
  API base is same-origin.
- **Styling**: Tailwind v4 `@theme` tokens, Tokyo Night dark (default) and a light theme via
  `data-theme`; xterm themes follow the app theme.

## Tauri apps (`ui/src-tauri/`)

One Tauri project; the Linux desktop build (`.deb`, `.rpm`, AppImage) registers a tray icon and
hides to tray on close. The app identifier is `cc.jsnode.svrctlrs`. The Android target is
parked (`.forgejo/workflows-parked/`); the phone uses the web app.

## Build and delivery

`Dockerfile` builds the SPA (node 22), then the server and `svrctl` (cargo-chef + sccache),
into a `debian:bookworm-slim` image running as uid 1000 with a `svrctl health` healthcheck.
CI (`.forgejo/workflows/`) is described in `DEVELOPMENT.md`.
