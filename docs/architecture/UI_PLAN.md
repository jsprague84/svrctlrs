# Sprint 6: Dioxus Web UI Plan

## Overview

Sprint 6 implements a modern, responsive web dashboard for SvrCtlRS using Dioxus 0.7. The UI provides real-time monitoring, server management, plugin configuration, and task scheduling through an intuitive interface.

## Technology Stack

- **Framework**: Dioxus 0.7 (fullstack mode)
- **Backend**: Axum (already implemented)
- **State Management**: Dioxus Signals + Context API
- **Routing**: Dioxus Router
- **Styling**: Inline CSS with CSS variables (easy dark/light mode)
- **API Communication**: Reqwest for REST API calls
- **Real-time Updates**: Polling (future: WebSockets)

## Theme & Design System

### Color Palette

**Light Mode**:
```css
--bg-primary: #ffffff;
--bg-secondary: #f5f7fa;
--bg-tertiary: #e5e9f0;
--text-primary: #2e3440;
--text-secondary: #4c566a;
--text-muted: #6c7a89;
--accent-primary: #5e81ac;    /* Blue */
--accent-success: #a3be8c;    /* Green */
--accent-warning: #ebcb8b;    /* Yellow */
--accent-error: #bf616a;      /* Red */
--accent-info: #88c0d0;       /* Cyan */
--border-color: #d8dee9;
--shadow: rgba(0, 0, 0, 0.1);
```

**Dark Mode**:
```css
--bg-primary: #2e3440;
--bg-secondary: #3b4252;
--bg-tertiary: #434c5e;
--text-primary: #eceff4;
--text-secondary: #d8dee9;
--text-muted: #a8b0c0;
--accent-primary: #81a1c1;    /* Blue */
--accent-success: #a3be8c;    /* Green */
--accent-warning: #ebcb8b;    /* Yellow */
--accent-error: #bf616a;      /* Red */
--accent-info: #88c0d0;       /* Cyan */
--border-color: #4c566a;
--shadow: rgba(0, 0, 0, 0.3);
```

### Typography

- **Font Family**: System fonts (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif`)
- **Font Sizes**:
  - Heading 1: 2rem (32px)
  - Heading 2: 1.5rem (24px)
  - Heading 3: 1.25rem (20px)
  - Body: 1rem (16px)
  - Small: 0.875rem (14px)
  - Tiny: 0.75rem (12px)

### Spacing Scale

- XS: 4px
- SM: 8px
- MD: 16px
- LG: 24px
- XL: 32px
- 2XL: 48px

### Border Radius

- Small: 4px
- Medium: 8px
- Large: 12px
- Full: 9999px (pills/badges)

## Layout Structure

### Main Layout

```
┌─────────────────────────────────────────────┐
│  Header (60px fixed)                        │
│  [Logo] [Server: localhost] [Theme] [User] │
├─────────┬───────────────────────────────────┤
│         │                                   │
│ Sidebar │  Main Content Area                │
│ (240px) │                                   │
│         │  ┌─────────────────────────────┐  │
│ • Dash  │  │                             │  │
│ • Srv   │  │    Route Content            │  │
│ • Plug  │  │                             │  │
│ • Task  │  │                             │  │
│ • Logs  │  └─────────────────────────────┘  │
│         │                                   │
│         │                                   │
│         │                                   │
│ [v0.1]  │                                   │
└─────────┴───────────────────────────────────┘
```

### Responsive Breakpoints

- **Desktop**: > 1024px (sidebar visible)
- **Tablet**: 768px - 1024px (sidebar collapsible)
- **Mobile**: < 768px (hamburger menu)

## Routes

Using Dioxus Router:

```rust
#[derive(Routable, Clone, PartialEq)]
enum Route {
    #[layout(AppLayout)]
        #[route("/")]
        Dashboard {},

        #[route("/servers")]
        Servers {},

        #[route("/servers/:id")]
        ServerDetail { id: String },

        #[route("/plugins")]
        Plugins {},

        #[route("/plugins/:id")]
        PluginDetail { id: String },

        #[route("/tasks")]
        Tasks {},

        #[route("/logs")]
        Logs {},

        #[route("/settings")]
        Settings {},

    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}
```

## Page Designs

### 1. Dashboard (`/`)

**Purpose**: System overview and health at a glance

**Layout**:
```
┌─────────────────────────────────────────┐
│ Dashboard                               │
├─────────────────────────────────────────┤
│                                         │
│ [Status Cards Row - 4 cards]           │
│ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐   │
│ │Srvrs │ │Plugs │ │Tasks │ │Status│   │
│ │  3   │ │  5   │ │ 12   │ │  OK  │   │
│ └──────┘ └──────┘ └──────┘ └──────┘   │
│                                         │
│ [Active Tasks - 2 column grid]         │
│ ┌─────────────────┬─────────────────┐  │
│ │ Running Now     │ Recent Results  │  │
│ │ • Docker Health │ ✓ Updates Check │  │
│ │ • Speed Test    │ ✓ Weather       │  │
│ └─────────────────┴─────────────────┘  │
│                                         │
│ [Recent Notifications - List]          │
│ ┌─────────────────────────────────────┐│
│ │ 🟢 Docker: All containers healthy  ││
│ │ 🟡 Updates: 5 updates available    ││
│ │ 🔵 Weather: 72°F, Sunny            ││
│ └─────────────────────────────────────┘│
│                                         │
└─────────────────────────────────────────┘
```

**Components**:
- `StatusCard` - Metric display with icon, value, label
- `TaskList` - Live updating task status
- `NotificationFeed` - Recent notifications with priority colors

### 2. Servers (`/servers`)

**Purpose**: Manage monitored servers

**Layout**:
```
┌─────────────────────────────────────────┐
│ Servers                    [+ Add]      │
├─────────────────────────────────────────┤
│                                         │
│ [Search: ________] [Filter: ▼]         │
│                                         │
│ Server List (Table)                     │
│ ┌────────────────────────────────────┐ │
│ │ Name      │ Status │ CPU │ Mem    │ │
│ ├───────────┼────────┼─────┼────────┤ │
│ │localhost  │ 🟢 Up  │ 45% │ 2.1GB │ │
│ │server1    │ 🟢 Up  │ 32% │ 1.8GB │ │
│ │server2    │ 🔴 Down│  -  │   -   │ │
│ └────────────────────────────────────┘ │
│                                         │
└─────────────────────────────────────────┘
```

**Components**:
- `ServerTable` - Sortable, filterable server list
- `ServerRow` - Individual server with status indicator
- `AddServerModal` - Form for adding new servers

### 3. Plugins (`/plugins`)

**Purpose**: View and configure plugins

**Layout**:
```
┌─────────────────────────────────────────┐
│ Plugins                                 │
├─────────────────────────────────────────┤
│                                         │
│ [Core Plugins]                          │
│ ┌─────────────────────────────────────┐│
│ │ 🐳 Docker                      [⚙️] ││
│ │    Container health & cleanup       ││
│ │    Status: Enabled | 3 tasks       ││
│ ├─────────────────────────────────────┤│
│ │ 📦 Updates                     [⚙️] ││
│ │    OS update management             ││
│ │    Status: Enabled | 3 tasks       ││
│ └─────────────────────────────────────┘│
│                                         │
│ [Add-on Plugins]                        │
│ ┌─────────────────────────────────────┐│
│ │ 🌤️  Weather (Optional)         [⚙️] ││
│ │    Weather monitoring               ││
│ │    Status: Disabled                ││
│ └─────────────────────────────────────┘│
│                                         │
└─────────────────────────────────────────┘
```

**Components**:
- `PluginCard` - Plugin info with enable/disable toggle
- `PluginDetail` - Detailed view with configuration
- `TaskList` - Plugin's scheduled tasks

### 4. Tasks (`/tasks`)

**Purpose**: View and manage scheduled tasks

**Layout**:
```
┌─────────────────────────────────────────┐
│ Scheduled Tasks            [Run Now ▼]  │
├─────────────────────────────────────────┤
│                                         │
│ [Tabs: All | Running | Scheduled | Past]│
│                                         │
│ Task List (Cards)                       │
│ ┌─────────────────────────────────────┐│
│ │ Docker Health Check                 ││
│ │ Schedule: */5 * * * * (Every 5 min) ││
│ │ Last run: 2 min ago ✓               ││
│ │ [Run Now] [View Logs] [Edit]        ││
│ ├─────────────────────────────────────┤│
│ │ Weather Update                      ││
│ │ Schedule: 0 6 * * * (Daily 6 AM)    ││
│ │ Last run: 3 hours ago ✓             ││
│ │ [Run Now] [View Logs] [Edit]        ││
│ └─────────────────────────────────────┘│
│                                         │
└─────────────────────────────────────────┘
```

**Components**:
- `TaskCard` - Task info with actions
- `TaskScheduleEditor` - Cron expression builder
- `TaskLogViewer` - Execution history and logs

### 5. Settings (`/settings`)

**Purpose**: Application configuration

**Layout**:
```
┌─────────────────────────────────────────┐
│ Settings                                │
├─────────────────────────────────────────┤
│                                         │
│ [Tabs: General | Plugins | Notif | API]│
│                                         │
│ General Settings                        │
│ ┌─────────────────────────────────────┐│
│ │ Theme                               ││
│ │ ( ) Light  (•) Dark  ( ) Auto      ││
│ │                                     ││
│ │ Refresh Interval                    ││
│ │ [____30___] seconds                ││
│ │                                     ││
│ │ SSH Key Path                        ││
│ │ [~/.ssh/id_rsa              ]      ││
│ │                                     ││
│ │           [Save Changes]            ││
│ └─────────────────────────────────────┘│
│                                         │
└─────────────────────────────────────────┘
```

## Component Library

### Core Components

#### StatusCard
```rust
#[component]
fn StatusCard(
    icon: String,
    label: String,
    value: String,
    color: String, // "success" | "warning" | "error" | "info"
) -> Element
```

#### DataTable
```rust
#[component]
fn DataTable<T>(
    data: Vec<T>,
    columns: Vec<Column>,
    sortable: bool,
    filterable: bool,
) -> Element
```

#### Modal
```rust
#[component]
fn Modal(
    title: String,
    is_open: Signal<bool>,
    children: Element,
) -> Element
```

#### Button
```rust
#[component]
fn Button(
    label: String,
    variant: String, // "primary" | "secondary" | "danger"
    size: String,    // "sm" | "md" | "lg"
    onclick: EventHandler<MouseEvent>,
) -> Element
```

#### Badge
```rust
#[component]
fn Badge(
    text: String,
    color: String, // "success" | "warning" | "error" | "info"
) -> Element
```

### Layout Components

#### AppLayout
- Header with logo, server selector, theme toggle, user menu
- Sidebar with navigation
- Main content area with routing
- Footer with version info

#### Card
- Container with padding, border, shadow
- Optional header and footer

#### Grid / Flex
- Responsive layout utilities

## State Management

### Global State (Context API)

```rust
#[derive(Clone, Copy)]
struct AppState {
    theme: Signal<Theme>,
    servers: Signal<Vec<Server>>,
    plugins: Signal<Vec<Plugin>>,
    tasks: Signal<Vec<Task>>,
    notifications: Signal<Vec<Notification>>,
    current_server: Signal<Option<String>>,
}

#[derive(Clone, PartialEq)]
enum Theme {
    Light,
    Dark,
    Auto,
}
```

### API Client

```rust
struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    async fn get_servers(&self) -> Result<Vec<Server>>;
    async fn get_plugins(&self) -> Result<Vec<Plugin>>;
    async fn get_tasks(&self) -> Result<Vec<Task>>;
    async fn execute_task(&self, plugin_id: &str, task_id: &str) -> Result<PluginResult>;
    async fn get_server_status(&self) -> Result<ServerStatus>;
}
```

### Real-time Updates

Using `use_future` hook with polling:

```rust
let servers = use_signal(Vec::new);

use_future(move || async move {
    loop {
        if let Ok(data) = api_client.get_servers().await {
            servers.set(data);
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
});
```

## Functionality Features

### Real-time Monitoring
- Auto-refresh every 30 seconds (configurable)
- Visual indicators for status changes
- Live task execution status

### Server Management
- Add/remove servers
- View server health metrics (when health plugin implemented)
- SSH connection status

### Plugin Control
- Enable/disable plugins (requires rebuild for add-ons)
- Configure plugin settings
- View plugin tasks and schedules

### Task Scheduling
- View all scheduled tasks
- Manually trigger tasks
- View task execution history
- Configure cron schedules (view-only for now)

### Notifications
- Display recent notifications
- Filter by priority (info, success, warning, error)
- Clear/dismiss notifications

### Theme Switching
- Light/Dark/Auto modes
- Persist user preference
- Smooth transitions

### Responsive Design
- Mobile-friendly navigation
- Collapsible sidebar
- Touch-friendly controls
- Optimized for tablets

## Implementation Phases

### Phase 1: Foundation (Core Structure)
- [ ] Set up Dioxus fullstack project
- [ ] Implement routing structure
- [ ] Create AppLayout with header/sidebar
- [ ] Implement theme system with CSS variables
- [ ] Create basic component library (Button, Card, Badge)

### Phase 2: Dashboard & API Integration
- [ ] Implement API client
- [ ] Create Dashboard page with status cards
- [ ] Implement real-time data fetching
- [ ] Add notification feed

### Phase 3: Server & Plugin Pages
- [ ] Servers page with table
- [ ] Plugins page with cards
- [ ] Plugin detail views
- [ ] Enable/disable controls (UI only for add-ons)

### Phase 4: Tasks & Settings
- [ ] Tasks page with task cards
- [ ] Manual task execution
- [ ] Settings page with theme toggle
- [ ] Configuration persistence

### Phase 5: Polish & Testing
- [ ] Responsive design refinements
- [ ] Loading states and error handling
- [ ] Accessibility improvements
- [ ] Performance optimization
- [ ] Browser testing

## Technical Decisions

### Why Inline CSS with Variables?
- No build-time CSS processing needed
- Easy theme switching with CSS custom properties
- Scoped styling with Dioxus components
- Better for server-side rendering
- Simpler deployment

### Why Context API over Global Signals?
- Better for component-specific state
- Cleaner testing
- More explicit data flow
- Multiple instances support (though not needed here)

### Why Polling over WebSockets?
- Simpler implementation for v1
- Existing REST API already in place
- WebSockets can be added later without UI changes
- Good enough for 30-second refresh intervals

## Future Enhancements (Post-Sprint 6)

- WebSocket support for real-time updates
- Advanced charts and graphs
- Log streaming viewer
- Notification history with search
- Task execution timeline visualization
- Plugin marketplace/discovery
- Multi-user support with authentication
- Mobile app (Dioxus supports mobile!)

## File Structure

```
server/
├── Cargo.toml (add Dioxus dependencies)
├── src/
│   ├── main.rs (serve both API and UI)
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── app.rs (main app component)
│   │   ├── routes.rs (route definitions)
│   │   ├── theme.rs (theme system)
│   │   ├── api_client.rs (API integration)
│   │   ├── components/
│   │   │   ├── mod.rs
│   │   │   ├── layout.rs (AppLayout, Header, Sidebar)
│   │   │   ├── status_card.rs
│   │   │   ├── data_table.rs
│   │   │   ├── button.rs
│   │   │   ├── modal.rs
│   │   │   └── badge.rs
│   │   ├── pages/
│   │   │   ├── mod.rs
│   │   │   ├── dashboard.rs
│   │   │   ├── servers.rs
│   │   │   ├── plugins.rs
│   │   │   ├── tasks.rs
│   │   │   ├── settings.rs
│   │   │   └── not_found.rs
│   │   └── state.rs (global state management)
│   ├── routes/ (existing API routes)
│   └── ...
└── assets/
    ├── logo.svg
    └── favicon.ico
```

## Success Criteria

- ✅ All pages render correctly
- ✅ Navigation works smoothly
- ✅ Real-time data updates automatically
- ✅ Theme switching works
- ✅ Responsive on mobile/tablet/desktop
- ✅ Can manually trigger tasks
- ✅ Settings persist
- ✅ No console errors
- ✅ Fast page loads (< 2s)
- ✅ Works in Chrome, Firefox, Safari, Edge
