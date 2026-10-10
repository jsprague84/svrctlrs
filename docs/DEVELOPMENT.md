# Development

## Toolchain

- Rust stable (CI image: `git.js-node.cc/jsprague/rust-ci`, Docker build: `rust:1.98-bookworm`)
- Node 22.17+ (SvelteKit 3's floor), npm 10+
- For Tauri: the system deps in `.forgejo/Dockerfile.tauri-desktop` (WebKitGTK 4.1, GTK 3,
  libayatana-appindicator, librsvg) and for Android JDK 17, SDK platform 36, NDK 27.

On NixOS without a global toolchain:

```bash
nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#clippy nixpkgs#rustfmt nixpkgs#pkg-config nixpkgs#gcc \
  --command cargo clippy --workspace --all-targets -- -D warnings
```

## Run locally

```bash
# terminal 1 — server on :8080 (plain http, so disable the Secure cookie flag)
SESSION_SECURE=false ADMIN_PASSWORD=dev ENCRYPTION_KEY=$(openssl rand -hex 32) \
  DATABASE_URL=sqlite:data/svrctlrs.db cargo run --package server

# terminal 2 — SPA with HMR; /api and /ws are proxied to :8080
cd ui && npm ci && npm run dev
```

Open http://localhost:5173 and log in as `admin`/`dev`. To test the server-served SPA instead,
`npm run build` and start the server with `SPA_DIR=ui/build`.

Desktop app against the dev server: `cd ui && npm run tauri:dev`. Android:
`TAURI_DEV_HOST=<your LAN IP> npm run tauri android dev`.

## Quality gates

These are exactly what CI runs; run them before pushing.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo audit                                   # cargo install cargo-audit
cd ui && npm run check && npm run lint && npm run test && npm run build && npm audit --omit=dev --audit-level=high
```

`npm run format` applies Prettier (tabs, single quotes, 100 columns); `npm run lint` is ESLint
(JS + typescript-eslint + eslint-plugin-svelte, Prettier-compatible) plus a Prettier check.
Unit tests are Vitest files next to the code (`*.test.ts`); keep them for pure modules.

Tests today: 74 Rust tests across `core`, `database` and `server` (auth middleware, host keys,
limiter, tickets, session registry and replay ring, svrctl backup) and 11 Vitest tests in the
UI. Security-relevant changes come with tests.

`scripts/dev/pty-session-e2e.py` drives the persistent-session protocol end to end (shell,
dropped socket, reattach with replay, `closed`, `session_gone`, TTL) against a locally running
server and a real SSH target; its docstring has the exact invocation. Run it after touching
`sessions.rs`, `terminal_pty.rs` or the pane's attach logic.

## Repository layout

See `ARCHITECTURE.md`. Conventions for code live in `CLAUDE.md`; they apply to humans too.

## CI/CD

Everything runs on Forgejo Actions at `git.js-node.cc/jsprague/svrctlrs` using the dev-space
standard runners on epyc-ai. GitHub (`github.com/jsprague84/svrctlrs`) is a **push mirror**
only — Forgejo pushes every commit there (and every 8 h) with a fine-grained GitHub token that
has *Contents: read and write* on that one repository; nothing runs on GitHub. Mirror state:
repository Settings → Mirror settings, or `GET /api/v1/repos/jsprague/svrctlrs/push_mirrors`.

| Workflow | Trigger | Runner | Does |
|---|---|---|---|
| `ci.yaml` | every branch, tags, PRs to master | `rust-high` | fmt, clippy, cargo-audit, tests, svelte-check, eslint + prettier, vitest, SPA build, npm audit |
| `docker.yaml` | `master`, `v*`, manual | `build` | builds the image with buildx, pushes `latest`/`sha`/`vX.Y.Z` to the Forgejo registry; on tags creates the release; on `master` waits for CI to be green and deploys to docker-vm |
| `desktop.yaml` | `v*`, manual | `rust-high` | `.deb`/`.rpm` (gate) and AppImage (best effort), attached to the release |
| `workflows-parked/android.yaml` | — | `android` | parked 2026-10-10 (the Android app is unused; the installable web app replaces it). Move it back to `workflows/` and run `tauri android init` for the `cc.jsnode.svrctlrs` identifier to revive it |

Rules, from the forgejo-ci standard: pick runners by label (`rust-high`, `build`, `android`),
never `runs-on: docker`; Rust jobs run in `svrctlrs-ci` (rust-ci + Node 22) and clone by hand;
image jobs use `catthehacker/ubuntu:act-latest` with the standard Docker actions and get the
daemon from the runner's DinD sidecar at `tcp://docker:2375` (no socket mounts, no TLS); share
`/ci-cache/sccache` only for cargo (npm and gradle caches lock properly).

Secrets: `FORGEJO_REGISTRY_TOKEN_ALL` (user-level, inherited) for the registry;
repository-level `RELEASE_TOKEN`, `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`,
`ANDROID_KEY_PASSWORD`, `DOCKER_VM_DEPLOY_KEY`.

### CI images

Built locally and pushed to the registry when their Dockerfile changes:

```bash
docker buildx build -t git.js-node.cc/jsprague/svrctlrs-ci:latest      -f .forgejo/Dockerfile.ci .
docker buildx build -t git.js-node.cc/jsprague/tauri-desktop-ci:latest -f .forgejo/Dockerfile.tauri-desktop .
docker buildx build -t git.js-node.cc/jsprague/tauri-android-ci:latest -f .forgejo/Dockerfile.tauri-android .
docker push git.js-node.cc/jsprague/<name>:latest
```

`svrctlrs-ci` layers Node on `rust-ci` (from the rsthere repo); the Tauri images layer on
`svrctlrs-ci`. Build them on epyc-ai or another fast machine, not the laptop.

## Releasing

1. Pick the version. 3.x is this product; 1.x/2.x tags belong to the retired platform.
2. Set it in `Cargo.toml` (workspace), `ui/package.json` (`npm version X.Y.Z --no-git-tag-version`
   in `ui/`), `ui/src-tauri/tauri.conf.json` and `ui/src-tauri/Cargo.toml`; run
   `cargo update --workspace` so `Cargo.lock` follows.
3. Add a `## [X.Y.Z] - YYYY-MM-DD` section to `CHANGELOG.md`; the release job uses it as notes.
4. Commit, make sure CI is green, then `git tag vX.Y.Z && git push origin vX.Y.Z`.
5. The tag publishes the image, the APK and the desktop packages to the Forgejo release. The
   release job fails if the tag does not equal `v` + the `Cargo.toml` version.

`master` auto-deploys to docker-vm; tags never deploy. To ship a release to docker-vm, merge it
to `master` (or run `deploy.sh <sha>` on docker-vm).

## Branching

`master` is the integration and deploy branch. Work on short-lived branches; CI runs on all of
them. The `ralph/*` branches were the 2026-03 autonomous-loop phases and are merged.
