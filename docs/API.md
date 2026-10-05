# API reference

Base URL: `https://<host>/api/v1`. All bodies are JSON. Timestamps are RFC 3339 UTC.

## Authentication

Every endpoint except `POST /api/v1/auth/login`, `GET /api/v1/health` and the HTML login
routes requires one of:

| Method | How | Used by |
|---|---|---|
| Session cookie | set by either login route | browser |
| Bearer token | `Authorization: Bearer <session_id>` + `X-User-Id: <id>` | Tauri desktop/Android |
| Query parameters | `?token=<session_id>&user_id=<id>` on WebSocket upgrades only | all clients |

The token *is* the session id returned by the JSON login. It expires after 24 hours of
inactivity. Unauthenticated API calls get `401`; unauthenticated page loads redirect to
`/auth/login`.

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
| POST | `/servers/{id}/test` | — | opens an SSH connection and records `last_seen_at`/`last_error`. **Currently bypasses host-key verification** (ROADMAP P0). |

Server object: `id, name, hostname, port, username, credential_id, description, is_local,
enabled, os_type, os_distro, package_manager, docker_available, systemd_available, metadata,
last_seen_at, last_error, created_at, updated_at`.

## Credentials

| Method | Path | Body |
|---|---|---|
| GET | `/credentials` | — (values are returned as `[REDACTED]`) |
| POST | `/credentials` | `name`, `credential_type` (`password` \| `ssh_key` \| `api_token` …), `value`, `description?`, `username?`, `metadata?` |
| GET | `/credentials/{id}` | returns `has_value: true/false` instead of the value |
| PUT | `/credentials/{id}` | `name?`, `description?`, `value?`, `username?`, `metadata?` |
| DELETE | `/credentials/{id}` | — |

For `ssh_key` credentials `value` may be a path readable by the server (for example a key
mounted at `/home/svrctlrs/.ssh/id_ed25519`) or the key material itself. A passphrase goes in
`metadata.passphrase`. `value` is encrypted at rest; `metadata` is not (ROADMAP P0).

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

## Notifications

| Method | Path | Body | Effect |
|---|---|---|---|
| POST | `/notifications/test` | — | sends a test message with the current `notification.*` settings |

## WebSocket protocol

Both endpoints are JSON text frames. Authenticate with the session cookie or
`?token=<session_id>&user_id=<id>`.

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
later connection presenting a different key *of the same type* is refused with an `error`
frame explaining the mismatch.
