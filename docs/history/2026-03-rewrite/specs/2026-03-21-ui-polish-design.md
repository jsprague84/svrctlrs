# UI Polish & Settings Redesign — Design Specification

**Date:** 2026-03-21
**Status:** Draft
**Author:** Johnathon Sprague + Claude
**Branch:** ralph/phase5-rstify (base for implementation)

---

## 1. Overview

This spec addresses UI feature gaps and the settings page redesign identified during Phase 1-5 testing. The settings page is rebuilt from scratch as a tabbed interface with purpose-built sections, replacing the legacy generic key-value editor. Additional fixes close gaps in profile management, quick commands, and default profile auto-loading.

### Goals

- Replace the legacy settings page with a clean, tabbed settings UI
- Provide Quick Commands management (CRUD) inside the settings page
- Fix profile management gaps (edit, auto-load default)
- Purge all legacy settings from the old job system
- Maintain consistency with existing design system (Tokyo Night theme, shared components)
- Work well on both desktop and mobile

### Non-Goals

- Changing the terminal page layout (already complete)
- Adding new backend features (all APIs already exist)
- Modifying servers or credentials pages (already complete)

---

## 2. Settings Page Redesign

### 2.1 Tab Structure

The settings page uses a horizontal tab bar with 4 tabs. Each tab renders a purpose-built form section instead of raw key-value editing.

**Tabs:**

| Tab | Icon | Content |
|-----|------|---------|
| General | ⚙ (Settings icon) | Server URL (Tauri), theme, biometric, default profile |
| Notifications | 🔔 (Bell icon) | Rstify/Gotify/ntfy config, health checks, test button |
| Quick Commands | ⌨ (Terminal icon) | CRUD list of saved commands with categories |
| About | ℹ (Info icon) | App version, platform, keyboard shortcuts |

**Responsive behavior:**
- Desktop (>= 768px): full text labels on tabs
- Mobile (< 768px): icon-only tabs (tooltip or label below icon optional)

### 2.2 General Tab

**Server Connection (Tauri only):**
- Show current server URL in a read-only field
- "Change" button re-opens the ServerUrlSetup flow
- Connection status indicator (Connected / Disconnected)
- Entire section hidden when `isWeb()` (not in Tauri mode)

**Appearance:**
- Theme toggle: Dark / Light segmented buttons (replaces sidebar toggle — keep sidebar toggle too for quick access)

**Security (Tauri only):**
- Biometric unlock toggle
- Only shown when `isTauri()` and biometric is available

**Data:**
- Default terminal profile selector (dropdown of saved profiles + "None")
- When changed, immediately saves the setting
- Selected profile auto-loads on next app start

### 2.3 Notifications Tab

Purpose-built form replacing the generic key-value editor for notification.* settings.

**Fields:**
- Enable notifications toggle (boolean)
- Provider selector: Gotify (rstify compatible) / ntfy (dropdown)
- Server URL input (text, monospace)
- Application token input (password field, masked)
- Topic input (text, shown only when provider is ntfy)

**Health Checks section:**
- Enable health checks toggle (boolean)
- Check interval selector: 1 min / 5 min / 15 min / 30 min (dropdown)

**Actions:**
- Test Notification button — sends test via POST /api/v1/notifications/test
- Save button — writes all notification.* settings to the settings API

**Behavior:**
- On load: read notification.* settings from API and populate form
- On save: write all fields back as individual settings keys
- Test button: enabled only when URL and token are filled
- Toast feedback for save success/failure and test result

### 2.4 Quick Commands Tab

Full CRUD management for quick commands. Commands are used via the command palette (Ctrl+K) — this tab is for configuration only.

**List view:**
- Each command shows: name, command string (monospace), category badge, server scope badge (global or server name)
- Edit button (pencil icon) opens edit modal
- Delete button (X icon) with confirmation

**Add Command button** opens a modal with:
- Name (text input, required)
- Command (text input or textarea, required, monospace)
- Category (text input, default "general")
- Server scope (dropdown: "All Servers (global)" + list of servers, default global)

**Server data access:** `QuickCommandsSettings.svelte` imports `serversState` directly and calls `getServers()` to populate the server scope dropdown. This is consistent with how other components access shared state.

**Edit modal:** Same fields as create, pre-populated with existing values.

**Empty state:** "No quick commands yet. Add one to use it from the command palette (Ctrl+K)."

### 2.5 About Tab

Static information page:
- App name + version (from package.json or build info)
- Platform (Web / Desktop / Mobile via `getPlatform()`)
- Server URL (when in Tauri mode)
- Server health status (use `get('/health')` from `$lib/api/client.ts` which auto-prefixes `/api/v1`)

**Keyboard shortcuts reference:**
A two-column table of all keyboard shortcuts, grouped by category:

| Category | Shortcuts |
|----------|-----------|
| Tabs | Ctrl+Shift+T (new), Ctrl+Shift+W (close), Ctrl+Tab/Shift+Tab (switch) |
| Panes | Alt+1-4 (focus slot), Alt+[/] (cycle), Alt+Arrow (spatial) |
| Layout | Ctrl+\ (cycle layouts) |
| Actions | Ctrl+K (command palette), Ctrl+Shift+F (search) |

---

## 3. Profile Management Fixes

### 3.1 Default Profile Auto-Load

In `+page.svelte` (terminal page) onMount, use a `$effect` that reacts to profiles being loaded:

```typescript
$effect(() => {
    const profiles = profilesState.getProfiles();
    if (profiles.length > 0 && tabs.length === 0) {
        const defaultProfile = profiles.find(p => p.is_default);
        if (defaultProfile) {
            terminalState.applyProfile(defaultProfile.layout, defaultProfile.pane_configs ?? [], serverNames);
        }
    }
});
```

This avoids the race condition with async data loading — the `$effect` only fires once profiles are populated. It only applies on initial load (when `tabs.length === 0`).

**Note:** The existing `+layout.svelte` has dead code after a `return` statement in onMount (sidebar preference read). This must be moved above the `return` when modifying the file.

### 3.2 Profile Edit

Add metadata-only edit capability (rename, description, set as default). This is NOT re-capturing the terminal layout — just editing profile metadata.

- Add a small edit (pencil) icon on hover next to each profile in the sidebar (alongside the existing delete button)
- Edit opens a simple modal (NOT ProfileManager — different purpose) with: name input, description input, is_default toggle
- Save calls `profilesState.updateProfile(id, { name, description, is_default })`
- Create a new `ProfileEditModal.svelte` component for this (simpler than ProfileManager which handles layout capture)

---

## 4. Legacy Settings Cleanup

### 4.1 Database Migration

Create migration `021_purge_and_seed_settings.sql`:

```sql
-- Remove all legacy settings from old job system
DELETE FROM settings WHERE key NOT LIKE 'notification.%';

-- Seed app-level settings needed by the new Settings UI
INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('app.default_profile_id', '', 'string', 'Terminal profile ID to auto-load on app start');
```

This removes legacy settings (ssh.*, plugin.*, etc.) while preserving notification settings (seeded in migration 020) and seeding new app settings.

### 4.2 Settings API Behavior

The settings page no longer uses the generic settings API for display. Instead:
- Notifications tab reads/writes specific notification.* keys using `updateSetting()` (keys are pre-seeded, no upsert needed)
- General tab reads/writes `app.default_profile_id` using `updateSetting()` (key seeded in migration 021)
- The generic settings CRUD API remains available for programmatic use but has no raw key-value UI
- The existing `set_setting()` function in `database/src/queries/settings.rs` does an upsert (INSERT ON CONFLICT UPDATE) — use this if a setting might not exist yet

---

## 5. Component Architecture

### New Components

Create new directory `ui/src/lib/components/settings/`:
- `ui/src/lib/components/settings/GeneralSettings.svelte` — General tab content
- `ui/src/lib/components/settings/NotificationSettings.svelte` — Notifications tab content
- `ui/src/lib/components/settings/QuickCommandsSettings.svelte` — Quick Commands tab content
- `ui/src/lib/components/settings/AboutSettings.svelte` — About tab content

Other new components:
- `ui/src/routes/settings/+page.svelte` — complete rewrite with tab system
- `ui/src/lib/components/terminal/ProfileEditModal.svelte` — simple metadata edit modal (name, description, is_default)

### Modified Components

- `ui/src/routes/+layout.svelte` — add default profile auto-load on mount
- `ui/src/lib/components/layout/Sidebar.svelte` — add profile edit button (pencil icon on hover)
- `ui/src/lib/components/terminal/ProfileManager.svelte` — support edit mode (pre-populate from existing profile)

### Reused Components

- `Modal.svelte` — for quick command create/edit and profile edit
- `Button.svelte`, `Input.svelte`, `Select.svelte` — form controls
- `Badge.svelte` — category and server scope badges
- `Toast` — feedback for save/test actions

---

## 6. Mobile Considerations

### Tab Bar
- Desktop: full text labels (`General`, `Notifications`, `Quick Commands`, `About`)
- Mobile: icon-only tabs with lucide-svelte icons (Settings, Bell, Terminal, Info)
- Tab bar uses `overflow-x-auto` for safety but icons should fit in 4 slots

### Form Controls
- All inputs full-width on mobile
- Toggle switches thumb-friendly (minimum 36px wide)
- Quick command list items stack vertically (name on top, command below, badges wrap)
- Edit/delete buttons: visible always on mobile (no hover state on touch devices)

### Bottom Safe Area
- Form content scrollable with `pb-[env(safe-area-inset-bottom)]` on the scroll container

---

## 7. Implementation Scope

### What Changes
1. Settings page complete rewrite (4 tab components + tab container)
2. Quick commands CRUD UI (within settings)
3. Profile edit (sidebar pencil icon + ProfileManager edit mode)
4. Default profile auto-load (one addition in layout/page onMount)
5. Migration to purge legacy settings
6. About tab with keyboard shortcuts reference

### What Doesn't Change
- Terminal page (complete)
- Servers page (complete)
- Credentials page (complete)
- Command palette (complete)
- Mobile UX (status bar, gestures, extra keys — all complete)
- Backend APIs (all exist)
- Sidebar navigation structure (4 items: Terminal, Servers, Credentials, Settings)

---

## 8. Success Criteria

- Settings page loads with 4 tabs, no legacy settings visible
- Notification config can be saved and tested from the UI
- Quick commands can be created, edited, and deleted from the Settings > Quick Commands tab
- Created quick commands appear in the command palette
- Default profile auto-loads on app start
- Profiles can be renamed/edited from the sidebar
- All tabs work on mobile with icon-only tab labels
- Keyboard shortcuts reference visible in About tab
- `npm run check` and `npm run build` pass with 0 errors
