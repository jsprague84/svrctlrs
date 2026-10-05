# Roadmap — gaps and improvements

From the 2026-10-04 audit of the code, UI, CI and docs. Each item names the evidence so it can
be verified or closed. Priorities: **P0** security or correctness that affects users now;
**P1** needed for a first-class self-hosted app; **P2** polish and hygiene; **P3** features.

## P0 — Security (details in `SECURITY.md`, "Known weaknesses")

| # | Change | Where |
|---|---|---|
| ~~1~~ | Done 2026-10-05: WebSocket upgrades require `Origin` = request host / `X-Forwarded-Host` or an `ALLOWED_ORIGINS` entry (403 otherwise); tested | `auth::websocket_origin_allowed`, both WS handlers |
| ~~2~~ | Done 2026-10-05: token auth resolves the session through the store (expiry-filtered) and takes `user_id` from the record; `X-User-Id`/`?user_id=` ignored; `CurrentUser` extractor replaces per-handler session reads; session id cycled on login; tested | `server/src/routes/ui/auth.rs` |
| ~~3~~ | Done 2026-10-05: one-time 30 s WebSocket tickets (`POST /api/v1/auth/ws-ticket`), redeemed by the middleware on upgrades only; native clients use them with a token fallback for old servers; `?token=` stays accepted (deprecated) for old app builds — remove after the next APK rollout | `server/src/ws_tickets.rs`, `auth.rs`, `TerminalPane.svelte` |
| ~~4~~ | Done 2026-10-05 for the browser (cookie only; no token in `localStorage`). Native apps: move the token to the platform keystore (P1) | `ui/src/routes/auth/login/+page.svelte`, `ui/src/lib/api/client.ts` |
| ~~5~~ | Done 2026-10-05: `server/src/host_keys.rs` evaluates against every stored key; both connectors fail closed; tested | `host_keys.rs`, `terminal.rs`, `terminal_pty.rs` |
| ~~6~~ | Done 2026-10-05: connection test uses the command connector (TOFU first, password credentials supported); `ssh.rs` deleted | `server/src/routes/api/servers.rs` |
| ~~7~~ | Done 2026-10-05: profiles and quick commands scoped per user with 404 on foreign ids; tested | `terminal_profiles.rs`, `quick_commands.rs` |
| ~~8~~ | Done 2026-10-05: metadata encrypted with its own flag (migration 022), start-up pass for legacy rows, redacted `Debug`; passphrases are now settable from the UI/API and used by both connectors. Also fixed: pasted key material was treated as a file path, so "Paste key" credentials could never connect | `database/migrations/022`, `server/src/ssh_keys.rs`, `credentials.rs` |
| ~~9~~ | Done 2026-10-05: the server refuses to start when encryption cannot initialise; the database layer refuses plaintext. (Production had run in the fallback since 2026-09-14 because the compose passed `ENCRYPTION_KEY=` empty.) | `server/src/main.rs`, `database/src/queries/credentials.rs` |
| ~~9a~~ | Done 2026-10-05: start-up encrypts every `encrypted = 0` row in one transaction and logs the count | `encrypt_plaintext_credentials` |
| ~~10~~ | Done 2026-10-05: sliding-window limiter (10 failures / 15 min) per client address and per username on both login routes, 429 + `Retry-After`; 12-character minimum for new passwords; session id cycled on login | `server/src/login_limiter.rs`, `auth.rs`, `users.rs` |
| 11 | Server-crate tests for all of the above: auth middleware (cookie, Bearer, query token, exemptions), TOFU accept/mismatch, WS Origin check | `server/src/routes/**` |
| ~~12~~ | Done 2026-10-05: russh 0.51 → 0.64 and async-ssh2-tokio 0.8 → 0.13 (which brings russh 0.62.7); `russh-keys` dropped for `russh::keys`; host certificates are pinned by their embedded key. RUSTSEC-2026-0154/-0153 cleared; only `rsa` RUSTSEC-2023-0071 (Marvin, transitive via `ssh-key`, no fix) remains ignored. First live PTY/CMD connection after deploy is the behavioral check — no SSH integration test exists yet | `Cargo.toml`, `host_keys.rs`, `ci.yaml` |

## P1 — First-class app

| Area | Gap | Proposal |
|---|---|---|
| ~~Settings UI~~ | Done 2026-10-05: both settings forms load the store before reading it | — |
| Reconnect | Done 2026-10-05: an established session that errors now reconnects; all six backoff steps are used. Still open: add jitter | `TerminalPane.svelte` |
| Users | Done 2026-10-05: `/api/v1/users` list/create/delete (not self, not last) and `PUT /users/me/password`; Settings → Users tab. Still open: a `role` column so not every account is an administrator | `server/src/routes/api/users.rs` |
| Host keys | Done 2026-10-05: `GET/DELETE /api/v1/servers/{id}/host-keys` with `SHA256:` fingerprints; Keys modal on the Servers page | `servers.rs`, `host_keys::fingerprint` |
| Audit log | nothing records who opened a shell where | `audit_log` table written by login, WS connect, server/credential changes; a Settings page to read it |
| Backups | no in-app export | `svrctl backup` that copies the DB (via `VACUUM INTO`) and key to a tarball; document restore |
| Health | `/api/v1/health` is static | add DB ping and migration state; keep it unauthenticated but minimal |
| Observability | `tracing-subscriber` has `json` enabled but is never used | `LOG_FORMAT=json`, request ids |
| Desktop | Done 2026-10-05 (unverified until the next desktop CI build): tray uses the bundled icon; close hides to the tray only when a tray was created, otherwise it quits. The tray menu already had Show/Quit | `ui/src-tauri/src/lib.rs` |
| Identity | Tauri `app.js-node.svrctlrs` vs Flatpak `cc.jsnode.svrctlrs` → two desktop IDs | pick one (`cc.jsnode.svrctlrs` is the standard reverse-DNS of js-node.cc); note it changes the Android `applicationId`, so decide before the first signed public APK |
| Android | `usesCleartextTraffic=false` with no UI warning for `http://` URLs; AAB built but never published; biometrics plugin registered but unused | validate scheme in `ServerUrlSetup`; publish the AAB or drop it; wire biometrics to unlock the stored token or remove the dependency |
| Frontend tooling | no eslint/prettier/vitest, no `engines` | add them with a `lint`/`test` script and a CI step; `engines.node >= 22.12` |
| a11y | icon buttons without labels (`TerminalTabs`, `Toast`, `TerminalPrefsPanel`), palette `role=option` without `listbox`, tabs without arrow-key navigation, toast without `aria-live`, 8 svelte-check warnings | fix the warnings first; they are the cheap half |
| Flatpak | GNOME 46 runtime is end-of-life; no AppStream metainfo; bundles a binary built outside the sandbox | move to 48, add `metainfo.xml`, build inside flatpak-builder or drop Flatpak in favour of AppImage |
| Docs | no screenshots | add to README and USER-GUIDE once the UI settles |

## P2 — Hygiene

- Collapse the two remaining SSH connection stacks (`terminal.rs`, `terminal_pty.rs`) into
  one connector with one default-key list (the TOFU decision is already shared).
- `server/src/config.rs` and `AppState.config` are never read; either use the TOML (servers
  from config) or delete it and the `--config` flag.
- Replace stringly `core::Error` variants with typed errors and `IntoResponse`.
- `database/src/lib.rs` `#![allow(ambiguous_glob_reexports)]` hides name collisions; export
  explicitly.
- Remove dead frontend modules: `Header.svelte`, `ServerSelector.svelte`, `api/index.ts`,
  unused exports in state modules; drop unused `@tauri-apps/api`, `plugin-http`, `plugin-os`
  (or use them).
- Squash migrations `000`–`015` into one baseline for fresh installs (keep the old files for
  existing databases behind a version check).
- Remove the global `npm i -g @tauri-apps/cli` from both Tauri CI images (the devDependency is
  used); confirm `flatpak-builder` runs unprivileged on the runner or move it out.
- Update the Tauri JS/Rust deps (2.10 → 2.12), SvelteKit 2.55 → 2.70, Svelte 5.57, and plan the
  SvelteKit 3 / adapter-static 4 / lucide 1.0 majors.
- Tag hygiene: `v0.1.0` is newer than `v0.1.1`; both are superseded by `v3.0.0`. Leave them (a
  tag delete breaks nothing but history) and never reuse pre-3.0 numbers.
- GitHub: the repository there is a stale mirror of the 2025 code. Archive it, or configure a
  Forgejo push mirror if a backup copy is wanted.

## P3 — Features

In rough order of value for a terminal manager: SFTP/file browser (russh has the subsystem),
port forwarding, session recording and playback (asciicast from the PTY stream), SSH key
generation in the credentials UI, quick-command variables (`{{host}}`, prompts), PWA manifest
so the browser UI installs on phones, desktop auto-update (`tauri-plugin-updater` against the
Forgejo release feed), 2FA/WebAuthn for the app login, SSH agent forwarding, and a help page
that hosts the shortcut table and this guide.

## Done in the 2026-10-04 pass

- CI/CD moved fully to Forgejo on the standard runners; GitHub workflows removed; image
  published to the Forgejo registry with revision labels; master auto-deploys to docker-vm;
  tags produce releases with notes from `CHANGELOG.md`.
- Versions unified at 3.0.0 across Cargo, UI and Tauri; About page reads it from `package.json`.
- Empty `ALLOWED_ORIGINS` no longer switches the cookie to `SameSite=None`.
- `/api/v1/health` reports the build revision.
- Image: pinned Rust, no OpenSSL/curl/sqlite3, revision label; `.dockerignore` excludes
  `node_modules`, builds and the Tauri tree.
- Compose: Forgejo image, `secure-auth-chain@file` for the UI and headers-only for API/WS,
  no Docker group id.
- Docs rewritten for the current product; job-era docs archived; design artifacts under
  `docs/history/`.
