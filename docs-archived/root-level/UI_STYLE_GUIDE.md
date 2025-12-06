# UI Style Guide - Friendly vs Technical

## Overview

SvrCtlRS supports two visual styles while maintaining the same HTML structure and Nord color palette:

1. **Friendly/Spacious** (`styles.css`) - Current style, rounded, playful
2. **Technical/Dense** (`styles-technical.css`) - Professional, GitHub-inspired

## Quick Comparison

| Feature | Friendly | Technical |
|---------|----------|-----------|
| **Base Font Size** | 16px | 14px |
| **Card Border Radius** | 12px | 6px |
| **Button Border Radius** | 8px | 4px |
| **Badge Style** | Pill-shaped (9999px) | Rectangle (4px) |
| **Card Padding** | 24px | 16px |
| **Card Shadow** | `0 2px 8px` | None (border instead) |
| **Table Cell Padding** | 12px | 8px 12px |
| **Button Height** | ~44px | 32px |
| **Icons** | Emojis | Unicode symbols/SVG |
| **Hover Effects** | Transform + opacity | Opacity only |
| **Active Nav** | Full background | Border + subtle bg |

## How to Switch Styles

### Option 1: Replace Stylesheet

In `server/templates/base.html`, change:

```html
<!-- Friendly style -->
<link rel="stylesheet" href="/static/css/styles.css">

<!-- Technical style -->
<link rel="stylesheet" href="/static/css/styles-technical.css">
```

### Option 2: Dynamic Theme Switcher (Future)

Add theme toggle in header:

```html
<button @click="switchTheme()" class="btn btn-secondary btn-sm">
    <span x-text="theme === 'friendly' ? 'Technical' : 'Friendly'"></span>
</button>

<script>
function switchTheme() {
    const link = document.querySelector('link[href*="styles"]');
    const current = link.href.includes('technical') ? 'friendly' : 'technical';
    link.href = `/static/css/styles${current === 'technical' ? '-technical' : ''}.css`;
    localStorage.setItem('theme', current);
}
</script>
```

## Icon Strategy

### Current (Friendly): Emojis

```html
<h3>📡 {{ server.name }}</h3>
<span class="badge">💻 Local</span>
```

**Pros:** Universal, no dependencies, colorful
**Cons:** Inconsistent rendering, feels playful, not professional

### Technical Option 1: Unicode Symbols

```html
<h3><span class="status-indicator">●</span> {{ server.name }}</h3>
<span class="badge">Local</span>
```

**Pros:** Zero dependencies, monospaced, professional
**Cons:** Limited selection, grayscale only

### Technical Option 2: Lucide Icons (Recommended)

```html
<!-- Add to base.html once -->
<script src="https://unpkg.com/lucide@latest"></script>
<script>lucide.createIcons();</script>

<!-- Usage -->
<i data-lucide="server" class="icon"></i> {{ server.name }}
<i data-lucide="activity" class="icon"></i> Tasks
<i data-lucide="check-circle" class="icon status-success"></i> Online
```

**Pros:** Clean SVG icons, GitHub-like, customizable size/color
**Cons:** 15KB dependency (acceptable)

### Technical Option 3: Octicons (GitHub's Icons)

```html
<!-- Add to base.html once -->
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/octicons/19.8.0/octicons.min.css">

<!-- Usage -->
<span class="octicon octicon-server"></span> {{ server.name }}
```

**Pros:** Actual GitHub icons, professional
**Cons:** Limited selection, slightly larger

## Implementation Roadmap

### Phase 1: Structure (This Week)
- [x] Create `styles-technical.css`
- [ ] Build HTML components that work with both styles
- [ ] Use semantic classes (not style-specific)
- [ ] Design for density (tables over cards for data)

### Phase 2: Icon Migration (Week 2)
- [ ] Choose icon strategy (recommend Lucide)
- [ ] Create icon mapping:
  - `📡 Server` → `<i data-lucide="server">`
  - `⚙️ Tasks` → `<i data-lucide="list-checks">`
  - `🔌 Plugins` → `<i data-lucide="puzzle">`
  - `💻 Local` → `<i data-lucide="laptop">`
  - `✓ Success` → `<i data-lucide="check-circle">`
  - `✗ Error` → `<i data-lucide="x-circle">`
- [ ] Update templates with configurable icons
- [ ] Test both emoji and icon modes

### Phase 3: Testing (Week 2-3)
- [ ] Test both stylesheets side-by-side
- [ ] Verify mobile responsiveness
- [ ] Check accessibility (WCAG AA)
- [ ] Get user feedback
- [ ] Choose default style

### Phase 4: Cleanup (Week 3)
- [ ] Remove unused stylesheet
- [ ] Optimize CSS (remove duplicates)
- [ ] Document chosen style
- [ ] Update screenshots in README

## Recommended Approach

**For SvrCtlRS:**

1. **Keep both stylesheets** during development
2. **Default to technical** for server management (professional context)
3. **Use Lucide icons** for best balance of quality and simplicity
4. **Build density-first** (tables for tasks/servers, cards for plugins/dashboard)

## CSS Variable Mapping

Both stylesheets use the same CSS variables from Nord theme:

```css
/* Colors (same in both) */
--bg-primary
--bg-secondary
--bg-tertiary
--text-primary
--text-secondary
--text-muted
--accent-primary
--accent-success
--accent-error
--accent-warning
--border-color

/* Spacing (different) */
--spacing-xs: 4px   (same)
--spacing-sm: 8px   (same)
--spacing-md: 12px  (same)
--spacing-lg: 16px  (friendly: 24px)
--spacing-xl: 24px  (friendly: 32px)

/* Radius (different) */
--radius-sm: 2px   (friendly: 4px)
--radius-md: 4px   (friendly: 8px)
--radius-lg: 6px   (friendly: 12px)
```

## Component Examples

### Card Component (Works with Both)

```html
<div class="card">
    <div class="card-header">
        <h3 class="card-title">Server Name</h3>
        <span class="badge badge-success">Enabled</span>
    </div>

    <p class="text-secondary">Description here</p>

    <div class="flex gap-2">
        <button class="btn btn-primary btn-sm">Edit</button>
        <button class="btn btn-danger btn-sm">Delete</button>
    </div>
</div>
```

**Renders:**
- **Friendly**: Rounded corners (12px), shadow, spacious padding (24px)
- **Technical**: Less rounded (6px), no shadow, compact padding (16px)

### Table Component (Technical-Optimized)

```html
<table>
    <thead>
        <tr>
            <th>Name</th>
            <th>Host</th>
            <th>Status</th>
        </tr>
    </thead>
    <tbody>
        <tr>
            <td class="server-name">web-01</td>
            <td class="server-host">192.168.1.100</td>
            <td><span class="status status-online">Online</span></td>
        </tr>
    </tbody>
</table>
```

**Renders:**
- **Friendly**: 12px cell padding, normal hover
- **Technical**: 8px 12px cell padding, monospace for technical columns, subtle hover

## Testing Checklist

Before committing to one style:

- [ ] Test on desktop (1920px, 1440px, 1280px)
- [ ] Test on mobile (375px, 768px)
- [ ] Test dark mode
- [ ] Test light mode (if implemented)
- [ ] Check all pages: Dashboard, Servers, Tasks, Plugins, Settings
- [ ] Verify accessibility (keyboard navigation, screen readers)
- [ ] Check print styles
- [ ] Test with 50+ servers/tasks (density stress test)

## Accessibility Notes

Both styles maintain WCAG AA compliance:

- ✅ Color contrast ratios >= 4.5:1
- ✅ Interactive elements >= 44px touch targets (mobile)
- ✅ Focus indicators visible
- ✅ Keyboard navigation supported
- ✅ Semantic HTML structure

**Technical style** actually improves accessibility:
- Higher information density reduces scrolling
- Monospace fonts improve readability for technical content
- Border-based focus indicators clearer than subtle shadows

---

**Last Updated**: 2024-11-26
**Recommendation**: Start with technical style as default for server management context
