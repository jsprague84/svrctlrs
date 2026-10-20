# CLAUDE.md — AI development guide

**Updated:** 2026-10-19 · **Version:** 3.3.0 · **Default branch:** `master`

SvrCtlRS is a self-hosted **terminal/SSH management tool**: a Rust (Axum) server with a
SvelteKit 5 SPA, wrapped by Tauri 2 for Linux desktop and Android. It is *not* the 1.x/2.x
plugin-based monitoring platform; that product was retired in March 2026 and its code and docs
are gone. If anything you read (an old doc, a brain note, a chat summary) talks about plugins,
schedulers, jobs, HTMX or Askama, it is describing the retired product.

## Read first

- `docs/ARCHITECTURE.md` — crates, UI structure, request flow, data model.
- `docs/SECURITY.md` — auth, encryption, host keys, and the **known weaknesses** list. Do not
  reintroduce anything on that list; fix items from it when you touch nearby code.
- `docs/ROADMAP.md` — prioritized gaps. Pick work from here unless told otherwise.
- `docs/DEVELOPMENT.md` — build, test, CI, release procedure.

## Layout

```
core/        encryption (AES-256-GCM), error type, notification senders
database/    sqlx models, queries, embedded migrations (migrations/ current, migrations-legacy/ frozen)
server/      Axum app: main.rs, app.rs (the router stack), ssh.rs (the one SSH client),
             sessions.rs (server-owned PTYs), recordings.rs (opt-in session recording),
             api_tokens.rs + approvals.rs (agent tokens: presets, held deletes),
             routes/{api/*, terminal_pty.rs, ui/auth.rs}, health_monitor.rs,
             bin/svrctl.rs (CLI used by the container healthcheck),
             test_support/ (tests only: loopback SSH server, in-memory database, the full
             app on a temporary database)
ui/          SvelteKit 5 SPA (adapter-static → ui/build, served by the server at SPA_DIR)
ui/src-tauri/  Tauri 2 desktop + Android (excluded from the Cargo workspace)
.forgejo/    CI images and workflows (ci, docker, android, desktop)
scripts/ci/  release helper; scripts/deploy/ docker-vm deploy (forced-command target)
```

## Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ui && npm ci && npm run check && npm run build
cd ui && npm run dev            # Vite + HMR, proxies /api and /ws to localhost:8080
cd ui && npm run tauri:dev      # desktop app against the dev server
```

No Rust toolchain on the t490 laptop: wrap cargo in
`nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#clippy nixpkgs#rustfmt nixpkgs#pkg-config nixpkgs#gcc --command ...`.
CI's clippy may be older or newer than yours; green locally is not proof.

## Conventions

- **Rust**: queries return `svrctlrs_core::Result` with the typed variants (`NotFound`,
  `Conflict`, `InvalidInput`, `Database(anyhow)` with `.context(..)`); API handlers return
  `Result<_, ApiError>` and use `?` on them — `ApiError` (`server/src/routes/api/mod.rs`) maps
  the kind to 404/409/400 and logs anything else as a 500. Use its constructors
  (`bad_request`, `not_found`, …) only for checks the handler makes itself;
  `#[instrument]` on handlers and queries (skip inputs that hold secrets). Register new API
  routes in `server/src/routes/api/mod.rs::routes()`, never in `main.rs`. Use
  `sqlx::QueryBuilder` for dynamic UPDATEs. Scope every per-user query by `user_id`.
  No `unwrap()` on request paths.
- **Migrations**: two embedded chains (`database/src/lib.rs::run_migrations`).
  `database/migrations/` is current: `100_baseline.sql` plus everything after it; add the next
  change with the next free number (`109_…` after `108_approvals.sql`). `database/migrations-legacy/` (`000`–`024`) is frozen and only runs for a
  database that predates the baseline, which is then marked as having `100` applied. Never
  edit a shipped migration (sqlx checksums them).
- **Svelte 5**: runes only (`$state`, `$derived`, `$effect`, `$props`); callback props, not
  `createEventDispatcher`. State modules in `ui/src/lib/state/*.svelte.ts` export getters.
  Use `extractErrorMessage()` from `#lib/utils/error` in every catch. Mobile breakpoint is
  `md:` (768 px) plus `isMobile()` for JS. Tailwind v4 tokens live in `ui/src/app.css`.
- **Versions**: one number in `Cargo.toml` (workspace), `ui/package.json`,
  `ui/src-tauri/tauri.conf.json` and `ui/src-tauri/Cargo.toml`. The release job fails if the
  tag disagrees with `Cargo.toml`. Bump all four together; add a `CHANGELOG.md` section.
- **Docs**: when behavior changes, update the doc that describes it in the same commit.

## CI/CD (Forgejo, dev-space standard)

Runners are the epyc-ai labels `rust-high` (cargo, in `svrctlrs-ci`), `build` (images,
DinD at `tcp://docker:2375`) and `android`. Never `runs-on: docker`, never mount a Docker
socket or TLS certs, share only `/ci-cache/sccache` (plus npm/gradle caches). `ci.yaml` runs on
every branch; `docker.yaml` publishes `master` to `git.js-node.cc/jsprague/svrctlrs` and
deploys it to docker-vm through a forced-command key; `v*` tags publish image, APK and desktop
packages to a Forgejo release. GitHub is retired. Details: `docs/DEVELOPMENT.md`.

## Security posture to keep in mind

Any authenticated user gets a shell on every configured server. Treat the login, session/token handling, WebSocket auth, CORS and
host-key code as security-critical: read `docs/SECURITY.md` before changing them and add
server-crate tests with the change.
