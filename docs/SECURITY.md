# Security

SvrCtlRS hands an authenticated user an interactive shell on every server it knows about, and
on the container itself through a `local` server. Treat the SvrCtlRS login as equivalent to
root on those hosts, and the server's SQLite file plus encryption key as equivalent to their
SSH credentials.

This document describes the model as implemented in 3.0.0 and lists the known weaknesses so
they are not forgotten. Fixes are tracked in `ROADMAP.md` (P0 Security).

## Threat model

| Asset | Protected by | Exposed to |
|---|---|---|
| Shell access to servers | SvrCtlRS login (argon2 password), edge passkey login when deployed behind tinyauth | anyone who can authenticate or steal a session |
| Stored credentials (passwords, key material) | AES-256-GCM with `ENCRYPTION_KEY` | anyone with the DB *and* the key, or with a valid session (the server decrypts on use) |
| Server inventory, host keys | database file permissions (uid 1000 in the container) | anyone with the volume |
| Sessions / tokens | SQLite `tower_sessions`, 24 h inactivity expiry | anyone with the DB; the client that holds the token |

Assumed: TLS terminates at a reverse proxy; the SvrCtlRS container is on a private Docker
network; the operator controls the host. Not assumed: trustworthy browser extensions or
other web pages the operator visits (see CSWSH below).

## Authentication

- **Users** — `users` table, argon2 (default parameters). The first start seeds one admin from
  `ADMIN_USERNAME`/`ADMIN_PASSWORD` when the table is empty; later starts never overwrite.
  Passwords must be at least 12 characters. Failed logins are limited to 10 per 15 minutes
  per client address and per username (429 with `Retry-After`). Accounts are managed under
  Settings → Users (`/api/v1/users`); a password change requires the current password. Every
  account is an administrator, so put the UI behind the edge passkey login as well.
- **Sessions** — tower-sessions, SQLite store, `HttpOnly`, `Secure` unless
  `SESSION_SECURE=false`, `SameSite=Strict` by default; `SameSite=None` only when
  `ALLOWED_ORIGINS` is set for the native clients. Expiry: 24 h of inactivity. Logout flushes
  the session.
- **Token auth for native clients** — `POST /api/v1/auth/login` returns the session id; the
  Tauri apps send it as `Authorization: Bearer`. For WebSockets (no headers on a handshake)
  they first obtain a one-time ticket from `POST /api/v1/auth/ws-ticket` and pass `?ticket=`.
  The user is always read from the session record, never from the client.
- **Exempt routes** — `/auth/login`, `/auth/logout`, `/api/v1/auth/login`, `/api/v1/health`,
  `/_app/*` and `OPTIONS` preflights. `/api/v1/health` reveals only version and revision.

## Credential encryption

`core/src/encryption.rs`: AES-256-GCM, a fresh 12-byte nonce from the OS RNG per encryption,
ciphertext stored as `nonce || ciphertext` (base64) with `credentials.encrypted = 1`; the
metadata column (SSH key passphrase) is encrypted the same way (`metadata_encrypted = 1`).
Key source, in order: `ENCRYPTION_KEY` (64 hex chars) → `data/encryption.key` (created with
mode 0600 if absent). An invalid value (including an empty string) stops the server; rows that
an older server wrote in plaintext are encrypted at the next start. API responses never return
credential values: lists show `[REDACTED]`, single reads return `has_value`.

Back the key up with the database. There is no key rotation; changing the key makes every
stored credential undecryptable.

## SSH host keys

Trust on first use. The first connection to a server stores its host key in
`server_host_keys`; a later connection presenting a key that matches none of the keys stored
for that server — whatever its algorithm — is refused with an error in the terminal. The same
decision (`server/src/host_keys.rs`) runs inside the one SSH client (`server/src/ssh.rs`) that
the shell, the connection test and "Authorize key" all use, and a database error refuses the
connection. Stored keys are listed with
OpenSSH-style fingerprints and can be forgotten per server (Servers → Keys,
`/api/v1/servers/{id}/host-keys`); forgetting is logged at `warn`.

## Transport and headers

The server sets `Content-Security-Policy` (`default-src 'self'`, with `'unsafe-inline'`
scripts for the SvelteKit bootstrap and `wasm-unsafe-eval` for the xterm image addon),
`Referrer-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`. HTTP requests
time out after 30 s; WebSockets do not. TLS is the proxy's job.

## Deployment guidance

- Put the UI behind the edge passkey login (`secure-auth-chain@file` in the reference compose)
  so a password alone is never enough from the internet. The API/WS router is headers-only
  because the native apps need it; see the CSWSH item below for why `ALLOWED_ORIGINS` should
  stay empty on browser-only deployments.
- Mount a dedicated key directory, not your `~/.ssh`, if you use key-path credentials.
- Keep `.env` at mode 0600; it holds `ENCRYPTION_KEY` and the bootstrap password.
- Prefer `ssh_key` credentials with the key material pasted into the credential (stored
  encrypted, passphrase included) over key *paths*, which encrypt only the path string and
  leave the key on the host filesystem.

## Known weaknesses (3.0.0)

Ordered by impact. Each has a ROADMAP entry.

1. ~~**Cross-site WebSocket hijacking**~~ — fixed after 3.0.0: both WebSocket upgrades
   refuse an `Origin` that is neither the request's own host (or `X-Forwarded-Host`) nor one
   of `ALLOWED_ORIGINS`, with 403. Requests without `Origin` (non-browser clients) fall
   through to authentication. Browsers always send `Origin` on a handshake, so a malicious
   page can no longer ride the operator's cookie into `wss://host/ws/terminal/pty`.
2. ~~**Client-asserted identity on token auth**~~ — fixed after 3.0.0: a Bearer or `?token=`
   value is resolved through the session store, which only returns unexpired records, and the
   user id is read from that record. `X-User-Id` and `?user_id=` are ignored. Every
   authenticated request carries a `CurrentUser` extension; handlers take it as an extractor
   instead of reading the session themselves.
3. ~~**Token in URLs**~~ — fixed after 3.0.0: native apps exchange their token for a one-time
   ticket (`POST /api/v1/auth/ws-ticket`, 30 s, consumed on first use) and put only that in
   the WebSocket URL; the request span logs a redacted URI (`token`, `ticket` and `user_id`
   query values are replaced). `?token=` is still accepted for
   older app builds and is deprecated; remove it once the 3.x APK is on every device.
4. ~~**Token in `localStorage`**~~ — fixed after 3.0.0 for the browser: the web UI no longer
   stores the session id (the HttpOnly cookie is the only credential) and nothing sends a
   user id. The Tauri apps still keep their token in `localStorage`; moving it to the
   platform keystore is a P1 item.
5. ~~**Host-key check is per key type**~~ — fixed after 3.0.0: one shared decision
   (`server/src/host_keys.rs`) compares the presented key against *every* key stored for the
   server; an unseen key type with keys on file is a mismatch, not a first contact. Both
   connectors fail closed on a database error. A legitimately changed host key now needs an
   administrator to delete the server's rows in `server_host_keys` (UI pending, ROADMAP).
6. ~~**`POST /servers/{id}/test` skips host-key verification**~~ — fixed after 3.0.0: the
   connection test goes through the shared SSH client, which verifies the host key during the
   handshake and supports password credentials.
7. ~~**`local` servers are container RCE for any user**~~ — fixed 2026-10-18: command mode,
   the only path that executed anything locally, is gone. `POST /servers` refuses
   `is_local: true` (400) and the SSH client refuses to connect to a legacy local row; the
   column stays for existing databases.
8. **No roles, no 2FA** — every user is equal and can manage accounts. Login attempts are
   rate limited (10 failures per 15 minutes per client address *and* per username, 429 with
   `Retry-After`; in-memory, so a restart resets it), new passwords must be at least 12
   characters, and an **audit log** (Settings → Audit, `/api/v1/audit`) records logins,
   sessions, commands and changes with the client address, kept 180 days.
9. ~~**`credentials.metadata` is plaintext**~~ — fixed after 3.0.0: metadata (the SSH key
   passphrase) is encrypted like the value, with its own `metadata_encrypted` flag, and the
   start-up pass encrypts any legacy plaintext metadata. `Debug` output redacts it.
10. ~~**Per-record access**~~ — fixed after 3.0.0: `get`/`update`/`delete` for profiles and
    quick commands are scoped to the calling user (profiles with no owner stay shared), and a
    foreign id answers 404, not 403, so ids cannot be probed. Setting a default profile no
    longer clears other users' defaults.
11. ~~**Session id is not rotated on login; no CSRF guard on the HTML forms**~~ — fixed after
    3.0.0: both login handlers cycle the session id, and the HTML login and logout routes
    refuse cross-site requests (`Sec-Fetch-Site` other than `same-origin`/`none`, or an
    `Origin` that is not this host) with 403. The JSON routes are protected by their
    `Content-Type` preflight.
12. ~~**Plaintext fallback**~~ — fixed after 3.0.0: the server refuses to start when the key
    cannot be initialised, the database layer refuses to store or return a secret without a
    key, and start-up encrypts any `encrypted = 0` rows left by an earlier plaintext run.
    (Production ran in that fallback from 2026-09-14 to 2026-10-04 because the compose file
    passed an empty `ENCRYPTION_KEY`.)

## Reporting

This is a personal project without a disclosure programme. Open an issue at
`git.js-node.cc/jsprague/svrctlrs` or contact the maintainer directly.
