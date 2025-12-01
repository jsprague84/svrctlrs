# SvrCtlRS Architecture Documentation

**Version**: v2.1.0
**Last Updated**: 2025-11-30
**Status**: Production Ready

## Table of Contents

1. [System Overview](#system-overview)
2. [Workspace Structure](#workspace-structure)
3. [Application Layers](#application-layers)
4. [Frontend-Backend Communication](#frontend-backend-communication)
5. [Database Architecture](#database-architecture)
6. [Display Model Pattern](#display-model-pattern)
7. [Data Flow Diagrams](#data-flow-diagrams)
8. [Deprecated Elements Review](#deprecated-elements-review)

---

## System Overview

SvrCtlRS is an **infrastructure monitoring and automation platform** built with:

- **Backend**: Axum 0.8 web framework
- **Frontend**: HTMX 2.0.3 + Alpine.js 3.14.1 (HTML-over-the-wire)
- **Templates**: Askama 0.12 (compiled Jinja-like templates)
- **Database**: SQLite with sqlx (compile-time checked queries)
- **Runtime**: Tokio async runtime
- **Scheduler**: Built-in cron-like scheduler (no external dependencies)

### Architecture Philosophy

1. **Server-Driven UI**: HTMX enables rich interactivity without client-side state management
2. **Optimized Queries**: JOINed database queries with specialized result structs
3. **Display Models**: Separate database models from UI presentation models
4. **Job-Based Architecture**: Flexible job execution system with templates, schedules, and command types
5. **Type Safety**: Compile-time SQL checking and strong typing throughout

---

## Workspace Structure

```
svrctlrs/
├── core/                      # Shared types, traits, and utilities
│   └── src/
│       ├── lib.rs            # Public API exports
│       ├── error.rs          # Unified error types
│       ├── executor.rs       # Job execution engine
│       ├── notifications.rs  # Gotify + ntfy.sh notification system
│       ├── remote.rs         # SSH remote execution
│       └── types.rs          # Shared domain types
│
├── database/                  # SQLite abstraction layer
│   ├── src/
│   │   ├── lib.rs            # Database pool management
│   │   ├── error.rs          # Database-specific errors
│   │   ├── migrations.rs     # Schema migration runner
│   │   └── queries/          # Query modules by domain
│   │       ├── mod.rs
│   │       ├── servers.rs            # Server CRUD + JOINed queries
│   │       ├── credentials.rs        # SSH credentials
│   │       ├── tags.rs               # Server tags
│   │       ├── job_templates.rs      # Job templates + step counts
│   │       ├── job_schedules.rs      # Job schedules + names/counts
│   │       ├── job_runs.rs           # Job execution history
│   │       └── notifications.rs      # Notification channels/policies
│   └── migrations/            # SQL migration files
│
├── scheduler/                 # Built-in cron scheduler
│   └── src/
│       ├── lib.rs            # Scheduler implementation
│       └── cron.rs           # Cron expression parsing
│
└── server/                    # Web server and UI
    ├── src/
    │   ├── main.rs           # Entry point, server startup
    │   ├── config.rs         # Configuration loading (env + TOML)
    │   ├── state.rs          # Application state (Arc-wrapped)
    │   ├── templates.rs      # Askama template structs + Display models
    │   └── routes/
    │       ├── mod.rs        # Route registration
    │       ├── api/          # REST API endpoints
    │       │   └── ...
    │       └── ui/           # HTMX UI route handlers
    │           ├── dashboard.rs
    │           ├── servers.rs
    │           ├── credentials.rs
    │           ├── job_templates.rs
    │           ├── job_schedules.rs
    │           ├── job_runs.rs
    │           ├── notifications.rs
    │           └── tasks.rs
    │
    ├── templates/             # Askama HTML templates
    │   ├── base.html         # Base layout (navigation, header, footer)
    │   ├── pages/            # Full page templates
    │   │   ├── dashboard.html
    │   │   ├── servers.html
    │   │   ├── job_templates.html
    │   │   ├── job_schedules.html
    │   │   └── ...
    │   └── components/       # Reusable HTMX partials
    │       ├── server_list.html
    │       ├── server_form.html
    │       ├── job_template_card.html
    │       ├── notification_policy_list.html
    │       └── ...
    │
    └── static/               # Static assets (served by Axum)
        ├── css/
        │   └── styles.css    # Nord-inspired theme, responsive design
        └── js/
            ├── htmx.min.js   # HTMX library (2.0.3)
            └── alpine.min.js # Alpine.js for client-side state (3.14.1)
```

### Crate Dependency Graph

```
server
  ├── core
  ├── database
  │     └── core
  ├── scheduler
  └── plugins/*
        └── core

database
  └── core

scheduler
  (no dependencies)

plugins/*
  └── core
```

**Key Insight**: `core` is the foundation crate. `database` depends on `core` for error types. `server` orchestrates everything.

---

## Application Layers

### Layer 1: Database Layer (`database/`)

**Responsibilities**:
- SQLite connection pool management
- Schema migrations
- Type-safe query execution with sqlx
- Optimized JOINed queries

**Key Patterns**:

1. **Basic CRUD Queries**:
   ```rust
   // Simple insert
   pub async fn create_server(pool: &Pool<Sqlite>, server: &Server) -> Result<i64>

   // Simple select
   pub async fn get_server(pool: &Pool<Sqlite>, id: i64) -> Result<Server>
   ```

2. **Optimized JOINed Queries**:
   ```rust
   // Struct for JOINed result (includes related data)
   #[derive(Debug, Clone, sqlx::FromRow)]
   pub struct ServerWithDetails {
       // Flattened server fields
       pub id: i64,
       pub name: String,
       pub host: String,
       // ... all Server fields ...

       // Related data from JOINs
       pub credential_name: Option<String>,
       pub tag_names: String,  // Comma-separated
   }

   // Query with JOINs
   pub async fn list_servers_with_details(pool: &Pool<Sqlite>) -> Result<Vec<ServerWithDetails>> {
       sqlx::query_as::<_, ServerWithDetails>(r#"
           SELECT
               s.*,
               c.name as credential_name,
               COALESCE(GROUP_CONCAT(t.name, ', '), '') as tag_names
           FROM servers s
           LEFT JOIN credentials c ON s.credential_id = c.id
           LEFT JOIN server_tags st ON s.id = st.server_id
           LEFT JOIN tags t ON st.tag_id = t.id
           GROUP BY s.id
       "#)
       .fetch_all(pool)
       .await
   }
   ```

3. **Specialized Result Structs**:
   - `*WithDetails` - Full object graph (e.g., `ServerWithDetails`)
   - `*WithNames` - IDs replaced with display names (e.g., `JobScheduleWithNames`)
   - `*WithCounts` - Includes aggregate counts (e.g., `JobTemplateWithCounts`)

**Why This Pattern?**:
- **Performance**: Single query instead of N+1 queries
- **Efficiency**: Reduces roundtrips to database
- **Type Safety**: sqlx validates queries at compile time

---

### Layer 2: Core Business Logic (`core/`)

**Responsibilities**:
- Define domain types (`Server`, `JobTemplate`, `NotificationPolicy`, etc.)
- Plugin trait and registry
- Notification system (Gotify, ntfy.sh)
- Remote execution (SSH)
- Error types

**Key Components**:

1. **Plugin Trait** (`plugin.rs`):
   ```rust
   #[async_trait]
   pub trait Plugin: Send + Sync {
       fn metadata(&self) -> PluginMetadata;
       fn scheduled_tasks(&self) -> Vec<ScheduledTask>;
       async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult>;
   }
   ```

2. **Notification System** (`notifications.rs`):
   ```rust
   pub struct NotificationManager {
       client: Arc<Client>,
       configs: HashMap<String, NotificationConfig>,
   }

   impl NotificationManager {
       pub async fn send_for_service(&self, service: &str, msg: &NotificationMessage) -> Result<()>
   }
   ```

3. **Remote Execution** (`remote.rs`):
   ```rust
   pub struct RemoteExecutor {
       ssh_key_path: Option<String>,
   }

   impl RemoteExecutor {
       pub async fn execute(&self, server: &Server, command: &str) -> Result<CommandOutput>
   }
   ```

---

### Layer 3: Scheduler (`scheduler/`)

**Responsibilities**:
- Parse cron expressions
- Schedule and execute tasks
- Manage task lifecycle

**Key API**:
```rust
pub struct Scheduler {
    tasks: HashMap<String, ScheduledTask>,
}

impl Scheduler {
    pub async fn add_task(&mut self, task: ScheduledTask) -> Result<()>
    pub async fn remove_task(&mut self, task_id: &str) -> Result<()>
    pub async fn task_count(&self) -> usize
}
```

---

### Layer 4: Web Server (`server/`)

**Responsibilities**:
- HTTP server (Axum)
- Route handlers (API + UI)
- Template rendering (Askama)
- Static file serving
- Application state management

**Key Components**:

1. **Application State** (`state.rs`):
   ```rust
   pub struct AppState {
       db: Arc<Database>,
       scheduler: Arc<RwLock<Option<Scheduler>>>,
       plugin_registry: Arc<PluginRegistry>,
       config: Arc<Config>,
   }
   ```

2. **Route Handlers** (`routes/ui/*.rs`):
   ```rust
   // Example: server/src/routes/ui/servers.rs
   async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
       let user = get_user_from_session().await;

       // Get data with optimized JOINed query
       let servers = queries::servers::list_servers_with_details(state.db.pool()).await?;

       // Convert to display models
       let server_displays: Vec<ServerDisplay> = servers.into_iter().map(Into::into).collect();

       // Render template
       let template = ServersTemplate { user, servers: server_displays };
       Ok(Html(template.render()?))
   }
   ```

3. **Display Models** (`templates.rs`):
   ```rust
   /// Display model for UI (derived from database model)
   #[derive(Debug, Clone)]
   pub struct ServerDisplay {
       pub id: i64,
       pub name: String,
       pub host: String,
       pub credential_name: String,  // Populated from JOIN
       pub tags: Vec<String>,        // Parsed from comma-separated
       // ... computed fields ...
   }

   // Conversion from database model
   impl From<ServerWithDetails> for ServerDisplay {
       fn from(s: ServerWithDetails) -> Self {
           ServerDisplay {
               id: s.id,
               name: s.name,
               host: s.host,
               credential_name: s.credential_name.unwrap_or_else(|| "(None)".to_string()),
               tags: s.tag_names.split(", ").filter(|t| !t.is_empty()).map(String::from).collect(),
               // ...
           }
       }
   }
   ```

---

### Layer 5: Frontend (Templates + HTMX)

**Responsibilities**:
- HTML rendering (Askama templates)
- User interactions (HTMX)
- Client-side state (Alpine.js)
- Styling (CSS)

**Template Structure**:

1. **Base Layout** (`templates/base.html`):
   - HTML skeleton
   - Navigation sidebar
   - Header with user info
   - Content area ({% block content %})
   - Loads HTMX, Alpine.js, Lucide icons

2. **Page Templates** (`templates/pages/*.html`):
   - Extend base layout
   - Define full page structure
   - Set active navigation item
   - Load page-specific data

3. **Component Templates** (`templates/components/*.html`):
   - Reusable partial templates
   - Rendered by HTMX requests
   - Enable partial page updates
   - Examples: lists, forms, cards

---

## Frontend-Backend Communication

### HTMX Request-Response Patterns

SvrCtlRS uses **HTML-over-the-wire** architecture. The server renders HTML, and HTMX swaps it into the DOM.

#### Pattern 1: Initial Page Load

```
Browser                  Server (Axum)               Database
   |                          |                          |
   |-- GET /servers --------->|                          |
   |                          |-- list_servers_with_ --->|
   |                          |   details()              |
   |                          |<-------------------------|
   |                          |                          |
   |                          |-- Convert to Display --->|
   |                          |   models (From trait)    |
   |                          |                          |
   |                          |-- Render Askama -------->|
   |                          |   template               |
   |<-- HTML (full page) -----|                          |
   |                          |                          |
```

**Code Flow**:
1. User navigates to `/servers`
2. Axum routes to `servers_page()` handler in `server/src/routes/ui/servers.rs`
3. Handler queries database with `list_servers_with_details()`
4. Database returns `Vec<ServerWithDetails>` (JOINed query result)
5. Handler converts to `Vec<ServerDisplay>` using `From` trait
6. Handler renders `ServersTemplate` with Askama
7. Full HTML page returned to browser

---

#### Pattern 2: Form Submission (Create)

```
Browser                  Server (Axum)               Database
   |                          |                          |
   |-- POST /servers -------->|                          |
   |   (form data)            |                          |
   |                          |-- create_server() ------>|
   |                          |<-- new server ID --------|
   |                          |                          |
   |                          |-- list_servers_with_ --->|
   |                          |   details()              |
   |                          |<-------------------------|
   |                          |                          |
   |                          |-- Render component ----->|
   |                          |   template               |
   |<-- HTML (server list)----|                          |
   |                          |                          |
   |-- Swap into #server-list |                          |
   |                          |                          |
```

**HTMX Attributes**:
```html
<form hx-post="/servers"
      hx-target="#server-list"
      hx-swap="innerHTML">
    <input type="text" name="name" required>
    <input type="text" name="host" required>
    <button type="submit">Add Server</button>
</form>
```

**Handler Code**:
```rust
async fn create_server(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,
) -> Result<Html<String>, AppError> {
    // Validate and create
    queries::servers::create_server(state.db.pool(), &input.into()).await?;

    // Get updated list
    let servers = queries::servers::list_servers_with_details(state.db.pool()).await?;
    let displays: Vec<ServerDisplay> = servers.into_iter().map(Into::into).collect();

    // Render just the list component
    let template = ServerListComponent { servers: displays };
    Ok(Html(template.render()?))
}
```

**What Happens**:
1. User submits form
2. HTMX sends POST to `/servers` with form data
3. Handler creates server in database
4. Handler queries updated server list
5. Handler renders **only** the `ServerListComponent` template
6. HTMX swaps the HTML into `#server-list` div
7. **No page reload** - seamless update

---

#### Pattern 3: Delete with Confirmation

```
Browser                  Server (Axum)               Database
   |                          |                          |
   |-- User clicks Delete --->|                          |
   |<-- hx-confirm dialog-----|                          |
   |   "Delete server X?"     |                          |
   |                          |                          |
   |-- User confirms -------->|                          |
   |                          |                          |
   |-- DELETE /servers/5 ---->|                          |
   |                          |-- delete_server(5) ----->|
   |                          |<-- OK -------------------|
   |                          |                          |
   |<-- Empty response -------|                          |
   |   (200 OK)               |                          |
   |                          |                          |
   |-- Remove element from -->|                          |
   |   DOM (outerHTML swap)   |                          |
```

**HTMX Attributes**:
```html
<button hx-delete="/servers/{{ server.id }}"
        hx-target="#server-{{ server.id }}"
        hx-swap="outerHTML"
        hx-confirm="Delete {{ server.name }}?">
    Delete
</button>
```

**Handler Code**:
```rust
async fn delete_server(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    queries::servers::delete_server(state.db.pool(), id).await?;
    Ok(Html(String::new()))  // Empty response removes element
}
```

---

#### Pattern 4: Auto-Refresh

```
Browser                  Server (Axum)               Database
   |                          |                          |
   |-- Initial load --------->|                          |
   |<-- HTML with polling-----|                          |
   |                          |                          |
   |   ... 5 seconds pass ... |                          |
   |                          |                          |
   |-- GET /tasks/list ------>|                          |
   |                          |-- list_tasks() --------->|
   |                          |<-------------------------|
   |<-- Updated HTML ---------|                          |
   |-- Swap content --------->|                          |
   |                          |                          |
   |   ... 5 seconds pass ... |                          |
   |                          |                          |
   |-- GET /tasks/list ------>|                          |
   |   (repeat)               |                          |
```

**HTMX Attributes**:
```html
<div id="task-list"
     hx-get="/tasks/list"
     hx-trigger="every 5s"
     hx-swap="innerHTML">
    {% include "components/task_list.html" %}
</div>
```

**What Happens**:
1. Initial page load includes task list
2. HTMX polls `/tasks/list` every 5 seconds
3. Server renders fresh task list component
4. HTMX swaps updated HTML into `#task-list`
5. **Live updates** without manual refresh

---

#### Pattern 5: Inline Form (Edit in Place)

```html
<!-- Display mode -->
<div id="schedule-{{ schedule.id }}-display">
    <span>{{ schedule.name }}</span>
    <button hx-get="/schedules/{{ schedule.id }}/edit"
            hx-target="#schedule-{{ schedule.id }}-display"
            hx-swap="outerHTML">
        Edit
    </button>
</div>

<!-- Edit mode (returned by GET /schedules/5/edit) -->
<div id="schedule-{{ schedule.id }}-display">
    <form hx-post="/schedules/{{ schedule.id }}"
          hx-target="#schedule-{{ schedule.id }}-display"
          hx-swap="outerHTML">
        <input name="name" value="{{ schedule.name }}">
        <button type="submit">Save</button>
        <button hx-get="/schedules/{{ schedule.id }}/cancel"
                hx-target="#schedule-{{ schedule.id }}-display"
                hx-swap="outerHTML">
            Cancel
        </button>
    </form>
</div>
```

**Flow**:
1. User clicks "Edit" button
2. HTMX requests edit form from server
3. Server renders form with current values
4. HTMX swaps form in place of display
5. User edits and submits
6. Server saves and returns updated display mode
7. HTMX swaps display back in place

---

### Alpine.js Usage (Client-Side State)

Alpine.js handles **minimal client-side state** (UI-only concerns):

```html
<body x-data="{
    sidebarOpen: false,
    theme: localStorage.getItem('theme') || 'dark'
}">
    <!-- Mobile menu toggle -->
    <button @click="sidebarOpen = !sidebarOpen" class="md:hidden">
        ☰
    </button>

    <!-- Sidebar with conditional class -->
    <aside :class="{ 'open': sidebarOpen }" class="sidebar">
        <!-- Navigation -->
    </aside>

    <!-- Theme toggle -->
    <button @click="theme = (theme === 'light' ? 'dark' : 'light');
                     localStorage.setItem('theme', theme)"
            :class="theme">
        <span x-show="theme === 'light'">🌙</span>
        <span x-show="theme === 'dark'">☀️</span>
    </button>
</body>
```

**Key Points**:
- Alpine manages **only UI state** (sidebar open/closed, theme preference)
- **No application data** in Alpine (servers, schedules, etc.)
- All data comes from server via HTMX

---

## Database Architecture

### Schema Overview

```sql
-- Core entities
servers             -- Servers to monitor/control
credentials         -- SSH credentials for remote access
tags                -- Categorization tags
server_tags         -- Many-to-many: servers <-> tags

-- Job system
job_types           -- Categories of jobs (backup, update, health, etc.)
job_templates       -- Reusable job definitions with steps
job_template_steps  -- Steps within a job template
job_schedules       -- Scheduled executions of job templates
job_runs            -- Execution history

-- Notifications
notification_channels         -- Gotify/ntfy.sh endpoints
notification_policies         -- Rules for when to notify
notification_policy_channels  -- Many-to-many: policies <-> channels
notification_policy_job_templates  -- Many-to-many: policies <-> job templates

-- Plugin system
plugin_tasks        -- Tasks registered by plugins
```

### Optimized Query Patterns

#### Example 1: Servers with Details

**Database Query**:
```sql
SELECT
    s.*,                                              -- All server columns
    c.name as credential_name,                        -- JOIN credentials
    COALESCE(GROUP_CONCAT(t.name, ', '), '') as tag_names  -- Aggregate tags
FROM servers s
LEFT JOIN credentials c ON s.credential_id = c.id
LEFT JOIN server_tags st ON s.id = st.server_id
LEFT JOIN tags t ON st.tag_id = t.id
GROUP BY s.id
ORDER BY s.name
```

**Result Struct**:
```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServerWithDetails {
    // All columns from servers table
    pub id: i64,
    pub name: String,
    pub host: String,
    pub location: String,
    pub is_local: bool,
    pub credential_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // JOINed data
    pub credential_name: Option<String>,  // From credentials table
    pub tag_names: String,                // Aggregated from tags table
}
```

**Benefits**:
- **1 query** instead of `1 + N (credentials) + N (tags)`
- All data loaded in single roundtrip
- Type-safe with sqlx compile-time checking

---

#### Example 2: Job Templates with Step Counts

**Database Query**:
```sql
SELECT
    jt.*,
    COUNT(DISTINCT jts.id) as step_count,              -- Count steps
    COUNT(DISTINCT js.id) as schedule_count,           -- Count schedules
    GROUP_CONCAT(DISTINCT s.name, ', ') as server_names  -- Server names
FROM job_templates jt
LEFT JOIN job_template_steps jts ON jt.id = jts.job_template_id
LEFT JOIN job_schedules js ON jt.id = js.job_template_id
LEFT JOIN servers s ON INSTR(jt.server_ids, s.id) > 0
GROUP BY jt.id
ORDER BY jt.display_name
```

**Result Struct**:
```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobTemplateWithCounts {
    // All columns from job_templates table
    pub id: i64,
    pub display_name: String,
    pub job_type_id: i64,
    pub server_ids: String,  // Comma-separated IDs
    // ... other fields ...

    // Aggregated counts
    pub step_count: i64,
    pub schedule_count: i64,
    pub server_names: Option<String>,  // Comma-separated names
}
```

---

#### Example 3: Job Schedules with Names and Counts

**Database Query**:
```sql
SELECT
    js.*,
    jt.display_name as template_name,
    GROUP_CONCAT(DISTINCT s.name, ', ') as server_names,
    (SELECT COUNT(*) FROM job_runs jr
     WHERE jr.job_schedule_id = js.id AND jr.status_str = 'success') as success_count,
    (SELECT COUNT(*) FROM job_runs jr
     WHERE jr.job_schedule_id = js.id AND jr.status_str = 'failed') as failure_count
FROM job_schedules js
JOIN job_templates jt ON js.job_template_id = jt.id
LEFT JOIN servers s ON INSTR(jt.server_ids, s.id) > 0
GROUP BY js.id
ORDER BY js.enabled DESC, jt.display_name
```

**Result Struct**:
```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobScheduleWithNames {
    // All columns from job_schedules table
    pub id: i64,
    pub job_template_id: i64,
    pub cron_expression: String,
    pub enabled: bool,
    // ... other fields ...

    // JOINed/computed data
    pub template_name: String,        // From job_templates
    pub server_names: Option<String>, // From servers (aggregated)
    pub success_count: i64,           // Subquery count
    pub failure_count: i64,           // Subquery count
}
```

---

### Migration System

**Location**: `database/migrations/`

**Format**:
```
001_initial_schema.sql
002_add_credentials.sql
003_add_tags.sql
...
```

**Execution**:
- Migrations run automatically on server startup
- sqlx tracks applied migrations in `_sqlx_migrations` table
- Idempotent (safe to run multiple times)

**Example Migration**:
```sql
-- 001_initial_schema.sql
CREATE TABLE IF NOT EXISTS servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    host TEXT NOT NULL,
    location TEXT NOT NULL DEFAULT '',
    is_local BOOLEAN NOT NULL DEFAULT 0,
    credential_id INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (credential_id) REFERENCES credentials(id)
);

CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
CREATE INDEX IF NOT EXISTS idx_servers_credential_id ON servers(credential_id);
```

---

## Display Model Pattern

### Why Display Models?

**Problem**: Database models aren't always ideal for UI rendering.

**Example Issues**:
- Foreign keys (IDs) instead of names
- Comma-separated strings need parsing
- Need computed fields (e.g., "Next run: in 2 hours")
- Want to hide internal fields from templates

**Solution**: Separate **database models** from **display models**.

---

### Pattern Implementation

#### Step 1: Database Model (from query)

```rust
// database/src/queries/servers.rs
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServerWithDetails {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub credential_id: Option<i64>,
    pub credential_name: Option<String>,  // From JOIN
    pub tag_names: String,                // Comma-separated
    // ... other fields ...
}
```

---

#### Step 2: Display Model (for UI)

```rust
// server/src/templates.rs
#[derive(Debug, Clone)]
pub struct ServerDisplay {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub location: String,

    // Transformed fields
    pub credential_name: String,  // Never Option (default: "(None)")
    pub tags: Vec<String>,        // Parsed from comma-separated

    // Computed fields
    pub display_location: String, // "Location" or "(Not set)"
    pub is_local: bool,
}
```

---

#### Step 3: Conversion (From trait)

```rust
// server/src/templates.rs
impl From<ServerWithDetails> for ServerDisplay {
    fn from(s: ServerWithDetails) -> Self {
        ServerDisplay {
            id: s.id,
            name: s.name.clone(),
            host: s.host.clone(),
            location: s.location.clone(),

            // Transform Option to String with default
            credential_name: s.credential_name.unwrap_or_else(|| "(None)".to_string()),

            // Parse comma-separated tags
            tags: s.tag_names
                .split(", ")
                .filter(|t| !t.is_empty())
                .map(String::from)
                .collect(),

            // Compute display fields
            display_location: if s.location.is_empty() {
                "(Not set)".to_string()
            } else {
                s.location.clone()
            },
            is_local: s.is_local,
        }
    }
}
```

---

#### Step 4: Usage in Handler

```rust
// server/src/routes/ui/servers.rs
async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;

    // Query database (returns ServerWithDetails)
    let servers_db = queries::servers::list_servers_with_details(state.db.pool()).await?;

    // Convert to display models
    let servers: Vec<ServerDisplay> = servers_db.into_iter().map(Into::into).collect();

    // Render template
    let template = ServersTemplate { user, servers };
    Ok(Html(template.render()?))
}
```

---

#### Step 5: Template Usage

```html
<!-- server/templates/pages/servers.html -->
{% for server in servers %}
<div class="server-card">
    <h3>{{ server.name }}</h3>
    <p>Host: {{ server.host }}</p>
    <p>Location: {{ server.display_location }}</p>
    <p>Credential: {{ server.credential_name }}</p>

    <!-- Tags (already parsed into Vec) -->
    <div class="tags">
        {% for tag in server.tags %}
        <span class="badge">{{ tag }}</span>
        {% endfor %}
    </div>
</div>
{% endfor %}
```

---

### Current Display Models

| Display Model | Source Database Model | Key Transformations |
|--------------|----------------------|---------------------|
| `ServerDisplay` | `ServerWithDetails` | Parse tags, default credential name |
| `JobTemplateDisplay` | `JobTemplateWithCounts` | Parse server IDs, format counts |
| `JobScheduleDisplay` | `JobScheduleWithNames` | Compute next run time, format counts |
| `JobRunDisplay` | `JobRun` | Resolve server/template names, format duration |
| `NotificationPolicyDisplay` | `NotificationPolicyWithDetails` | Populate policy_channels, format scope |
| `CredentialDisplay` | `Credential` | (Minimal transformation) |
| `TagDisplay` | `Tag` | (Minimal transformation) |

---

## Data Flow Diagrams

### Complete Request Flow (Server List Example)

```
┌─────────┐
│ Browser │
└────┬────┘
     │
     │ 1. GET /servers
     │
     ▼
┌─────────────────┐
│ Axum Router     │
│ (server/main.rs)│
└────┬────────────┘
     │
     │ 2. Route to servers_page()
     │
     ▼
┌──────────────────────────────┐
│ UI Handler                   │
│ (routes/ui/servers.rs)       │
│                              │
│ async fn servers_page(       │
│     State(state): State<...> │
│ ) -> Result<Html<String>>    │
└────┬─────────────────────────┘
     │
     │ 3. Get user from session
     │
     ▼
┌──────────────────────────┐
│ get_user_from_session()  │
│ (Hardcoded for now)      │
└────┬─────────────────────┘
     │
     │ 4. Return User { name: "admin" }
     │
     ▼
┌──────────────────────────────┐
│ Database Query               │
│ (database/queries/servers.rs)│
│                              │
│ list_servers_with_details()  │
└────┬─────────────────────────┘
     │
     │ 5. Execute JOINed SQL query
     │
     ▼
┌────────────────────────┐
│ SQLite Database        │
│                        │
│ SELECT s.*,            │
│   c.name as cred_name, │
│   GROUP_CONCAT(...)    │
│ FROM servers s         │
│ LEFT JOIN credentials c│
│ LEFT JOIN server_tags  │
│ ...                    │
└────┬───────────────────┘
     │
     │ 6. Return Vec<ServerWithDetails>
     │
     ▼
┌──────────────────────────────┐
│ Display Model Conversion     │
│ (server/templates.rs)        │
│                              │
│ impl From<ServerWithDetails> │
│     for ServerDisplay        │
└────┬─────────────────────────┘
     │
     │ 7. Convert each ServerWithDetails
     │    to ServerDisplay
     │    (parse tags, default values, etc.)
     │
     ▼
┌──────────────────────────────┐
│ Template Rendering           │
│ (server/templates.rs)        │
│                              │
│ ServersTemplate {            │
│     user,                    │
│     servers: Vec<Display>    │
│ }                            │
└────┬─────────────────────────┘
     │
     │ 8. Askama renders template
     │    (server/templates/pages/servers.html)
     │
     ▼
┌─────────────────────────┐
│ HTML String             │
│                         │
│ <!DOCTYPE html>         │
│ <html>                  │
│   <body>                │
│     <div class="card">  │
│       Server 1          │
│     </div>              │
│   </body>               │
│ </html>                 │
└────┬────────────────────┘
     │
     │ 9. Return Html(rendered_html)
     │
     ▼
┌─────────────────┐
│ Axum Response   │
│ HTTP 200 OK     │
│ Content-Type:   │
│   text/html     │
└────┬────────────┘
     │
     │ 10. Send to browser
     │
     ▼
┌─────────┐
│ Browser │
│ Renders │
│ HTML    │
└─────────┘
```

---

### HTMX Partial Update Flow (Delete Server Example)

```
┌─────────┐
│ Browser │
└────┬────┘
     │
     │ User clicks Delete button with:
     │ hx-delete="/servers/5"
     │ hx-target="#server-5"
     │ hx-swap="outerHTML"
     │ hx-confirm="Delete server?"
     │
     ▼
┌──────────────────┐
│ HTMX Intercepts  │
│ - Shows confirm  │
│ - User clicks OK │
└────┬─────────────┘
     │
     │ DELETE /servers/5
     │
     ▼
┌─────────────────┐
│ Axum Router     │
└────┬────────────┘
     │
     │ Route to delete_server()
     │
     ▼
┌──────────────────────────────┐
│ UI Handler                   │
│ async fn delete_server(      │
│     State(state),            │
│     Path(id): Path<i64>      │
│ )                            │
└────┬─────────────────────────┘
     │
     │ Extract id = 5
     │
     ▼
┌──────────────────────────┐
│ Database Query           │
│ delete_server(pool, 5)   │
└────┬─────────────────────┘
     │
     │ DELETE FROM servers WHERE id = 5
     │
     ▼
┌────────────────┐
│ SQLite DB      │
│ Row deleted    │
└────┬───────────┘
     │
     │ OK
     │
     ▼
┌──────────────────────────┐
│ Handler returns          │
│ Ok(Html(String::new()))  │
│ (Empty response)         │
└────┬─────────────────────┘
     │
     │ HTTP 200 OK
     │ Content-Type: text/html
     │ Body: (empty)
     │
     ▼
┌──────────────────┐
│ HTMX Receives    │
│ - Swap mode:     │
│   "outerHTML"    │
│ - Target:        │
│   "#server-5"    │
│ - Content: empty │
└────┬─────────────┘
     │
     │ Remove element from DOM
     │
     ▼
┌─────────────────┐
│ Browser         │
│ Element removed │
│ (No page reload)│
└─────────────────┘
```

---

### Plugin Execution Flow

```
┌──────────────┐
│ Scheduler    │
│ (Tokio task) │
└────┬─────────┘
     │
     │ Every minute: check cron expressions
     │
     ▼
┌────────────────────┐
│ Task due to run?   │
│ (e.g., "check_    │
│  docker_health")   │
└────┬───────────────┘
     │
     │ Yes - task is due
     │
     ▼
┌──────────────────────────┐
│ Plugin Registry          │
│ get_plugin("docker")     │
└────┬─────────────────────┘
     │
     │ Return Arc<DockerPlugin>
     │
     ▼
┌──────────────────────────────┐
│ Build Plugin Context         │
│ - db_pool                    │
│ - notification_manager       │
│ - config                     │
└────┬─────────────────────────┘
     │
     │ Call plugin.execute("check_docker_health", context)
     │
     ▼
┌──────────────────────────┐
│ DockerPlugin::execute()  │
└────┬─────────────────────┘
     │
     │ Match task_id
     │
     ▼
┌────────────────────────────────┐
│ check_containers()             │
│ - Query Docker API             │
│ - Check container health       │
│ - Identify issues              │
└────┬───────────────────────────┘
     │
     │ Found 2 unhealthy containers
     │
     ▼
┌──────────────────────────────────┐
│ Send Notification                │
│ context.notification_manager     │
│   .send_for_service("docker", ..)│
└────┬─────────────────────────────┘
     │
     │ Look up "docker" notification config
     │
     ▼
┌────────────────────────────┐
│ NotificationManager        │
│ - Get DOCKER_GOTIFY_KEY    │
│ - Get DOCKER_NTFY_TOPIC    │
└────┬───────────────────────┘
     │
     │ Send to Gotify and ntfy.sh
     │
     ▼
┌─────────────────────────┐
│ HTTP POST to:           │
│ - Gotify server         │
│ - ntfy.sh server        │
└────┬────────────────────┘
     │
     │ Notifications sent
     │
     ▼
┌─────────────────────────┐
│ Return PluginResult     │
│ - status: "error"       │
│ - message: "2 issues"   │
└────┬────────────────────┘
     │
     │ Log result
     │
     ▼
┌─────────────────────────┐
│ Scheduler               │
│ - Save to job_runs      │
│ - Update success/fail   │
│   counts                │
└─────────────────────────┘
```

---

## Deprecated Elements Review

### ✅ No Deprecated Code Found

**Review Date**: 2025-11-30

#### Areas Checked:

1. **Display Model From Implementations** ✅
   - All TODO comments have been resolved
   - `JobScheduleDisplay` now uses actual `success_count` and `failure_count` from database
   - `NotificationPolicyDisplay` legacy fields (`channel_id`, `channel_name`) marked as deprecated with clear comments
   - Multi-channel support properly implemented with `policy_channels` field

2. **Database Queries** ✅
   - All optimized JOINed queries in use
   - No redundant N+1 query patterns found
   - Specialized result structs (`*WithDetails`, `*WithNames`, `*WithCounts`) consistently used
   - Old query functions removed in favor of optimized versions

3. **Templates** ✅
   - All component templates updated to use optimized display models
   - `notification_policy_list.html` uses `policy_channels` instead of legacy single `channel_name`
   - No references to deprecated fields in active templates

4. **Route Handlers** ✅
   - All handlers use optimized query functions
   - Display model conversions use `From` trait consistently
   - `get_policies_list()` properly populates `policy_channels` for each policy

5. **Backward Compatibility Code** ✅
   - No unnecessary backward compatibility shims
   - No unused re-exports or type aliases
   - No `_unused` variable patterns or `// removed` comments

#### Migration Cleanup Status:

**Dioxus → HTMX Migration**: ✅ Complete
- No Dioxus dependencies in `Cargo.toml`
- No `.rs` component files in old structure
- All UI is now Askama templates + HTMX
- `server/static/` contains only HTMX, Alpine.js, and CSS

#### Potential Future Improvements:

1. **NotificationPolicyDisplay Legacy Fields**:
   - Current: Marked as legacy with comments
   - Future: Could be removed entirely if confirmed unused
   - Location: `server/src/templates.rs:1707-1708`
   - **Recommendation**: Monitor for 1-2 releases, then remove

2. **User Authentication**:
   - Current: `get_user_from_session()` returns hardcoded `User { name: "admin" }`
   - Location: `server/src/routes/ui/mod.rs`
   - **Not deprecated**: This is a placeholder for future auth implementation
   - **Recommendation**: Implement proper session-based auth when multi-user support is needed

---

## Summary

### Key Architectural Strengths

1. **Clear Layer Separation**:
   - Database layer handles persistence
   - Core handles business logic
   - Server handles HTTP/UI
   - Plugins extend functionality

2. **Optimized Data Flow**:
   - JOINed queries reduce roundtrips
   - Display models separate concerns
   - HTMX enables partial updates

3. **Type Safety**:
   - sqlx compile-time query checking
   - Strong typing throughout
   - `From` trait for safe conversions

4. **Maintainability**:
   - Consistent patterns across codebase
   - Well-structured templates
   - Clear naming conventions

### Technology Benefits

| Technology | Benefit |
|-----------|---------|
| **Axum** | Fast, type-safe routing |
| **HTMX** | Simple interactivity without heavy JavaScript |
| **Askama** | Compiled templates (no runtime parsing) |
| **sqlx** | Compile-time SQL validation |
| **Tokio** | Efficient async runtime |
| **Alpine.js** | Minimal client-side state management |

### Communication Patterns Summary

| Pattern | Use Case | Example |
|---------|----------|---------|
| **Initial Page Load** | Full page render | Dashboard, server list |
| **Form Submission** | Create/update entity | Add server, edit schedule |
| **Partial Update** | Refresh component | Task list auto-refresh |
| **Delete** | Remove entity | Delete server (with confirm) |
| **Inline Edit** | Edit in place | Schedule name editing |

---

**Document Status**: ✅ Complete and Current
**Deprecated Elements**: ✅ None Found
**Architecture Health**: ✅ Clean and Consistent

