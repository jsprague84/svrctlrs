# SvrCtlRS Next Phase — Design Specification

**Date:** 2026-03-20
**Status:** Draft
**Author:** Johnathon Sprague + Claude
**Branch:** ralph/code-quality-audit (base for implementation)

---

## 1. Overview

SvrCtlRS is a terminal/SSH management tool for remote server access. The current application is a SvelteKit 5 SPA served by an Axum (Rust) backend with WebSocket-based terminal emulation via xterm.js.

This spec covers three areas of improvement:

1. **Terminal experience** — session persistence, profiles, command palette
2. **Mobile UX** — invisible-until-needed design for phone-sized screens
3. **Tauri v2 integration** — native desktop and mobile apps from the existing SvelteKit codebase

### Goals

- Make the in-browser terminal competitive with native terminals (Alacritty-class rendering and feel)
- Make mobile usable for quick server checks, light ops, and emergency response
- Ship desktop and mobile native apps without maintaining separate codebases
- Keep the UI clean and uncluttered — features are invisible until invoked

### Non-Goals

- Replacing the existing Rust backend server architecture
- Building a job scheduling or automation system (stripped in favor of terminal-first design)
- Supporting offline SSH (all connections route through the SvrCtlRS server, except the future local SSH stretch goal)

---

## 2. Terminal Improvements

### 2.1 Session Serialize/Restore

**What:** Add `@xterm/addon-serialize` to preserve terminal scrollback across disconnects, page reloads, and mobile browser suspension.

**Behavior:**
- On disconnect or `visibilitychange` (tab hidden): serialize terminal buffer to sessionStorage keyed by tab ID
- On reconnect or tab restore: deserialize buffer back into the terminal before new output appears
- Buffer is cleared when the user explicitly closes a tab
- sessionStorage (not localStorage) — survives refresh but not browser close, which is the right semantic

**Size limit handling:** Before serialization, if the terminal's scrollback buffer exceeds a threshold, reduce the terminal's `scrollback` option temporarily to truncate oldest lines, serialize, then restore the original scrollback value. This ensures the serialized output is valid (no mid-escape-sequence corruption). Target: 512KB max per tab.

**PTY session loss:** Serialized buffer is **display-only** — it preserves what the user can see (scrollback history) but does not restore the shell session. When a PTY session drops (e.g., phone screen off, OS kills WebSocket), the app shows the restored scrollback as read-only with a reconnect prompt overlay: "Session disconnected. [Reconnect] [New Session]". Reconnecting starts a fresh shell; the old output remains visible above.

**Implementation:**
- Install `@xterm/addon-serialize` (not currently in package.json)
- Load addon in `TerminalPane.svelte`
- Serialize on: `disconnect()`, `document.visibilitychange` (hidden), `beforeunload`
- Restore on: `connect()` before first output, tab reactivation
- Storage key: `svrctlrs-term-buffer-${tabId}`

### 2.2 Terminal Profiles UI

**What:** Build the UI for saving and loading terminal layouts. The database table `terminal_profiles` exists (migration 014) with `name`, `description`, `layout`, `pane_configs`, `quick_commands`, `is_default` fields.

**Schema alignment needed:** The existing database uses layout values `'1'`, `'2h'`, `'2v'`, `'4'` while the frontend TypeScript uses `'single' | 'split-h' | 'split-v' | 'quad'`. The canonical values will be the **frontend convention** (`single`, `split-h`, `split-v`, `quad`). The backend model will be updated to store these values directly. A migration will update any existing rows.

**PaneConfig extension needed:** The existing `PaneConfig` struct in `database/src/models/terminal_profile.rs` only has `server_id: Option<i64>`. A `mode` field (`"pty" | "cmd"`) must be added to support per-pane mode persistence.

**Data shape (pane_configs JSON):**
```json
{
  "layout": "split-h",
  "panes": [
    { "server_id": 1, "mode": "pty" },
    { "server_id": 3, "mode": "pty" }
  ]
}
```

Note: JSON uses snake_case (`server_id`) to match Rust serde defaults.

**User scoping:** All profiles and quick commands are scoped to the authenticated user via `user_id` foreign key. The `terminal_profiles` table needs a `user_id` column added via migration.

**Behavior:**
- **Save profile:** Hamburger menu > "Save Layout as Profile" — captures current tab count, server assignments, layout mode, per-pane server+mode
- **Load profile:** Sidebar shows saved profiles below the server list. Click a profile to open all tabs with servers pre-connected
- **Manage profiles:** Settings page or dedicated profiles section with rename, delete, set-as-default
- **Default profile:** Auto-loads on app start if set (replaces the empty single-tab default)

**Implementation:**
- Database migration: add `user_id` to `terminal_profiles`, update layout enum values
- Extend `PaneConfig` struct with `mode: Option<String>`
- API endpoints: CRUD on `terminal_profiles` (scoped to authenticated user)
- New UI: profile list in sidebar, save dialog (modal), manage page
- Frontend state: `profiles.svelte.ts` module following existing pattern
- Profile loading: create tabs programmatically via `terminalState`, trigger auto-connect

### 2.3 Command Palette / Quick Commands

**What:** A searchable overlay palette for all actions and saved command snippets. Single entry point for everything beyond typing in the terminal.

**Invocation:**
- Desktop: `Ctrl+K` (or `Cmd+K` on macOS)
- Mobile: double-tap the terminal area
- Always available: hamburger menu > "Command Palette"

**Palette contents (in search order):**
1. **Quick commands** — user-saved snippets (per-server or global)
2. **Recent commands** — pulled from command history
3. **Actions** — New Tab, Close Tab, Disconnect, Switch Server, Toggle Theme, Open Settings, Save Profile, Load Profile
4. **Run Snippet** (CMD mode) — execute a one-off command without a PTY session

**Quick commands storage:** A new `quick_commands` database table with columns: `id`, `user_id`, `name`, `command`, `server_id` (nullable for global), `category`, `sort_order`, `created_at`, `updated_at`. This replaces the existing `quick_commands` TEXT field on `terminal_profiles` (which stored a flat `Vec<String>` — too limited for the rich command model needed here). The profile field will be dropped in the migration.

**Quick command features:**
- Name, command string, optional server scope (global or specific server)
- Variable substitution: `{{server}}`, `{{date}}`, `{{user}}`
- Categories/tags for organization
- In PTY mode, selected command is injected into the active shell session (written to PTY input)
- In CMD mode fallback, command is executed via the non-interactive WebSocket

**Double-tap gesture handling:**
- `touch-action: manipulation` is already set on body (eliminates double-tap zoom)
- Double-tap detection: two taps within 300ms, both within 20px radius
- Must not conflict with xterm.js touch scrolling or text selection
- xterm.js touch events are handled on the canvas; the double-tap listener attaches to the container div wrapping the canvas
- If a single tap starts text selection (long press), the double-tap timer is cancelled

**UI design:**
- Desktop: centered overlay with search input at top (like VS Code's Ctrl+Shift+P)
- Mobile: bottom sheet (thumb-reachable) with search input at top
- Fuzzy search across names, commands, and action labels
- Keyboard navigable: arrow keys + Enter to select
- Dismisses on Escape, outside click, or selection

**Implementation:**
- New database table + migration for `quick_commands`
- New component: `CommandPalette.svelte` (responsive: overlay on desktop, bottom sheet on mobile)
- State: `quickCommands.svelte.ts` module
- Gesture detection: double-tap handler on the TerminalPane container div
- Desktop keyboard shortcut: add to existing keyboard handler in `+page.svelte`

---

## 3. Mobile UX Design

### 3.1 Design Principle: Invisible Until Needed

On mobile (< 768px), the terminal occupies 100% of available screen space. All controls are accessed via gestures, the command palette, or a minimal hamburger menu. No persistent toolbars, tab bars, or action buttons consume vertical space.

### 3.2 Screen Layout

**Keyboard hidden (portrait):**
```
┌──────────────────────────────┐
│ ● prod-web-01    ●●● ☰      │  ← 20px status bar
│                              │
│  admin@prod:~$ docker ps     │
│  CONTAINER  IMAGE    STATUS  │
│  a1b2c3d4   nginx    Up 3d  │
│  b2c3d4e5   postgres Up 3d  │
│  c3d4e5f6   node:20  Up 12h │
│                              │
│  admin@prod:~$ df -h         │
│  /dev/sda1   50G  32G  16G  │
│  /dev/sdb1  200G 185G  11G  │
│                              │
│  admin@prod:~$ █             │
│                              │
│  double-tap · swipe ← → tabs │  ← hint (fades after first use)
└──────────────────────────────┘
```

**Keyboard visible (portrait):**
```
┌──────────────────────────────┐
│ ● prod-web-01    ●●● ☰      │  ← status bar
│  /dev/sdb1  200G 185G  11G  │
│  admin@prod:~$ systemctl     │
│   restart nginx█             │  ← terminal (reduced, ~4 lines)
├──────────────────────────────┤
│ ESC TAB CTL ALT ↑ ↓ ← → |  │  ← extra keys row (~28px)
├──────────────────────────────┤
│       on-screen keyboard     │  ← ~45% of viewport
└──────────────────────────────┘
```

### 3.3 Status Bar

- **Height:** ~20px, single row
- **Left:** Connection dot (green/yellow/red) + server name (truncated with ellipsis)
- **Right:** Tab dot indicators + hamburger icon (☰)
- **Behavior:** Always visible, even with keyboard up

### 3.4 Tab Indicators

- **Dot-based** — small circles in the status bar, right of center
- **Only shown when 2+ tabs exist** — single tab = clean status bar
- **Active tab:** 6px dot in accent color (brand blue)
- **Other tabs:** 4px dots colored by connection status (green=connected, red=disconnected, muted=idle)
- **Overflow:** Beyond ~8 tabs, show count badge (e.g., `+3`)
- **Tap dots area:** Opens full tab list as a bottom sheet overlay
- **Swipe left/right on terminal:** Switches tabs, active dot moves

### 3.5 Extra Keys Row

- **Appears only when the on-screen keyboard is visible**
- **Tauri mobile:** Keyboard plugin fires `keyboardWillShow`/`keyboardWillHide` events with `keyboardHeight` — reliable and precise
- **Web fallback:** Monitor `window.visualViewport.resize` events. Heuristic: if viewport height shrinks by >150px from the last known "keyboard hidden" height, assume keyboard is visible. On iOS Safari, also listen for `focusin`/`focusout` on the xterm canvas as a secondary signal. This is imperfect but functional for the web-only case.
- **Position:** Between terminal output and keyboard
- **Keys:** ESC, TAB, CTL, ALT, ↑, ↓, ←, →, | (pipe)
- **CTL/ALT behavior:** Toggle modifier state — tap CTL, then tap 'c' = Ctrl+C. Visual highlight on the key indicates active modifier. Auto-clears after the next keypress.
- **Styling:** Matches terminal theme, minimal height (~28px)

### 3.6 Gestures

| Gesture | Action |
|---------|--------|
| Double-tap terminal | Open command palette (bottom sheet) |
| Swipe left/right | Switch tabs |
| Tap status bar dots | Open tab list overlay |
| Tap ☰ | Open full menu (servers, credentials, settings, profiles) |

### 3.7 PTY-First Design

- **PTY is the default mode** — mode selector hidden on mobile
- **No command input bar in PTY** — keyboard types directly into the shell
- **CMD mode accessible** via command palette > "Run Snippet"
- **CMD mode command input** appears as a temporary bottom sheet when invoked

---

## 4. Tauri v2 Architecture

### 4.1 Risk Assessment

Tauri v2 mobile support (iOS/Android) is stable but younger than the desktop story. Known risks:

- **WebView debugging on iOS** — less mature tooling than Android's Chrome DevTools
- **Plugin ecosystem** — fewer mobile-specific plugins than Capacitor or React Native; keyboard and biometric plugins need validation on target OS versions
- **App Store compliance** — WebView-based apps must meet minimum native-feel requirements

**Mitigation:** Phase 3 (Tauri Desktop) includes a validation spike: set up the Tauri project, build for one mobile platform, and verify the keyboard plugin and biometric plugin work before committing to the full Phase 4 mobile implementation.

### 4.2 Deployment Modes

| Mode | Shell | SSH Path | Native Features |
|------|-------|----------|-----------------|
| **Web** (current) | Browser | WebSocket → SvrCtlRS server → SSH | None |
| **Desktop** (new) | Tauri window | WebSocket → SvrCtlRS server → SSH | System tray, native shortcuts, window management |
| **Mobile** (new) | Tauri WebView | WebSocket → SvrCtlRS server → SSH | Keyboard management, biometrics, push notifications |

### 4.3 Project Structure

```
svrctlrs/
├── ui/                    # SvelteKit app (shared across all platforms)
│   └── src/
│       ├── lib/
│       │   ├── platform/  # NEW: platform detection + Tauri IPC wrappers
│       │   │   ├── index.ts         # isWeb(), isTauri(), isTauriMobile(), isTauriDesktop()
│       │   │   ├── keyboard.ts      # Keyboard show/hide events (Tauri or viewport heuristic)
│       │   │   ├── biometrics.ts    # Biometric/PIN auth (Tauri only, no-op on web)
│       │   │   └── notifications.ts # Push notifications (Tauri only, no-op on web)
│       │   └── ...existing code...
├── src-tauri/             # NEW: Tauri shell
│   ├── src/
│   │   ├── main.rs        # Tauri app entry
│   │   ├── commands/      # IPC commands (Rust functions callable from JS)
│   │   │   ├── keyboard.rs
│   │   │   ├── auth.rs    # Biometric bridge
│   │   │   └── ssh.rs     # Local SSH (stretch goal)
│   │   └── plugins/       # Tauri plugin configurations
│   ├── Cargo.toml         # Tauri dependencies
│   ├── tauri.conf.json    # Tauri configuration
│   ├── gen/               # Generated iOS/Android projects
│   │   ├── android/
│   │   └── apple/
│   └── icons/             # App icons for all platforms
├── server/                # Existing Axum backend (unchanged)
├── database/              # Existing SQLite layer (unchanged)
└── core/                  # Existing shared crate (unchanged)
```

### 4.4 Platform Detection Layer

```typescript
// ui/src/lib/platform/index.ts
export function isWeb(): boolean          // Running in plain browser
export function isTauri(): boolean        // Running inside Tauri (any platform)
export function isTauriMobile(): boolean  // Tauri on iOS or Android
export function isTauriDesktop(): boolean // Tauri on macOS/Windows/Linux
export function getPlatform(): 'web' | 'tauri-desktop' | 'tauri-mobile'
```

Detection method: check for `window.__TAURI_INTERNALS__` (Tauri v2 global). Platform discrimination via Tauri's `os` plugin or user agent parsing.

Used to conditionally enable:
- Extra keys row (Tauri mobile only — has keyboard API)
- Biometric settings page (Tauri only)
- Push notification settings (Tauri mobile only)
- System tray (Tauri desktop only)
- Local SSH option (Tauri only, stretch goal)

### 4.5 Keyboard Management (Mobile)

**Problem:** On-screen keyboard covers ~45% of portrait screen. The terminal must resize to fit the remaining space, and the extra keys row must appear/disappear in sync.

**Solution (Tauri):**
- Tauri's keyboard plugin fires `keyboardWillShow` with `keyboardHeight`
- SvelteKit receives this via IPC and updates terminal container height
- xterm `fit()` is called after container resize
- Extra keys row component visibility bound to keyboard state

**Solution (Web fallback):**
- Monitor `window.visualViewport.resize` events
- Heuristic: viewport height decrease >150px = keyboard visible
- On iOS Safari: also listen for `focusin`/`focusout` on terminal input elements as secondary signal
- Less reliable — false positives from address bar show/hide and orientation changes
- Mitigated by requiring the height delta threshold and debouncing (200ms)

### 4.6 Authentication (Mobile)

**Flow:**
1. App launches → check if biometric/PIN is enabled in settings
2. If enabled: OS biometric prompt (Face ID / fingerprint / device PIN fallback)
3. On success: retrieve stored session token from Tauri secure storage
4. If token expired: show server login form (username/password)
5. On login: store new session token in Tauri secure storage

**Layers:**
- **Device lock** (biometric/PIN) — Tauri-managed, unlocks the app locally
- **Server auth** (session cookie) — existing SvrCtlRS authentication, unchanged
- **Credential encryption** — existing AES-256-GCM at rest on the server, unchanged

The OS-level biometric prompt automatically offers device PIN as fallback when biometrics fail. No custom PIN implementation needed.

### 4.7 Push Notifications via Rstify

**Concept:** The SvrCtlRS server monitors for events and pushes alerts via the user's existing self-hosted notification platform, **rstify** (github.com/jsprague84/rstify).

**Events to notify on:**
- Server goes unreachable (SSH connection health check)
- Long-running command completes (CMD mode)
- Output matches a user-defined pattern (e.g., "ERROR", "OOM")

**Integration approach:** Rstify has a Rust backend and is compatible with Gotify, ntfy, and other notification formats. SvrCtlRS server POSTs notifications to rstify's API using the Gotify/ntfy-compatible format. Rstify handles FCM push delivery to mobile devices. No need to build a custom push relay — the infrastructure already exists.

**Prerequisites already available:**
- FCM account (shared across user's existing apps)
- Rstify instance (self-hosted, already running)
- Apple Developer account for APNs (if rstify supports iOS push)

**Implementation path:**
- Server-side: notification service that POSTs to rstify API on monitored events
- Configuration: rstify URL + API token stored in SvrCtlRS settings
- Tauri plugin: `tauri-plugin-notification` for local/foreground notifications
- Phase 5 implementation — not MVP but infrastructure prerequisites are already met

### 4.8 Local SSH (Phase 6)

**Concept:** Tauri's Rust backend runs `russh` directly on-device for SSH to machines on the same local network, bypassing the SvrCtlRS server entirely.

**Use case:** On the same WiFi as your servers — SSH directly without routing through the remote server.

**Implementation:** Tauri IPC command that accepts hostname/credentials and returns a channel to the frontend. The xterm terminal connects to this local channel instead of the remote server's WebSocket. Credentials can be stored locally in Tauri secure storage or synced from the SvrCtlRS server.

**Planned for Phase 6 — after core mobile app is stable.**

### 4.9 Remote Commands via Rstify Webhooks (Phase 6)

**Concept:** Receive secure remote commands through rstify's messaging/webhook system. Rstify pushes a command payload to the SvrCtlRS mobile app (or server), which executes it on the target server.

**Use cases:**
- Trigger a predefined quick command from another system (CI/CD, monitoring alert, chatbot)
- "Restart nginx on prod-web-01" sent as a webhook from rstify

**Security:** Commands must be pre-approved (only execute from the quick commands list, not arbitrary input). Authentication via webhook secret + command allowlist. Execution confirmation sent back via rstify notification.

**Planned for Phase 6 — requires both rstify integration and quick commands to be stable first.**

---

## 5. Design System Consolidation

### 5.1 Design Tokens

Extract theme values from inline `app.css` into a structured system:

```css
/* tokens.css — single source of truth */
@theme {
  /* Spacing density */
  --density-comfortable: 1;      /* desktop default */
  --density-compact: 0.75;       /* mobile */

  /* Motion */
  --duration-fast: 100ms;
  --duration-normal: 200ms;
  --duration-slow: 300ms;
  --easing-default: cubic-bezier(0.4, 0, 0.2, 1);

  /* Existing color tokens remain in app.css @theme block */
}
```

### 5.2 Component Density

A global density token that scales padding/gaps across all components:

- **Comfortable** (desktop) — current sizing
- **Compact** (mobile, or user preference) — 75% padding/gaps
- Applied via a CSS class on the root element (`data-density="compact"`)
- Components use `calc()` or responsive utilities to respect density

### 5.3 Platform Detection

The `ui/src/lib/platform/` module provides:
- Runtime platform detection (web vs Tauri vs Tauri mobile)
- Wrapper functions that no-op on unsupported platforms (biometrics on web returns "not available")
- Conditional feature rendering: `{#if isTauriMobile()}...{/if}`

---

## 6. CLAUDE.md Rewrite

The root `CLAUDE.md` is completely stale — it describes the old job-based automation system (HTMX, Alpine.js, Askama templates, job types, command templates, job schedules, etc.) that was removed months ago. This is the single biggest source of confusion for AI assistants and must be rewritten as the first task.

**The new CLAUDE.md must reflect:**
- **Purpose:** Terminal/SSH management tool for remote server access (not a job scheduler)
- **Architecture:** SvelteKit 5 SPA + Axum (Rust) backend + SQLite
- **Active branch:** `ralph/code-quality-audit` (branched from `ralph/mobile-responsive`). The `main` branch is stale and should not be used.
- **Frontend:** SvelteKit 5 with Svelte runes, Tailwind CSS v4, xterm.js 5.5, adapter-static
- **Backend:** Axum 0.8, SQLite via sqlx, russh (PTY), async-ssh2-tokio (CMD), tower-sessions
- **Key features:** Interactive PTY terminal, server/credential CRUD, terminal profiles, SSH host key verification (TOFU)
- **Build commands:** cargo build/clippy/test for backend, npm run check/build/dev for frontend
- **Design system:** Tokyo Night theme, CSS variables, shared UI components
- **Future direction:** Tauri v2 for desktop + mobile apps (reference the design spec)

**Remove all references to:** job types, command templates, job schedules, job runs, HTMX, Alpine.js, Askama templates, plugins, webhook endpoints, notification channels/policies, scheduler crate, composite workflows.

---

## 7. Database Cleanup

The database contains tables from the original job-based automation system (migrations 001-011) that are no longer used on this branch. These should be audited and cleaned up:

**Tables to evaluate for removal:**
- `job_types`, `command_templates`, `job_templates`, `job_template_steps` — job system (removed)
- `job_schedules`, `job_runs`, `server_job_results`, `step_execution_results` — job execution (removed)
- `notification_channels`, `notification_policies`, `notification_policy_channels`, `notification_log` — old notification system (replaced by rstify integration)
- `tags`, `server_tags` — server organization (evaluate if still useful)
- `server_capabilities` — auto-detection (evaluate if still useful)
- `webhooks`, `task_history`, `metrics` — legacy features

**Approach:** Create a cleanup migration that drops unused tables. Preserve `servers`, `credentials`, `users`, `settings`, `terminal_profiles`, `server_host_keys`, and any tables needed for new features. This reduces schema noise and makes the database easier to reason about.

**Timing:** Phase 1, before adding new tables for quick commands. Clean slate for the new schema additions.

---

## 8. Implementation Phases

### Phase 1: Foundation (Web)
- Rewrite CLAUDE.md to reflect current terminal-first architecture
- Database cleanup: drop unused tables from old job system
- Session serialize/restore addon
- Terminal profiles UI (save/load/manage) — includes schema migration for user_id, layout values, PaneConfig.mode
- Command palette component + quick commands table + CRUD
- Design system tokens + density mode

### Phase 2: Mobile UX Refinements (Web)
- Invisible-until-needed mobile layout (status bar, no tab bar)
- Tab dot indicators with connection status colors
- Double-tap gesture for command palette
- Swipe tab navigation
- PTY session disconnect/reconnect UX (serialized buffer + reconnect prompt)

### Phase 3: Tauri Desktop App + Mobile Validation Spike
- Tauri v2 project setup (`src-tauri/`)
- SvelteKit integration (adapter-static already configured, add tauri.conf.json)
- Platform detection layer (`ui/src/lib/platform/`)
- Desktop build verification (macOS, Linux, Windows)
- System tray integration
- **Mobile spike:** Set up one mobile target (Android or iOS), validate keyboard plugin and biometric plugin work on real device before committing to Phase 4

### Phase 4: Tauri Mobile App
- iOS and Android target setup (if spike successful)
- Keyboard management plugin + extra keys row
- Biometric authentication with device PIN fallback
- Mobile-specific gesture handling refinements
- App Store / Play Store builds

### Phase 5: Rstify Integration
- Push notifications via rstify API (Gotify/ntfy-compatible format)
- Server health monitoring with configurable alert rules
- Command completion notifications
- Notification settings UI in SvrCtlRS

### Phase 6: Advanced Features
- Local SSH on-device (russh in Tauri backend) — direct SSH without routing through server
- Remote commands via rstify webhooks — execute pre-approved quick commands remotely
- Command completion notifications in Tauri mobile app

---

## 9. Success Criteria

- Terminal renders at 60fps with WebGL acceleration
- Terminal scrollback survives page refresh and mobile browser suspension; reconnect prompt shown on PTY session loss
- Profiles save and restore multi-tab layouts with one click, scoped to authenticated user
- Command palette opens in <100ms on desktop, <200ms on mobile
- Mobile portrait mode shows terminal + extra keys + keyboard with zero wasted space
- Tab switching via swipe feels native (no jank, <16ms frame time)
- Tauri desktop app launches and connects to remote SvrCtlRS server
- Tauri mobile app passes biometric auth and opens terminal
- All three deployment modes (web, desktop, mobile) use the same SvelteKit build
- Quick commands injectable into active PTY session without mode switching
- Database contains only tables relevant to current functionality (no legacy job system clutter)
- Rstify integration delivers push notifications for server health events
