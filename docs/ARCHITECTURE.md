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
 │               recordings · tokens · approvals                           │
 │               files (SFTP browse/transfer, outside the request timeout) │
 │ routes/terminal_pty.rs  interactive shell over WebSocket                 │
 │ ssh.rs                  the one SSH client (russh): host keys, auth, exec │
 │ ssh_config.rs           reads Host blocks of a pasted ssh_config (import)  │
 │ diagnose.rs             the connection walked step by step, with fixes     │
 │ key_ops.rs              install/remove public keys (authorize, rotate)     │
 │ sessions.rs             server-owned PTY sessions, replay ring, TTL      │
│ persistent_shells.rs    shells kept in tmux across a restart             │
 │ recordings.rs           opt-in asciicast recording, encrypted, per user  │
 │ health_monitor.rs       periodic checks → notifications (rstify/Gotify) │
 │ ServeDir(SPA_DIR) fallback → index.html                                  │
 └────────────┬──────────────────────────────┬─────────────────────────────┘
              │ sqlx                          │ SSH
        SQLite (data/svrctlrs.db)        your servers (TOFU host keys)
        + data/recordings/<user>/*.cast.enc
```

## Workspace crates

| Crate | Purpose | Key files |
|---|---|---|
| `api` (`svrctlrs-api`) | the REST API's request and response types with their OpenAPI schemas; used by the server's handlers and by `svrctl` | `src/*.rs` (one module per resource) |
| `core` | AES-256-GCM credential encryption, error type, notification senders, webhook and Matrix posting | `src/encryption.rs`, `src/error.rs`, `src/notifications.rs` |
| `database` | sqlx models, query modules, embedded migrations | `src/models/*.rs`, `src/queries/*.rs`, `migrations/` |
| `server` | Axum application and the `svrctl` CLI (`src/bin/svrctl/`: the agent CLI over the API with `svrctlrs-api` types, plus the local `backup` and `reset-password`); `src/test_support/` (tests only) holds a loopback SSH server, database fixtures and the full app on a temporary database; `tests/svrctl.rs` runs both binaries | `src/main.rs`, `src/routes/**`, `src/bin/svrctl/` |

`ui/src-tauri` is a separate Cargo project (excluded from the workspace) so the server build
does not pull in Tauri's GTK/WebKit dependencies.

## Request flow

1. **Middleware** (outermost first): request id (`server/src/request_id.rs`: minted per
   request, client values dropped, echoed as `x-request-id`) → CORS → security headers (CSP,
   Referrer-Policy, X-Frame-Options, nosniff) → gzip → tracing (span carries `request_id`,
   method and a redacted URI) → session manager → `require_auth`.
   HTTP routes also sit behind a 30 s `TimeoutLayer`; the WebSocket and the SFTP file routes
   (`routes/api/files.rs`, streamed transfers) do not.
2. **`require_auth`** (`server/src/routes/ui/auth.rs`) exempts `/auth/login`,
   `/auth/login/totp` (a password sign-in's second step, `server/src/sign_in/totp.rs`),
   `/auth/oidc/start` and `/auth/oidc/callback` (sign-in with an OpenID Connect provider:
   `routes/ui/oidc.rs` over `server/src/sign_in/oidc.rs` and `jwt.rs`; how anyone may sign in
   is `SignInConfig`, read from the environment at start-up), `/auth/logout`,
   `/api/v1/auth/login`, `/api/v1/health`, the static app shell, `/manifest.webmanifest` and
   `OPTIONS` (list in `SECURITY.md`). Everything else needs a session cookie, an API token
   (`Bearer svr_…`, `server/src/api_tokens.rs`: hashed lookup, then the token's access preset
   decides by method and path, 403 for anything it does not name), a session Bearer token (the
   session id, looked up in the session store), or on a WebSocket upgrade a one-time
   `?ticket=`; the user always comes from the session or token record. An API token is looked
   at first and alone (a cookie beside it is ignored); two `Authorization` headers, or a login
   carrying one, get 400, which keeps the edge's token router (DEPLOYMENT.md) from reaching
   anything a token does not allow. A token request runs
   inside `audit::with_token`, so every audit row it writes names the token. For a token set
   to ask first, the three gated deletes stop here: `approvals::hold` stores the request,
   sends its link and answers 202; on the owner's approval `approvals::decide` replays the call
   through `AppState::api_router` (the API routes without the auth layer) as the token. Unauthenticated
   browser requests redirect to `/auth/login`; API requests get 401. The whole stack is built
   by `server/src/app.rs::router`, which `main` serves and the end-to-end tests drive.
3. **API handlers** (`server/src/routes/api/*.rs`) validate input, call `database` queries,
   and return `svrctlrs-api` types as JSON. Each module's router is a utoipa
   `OpenApiRouter`; `routes!(handler)` routes and documents a handler at once, and
   `routes::openapi()` assembles the document served at `GET /api/v1/openapi.json` (and kept
   in `docs/openapi.json`). The task-level operations (`routes/api/ops.rs`: diagnose, exec,
   onboard, rotate-key) sit outside the request timeout beside the file routes; diagnose
   (`diagnose.rs`) walks DNS → TCP or tunnel → handshake → host key → login per hop with the
   same handler and login code as `ssh::connect` (split into `Handler::new` and
   `ssh::authenticate` for it), and key installs go through `key_ops.rs`. Errors use the
   `ApiError` envelope (see `API.md`): `ApiError` implements
   `IntoResponse` and converts from `svrctlrs_core::Error` by kind (`NotFound` → 404,
   `Conflict` → 409, `InvalidInput` → 400, anything else a logged 500), so handlers use `?`.
4. **The terminal WebSocket** carries a small JSON protocol (`API.md`, "WebSocket protocol");
   PTY output itself travels as binary frames (`seq` + raw bytes) so the terminal decodes the
   stream. PTY sessions live in `sessions.rs` and outlive their sockets. Each socket reads a
   session through a `Follower` (the replay ring, then live frames, the ring again when it
   falls behind, a redraw when even the ring has moved on); the PTY driver never waits for a
   slow client. On a server with `record_sessions`, a recorder (`recordings.rs`) follows the
   session the same way and writes asciicast v2, encrypted per chunk with the credential key,
   to `RECORDINGS_DIR/<user id>/`; `routes/api/recordings.rs` serves it back, decrypted, to
   its owner only. On a server with `persistent_shell`, the shell runs in tmux there
   (`persistent_shells.rs`): a restart or a lost connection *releases* the session (clients
   reconnect), and an `attach` for a session that is not live opens a new connection to the
   same tmux session; housekeeping ends kept shells nobody reattaches to. Hourly housekeeping applies `recording.retention_days` and removes files
   no row knows. Every SSH connection
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
  `SESSION_SECURE=false`, always `SameSite=Strict`. The desktop app's sessions come from
  `POST /api/v1/auth/app-token` (`routes/ui/app_sign_in.rs`), 30 days.
- Credentials: `value` encrypted with AES-256-GCM (`core/src/encryption.rs`). The key comes
  from `ENCRYPTION_KEY` (64 hex chars), else `data/encryption.key`, else is generated.
- API tokens: `api_tokens` table, SHA-256 of the secret only; presets enforced by
  `Preset::allows` in the auth layer; never in a URL.
- SSH host keys: `server_host_keys` table, one row per `(server_id, key_type)`; `trusted_at`
  NULL is the key seen on first contact, waiting for the user to confirm its fingerprint.

## Data model

| Table | Holds |
|---|---|
| `users` | username, argon2 hash |
| `tower_sessions` | session records (managed by the store) |
| `servers` | name, hostname, port, username (NULL = the credential's user, else root), `credential_id`, `via_server_id` (jump host; `ON DELETE SET NULL`), `tags` (JSON array), `startup_command`, `record_sessions`, `persistent_shell`, `enabled`, `last_seen_at`, `last_error` |
| `credentials` | name, type (`password`, `ssh_key`, …), encrypted `value`, optional `username`, JSON `metadata`, `encrypted` flag |
| `server_host_keys` | `(server_id, key_type)` → public key, first/last seen, `trusted_at` (NULL = waiting for confirmation) |
| `terminal_profiles` | per-user layout + pane configs (`server_id` per pane), `is_default` |
| `quick_commands` | per-user snippets: name, command, optional `server_id`, category, sort order |
| `settings` | key/value with `value_type`; `notification.*` and `app.default_profile_id` are seeded |
| `user_settings` | a user's own settings: (`user_id`, `key`) → `value`; `terminal` (a JSON document the UI owns) and `session_keep` (seconds, read by the server) (`ON DELETE CASCADE`) |
| `persistent_shells` | shells kept in tmux: session `id`, `user_id`, `server_id` (both `ON DELETE CASCADE`), `tmux_name` — what a restarted server needs to reattach |
| `login_failures` | the login limiter's failed attempts: `key` (`ip:…` / `user:…`), `failed_at` (Unix seconds); pruned past the 15-minute window |
| `user_totp` | one TOTP enrolment per user: encrypted base32 `secret`, `confirmed_at` (NULL while pending), `last_step` (no code twice) |
| `user_recovery_codes` | SHA-256 of each one-time recovery code, `used_at` |
| `user_identities` | an account's OpenID Connect identities: `issuer` + `subject` (unique), `email`/`display_name` as last seen, `last_used_at` |
| `api_tokens` | a user's named API tokens: SHA-256 `token_hash` (unique), display `prefix`, `preset`, `expires_at` (NULL = never), `last_used_at`/`last_used_ip`, `revoked_at`, `require_approval`; name unique per user (`ON DELETE CASCADE`) |
| `approvals` | held token calls: random `id` (in the link), `user_id` (`ON DELETE CASCADE`), `token_id` (`ON DELETE SET NULL`) and `token_name`, `action`, `method`, `path`, target, `url`, `status` (`pending`/`approved`/`denied`), `expires_at`, the decision, and the approved call's `result_status` and `result` |
| `audit_log` | who did what: user, client address, action, target, JSON details, and the API token behind it (`token_id`, `token_name`, no foreign key so the name outlives the token); pruned after 180 days |
| `session_recordings` | one recorded shell: owner `user_id` (`ON DELETE CASCADE`), `server_id` (`ON DELETE SET NULL`) and `server_name`, `session_id`, the encrypted file's name, start size, times, plaintext `size_bytes`, `status`, `end_reason` |

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
  `/credentials`, `/recordings` (the user's recordings; `?server=` filters, `?play=` opens
  one), `/settings` (General, Notifications, Quick Commands, Users, Audit, About), `/auth/login`.
- **Recordings**: `lib/recordings/cast.ts` reads asciicast v2 and does the player's arithmetic
  (shortened pauses, where a moment falls, output coalesced into writes with resizes in
  place; Vitest-tested); `components/recordings/RecordingPlayer.svelte` plays it in a
  read-only xterm at the recorded size, scaled down to fit, seeking back by redrawing from the
  start.
- **State**: `ui/src/lib/state/*.svelte.ts` — `terminal` (tabs, slots, layouts, profile
  apply, `resumeSession`), `terminalPrefs` (xterm options and the predictive-echo mode, kept
  on the account through `GET/PUT /users/me/preferences/terminal` and copied to localStorage
  so the terminal starts with them; `lib/terminal/prefsSync.ts` saves a change after 800 ms of
  quiet, never lets the server overwrite an unsaved change, uploads a browser's own
  preferences when the account has none, and pulls again whenever the page is shown),
  `sessions` (the user's live server-side shells, for the resume list), `servers`,
  `credentials`, `profiles`, `quickCommands`, `settings`, `theme`, `toast`, `mobile`.
- **Quick-command variables**: `lib/utils/snippet.ts` (Vitest) finds `{{word}}` and
  `{{word:default}}` (plain identifiers only, Go template keywords reserved), fills the
  built-ins from the tab's server and lists the rest; `TerminalView.runQuickCommand` types the
  result or opens `SnippetVarsDialog` to ask first.
- **API client**: `ui/src/lib/api/client.ts` adds `Authorization: Bearer` with the desktop
  app's token (`platform/appToken.ts`: OS keyring through Tauri commands, held in memory; the
  browser uses its cookie). On 401 the browser loads the server's login page, the app its own
  "Sign in with your browser" page.
- **Terminal session**: `ui/src/lib/terminal/ptySession.ts` is the WebSocket side of one pane,
  plain TypeScript with no xterm or Svelte in it: `shell` or `attach`, binary output frames and
  their `seq`, replay, the ping/pong watchdog, reconnect backoff, the host-key challenge, and
  the session record plus serialized screen in sessionStorage. The pane hands it a small
  `TerminalSurface` (write, reset, size, serialize) and reads a `SessionView` (status, banner,
  reconnect overlay, host key) back. Socket, timers, storage and the trust call are injectable,
  so `ptySession.test.ts` drives the whole protocol with fakes. `socketUrl.ts` builds the URL
  (cookie in the browser, one-time ticket for the native apps).
- **Predictive echo**: `ui/src/lib/terminal/localEcho.ts`. The session hands it each keystroke
  before sending it and calls `settled()` whenever the terminal has parsed all the output
  written so far (xterm's write callback). Printable keystrokes become predictions at the
  cursor; a prediction is confirmed when the **screen** shows it (its cell holds the character
  and the cursor has passed it), so colour, zsh redraws and syntax highlighting confirm as well
  as a bare echo. Predictions are drawn only in an epoch the shell has already echoed in (mosh's
  rule; non-text keys, unexplained output, mismatches and a 2 s timeout start a new one), never
  in the alternate screen or at a password prompt, and in `auto` only while the measured echo
  latency is above 30 ms. The pane implements its `EchoScreen` over xterm's buffer and draws the
  predictions as xterm decorations (one per line, the terminal's font, a dotted underline, a
  caret after them), so nothing is ever written into the terminal and a wrong guess cannot
  corrupt the shell's screen. `SECURITY.md` has the guarantees and the one gap.
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
hides to tray on close. `src-tauri/src/auth.rs` signs it in through the system browser (a
`127.0.0.1` listener, PKCE, `tauri-plugin-opener`) and keeps the token with `keyring`. Besides
the CI packages, `flake.nix` builds it the nixpkgs way (`packaging/nix/desktop.nix`). The app identifier is `cc.jsnode.svrctlrs`. The Android target is
parked (`.forgejo/workflows-parked/`); the phone uses the web app.

## Build and delivery

`Dockerfile` builds the SPA (node 22), then the server and `svrctl` (cargo-chef + sccache, kept
warm by a persistent buildx builder in CI),
into a `debian:bookworm-slim` image running as uid 1000 with a `svrctl health` healthcheck.
CI (`.forgejo/workflows/`) is described in `DEVELOPMENT.md`.
