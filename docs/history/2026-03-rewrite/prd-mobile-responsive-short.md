# PRD: Mobile Responsive Optimization

**Version**: 1.0
**Created**: 2026-03-02
**Status**: Ready for Implementation
**Priority**: High

---

## Problem Statement

The SvrCtlRS SvelteKit terminal UI is 100% desktop-only with zero responsive Tailwind classes. On a 375px phone:

- **Sidebar** (w-56 = 224px) takes 60% of the screen, leaving 151px for content
- **TerminalPrefsPanel** (w-72 = 288px) covers 77% and cannot be dismissed
- **Toolbar** items all in a single flex row — overflow/compress on phones
- **Search bar** (w-64) pushes Prev/Next/Close buttons off-screen
- **SplitView** renders 2-column grid at 170px per pane (unreadable)
- **Tab labels** hardcoded to max-w-[120px] = 32% of phone width per tab

**Mobile Responsiveness Score: 2/10** — completely non-functional on mobile.

---

## Goals

1. Full application usability on phones (375-430px) without feature reduction
2. Hamburger drawer pattern for sidebar navigation on mobile
3. Bottom sheet pattern for TerminalPrefsPanel on mobile
4. Forced single-pane terminal layout on small screens
5. 44px minimum touch targets for all interactive elements
6. Desktop layout completely unchanged at >= 768px

---

## Non-Goals

- PWA manifest or service worker (separate PRD)
- Capacitor / native app wrapper
- Offline support
- Mobile-specific features (swipe gestures, haptic feedback)
- Visual theme redesign
- New components — only Tailwind responsive classes on existing components

---

## Key Decisions

- **Breakpoint**: Tailwind `md:` (768px) — below = mobile, above = desktop
- **Sidebar**: Hamburger drawer (fixed overlay with backdrop)
- **Prefs Panel**: Bottom sheet on mobile (70vh, slide up)
- **Terminal**: Full PTY + CMD on mobile (no feature reduction)
- **Layouts**: Single-pane forced on mobile, all 4 layouts available on desktop

---

## Stories Summary

| # | Title | Key Files |
|---|-------|-----------|
| US-001 | Sidebar hamburger drawer | +layout.svelte, Sidebar.svelte |
| US-002 | Toolbar responsive | +page.svelte |
| US-003 | Prefs panel bottom sheet | TerminalPrefsPanel.svelte, +page.svelte |
| US-004 | Single layout + compact tabs | mobile.svelte.ts (NEW), SplitView.svelte, TerminalTabs.svelte |
| US-005 | Search bar + command input | +page.svelte, CommandInput.svelte |
| US-006 | Content pages responsive | servers, credentials, settings pages, Modal.svelte |
| US-007 | Touch targets + polish | app.html, app.css, Sidebar.svelte, toolbar buttons |

---

## Technical Notes

### Stack
- Tailwind CSS v4 (`@import 'tailwindcss'` with `@theme` block)
- SvelteKit 5 with Svelte 5 runes ($state, $derived, $effect)
- adapter-static (SPA)
- All responsive via `md:` prefix — no custom @media queries

### Context7 Research Requirement
Each story MUST use the Context7 MCP tool to query latest Tailwind CSS v4 and SvelteKit 5 documentation before implementing. This ensures usage of current best practices and avoids deprecated patterns.

### Quality Checks
```bash
cd ui && npm run check    # TypeScript typecheck
cd ui && npm run build    # Production build
```

### Key Existing Patterns
- `$state` + localStorage for persistence (theme.svelte.ts, terminalPrefs.svelte.ts)
- Sidebar already has collapsed/expanded toggle with localStorage
- TerminalPrefsPanel already has open/close with slide transition
- SplitView uses `$derived` grid classes from layout prop
