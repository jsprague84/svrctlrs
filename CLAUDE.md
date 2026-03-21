# CLAUDE.md — AI Development Guide

**Last Updated:** 2026-03-20
**Architecture:** Terminal/SSH Management Tool (SvelteKit 5 + Axum + SQLite)
**Active Branch:** `ralph/phase1-foundation` (branched from `ralph/code-quality-audit`)

> **WARNING:** The `main` branch contains a completely different, stale architecture (job-based
> automation with HTMX/Askama). Never use `main` as a base for new work. Always branch from the
> active development branch listed above.

---

## Project Purpose

SvrCtlRS is a **terminal/SSH management tool** for remote server access. It provides:
- Interactive PTY terminal sessions via WebSocket (xterm.js 6 + russh)
- Non-interactive command execution (async-ssh2-tokio)
- Server profile and credential management
- Multi-tab, split-pane terminal layouts
- SSH host key verification (TOFU — Trust On First Use)

## Architecture

### Frontend: SvelteKit 5 SPA (`ui/`)
- **Framework:** SvelteKit 5 with Svelte 5 runes (`$state`, `$derived`, `$effect`, `$props`)
- **Styling:** Tailwind CSS v4 with `@theme` tokens (Tokyo Night dark + light themes)
- **Terminal:** xterm.js 6.x with WebGL rendering, addons: fit, search, web-links, webgl, unicode11, clipboard, image, serialize
- **Build:** Vite 8, adapter-static (SPA output to `ui/build/`)
- **State:** Svelte 5 runes in `ui/src/lib/state/*.svelte.ts` modules
- **Icons:** lucide-svelte
- **API Client:** `ui/src/lib/api/client.ts` — thin wrappers around fetch with `ApiError` class

### Backend: Axum (Rust) (`server/`)
- **Framework:** Axum 0.8 with tower middleware
- **Database:** SQLite via sqlx with sequential migrations
- **SSH:** russh (PTY/interactive), async-ssh2-tokio (non-interactive CMD mode)
- **Auth:** tower-sessions with SQLite store, argon2 password hashing
- **Encryption:** AES-256-GCM for credentials at rest (`core/src/encryption.rs`)

### Workspace Crates
- **`core/`** — Shared types, encryption, error handling
- **`database/`** — SQLite models, queries, migrations
- **`server/`** — Axum server, routes, WebSocket handlers
- **`scheduler/`** — Legacy crate, unused on this branch (retained in workspace for compatibility)

## Build Commands

```bash
# Frontend (SvelteKit)
cd ui
npm install          # Install dependencies
npm run dev          # Dev server (Vite HMR)
npm run build        # Production build (adapter-static)
npm run check        # TypeScript + Svelte typecheck

# Backend (Rust)
cargo build --workspace                    # Build all crates
cargo clippy --workspace -- -D warnings    # Lint (treat warnings as errors)
cargo test --workspace                     # Run tests
```

## Directory Structure

```
ui/                              # SvelteKit 5 frontend
├── src/
│   ├── routes/                  # Page routes (+page.svelte, +layout.svelte)
│   ├── lib/
│   │   ├── api/                 # API client modules (client.ts, servers.ts, credentials.ts, etc.)
│   │   ├── components/
│   │   │   ├── terminal/        # Terminal components (TerminalPane, Tabs, SplitView, CommandInput, etc.)
│   │   │   ├── layout/          # Layout components (Sidebar)
│   │   │   └── ui/              # Shared UI primitives (Button, Modal, Input, Select, Badge, Toast)
│   │   ├── state/               # Svelte 5 runes state modules (*.svelte.ts)
│   │   ├── types/               # TypeScript type definitions
│   │   └── utils/               # Utility functions (error.ts)
│   ├── app.css                  # Tailwind v4 @theme tokens + theme overrides
│   └── app.html                 # HTML entry point
server/                          # Axum backend
├── src/
│   ├── main.rs                  # Server entry, middleware, route registration
│   ├── config.rs                # TOML configuration loading
│   ├── state.rs                 # AppState (DB pool, config)
│   ├── ssh.rs                   # SSH utilities (DEPRECATED — uses NoCheck, prefer terminal routes)
│   └── routes/
│       ├── api/                 # REST API endpoints
│       │   ├── mod.rs           # Route registration via routes() -> Router<AppState>
│       │   ├── credentials.rs   # Credential CRUD
│       │   ├── servers.rs       # Server CRUD + connection test
│       │   └── settings.rs      # Settings CRUD
│       ├── terminal.rs          # CMD mode WebSocket (non-interactive)
│       ├── terminal_pty.rs      # PTY mode WebSocket (interactive shell)
│       └── ui/                  # Auth routes (login/logout)
database/                        # SQLite database layer
├── src/
│   ├── models/                  # Rust structs (Server, Credential, User, TerminalProfile, etc.)
│   └── queries/                 # Query functions per model (use QueryBuilder for dynamic SQL)
├── migrations/                  # Sequential SQL migrations (000-018+)
core/                            # Shared utilities
├── src/
│   ├── encryption.rs            # AES-256-GCM credential encryption
│   ├── error.rs                 # Error types
│   └── types.rs                 # Shared types
```

## Key Patterns

### State Management (Frontend)
State modules in `ui/src/lib/state/*.svelte.ts` export getter functions wrapping `$state`:
```typescript
let data = $state<T[]>([]);
let error = $state<string | null>(null);
export function getData() { return data; }
export function getError() { return error; }
export async function loadData() {
    try { data = await api.list(); }
    catch (e) { error = extractErrorMessage(e, 'Failed to load'); }
}
```

### Error Handling (Frontend)
Use `extractErrorMessage()` from `$lib/utils/error.js` in all catch blocks. It handles `ApiError` body extraction, `Error` instances, and unknown types.

### Svelte 5 Component Props
Use **callback props** (`onSomething: () => void`), NOT `createEventDispatcher` (Svelte 4). Example: `onStatusChange`, `onOpenPalette`.

### Database Queries (Backend)
- Use `sqlx::QueryBuilder` for dynamic UPDATE queries (not manual String::push_str)
- Use `anyhow::Context` for error handling throughout
- List operations redact sensitive fields (credential values)
- All queries scoped by user_id where applicable

### API Route Registration
New API routes go in `server/src/routes/api/mod.rs` via `.nest()` in the `routes()` function. Do NOT modify `main.rs` for route registration.

### Terminal Modes
- **PTY** (default): Interactive shell via russh — keyboard input goes directly to shell
- **CMD**: Non-interactive command execution via async-ssh2-tokio — for scripted/automated commands

### Theme System
CSS variables defined in `app.css` `@theme` block. Dark (Tokyo Night) and light themes via `[data-theme="light"]`. Terminal themes in `terminal-theme.ts` sync with app theme reactively.

### Mobile Breakpoint
`md:` (768px). Below = mobile layout. Use `isMobile()` from `$lib/state/mobile.svelte.ts` for JS-level responsive logic. Tailwind `md:` prefix for CSS-only responsive.

## Configuration

- **Database:** `DATABASE_URL=sqlite:data/svrctlrs.db`
- **SPA:** `SPA_DIR=ui/build` (path to SvelteKit static build)
- **Auth:** `ADMIN_USERNAME` + `ADMIN_PASSWORD` env vars (initial admin user)
- **Encryption:** `ENCRYPTION_KEY` env var (required — 64 hex chars for AES-256-GCM)
- **Session:** `SESSION_SECURE=true/false` (set false for local dev without HTTPS)

## Tauri Desktop App

The SvelteKit SPA is wrapped in Tauri v2 for native desktop builds. The Tauri project lives in `ui/src-tauri/` (excluded from the root Cargo workspace).

```bash
# Development (opens native window with Vite HMR)
cd ui && npm run tauri:dev

# Production build (produces binary + .deb + .rpm)
cd ui && npm run tauri:build
```

**System requirements (Fedora):** `webkit2gtk4.1-devel`, `libsoup3-devel`, `openssl-devel`, `librsvg2-devel`, `libayatana-appindicator-gtk3-devel`

### Platform Detection
- `ui/src/lib/platform/index.ts` — `isWeb()`, `isTauri()`, `isTauriMobile()`, `isTauriDesktop()`
- `ui/src/lib/platform/keyboard.ts` — mobile keyboard visibility detection (web fallback)
- In Tauri mode, API calls use a configurable server URL from localStorage (`svrctlrs-server-url`)
- In web mode, relative URLs (same-origin) — no configuration needed

### Key Differences (Web vs Tauri)
- **Web:** SPA served by Axum, API same-origin, credentials: 'same-origin'
- **Tauri:** SPA loaded from filesystem, API to remote server, credentials: 'include', requires server URL setup on first launch

## Future Direction

Push notifications via rstify (self-hosted, Gotify/ntfy compatible). Tauri mobile app (iOS/Android) planned for Phase 4. See `docs/superpowers/specs/2026-03-20-svrctlrs-next-phase-design.md` for the full design specification.
