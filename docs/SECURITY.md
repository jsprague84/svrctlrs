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
  There is no user-management API or password-change flow yet, and no password policy, so
  pick a strong admin password and put the UI behind the edge passkey login.
- **Sessions** — tower-sessions, SQLite store, `HttpOnly`, `Secure` unless
  `SESSION_SECURE=false`, `SameSite=Strict` by default; `SameSite=None` only when
  `ALLOWED_ORIGINS` is set for the native clients. Expiry: 24 h of inactivity. Logout flushes
  the session.
- **Token auth for native clients** — `POST /api/v1/auth/login` returns the session id; the
  Tauri apps send it as `Authorization: Bearer` with `X-User-Id`, and as `?token=&user_id=`
  on WebSocket upgrades (browsers cannot set headers on a WebSocket handshake).
- **Exempt routes** — `/auth/login`, `/auth/logout`, `/api/v1/auth/login`, `/api/v1/health`,
  `/_app/*` and `OPTIONS` preflights. `/api/v1/health` reveals only version and revision.

## Credential encryption

`core/src/encryption.rs`: AES-256-GCM, a fresh 12-byte nonce from the OS RNG per encryption,
ciphertext stored as `nonce || ciphertext` (base64) with `credentials.encrypted = 1`.
Key source, in order: `ENCRYPTION_KEY` (64 hex chars) → `data/encryption.key` (created with
mode 0600 if absent). API responses never return credential values: lists show
`[REDACTED]`, single reads return `has_value`.

Back the key up with the database. There is no key rotation; changing the key makes every
stored credential undecryptable.

## SSH host keys

Trust on first use. The first connection to a server stores its host key in
`server_host_keys` keyed by `(server_id, key_type)`; later connections presenting a different
key of that type are refused with an error in the terminal. This applies to both terminal
modes. There is no UI to view or revoke stored keys yet (delete the row to re-trust).

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
- Prefer `ssh_key` credentials with the key material stored in the credential (encrypted)
  over key *paths*, which encrypt only the path string.

## Known weaknesses (3.0.0)

Ordered by impact. Each has a ROADMAP entry.

1. **Cross-site WebSocket hijacking** — neither WebSocket handler checks the `Origin` header.
   With `ALLOWED_ORIGINS` set the session cookie is `SameSite=None`, so a malicious page the
   operator visits can open `wss://host/ws/terminal/pty` with the operator's cookie and get a
   shell. Mitigation today: leave `ALLOWED_ORIGINS` empty on browser-only deployments
   (cookie stays `Strict`), or keep tinyauth in front of `/ws/`. Fix: reject upgrades whose
   `Origin` is not the server's own origin or an allowed origin.
2. **Client-asserted identity on token auth** — the Bearer path verifies that the session id
   exists, then trusts the client-supplied `X-User-Id`/`user_id` and writes it into the
   session. Any valid token lets its holder act as another user for profiles and quick
   commands. Also, the token lookup does not check `expiry_date`, so expired sessions remain
   valid tokens until the 5-minute sweeper deletes them. Fix: read `user_id` from the
   session record, filter by expiry.
3. **Token in URLs and logs** — the WebSocket token travels in the query string and the
   request tracing span records the full URI at `info`. Fix: redact `token`/`user_id` in the
   span, and move to a short-lived one-time WebSocket ticket.
4. **Token in `localStorage`** — the web UI stores the session id and user id in
   `localStorage` (not only the Tauri apps), which the CSP's `'unsafe-inline'` makes reachable
   to any injected script. Fix: cookies only in the browser; secure storage on devices.
5. **Host-key check is per key type** — a man-in-the-middle presenting a different key
   *algorithm* than the stored one is TOFU-accepted. The PTY path also fails open on a
   database error. Fix: compare against all stored keys for the server; fail closed.
6. **`POST /servers/{id}/test` skips host-key verification** — it uses the legacy
   `server/src/ssh.rs` with `ServerCheckMethod::NoCheck`. Fix: route it through the TOFU
   path and delete `ssh.rs`.
7. **`local` servers are container RCE for any user** — `is_local: true` runs `sh -c` as the
   server process. Acceptable for a single-operator install; needs a role or an explicit
   server-side opt-in before multi-user use.
8. **No roles, no rate limiting, no 2FA, no audit log** — every user is equal; login attempts
   are unlimited (the edge passkey login is the compensating control today).
9. **`credentials.metadata` is plaintext** — it carries SSH key passphrases. Fix: encrypt it
   or move the passphrase into `value`.
10. **Per-record access** — `get`/`update`/`delete` for profiles and quick commands are not
    scoped by `user_id`; only `list` is.
11. **Session id is not rotated on login** (fixation via the cookie path), and form login /
    logout are simple requests with no CSRF token.
12. **Plaintext fallback** — if encryption initialisation fails the server logs a warning and
    stores new credentials unencrypted rather than refusing to start.

## Reporting

This is a personal project without a disclosure programme. Open an issue at
`git.js-node.cc/jsprague/svrctlrs` or contact the maintainer directly.
