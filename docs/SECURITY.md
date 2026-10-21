# Security

SvrCtlRS hands an authenticated user an interactive shell on every server it knows about, and
on the container itself through a `local` server. Treat the SvrCtlRS login as equivalent to
root on those hosts, and the server's SQLite file plus encryption key as equivalent to their
SSH credentials.

This document describes the model as implemented in 3.0.0 and lists the known weaknesses so
they are not forgotten. Fixes are tracked in `ROADMAP.md` (History, P0 Security).

## Threat model

| Asset | Protected by | Exposed to |
|---|---|---|
| Shell access to servers | SvrCtlRS sign-in: an OpenID Connect provider (the reference deployment: Pocket ID passkeys, password sign-in off), or an argon2 password with optional TOTP behind a login proxy | anyone who can authenticate or steal a session |
| Stored credentials (passwords, key material) | AES-256-GCM with `ENCRYPTION_KEY` | anyone with the DB *and* the key, or with a valid session (the server decrypts on use) |
| Server inventory, host keys | database file permissions (uid 1000 in the container) | anyone with the volume |
| Sessions / tokens | SQLite `tower_sessions`, 24 h inactivity expiry | anyone with the DB; the client that holds the token |
| API tokens (agents, scripts) | only a SHA-256 hash stored; access preset; optional expiry; revocable | whoever holds the secret, within its preset, until it expires or is revoked |
| Session recordings (opt-in) | AES-256-GCM with `ENCRYPTION_KEY`; owner-only access | anyone with the files *and* the key; the owner's session |

Assumed: TLS terminates at a reverse proxy; the SvrCtlRS container is on a private Docker
network; the operator controls the host. Not assumed: trustworthy browser extensions or
other web pages the operator visits (see CSWSH below).

## Authentication

- **Users** — `users` table, argon2 (default parameters). The first start seeds one admin from
  `ADMIN_USERNAME`/`ADMIN_PASSWORD` when the table is empty; later starts never overwrite.
  Passwords must be at least 12 characters. Failed logins are limited to 10 per 15 minutes
  per client address and per username (429 with `Retry-After`); the failures are kept in the
  database (`login_failures`), so a restart or a deploy does not reset the count, and an
  attempt is refused when the limiter cannot be read. Accounts are managed under
  Settings → Users (`/api/v1/users`); a password change requires the current password. Every
  account is an administrator, so put the whole app behind the edge passkey login as well
  ("Deployment guidance"). Lockout
  recovery is `svrctl reset-password <user>` on the host (it opens the database directly, so
  it needs `docker exec`, never the network): it ends that account's sessions (browser and
  native app) and is audited as `user.password_reset`. API tokens are separate credentials and
  survive it, as they survive a password change; revoke them under Settings → Access.
- **Sign-in with an OpenID Connect provider** (optional, `OIDC_ISSUER`; DEPLOYMENT.md) —
  the authorization code flow with PKCE (S256), `state` and `nonce`, run by the server itself
  (`server/src/sign_in/oidc.rs`). A flow lives ten minutes in memory under its `state` and is
  bound to the browser that started it by a random value in its own `HttpOnly`,
  `SameSite=Lax` cookie (`svrctlrs_oidc`, path `/auth/oidc`), compared in constant time; it is
  used up by the first callback, right or wrong. The ID token comes straight from the token
  endpoint over TLS and is still checked in full: signature against the provider's published
  keys (RS256, ES256 or EdDSA only, each with a key of its own type, so `none`, HMAC and
  key-type confusion are refused; RSA keys under 2048 bits are not trusted; the
  verification is the `ssh-key` code russh already uses, `sign_in/jwt.rs`), `iss`, `aud` and
  `azp`, `exp` and `iat` with a minute's leeway, the flow's `nonce`, a subject, and the
  callback's `iss` parameter (RFC 9207) when sent. `OIDC_ALLOWED_GROUPS` adds a group check.
  The account is found by `(issuer, subject)` only, never by e-mail address, and nothing is
  ever created: an identity is linked by its account while signed in (Settings → Access →
  Sign-in → Link, a same-origin navigation; one identity belongs to one account) and an
  unlinked one is refused (`login.oidc_unlinked`). Failed callbacks count against the login
  limiter by address. After the callback the browser gets a small page that moves on to the
  app instead of a redirect, so the new `SameSite=Strict` session cookie is sent. Audit:
  `login` with `method: oidc`, `user.identity_linked`, `user.identity_unlinked`,
  `login.oidc_refused` (group). Signing out lands on `/auth/login?signed_out=1`, which does not
  send you straight back to the provider.
- **`PASSWORD_LOGIN=off`** refuses every password sign-in (`/auth/login`, its TOTP step and
  `/api/v1/auth/login` answer 403, the latter `PASSWORD_LOGIN_OFF`), so the provider's
  sign-in — a passkey, at Pocket ID — is the only way in. It needs a provider (the server does
  not start otherwise), the last linked identity cannot be unlinked, and break-glass is the
  environment flag on the host plus `svrctl reset-password`. The setting is read at start-up
  only: no session or token can change it. API tokens are not affected.
- **Two-factor sign-in (TOTP)** — optional per account (Settings → Access → Sign-in):
  RFC 6238, HMAC-SHA1, 6 digits, 30 s, one step of clock drift either way. Turning it on
  takes the current password, shows the secret (and a QR code) once, and takes effect only
  when a code from the app is confirmed; that answer carries ten one-time recovery codes
  (60 random bits each, stored as SHA-256 hashes). The secret is stored encrypted with
  `ENCRYPTION_KEY`, like a credential. With it on, a right password only starts a five-minute
  second step (`/auth/login/totp`) in a fresh session that signs nobody in; a code is accepted
  once (the last accepted time step is kept, and claimed atomically), five wrong codes end the
  step, and every wrong code counts against the login limiter like a wrong password. The JSON
  login takes the code as `totp` and answers `401 TOTP_REQUIRED` / `TOTP_INVALID`. Turning it
  off and new recovery codes take the password too. The audit records how each sign-in was
  made (`login` with `method`: `password`, `password+totp`, `password+recovery_code`),
  `login.code_failed`, `user.totp_enabled` / `user.totp_disabled` and
  `user.recovery_codes_renewed`. A lost authenticator without recovery codes:
  `svrctl reset-password <user> --clear-totp` on the host.
- **Re-entering the password** (changing it, two-factor changes) is checked against the
  login limiter, and a wrong one answers `403 PASSWORD_INCORRECT`, so a hijacked session cannot
  guess it at leisure and the web app does not mistake the answer for "signed out".
- **No API token reaches any of this.** No preset names `/api/v1/users/me/sign-in`,
  `/users/me/totp…` or `/users/me/identities/…` (tested for every preset).
- **Sessions** — tower-sessions, SQLite store, `HttpOnly`, `Secure` unless
  `SESSION_SECURE=false`, `SameSite=Strict` by default; `SameSite=None` only when
  `ALLOWED_ORIGINS` is set for the native clients. Expiry: 24 h of inactivity. Logout flushes
  the session.
- **Token auth for native clients** — `POST /api/v1/auth/login` returns the session id; the
  Tauri apps send it as `Authorization: Bearer`. For WebSockets (no headers on a handshake)
  they first obtain a one-time ticket from `POST /api/v1/auth/ws-ticket` and pass `?ticket=`.
  The user is always read from the session record, never from the client. Behind a passkey
  edge like the reference one these apps cannot sign in (they cannot complete the passkey
  redirect); signing in through the system browser is ROADMAP item 22.
- **API tokens** (agents and scripts: `svrctl`, Hermes, Claude Code) — see "API tokens" below.
- **Exempt routes** — `/auth/login`, `/auth/login/totp`, `/auth/oidc/start`,
  `/auth/oidc/callback`, `/auth/logout`, `/api/v1/auth/login`, `/api/v1/health`,
  the static app shell (`/_app/*`, `/icons/*`, `/fonts/*`, `/favicon.svg`,
  `/service-worker.js`), `/manifest.webmanifest` and `OPTIONS` preflights. `/api/v1/health`
  reveals only version and revision. The manifest is served by the server
  (`routes/ui/manifest.rs`): signed out it is the shipped file; with a session cookie it adds
  shortcuts naming the user's own three most recently opened servers (`Cache-Control:
  private, no-cache`, `Vary: Cookie`), read from that user's audit rows only.

## API tokens

Named credentials the owner creates under Settings → Access → Agent tokens
(`server/src/api_tokens.rs`, `routes/api/tokens.rs`, table `api_tokens`):

- **Format and storage.** `svr_` plus 40 random letters and digits (about 238 bits from the OS
  RNG). The secret is shown once, in the response that creates it; the database keeps only its
  SHA-256 (a slow password hash adds nothing for a random secret but latency on every request)
  and the first ten characters (`svr_` + 6) so a list can tell tokens apart. Lists never
  return a secret or a hash.
- **Use.** `Authorization: Bearer svr_…` only. A token in a URL (`?token=svr_…`) is refused
  (`401 TOKEN_IN_URL`) so it never lands in proxy or access logs; a WebSocket takes a one-time
  ticket instead (`POST /api/v1/auth/ws-ticket` with the token), and the ticket carries the
  token's identity and preset to the upgrade, where the token is checked again (revoked in
  the 30 s between: refused). A WebSocket client that can set headers may instead send the
  token on the handshake itself, which is what an agent does behind the reference edge.
- **A token request is the token's alone.** The auth layer looks at an API token before
  anything else, so a session cookie sent with it counts for nothing; a request with two
  `Authorization` headers is refused (400), and so is a login (`/auth/login`,
  `/api/v1/auth/login`) that carries any `Authorization` header. This is what lets the
  reference edge pass requests carrying `Bearer svr_…` without the passkey: such a request
  can do what the token allows and nothing else. Tested in `routes/ui/auth.rs` and the e2e.
- **Identity.** A token acts as its owner: the auth layer sets `CurrentUser` to the owner,
  exactly as a session does, plus a `TokenAuth` extension naming the token. Every audit row
  the request writes carries `token_id` and `token_name` (set for the whole request by the
  middleware, so no handler can forget it; the terminal socket carries it past the upgrade),
  and the Audit tab shows "via <token>" under the user. Last use (time and client address) is
  recorded at most once a minute.
- **Access presets**, decided in one place (`Preset::allows`) by method and path before any
  handler runs, **deny by default**: anything a preset does not name is `403 TOKEN_SCOPE`.
  - *Read-only*: `GET` on the inventory and configuration (servers, host keys, credentials
    with values redacted, settings with secrets blanked, profiles, quick commands, users,
    audit, sessions, recording metadata). No changes, no terminals, no files, no recording
    content.
  - *Servers & keys*: read-only plus creating, changing and deleting servers and
    credentials, importing, testing, trusting and forgetting host keys, generating and
    authorizing keys. No terminals or files.
  - *Full admin*: everything the owner can do, terminals (`/ws/terminal/pty` through a
    ticket), the SFTP file browser and `exec` (one command on a server) included.
  - *Servers & keys* also runs the operations that manage access without a shell:
    `diagnose`, `onboard` (which trusts a host key only with the fingerprint the caller
    compared) and `rotate-key`. These run on svrctlrs with the stored credentials; no
    request carries or returns a password or private key, so nothing secret passes through an
    agent (or its cloud model).
  - **No preset** names `/api/v1/tokens`, creating or deleting accounts, or changing a
    password. A token can never mint, change or revoke tokens or accounts, so revoking a
    token ends everything it can do, and a leaked token cannot leave a second way in behind.
- **Lifetime.** 30, 90 or 365 days, or never (the API takes any 1–3650 days or `null`).
  Expired and revoked tokens answer `401 TOKEN_EXPIRED` / `TOKEN_REVOKED`; the row stays,
  marked, until the owner removes it, so the audit and the list still explain it.
- **Revocation** is one tap and immediate: the next request is refused, and a terminal opened
  with the token is told and closed; a shell that terminal started is ended (a shell it only
  attached to, opened elsewhere by the owner, is detached and left running). Deleting the
  owner deletes their tokens (`ON DELETE CASCADE`).

- **Approvals.** A token can be set to *ask first* (the default for new tokens; existing ones
  keep working as before until switched): then deleting a server, deleting a credential and
  forgetting a server's host keys — and nothing else — answer `202` with a pending request
  instead of running (`server/src/approvals.rs`, table `approvals`). The request names the
  token, the call and its target, carries an unguessable id, and expires after an hour; asking
  again returns the same request. Its link goes out through Settings → Notifications →
  Approval requests (push, a webhook, a Matrix room, or nowhere: the caller's answer carries
  it too). **Only the owner, signed in, can approve or deny**: no preset names
  `/api/v1/approvals` or its decisions, so an agent cannot approve its own request, and an
  approval given by replying to the agent would be the token approving itself, which is just
  the "don't ask" setting. A token may read its request (`GET /api/v1/approvals/{id}`,
  `?wait=` up to 25 s) to wait for the answer. On approval the server makes the held call
  through the same API router as the token — the handler's own checks run, and its audit row
  names the token — but only if the token is still active and its preset still covers the
  call (otherwise nothing runs and the request shows `failed`, 410). The decision is one
  conditional update, so a request runs at most once and never after it expired. Audit:
  `approval.requested` (the token), `approval.approved` / `approval.denied` (the owner).
  Settings a full-admin token may change include where approval requests go; redirecting
  them hides requests from the owner but cannot approve one. The Matrix access token and the
  webhook URL are write-only through the API, like the push token; like it they are stored in
  the settings table unencrypted.

**What a never-expiring full-admin token means.** It is a password-equivalent credential for
every configured server that never times out: whoever reads it from an agent's config, a
shell history or a log has a shell on every server until the owner revokes it. It is not
covered by the edge passkey login (requests carrying a token have their own router without
it), so it works from wherever the API is reachable. Prefer *servers & keys* for an agent that
administers the inventory, give each agent its own token so one can be revoked alone, and
keep full-admin tokens in a secret store. The audit log shows what each token did.

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

First contact is confirmed, not trusted blindly. The first connection to a server records the
key it presents in `server_host_keys` with `trusted_at` NULL and refuses the shell, the
connection test and SFTP with the key's type and `SHA256:` fingerprint
(`HOST_KEY_UNCONFIRMED`; the terminal shows a dialog). The user compares it with
`ssh-keygen -lf /etc/ssh/ssh_host_*_key.pub` on the host and confirms it
(`POST /api/v1/servers/{id}/host-keys/trust` with that fingerprint, so a key that changed in
between is not trusted by accident; audited as `server.host_key_trusted`). Only one unconfirmed
key is kept per server; a different key on the next attempt replaces it. From then on a
connection presenting a key that matches none of the *trusted* keys for that server — whatever
its algorithm — is refused with a mismatch error. The same decision (`server/src/host_keys.rs`)
runs inside the one SSH client (`server/src/ssh.rs`) that the shell, the connection test, SFTP
and "Authorize key" all use, and a database error refuses the connection. A jump host is a
server like any other: its key is confirmed and checked on its own connection before the
target's connection is tunnelled through it, and the refusal names whose key is waiting. Keys are listed with
their fingerprints and trust state and can be forgotten per server (Servers → Keys,
`DELETE /api/v1/servers/{id}/host-keys`, audited as `server.host_keys_forgotten`), after which
the next connection asks for confirmation again. Keys stored before this existed (trust on
first use) were marked trusted by the migration.

## Session recordings

Off unless a server's **Record sessions** is set (opt-in per server; turning it on or off is
audited as `server.updated` with `record_sessions`). From then on every shell opened on that
server is recorded for the user who opened it (`server/src/recordings.rs`):

- **What.** What the terminal showed: the shell's output, which includes the echo of typed
  commands, and its resizes, as asciicast v2. Keystrokes are never recorded, so a password
  typed at a prompt that does not echo (`sudo`, `ssh`, `read -s`) is not in it; anything a
  program prints (`cat .env`) is. The new shell says "This session is recorded" and its pane
  shows a REC mark, so the person being recorded knows.
- **Whose.** A recording belongs to the user whose shell it was. Listing, playing,
  downloading and deleting are scoped by `user_id` in the query itself, and another user's
  recording answers 404 like a missing one (Warpgate's 2026 CVE leaked live recordings
  between users; this is the shape that prevents it). Server-crate tests and the e2e try every
  route as a second user.
- **At rest.** `RECORDINGS_DIR/<user id>/<random>.cast.enc`, directory 0700 and file 0600,
  every chunk encrypted with `ENCRYPTION_KEY` (AES-256-GCM, as for credentials), so a copied
  volume or backup without the key shows nothing. File names are random and never come from a
  request. Downloads are served `Cache-Control: private, no-store`.
- **Lifetime.** Deleted by an hourly sweep after `recording.retention_days` (default 30; 0
  keeps them), or by the owner. A recording stops at `RECORDING_MAX_MB` (default 100) and the
  shell goes on unrecorded. Deleting a user deletes their recordings (rows by foreign key,
  files by the sweep).
- **Audit.** `session.pty_opened` says whether the shell was recorded; downloads
  (`recording.downloaded`) and deletions (`recording.deleted`) are logged.

Every account is an administrator (weakness 8 below), so any user can switch recording on for
a server and thereby have the shells other users open there recorded. Those recordings belong
to the users who opened the shells, not to whoever switched recording on, and the terminal
tells them.

## Predictive echo

The pane draws typed characters before the shell echoes them (`ui/src/lib/terminal/localEcho.ts`,
default "on slow links"). Done carelessly this puts a password on screen, so nothing is drawn
until the shell has echoed a keystroke in the current *epoch*, and every non-text key (Enter,
Ctrl-C, arrows, Tab), unexplained output (a prompt being printed), mismatch and timeout starts a
new one. A prompt that does not echo never confirms its epoch, so nothing typed there is drawn;
on top of that, nothing is predicted on a line ending in a password/passphrase/PIN prompt or in
the alternate screen. A prediction is confirmed only when its cell holds the character **and**
the cursor has moved past it, so a typed space over a blank cell confirms nothing. Unit tests
cover these cases and the phone check types at a `read -s` prompt and fails if anything is drawn.
Predictions are only drawn over the page; they are never sent anywhere or recorded.

What it cannot see: a program that turns echo off **silently** (no prompt, no output) right
after the shell echoed type-ahead, without a key in between (`sleep 2; read -s` while typing
ahead). There the first characters can show, underlined, for up to two seconds before they are
withdrawn. Set Preferences → Behavior → Predictive Echo to **Off** to rule it out.

## Transport and headers

The server sets `Content-Security-Policy` (`default-src 'self'`, with `'unsafe-inline'`
scripts for the SvelteKit bootstrap and `wasm-unsafe-eval` for the xterm image addon),
`Referrer-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`. HTTP requests
time out after 30 s; WebSockets do not. TLS is the proxy's job.

## Deployment guidance

- A password alone must never sign in from the internet. The reference deployment does it
  with the provider as the only way in: the app signs in with Pocket ID (a passkey) over OIDC,
  `PASSWORD_LOGIN=off`, and one headers-only router in front (DEPLOYMENT.md, "Reverse proxy and
  edge auth"); the compose defaults to `off` and the server will not start that way without a
  provider. Without a provider, put every browser path behind a login proxy and let only
  requests carrying `Authorization: Bearer svr_…` (never the login) and `/api/v1/health` skip
  it; any other bypass of the proxy for `/api/` re-opens the password login to the internet
  (known weakness 13). Break-glass (`PASSWORD_LOGIN=on`) opens the password form for as long
  as it is on: do it from the LAN or with the proxy chain back on the router. See the CSWSH
  item below for why `ALLOWED_ORIGINS` should stay empty on browser-only deployments.
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
   administrator to forget the server's keys (Servers → Keys, or `DELETE /api/v1/servers/{id}/host-keys`).
   Since 2026-10-18 the first contact is no longer trusted silently either: the key is recorded
   and the user confirms its fingerprint before any connection goes through ("SSH host keys").
6. ~~**`POST /servers/{id}/test` skips host-key verification**~~ — fixed after 3.0.0: the
   connection test goes through the shared SSH client, which verifies the host key during the
   handshake and supports password credentials.
7. ~~**`local` servers are container RCE for any user**~~ — fixed 2026-10-18: command mode,
   the only path that executed anything locally, is gone. `POST /servers` refuses
   `is_local: true` (400); since the migration baseline (2026-10-18) the model has no local
   flag at all, and legacy local rows are disabled with `hostname = 'localhost'`.
8. **No roles** — every user is equal and can manage accounts (single-user today; roles are
   "maybe later" in `ROADMAP.md`, required before the app is offered to other people).
   Two-factor sign-in (TOTP) is optional per account since ROADMAP 21. Login attempts are
   rate limited (10 failures per 15 minutes per client address *and* per username, 429 with
   `Retry-After`; kept in the database since migration 109, so a restart does not reset
   it), new passwords must be at least 12
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
13. ~~**A password alone was enough from the internet**~~ — fixed 2026-10-19: the reference
    compose sent all of `/api/` and `/ws/` through a headers-only router for the native apps,
    so `POST /api/v1/auth/login` took a password without the edge passkey, and the session it
    returned opened shells on every server. Now every browser path is behind the passkey; a
    narrow router passes only requests carrying an API token (never the login) plus the health
    check, and the app judges a token request by the token alone, refuses a second
    `Authorization` header and refuses a login that carries one (tested). Found by asking the
    login route from outside: the app answered instead of the edge. The native apps' password
    login no longer passes that edge; they return with OIDC sign-in (ROADMAP 21–22).

## Reporting

This is a personal project without a disclosure programme. Open an issue at
`git.js-node.cc/jsprague/svrctlrs` or contact the maintainer directly.
