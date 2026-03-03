# PRD: Mobile Responsive Optimization

## Introduction

The SvrCtlRS SvelteKit terminal UI is currently 100% desktop-only with zero responsive Tailwind classes, zero media queries, and hardcoded fixed widths on critical components. On a 375px phone screen, the sidebar consumes 60% of the viewport, the TerminalPrefsPanel covers 77% and cannot be dismissed, the toolbar overflows, split views render unreadably, and the search bar pushes buttons off-screen. This PRD covers making the entire application fully responsive for phone-first (375-430px) usage while preserving the desktop experience.

## Goals

- Make the full application usable on phones (375-430px) without feature reduction
- Implement a hamburger drawer pattern for sidebar navigation on mobile
- Convert the TerminalPrefsPanel to a bottom sheet on mobile
- Force single-pane layout on small screens
- Ensure all interactive elements meet 44px minimum touch target size
- Use Tailwind CSS v4 responsive breakpoints (`sm:`, `md:`, `lg:`) and container queries
- Preserve existing desktop layout and behavior unchanged

## User Stories

### US-001: Convert sidebar to hamburger drawer on mobile
**Description:** As a mobile user, I want the sidebar to be hidden behind a hamburger menu so the full screen is available for content.

**Acceptance Criteria:**
- [ ] Sidebar has `hidden md:flex` — hidden on mobile by default, visible on `md+`
- [ ] Hamburger button (Menu icon) visible only on `< md` screens, positioned in top-left of layout
- [ ] Tapping hamburger opens sidebar as fixed overlay (`fixed inset-0 z-40`) sliding in from left
- [ ] Semi-transparent backdrop behind sidebar overlay; tapping backdrop closes sidebar
- [ ] Close button (X) visible in sidebar header when in overlay mode
- [ ] Sidebar auto-closes when a nav link is tapped (on mobile only)
- [ ] Desktop behavior (collapsible, persistent) completely unchanged
- [ ] Sidebar overlay state managed via a `mobileOpen` state variable in `+layout.svelte`
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-002: Make terminal page toolbar responsive
**Description:** As a mobile user, I want the terminal toolbar to fit on my screen so I can access server selection and controls without horizontal scrolling.

**Acceptance Criteria:**
- [ ] Toolbar wraps to two rows on `< md`: row 1 = server select + mode select + connect button; row 2 = connection badge + action icons
- [ ] App title "SvrCtlRS" hidden on `< md` (`hidden md:block`)
- [ ] TerminalIcon hidden on `< md`
- [ ] Server select and mode select use `flex-1 min-w-0` to shrink proportionally on small screens
- [ ] Action buttons (clear, copy, download, settings) remain icon-only with no layout change
- [ ] Layout uses `flex-wrap` with appropriate gap
- [ ] Desktop toolbar layout unchanged (single row)
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-003: Convert TerminalPrefsPanel to mobile bottom sheet
**Description:** As a mobile user, I want the preferences panel to appear as a bottom sheet I can easily dismiss, instead of an overlay that covers most of my screen.

**Acceptance Criteria:**
- [ ] On `< md`: panel renders as bottom sheet — `fixed bottom-0 left-0 right-0 h-[70vh]` with rounded top corners, slide-up animation (`translate-y-full` / `translate-y-0`)
- [ ] On `>= md`: existing behavior preserved — `absolute right-0 top-0 bottom-0 w-72` slide from right
- [ ] Bottom sheet has a visible drag handle bar at top (decorative, 40px wide centered gray bar)
- [ ] Backdrop overlay on mobile (`fixed inset-0 bg-black/40 z-10`) — tapping closes panel
- [ ] Close button visible in header on both mobile and desktop
- [ ] Panel scrollable on both layouts
- [ ] `z-index` high enough to sit above terminal content
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-004: Force single layout and compact tabs on mobile
**Description:** As a mobile user, I want split layouts disabled and tabs compacted so the terminal is readable on my phone.

**Acceptance Criteria:**
- [ ] SplitView: on `< md` screens, always render `grid-cols-1 grid-rows-1` regardless of `layout` prop — use a Svelte `$effect` or `$derived` that checks `window.innerWidth` or a `matchMedia` listener
- [ ] Layout selector buttons in TerminalTabs hidden on `< md` (`hidden md:flex`)
- [ ] Tab items: reduce padding to `px-2 py-1` on mobile, `px-3 py-1.5` on `md+`
- [ ] Tab label max-width: `max-w-[60px] md:max-w-[120px]`
- [ ] Mode badge (`CMD`/`PTY`) hidden on `< md` to save space
- [ ] New tab button and close button touch targets are at least 44px
- [ ] Horizontal scroll still works for overflow tabs on mobile
- [ ] Desktop split layouts unchanged
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-005: Responsive search bar and command input
**Description:** As a mobile user, I want the search bar and command input to fit the screen width.

**Acceptance Criteria:**
- [ ] Search input: `w-full md:w-64` (full width on mobile, fixed on desktop)
- [ ] Search bar wraps controls: input on first line full-width, Prev/Next/Close buttons below on mobile (use `flex-wrap`)
- [ ] CommandInput component: input takes full width on mobile, submit button doesn't get pushed off-screen
- [ ] Padding reduced on mobile: `px-2 md:px-4` on search bar container
- [ ] Desktop search bar layout unchanged (single row)
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-006: Responsive content pages (servers, credentials, settings)
**Description:** As a mobile user, I want the servers, credentials, and settings pages to be usable on my phone.

**Acceptance Criteria:**
- [ ] Servers page: server cards stack vertically on mobile with full-width cards; action buttons (edit, delete, test) use icon-only on `< md`
- [ ] Credentials page: credential cards stack vertically; type badge and actions fit on one line
- [ ] Settings page: settings list uses full width; inline edit input doesn't overflow
- [ ] All page headers: title + "Add" button use `flex-wrap` or stack on mobile
- [ ] Modal component: modals are `w-full max-w-lg` and have `max-h-[90vh] overflow-y-auto` on mobile
- [ ] Form inputs within modals use full width
- [ ] Desktop layouts unchanged
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

### US-007: Touch targets and general mobile polish
**Description:** As a mobile user, I want all buttons and interactive elements to be easy to tap accurately.

**Acceptance Criteria:**
- [ ] All buttons and interactive elements have minimum 44x44px touch target on mobile (use `min-h-[44px] min-w-[44px]` or padding to achieve)
- [ ] Add `touch-manipulation` CSS to body to eliminate 300ms tap delay
- [ ] Icon-only buttons in toolbar: `p-2 md:p-1` to increase touch area on mobile
- [ ] Sidebar nav items: `py-2.5 md:py-1.5` for taller touch targets on mobile
- [ ] Tab close button: `p-1.5 md:p-0.5` for easier tapping
- [ ] Select dropdowns: `py-2 md:py-1` for taller targets on mobile
- [ ] Viewport meta tag already has `width=device-width, initial-scale=1` (verified) — add `viewport-fit=cover` for notched phones
- [ ] Add `env(safe-area-inset-*)` padding to sidebar overlay and bottom sheet for notched devices
- [ ] Production build succeeds (`cd ui && npm run build`)
- [ ] Typecheck passes (`cd ui && npm run check`)
- [ ] Verify in browser using dev-browser skill

## Functional Requirements

- FR-1: Sidebar must use hamburger drawer pattern on screens `< 768px` (Tailwind `md` breakpoint)
- FR-2: All responsive behavior must use Tailwind CSS v4 responsive prefix classes (`sm:`, `md:`, `lg:`) — no custom media queries in `<style>` blocks
- FR-3: Terminal toolbar must not horizontally overflow on 375px screens
- FR-4: TerminalPrefsPanel must be dismissible on all screen sizes via visible close affordance
- FR-5: Split/quad terminal layouts must be disabled on `< 768px` — force single pane
- FR-6: All form inputs and modals must be usable on 375px screens without horizontal scrolling
- FR-7: Touch targets must be minimum 44x44px on mobile per WCAG 2.5.5
- FR-8: Desktop layout and behavior must remain completely unchanged at `>= 768px`
- FR-9: No JavaScript-based responsive detection for layout — use CSS/Tailwind responsive classes wherever possible (exception: SplitView may need `matchMedia` for grid override)

## Non-Goals

- No PWA manifest or service worker (separate PRD)
- No Capacitor/native app wrapper
- No offline support
- No mobile-specific features (e.g., swipe gestures, haptic feedback)
- No redesign of the visual theme or color system
- No new components — use Tailwind responsive classes on existing components

## Design Considerations

- **Breakpoint strategy**: Mobile-first using Tailwind's `md:` prefix (768px). Below `md` = mobile phone layout. Above `md` = existing desktop layout unchanged.
- **Sidebar pattern**: Hamburger drawer is standard for admin tools on mobile (matches Rundeck, AWX, Grafana patterns). Fixed overlay with backdrop, not push-content.
- **Bottom sheet**: TerminalPrefsPanel on mobile follows Material Design bottom sheet pattern — 70vh height, rounded top corners, drag handle.
- **Touch targets**: WCAG 2.5.5 (Level AAA) recommends 44x44px. Use padding increases, not element size increases, to preserve visual density on desktop.

## Technical Considerations

- **Tailwind CSS v4**: Uses `@import 'tailwindcss'` syntax with `@theme` block for custom properties. Default breakpoints: `sm: 640px`, `md: 768px`, `lg: 1024px`, `xl: 1280px`.
- **Container queries**: Tailwind v4 supports `@container` and `@md:` variants for parent-size-based responsive design. Consider for SplitView pane content.
- **SvelteKit SPA**: App uses `adapter-static`. All responsive logic is client-side CSS. No SSR considerations for responsive behavior.
- **Existing state pattern**: `$state` + localStorage used for sidebar collapsed state and terminal prefs — same pattern for `mobileOpen` state.
- **Context7 research**: Each story should use Context7 MCP tool to query latest Tailwind CSS v4 and SvelteKit 5 responsive patterns before implementing. This ensures usage of current best practices.

## Success Metrics

- Application fully usable on a 375px wide viewport (iPhone SE) without horizontal scrolling
- All interactive elements tappable without precision targeting
- Terminal (PTY and CMD modes) fully functional on mobile
- Desktop layout pixel-identical to current implementation at `>= 768px`
- Zero TypeScript errors, zero new build warnings
- `npm run check` and `npm run build` pass cleanly

## Open Questions

- Should we add a media query to auto-close TerminalPrefsPanel when resizing from mobile to desktop (or vice versa)?
- Should keyboard shortcuts be disabled on mobile (since there's no physical keyboard)?
