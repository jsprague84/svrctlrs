# CLAUDE.md — AI development guide

**Updated:** 2026-10-11 · **Version:** 3.0.0 · **Default branch:** `master`

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
core/        encryption (AES-256-GCM), error types, shared types
database/    sqlx models, queries, embedded migrations (database/migrations/)
server/      Axum app: main.rs, routes/{api/*, terminal.rs, terminal_pty.rs, ui/auth.rs},
             health_monitor.rs, bin/svrctl.rs (CLI used by the container healthcheck)
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

- **Rust**: `anyhow::Context` for errors in queries; API handlers return
  `(StatusCode, Json<ApiError>)` via the helpers in `server/src/routes/api/mod.rs`;
  `#[instrument]` on handlers and queries (skip inputs that hold secrets). Register new API
  routes in `server/src/routes/api/mod.rs::routes()`, never in `main.rs`. Use
  `sqlx::QueryBuilder` for dynamic UPDATEs. Scope every per-user query by `user_id`.
  No `unwrap()` on request paths.
- **Migrations**: sequential, embedded with `sqlx::migrate!`, never edited after they ship
  (sqlx checksums them). Latest is `023`.
- **Svelte 5**: runes only (`$state`, `$derived`, `$effect`, `$props`); callback props, not
  `createEventDispatcher`. State modules in `ui/src/lib/state/*.svelte.ts` export getters.
  Use `extractErrorMessage()` from `#lib/utils/error` in every catch. Mobile breakpoint is
  `md:` (768 px) plus `isMobile()` for JS. Tailwind v4 tokens live in `ui/app.css`.
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

Any authenticated user gets a shell on every configured server (and on the container itself
via a `local` server). Treat the login, session/token handling, WebSocket auth, CORS and
host-key code as security-critical: read `docs/SECURITY.md` before changing them and add
server-crate tests with the change.
