Plan to implement                                                                                   │
│                                                                                                     │
│ SvrCtlRS -> Svelte 5 Terminal App Rewrite                                                           │
│                                                                                                     │
│ Context                                                                                             │
│                                                                                                     │
│ SvrCtlRS is being simplified from a job-based infrastructure platform to a focused terminal         │
│ application. The entire HTMX/Alpine.js/Askama frontend is replaced with a Svelte 5 SPA. The backend │
│  is stripped of job scheduling, notifications, and workflow features — keeping only server          │
│ management, credentials, SSH, and WebSocket terminal handlers.                                      │
│                                                                                                     │
│ No backend WebSocket protocol changes — the CMD (/ws/terminal) and PTY (/ws/terminal/pty) handlers  │
│ stay as-is.                                                                                         │
│                                                                                                     │
│ Final Project Structure                                                                             │
│                                                                                                     │
│ svrctlrs/                                                                                           │
│ ├── ui/                          # NEW - Svelte 5 SPA                                               │
│ │   ├── package.json             # svelte 5, sveltekit, tailwind 4, xterm, lucide-svelte            │
│ │   ├── svelte.config.js         # adapter-static, SPA fallback                                     │
│ │   ├── vite.config.ts           # tailwind plugin, dev proxy for /api + /ws                        │
│ │   ├── src/                                                                                        │
│ │   │   ├── app.html / app.css / app.d.ts                                                           │
│ │   │   ├── lib/                                                                                    │
│ │   │   │   ├── api/             # client.ts, servers.ts, credentials.ts, settings.ts               │
│ │   │   │   ├── types/           # server.ts, credential.ts, setting.ts, terminal.ts                │
│ │   │   │   ├── state/           # *.svelte.ts files with $state runes                              │
│ │   │   │   └── components/                                                                         │
│ │   │   │       ├── ui/          # Button, Modal, Badge, Toast, Input, Select                       │
│ │   │   │       ├── layout/      # Sidebar, Header                                                  │
│ │   │   │       └── terminal/    # TerminalPane, TerminalTabs, SplitView, CommandInput              │
│ │   │   └── routes/                                                                                 │
│ │   │       ├── +layout.ts       # export const ssr = false                                         │
│ │   │       ├── +layout.svelte   # App shell (sidebar + header + slot)                              │
│ │   │       ├── +page.svelte     # Terminal (home/default route)                                    │
│ │   │       ├── servers/+page.svelte                                                                │
│ │   │       ├── credentials/+page.svelte                                                            │
│ │   │       └── settings/+page.svelte                                                               │
│ │   └── build/                   # Output served by Axum                                            │
│ ├── core/                        # STRIPPED (remove executor, notifications)                        │
│ ├── server/                      # STRIPPED (remove Askama/HTMX, serve SPA)                         │
│ ├── database/                    # STRIPPED (remove job/notification models+queries)                │
│ └── (scheduler/ DELETED)                                                                            │
│                                                                                                     │
│ Tech Stack (matching rustRoast)                                                                     │
│                                                                                                     │
│ ┌───────────┬──────────────────────────────────┬──────────┐                                         │
│ │   Layer   │            Technology            │ Version  │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ Framework │ SvelteKit (SPA mode)             │ ^2.50.2  │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ UI        │ Svelte 5 (runes only, NO stores) │ ^5.51.0  │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ Build     │ Vite                             │ ^7.3.1   │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ CSS       │ Tailwind CSS 4                   │ ^4.2.1   │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ Terminal  │ @xterm/xterm + addons            │ ^5.5.0   │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ Icons     │ lucide-svelte                    │ ^0.575.0 │                                         │
│ ├───────────┼──────────────────────────────────┼──────────┤                                         │
│ │ Types     │ TypeScript                       │ ^5.9.3   │                                         │
│ └───────────┴──────────────────────────────────┴──────────┘                                         │
│                                                                                                     │
│ Implementation Phases                                                                               │
│                                                                                                     │
│ Phase 1: SvelteKit Scaffolding + Coexistence                                                        │
│                                                                                                     │
│ Goal: SvelteKit app builds and is served at /app/ alongside existing HTMX UI.                       │
│                                                                                                     │
│ Create:                                                                                             │
│ - ui/package.json — deps matching rustRoast pattern                                                 │
│ - ui/svelte.config.js — adapter-static({ fallback: 'index.html' })                                  │
│ - ui/vite.config.ts — tailwindcss + sveltekit plugins, proxy /api+/ws → localhost:8081              │
│ - ui/tsconfig.json                                                                                  │
│ - ui/src/app.html, app.css (Tailwind 4 + theme CSS vars), app.d.ts                                  │
│ - ui/src/routes/+layout.ts — export const ssr = false                                               │
│ - ui/src/routes/+layout.svelte — placeholder shell                                                  │
│ - ui/src/routes/+page.svelte — placeholder page                                                     │
│                                                                                                     │
│ Modify server/src/routes/ui/mod.rs:                                                                 │
│ - Add ServeDir + ServeFile fallback at /app → ui/build/                                             │
│                                                                                                     │
│ Verify: cd ui && npm install && npm run build, navigate to localhost:8081/app/.                     │
│                                                                                                     │
│ Phase 2: Types + API Client + Shared State                                                          │
│                                                                                                     │
│ Goal: Type-safe data layer and reactive state with Svelte 5 runes.                                  │
│                                                                                                     │
│ Types (ui/src/lib/types/):                                                                          │
│ - server.ts — Server, CreateServer, UpdateServer (mirrors Rust models)                              │
│ - credential.ts — Credential, CredentialType                                                        │
│ - setting.ts — Setting                                                                              │
│ - terminal.ts — CMD/PTY WebSocket request/response message types                                    │
│                                                                                                     │
│ API (ui/src/lib/api/):                                                                              │
│ - client.ts — fetch wrapper (same-origin cookies, 401→redirect, error handling)                     │
│ - servers.ts — GET/POST/PUT/DELETE /api/v1/servers, POST /api/v1/servers/{id}/test                  │
│ - credentials.ts — GET/POST/PUT/DELETE /api/v1/credentials                                          │
│ - settings.ts — GET/PUT /api/v1/settings                                                            │
│                                                                                                     │
│ State (ui/src/lib/state/*.svelte.ts — all use $state runes, NO Svelte stores):                      │
│ - servers.svelte.ts — $state<Server[]>, loadServers(), selectServer(), CRUD                         │
│ - credentials.svelte.ts — $state<Credential[]>, CRUD                                                │
│ - settings.svelte.ts — $state<Setting[]>, terminal preferences                                      │
│ - theme.svelte.ts — Tokyo Night theme constants (single theme, no switching needed)                 │
│ - terminal.svelte.ts — $state<TerminalTab[]>, tab/pane/layout management                            │
│ - toast.svelte.ts — $state<Toast[]>, notification toasts                                            │
│                                                                                                     │
│ Theme CSS in app.css — port Tokyo Night only from existing styles-technical.css as CSS custom       │
│ properties consumed by Tailwind. No multi-theme support (Nord Dark and Light removed).              │
│                                                                                                     │
│ Verify: npm run check passes.                                                                       │
│                                                                                                     │
│ Phase 3: Core Terminal Component                                                                    │
│                                                                                                     │
│ Goal: Working single-terminal TerminalPane.svelte with CMD + PTY WebSocket modes.                   │
│                                                                                                     │
│ Create:                                                                                             │
│ - ui/src/lib/components/terminal/TerminalPane.svelte — core xterm.js wrapper                        │
│ - ui/src/lib/components/terminal/terminal-theme.ts — xterm.js Tokyo Night theme colors              │
│                                                                                                     │
│ TerminalPane.svelte — the critical component:                                                       │
│ - Creates Terminal + addons: FitAddon, SearchAddon, WebLinksAddon, WebglAddon (canvas fallback),    │
│ Unicode11Addon, ClipboardAddon, ImageAddon                                                          │
│ - WebSocket to /ws/terminal (CMD) or /ws/terminal/pty (PTY)                                         │
│ - $effect for init/cleanup lifecycle, ResizeObserver for auto-fit                                   │
│ - Tokyo Night theme applied on init                                                                 │
│ - Props: serverId, mode, event callbacks                                                            │
│ - Methods: connect(), disconnect(), executeCommand(), clear(), search(), copyOutput(),              │
│ downloadOutput(), focus(), fit()                                                                    │
│ - Command history (localStorage per server), keep-alive ping 30s, reconnect backoff                 │
│                                                                                                     │
│ WebSocket protocols (match existing backend exactly — no backend changes):                          │
│ - CMD: {type:"execute", server_id, command, cols, rows} → {type:"output"|"exit"|"error", data,      │
│ exit_code}                                                                                          │
│ - PTY: {type:"shell", server_id, cols, rows} + {type:"input", data} →                               │
│ {type:"output"|"connected"|"error", data}                                                           │
│                                                                                                     │
│ Verify: Render single TerminalPane on home page, connect to server, execute command in both modes.  │
│                                                                                                     │
│ Phase 4: Multi-Tab + Split-Pane Layout                                                              │
│                                                                                                     │
│ Goal: Tab bar, multiple terminals, CSS Grid split layouts.                                          │
│                                                                                                     │
│ Create:                                                                                             │
│ - ui/src/lib/components/terminal/TerminalTabs.svelte — tab bar (indicator dots, mode badges, close, │
│  layout buttons, + button)                                                                          │
│ - ui/src/lib/components/terminal/SplitView.svelte — CSS Grid managing 1-4 visible panes             │
│ - ui/src/lib/components/terminal/CommandInput.svelte — CMD mode input with history                  │
│ - ui/src/lib/components/terminal/ConnectionBadge.svelte                                             │
│                                                                                                     │
│ Layouts (CSS Grid):                                                                                 │
│ - single: 1fr / 1fr, split-h: 1fr / 1fr 1fr, split-v: 1fr 1fr / 1fr, quad: 1fr 1fr / 1fr 1fr        │
│                                                                                                     │
│ State (in terminal.svelte.ts): tabs[], activeTabId, layout, visibleSlots[], max 10 tabs             │
│                                                                                                     │
│ Keyboard shortcuts (capture phase in +page.svelte):                                                 │
│ - Ctrl+Shift+T — new tab, Ctrl+Shift+W — close tab                                                  │
│ - Ctrl+Tab / Ctrl+Shift+Tab — cycle tabs                                                            │
│ - Ctrl+\ — toggle split, Ctrl+Shift+F — search                                                      │
│                                                                                                     │
│ Verify: Create 3 tabs, connect to different servers, switch layouts, verify independent sessions.   │
│                                                                                                     │
│ Phase 5: App Layout + Server Sidebar                                                                │
│                                                                                                     │
│ Goal: Full app shell with collapsible sidebar, server list, navigation.                             │
│                                                                                                     │
│ Create/Update:                                                                                      │
│ - ui/src/lib/components/layout/Sidebar.svelte — collapsible, server cards with Connect button, nav  │
│ links                                                                                               │
│ - ui/src/lib/components/layout/Header.svelte — connection info, mode toggle, actions                │
│ - ui/src/lib/components/terminal/ServerSelector.svelte — server cards with status indicators        │
│ - ui/src/routes/+layout.svelte — full layout: sidebar + header + slot                               │
│                                                                                                     │
│ Verify: Full terminal workflow end-to-end.                                                          │
│                                                                                                     │
│ Phase 6: CRUD Pages                                                                                 │
│                                                                                                     │
│ Goal: Server, credential, and settings management.                                                  │
│                                                                                                     │
│ Create:                                                                                             │
│ - ui/src/lib/components/ui/ — Button, Input, Select, Modal, Badge, Toast                            │
│ - ui/src/routes/servers/+page.svelte — list, add/edit modal, test connection, delete                │
│ - ui/src/routes/credentials/+page.svelte — list, add/edit (SSH Key, Password, API Token)            │
│ - ui/src/routes/settings/+page.svelte — terminal settings (font, cursor, scrollback)                │
│                                                                                                     │
│ Phase 7: Backend Cleanup                                                                            │
│                                                                                                     │
│ Goal: Strip all job/notification/scheduler code, switch to serving Svelte SPA.                      │
│                                                                                                     │
│ Delete entirely:                                                                                    │
│ - scheduler/ directory                                                                              │
│ - server/templates/ directory                                                                       │
│ - server/static/ directory                                                                          │
│ - server/src/routes/job_runs_ws.rs                                                                  │
│ - server/src/routes/ui/ — all EXCEPT auth.rs                                                        │
│ - server/src/routes/api/ — catalog, job_types, job_templates, job_schedules, job_runs,              │
│ notifications, tags                                                                                 │
│ - server/src/templates.rs, server/src/filters.rs                                                    │
│ - database/src/models/ — job_type, job_template, job_schedule, job_run, notification, tag,          │
│ job_catalog                                                                                         │
│ - database/src/queries/ — same set                                                                  │
│ - database/src/notification_service.rs                                                              │
│ - core/src/executor.rs, core/src/notifications.rs                                                   │
│                                                                                                     │
│ Modify:                                                                                             │
│ - Cargo.toml — remove scheduler from workspace members                                              │
│ - server/Cargo.toml — remove askama, askama_web, svrctlrs-scheduler, axum-extra                     │
│ - server/src/main.rs — remove scheduler, serve SPA at / via ServeDir("ui/build") with index.html    │
│ fallback                                                                                            │
│ - server/src/state.rs — remove scheduler, job_run_tx, notification_service                          │
│ - server/src/routes.rs — remove job_runs_ws                                                         │
│ - server/src/routes/api/mod.rs — remove job/notification/tag/catalog routes, simplify status        │
│ endpoint                                                                                            │
│ - database/src/lib.rs, models/mod.rs, queries/mod.rs — remove unused modules                        │
│                                                                                                     │
│ Keep unchanged:                                                                                     │
│ - server/src/routes/terminal.rs + terminal_pty.rs (WebSocket handlers)                              │
│ - server/src/routes/api/servers.rs, credentials.rs, settings.rs                                     │
│ - server/src/ssh.rs                                                                                 │
│ - All database/migrations/ files (never delete migrations)                                          │
│                                                                                                     │
│ Move: Terminal profile routes from ui/debug.rs → api/terminal_profiles.rs                           │
│                                                                                                     │
│ Phase 8: Auth + Docker                                                                              │
│                                                                                                     │
│ Auth: Keep server-side login page (minimal standalone HTML). Svelte API client redirects to         │
│ /auth/login on 401. SPA assets exempted from auth middleware.                                       │
│                                                                                                     │
│ Dockerfile: Add Node.js build stage:                                                                │
│ FROM node:22-slim AS frontend                                                                       │
│ WORKDIR /app/ui                                                                                     │
│ COPY ui/package.json ui/package-lock.json ./                                                        │
│ RUN npm ci                                                                                          │
│ COPY ui/ ./                                                                                         │
│ RUN npm run build                                                                                   │
│ Then COPY --from=frontend /app/ui/build /app/ui/build in runtime stage.                             │
│                                                                                                     │
│ Env vars: Add SPA_DIR (default: ui/build), remove STATIC_DIR.                                       │
│                                                                                                     │
│ Verification                                                                                        │
│                                                                                                     │
│ After each phase: npm run check, npm run build, cargo build --workspace, cargo clippy --workspace   │
│ -- -D warnings                                                                                      │
│                                                                                                     │
│ Final: open app → 3 tabs → connect to different servers → switch layouts → manage                   │
│ servers/credentials → keyboard shortcuts all work.                                                  │
╰─────────────────────────────────────────────────────────────────────────────────────────────────────
