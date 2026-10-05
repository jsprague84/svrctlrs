# Archived documentation

**Not maintained. Historical reference only.** Nothing in this directory describes the
current product; current docs are in `/README.md`, `/CLAUDE.md` and `/docs/`.

SvrCtlRS has had three architectures:

| Era | Dates | What it was | Archived here |
|---|---|---|---|
| Plugin platform (1.x–2.1) | 2025-11 | fixed monitoring plugins (Docker, updates, health, weather, speedtest), cron scheduler, Dioxus then HTMX UI | `root-level/`, `old-docs/` (archived 2025-11-28) |
| Job platform (2.2) | 2025-11 → 2025-12 | user-defined job types/templates/schedules, HTMX + Askama UI, Gotify/ntfy | `job-era-2025-12/` (archived 2026-10-04) |
| Terminal tool (3.x) | 2026-03 → | SvelteKit SPA + Tauri clients, PTY terminals over WebSocket | — current; its design history is in `/docs/history/` |

The job platform's database schema was dropped by migration `019`; the plugin platform's by
`011`. Both sets of docs reference tables, routes, templates and crates that no longer exist.
