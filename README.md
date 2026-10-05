# SvrCtlRS

Self-hosted terminal and SSH management for your servers: interactive shells in the browser,
on the desktop, and on Android, with server profiles, encrypted credentials, multi-pane layouts
and push notifications. One Rust binary, one SQLite file.

**Version 3.0.0** · Rust (Axum) · SvelteKit 5 · xterm.js 6 · Tauri 2 · SQLite

> 3.x is a different product from the 1.x/2.x releases. Those were a plugin-based monitoring
> platform and were retired in March 2026; their code and documentation live in
> `docs-archived/`.

## What it does

- **Interactive terminals** — full PTY sessions over WebSocket (russh on the server, xterm.js
  with WebGL rendering in the client). Tabs, four layouts (single, split horizontal, split
  vertical, quad), up to 10 sessions, output search, session restore across page navigation.
- **Command mode** — run a one-off command on a server and capture its output and exit code,
  without a shell.
- **Servers and credentials** — server profiles (host, port, user, credential), password and
  SSH-key credentials encrypted at rest with AES-256-GCM, and a `local` server that opens a
  shell inside the container.
- **Host-key verification** — trust on first use; a changed key blocks the connection.
- **Terminal profiles** — save a layout plus which server opens in each pane; mark one as the
  default to auto-load at start.
- **Quick commands** — a per-user library of snippets, grouped by category, launched from the
  command palette (`Ctrl+K`).
- **Notifications** — [rstify](https://git.js-node.cc/jsprague/rstify)/Gotify/ntfy push for
  server health checks and command completion.
- **Clients** — the web UI served by the server; a Tauri desktop app for Linux (`.deb`,
  `.rpm`, AppImage, Flatpak); and a Tauri Android app with a mobile layout, touch gestures, an
  extra-keys row and swipe navigation between tabs.

## Quick start (Docker)

```bash
mkdir -p ~/docker-compose/svrctlrs && cd ~/docker-compose/svrctlrs
curl -fsSLO https://git.js-node.cc/jsprague/svrctlrs/raw/branch/master/docker-compose.yml
curl -fsSL  https://git.js-node.cc/jsprague/svrctlrs/raw/branch/master/.env.example -o .env
chmod 600 .env
# Edit .env: set ADMIN_PASSWORD and ENCRYPTION_KEY (openssl rand -hex 32)
docker compose up -d
```

The compose file is written for a Traefik edge with a `proxy` network; for a plain host, remove
the `labels:` and add `ports: ["8080:8080"]`. The first start creates the admin user from
`ADMIN_USERNAME`/`ADMIN_PASSWORD`. Full instructions, including the reverse-proxy and
cross-origin settings the desktop and Android apps need, are in
[docs/DEPLOYMENT.md](docs/DEPLOYMENT.md).

Images are published to `git.js-node.cc/jsprague/svrctlrs` with tags `latest`, `vX.Y.Z` and
the commit SHA.

## Desktop and Android

Packages are attached to each [release](https://git.js-node.cc/jsprague/svrctlrs/releases). On
first launch the app asks for your server URL and checks `/api/v1/health`; after that you log
in with your SvrCtlRS account. The server must list the app's origin in `ALLOWED_ORIGINS` (see
the deployment guide).

## Documentation

| Document | For |
|---|---|
| [docs/USER-GUIDE.md](docs/USER-GUIDE.md) | Using the terminal, layouts, profiles, quick commands, mobile gestures |
| [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) | Running the server: compose, environment, reverse proxy, backups, upgrades |
| [docs/SECURITY.md](docs/SECURITY.md) | Authentication, encryption, host keys, what to protect, known weaknesses |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crates, UI structure, request flow, data model |
| [docs/API.md](docs/API.md) | REST and WebSocket reference |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Building, testing, CI/CD, releasing |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Gaps and planned improvements, prioritized |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## Development

```bash
# Backend (needs a Rust toolchain)
cargo run --package server              # http://localhost:8080, SESSION_SECURE=false for http
# Frontend (Node 22+)
cd ui && npm ci && npm run dev          # Vite dev server proxies /api and /ws to :8080
```

Quality gates, the same ones CI runs: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and in `ui/`
`npm run check && npm run build`. See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

CI/CD runs on Forgejo Actions at `git.js-node.cc`, on the project's own runners. Every push is
linted and tested; `master` publishes an image and deploys; `v*` tags publish an image, the
Android APK and the desktop packages to a release.

## License

MIT or Apache-2.0, at your option — see [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
