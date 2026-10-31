# CLAUDE.md — AI development guide

**Updated:** 2026-10-31 · **Version:** 4.1.0 · **Default branch:** `master`

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
- `docs/agents.md` — agents are first class: `svrctl` + the skill, token presets, approvals,
  and the parity test every new route must pass. `docs/cli.md` — every `svrctl` command with
  the preset it needs.

## Layout

```
api/         svrctlrs-api: the REST API's request/response types (+ OpenAPI schemas), presets.rs
             (what each token preset may call: the auth layer's and svrctl's one table)
core/        encryption (AES-256-GCM), error type, notification senders
database/    sqlx models, queries, embedded migrations (migrations/ current, migrations-legacy/ frozen),
             backup.rs (the backup copy and restore, shared by server and svrctl)
server/      Axum app: main.rs, app.rs (the router stack), ssh.rs (the one SSH client),
             sessions.rs (server-owned PTYs), persistent_shells.rs (shells kept in tmux),
             recordings.rs (opt-in session recording), backups.rs (nightly database copies),
             api_tokens.rs + approvals.rs (agent tokens: format and check, held deletes),
             diagnose.rs + key_ops.rs + routes/api/ops.rs (diagnose, exec, onboard, rotate-key),
             routes/{api/*, terminal_pty.rs, ui/auth.rs}, health_monitor.rs,
             bin/svrctl/ (the agent CLI over the API + local backup/restore/reset-password;
             also the container healthcheck; surface.rs: every API operation's command, the
             parity test), tests/svrctl.rs (both binaries end to end),
             test_support/ (tests only: loopback SSH server, in-memory database, the full
             app on a temporary database)
ui/          SvelteKit 5 SPA (adapter-static → ui/build, served by the server at SPA_DIR)
ui/src-tauri/  Tauri 2 desktop + Android (excluded from the Cargo workspace)
flake.nix, packaging/nix/  the desktop app as a Nix package (NixOS; no hashes to update, check `nix build .#desktop`)
.forgejo/    CI images and workflows (ci, docker, android, desktop)
scripts/dev/ the loop (prepush, ci-watch, gates, ci-lint, ci-log, ci-rerun, ci-dispatch on
             forgejo.sh) and the e2e/browser checks (run-e2e.sh, run-phone-check.sh, run-ui-screens.sh)
scripts/ci/  release helper; scripts/deploy/ the forced-command targets: docker-vm deploy,
             hermes-agent-sync.sh (hermes installs the deployed skill and svrctl)
skills/      svrctlrs/SKILL.md, the agent skill for svrctl (linked from .claude/skills/)
```

## Commands

The loop (DEVELOPMENT.md, "The loop"): heavy checks run on epyc-ai, not here.

```bash
scripts/dev/prepush.sh             # before a push, seconds: fmt; svelte-check + Prettier if ui/ changed
scripts/dev/ci-watch.sh [sha]      # after it: exits 0 only when every job is green and prod runs the sha
scripts/dev/ci-log.sh [sha] [job]  # a job's whole log; ci-rerun.sh when the runner failed, not the code
scripts/dev/gates.sh [--e2e]       # every CI gate locally (CI down, migrations, security code)
scripts/dev/ci-lint.sh             # fmt + clippy in CI's image: CI's clippy is not yours
cd ui && npm run dev               # Vite + HMR, proxies /api and /ws to localhost:8080
cd ui && npm run tauri:dev         # desktop app against the dev server
```

No Rust toolchain on the t490 laptop: wrap cargo in
`nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#clippy nixpkgs#rustfmt nixpkgs#pkg-config nixpkgs#gcc --command ...`
(the scripts do). Lint with `ci-lint.sh`, never the laptop's clippy: green locally is not proof.

## Conventions

- **Rust**: queries return `svrctlrs_core::Result` with the typed variants (`NotFound`,
  `Conflict`, `InvalidInput`, `Database(anyhow)` with `.context(..)`); API handlers return
  `Result<_, ApiError>` and use `?` on them — `ApiError` (`server/src/routes/api/mod.rs`) maps
  the kind to 404/409/400 and logs anything else as a 500. Responses and request bodies are
  `svrctlrs-api` types (`api/src/`), never `json!`; a handler carries `#[utoipa::path]` and is
  registered with `routes!(…)`; regenerate `docs/openapi.json` and, for a deliberate shape
  change, `server/testdata/api-shapes.json` (DEVELOPMENT.md, "API types"); a new operation
  gets its row in svrctl's parity table (`server/src/bin/svrctl/surface.rs`: a command, or
  `api`/`none` with a reason) and, if tokens may call it, a line in `api/src/presets.rs`. Use its constructors
  (`bad_request`, `not_found`, …) only for checks the handler makes itself;
  `#[instrument]` on handlers and queries (skip inputs that hold secrets). Register new API
  routes in `server/src/routes/api/mod.rs::routes()`, never in `main.rs`. Use
  `sqlx::QueryBuilder` for dynamic UPDATEs. Scope every per-user query by `user_id`.
  No `unwrap()` on request paths.
- **Migrations**: two embedded chains (`database/src/lib.rs::run_migrations`).
  `database/migrations/` is current: `100_baseline.sql` plus everything after it; add the next
  change with the next free number (`113_…` after `112_devices.sql`). `database/migrations-legacy/` (`000`–`024`) is frozen and only runs for a
  database that predates the baseline, which is then marked as having `100` applied. Never
  edit a shipped migration (sqlx checksums them).
- **Svelte 5**: runes only (`$state`, `$derived`, `$effect`, `$props`); callback props, not
  `createEventDispatcher`. State modules in `ui/src/lib/state/*.svelte.ts` export getters.
  Use `extractErrorMessage()` from `#lib/utils/error` in every catch. Mobile breakpoint is
  `md:` (768 px) plus `isMobile()` for JS. Tailwind v4 tokens live in `ui/src/app.css`.
- **Versions**: one number in `Cargo.toml` (workspace), `ui/package.json`,
  `ui/src-tauri/tauri.conf.json` and `ui/src-tauri/Cargo.toml`. Bump all four together (and
  `docs/openapi.json`); add a `CHANGELOG.md` section. `scripts/ci/check-version.sh [vX.Y.Z]`
  checks them, in CI's Lint on every push and with the tag in the release job.
- **Docs**: when behavior changes, update the doc that describes it in the same commit.
- **Commits**: Conventional Commits. A `feat` commit that changes app code also touches the
  agent path (`skills/svrctlrs/`, `docs/cli.md`, `docs/agents.md`, an eval task) and `docs/` or
  `CHANGELOG.md`, or says why not in an `Agent-path: …` / `Docs: …` trailer;
  `scripts/ci/agent-path.sh` checks it in prepush and CI (DEVELOPMENT.md, "The commit check").

## CI/CD (Forgejo, dev-space standard)

Runners are the epyc-ai labels `rust-high` (cargo in `rust-ci:latest`, npm in `svrctlrs-ci`), `build` (images,
DinD at `tcp://docker:2375`) and `android`. Never `runs-on: docker`, never mount a Docker
socket or TLS certs, share only `/ci-cache/sccache` (plus npm/gradle caches). `ci.yaml` runs on
every branch; `docker.yaml` publishes `master` to `git.js-node.cc/jsprague/svrctlrs` and
deploys it to docker-vm through a forced-command key; `v*` tags publish image, APK and desktop
packages to a Forgejo release. GitHub is retired. `docker.yaml` ignores docs-only pushes, so
never end a push on a docs-only commit. Details: `docs/DEVELOPMENT.md`.

## Sessions

Work runs in sessions of a few ROADMAP "Next" items, one commit series each. Say unasked, in
one line ("A new session is best here: …"), when a new session is best: an item or batch has
shipped and is verified live, the next item is a different kind of work, context is long
(large reads, a compaction), or the session prompt said a part is a session of its own.
Before stopping, write the next session's first prompt to the brain note
`~/brain/spaces/dev/projects/svrctlrs/next-session.md` (what to read, what is live, the
numbered items, the rules), move the used one to `next-session-<date>.md`, and update
`hot.md` there.

## Security posture to keep in mind

Any authenticated user gets a shell on every configured server. Treat the login, session/token handling, WebSocket auth, CORS and
host-key code as security-critical: read `docs/SECURITY.md` before changing them and add
server-crate tests with the change.
