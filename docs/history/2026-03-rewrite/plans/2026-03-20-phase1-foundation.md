# Phase 1: Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite CLAUDE.md, clean up legacy database tables, upgrade all dependencies to latest versions, and add terminal session persistence, profiles UI, command palette, and design system tokens.

**Architecture:** SvelteKit 5 SPA (frontend) + Axum/Rust (backend) + SQLite. All new features follow existing patterns: Svelte 5 runes for state, Tailwind CSS v4 for styling, sqlx QueryBuilder for dynamic SQL, Askama-free (pure JSON API + SvelteKit SPA).

**Tech Stack:** Svelte 5.54+, SvelteKit 2.55+, Tailwind CSS 4.2+, xterm.js 6.0+, Vite 8+, Axum 0.8, sqlx, russh

**Spec Reference:** `docs/superpowers/specs/2026-03-20-svrctlrs-next-phase-design.md`

**Branch Strategy:** Create new branch `ralph/phase1-foundation` from `ralph/code-quality-audit`. The `main` branch is stale and must never be used as a base.

---

## File Structure

### New Files
- `database/migrations/019_cleanup_and_quick_commands.sql` — Drop legacy tables, add quick_commands table, extend terminal_profiles
- `ui/src/lib/api/profiles.ts` — Terminal profiles API client
- `ui/src/lib/api/quickCommands.ts` — Quick commands API client
- `ui/src/lib/state/profiles.svelte.ts` — Profiles reactive state
- `ui/src/lib/state/quickCommands.svelte.ts` — Quick commands reactive state
- `ui/src/lib/components/terminal/CommandPalette.svelte` — Command palette overlay/bottom sheet
- `ui/src/lib/components/terminal/ProfileManager.svelte` — Profile save/load UI
- `ui/src/lib/types/profile.ts` — Profile and QuickCommand TypeScript types
- `server/src/routes/api/profiles.rs` — Terminal profiles REST endpoints
- `server/src/routes/api/quick_commands.rs` — Quick commands REST endpoints
- `database/src/models/quick_command.rs` — QuickCommand model
- `database/src/queries/quick_commands.rs` — QuickCommand queries

### Modified Files
- `CLAUDE.md` — Complete rewrite
- `ui/package.json` — Dependency upgrades + @xterm/addon-serialize
- `ui/src/app.css` — Design system tokens (density, motion)
- `ui/src/lib/components/terminal/TerminalPane.svelte` — Serialize addon, double-tap gesture
- `ui/src/lib/components/terminal/TerminalTabs.svelte` — Profile quick-load indicator
- `ui/src/lib/components/layout/Sidebar.svelte` — Profiles section
- `ui/src/lib/types/terminal.ts` — Extended TerminalPreferences type
- `ui/src/lib/state/terminal.svelte.ts` — Profile load/save integration
- `ui/src/routes/+page.svelte` — Ctrl+K shortcut, command palette integration
- `database/src/models/terminal_profile.rs` — PaneConfig.mode field, user_id
- `database/src/models/mod.rs` — Export quick_command module
- `database/src/queries/mod.rs` — Export quick_commands module
- `database/src/queries/terminal_profiles.rs` — User-scoped queries
- `server/src/routes/api/mod.rs` — Register new routes
- `server/src/main.rs` — Wire new routers

---

## Task 1: Create Branch and Upgrade Dependencies

**Files:**
- Modify: `ui/package.json`

- [ ] **Step 1: Create the new branch**

```bash
git checkout ralph/code-quality-audit
git checkout -b ralph/phase1-foundation
```

- [ ] **Step 2: Upgrade all npm dependencies to latest**

```bash
cd ui
npm install @xterm/xterm@latest @xterm/addon-fit@latest @xterm/addon-search@latest @xterm/addon-web-links@latest @xterm/addon-webgl@latest @xterm/addon-unicode11@latest @xterm/addon-clipboard@latest @xterm/addon-image@latest @xterm/addon-serialize@latest
npm install svelte@latest @sveltejs/kit@latest @sveltejs/adapter-static@latest @sveltejs/vite-plugin-svelte@latest
npm install tailwindcss@latest @tailwindcss/vite@latest
npm install vite@latest typescript@latest svelte-check@latest
npm install lucide-svelte@latest
```

- [ ] **Step 3: Check for xterm 6.0 breaking changes**

Read the xterm.js 6.0 changelog/migration guide. Key areas to verify:
- Import path changes
- Addon API changes (constructor signatures, method names)
- Terminal options deprecations
- WebGL addon API changes

```bash
cd ui && npm run check
```

Fix any TypeScript errors from the upgrade. Common xterm 6.0 changes:
- `Terminal` constructor options may have renamed fields
- Addon `.activate()` method may be replaced
- Some events may have changed signatures

- [ ] **Step 4: Verify build passes**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 5: Verify Rust workspace builds**

```bash
cargo clippy --workspace -- -D warnings && cargo test --workspace
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: upgrade all dependencies to latest versions

- xterm.js 5.5 -> 6.0 (major) with all addon upgrades
- svelte 5.53 -> 5.54, sveltekit 2.53 -> 2.55
- vite 7.3 -> 8.0 (major)
- tailwindcss 4.2.1 -> 4.2.2
- Add @xterm/addon-serialize for session persistence"
```

---

## Task 2: Rewrite CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Read the current CLAUDE.md**

Read the full file to understand what's there (1,413 lines of stale job-system documentation).

- [ ] **Step 2: Write the new CLAUDE.md**

Replace the entire file with content reflecting the current terminal-first architecture. The new CLAUDE.md should cover:

```markdown
# CLAUDE.md — AI Development Guide

**Last Updated:** 2026-03-20
**Architecture:** Terminal/SSH Management Tool (SvelteKit + Axum + SQLite)
**Active Branch:** ralph/phase1-foundation (branched from ralph/code-quality-audit)

> **WARNING:** The `main` branch contains a completely different, stale architecture (job-based
> automation with HTMX/Askama). Never use `main` as a base for new work. Always branch from the
> active development branch listed above.

---

## Project Purpose

SvrCtlRS is a **terminal/SSH management tool** for remote server access. It provides:
- Interactive PTY terminal sessions via WebSocket (xterm.js + russh)
- Non-interactive command execution (async-ssh2-tokio)
- Server profile and credential management
- Multi-tab, split-pane terminal layouts
- SSH host key verification (TOFU)

## Architecture

### Frontend: SvelteKit 5 SPA
- **Framework:** SvelteKit 5 with Svelte 5 runes ($state, $derived, $effect)
- **Styling:** Tailwind CSS v4 with @theme tokens (Tokyo Night dark + light themes)
- **Terminal:** xterm.js 6.x with WebGL rendering, 8+ addons
- **Build:** Vite 8, adapter-static (SPA output to ui/build/)
- **State:** Svelte 5 runes in ui/src/lib/state/*.svelte.ts modules
- **Icons:** lucide-svelte

### Backend: Axum (Rust)
- **Framework:** Axum 0.8 with tower middleware
- **Database:** SQLite via sqlx with migrations
- **SSH:** russh (PTY/interactive), async-ssh2-tokio (non-interactive)
- **Auth:** tower-sessions with SQLite store, argon2 password hashing
- **Encryption:** AES-256-GCM for credentials at rest

### Workspace Crates
- **core/** — Shared types, encryption, error handling
- **database/** — SQLite models, queries, migrations
- **server/** — Axum server, routes, WebSocket handlers

## Build Commands

### Frontend (SvelteKit)
cd ui
npm install          # Install dependencies
npm run dev          # Dev server (Vite HMR)
npm run build        # Production build (adapter-static)
npm run check        # TypeScript + Svelte typecheck
npm run preview      # Preview production build

### Backend (Rust)
cargo build --workspace                    # Build all crates
cargo clippy --workspace -- -D warnings    # Lint
cargo test --workspace                     # Run tests

## Directory Structure

ui/                           # SvelteKit 5 frontend
├── src/
│   ├── routes/               # Page routes (+page.svelte, +layout.svelte)
│   ├── lib/
│   │   ├── api/              # API client modules (client.ts, servers.ts, etc.)
│   │   ├── components/
│   │   │   ├── terminal/     # Terminal components (TerminalPane, Tabs, SplitView, etc.)
│   │   │   ├── layout/       # Layout components (Sidebar, Header)
│   │   │   └── ui/           # Shared UI primitives (Button, Modal, Input, etc.)
│   │   ├── state/            # Svelte 5 runes state modules (*.svelte.ts)
│   │   ├── types/            # TypeScript type definitions
│   │   └── utils/            # Utility functions
│   ├── app.css               # Tailwind v4 @theme tokens
│   └── app.html              # HTML entry point
server/                       # Axum backend
├── src/
│   ├── main.rs               # Server entry, middleware, route registration
│   ├── config.rs             # TOML configuration
│   ├── state.rs              # AppState (DB pool, config)
│   ├── ssh.rs                # SSH utilities (DEPRECATED — use terminal routes)
│   └── routes/
│       ├── api/              # REST API (servers, credentials, settings)
│       ├── terminal.rs       # CMD mode WebSocket (non-interactive)
│       ├── terminal_pty.rs   # PTY mode WebSocket (interactive shell)
│       └── ui/               # Auth routes (login/logout)
database/                     # SQLite database layer
├── src/
│   ├── models/               # Rust structs (Server, Credential, User, etc.)
│   └── queries/              # Query functions per model
├── migrations/               # Sequential SQL migrations (000-018+)
core/                         # Shared utilities
├── src/
│   ├── encryption.rs         # AES-256-GCM credential encryption
│   ├── error.rs              # Error types
│   └── types.rs              # Shared types

## Key Patterns

### State Management (Frontend)
State modules in ui/src/lib/state/*.svelte.ts export getter functions wrapping $state:
  let data = $state<T[]>([]);
  export function getData() { return data; }
  export async function loadData() { ... }

### Database Queries (Backend)
Use sqlx::QueryBuilder for dynamic UPDATE queries. Use anyhow::Context for error
handling. All list operations should redact sensitive fields (credentials).

### Terminal Modes
- PTY (default): Interactive shell via russh — keyboard input goes directly to shell
- CMD: Non-interactive command execution via async-ssh2-tokio — for scripted/automated commands

### Theme System
CSS variables defined in app.css @theme block. Dark (Tokyo Night) and light themes.
Terminal themes in terminal-theme.ts sync with app theme reactively.

### Mobile Breakpoint
md: (768px). Below = mobile layout. Use isMobile() from lib/state/mobile.svelte.ts
for JS-level responsive logic. Tailwind md: prefix for CSS-only responsive.

## Future Direction

Tauri v2 integration planned for desktop (macOS/Win/Linux) and mobile (iOS/Android)
native apps. See docs/superpowers/specs/2026-03-20-svrctlrs-next-phase-design.md for
the full design specification.
```

- [ ] **Step 3: Verify no references to stale concepts remain**

Search the new CLAUDE.md for: job, plugin, HTMX, Alpine, Askama, scheduler, command_template, job_schedule. None should appear except in the WARNING about the main branch.

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: rewrite CLAUDE.md for terminal-first architecture

Complete rewrite removing all references to the stale job-based
automation system. Documents current SvelteKit + Axum + xterm.js
architecture, build commands, directory structure, and key patterns."
```

---

## Task 3: Database Cleanup Migration

**Files:**
- Create: `database/migrations/019_cleanup_and_quick_commands.sql`
- Modify: `database/src/models/terminal_profile.rs`
- Modify: `database/src/models/mod.rs`
- Create: `database/src/models/quick_command.rs`
- Modify: `database/src/queries/mod.rs`
- Create: `database/src/queries/quick_commands.rs`

- [ ] **Step 1: Write the migration**

Create `database/migrations/019_cleanup_and_quick_commands.sql`:

```sql
-- Phase 1: Drop legacy tables from the old job-based system
-- These tables were created by migrations 000-013, 015 but are no longer
-- used on the current terminal-first architecture.

DROP TABLE IF EXISTS step_execution_results;
DROP TABLE IF EXISTS server_job_results;
DROP TABLE IF EXISTS job_runs;
DROP TABLE IF EXISTS job_schedules;
DROP TABLE IF EXISTS job_template_steps;
DROP TABLE IF EXISTS job_templates;
DROP TABLE IF EXISTS command_templates;
DROP TABLE IF EXISTS job_types;
DROP TABLE IF EXISTS notification_policy_channels;
DROP TABLE IF EXISTS notification_policies;
DROP TABLE IF EXISTS notification_channels;
DROP TABLE IF EXISTS notification_log;
DROP TABLE IF EXISTS webhooks;
DROP TABLE IF EXISTS task_history;
DROP TABLE IF EXISTS metrics;
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS server_tags;
DROP TABLE IF EXISTS job_catalog_entries;
DROP TABLE IF EXISTS job_catalog_categories;

-- Phase 2: Add user_id to terminal_profiles for multi-user scoping
ALTER TABLE terminal_profiles ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE CASCADE;

-- Phase 3: Update layout enum values to match frontend convention
UPDATE terminal_profiles SET layout = 'single' WHERE layout = '1';
UPDATE terminal_profiles SET layout = 'split-h' WHERE layout = '2h';
UPDATE terminal_profiles SET layout = 'split-v' WHERE layout = '2v';
UPDATE terminal_profiles SET layout = 'quad' WHERE layout = '4';

-- Phase 4: Create quick_commands table (replaces quick_commands field on terminal_profiles)
CREATE TABLE IF NOT EXISTS quick_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    server_id INTEGER REFERENCES servers(id) ON DELETE SET NULL,
    category TEXT DEFAULT 'general',
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_quick_commands_user ON quick_commands(user_id);
CREATE INDEX IF NOT EXISTS idx_quick_commands_server ON quick_commands(server_id);

-- Phase 5: Drop the old quick_commands field from terminal_profiles
-- SQLite doesn't support DROP COLUMN before 3.35.0, so we recreate the table
-- Actually, SQLite 3.35+ does support DROP COLUMN. Check version at runtime.
-- For safety, leave the column — it will be ignored by the application.
-- The new quick_commands table replaces its functionality.
```

- [ ] **Step 2: Verify migration runs**

```bash
cargo test --workspace
```

The test suite runs migrations on in-memory SQLite databases. If tests pass, the migration is valid.

- [ ] **Step 3: Create QuickCommand model**

Create `database/src/models/quick_command.rs`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QuickCommand {
    pub id: i64,
    pub user_id: Option<i64>,
    pub name: String,
    pub command: String,
    pub server_id: Option<i64>,
    pub category: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateQuickCommand {
    pub name: String,
    pub command: String,
    pub server_id: Option<i64>,
    pub category: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateQuickCommand {
    pub name: Option<String>,
    pub command: Option<String>,
    pub server_id: Option<i64>,
    pub category: Option<String>,
    pub sort_order: Option<i32>,
}
```

- [ ] **Step 4: Extend PaneConfig with mode field**

In `database/src/models/terminal_profile.rs`, update:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneConfig {
    pub server_id: Option<i64>,
    pub mode: Option<String>,  // "pty" or "cmd"
}
```

- [ ] **Step 5: Register new model module**

Add to `database/src/models/mod.rs`:
```rust
pub mod quick_command;
```

And add to `database/src/lib.rs` re-exports if using wildcard exports.

- [ ] **Step 6: Create QuickCommand queries**

Create `database/src/queries/quick_commands.rs` with CRUD operations following the existing pattern in `terminal_profiles.rs`:
- `list_quick_commands(pool, user_id)` — scoped to user
- `list_quick_commands_for_server(pool, user_id, server_id)` — filtered by server
- `get_quick_command(pool, id)` — single fetch
- `create_quick_command(pool, user_id, input)` — create with user_id
- `update_quick_command(pool, id, input)` — partial update via QueryBuilder
- `delete_quick_command(pool, id)` — delete

Use `anyhow::Context` and `sqlx::QueryBuilder` consistently with existing patterns.

- [ ] **Step 7: Register query module**

Add to `database/src/queries/mod.rs`:
```rust
pub mod quick_commands;
```

- [ ] **Step 8: Update terminal_profiles queries for user scoping**

In `database/src/queries/terminal_profiles.rs`, update `list_terminal_profiles` and `create_terminal_profile` to accept and filter by `user_id`.

- [ ] **Step 9: Verify everything compiles and tests pass**

```bash
cargo clippy --workspace -- -D warnings && cargo test --workspace
```

- [ ] **Step 10: Commit**

```bash
git add database/
git commit -m "feat: database cleanup and quick commands table

- Drop 20+ legacy tables from old job/notification/plugin system
- Add user_id to terminal_profiles for multi-user scoping
- Normalize layout values to frontend convention
- Create quick_commands table with user/server scoping
- Add QuickCommand model and CRUD queries
- Extend PaneConfig with mode field"
```

---

## Task 4: Backend API Endpoints for Profiles and Quick Commands

**Files:**
- Create: `server/src/routes/api/profiles.rs`
- Create: `server/src/routes/api/quick_commands.rs`
- Modify: `server/src/routes/api/mod.rs`
- Modify: `server/src/main.rs`

- [ ] **Step 1: Create profiles API router**

Create `server/src/routes/api/profiles.rs` following the pattern in `credentials.rs`:
- `GET /` — list profiles for authenticated user
- `GET /:id` — get single profile
- `POST /` — create profile (assigns user_id from session)
- `PUT /:id` — update profile
- `DELETE /:id` — delete profile

Extract user_id from the session (check how `auth.rs` middleware works).

- [ ] **Step 2: Create quick commands API router**

Create `server/src/routes/api/quick_commands.rs`:
- `GET /` — list all quick commands for user (optional `?server_id=` filter)
- `GET /:id` — get single command
- `POST /` — create command
- `PUT /:id` — update command
- `DELETE /:id` — delete command

- [ ] **Step 3: Register routes**

In `server/src/routes/api/mod.rs`, add:
```rust
pub mod profiles;
pub mod quick_commands;
```

In `server/src/main.rs`, register under `/api/v1/profiles` and `/api/v1/quick-commands`.

- [ ] **Step 4: Verify compilation and test**

```bash
cargo clippy --workspace -- -D warnings && cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add server/
git commit -m "feat: REST API endpoints for profiles and quick commands

- GET/POST/PUT/DELETE /api/v1/profiles (user-scoped)
- GET/POST/PUT/DELETE /api/v1/quick-commands (user-scoped, server-filterable)"
```

---

## Task 5: Frontend API Clients and State Modules

**Files:**
- Create: `ui/src/lib/types/profile.ts`
- Create: `ui/src/lib/api/profiles.ts`
- Create: `ui/src/lib/api/quickCommands.ts`
- Create: `ui/src/lib/state/profiles.svelte.ts`
- Create: `ui/src/lib/state/quickCommands.svelte.ts`

- [ ] **Step 1: Create TypeScript types**

Create `ui/src/lib/types/profile.ts`:

```typescript
export interface TerminalProfile {
    id: number;
    name: string;
    description: string | null;
    layout: 'single' | 'split-h' | 'split-v' | 'quad';
    pane_configs: PaneConfig[] | null;
    is_default: boolean;
    created_at: string;
    updated_at: string;
}

export interface PaneConfig {
    server_id: number | null;
    mode: 'pty' | 'cmd' | null;
}

export interface CreateProfile {
    name: string;
    description?: string;
    layout: string;
    pane_configs?: PaneConfig[];
    is_default?: boolean;
}

export interface QuickCommand {
    id: number;
    name: string;
    command: string;
    server_id: number | null;
    category: string;
    sort_order: number;
    created_at: string;
    updated_at: string;
}

export interface CreateQuickCommand {
    name: string;
    command: string;
    server_id?: number | null;
    category?: string;
}
```

Export from `ui/src/lib/types/index.ts`.

- [ ] **Step 2: Create API client modules**

Create `ui/src/lib/api/profiles.ts` and `ui/src/lib/api/quickCommands.ts` following the pattern in `servers.ts` — thin wrappers around `get/post/put/del` from `client.ts`.

- [ ] **Step 3: Create state modules**

Create `ui/src/lib/state/profiles.svelte.ts` and `ui/src/lib/state/quickCommands.svelte.ts` following the pattern in `servers.svelte.ts` — `$state` variables with exported getters, loaders, and CRUD wrappers. Use `extractErrorMessage` for error handling.

- [ ] **Step 4: Verify typecheck and build**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/
git commit -m "feat: frontend types, API clients, and state for profiles and quick commands"
```

---

## Task 6: Session Serialize/Restore

**Files:**
- Modify: `ui/src/lib/components/terminal/TerminalPane.svelte`

- [ ] **Step 1: Import and load the serialize addon**

In `TerminalPane.svelte`, add:
```typescript
import { SerializeAddon } from '@xterm/addon-serialize';
```

Create and load the addon alongside the other addons:
```typescript
let serializeAddon: SerializeAddon | null = null;
// In initTerminal():
serializeAddon = new SerializeAddon();
terminal.loadAddon(serializeAddon);
```

- [ ] **Step 2: Add serialize on disconnect/visibility change**

Add a function to serialize the buffer to sessionStorage:

```typescript
function serializeBuffer() {
    if (!serializeAddon || !terminal) return;
    try {
        const data = serializeAddon.serialize();
        if (data.length > 512 * 1024) return; // Skip if too large
        sessionStorage.setItem(`svrctlrs-term-buffer-${tabId}`, data);
    } catch { /* sessionStorage full or unavailable */ }
}
```

Call `serializeBuffer()` in:
- `disconnect()` method
- A `visibilitychange` event listener (when `document.hidden` becomes true)

- [ ] **Step 3: Add restore on connect**

Before connecting, check for a saved buffer:

```typescript
function restoreBuffer() {
    const key = `svrctlrs-term-buffer-${tabId}`;
    const data = sessionStorage.getItem(key);
    if (data && terminal) {
        terminal.write(data);
        sessionStorage.removeItem(key);
    }
}
```

Call `restoreBuffer()` at the start of `connect()` before opening the WebSocket.

- [ ] **Step 4: Clean up buffer on explicit tab close**

When the user closes a tab, remove the stored buffer:
```typescript
sessionStorage.removeItem(`svrctlrs-term-buffer-${tabId}`);
```

- [ ] **Step 5: Verify typecheck and build**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/terminal/TerminalPane.svelte
git commit -m "feat: terminal session serialize/restore via @xterm/addon-serialize

- Serialize scrollback buffer to sessionStorage on disconnect/tab hide
- Restore buffer on reconnect (display-only, shell session not resumed)
- 512KB size limit per tab
- Buffer cleaned up on explicit tab close"
```

---

## Task 7: Command Palette Component

**Files:**
- Create: `ui/src/lib/components/terminal/CommandPalette.svelte`
- Modify: `ui/src/routes/+page.svelte`
- Modify: `ui/src/lib/components/terminal/TerminalPane.svelte`

- [ ] **Step 1: Create the CommandPalette component**

Create `ui/src/lib/components/terminal/CommandPalette.svelte`:

Props: `open: boolean`, `onClose: () => void`, `onSelectCommand: (cmd: string) => void`, `serverId: number | null`

Behavior:
- Desktop: centered overlay with search input, fuzzy-filtered list of commands/actions
- Mobile (`isMobile()`): bottom sheet (reuse the pattern from TerminalPrefsPanel)
- Categories: Quick Commands, Recent Commands, Actions
- Keyboard navigable: arrow keys to move, Enter to select, Escape to close
- Fuzzy search: filter `name` and `command` fields as user types

Structure:
```svelte
{#if open}
  <!-- backdrop -->
  <button class="fixed inset-0 bg-black/40 z-40" onclick={onClose} />

  <!-- palette container: centered on desktop, bottom sheet on mobile -->
  <div class="fixed z-50 {isMobile() ? 'bottom-0 inset-x-0' : 'top-1/4 left-1/2 -translate-x-1/2'} ...">
    <input bind:value={query} placeholder="Type a command..." autofocus />
    <div class="overflow-y-auto max-h-[50vh]">
      {#each filtered as item}
        <button onclick={() => select(item)} class="...">
          <span>{item.name}</span>
          <span class="text-text-muted">{item.command}</span>
        </button>
      {/each}
    </div>
  </div>
{/if}
```

- [ ] **Step 2: Add Ctrl+K shortcut to +page.svelte**

In the existing keyboard handler in `+page.svelte`, add:
```typescript
if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    paletteOpen = !paletteOpen;
}
```

Add the CommandPalette component to the template:
```svelte
<CommandPalette
    open={paletteOpen}
    onClose={() => paletteOpen = false}
    onSelectCommand={handlePaletteCommand}
    serverId={activeTab?.serverId ?? null}
/>
```

- [ ] **Step 3: Add double-tap gesture to TerminalPane**

In `TerminalPane.svelte`, add a double-tap detector on the terminal container div:

```typescript
let lastTapTime = 0;
function handleTouchEnd(e: TouchEvent) {
    const now = Date.now();
    if (now - lastTapTime < 300) {
        e.preventDefault();
        dispatch('open-palette');
    }
    lastTapTime = now;
}
```

Wire up in the template container div and dispatch an event that `+page.svelte` listens for.

- [ ] **Step 4: Handle palette command selection**

In `+page.svelte`, `handlePaletteCommand(cmd)`:
- If active tab is PTY and connected: write the command to the PTY input
- If action (New Tab, Close Tab, etc.): execute the action directly
- Close the palette after selection

- [ ] **Step 5: Populate palette with quick commands + actions**

Load quick commands from state on palette open. Merge with hardcoded actions:
```typescript
const actions = [
    { name: 'New Tab', command: '', action: () => handleNewTab() },
    { name: 'Close Tab', command: '', action: () => { if (activeTabId) terminalState.closeTab(activeTabId) } },
    { name: 'Toggle Theme', command: '', action: () => themeState.toggleTheme() },
    // etc.
];
```

- [ ] **Step 6: Verify typecheck and build**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 7: Commit**

```bash
git add ui/src/lib/components/terminal/CommandPalette.svelte ui/src/routes/+page.svelte ui/src/lib/components/terminal/TerminalPane.svelte
git commit -m "feat: command palette with Ctrl+K and double-tap invocation

- Searchable overlay (desktop) / bottom sheet (mobile)
- Quick commands, recent history, and app actions
- Keyboard navigable with fuzzy search
- Double-tap gesture on mobile terminal area
- Injects selected command into active PTY session"
```

---

## Task 8: Terminal Profiles UI

**Files:**
- Create: `ui/src/lib/components/terminal/ProfileManager.svelte`
- Modify: `ui/src/lib/components/layout/Sidebar.svelte`
- Modify: `ui/src/lib/state/terminal.svelte.ts`

- [ ] **Step 1: Create ProfileManager component**

Create `ui/src/lib/components/terminal/ProfileManager.svelte`:

Two modes:
1. **Save dialog** (Modal): name input, description, checkbox for is_default. Captures current layout + tab server assignments as pane_configs.
2. **Load action**: When a profile is clicked in the sidebar, create tabs matching the profile's pane_configs and trigger auto-connect.

- [ ] **Step 2: Add profiles section to Sidebar**

In `Sidebar.svelte`, add a "Profiles" section below the server list:

```svelte
<!-- Profiles -->
{#if profiles.length > 0}
  <div class="px-2 pt-2">
    <span class="text-xs text-sidebar-muted uppercase">Profiles</span>
  </div>
  {#each profiles as profile}
    <button onclick={() => loadProfile(profile)} class="...">
      <Layout class="w-4 h-4" />
      {#if mobileOpen || !collapsed}
        <span>{profile.name}</span>
      {/if}
    </button>
  {/each}
{/if}
```

- [ ] **Step 3: Add save/load functions to terminal state**

In `terminal.svelte.ts`, add:
- `captureProfileState()` — returns current layout + per-tab server/mode as PaneConfig[]
- `applyProfile(profile)` — closes all tabs, creates new tabs matching profile pane_configs, triggers auto-connect

- [ ] **Step 4: Wire up save action in hamburger menu / command palette**

Add "Save Layout as Profile" to the command palette actions and/or hamburger menu.

- [ ] **Step 5: Verify typecheck and build**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/ ui/src/lib/state/ ui/src/routes/
git commit -m "feat: terminal profiles UI — save, load, and manage layouts

- Save current layout + server assignments as named profile
- Load profiles from sidebar (creates tabs, auto-connects)
- Profiles scoped to authenticated user
- Save accessible from command palette and hamburger menu"
```

---

## Task 9: Design System Tokens

**Files:**
- Modify: `ui/src/app.css`

- [ ] **Step 1: Add density and motion tokens to @theme block**

In `ui/src/app.css`, extend the `@theme` block:

```css
@theme {
    /* ...existing color tokens... */

    /* Density */
    --density: 1;

    /* Motion */
    --duration-fast: 100ms;
    --duration-normal: 200ms;
    --duration-slow: 300ms;
    --easing-default: cubic-bezier(0.4, 0, 0.2, 1);
}
```

Add a compact density class:
```css
[data-density="compact"] {
    --density: 0.75;
}
```

- [ ] **Step 2: Apply density to key components**

Update Button, Input, Select padding to use density-scaled values where it makes sense. This can be done incrementally — start with the Button component and expand later.

- [ ] **Step 3: Set compact density on mobile**

In `+layout.svelte` or `app.html`, set `data-density="compact"` when `isMobile()` is true.

- [ ] **Step 4: Verify typecheck and build**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/app.css ui/src/lib/components/ ui/src/routes/
git commit -m "feat: design system tokens — density mode and motion tokens

- Add --density, --duration-*, --easing-default CSS tokens
- Compact density (0.75x padding) auto-applied on mobile
- Foundation for consistent spacing across all platforms"
```

---

## Task 10: Integration Testing and Final Verification

- [ ] **Step 1: Run full backend quality checks**

```bash
cargo clippy --workspace -- -D warnings && cargo test --workspace
```

- [ ] **Step 2: Run full frontend quality checks**

```bash
cd ui && npm run check && npm run build
```

- [ ] **Step 3: Start the server and test manually**

```bash
RUST_LOG=info DATABASE_URL=sqlite:data/svrctlrs.db SPA_DIR=ui/build SESSION_SECURE=false ADMIN_USERNAME=admin ADMIN_PASSWORD=admin cargo run -p server --bin server -- --addr 0.0.0.0:8081
```

Verify:
- Login works
- Terminal connects via PTY
- Command palette opens with Ctrl+K
- Quick commands can be created/executed
- Profiles can be saved and loaded
- Terminal buffer restores after page refresh
- All CRUD pages (servers, credentials, settings) still work

- [ ] **Step 4: Run /simplify skill for code quality review**

Use the simplify skill to review all changed files for reuse opportunities, consistency, and efficiency.

- [ ] **Step 5: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore: Phase 1 integration testing and cleanup"
```
