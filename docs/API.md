# API reference

Base URL: `https://<host>/api/v1`. All bodies are JSON. Timestamps are RFC 3339 UTC.

## Authentication

Every endpoint except `POST /api/v1/auth/login`, `GET /api/v1/health` and the HTML login
routes requires one of:

| Method | How | Used by |
|---|---|---|
| Session cookie | set by either login route | browser (also on WebSocket upgrades) |
| Bearer token | `Authorization: Bearer <session_id>` | Tauri desktop/Android |
| One-time ticket | `?ticket=<ticket>` on WebSocket upgrades, from `POST /auth/ws-ticket` | Tauri desktop/Android |
| Query token | `?token=<session_id>` on WebSocket upgrades | older native builds; deprecated |

The token *is* the session id returned by the JSON login. It expires after 24 hours of
inactivity. The user is always taken from the session record; `X-User-Id` and `user_id`
parameters are ignored. Unauthenticated API calls get `401`; unauthenticated page loads
redirect to `/auth/login`. Ten failed logins per 15 minutes, per client address or per
username, answer `429` with `Retry-After`.

```http
POST /api/v1/auth/ws-ticket          (authenticated)
200 {"ticket": "<43 url-safe chars>", "expires_in_secs": 30}
```

A ticket is bound to the caller, valid for 30 seconds and consumed by the first WebSocket
upgrade that presents it; it is not accepted on ordinary requests.

```http
POST /api/v1/auth/login
Content-Type: application/json

{"username": "admin", "password": "…"}

200 {"success": true, "user_id": 1, "session_id": "…"}
401 {"error": "Invalid username or password"}
```

`POST /auth/login` (form fields `username`, `password`) is the HTML fallback and redirects to
`/`. `POST /auth/logout` flushes the session and redirects to the login page.

## Envelope

Successful responses return the resource directly (an object or an array). Errors return:

```json
{"success": false, "error": {"code": "NOT_FOUND", "message": "Server not found"}}
```

Codes: `BAD_REQUEST` (400), `NOT_FOUND` (404), `CONFLICT` (409), `INTERNAL_ERROR` (500).

## Health and status

| Method | Path | Auth | Response |
|---|---|---|---|
| GET | `/health` | no | `{"status":"ok","service":"svrctlrs","version":"3.0.0","revision":"<git sha>"}` — `revision` is baked into the container image; `unknown` outside CI builds |
| GET | `/status` | yes | `{"status":"running","version":"…","resources":{"servers":<count>}}` |

## Servers

| Method | Path | Body | Notes |
|---|---|---|---|
| GET | `/servers` | — | list |
| POST | `/servers` | `name`, `hostname?`, `port` (default 22), `username?`, `credential_id?`, `description?`, `is_local` (bool), `enabled` (bool), `metadata?` (object) | `is_local: true` opens a shell *inside the SvrCtlRS container* — see SECURITY.md |
| GET | `/servers/{id}` | — | |
| PUT | `/servers/{id}` | any subset of the create fields plus `os_type?`, `os_distro?` | |
| DELETE | `/servers/{id}` | — | |
| POST | `/servers/{id}/test` | — | verifies the host key (trust on first use), authenticates with the server's credential and runs a trivial command; returns `success`, `message`, `latency_ms` |
| GET | `/servers/{id}/host-keys` | — | stored host keys: `id`, `key_type`, `fingerprint` (`SHA256:…`, OpenSSH style), `first_seen_at`, `last_seen_at` — never the key itself |
| DELETE | `/servers/{id}/host-keys` | — | forget every stored key; the next connection trusts what it sees. Returns `removed` |

Server object: `id, name, hostname, port, username, credential_id, description, is_local,
enabled, os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
last_seen_at, last_error, created_at, updated_at`.

## Credentials

| Method | Path | Body |
|---|---|---|
| GET | `/credentials` | — (no secrets: `has_value`, `has_passphrase`) |
| POST | `/credentials` | `name`, `credential_type` (`password` \| `ssh_key` \| `api_token`), `value`, `description?`, `username?`, `passphrase?` |
| GET | `/credentials/{id}` | `has_value`, `has_passphrase`, `in_use` instead of the secrets |
| PUT | `/credentials/{id}` | `name?`, `description?`, `value?`, `username?`, `passphrase?` (omit to keep, `""` to remove) |
| DELETE | `/credentials/{id}` | — |

For `ssh_key` credentials `value` is either the key material itself (`-----BEGIN …`) or a path
readable by the server (for example a key mounted at `/home/svrctlrs/.ssh/id_ed25519`).
`passphrase` unlocks an encrypted key. Value and passphrase are both encrypted at rest.

## Settings

| Method | Path | Body |
|---|---|---|
| GET | `/settings` | — |
| POST | `/settings` | `key`, `value`, `value_type` (`string` \| `number` \| `boolean` \| `json`), `description?` |
| GET | `/settings/{key}` | — |
| PUT | `/settings/{key}` | `{"value": "…"}` — validated against the stored `value_type` |

Seeded keys: `notification.enabled`, `notification.provider` (`gotify` \| `ntfy`),
`notification.url`, `notification.token`, `notification.topic`,
`notification.health_check_enabled`, `notification.health_check_interval_seconds`,
`app.default_profile_id`.

## Terminal profiles (per user)

| Method | Path | Body |
|---|---|---|
| GET | `/profiles` | — |
| POST | `/profiles` | `name`, `layout` (`single` \| `split-h` \| `split-v` \| `quad`), `description?`, `pane_configs?` (array of `{server_id?, mode?}`), `is_default` |
| GET / PUT / DELETE | `/profiles/{id}` | PUT takes any subset |

## Quick commands (per user)

| Method | Path | Body |
|---|---|---|
| GET | `/quick-commands` | — |
| POST | `/quick-commands` | `name`, `command`, `server_id?`, `category?` (default `general`), `sort_order?` |
| GET / PUT / DELETE | `/quick-commands/{id}` | PUT takes any subset |

## Users

Every account is an administrator until roles exist, so any user may manage accounts.

| Method | Path | Body | Notes |
|---|---|---|---|
| GET | `/users` | — | `id`, `username`, `is_me`, timestamps; never hashes |
| POST | `/users` | `username` (1–64 chars), `password` (≥ 12 chars) | 409 if the name exists |
| DELETE | `/users/{id}` | — | refused for your own account and for the last account |
| PUT | `/users/me/password` | `current_password`, `new_password` (≥ 12 chars) | 401 when the current password is wrong |

Deleting a user cascades to their sessions, profiles and quick commands.

## Notifications

| Method | Path | Body | Effect |
|---|---|---|---|
| POST | `/notifications/test` | — | sends a test message with the current `notification.*` settings |

## WebSocket protocol

Both endpoints are JSON text frames. Authenticate with the session cookie (browser) or a
one-time `?ticket=` (native apps). The upgrade's `Origin` must be the server's own host or one
of `ALLOWED_ORIGINS`, else `403`.

### `/ws/terminal/pty` — interactive shell

Client → server:

| `type` | Fields | Meaning |
|---|---|---|
| `shell` | `server_id`, `cols`, `rows` | open a PTY on the server (omit `server_id` for none configured → error) |
| `input` | `data` | keystrokes |
| `resize` | `cols`, `rows` | terminal resized |
| `ping` | — | keepalive |
| `close` | — | end the session |

Server → client: `{"type": "connected"}` once the shell is up, `{"type": "output", "data": "…"}`
for terminal output, `{"type": "error", "data": "…"}`, `{"type": "pong"}`.

### `/ws/terminal` — command mode

Client → server: `{"type": "execute", "server_id": 1, "command": "uptime", "env": {…},
"cols": 120, "rows": 40}` or `{"type": "ping"}`.

Server → client: `output` frames with `data`, then `{"type": "exit", "data": "", "exit_code": 0}`;
`error` and `pong` as above.

Host-key handling on both endpoints: the first connection to a server stores its key; a
later connection presenting a key that matches none stored for that server is refused with an
`error` frame explaining the mismatch. Profiles and quick commands are per user: another
user's id answers 404.
