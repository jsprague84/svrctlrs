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
SESSION_SECURE=false ADMIN_PASSWORD=dev-password-1 ENCRYPTION_KEY=$(openssl rand -hex 32) \
  DATABASE_URL=sqlite:data/svrctlrs.db cargo run --package server

# terminal 2 — SPA with HMR; /api and /ws are proxied to :8080
cd ui && npm ci && npm run dev
```

Open http://localhost:5173 and log in as `admin`/`dev-password-1`. To test the server-served SPA instead,
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

Tests today: about 175 Rust tests across `core`, `database` and `server` (auth middleware,
API tokens and their presets, approvals, host keys, limiter, tickets, session registry and
replay ring, `ssh_config` import, the API's response shapes and OpenAPI document, svrctl
backup) and 167 Vitest tests in the UI, most of them for the terminal session protocol
(`ui/src/lib/terminal/ptySession.test.ts`: attach and replay, keep-alive watchdog, reconnect
backoff, host-key challenge, persistence, predictive echo) driven with a fake socket and fake
timers, and for the predictive echo itself (`localEcho.test.ts`, against a small fake screen
in `fakeEchoScreen.ts`: epochs, password prompts, the alternate screen, mismatches,
backspace, timeouts, the latency gate). Security-relevant changes come with tests.

The SSH client is tested in CI against an **in-process SSH server**
(`server/src/test_support/ssh_server.rs`): russh's server side on a loopback port with its own
host key, a password and/or key login, a pretend shell (echoes input, answers `exit N` and
`big N`, records PTY, environment, resize and input requests) and `direct-tcpip`, so it can be
its own jump host. `test_support/mod.rs` adds a migrated in-memory database, the encryption
key and helpers to add servers and credentials and to trust recorded host keys. The tests in
`ssh.rs` (`loopback_tests`) cover first contact, trust, a changed key, password and key logins
and a jump host with a loop; `routes/terminal_pty.rs` drives the PTY driver (`open_shell`,
`spawn_pty_driver`) through a session: startup command, input, resize, exit status, close, a
shell through a jump host and a 300 kB burst. They take under a second and need no network
beyond 127.0.0.1. The loopback server also answers exec requests (`echo X`, `exit N`), so the
connection test's success path runs too, and it applies the `authorized_keys` install and
removal commands of `key_ops.rs` to its accepted keys, so authorize-key, onboard and
rotate-key are tested end to end (`routes/api/ops.rs`): a new key really logs in afterwards
and a removed one really does not.
With `Options::with_sftp(dir)` it serves a directory as `/` over the `sftp` subsystem
(`test_support/sftp_server.rs`; a file without its owner-write bit refuses writes even as
root), and `routes/api/files.rs` (`loopback`) drives the file routes through the whole app:
listing, download, upload, folders, rename, delete, and the editor's tagged save (`ETag` and
`If-Match`: a change behind its back, a deleted file, a large file, a symlink, a read-only
file).

`server/tests/svrctl.rs` starts the real `server` binary on a temporary database and drives
the real `svrctl` binary against it: JSON on a pipe, exit codes, errors with hints, token
presets, `import` from stdin, and a delete held for approval that `--wait` sees through once
the owner approves.

`scripts/eval/hermes/` is the agent eval of ROADMAP item 19: a throwaway svrctlrs and three
SSH hosts in Docker on the Hermes VM, five tasks run as one-shot Hermes sessions, a grader for
the end state, and a transcript summarizer; its README has the results. Run it after changing
`svrctl`, the skill or the operations.

**API types and the OpenAPI document.** Handlers return types from the `svrctlrs-api` crate
(`api/src/*.rs`, `Serialize + ToSchema`) and are registered with
`OpenApiRouter::routes(routes!(…))` plus `#[utoipa::path(…)]`, so a route is routed and
documented by the same line (`server/src/routes.rs::openapi()` assembles the document). Two
tests guard the contract:

- `server/testdata/api-shapes.json` records the JSON shape (keys and value types) of about 90
  responses, produced through the real router (`server/src/api_shapes.rs`). A change to any
  key or type fails the test; after a deliberate change run
  `UPDATE_API_SHAPES=1 cargo test -p server api_shapes` and review the diff (the web UI reads
  these responses).
- `docs/openapi.json` must equal the generated document; regenerate with
  `UPDATE_OPENAPI=1 cargo test -p server openapi_document`. A third test checks that every
  route a token preset allows is in the document.

A new endpoint: types in `api/src/<module>.rs`, `#[utoipa::path]` on the handler,
`routes!(handler)` in the module's `routes()`, a line in the preset table in
`server/src/api_tokens.rs` (or none: tokens are refused by default), a case in
`api_shapes.rs`, then both regenerations.

`scripts/dev/pty-session-e2e.py` drives the SSH and session paths end to end against a locally
running server and a real SSH target: first-contact host-key confirmation, the connection
test, a jump host (the target through itself), SFTP, tags, `ssh_config` import, the startup
command, then the persistent-session protocol (shell, dropped socket, reattach with replay,
`closed`, `session_gone`, TTL), and session recording (a resize in the cast, a password typed
at a `stty -echo` prompt absent from it, 404 for a second account on every route), and agent
tokens (a full-admin token opens a shell through the ticket exchange without a cookie, its
audit row names it, a read-only token gets 403 for tickets and files, revoking ends the
shell), and approvals (a token set to ask first gets 202 for a delete, cannot approve it,
and the owner's approval makes the call), and the operations against the real host:
diagnose (direct, through the jump host, a closed port), exec (stdout, stderr, exit status, a
timeout) and onboard without a new key. Rotate-key and onboarding with a new key stay in the
loopback tests: against a real host they would edit its `authorized_keys`.
`scripts/dev/run-e2e.sh` starts a throwaway debug server and
runs both frame formats:

```bash
cargo build -p server && (cd ui && npm run build)
E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 scripts/dev/run-e2e.sh
```

Run it after touching `ssh.rs`, `sessions.rs`, `terminal_pty.rs`, the servers API or the
pane's transport (`ui/src/lib/terminal/`). It is what the loopback tests cannot be: a real
OpenSSH `sshd`, SFTP, the WebSocket protocol in both frame formats, and the session API. CI
cannot run it (no SSH target); say in the commit that it passed.

`scripts/dev/run-e2e.sh backpressure` runs `scripts/dev/backpressure-check.py` instead: a
200 MiB burst read at full speed (every byte, in order), the same burst against a client that
stops reading for 15 s (the server's memory, read from `/proc`, must stay within 64 MB of where
it started; output the ring no longer holds arrives as `replay_window_lost` plus a redraw),
and Ctrl-C under `yes`. Run it after changing how output reaches sockets (`sessions.rs`'s
`Follower`, the forwarder in `terminal_pty.rs`). `BURST_MB`, `STALL_SECS` and `LIMIT_MB` adjust
it.

`scripts/dev/pane-phone-check.py` is the same idea for the terminal pane on a phone: a Pixel 7
emulation in Chromium with real touch events (CDP), against a throwaway server and the same
SSH target. It goes through the first-contact dialog, typing, tap-to-move-cursor, long-press
copy, a swipe between two tabs, CTL from the extra-keys row, predictive echo over a link it
slows itself (incoming socket messages delayed 250 ms in the page: typed text is drawn ahead
of the echo and cleared after it, and nothing is drawn at a `read -s` prompt) and a reload that
resumes the shell, account preferences (a change on the phone reaches the server and a fresh
desktop browser) and a quick command with variables run through its dialog, and fails if a
hidden pane resizes its shell. Run it after touching
`TerminalPane.svelte`, `TerminalView.svelte` or `ui/src/lib/terminal/`:

```bash
E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu CHROMIUM=$(command -v chromium) scripts/dev/run-phone-check.sh
THEME=light E2E_SSH_HOST=… scripts/dev/run-phone-check.sh   # the same in light mode
```

`scripts/dev/ui-screens.py` (run by `scripts/dev/run-ui-screens.sh`) seeds a throwaway server
with sample servers, tags, credentials, quick commands and a second account, then takes a
screenshot of every screen at phone and desktop size in both themes. It fails if a page
scrolls sideways, if a phone control is under 40 px to the finger (the extra-keys row is the
one deliberate exception), if Escape does not close a modal or the row menu, or if Tab leaves a
modal. On Settings → Access it creates a full-admin token that never expires (the dialog
must warn), reads it from the copy-once display, uses it from outside the browser (200 on the
server list, 403 on `/tokens`), checks the list never shows it, and revokes it (then 401);
then a token set to ask first tries to delete a server, the link opens the request on
`/approvals`, and Deny leaves the server in place.
With an SSH target it adds a live shell, the file browser, the file viewer and the Sessions
panel (in both themes), and (the live server records sessions) the recordings list and the player, checking that the player shows the
recorded shell and closes on Escape; the pictures in
`docs/images/` come from that run (dark theme, phone shots at half size, `pngquant`-ed). The
live server is named `lab-01`, and each shot first rewrites its real address on the page to
`192.0.2.10` (a documentation address) and fails if the address is still visible, so published
pictures never show the target. Logged out, it shoots the server's sign-in page under a dark
and a light OS theme and checks that it, and a first visit to the app, follow the OS. Run it after UI changes:

```bash
E2E_OUT=/tmp/shots CHROMIUM=$(command -v chromium) scripts/dev/run-ui-screens.sh   # SCREENS=phone-dark to narrow it
```

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
| `docker.yaml` | `master`, `v*`, manual | `build` | builds the image with buildx for `linux/amd64` and `linux/arm64` (cross-compiled on the amd64 runner: every `RUN` happens on `$BUILDPLATFORM`, the runtime stage only copies, so no QEMU), pushes `latest`/`sha`/`vX.Y.Z` to the Forgejo registry, in a named buildx builder (`svrctlrs`) that outlives the job so cargo-chef's dependency layer and sccache's cache mount stay warm (cache capped at 20 GB; Forgejo's registry does not keep a BuildKit registry cache, tried 2026-10-21); on tags creates the release; on `master` waits for the commit's CI push run to be green (a PR run on the same commit does not count) and deploys to docker-vm |
| `desktop.yaml` | `v*`, manual | `rust-high` | `.deb`/`.rpm` (gate) and AppImage (best effort), attached to the release; a manual run keeps them as a workflow artifact |
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
   `cargo update --workspace` so `Cargo.lock` follows, and regenerate `docs/openapi.json`
   (`UPDATE_OPENAPI=1 cargo test -p server openapi_document`), which carries the version.
3. Add a `## [X.Y.Z] - YYYY-MM-DD` section to `CHANGELOG.md`; the release job uses it as notes.
4. Commit, make sure CI is green, then `git tag vX.Y.Z && git push origin vX.Y.Z`.
   The Nix flake (`flake.nix`, `packaging/nix/desktop.nix`) reads the version from
   `ui/src-tauri/Cargo.toml`; after a change to `ui/package-lock.json` update its `npmDeps`
   hash (`nix run nixpkgs#prefetch-npm-deps -- ui/package-lock.json`) and check
   `nix build .#desktop`.
5. The tag publishes the image, the APK and the desktop packages to the Forgejo release. The
   release job fails if the tag does not equal `v` + the `Cargo.toml` version.

`master` auto-deploys to docker-vm; tags never deploy. To ship a release to docker-vm, merge it
to `master` (or run `deploy.sh <sha>` on docker-vm).

## Branching

`master` is the integration and deploy branch. Work on short-lived branches; CI runs on all of
them.
