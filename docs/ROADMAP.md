# Roadmap — gaps and improvements

From the 2026-10-04 audit of the code, UI, CI and docs. Each item names the evidence so it can
be verified or closed. Priorities: **P0** security or correctness that affects users now;
**P1** needed for a first-class self-hosted app; **P2** polish and hygiene; **P3** features.

## P0 — Security (details in `SECURITY.md`, "Known weaknesses")

| # | Change | Where |
|---|---|---|
| 1 | Reject WebSocket upgrades whose `Origin` is not our own origin or in `ALLOWED_ORIGINS`; keep the cookie `SameSite=Strict` unless native clients are configured | `server/src/routes/terminal.rs`, `terminal_pty.rs`, `main.rs` |
| 2 | Token auth: load `user_id` from the session record and check `expiry_date > now`; stop writing client-supplied `X-User-Id` into the session; cycle the session id on login | `server/src/routes/ui/auth.rs` |
| 3 | Redact `token`/`user_id` from the tracing span; then replace the query-string token with a one-time WebSocket ticket | `main.rs` (TraceLayer), both WS handlers, `ui/.../TerminalPane.svelte` |
| 4 | Browser clients: stop writing the session id to `localStorage`; use the cookie only | `ui/src/routes/auth/login/+page.svelte`, `ui/src/lib/api/client.ts` |
| 5 | Host keys: compare against every stored key for the server, not per `key_type`; fail closed on DB errors in the PTY path | `terminal.rs`, `terminal_pty.rs`, `database/src/queries/server_host_keys.rs` |
| 6 | Route `POST /servers/{id}/test` through the TOFU connector and delete `server/src/ssh.rs` | `server/src/routes/api/servers.rs` |
| 7 | Scope `get`/`update`/`delete` of profiles and quick commands by `user_id` | `database/src/queries/terminal_profiles.rs`, `quick_commands.rs` |
| 8 | Encrypt `credentials.metadata` (holds passphrases) or move the passphrase into `value`; remove the `Debug` derive that prints it | `database/src/models/credential.rs`, `server/src/routes/api/credentials.rs` |
| 9 | Refuse to start when encryption cannot initialise instead of falling back to plaintext | `server/src/main.rs` |
| 10 | Login rate limiting (per IP and per user) and a minimum password length | `auth.rs`, `database/src/queries/users.rs` |
| 11 | Server-crate tests for all of the above: auth middleware (cookie, Bearer, query token, exemptions), TOFU accept/mismatch, WS Origin check | `server/src/routes/**` |

## P1 — First-class app

| Area | Gap | Proposal |
|---|---|---|
| Settings UI | `settingsState.loadSettings()` is never called, so Notification and Default-profile forms open blank | call it in `+layout.svelte` or the settings page |
| Reconnect | `onerror` sets status `error` before `onclose` reads it, so established sessions never auto-reconnect; `RECONNECT_DELAYS[5]` unreachable | reorder in `TerminalPane.svelte`, add jitter |
| Users | no password change, no user management, single role | `/api/v1/users` CRUD (admin only), `role` column, password change in Settings |
| Host keys | no way to see or revoke stored keys | `GET/DELETE /api/v1/servers/{id}/host-keys` + a panel on the server card |
| Audit log | nothing records who opened a shell where | `audit_log` table written by login, WS connect, server/credential changes; a Settings page to read it |
| Backups | no in-app export | `svrctl backup` that copies the DB (via `VACUUM INTO`) and key to a tarball; document restore |
| Health | `/api/v1/health` is static | add DB ping and migration state; keep it unauthenticated but minimal |
| Observability | `tracing-subscriber` has `json` enabled but is never used | `LOG_FORMAT=json`, request ids |
| Desktop | tray icon has no image (invisible on most desktops); close-to-tray leaves no way to quit without a tray | set `.icon()` from the bundled icons; add a Quit menu item; fall back to normal close when no tray |
| Identity | Tauri `app.js-node.svrctlrs` vs Flatpak `cc.jsnode.svrctlrs` → two desktop IDs | pick one (`cc.jsnode.svrctlrs` is the standard reverse-DNS of js-node.cc); note it changes the Android `applicationId`, so decide before the first signed public APK |
| Android | `usesCleartextTraffic=false` with no UI warning for `http://` URLs; AAB built but never published; biometrics plugin registered but unused | validate scheme in `ServerUrlSetup`; publish the AAB or drop it; wire biometrics to unlock the stored token or remove the dependency |
| Frontend tooling | no eslint/prettier/vitest, no `engines` | add them with a `lint`/`test` script and a CI step; `engines.node >= 22.12` |
| a11y | icon buttons without labels (`TerminalTabs`, `Toast`, `TerminalPrefsPanel`), palette `role=option` without `listbox`, tabs without arrow-key navigation, toast without `aria-live`, 8 svelte-check warnings | fix the warnings first; they are the cheap half |
| Flatpak | GNOME 46 runtime is end-of-life; no AppStream metainfo; bundles a binary built outside the sandbox | move to 48, add `metainfo.xml`, build inside flatpak-builder or drop Flatpak in favour of AppImage |
| Docs | no screenshots | add to README and USER-GUIDE once the UI settles |

## P2 — Hygiene

- Collapse the three SSH connection stacks (`ssh.rs`, `terminal.rs`, `terminal_pty.rs`) into
  one connector with one default-key list and one TOFU implementation.
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
