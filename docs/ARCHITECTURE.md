# Architecture

SvrCtlRS 3.x is a single Rust server process that serves a SvelteKit single-page app and a
small REST + WebSocket API. The same SPA is packaged by Tauri 2 as a Linux desktop app and an
Android app; those talk to a remote server over the same API.

```
 browser / Tauri desktop / Tauri Android
        │  HTTPS: SPA assets, /api/v1/*        │  WSS: /ws/terminal, /ws/terminal/pty
        ▼                                       ▼
 ┌─────────────────────────── server (Axum 0.8) ───────────────────────────┐
 │ require_auth middleware ── sessions (tower-sessions, SQLite store)       │
 │ routes/api/*  servers · credentials · settings · profiles ·             │
 │               quick-commands · notifications · health · status          │
 │ routes/terminal.rs      CMD mode  (async-ssh2-tokio, one command)        │
 │ routes/terminal_pty.rs  PTY mode  (russh, interactive shell)             │
 │ health_monitor.rs       periodic checks → notifications (rstify/Gotify) │
 │ ServeDir(SPA_DIR) fallback → index.html                                  │
 └────────────┬──────────────────────────────┬─────────────────────────────┘
              │ sqlx                          │ SSH
        SQLite (data/svrctlrs.db)        your servers (TOFU host keys)
```

## Workspace crates

| Crate | Purpose | Key files |
|---|---|---|
| `core` | AES-256-GCM credential encryption, error types, shared types | `src/encryption.rs`, `src/error.rs` |
| `database` | sqlx models, query modules, embedded migrations | `src/models/*.rs`, `src/queries/*.rs`, `migrations/` |
| `server` | Axum application and the `svrctl` CLI | `src/main.rs`, `src/routes/**`, `src/bin/svrctl.rs` |

`ui/src-tauri` is a separate Cargo project (excluded from the workspace) so the server build
does not pull in Tauri's GTK/WebKit dependencies.

## Request flow

1. **Middleware** (outermost first): CORS → security headers (CSP, Referrer-Policy,
   X-Frame-Options, nosniff) → gzip → tracing → session manager → `require_auth`.
   HTTP routes also sit behind a 30 s `TimeoutLayer`; WebSocket routes do not.
2. **`require_auth`** (`server/src/routes/ui/auth.rs`) exempts `/auth/login`, `/auth/logout`,
   `/api/v1/auth/login`, `/api/v1/health`, `/_app/*` and `OPTIONS`. Everything else needs
   either a session cookie, or a Bearer token (the session id) with `X-User-Id`, or for
   WebSockets `?token=&user_id=` query parameters. Unauthenticated browser requests redirect to
   `/auth/login`; API requests get 401.
3. **API handlers** (`server/src/routes/api/*.rs`) validate input, call `database` queries,
   and return JSON. Errors use the `ApiError` envelope (see `API.md`).
4. **Terminal WebSockets** carry a small JSON protocol (`API.md`, "WebSocket protocol"). PTY
   sessions run a russh client task per socket; CMD mode runs one command and closes.
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
- SSH host keys: `server_host_keys` table, trust-on-first-use per `(server_id, key_type)`.

## Data model

| Table | Holds |
|---|---|
| `users` | username, argon2 hash |
| `tower_sessions` | session records (managed by the store) |
| `servers` | name, hostname, port, username, `credential_id`, `is_local`, `enabled`, detected OS facts, `last_seen_at`, `last_error` |
| `credentials` | name, type (`password`, `ssh_key`, …), encrypted `value`, optional `username`, JSON `metadata`, `encrypted` flag |
| `server_host_keys` | `(server_id, key_type)` → public key, first/last seen |
| `terminal_profiles` | per-user layout + pane configs (`server_id`, `mode` per pane), `is_default` |
| `quick_commands` | per-user snippets: name, command, optional `server_id`, category, sort order |
| `settings` | key/value with `value_type`; `notification.*` and `app.default_profile_id` are seeded |

Migrations `000`–`015` built and then dropped two earlier schemas (the plugin and job
systems); `016`–`021` are the current product. A fresh database replays all of them. They are
embedded with `sqlx::migrate!` and checksummed, so never edit a shipped migration.

## Frontend (`ui/`)

- **SvelteKit 5** with `ssr = false`, `adapter-static`, fallback `index.html`. Build output
  `ui/build` is what the server serves and what Tauri bundles.
- **Routes**: `/` (terminal, rendered by the layout so it survives navigation), `/servers`,
  `/credentials`, `/settings` (General, Notifications, Quick Commands, About), `/auth/login`.
- **State**: `ui/src/lib/state/*.svelte.ts` — `terminal` (tabs, slots, layouts, profile
  apply), `terminalPrefs` (xterm options, localStorage), `servers`, `credentials`, `profiles`,
  `quickCommands`, `settings`, `theme`, `toast`, `mobile`.
- **API client**: `ui/src/lib/api/client.ts` adds `Authorization: Bearer` + `X-User-Id` when a
  token is stored, redirects to login on 401.
- **Terminal**: `components/terminal/TerminalPane.svelte` (xterm.js 6 + WebSocket, WebGL with
  DOM fallback, reconnect, buffer restore via the serialize addon, touch gestures),
  `TerminalView.svelte` (layouts, keyboard shortcuts, command palette), `TerminalTabs`,
  `SplitView`, `CommandPalette`, `TerminalPrefsPanel`, `ProfileManager`.
- **Mobile**: `MobileStatusBar`, `TabDots`, `ExtraKeysRow`, swipe between tabs, visual-viewport
  keyboard detection (`platform/keyboard.svelte.ts`).
- **Platform**: `platform/index.ts` detects Tauri and stores the server URL; in the browser the
  API base is same-origin.
- **Styling**: Tailwind v4 `@theme` tokens, Tokyo Night dark (default) and a light theme via
  `data-theme`; xterm themes follow the app theme.

## Tauri apps (`ui/src-tauri/`)

One Tauri project builds both targets. Desktop registers a tray icon and hides to tray on
close; Android registers the biometric plugin (not yet wired into the UI). The app identifier
is `app.js-node.svrctlrs` (Android `app.js_node.svrctlrs`); the Flatpak manifest in
`.forgejo/` still uses `cc.jsnode.svrctlrs` — see `ROADMAP.md`.

## Build and delivery

`Dockerfile` builds the SPA (node 22), then the server and `svrctl` (cargo-chef + sccache),
into a `debian:bookworm-slim` image running as uid 1000 with a `svrctl health` healthcheck.
CI (`.forgejo/workflows/`) is described in `DEVELOPMENT.md`.
