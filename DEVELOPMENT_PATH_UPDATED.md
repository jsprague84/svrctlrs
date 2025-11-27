# SvrCtlRS Development Path - Updated 2024-11-26

**Assessment Grade**: B+ (85/100) - Production-Ready with Gaps
**Focus**: Multi-Server Management UI Excellence

---

## Current Status Summary

### ✅ Recently Completed (Last 24 Hours)
1. **Fixed ntfy notifications** - Authentication support added
2. **Fixed database errors** - task_history table name corrected
3. **Fixed health plugin** - Returns success when check completes
4. **Fixed Axum 0.8 compatibility** - Path parameter syntax `{id}`

### 🎯 Core Strengths
- Excellent plugin architecture (trait-based, extensible)
- Modern tech stack (Axum 0.8, HTMX 2.0, Askama, SQLx)
- Superior deployment (1 container vs weatherust's 7)
- 95% feature parity with weatherust
- Server-grouped task view already implemented

### 🔴 Critical Gaps (from Comprehensive Assessment)
1. **Security**: No auth, no CSRF, timing attacks, token leakage
2. **UI Organization**: 1042-line route file, basic dashboard
3. **Multi-Server UI**: No grouping, no real-time status, no bulk operations
4. **Testing**: 0% test coverage

---

## Strategic Priority: Multi-Server Management UI

### Why This is Priority #1

**User's Key Insight:**
> "The biggest advantage of this web app is managing many remote servers easily and using the UI for centralized configuration."

**Current Limitation:**
- Servers displayed in simple grid
- No grouping (by environment, tags, status)
- No filtering or sorting
- No real-time status indicators
- No bulk operations
- Doesn't scale visually beyond 10-15 servers

**Goal:**
Build a **best-in-class multi-server management UI** that makes managing 50+ servers as easy as managing 5.

---

## Phase 1: UI Restructuring for Scale (2-3 weeks)

### Week 1: Foundation & Architecture

#### 1.1 Route Organization (Days 1-2)
**Problem**: `ui_routes.rs` is 1042 lines (Assessment: Issue 3)

**Solution**: Split into focused modules

**New Structure:**
```
server/src/routes/
├── ui/
│   ├── mod.rs           # Router assembly
│   ├── dashboard.rs     # Dashboard page (~100 lines)
│   ├── servers.rs       # Server management (~300 lines)
│   ├── tasks.rs         # Task management (already split ✓)
│   ├── plugins.rs       # Plugin config (~150 lines)
│   ├── settings.rs      # Settings (~100 lines)
│   └── auth.rs          # Authentication stubs (~100 lines)
└── api/
    └── ... (existing API routes)
```

**Benefits:**
- Easier to navigate and maintain
- Follows Axum 0.8 best practices
- Matches task route organization (already done)
- Enables parallel development

#### 1.2 Server Data Model Enhancement (Days 2-3)
**Add to database schema:**

```sql
-- Migration 009: Server grouping and status
ALTER TABLE servers ADD COLUMN tags TEXT;  -- JSON: ["production", "database"]
ALTER TABLE servers ADD COLUMN environment TEXT;  -- "production", "staging", "dev"
ALTER TABLE servers ADD COLUMN group_name TEXT;  -- Custom grouping

CREATE TABLE IF NOT EXISTS server_status (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('online', 'offline', 'unknown')),
    last_checked_at TIMESTAMP NOT NULL,
    response_time_ms INTEGER,
    cpu_usage REAL,
    memory_usage REAL,
    disk_usage REAL,
    error_message TEXT,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_server_status_server_id ON server_status(server_id);
CREATE INDEX idx_server_status_last_checked ON server_status(last_checked_at);
```

**Benefits:**
- Foundation for grouping and filtering
- Real-time status tracking
- Historical performance data

#### 1.3 Server Grouping Implementation (Days 3-5)
**Pattern**: Reuse task grouping logic (already implemented in `routes/ui/tasks.rs`)

**Apply to servers:**
```rust
// server/src/routes/ui/servers.rs
use std::collections::HashMap;

pub struct ServerGroup {
    pub group_name: String,
    pub servers: Vec<Server>,
    pub online_count: usize,
    pub total_count: usize,
}

async fn server_list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let servers = get_servers(&state).await?;

    // Group by environment (production, staging, dev)
    let mut server_groups = HashMap::<String, Vec<Server>>::new();
    for server in servers {
        let group = server.environment
            .clone()
            .unwrap_or_else(|| "Uncategorized".to_string());
        server_groups.entry(group).or_default().push(server);
    }

    // Convert to sorted vector
    let mut groups: Vec<ServerGroup> = server_groups
        .into_iter()
        .map(|(name, servers)| {
            let online = servers.iter().filter(|s| s.is_online()).count();
            ServerGroup {
                group_name: name,
                total_count: servers.len(),
                online_count: online,
                servers,
            }
        })
        .collect();

    // Sort: Production first, then alphabetically
    groups.sort_by(|a, b| {
        if a.group_name == "production" { return std::cmp::Ordering::Less; }
        if b.group_name == "production" { return std::cmp::Ordering::Greater; }
        a.group_name.cmp(&b.group_name)
    });

    let template = ServerListTemplate { groups };
    Ok(Html(template.render()?))
}
```

**Template** (accordions with lazy loading):
```html
<!-- templates/components/server_groups.html -->
{% for group in groups %}
<div class="accordion-item" id="group-{{ group.group_name }}">
    <button class="accordion-header"
            hx-get="/servers/group/{{ group.group_name }}/details"
            hx-target="#group-{{ group.group_name }}-content"
            hx-swap="innerHTML"
            hx-trigger="click once">
        <span class="group-title">{{ group.group_name }}</span>
        <span class="badge badge-success">{{ group.online_count }}/{{ group.total_count }} online</span>
        <span class="chevron">▼</span>
    </button>

    <div id="group-{{ group.group_name }}-content" class="accordion-content">
        <!-- Loaded lazily on first click -->
    </div>
</div>
{% endfor %}
```

**Benefits:**
- Handles 50+ servers efficiently
- Familiar UX (matches task grouping)
- Reduces initial page load

### Week 2: Advanced Server Management

#### 2.1 Filtering & Sorting (Days 1-2)
**Implementation** (Query parameters + HTMX):

```html
<!-- Server filters -->
<div class="server-filters"
     hx-get="/servers/list"
     hx-trigger="change from:.filter-input, input changed delay:500ms from:.filter-input"
     hx-target="#server-list"
     hx-push-url="true"
     hx-include=".filter-input">

    <input type="text"
           name="search"
           class="filter-input"
           placeholder="Search servers..."
           value="{{ search }}">

    <select name="environment" class="filter-input">
        <option value="">All Environments</option>
        <option value="production">Production</option>
        <option value="staging">Staging</option>
        <option value="dev">Development</option>
    </select>

    <select name="status" class="filter-input">
        <option value="">All Statuses</option>
        <option value="online">Online</option>
        <option value="offline">Offline</option>
    </select>
</div>

<div id="server-list">
    {% include "components/server_groups.html" %}
</div>
```

**Route handler:**
```rust
#[derive(Debug, Deserialize)]
struct ServerFilters {
    search: Option<String>,
    environment: Option<String>,
    status: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

async fn server_list_filtered(
    State(state): State<AppState>,
    Query(filters): Query<ServerFilters>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let servers = queries::servers::list_servers_filtered(
        db.pool(),
        filters.search.as_deref(),
        filters.environment.as_deref(),
        filters.status.as_deref(),
        filters.sort_by.as_deref().unwrap_or("name"),
        filters.sort_order.as_deref().unwrap_or("asc"),
    ).await?;

    // Group and render...
}
```

**Benefits:**
- Server-side filtering (fast, scalable)
- Bookmarkable URLs (`hx-push-url="true"`)
- Debounced search (500ms delay)

#### 2.2 Real-Time Status Updates (Days 2-3)
**Background status checker:**

```rust
// server/src/status_checker.rs
use tokio::time::{interval, Duration};

pub async fn start_status_checker(state: AppState) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            let db = state.db().await;
            let servers = queries::servers::list_enabled_servers(db.pool())
                .await
                .unwrap_or_default();

            for server in servers {
                tokio::spawn({
                    let state = state.clone();
                    async move {
                        if let Err(e) = check_server_status(&state, server.id).await {
                            tracing::warn!("Failed to check status for {}: {}", server.name, e);
                        }
                    }
                });
            }
        }
    });
}

async fn check_server_status(state: &AppState, server_id: i64) -> Result<()> {
    let start = std::time::Instant::now();

    // Try SSH connection
    let db = state.db().await;
    let server = queries::servers::get_server(db.pool(), server_id).await?;

    let status = match test_ssh_connection(&server).await {
        Ok(_) => ServerStatus {
            status: "online".to_string(),
            response_time_ms: start.elapsed().as_millis() as i32,
            error_message: None,
        },
        Err(e) => ServerStatus {
            status: "offline".to_string(),
            response_time_ms: None,
            error_message: Some(e.to_string()),
        },
    };

    queries::servers::update_server_status(db.pool(), server_id, &status).await?;
    Ok(())
}
```

**UI updates** (polling):
```html
<div id="server-status-indicators"
     hx-get="/servers/status/summary"
     hx-trigger="every 10s"
     hx-swap="innerHTML">
    {% for group in groups %}
    <div class="status-badge status-{{ group.status }}">
        {{ group.group_name }}: {{ group.online_count }}/{{ group.total_count }}
    </div>
    {% endfor %}
</div>
```

**Benefits:**
- Background monitoring (every 30s)
- Visual feedback (every 10s)
- Doesn't block UI

#### 2.3 Bulk Operations (Days 3-4)
**UI with checkboxes:**

```html
<div x-data="{ selectedServers: [] }">
    <div class="bulk-actions" x-show="selectedServers.length > 0">
        <span x-text="selectedServers.length + ' selected'"></span>

        <button hx-post="/servers/bulk/restart"
                hx-include="[name='server_ids']:checked"
                hx-confirm="Restart selected servers?"
                class="btn-warning">
            🔄 Restart Selected
        </button>

        <button hx-post="/servers/bulk/disable"
                hx-include="[name='server_ids']:checked"
                hx-confirm="Disable selected servers?"
                class="btn-danger">
            ⏸️ Disable Selected
        </button>
    </div>

    {% for server in servers %}
    <div class="server-card">
        <input type="checkbox"
               name="server_ids"
               value="{{ server.id }}"
               x-model="selectedServers">

        <h3>{{ server.name }}</h3>
        <span class="badge badge-{{ server.status }}">{{ server.status }}</span>
    </div>
    {% endfor %}
</div>
```

**Route handler:**
```rust
#[derive(Debug, Deserialize)]
struct BulkServerAction {
    server_ids: Vec<i64>,
}

async fn servers_bulk_restart(
    State(state): State<AppState>,
    Form(input): Form<BulkServerAction>,
) -> Result<Html<String>, AppError> {
    tracing::info!("Bulk restart: {} servers", input.server_ids.len());

    // Execute restart tasks
    for server_id in &input.server_ids {
        tokio::spawn({
            let state = state.clone();
            let id = *server_id;
            async move {
                if let Err(e) = execute_restart_tasks(&state, id).await {
                    tracing::error!("Restart failed for server {}: {}", id, e);
                }
            }
        });
    }

    Ok(Html(format!(
        r#"<div class="alert alert-success">
            Restart initiated for {} servers. Check task history for results.
        </div>"#,
        input.server_ids.len()
    )))
}
```

**Benefits:**
- Multi-select with Alpine.js state
- Bulk enable/disable/restart
- Async execution with progress tracking

### Week 3: Dashboard Enhancement

#### 3.1 Real-Time Dashboard (Days 1-3)
**New dashboard page:**

```html
<!-- templates/pages/dashboard.html -->
{% extends "base.html" %}

{% block content %}
<h1>Dashboard</h1>

<!-- System Overview Cards -->
<div class="stats-grid">
    <div class="card" hx-get="/dashboard/stats/servers" hx-trigger="every 10s" hx-swap="innerHTML">
        <h3>Servers</h3>
        <div class="stat-value">{{ servers_online }}/{{ servers_total }}</div>
        <div class="stat-label">Online</div>
    </div>

    <div class="card" hx-get="/dashboard/stats/tasks" hx-trigger="every 10s" hx-swap="innerHTML">
        <h3>Tasks</h3>
        <div class="stat-value">{{ tasks_running }}</div>
        <div class="stat-label">Running</div>
    </div>

    <div class="card" hx-get="/dashboard/stats/plugins" hx-trigger="load" hx-swap="innerHTML">
        <h3>Plugins</h3>
        <div class="stat-value">{{ plugins_enabled }}/{{ plugins_total }}</div>
        <div class="stat-label">Enabled</div>
    </div>
</div>

<!-- Server Status Grid -->
<div class="card">
    <h2>Server Status</h2>
    <div id="server-status-grid"
         hx-get="/dashboard/servers/status"
         hx-trigger="load, every 30s"
         hx-swap="innerHTML">
        <!-- Loaded via HTMX -->
    </div>
</div>

<!-- Recent Activity Feed -->
<div class="card">
    <h2>Recent Activity</h2>
    <div id="activity-feed"
         hx-get="/dashboard/activity"
         hx-trigger="load, every 10s"
         hx-swap="innerHTML">
        <!-- Task executions, config changes, etc. -->
    </div>
</div>

<!-- Quick Actions -->
<div class="card">
    <h2>Quick Actions</h2>
    <div class="quick-actions">
        <button hx-get="/servers/new" hx-target="#modal" class="btn-primary">
            ➕ Add Server
        </button>
        <button hx-get="/tasks/new" hx-target="#modal" class="btn-primary">
            ⚙️ Create Task
        </button>
        <button hx-post="/api/v1/config/reload" hx-target="#reload-result" class="btn-secondary">
            🔄 Reload Config
        </button>
    </div>
</div>
{% endblock %}
```

**Benefits:**
- Real-time stats (auto-updating every 10-30s)
- Server health at a glance
- Activity feed for recent changes
- Quick actions for common tasks

#### 3.2 Server Status Visualization (Days 3-5)
**Color-coded status grid:**

```html
<!-- templates/components/server_status_grid.html -->
<div class="server-status-grid">
    {% for group in server_groups %}
    <div class="status-group">
        <h3>{{ group.name }}</h3>
        <div class="server-grid">
            {% for server in group.servers %}
            <a href="/servers/{{ server.id }}"
               class="server-status-card status-{{ server.status }}"
               title="{{ server.name }} - {{ server.status }}">
                <div class="server-icon">🖥️</div>
                <div class="server-name">{{ server.name }}</div>
                <div class="server-metrics" x-show="server.status === 'online'">
                    <span>CPU: {{ server.cpu_usage }}%</span>
                    <span>Mem: {{ server.memory_usage }}%</span>
                </div>
            </a>
            {% endfor %}
        </div>
    </div>
    {% endfor %}
</div>
```

**CSS (Nord theme):**
```css
.server-status-card {
    border: 2px solid var(--border-color);
    border-radius: 8px;
    padding: 1rem;
    transition: all 0.2s;
}

.server-status-card.status-online {
    border-color: var(--nord14); /* Green */
    background: rgba(163, 190, 140, 0.1);
}

.server-status-card.status-offline {
    border-color: var(--nord11); /* Red */
    background: rgba(191, 97, 106, 0.1);
}

.server-status-card.status-unknown {
    border-color: var(--nord13); /* Yellow */
    background: rgba(235, 203, 139, 0.1);
}

.server-status-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 8px rgba(0,0,0,0.1);
}
```

**Benefits:**
- Visual server health overview
- Color-coded status (green/red/yellow)
- Grouped by environment
- Click to see details

---

## Phase 2: Security Hardening (1-2 weeks)

**Note**: Security is critical but doesn't block multi-server UI development. Can be done in parallel.

### Week 4: Authentication & CSRF

#### 4.1 Authentication System
- Add `tower-sessions` with SQLite store
- User table with bcrypt password hashing
- Login/logout UI
- Protected route middleware

#### 4.2 CSRF Protection
- Add `tower-csrf` middleware
- Update all forms with CSRF tokens
- Validate on POST/PUT/DELETE

#### 4.3 Security Patterns from Weatherust
- Constant-time token comparison (`subtle` crate)
- Token masking in logs
- File-based secret management (`_FILE` suffix)

---

## Phase 3: Feature Completeness (2-3 weeks)

### Week 5-6: Webhook API & Metrics

#### 5.1 Webhook System
- Incoming webhooks for remote triggers
- Action buttons in ntfy notifications
- Webhook authentication with constant-time comparison

#### 5.2 Metrics Visualization
- Chart.js integration
- CPU/memory/disk trends over time
- Task execution history charts
- Server performance comparisons

### Week 7: Polish & Testing

#### 7.1 Testing
- Unit tests for plugins (70% coverage target)
- Integration tests for routes
- `cargo clippy` and `cargo audit` in CI

#### 7.2 Performance
- Database query optimization
- Add indexes for filters
- Consider Redis for caching (if needed at scale)

---

## Success Metrics

### Phase 1 (UI Restructuring)
- [ ] Manage 50+ servers comfortably
- [ ] Group servers by environment/tags
- [ ] Filter & sort in <500ms
- [ ] Real-time status updates (30s background, 10s UI)
- [ ] Bulk operations (restart, enable, disable)
- [ ] Professional dashboard with metrics

### Phase 2 (Security)
- [ ] Authentication working
- [ ] CSRF protection on all forms
- [ ] No timing attack vulnerabilities
- [ ] Secrets not leaked in logs

### Phase 3 (Features)
- [ ] Webhook API functional
- [ ] Metrics charts rendering
- [ ] 70% test coverage
- [ ] All clippy warnings resolved

---

## Implementation Priority

**Week 1** (Starting Now):
1. Split `ui_routes.rs` into modules
2. Add server grouping database schema
3. Implement server grouping UI (reuse task pattern)

**Week 2**:
1. Add filtering & sorting
2. Background status checker
3. Bulk operations

**Week 3**:
1. Enhanced dashboard
2. Server status visualization
3. Activity feed

**Weeks 4-7**: Security + Features + Testing

---

## Key Design Decisions

### 1. Server Grouping Strategy
**Decision**: Group by environment first (production/staging/dev), then by tags

**Rationale**:
- Matches common infrastructure organization
- Production servers need special handling (warnings before bulk operations)
- Tags provide flexibility for custom grouping

### 2. Real-Time Updates Strategy
**Decision**: Background checker (30s) + UI polling (10s), not WebSocket/SSE yet

**Rationale**:
- Simpler to implement and maintain
- Sufficient for 50-100 servers
- Can upgrade to SSE later if needed (See: HTMX SSE extension)

### 3. Route Organization Strategy
**Decision**: Split by feature (servers, tasks, plugins), not by HTTP method

**Rationale**:
- Easier to find related code
- Matches Axum 0.8 best practices
- Already done for tasks (successful pattern)

### 4. Filtering Strategy
**Decision**: Server-side filtering with query parameters

**Rationale**:
- Scales to any number of servers
- Bookmarkable URLs
- Fast with proper database indexes

---

## Dependencies to Add

```toml
# server/Cargo.toml
[dependencies]
# Authentication (Phase 2)
tower-sessions = "0.13"
tower-sessions-sqlx-store = "0.13"
argon2 = "0.5"

# CSRF Protection (Phase 2)
tower-csrf = "0.1"

# Security
subtle = "2.6"  # Constant-time comparison

# Existing dependencies (confirm versions)
axum = "0.8"
htmx = "2.0"
askama = { version = "0.14", features = ["with-axum"] }  # Upgrade from 0.12
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-rustls", "chrono"] }
```

---

## Migration from Askama 0.12 → 0.14

**Current**: `askama = "0.12"`
**Target**: `askama = "0.14"`

**Breaking Changes** ([Askama Changelog](https://github.com/djc/askama/blob/main/CHANGELOG.md)):
- Update template syntax if using deprecated features
- Test all templates after upgrade

**Benefits**:
- Performance improvements
- Better error messages
- Latest features

---

## Documentation Updates Needed

1. **Update CLAUDE.md**: Reflect new route structure
2. **Update IMMEDIATE_PRIORITIES.md**: Mark UI restructuring as in-progress
3. **Create MULTI_SERVER_UI_GUIDE.md**: Document grouping, filtering, bulk operations
4. **Update README.md**: Mention improved multi-server management

---

## Next Steps (Immediate)

**Today (2024-11-26)**:
1. Create migration 009: Server grouping & status tables
2. Split `ui_routes.rs` into `routes/ui/servers.rs`
3. Implement basic server grouping (copy task grouping pattern)

**Tomorrow**:
1. Add server status checker background task
2. Update server list template with groups
3. Test with 10+ test servers

**This Week**:
1. Complete server grouping UI
2. Add filtering controls
3. Implement bulk operations

---

**Last Updated**: 2024-11-26 23:00
**Status**: Phase 1 starting - UI Restructuring for Multi-Server Management
**Next Review**: After Week 1 completion
