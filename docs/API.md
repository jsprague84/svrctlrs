# API reference

Base URL: `https://<host>/api/v1`. All bodies are JSON. Timestamps are RFC 3339 UTC.

**The machine-readable reference is the OpenAPI 3.1 document**: `GET /api/v1/openapi.json`
on a running server (any signed-in user or token) and `docs/openapi.json` in the repository,
which a test keeps equal to what the code generates. Every route and its request and response
types come from the handlers themselves (utoipa), and the types live in the `svrctlrs-api`
crate (`api/`), which `svrctl` uses too. This page is the guide: what the calls are for and
the rules around them. The terminal WebSocket is only described here.

## Authentication

Every endpoint except `POST /api/v1/auth/login`, `GET /api/v1/health` and the HTML login
routes requires one of:

| Method | How | Used by |
|---|---|---|
| Session cookie | set by either login route | browser (also on WebSocket upgrades) |
| API token | `Authorization: Bearer svr_…` (never in a URL) | agents, scripts, `svrctl` |
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

A ticket is bound to the caller (and the API token it was asked with, whose access the
upgrade then has), valid for 30 seconds and consumed by the first WebSocket upgrade that
presents it; it is not accepted on ordinary requests.

### API tokens

Created by a signed-in user (`/tokens` below); a token acts as its owner, within its preset.
Refusals use the error envelope:

| Status | `error.code` | When |
|---|---|---|
| 401 | `TOKEN_INVALID` | malformed (`svr_` + 40 letters and digits expected) or unknown |
| 401 | `TOKEN_EXPIRED` / `TOKEN_REVOKED` | the token's lifetime ended / it was revoked |
| 401 | `TOKEN_IN_URL` | `?token=svr_…`: tokens go in the header only |
| 403 | `TOKEN_SCOPE` | the preset does not name this method and path; `error.details: {preset, method, path}` |
| 202 | — | a token set to ask first called `DELETE /servers/{id}`, `DELETE /credentials/{id}` or `DELETE /servers/{id}/host-keys`: `{approval, message, wait_url}` (see "Approvals"); nothing ran yet |

Presets (`full_admin`, `servers_keys`, `read_only`) and exactly what each allows:
`SECURITY.md`, "API tokens", and the table in `server/src/api_tokens.rs`. No preset reaches
`/tokens`, `POST /users`, `DELETE /users/{id}` or `PUT /users/me/password`.

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

Codes: `BAD_REQUEST` (400), `UNAUTHORIZED` (401), `FORBIDDEN` (403), `NOT_FOUND` (404),
`CONFLICT` (409; also a credential or server name that is already taken, on create and
rename, and deleting a credential a server still uses), `HOST_KEY_MISMATCH` (409),
`HOST_KEY_UNCONFIRMED` (428, with `error.details: {key_type, fingerprint}` to confirm through
`POST /servers/{id}/host-keys/trust`), `INTERNAL_ERROR` (500). `error.details` is present only
when a client can act on it. A lookup
answers 404 only when the row is missing or belongs to another user; a database failure is a
500 whose message carries the cause, and the server log has the same text under the request's
`request_id`.

Every response carries an `x-request-id` header (a UUID minted by the server; a value sent by
the client is discarded). The same id is the `request_id` field of the request's log span, so
a failure reported with its header can be found in `docker logs` directly.

## Health and status

| Method | Path | Auth | Response |
|---|---|---|---|
| GET | `/health` | no | `{"status":"ok","database":"ok","service":"svrctlrs","version":"3.3.0","revision":"<git sha>"}` — pings the database; `503` with `status: degraded` when it is unreachable. `revision` is baked into the container image; `unknown` outside CI builds |
| GET | `/status` | yes | `{"status":"running","version":"…","resources":{"servers":<count>}}` |

## Servers

| Method | Path | Body | Notes |
|---|---|---|---|
| GET | `/servers` | — | list |
| POST | `/servers` | `name`, `hostname`, `port` (default 22), `username?` (default: the credential's user, else `root`), `credential_id?`, `via_server_id?`, `tags?` (array of strings), `startup_command?` (one line, ≤500 chars), `record_sessions?` (bool, default false), `description?`, `enabled` (bool) | `is_local: true` is refused with 400: local shells were retired, SvrCtlRS connects over SSH only |
| POST | `/servers/import` | `config` (the text of an `ssh_config`, up to 256 KiB), `credential_id?`, `tags?` for every created server | creates a server per concrete `Host` block (`HostName` with `%h`, `Port`, `User`; `ProxyJump`'s first hop becomes `via_server_id` when it names a created or existing server by name, else by hostname); wildcard and negated patterns, `Match` blocks and names that already exist are skipped. Returns `created[] {id, name, hostname}`, `skipped[] {name, reason}`, `notes[]`. 400 when no `Host` block was found. Nothing is connected to |
| GET | `/servers/{id}` | — | |
| PUT | `/servers/{id}` | any subset of the create fields | omitted fields are kept; `""` clears `username`/`description`; `credential_id: null` unassigns the credential |
| DELETE | `/servers/{id}` | — | |
| POST | `/servers/{id}/test` | — | verifies the host key, authenticates with the server's credential and runs a trivial command; 200 with `success`, `message`, `latency_ms` and, on failure, `code` (`HOST_KEY_UNCONFIRMED` with `details: {key_type, fingerprint}`, `HOST_KEY_MISMATCH`, else `BAD_REQUEST`) |
| GET | `/servers/{id}/host-keys` | — | recorded host keys: `id`, `key_type`, `fingerprint` (`SHA256:…`, OpenSSH style), `first_seen_at`, `last_seen_at`, `trusted_at` (null until confirmed), `trusted` — never the key itself |
| POST | `/servers/{id}/host-keys/trust` | `fingerprint` | trust the key recorded on first contact whose fingerprint this is; returns `host_key`. 404 when no unconfirmed key with that fingerprint is waiting |
| DELETE | `/servers/{id}/host-keys` | — | forget every recorded key; the next connection records what it sees and asks for confirmation. Returns `removed` |
| POST | `/servers/{id}/authorize-key` | `credential_id` (an `ssh_key` with key material) plus `password` (one-time, never stored) or `bootstrap_credential_id` | installs the public key in `~/.ssh/authorized_keys` over the bootstrap login, verifies the key logs in, sets it as the server's credential. 400 with the reason otherwise |

Server object: `id, name, hostname, port, username, credential_id, via_server_id, tags,
startup_command, record_sessions, description, enabled, last_seen_at, last_error, created_at,
updated_at`. `record_sessions` records every interactive shell opened on the server from then
on, for the user who opens it (see "Recordings"); shells already running are not affected.
`startup_command` is typed into every new interactive shell on the server right after it
starts (it echoes like typed input); one line of at most 500 characters, 400 otherwise, an
empty string on update clears it. It does not apply to `/test`, SFTP or authorize-key. `tags` is an array of strings,
stored trimmed, unique ignoring case (first spelling wins), sorted, at most 20 of up to 32
characters; on update it replaces the whole list. `via_server_id` names a jump host:
the connection is made through that server's SSH session (`direct-tcpip`), so `hostname` is
resolved from the jump host. On create and update it must be an existing server other than the
server itself and must not lead back to it (400 otherwise; chains of up to four hops); `null`
on update makes the server direct again. Deleting a jump host makes its dependants direct.
A host-key refusal names the server whose key it is in `details.server_id`/`server_name`,
which is the jump host's when that is the key waiting for confirmation.

## Files (SFTP)

All under `/servers/{id}/files`; `path` is an absolute remote path, and an empty or missing
`path` means the login's home directory. Each call opens its own SSH connection with the same
host-key and credential rules as the shell, runs the `sftp` subsystem, and disconnects. These
routes are not subject to the 30 s request timeout, so an upload may take as long as it needs.

| Method | Path | Query / body | Response |
|---|---|---|---|
| GET | `/files` | `path?` | `path` (canonical), `parent` (null at `/`), `entries[]`: `name`, `kind` (`dir`, `file`, `symlink`, `other`), `size`, `modified` (RFC 3339 or null), `mode` (`rwxr-xr-x`), `owner`, `group` — directories first, then by name |
| GET | `/files/content` | `path` | the file's bytes, `Content-Disposition: attachment`, streamed; 400 for a directory |
| PUT | `/files/content` | `path`, raw body | creates or overwrites the file with the body, streamed; `{path, bytes}` |
| POST | `/files/mkdir` | `{path}` | creates one directory |
| POST | `/files/rename` | `{from, to}` | 409 when `to` exists |
| DELETE | `/files` | `path` | deletes a file or an *empty* directory (409 otherwise; recursion belongs in the shell); `{path, kind}` |

Errors: 400 for a relative path or a NUL byte (`Path must be absolute`), 404 when the remote
path does not exist, 403 when the login lacks permission, otherwise 400 carrying the SFTP
server's message. Audit actions: `files.download`, `files.upload` (`bytes`), `files.mkdir`,
`files.rename`, `files.delete` (`kind`); listings are not audited.

## Credentials

| Method | Path | Body |
|---|---|---|
| GET | `/credentials` | — (no secrets: `has_value`, `has_passphrase`, plus `key_source` (`key` \| `path`, SSH keys only), `fingerprint` (`SHA256:…` for keys with material), `in_use`, `used_by` (server names)) |
| POST | `/credentials` | `name`, `credential_type` (`password` \| `ssh_key` \| `api_token`), `value`, `description?`, `username?`, `passphrase?` |
| GET | `/credentials/{id}` | `has_value`, `has_passphrase`, `in_use`, `key_source` and `fingerprint` as in the list, and for keys with material `public_key` (the `authorized_keys` line) — never the secrets |
| PUT | `/credentials/{id}` | `name?`, `description?` (`""` clears), `value?`, `username?` (`""` clears), `passphrase?` (omit to keep, `""` to remove) |
| DELETE | `/credentials/{id}` | — |
| POST | `/credentials/generate-key` | `name`, `description?`, `comment?` → 201 `{id, public_key}`; an ed25519 key stored encrypted as an `ssh_key` credential |

For `ssh_key` credentials `value` is either the *private* key material (`-----BEGIN …`) or an
absolute path readable by the server (for example a key mounted at
`/home/svrctlrs/.ssh/id_ed25519`). A public key (`ssh-ed25519 …`), a `.pub` path or a relative
path is refused with 400 and a message saying why.
`passphrase` unlocks an encrypted key. Value and passphrase are both encrypted at rest.

## Settings

| Method | Path | Body |
|---|---|---|
| GET | `/settings` | — grouped by prefix; each setting has `key, value, value_type, description, updated_at, secret, is_set`. A secret (`notification.token`, `approval.matrix_token`, `approval.webhook_url`) is write-only: `value` is always `""`, `is_set` says whether one is stored |
| POST | `/settings` | `key`, `value`, `value_type` (`string` \| `integer` \| `boolean` \| `json`), `description?` |
| PUT | `/settings` | `{"values": {"key": "value", …}}` — upserts all in one request; a secret with an empty value is left untouched. Returns `updated` |
| GET | `/settings/{key}` | — |
| PUT | `/settings/{key}` | `{"value": "…"}` — validated against the stored `value_type` |

Seeded keys: `notification.enabled`, `notification.provider` (`gotify` \| `ntfy`),
`notification.url`, `notification.token`, `notification.topic`,
`notification.health_check_enabled`, `notification.health_check_interval_seconds`,
`app.default_profile_id`, `recording.retention_days` (whole days, 0–3650, default 30; `0`
keeps recordings until deleted; anything else is refused with 400).

## Recordings (per user)

Recordings of the calling user's shells on servers with `record_sessions` set. Another
user's recording answers 404 on every route, like a missing one.

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/recordings` | `server_id?`, `limit` (1–500, default 100), `before` (id cursor) | `recordings` newest first, `retention_days`, `max_bytes` (the size limit per recording) |
| GET | `/recordings/{id}` | — | `recording` |
| GET | `/recordings/{id}/cast` | `download?` (`true` adds `Content-Disposition: attachment; filename*=…<server>-<YYYYMMDD-HHMMSS>.cast` and an audit entry) | the recording as [asciicast v2](https://docs.asciinema.org/manual/asciicast/v2/), `Content-Type: application/x-asciicast`, `Cache-Control: private, no-store`, streamed. A recording still being written returns what has been written so far (flushed every second) |
| DELETE | `/recordings/{id}` | — | deletes the row and the file; 409 while the shell is still being recorded (end its session first) |

Recording object: `id, user_id, server_id` (null once the server is deleted)`, server_name,
session_id, cols, rows` (the size at the start)`, started_at, ended_at, duration_ms,
size_bytes` (of the asciicast, i.e. of a download)`, status, end_reason`. `status` is
`recording`, `complete`, `truncated` (stopped at `RECORDING_MAX_MB`; the shell went on),
`interrupted` (the server stopped while it ran) or `failed` (writing failed; `end_reason`
says why). The asciicast has a header (`version: 2`, `width`, `height`, `timestamp`, `title`
= the server's name, `env.TERM`) and events `[seconds, "o", text]` for output, `[seconds,
"r", "COLSxROWS"]` for resizes and `[seconds, "m", label]` for markers (where output was lost
or the size limit cut it off). Times count from the shell's start. Only output is recorded,
never input.

## Terminal profiles (per user)

| Method | Path | Body |
|---|---|---|
| GET | `/profiles` | — |
| POST | `/profiles` | `name`, `layout` (`single` \| `split-h` \| `split-v` \| `quad`), `description?`, `pane_configs?` (array of `{server_id?}`; a legacy `mode` key is accepted and ignored), `is_default` |
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
| GET | `/users` | — | `id`, `username`, `is_me`, `last_login_at` (from the audit log, may be null), timestamps; never hashes |
| POST | `/users` | `username` (1–64 chars), `password` (≥ 12 chars) | 409 if the name exists |
| DELETE | `/users/{id}` | — | refused for your own account and for the last account |
| PUT | `/users/me/password` | `current_password`, `new_password` (≥ 12 chars) | 401 when the current password is wrong |
| GET | `/users/me/preferences/{key}` | — | your own document: `{key, value, updated_at}`; `value` is null until you save one |
| PUT | `/users/me/preferences/{key}` | `value` (a JSON object, ≤ 16 KiB) | replaces it; 400 for anything but an object or when too big |

The only preference key is `terminal` (the terminal's font, cursor, scrollback, scrolling,
leader key and predictive echo; `ui/src/lib/terminal/prefsSync.ts` has the fields and their
defaults); any other key answers 404. The server stores the document as given and the UI
merges it over its defaults, so fields a client does not know are ignored rather than fatal.

Deleting a user cascades to their sessions, profiles, quick commands, preferences,
recordings and API tokens.

## API tokens (signed-in users only)

| Method | Path | |
|---|---|---|
| GET | `/tokens` | the caller's tokens, newest first: `{tokens: [...]}` |
| POST | `/tokens` | `{name, preset, expires_in_days}` → 201 `{token, secret}` |
| PUT | `/tokens/{id}` | `{require_approval}` → `{token}` |
| POST | `/tokens/{id}/revoke` | → `{token}` (idempotent); terminals opened with it close |
| DELETE | `/tokens/{id}` | remove it (any state) → `{id, message}` |

```json
POST /api/v1/tokens
{"name": "hermes", "preset": "servers_keys", "expires_in_days": 90}

201 {"token": {"id": 3, "name": "hermes", "prefix": "svr_x7Kq2m", "preset": "servers_keys",
               "status": "active", "created_at": "…", "expires_at": "…",
               "last_used_at": null, "last_used_ip": null, "revoked_at": null},
     "secret": "svr_x7Kq2m…"}
```

`expires_in_days` is required: 1–3650, or `null` for a token that never expires.
`require_approval` (default `true`) makes the three gated deletes wait for the owner. `name` is
1–64 characters and unique per user (409). `status` is `active`, `expired` or `revoked`. The
`secret` appears in the create response only. Audited as `token.created`, `token.revoked`,
`token.deleted`. Another user's token answers 404.

## Approvals

A token with `require_approval` that calls one of the gated deletes gets:

```json
202 Location: /api/v1/approvals/<id>?wait=25
{"approval": {"id": "kq3…", "status": "pending", "action": "server.delete",
              "summary": "Delete server web-01", "method": "DELETE", "path": "/api/v1/servers/5",
              "target_type": "server", "target_id": 5, "target_name": "web-01",
              "token_id": 3, "token_name": "hermes", "client_ip": "10.10.1.35",
              "url": "https://svrctlrs.example/approvals?id=kq3…",
              "created_at": "…", "expires_at": "…", "decided_at": null,
              "result_status": null, "result": null},
 "message": "Delete server web-01 needs the owner's approval: …",
 "wait_url": "/api/v1/approvals/kq3…?wait=25"}
```

| Method | Path | Who | |
|---|---|---|---|
| GET | `/approvals` | signed in | the caller's requests, newest first (50): `{approvals, waiting}` |
| GET | `/approvals/{id}` | signed in, or any token of the owner | one request; `?wait=N` (≤ 25) answers as soon as it is decided |
| POST | `/approvals/{id}/approve` | signed in | runs the held call now, as the token → the request with `result_status` and `result` (the call's JSON) |
| POST | `/approvals/{id}/deny` | signed in | → the request, `denied`; nothing runs |
| POST | `/notifications/test-approval` | signed in, full admin | sends a sample request through the channel; body overrides the saved `channel`, `webhook_url`, `matrix_homeserver`, `matrix_room`, `matrix_token` |

`status`: `pending`, `expired` (an hour passed), `denied`, `approved` (running), `done` (the
call answered 2xx) or `failed` (it did not, or the token was revoked or removed first: 410,
nothing ran). Deciding twice, or after expiry, is 409. Settings: `approval.channel` (`push`,
`webhook`, `matrix`, `none`), `approval.webhook_url` and `approval.matrix_token` (write-only),
`approval.matrix_homeserver`, `approval.matrix_room`, `approval.link_base` (where links start;
empty: the address the call came to). The webhook receives `{title, message, url, approval}`.

## Operations

Task-level calls for agents (and the Servers page). They run on svrctlrs, from its container,
with the stored credentials: requests name credentials, never carry a password or private
key. All four live outside the 30 s request timeout.

| Method | Path | Presets | |
|---|---|---|---|
| POST | `/servers/{id}/diagnose` | servers & keys, full admin | → `Diagnosis`: `ok`, `verdict`, and per hop (jump hosts first) the steps `dns`, `tcp` (or `tunnel` behind a jump host), `handshake`, `host_key`, `login`, each `ok`/`failed`/`skipped` with `detail`, `fix` and `duration_ms` |
| POST | `/servers/{id}/exec` | full admin | `{command, timeout_secs?}` (1–600, default 60) → `{exit_code, stdout, stderr, stdout_truncated, stderr_truncated, timed_out, duration_ms}`; 1 MiB per stream; `exit_code` null after a timeout |
| POST | `/servers/onboard` | servers & keys, full admin | `{name, hostname, port?, username?, credential_id, via_server_id?, tags?, fingerprint?, new_key?, dry_run?}` → `{server_id, done, steps, next}` |
| POST | `/servers/{id}/rotate-key` | servers & keys, full admin | `{remove_old?, dry_run?}` → `{done, old_credential_id, new_credential_id, steps, next}` |

**diagnose** behaves like the connection test (a first contact records the presented key as
unconfirmed; the server's `last_seen_at`/`last_error` are updated) and is audited as
`server.diagnosed`. **exec** refuses an unconfirmed or mismatched host key like a shell (428 /
409) and is audited as `server.exec` with the command as typed (first 500 characters): keep
secrets out of command lines. **onboard** creates the server with `credential_id` as the way
in, or picks up a server of the same name and hostname; it stops with `next` when it needs
you: on first contact it returns the fingerprint (`host_key` step `waiting`) and trusts it only
when called again with that `fingerprint`, a different one fails. With `new_key` (default) it
generates `"<name> key"`, installs it with the way in, proves it and switches to it. **rotate-key**
needs a server that logs in with a stored key: it checks the key, generates
`"<name> key <date>"`, installs it with the old key, proves it, switches, and (with
`remove_old`, default) removes the old public key from `authorized_keys`; the old credential
is kept and `next` says whether other servers still use it. `dry_run` on onboard and
rotate-key lists the steps as `planned` and changes nothing. Step statuses: `ok`, `failed`,
`skipped`, `waiting`, `planned`.

## Audit log

| Method | Path | Query | Response |
|---|---|---|---|
| GET | `/audit` | `limit` (1–500, default 100), `before` (id cursor), `action` (comma-separated prefixes, e.g. `login,session.`), `user_id`, `q` (case-insensitive substring of action, target name, details, username, address or token name) | `entries` newest first, `next_before` for the following page or `null` |

Entry: `id, at, user_id, username, client_ip, action, target_type, target_id, target_name,
details` (a JSON object string), `token_id, token_name` (the API token the request came in
with, else null; the name as it was then). Actions: `login`, `login.failed`, `login.rate_limited`,
`logout`, `user.created`, `user.deleted`, `user.password_changed`, `server.created`,
`server.updated`, `server.deleted`, `server.imported` (details: `created` names, `skipped`
count), `server.host_key_trusted` (details: `key_type`,
`fingerprint`), `server.host_keys_forgotten` (details: `removed`), `credential.created`,
`credential.updated`, `credential.deleted`, `session.pty_opened` (details: `login`, `auth` —
`password` or the key used, `recorded`), `files.download`, `files.upload`, `files.mkdir`, `files.rename`,
`files.delete` (details: the remote path), `recording.downloaded`, `recording.deleted`,
`server.diagnosed` (details: `ok`, `verdict`), `server.exec` (details: `command`, `exit_code`,
`timed_out`, `duration_ms`), `server.onboarded`, `server.key_rotated` (details: the old and new
credential ids),
`token.created` (details: `preset`, `expires_at`, `prefix`, `require_approval`), `token.updated`,
`token.revoked`, `token.deleted`, `approval.requested` (names the token), `approval.approved`,
`approval.denied` (details: `approval`, `action`, `token`).
`server.created` and `server.updated` carry `record_sessions` when it was set or changed. `session.command` entries from the retired command mode remain
readable until they age out. Entries never contain secrets, are kept for 180 days, and outlive
deleted users.

## Notifications

| Method | Path | Body | Effect |
|---|---|---|---|
| POST | `/notifications/test` | optional `provider?`, `url?`, `token?`, `topic?` | sends a test message; with a body, those values override the stored settings (an empty `token` means the stored one), so a form can be tested before saving |

## WebSocket protocol

Both endpoints are JSON text frames. Authenticate with the session cookie (browser) or a
one-time `?ticket=` (native apps). The upgrade's `Origin` must be the server's own host or one
of `ALLOWED_ORIGINS`, else `403`.

### `/ws/terminal/pty` — interactive shell

Client → server:

| `type` | Fields | Meaning |
|---|---|---|
| `shell` | `server_id`, `cols`, `rows`, `binary?` | open a PTY on the server; answered with `connected` carrying a `session_id` |
| `attach` | `session_id`, `last_seq`, `cols?`, `rows?`, `binary?` | resume a session this user owns; the server replays output after `last_seq` |
| `input` | `data` | keystrokes |
| `resize` | `cols`, `rows` | terminal resized |
| `ping` | — | keepalive |
| `detach` | — | drop the connection, keep the shell (the same happens when the socket just closes) |
| `close` | — | end the shell |

Server → client:

| `type` | Fields | Meaning |
|---|---|---|
| `connected` | `session_id`, `data`, `recording` | the shell is up (`shell`) or reattached (`attach`); `recording` says whether its output is being recorded (a new shell's `data` also says so, or why recording failed) |
| `output` | `seq`, `data` | terminal output; `seq` increases by one per frame. Only for clients that did not send `binary: true` (see below) |
| `replay_start` / `replay_end` | — | brackets the frames resent after an `attach` |
| `replay_window_lost` | — | the client was further behind than the ring holds, on attach or later (it stopped reading during a burst); redraw from scratch, the whole ring follows |
| `closed` | `data` | the shell ended (exit, `close`, TTL expiry, server shutdown); the socket stays open but the session is gone |
| `error` | `data`, `code?` | `code: "session_gone"` — the id is unknown or expired, send `shell`; `code: "quota"` — too many sessions |
| `pong` | — | — |

**Binary output.** With `binary: true` on `shell`/`attach`, output is not wrapped in JSON:
each frame is a binary WebSocket message of 8 bytes big-endian `seq` followed by the PTY's raw
bytes. The terminal decodes the stream itself, so a multibyte character split across two SSH
reads arrives intact (the JSON form converts per frame, lossily). All other messages in both
directions stay JSON text. The web UI always asks for binary; the JSON form remains for older
clients.

Sessions live in the server, not in the socket. A session with no attached client is kept for
`SESSION_DETACH_TTL_SECS` (default 900) and then closed; a user may have at most
`SESSION_MAX_PER_USER` (default 16). The replay ring holds the last 4096 frames or 2 MiB of
output per session. Live sessions of the calling user are listed by `GET /api/v1/sessions`
(`sessions[]` with `id`, `server_id`, `server_name`, `created_at`, `attachments` — sockets
attached right now, 0 while detached — `latest_seq` and `recorded`, oldest first, plus `detach_ttl_secs`;
ended sessions are not listed) and ended by `DELETE /api/v1/sessions/{id}` (404 for an unknown
id or another user's session; every attached socket receives `closed`). The terminal UI's
**Sessions** panel is built on these two calls.

Host-key handling: the first connection to a server stores its key; a
later connection presenting a key that matches none stored for that server is refused with an
`error` frame explaining the mismatch. Profiles and quick commands are per user: another
user's id answers 404.
