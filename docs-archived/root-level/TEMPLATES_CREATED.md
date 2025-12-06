# SvrCtlRS UI Templates - Complete Restructure

## Summary

Created comprehensive HTMX/Askama HTML templates for the complete SvrCtlRS restructure, implementing a modern job-based architecture with server management, notifications, and scheduling capabilities.

**Total Templates Created:** 34 new files
**Files Updated:** 2 existing files
**Theme:** Tokyo Night with responsive design
**Technologies:** HTMX 2.0.3, Alpine.js 3.14.1, Lucide Icons

---

## Part 1: Credentials Management

### Pages
- **`pages/credentials.html`** - Main credentials management page
  - List view with add credential button
  - Description of credential functionality
  - HTMX-powered dynamic forms

### Components
- **`components/credential_list.html`** - Credential list display
  - Shows credential type (Password/SSH Key)
  - Displays usage count (number of servers)
  - Prevents deletion when in use
  - Edit and delete actions

- **`components/credential_form.html`** - Create/edit credential form
  - Dynamic form based on auth type (Alpine.js)
  - Password or SSH private key fields
  - Optional passphrase for encrypted keys
  - Security notes (encrypted at rest)

---

## Part 2: Tags Management

### Pages
- **`pages/tags.html`** - Main tags management page
  - Organizational tags for servers
  - Color-coded badges
  - Server count display

### Components
- **`components/tag_list.html`** - Tag list with server counts
  - Color-coded tag badges
  - Server association counts
  - Usage-based deletion protection

- **`components/tag_form.html`** - Create/edit tag form
  - Name validation (lowercase, no spaces)
  - Color picker integration
  - Live preview of tag appearance
  - Optional description

---

## Part 3: Servers (Updated)

### Pages
- **`pages/servers.html`** (updated) - Enhanced server management
  - Added description text
  - Maintained existing structure

### Components
- **`components/server_list_updated.html`** - Enhanced server list
  - Credential display (if assigned)
  - Tag badges with colors
  - Capability badges (Docker, systemd, apt, etc.)
  - Detect capabilities button
  - Improved visual hierarchy

- **`components/server_form_updated.html`** - Enhanced server form
  - Credential dropdown selection
  - Multi-select tag checkboxes
  - Manual username fallback
  - Enable/disable toggle
  - Connection testing

- **`components/server_capabilities.html`** - Capability display
  - Detected capabilities list
  - Icon representation
  - Empty state handling

---

## Part 4: Job Types

### Pages
- **`pages/job_types.html`** - Job type management
  - Categories of work (Shell, Docker, systemd, Composite)
  - Capability requirements
  - Command template associations

### Components
- **`components/job_type_list.html`** - Job type list
  - Execution type badges
  - Required capability display
  - Command template count
  - Usage-based deletion protection
  - View and edit actions

- **`components/job_type_form.html`** - Create/edit job type
  - Execution type selection
  - Multi-select capability requirements
  - Description field
  - Validation helpers

---

## Part 5: Command Templates

### Components
- **`components/command_template_list.html`** - Template list
  - Command preview with syntax highlighting
  - Default arguments display
  - Inline edit and delete actions
  - Add template button

- **`components/command_template_form.html`** - Template form
  - Command template editor
  - Variable placeholder guidance
  - Dynamic argument list (Alpine.js)
  - Add/remove arguments
  - Description field

---

## Part 6: Job Templates

### Pages
- **`pages/job_templates.html`** - Job template management
  - Reusable job configurations
  - Target server selection
  - On-demand and scheduled execution

### Components
- **`components/job_template_list.html`** - Template list
  - Job type display
  - Target type (All/Tagged/Specific)
  - Tag and server counts
  - Step count for composite jobs
  - Run Now action
  - Schedule count protection

- **`components/job_template_form.html`** - Template form
  - Job type selection
  - Target type selector (Alpine.js)
  - Conditional tag/server selection
  - Timeout configuration
  - Description field

---

## Part 7: Composite Job Steps

### Components
- **`components/job_template_steps.html`** - Step list
  - Ordered step display
  - Move up/down actions
  - Job type association
  - Continue-on-failure indicator
  - Inline edit and delete

- **`components/job_template_step_form.html`** - Step form
  - Step name and description
  - Job type selection
  - Order index configuration
  - Continue-on-failure toggle
  - Auto-incrementing step numbers

---

## Part 8: Job Schedules

### Pages
- **`pages/job_schedules.html`** - Schedule management
  - Replaces old tasks.html
  - Cron-based scheduling
  - Auto-refresh every 30s

### Components
- **`components/job_schedule_list.html`** - Grouped schedule list
  - Grouped by job template
  - Cron expression display
  - Next run prediction
  - Last run status
  - Enable/disable toggle
  - Run Now action

- **`components/job_schedule_form.html`** - Schedule form
  - Schedule name
  - Job template selection
  - Cron expression input
  - Cron syntax helper
  - Enable toggle

---

## Part 9: Job Runs

### Pages
- **`pages/job_runs.html`** - Job execution history
  - Recent runs display
  - Auto-refresh every 10s
  - Manual refresh button

### Components
- **`components/job_run_list.html`** - Paginated run list
  - Status badges (Success/Failed/Partial/Running)
  - Trigger type display
  - Server success/failure counts
  - Duration display
  - Expandable details
  - Pagination controls

- **`components/job_run_detail.html`** - Detailed run view
  - Execution timeline
  - Server-by-server results
  - Refresh results action
  - Status indicators

- **`components/server_job_results.html`** - Per-server results
  - Exit code display
  - Stdout/stderr in collapsible sections
  - Error message highlighting
  - Duration tracking
  - Empty state handling

---

## Part 10: Notification Channels

### Pages
- **`pages/notification_channels.html`** - Channel management
  - Gotify and ntfy.sh support
  - Test notification functionality
  - Usage tracking

### Components
- **`components/notification_channel_list.html`** - Channel list
  - Channel type badges
  - Endpoint display
  - Policy usage count
  - Test button
  - Usage-based deletion protection

- **`components/notification_channel_form.html`** - Channel form
  - Dynamic fields based on type (Alpine.js)
  - Gotify: URL + token
  - ntfy.sh: URL + topic + optional token
  - Security notes
  - Description field

---

## Part 11: Notification Policies

### Pages
- **`pages/notification_policies.html`** - Policy management
  - Define when to send notifications
  - Scope configuration
  - Multi-condition support

### Components
- **`components/notification_policy_list.html`** - Policy list
  - Channel association
  - Scope display (All/Specific/Type)
  - Condition display (Success/Failure/Partial)
  - Enable/disable toggle
  - Quick actions

- **`components/notification_policy_form.html`** - Policy form
  - Channel selection
  - Scope type selector (Alpine.js)
  - Conditional job template/type selection
  - Multi-condition checkboxes
  - Enable toggle
  - Description field

---

## Part 12: Updated Dashboard

### Pages
- **`pages/dashboard_updated.html`** - Enhanced dashboard
  - Updated stats (schedules, templates, runs)
  - Quick action grid
  - Recent job runs preview
  - System status cards
  - Notification metrics

---

## Part 13: Updated Base Layout

### Base Templates
- **`base_updated.html`** - Enhanced navigation
  - Organized into sections:
    - Infrastructure (Servers, Credentials, Tags)
    - Jobs (Types, Templates, Schedules, Runs)
    - Notifications (Channels, Policies)
    - System (Settings)
  - Section headers for organization
  - Maintained theme toggle
  - Mobile-responsive sidebar

---

## Template Features

### HTMX Integration
- Dynamic form loading (`hx-get`, `hx-post`, `hx-put`, `hx-delete`)
- Targeted content swapping (`hx-target`, `hx-swap`)
- Auto-refresh capabilities (`hx-trigger="every Ns"`)
- Confirmation dialogs (`hx-confirm`)
- Loading indicators

### Alpine.js Features
- Client-side state management
- Conditional field display
- Dynamic form arrays
- Theme persistence
- Mobile menu toggle

### Design Patterns
- **Empty States**: Helpful messages when no data exists
- **Loading States**: Indicators during async operations
- **Error Handling**: Alert messages with contextual styling
- **Validation**: Client-side hints and server-side feedback
- **Responsive Design**: Mobile-first with breakpoints
- **Accessibility**: Proper labels, ARIA attributes, semantic HTML

### Color Scheme (Tokyo Night)
- Primary: `#7aa2f7` (blue)
- Success: `#9ece6a` (green)
- Warning: `#e0af68` (orange)
- Error: `#f7768e` (red)
- Info: `#7dcfff` (cyan)
- Background: `#1a1b26`
- Cards: `#24283b`

### Icon System (Lucide)
All templates use Lucide icons with consistent sizing:
- Navigation: 16x16px
- Actions: 14x14px
- Small actions: 12x12px
- Headers: 20x20px

### Common Components
- **Badges**: Status indicators with contextual colors
- **Cards**: Content containers with headers and actions
- **Grids**: Responsive layouts (grid-1, grid-2, grid-3)
- **Forms**: Consistent styling with labels and helpers
- **Buttons**: Primary, secondary, danger variants
- **Alerts**: Info, success, warning, error messages

---

## File Organization

```
server/templates/
├── base.html (original)
├── base_updated.html (new navigation)
├── pages/
│   ├── credentials.html
│   ├── tags.html
│   ├── servers.html (updated)
│   ├── job_types.html
│   ├── job_templates.html
│   ├── job_schedules.html
│   ├── job_runs.html
│   ├── notification_channels.html
│   ├── notification_policies.html
│   └── dashboard_updated.html
└── components/
    ├── credential_list.html
    ├── credential_form.html
    ├── tag_list.html
    ├── tag_form.html
    ├── server_list_updated.html
    ├── server_form_updated.html
    ├── server_capabilities.html
    ├── job_type_list.html
    ├── job_type_form.html
    ├── command_template_list.html
    ├── command_template_form.html
    ├── job_template_list.html
    ├── job_template_form.html
    ├── job_template_steps.html
    ├── job_template_step_form.html
    ├── job_schedule_list.html
    ├── job_schedule_form.html
    ├── job_run_list.html
    ├── job_run_detail.html
    ├── server_job_results.html
    ├── notification_channel_list.html
    ├── notification_channel_form.html
    ├── notification_policy_list.html
    └── notification_policy_form.html
```

---

## Next Steps

### Backend Integration Required
1. Create Askama template structs in `server/src/templates.rs`
2. Implement route handlers in `server/src/ui_routes.rs`
3. Add database models and queries
4. Implement job execution engine
5. Add notification delivery logic

### Database Migrations
- Create tables for all new entities
- Set up foreign key relationships
- Add indexes for performance
- Seed example data for testing

### API Endpoints
- CRUD operations for all entities
- Job execution endpoints
- Schedule management
- Notification testing
- Capability detection

### Testing Checklist
- [ ] Mobile responsiveness on all pages
- [ ] Form validation (client and server)
- [ ] HTMX partial updates
- [ ] Alpine.js state management
- [ ] Icon rendering after HTMX swaps
- [ ] Empty state displays
- [ ] Error handling and messages
- [ ] Pagination functionality
- [ ] Auto-refresh behaviors
- [ ] Confirmation dialogs

---

## Notes

- All templates follow the existing Tokyo Night theme
- Mobile-first responsive design throughout
- Consistent component patterns for maintainability
- Comprehensive error and empty state handling
- Production-ready HTML with proper semantics
- Accessibility considerations included
- Ready for backend implementation

**Status**: All UI templates complete and ready for integration with backend logic.
