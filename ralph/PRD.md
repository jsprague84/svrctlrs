# PRD: SvrCtlRS Optimization & Terminal-First Refactor

**Version**: 1.0
**Created**: 2026-02-28
**Status**: Ready for Implementation
**Priority**: High

---

## Problem Statement

SvrCtlRS is an infrastructure automation platform, but the primary user workflow is **using it as a preconfigured web terminal to SSH into local servers from anywhere**. The current app has:

1. **No authentication** - Anyone with network access can SSH into any configured server
2. **Credentials stored in plaintext** - SSH keys and passwords unencrypted in SQLite
3. **Terminal buried in a modal** - The most-used feature is a secondary UI element
4. **SSH host keys not verified** - Vulnerable to man-in-the-middle attacks
5. **No graceful shutdown** - Server crash leaves orphaned connections
6. **Deprecated code** lingering in the codebase
7. **No request timeouts or security headers** - Missing standard web security

The application will later be forked to remove job scheduling and focus entirely on server management + terminal access. This PRD optimizes the app for that future direction.

---

## Goals

1. **Security**: Authentication, credential encryption, SSH host key verification
2. **Terminal UX**: Promote terminal from modal to full-page primary interface
3. **Modernization**: WebGL rendering, timeout middleware, security headers
4. **Cleanup**: Remove deprecated code, prepare architecture for fork

---

## Non-Goals

- Adding new job/scheduling features (these will be removed in fork)
- Multi-user RBAC (single-user auth is sufficient for now)
- Kubernetes or cloud provider integration

---

## Phase 1: Security Foundation

### 1.1 User Accounts & Password Hashing
- Add `users` table: id, username, password_hash, created_at, updated_at
- Use `argon2` crate for password hashing (modern replacement for bcrypt)
- Create initial admin user via environment variable or first-run setup
- Migration: `016_user_accounts.sql`

### 1.2 Login/Logout with Session Management
- Integrate `tower-sessions` + `tower-sessions-sqlx-store` (already in Cargo.toml)
- Implement login form submission with password verification
- Session cookie with configurable expiry
- Logout clears session
- Login page uses existing Askama templates

### 1.3 Authentication Middleware
- Tower middleware layer that checks session on every request
- Exempt routes: `/auth/login`, `/static/*`, health check
- Redirect unauthenticated users to login page
- Return 401 for unauthenticated API/WebSocket requests

### 1.4 Credential Encryption at Rest
- Encrypt credential `value` field using AES-256-GCM
- Encryption key from `ENCRYPTION_KEY` environment variable
- Migrate existing plaintext credentials to encrypted format
- Decrypt on read, encrypt on write (transparent to rest of app)

---

## Phase 2: Terminal-First Experience

### 2.1 Full-Page Terminal View
- New route: `GET /terminal` - dedicated full-page terminal
- Full viewport terminal with sidebar for server selection
- Server list with connection status indicators
- Quick-connect: click a server to open terminal immediately
- Replace modal as primary terminal interface (modal kept for command template testing)

### 2.2 PTY Mode Integration in Frontend
- Add toggle in terminal UI: "Command Mode" vs "Interactive Shell"
- Command mode: existing non-interactive WebSocket (`/ws/terminal`)
- Shell mode: PTY WebSocket (`/ws/terminal/pty`) with full interactive support
- Auto-detect mode based on command (e.g., `vim`, `top` → suggest PTY)
- Visual indicator showing current mode

### 2.3 xterm.js WebGL Renderer
- Load WebGL addon for GPU-accelerated rendering
- Graceful fallback to canvas on WebGL context loss
- Add Unicode11 addon (already loaded but not activated)
- Configure proper font stack with ligature support

### 2.4 Server Quick-Connect Dashboard
- Dashboard cards for each server with one-click "Connect" button
- Show server status (last seen, health indicator)
- Recent connections history
- Pinned/favorite servers at top

---

## Phase 3: Infrastructure Hardening

### 3.1 SSH Host Key Verification
- Implement known_hosts file management
- On first connect: prompt user to accept host key (TOFU model)
- Store accepted keys in database (`server_host_keys` table)
- Reject connections when host key changes (with override option)

### 3.2 Graceful Shutdown & Security Headers
- Handle SIGTERM/SIGINT for clean shutdown
- Close active WebSocket connections gracefully
- Add security headers: CSP, X-Frame-Options, X-Content-Type-Options
- Restrict CORS to configured origins (not permissive)

### 3.3 Request Timeout Middleware
- Add `tower::timeout::TimeoutLayer` with `HandleErrorLayer`
- Default 30s timeout for HTTP requests
- Separate timeout configuration for WebSocket upgrade requests
- Return proper 408 Request Timeout responses

### 3.4 Remove Deprecated RemoteExecutor
- Delete `core/src/remote.rs` (shell-based SSH executor)
- Remove all references from `core/src/lib.rs`
- Update `server/src/state.rs` to remove `executor` field
- Verify no remaining callers

---

## Technical Notes

### Dependencies to Add
- `argon2` - Password hashing (replaces bcrypt, recommended by OWASP)
- `aes-gcm` - Credential encryption at rest
- `rand` - Secure random key generation

### Dependencies Already Present (Unused)
- `tower-sessions` 0.13 - In Cargo.toml but not integrated
- `tower-sessions-sqlx-store` 0.13 - In Cargo.toml but not integrated

### xterm.js Modern Best Practices (from Context7)
- WebGL addon: `@xterm/addon-webgl` for GPU-accelerated rendering
- Handle `onContextLoss` for GPU driver recovery
- Fit addon already used correctly
- AttachAddon available for cleaner WebSocket piping (evaluate vs current custom protocol)

### Axum 0.8 Best Practices (from Context7)
- `HandleErrorLayer` wrapping `TimeoutLayer` for proper error responses
- `ServiceBuilder` for composing middleware layers
- Custom middleware via `axum::middleware::from_fn` for auth guard
