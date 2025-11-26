# SvrCtlRS Development Reference

**Version**: 2.1.0
**Last Updated**: 2024-11-26
**Assessment Grade**: B+ (85/100) - Production-Ready with Security Gaps

This document provides a comprehensive reference for developing and improving SvrCtlRS, based on a full codebase assessment, weatherust comparison, and 2025 best practices analysis.

---

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Technology Stack Status](#technology-stack-status)
3. [Critical Security Issues](#critical-security-issues)
4. [Architecture & Code Quality](#architecture--code-quality)
5. [UI Organization Needs](#ui-organization-needs)
6. [Weatherust Feature Parity](#weatherust-feature-parity)
7. [Development Roadmap](#development-roadmap)
8. [File Reference Guide](#file-reference-guide)
9. [Testing Requirements](#testing-requirements)
10. [Best Practices Checklist](#best-practices-checklist)

---

## Quick Reference

### Project Status

| Aspect | Status | Grade |
|--------|--------|-------|
| **Core Architecture** | ✅ Excellent | A+ |
| **Technology Stack** | ✅ Current (2025) | A |
| **Security** | 🔴 Critical Gaps | C |
| **UI Organization** | 🟡 Needs Work | B- |
| **Feature Completeness** | 🟡 95% Parity | B+ |
| **Testing** | 🔴 Minimal Coverage | D |
| **Documentation** | ✅ Comprehensive | A |

### Deployment Status

- **Current**: ⚠️ Soft Production (internal use only)
- **After Security Fixes**: ✅ Production-Ready
- **After UI Improvements**: ✅ Polished Platform
- **After Testing**: ⭐ Enterprise-Grade

---

## Technology Stack Status

### Current Versions (as of 2024-11-26)

| Technology | Current | Latest (2025) | Status | Action |
|-----------|---------|---------------|---------|--------|
| **Axum** | 0.8.x | 0.8.6 | ✅ Current | None |
| **HTMX** | 2.0.3 | 2.0.x | ✅ Current | None |
| **Askama** | 0.12.x | 0.14.0 | ⚠️ Outdated | Upgrade recommended |
| **Alpine.js** | 3.14.1 | 3.14.x | ✅ Current | None |
| **Tokio** | Latest | Latest | ✅ Current | None |
| **sqlx** | Latest | Latest | ✅ Current | None |
| **Bollard** | Latest | Latest | ✅ Current | None |

### Compliance with 2025 Best Practices

#### Axum 0.8 Patterns ✅
- [x] New path syntax: `/{id}` instead of `/:id`
- [x] `axum::serve(listener, app)` pattern
- [x] `Option<T>` extractor with `OptionalFromRequestParts`
- [x] Tower middleware integration
- [x] Clean architecture separation

**Sources**:
- [Tokio Axum 0.8 Announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0)
- [Axum Complete 2025 Guide](https://medium.com/rustaceans/rust-powered-apis-with-axum-a-complete-2025-guide-213a28bb44ac)

#### HTMX 2.0 Patterns ✅
- [x] Server-driven interactivity
- [x] HTML fragment swapping
- [x] `hx-get`, `hx-post`, `hx-swap` properly used
- [ ] `hx-confirm` for destructive actions (partially)
- [ ] CSRF protection (missing)
- [x] Auto-refresh with `hx-trigger`

**Sources**:
- [HTMX 2.0 Release](https://htmx.org/posts/2024-06-17-htmx-2-0-0-is-released/)
- [HTMX with Rust Guide](https://www.joshfinnie.com/blog/trying-out-htmx-with-rust/)

#### SQLx + Tokio Best Practices ⚠️
- [x] Connection pooling
- [x] Compile-time verification
- [x] `FromRow` derive macros
- [x] Async/await throughout
- [ ] Transaction support (missing)
- [ ] `cargo audit` in CI (missing)

**Sources**:
- [SQLx Rust Crate Guide 2025](https://generalistprogrammer.com/tutorials/sqlx-rust-crate-guide)
- [SQLx Documentation](https://docs.rs/sqlx/latest/sqlx/)

---

## Critical Security Issues

### 🔴 PRIORITY 1: Security Vulnerabilities (Fix Immediately)

#### 1. Timing Attack Vulnerability - Webhook Authentication

**Location**: `server/src/routes/webhooks.rs`

**Current Code** (vulnerable):
```rust
fn verify_token(provided: &str, expected: &str) -> bool {
    provided == expected  // ❌ Vulnerable to timing attacks
}
```

**Fix Required** (from weatherust pattern):
```rust
use subtle::ConstantTimeEq;

fn verify_token(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
}
```

**Why This Matters**:
- Attackers can extract webhook tokens character-by-character via timing analysis
- Standard string comparison returns immediately on first mismatch
- Constant-time comparison prevents timing-based token extraction

**Implementation Steps**:
1. Add `subtle = "2.5"` to `server/Cargo.toml`
2. Update `server/src/routes/webhooks.rs:verify_token()`
3. Add test case with wrong tokens to verify constant-time behavior

**Reference**: `weatherust/updatectl/src/webhook.rs:verify_token()`

---

#### 2. API Token Leakage in Debug Logs

**Location**: `core/src/notifications.rs`

**Current Code** (vulnerable):
```rust
tracing::debug!("Sending notification with token: {}", token);  // ❌ Exposes full token
```

**Fix Required** (from weatherust pattern):
```rust
// Add to core/src/notifications.rs
fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        "***".to_string()
    } else {
        format!("***{}...{}", &token[..3], &token[token.len()-3..])
    }
}

// Usage
tracing::debug!("Sending notification with token: {}", mask_token(token));
```

**Why This Matters**:
- API tokens in logs can be exposed via log aggregation systems
- Compromised tokens allow unauthorized notifications
- Masked tokens still allow debugging while protecting secrets

**Implementation Steps**:
1. Add `mask_token()` utility to `core/src/notifications.rs`
2. Update all `tracing::debug!()` calls that log tokens
3. Add test case to verify masking behavior

**Reference**: `weatherust/common/src/lib.rs:mask_token()`

---

#### 3. CSRF Protection Missing

**Location**: `server/src/ui_routes.rs` (all POST/PUT/DELETE routes)

**Current Code** (vulnerable):
```html
<!-- No CSRF token in forms -->
<form hx-post="/servers">
    <input type="text" name="name">
    <button type="submit">Save</button>
</form>
```

**Fix Required**:
```rust
// Add to server/Cargo.toml
tower-csrf = "0.1"

// Add middleware in server/src/main.rs
use tower_csrf::CsrfLayer;

let app = Router::new()
    .route("/servers", post(create_server))
    .layer(CsrfLayer::new());

// Update templates
<form hx-post="/servers">
    <input type="hidden" name="csrf_token" value="{{ csrf_token }}">
    <input type="text" name="name">
    <button type="submit">Save</button>
</form>
```

**Why This Matters**:
- Without CSRF protection, attackers can trick users into executing actions
- Any authenticated user visiting a malicious site can have requests forged
- Critical for any state-changing operations

**Implementation Steps**:
1. Add CSRF middleware to Axum router
2. Update all form templates to include CSRF token
3. Add CSRF token validation to all POST/PUT/DELETE handlers
4. Add CSRF token to HTMX requests (via meta tag + hx-headers)

---

#### 4. Authentication Not Implemented

**Location**: `server/src/routes/auth.rs`

**Current Code** (stub):
```rust
async fn get_user_from_session() -> Option<User> {
    // TODO: Implement session management
    None
}
```

**Fix Required**:
```rust
// Add session management
use tower_sessions::{Session, SessionManagerLayer};
use tower_sessions::cookie::Cookie;

#[derive(Debug, Serialize, Deserialize)]
struct User {
    id: i64,
    username: String,
}

async fn login(
    session: Session,
    Form(creds): Form<LoginCredentials>,
) -> Result<Redirect, AppError> {
    // Verify credentials (bcrypt password hash)
    let user = verify_credentials(&creds).await?;

    // Store in session
    session.insert("user_id", user.id).await?;

    Ok(Redirect::to("/"))
}

async fn get_user_from_session(session: Session) -> Option<User> {
    let user_id: i64 = session.get("user_id").await.ok()??;
    // Load user from database
    load_user(user_id).await.ok()
}
```

**Why This Matters**:
- No access control = anyone can manage servers and execute tasks
- Critical for production deployment
- Required for multi-user scenarios

**Implementation Steps**:
1. Add `tower-sessions` and `bcrypt` to dependencies
2. Create `users` table migration
3. Implement login/logout handlers
4. Add session middleware to router
5. Add authentication check to protected routes
6. Update UI templates with login/logout links

---

#### 5. File-Based Secret Management Missing

**Location**: `server/src/config.rs`

**Current Code** (incomplete):
```rust
// Only supports environment variables
let gotify_key = env::var("DOCKER_GOTIFY_KEY")?;
```

**Fix Required** (from weatherust pattern):
```rust
// Add utility function
fn get_secret(var_name: &str) -> Option<String> {
    // Try environment variable first
    if let Ok(value) = env::var(var_name) {
        return Some(value);
    }

    // Try file-based secret (for Docker secrets / Kubernetes)
    let file_var = format!("{}_FILE", var_name);
    if let Ok(path) = env::var(&file_var) {
        if let Ok(contents) = fs::read_to_string(&path) {
            return Some(contents.trim().to_string());
        }
    }

    None
}

// Usage
let gotify_key = get_secret("DOCKER_GOTIFY_KEY")
    .ok_or_else(|| anyhow!("DOCKER_GOTIFY_KEY or DOCKER_GOTIFY_KEY_FILE required"))?;
```

**Why This Matters**:
- Docker secrets and Kubernetes secrets are mounted as files
- Environment variables are visible in `docker inspect` and process lists
- File-based secrets are more secure in containerized environments

**Implementation Steps**:
1. Add `get_secret()` utility to `server/src/config.rs`
2. Update all secret loading to use `get_secret()`
3. Update documentation to mention `*_FILE` variables
4. Add example Docker Compose with secrets

**Reference**: `weatherust/common/src/lib.rs:get_key_or_file()`

---

### 🔴 PRIORITY 2: Broken Features (Fix After Security)

#### 1. ntfy.sh Notifications Not Working

**Status**: 🔴 **CRITICAL** (per IMMEDIATE_PRIORITIES.md)

**Location**: `core/src/notifications.rs:NtfyBackend`

**Debug Steps**:
1. Add detailed logging to ntfy send method
2. Verify URL format: `https://ntfy.sh/{topic}`
3. Check request headers: `Content-Type: application/json`
4. Verify action button format in JSON payload
5. Test with curl first to isolate issue

**Expected Payload**:
```json
{
  "topic": "svrctlrs",
  "title": "Alert Title",
  "message": "Alert body",
  "priority": 4,
  "actions": [
    {"action": "view", "label": "View", "url": "https://..."}
  ]
}
```

**Reference**: `weatherust/common/src/lib.rs:send_ntfy()`

**Test Command**:
```bash
curl -X POST https://ntfy.sh/svrctlrs \
  -H "Content-Type: application/json" \
  -d '{"topic":"svrctlrs","title":"Test","message":"Hello"}'
```

---

#### 2. Transaction Support Missing

**Location**: `database/src/lib.rs`

**Current Code** (risky):
```rust
// Multiple operations without transaction
async fn create_task_with_history(task: Task) -> Result<()> {
    sqlx::query!("INSERT INTO tasks ...")
        .execute(&pool).await?;  // ❌ Can fail after this

    sqlx::query!("INSERT INTO task_history ...")
        .execute(&pool).await?;  // ❌ Leaves inconsistent state

    Ok(())
}
```

**Fix Required**:
```rust
async fn create_task_with_history(task: Task) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!("INSERT INTO tasks ...")
        .execute(&mut *tx).await?;

    sqlx::query!("INSERT INTO task_history ...")
        .execute(&mut *tx).await?;

    tx.commit().await?;  // All-or-nothing
    Ok(())
}
```

**Why This Matters**:
- Partial failures leave database in inconsistent state
- Tasks can be created without history records
- History records can exist for deleted tasks

**Implementation Steps**:
1. Add `begin_transaction()` method to Database struct
2. Update all multi-step operations to use transactions
3. Add rollback logic for error cases
4. Add integration tests for transaction behavior

**Affected Files**:
- `database/src/queries/tasks.rs`
- `database/src/queries/servers.rs`
- `database/src/queries/plugins.rs`

---

#### 3. Status Reports Not Implemented

**Status**: 🟡 Feature gap

**Current Behavior**:
- Plugins only send notifications on errors/warnings
- No periodic "all systems OK" summaries

**Fix Required**:
```rust
// Add to plugin configuration
pub struct PluginConfig {
    pub send_summary: bool,  // Send periodic status reports
    pub summary_schedule: String,  // e.g., "0 0 9 * * MON" (weekly)
}

// Add summary task type
pub enum TaskType {
    Check,    // Existing: check and alert on problems
    Summary,  // New: send periodic summary of state
}

// Example summary notification
async fn send_summary(&self, ctx: &PluginContext) -> Result<()> {
    let status = self.check_all_containers(ctx).await?;

    let summary = format!(
        "Docker Status Summary:\n\
         ✅ Healthy: {} containers\n\
         ⚠️ Warnings: {} containers\n\
         🔴 Unhealthy: {} containers\n\
         💾 Disk usage: {:.1}GB",
        status.healthy, status.warnings, status.unhealthy, status.disk_gb
    );

    ctx.send_notification("docker", &NotificationMessage {
        title: "Weekly Docker Summary".into(),
        body: summary,
        priority: 3,
        actions: vec![],
    }).await?;

    Ok(())
}
```

**Reference**: Weatherust sends both alerts and summaries

---

## Architecture & Code Quality

### Strengths ⭐⭐⭐⭐⭐

1. **Plugin Architecture** - Clean trait-based design
   - `Plugin` trait with 3 required methods
   - Feature-flag compilation
   - Database-driven enablement
   - Easy to extend

2. **Workspace Organization** - Modular monorepo
   - `core/` - Shared types and traits
   - `server/` - Web server
   - `scheduler/` - Built-in scheduler
   - `database/` - Persistence layer
   - `plugins/` - Individual features

3. **Error Handling** - Structured and comprehensive
   - `thiserror` for structured errors
   - `anyhow` for dynamic errors
   - `Result<T>` everywhere
   - Good error context with `.context()`

4. **Single-Binary Deployment** - vs weatherust's 5-7 containers
   - Simpler deployment
   - Lower resource usage
   - Easier configuration
   - Built-in scheduler (no Ofelia)

### Architecture Issues ⚠️

#### Issue 1: Global State Anti-Pattern

**Location**: `server/src/ui_routes.rs`

**Current Code** (bad):
```rust
use once_cell::sync::OnceCell;

static APP_STATE: OnceCell<AppState> = OnceCell::new();

async fn some_route() -> Result<Html<String>, AppError> {
    let state = APP_STATE.get().unwrap();  // ❌ Global state
    // ...
}
```

**Fix Required**:
```rust
// Remove OnceCell global

async fn some_route(
    State(state): State<AppState>,  // ✅ Axum state injection
) -> Result<Html<String>, AppError> {
    // Use state parameter
}
```

**Why This Matters**:
- Breaks testability (can't create isolated state for tests)
- Breaks composability (can't run multiple instances)
- Not idiomatic Axum (framework provides state injection)
- Makes refactoring harder

**Files to Fix**:
- `server/src/ui_routes.rs` (primary issue)
- `server/src/state.rs` (remove global setter)

---

#### Issue 2: Task Schedule Persistence Mismatch

**Location**: `scheduler/src/lib.rs` + `database/migrations/004_create_tasks.sql`

**Problem**:
- Plugins define `scheduled_tasks()` in code
- Database stores task definitions
- Scheduler runs in-memory only
- On restart, scheduler uses plugin defaults, not database values

**Example Conflict**:
```rust
// Plugin defines:
ScheduledTask {
    id: "docker_health",
    schedule: "0 */5 * * * *",  // Every 5 minutes
}

// User changes in database to:
UPDATE tasks SET schedule = "0 */15 * * * *" WHERE id = "docker_health";

// After restart: scheduler uses plugin default (5 min), not DB value (15 min)
```

**Fix Required**:
```rust
// In server/src/state.rs:start_scheduler()
async fn start_scheduler(state: &AppState) -> Result<()> {
    let scheduler = Scheduler::new();

    // Load tasks from database, not plugins
    let tasks = state.database.read().await.get_all_tasks().await?;

    for task in tasks {
        if !task.enabled {
            continue;
        }

        // Register task with database schedule
        scheduler.add_task(
            &task.id,
            &task.schedule,  // ✅ Use database value
            create_handler(state.clone(), task),
        )?;
    }

    Ok(())
}
```

**Why This Matters**:
- User schedule changes in UI don't persist across restarts
- Confusing behavior (UI shows one schedule, system uses another)
- Defeats purpose of database-backed configuration

---

#### Issue 3: Large Route File

**Location**: `server/src/ui_routes.rs` - **1042 lines**

**Current Structure** (all in one file):
- Dashboard routes
- Server management routes
- Task management routes
- Plugin configuration routes
- Settings routes
- Authentication routes
- Static file serving

**Recommended Structure**:
```
server/src/routes/
├── mod.rs              # Router assembly
├── ui/
│   ├── mod.rs          # UI router
│   ├── dashboard.rs    # Dashboard routes (~100 lines)
│   ├── servers.rs      # Server CRUD (~200 lines)
│   ├── tasks.rs        # Task management (~200 lines)
│   ├── plugins.rs      # Plugin config (~150 lines)
│   ├── settings.rs     # Settings (~100 lines)
│   └── auth.rs         # Authentication (~100 lines)
├── api/                # REST API routes (existing)
└── webhooks/           # Webhook routes (existing)
```

**Migration Steps**:
1. Create `server/src/routes/ui/` directory
2. Extract dashboard routes to `dashboard.rs`
3. Extract server routes to `servers.rs`
4. Extract task routes to `tasks.rs`
5. Extract plugin routes to `plugins.rs`
6. Extract settings routes to `settings.rs`
7. Extract auth routes to `auth.rs`
8. Create `mod.rs` to assemble routers
9. Update `server/src/main.rs` to use new structure
10. Delete old `ui_routes.rs`

**Benefits**:
- Easier navigation
- Better code organization
- Easier to test individual route groups
- Follows Axum best practices

---

## UI Organization Needs

### Current UI Pages

| Page | Route | Status | Lines | Notes |
|------|-------|--------|-------|-------|
| Dashboard | `/` | ✅ Basic | ~50 | Needs metrics expansion |
| Servers | `/servers` | ✅ Complete | ~200 | Good CRUD implementation |
| Tasks | `/tasks` | ✅ Good | ~300 | Has inline editing |
| Plugins | `/plugins` | ⚠️ Partial | ~150 | Config save/load incomplete |
| Settings | `/settings` | ✅ Basic | ~100 | Needs expansion |
| Notifications | `/settings/notifications` | ✅ Complete | ~150 | Good implementation |
| Login | `/auth/login` | 🔴 Stub | ~50 | Not functional (TODO) |

### Missing UI Features

#### 1. Real-Time Monitoring Dashboard

**Current Dashboard** (basic):
```html
<!-- server/templates/pages/dashboard.html -->
<div class="stats">
    <div class="card">Total Servers: {{ server_count }}</div>
    <div class="card">Total Tasks: {{ task_count }}</div>
    <div class="card">Active Plugins: {{ plugin_count }}</div>
</div>
```

**Needed Dashboard** (comprehensive):
```html
<!-- Enhanced dashboard with metrics -->
<div class="dashboard">
    <!-- System Health Overview -->
    <div class="health-grid">
        <div class="card health-card" data-status="healthy">
            <h3>System Health</h3>
            <div class="status-indicator">✅ All Systems Operational</div>
            <div class="metrics">
                <span>CPU: 45%</span>
                <span>Memory: 62%</span>
                <span>Disk: 78%</span>
            </div>
        </div>
    </div>

    <!-- Server Status Cards -->
    <div class="server-grid">
        {% for server in servers %}
        <div class="card server-card" data-status="{{ server.status }}">
            <h4>{{ server.name }}</h4>
            <div class="status">{{ server.status }}</div>
            <div class="metrics">
                <div class="metric">
                    <label>Uptime</label>
                    <value>{{ server.uptime }}</value>
                </div>
                <div class="metric">
                    <label>Tasks</label>
                    <value>{{ server.task_count }}</value>
                </div>
            </div>
            <div class="actions">
                <button hx-get="/servers/{{ server.id }}/details">Details</button>
            </div>
        </div>
        {% endfor %}
    </div>

    <!-- Recent Activity Feed -->
    <div class="activity-feed"
         hx-get="/api/activity/recent"
         hx-trigger="every 10s"
         hx-swap="innerHTML">
        <!-- Auto-updating activity list -->
    </div>

    <!-- Metrics Charts (with Chart.js) -->
    <div class="charts">
        <div class="card">
            <h3>CPU Usage (24h)</h3>
            <canvas id="cpu-chart"></canvas>
        </div>
        <div class="card">
            <h3>Task Executions (7d)</h3>
            <canvas id="task-chart"></canvas>
        </div>
    </div>
</div>
```

**Implementation Steps**:
1. Add Chart.js to `server/static/js/`
2. Create `/api/metrics/timeseries` endpoint
3. Update dashboard template with charts
4. Add auto-refresh for metrics
5. Style with Nord theme colors

---

#### 2. Server Health Indicators

**Current** (no visual status):
```html
<!-- Servers just listed, no status indicator -->
<tr>
    <td>{{ server.name }}</td>
    <td>{{ server.host }}</td>
    <td><button>Edit</button></td>
</tr>
```

**Needed** (with status):
```html
<tr class="server-row" data-status="{{ server.status }}">
    <td>
        <span class="status-indicator status-{{ server.status }}">
            {% if server.status == "online" %}🟢{% endif %}
            {% if server.status == "offline" %}🔴{% endif %}
            {% if server.status == "unknown" %}⚪{% endif %}
        </span>
        {{ server.name }}
    </td>
    <td>{{ server.host }}</td>
    <td>
        <span class="last-seen">
            Last seen: {{ server.last_ping_at | timeago }}
        </span>
    </td>
    <td>
        <div class="server-metrics">
            CPU: {{ server.cpu_pct }}% |
            Mem: {{ server.mem_pct }}% |
            Tasks: {{ server.active_tasks }}
        </div>
    </td>
    <td>
        <button hx-get="/servers/{{ server.id }}/edit">Edit</button>
        <button hx-post="/servers/{{ server.id }}/ping">Ping</button>
    </td>
</tr>
```

**Implementation**:
1. Add `last_ping_at` and `status` columns to `servers` table
2. Create background task to ping servers every 60s
3. Update server row template with status indicators
4. Add CSS for status colors (Nord theme)
5. Add `timeago` filter to Askama

---

#### 3. Task Execution Real-Time Feedback

**Current** (no feedback):
```html
<button hx-post="/tasks/{{ task.id }}/run">Run Now</button>
```

**Needed** (with real-time feedback):
```html
<button hx-post="/tasks/{{ task.id }}/run"
        hx-swap="outerHTML"
        hx-indicator="#task-{{ task.id }}-spinner">
    <span id="task-{{ task.id }}-spinner" class="htmx-indicator">
        ⏳ Running...
    </span>
    <span>Run Now</span>
</button>

<!-- After execution, swap with result -->
<div class="task-result" hx-swap-oob="true" id="task-{{ task.id }}-result">
    {% if result.success %}
        <span class="success">✅ Completed in {{ result.duration }}ms</span>
    {% else %}
        <span class="error">❌ Failed: {{ result.error }}</span>
    {% endif %}
    <button hx-get="/tasks/{{ task.id }}/logs">View Logs</button>
</div>
```

**Implementation**:
1. Update `POST /tasks/{id}/run` to return execution result HTML
2. Add spinner indicator styles
3. Add result display component
4. Add log viewer modal
5. Consider WebSocket for long-running tasks

---

#### 4. Plugin Configuration UI Completion

**Current** (incomplete):
```rust
// server/src/ui_routes.rs
async fn save_plugin_config(
    Form(input): Form<PluginConfigInput>,
) -> Result<Html<String>, AppError> {
    // TODO: Implement config save logic
    Ok(Html("<div>Not implemented</div>".to_string()))
}
```

**Needed**:
```rust
async fn save_plugin_config(
    State(state): State<AppState>,
    Path(plugin_id): Path<String>,
    Form(input): Form<PluginConfigInput>,
) -> Result<Html<String>, AppError> {
    // Parse and validate config JSON
    let config: serde_json::Value = serde_json::from_str(&input.config)
        .context("Invalid JSON configuration")?;

    // Save to database
    state.database.write().await
        .update_plugin_config(&plugin_id, config)
        .await?;

    // Reload plugin with new config
    state.reload_plugins().await?;

    // Return updated plugin card
    let plugin = state.plugins.read().await.get(&plugin_id)?;
    let template = PluginCardTemplate { plugin };
    Ok(Html(template.render()?))
}
```

**Files to Update**:
- `server/src/ui_routes.rs:save_plugin_config()`
- `database/src/queries/plugins.rs:update_plugin_config()`
- `server/templates/components/plugin_config_form.html`

---

### Recommended Navigation Structure

```
📊 Dashboard
   ├─ System overview cards
   ├─ Recent activity feed
   ├─ Quick actions
   └─ Health indicators

🖥️ Infrastructure
   ├─ Servers (manage SSH hosts)
   ├─ Server Groups (organize by role/env)
   └─ SSH Keys (authentication management)

📋 Automation
   ├─ Tasks (task definitions)
   ├─ Task History (execution logs with search/filter)
   ├─ Schedules (cron schedule editor with validation)
   └─ Workflows (task dependencies/chains)

🔌 Plugins
   ├─ Installed (enable/disable/configure)
   ├─ Available (plugin marketplace)
   └─ Plugin Settings (per-plugin configuration forms)

📈 Monitoring
   ├─ Metrics (time-series charts: CPU, memory, disk)
   ├─ Logs (centralized log viewer with search)
   ├─ Alerts (alert history with filter)
   └─ Health Checks (service health dashboard)

🔔 Notifications
   ├─ Backends (Gotify/ntfy config)
   ├─ Rules (notification routing logic)
   └─ History (sent notifications log)

⚙️ Settings
   ├─ General (app-wide settings)
   ├─ Security (auth, API keys, webhook secrets)
   ├─ Database (backup/restore, migrations)
   └─ Audit Log (system activity log with search)
```

---

## Weatherust Feature Parity

### Successfully Ported Features ✅

| Feature | Weatherust | SvrCtlRS | Status |
|---------|-----------|----------|--------|
| Docker health monitoring | healthmon | docker plugin | ✅ 100% |
| Update detection | updatemon | updates plugin | ✅ 100% |
| Update execution | updatectl | updates plugin | ✅ 95% |
| Docker cleanup | updatectl | docker plugin | ✅ 100% |
| Speed testing | speedynotify | speedtest plugin | ✅ 100% |
| Weather monitoring | weatherust | weather plugin | ✅ 100% |
| Gotify notifications | common lib | NotificationManager | ✅ 100% |
| ntfy notifications | common lib | NotificationManager | 🔴 0% (broken) |
| Remote SSH execution | RemoteExecutor | RemoteExecutor | ✅ 95% |
| System health | N/A | health plugin | ✅ New feature |

### Missing Patterns from Weatherust 🔴

#### 1. Retry Logic with Exponential Backoff

**Weatherust Implementation**: `weatherust/common/src/retry.rs`

```rust
pub async fn retry_async<F, Fut, T, E>(
    mut operation: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;
    let mut attempts = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempts >= max_retries => return Err(e),
            Err(_) => {
                tokio::time::sleep(delay).await;
                attempts += 1;
                delay = delay.saturating_mul(2); // Exponential backoff
            }
        }
    }
}

pub fn is_retryable_http_error(error: &reqwest::Error) -> bool {
    if error.is_timeout() || error.is_connect() {
        return true;
    }

    if let Some(status) = error.status() {
        // Retry on 5xx and 429 (rate limit)
        return status.is_server_error() || status.as_u16() == 429;
    }

    false
}
```

**Where to Add in SvrCtlRS**:
1. Create `core/src/retry.rs` with retry utilities
2. Use in `core/src/notifications.rs` for notification sending
3. Use in `core/src/remote.rs` for SSH commands
4. Use in `plugins/docker/src/lib.rs` for Docker API calls

**Example Usage**:
```rust
use crate::retry::{retry_async, is_retryable_http_error};

async fn send_notification_with_retry(msg: &NotificationMessage) -> Result<()> {
    retry_async(
        || async { self.send_notification_internal(msg).await },
        3,  // max retries
        Duration::from_secs(1),  // initial delay
    ).await
}
```

---

#### 2. Notification Metrics

**Weatherust Pattern**: `weatherust/common/src/lib.rs`

```rust
// After every notification attempt
metrics::record_notification_sent(
    &service_name,      // "docker", "updates", etc.
    &backend_type,      // "gotify" or "ntfy"
    result.is_ok(),     // success boolean
);

// Stored in metrics table for analysis
```

**Where to Add in SvrCtlRS**:
1. Add to `core/src/notifications.rs:NotificationManager::send()`
2. Record in `metrics` table (already exists)
3. Create dashboard widget showing notification success rate
4. Alert if notification failure rate exceeds threshold

**Implementation**:
```rust
// In NotificationManager::send_for_service()
let start = Instant::now();
let result = backend.send(message).await;
let duration = start.elapsed();

// Record metric
database.record_metric(MetricRecord {
    server_id: None,  // System metric
    plugin_id: Some(service_id.to_string()),
    metric_name: format!("notification_sent_{}", backend.name()),
    metric_value: if result.is_ok() { 1.0 } else { 0.0 },
    metric_unit: "success".to_string(),
    timestamp: Utc::now(),
}).await?;

result
```

---

#### 3. Multi-Profile Cleanup Strategies

**Weatherust Implementation**: `weatherust/updatectl/src/cleanup.rs`

```rust
pub enum CleanupProfile {
    Conservative,  // Only safe operations
    Moderate,      // Some risk, high benefit
    Aggressive,    // Maximum cleanup
}

impl CleanupProfile {
    pub fn operations(&self) -> Vec<CleanupOperation> {
        match self {
            Self::Conservative => vec![
                CleanupOperation::AptAutoClean,
                CleanupOperation::DockerImagePrune { dangling_only: true },
            ],
            Self::Moderate => vec![
                CleanupOperation::AptAutoClean,
                CleanupOperation::AptAutoRemove,
                CleanupOperation::DockerImagePrune { dangling_only: false },
                CleanupOperation::DockerContainerPrune,
            ],
            Self::Aggressive => vec![
                CleanupOperation::AptAutoClean,
                CleanupOperation::AptAutoRemove,
                CleanupOperation::DockerSystemPrune { volumes: false },
                CleanupOperation::JournalctlVacuum { size_mb: 100 },
                CleanupOperation::TmpCleanup { days: 7 },
            ],
        }
    }
}
```

**Where to Add in SvrCtlRS**:
1. Add `CleanupProfile` enum to `plugins/docker/src/cleanup.rs`
2. Add `CleanupProfile` enum to `plugins/updates/src/cleanup.rs`
3. Add profile selection to plugin configuration UI
4. Add dry-run mode for preview

---

### Feature Advantages of SvrCtlRS ⭐

These are features SvrCtlRS has that weatherust lacks:

| Feature | SvrCtlRS | Weatherust |
|---------|----------|------------|
| Web UI | ✅ Full HTMX interface | ❌ Webhook API only |
| Database persistence | ✅ SQLite with migrations | ❌ Environment variables |
| Task management UI | ✅ CRUD + inline editing | ❌ Docker labels only |
| Built-in scheduler | ✅ Integrated cron | ❌ External (Ofelia) |
| Plugin system | ✅ Dynamic loading | ❌ Separate binaries |
| Single binary | ✅ One container | ❌ 5-7 containers |
| System health plugin | ✅ CPU/memory/disk | ❌ Not available |

---

## Development Roadmap

### Phase 1: Security & Stability (Weeks 1-2) 🔴

**Goal**: Fix critical security vulnerabilities and broken features

**Tasks**:
- [ ] Add constant-time token comparison (`subtle` crate)
  - File: `server/src/routes/webhooks.rs`
  - Test: Verify timing consistency

- [ ] Implement token masking for debug logs
  - File: `core/src/notifications.rs`
  - Add: `mask_token()` utility function

- [ ] Add CSRF protection middleware
  - File: `server/src/main.rs`
  - Add: `tower-csrf` layer
  - Update: All form templates

- [ ] Implement authentication system
  - Files: `server/src/routes/auth.rs`, `database/migrations/009_create_users.sql`
  - Add: `tower-sessions` middleware
  - Add: `bcrypt` password hashing

- [ ] Add file-based secret management
  - File: `server/src/config.rs`
  - Add: `get_secret()` utility
  - Update: All secret loading

- [ ] Fix ntfy.sh notifications
  - File: `core/src/notifications.rs`
  - Debug: Request format, headers, payload
  - Test: Manual curl + automated test

- [ ] Add transaction support
  - File: `database/src/lib.rs`
  - Add: `begin_transaction()` method
  - Update: All multi-step operations

**Success Criteria**:
- All security vulnerabilities fixed
- ntfy notifications working
- All tests passing
- Ready for production deployment

---

### Phase 2: UI Organization (Weeks 3-4) 🟡

**Goal**: Improve UI organization and user experience

**Tasks**:
- [ ] Split `ui_routes.rs` into modules
  - Create: `server/src/routes/ui/` directory
  - Extract: dashboard, servers, tasks, plugins, settings, auth
  - Delete: `server/src/ui_routes.rs`

- [ ] Build comprehensive monitoring dashboard
  - Add: System health overview cards
  - Add: Server status grid with indicators
  - Add: Recent activity feed
  - Add: Metrics charts (Chart.js)
  - Update: `server/templates/pages/dashboard.html`

- [ ] Implement server health indicators
  - Add: `last_ping_at`, `status` columns to servers table
  - Create: Background ping task
  - Update: Server list template with status indicators
  - Add: `timeago` Askama filter

- [ ] Add task execution real-time feedback
  - Update: `POST /tasks/{id}/run` handler
  - Add: Spinner indicators
  - Add: Execution result component
  - Add: Log viewer modal

- [ ] Complete plugin configuration UI
  - Implement: `save_plugin_config()` handler
  - Add: JSON validation
  - Add: Plugin reload logic
  - Test: All plugins configurable

**Success Criteria**:
- Code well-organized (<300 lines per file)
- Dashboard shows real-time metrics
- Server health visible at-a-glance
- Task execution provides immediate feedback
- All plugins configurable via UI

---

### Phase 3: Feature Completeness (Weeks 5-8) 🟢

**Goal**: Achieve feature parity with weatherust + add enhancements

**Tasks**:
- [ ] Implement webhook API for remote triggers
  - Add: `POST /api/webhooks/{plugin}/{task}` endpoints
  - Add: Token authentication
  - Update: Documentation

- [ ] Build task dependency/workflow engine
  - Add: `task_dependencies` table
  - Update: Scheduler to handle dependencies
  - Add: UI for defining dependencies

- [ ] Add metrics visualization
  - Add: Chart.js time-series charts
  - Create: `/api/metrics/timeseries` endpoint
  - Add: 7-day, 30-day, 90-day views

- [ ] Implement audit logging
  - Add: `audit_logs` table migration
  - Add: Audit middleware for all state changes
  - Create: Audit log viewer UI

- [ ] Add SSH key management UI
  - Add: `ssh_keys` table
  - Create: Key upload/management UI
  - Update: Server form to select key

- [ ] Implement server grouping/tagging
  - Add: `server_groups` and `server_tags` tables
  - Create: Group management UI
  - Add: Bulk operations on groups

- [ ] Add backup/restore functionality
  - Create: `/api/admin/backup` endpoint
  - Add: SQLite backup script
  - Add: Restore from backup UI

**Success Criteria**:
- All weatherust features ported
- Enhanced features working
- UI feature-complete
- Ready for multi-user scenarios

---

### Phase 4: Testing & Reliability (Weeks 9-10) 🔵

**Goal**: Achieve comprehensive test coverage and reliability

**Tasks**:
- [ ] Add unit tests for all plugins
  - Target: 70% code coverage
  - Test: Each plugin's execute() method
  - Test: Notification sending
  - Test: Remote execution

- [ ] Add integration tests for API routes
  - Test: All REST API endpoints
  - Test: Authentication flows
  - Test: Database transactions

- [ ] Add UI integration tests
  - Tool: Playwright or Selenium
  - Test: Critical user journeys
  - Test: Form submissions

- [ ] Add `cargo clippy` to CI pipeline
  - Update: `.github/workflows/`
  - Enforce: No clippy warnings

- [ ] Add `cargo audit` to CI pipeline
  - Update: `.github/workflows/`
  - Alert: On security advisories

- [ ] Implement retry logic everywhere
  - Port: `weatherust/common/src/retry.rs`
  - Use: In notifications, SSH, Docker API
  - Test: Network failure scenarios

- [ ] Add notification metrics
  - Record: Success/failure for every notification
  - Create: Dashboard widget
  - Alert: On high failure rate

- [ ] Performance testing
  - Test: 100+ servers
  - Test: 1000+ tasks
  - Optimize: Database queries

- [ ] Load testing
  - Tool: `wrk` or `k6`
  - Test: API endpoints under load
  - Optimize: Connection pooling

**Success Criteria**:
- 70%+ test coverage
- CI/CD enforces quality
- Performance validated
- Ready for scale

---

### Phase 5: Advanced Features (Weeks 11-12) 🟣

**Goal**: Add advanced features for production environments

**Tasks**:
- [ ] Replace polling with WebSocket/SSE
  - Add: WebSocket support to Axum
  - Replace: 5s auto-refresh with event-driven updates
  - Test: Real-time updates

- [ ] Implement multi-user support with RBAC
  - Add: `roles` and `permissions` tables
  - Add: Role-based access control
  - Add: User management UI

- [ ] Add API versioning
  - Add: `/api/v1/` prefix
  - Add: Version negotiation
  - Document: API changelog

- [ ] Implement rate limiting
  - Add: `tower-rate-limit` middleware
  - Configure: Per-user limits
  - Alert: On rate limit violations

- [ ] Add Prometheus metrics export
  - Add: `/metrics` endpoint
  - Export: Task counts, execution times, error rates
  - Document: Prometheus setup

- [ ] Build historical data analysis
  - Add: Data retention policies
  - Add: Trend analysis
  - Add: Anomaly detection

**Success Criteria**:
- Real-time updates working
- Multi-user support complete
- API versioned and rate-limited
- Prometheus integration working
- Enterprise-ready feature set

---

## File Reference Guide

### Core Files

**Plugin System**:
- `core/src/plugin.rs` - Plugin trait definition
- `core/src/types.rs` - Shared types (Server, PluginContext, etc.)
- `core/src/error.rs` - Error types

**Notifications**:
- `core/src/notifications.rs` - NotificationManager, backends
- Reference: `weatherust/common/src/lib.rs` - Weatherust notification patterns

**Remote Execution**:
- `core/src/remote.rs` - RemoteExecutor for SSH
- Reference: `weatherust/common/src/executor.rs` - Weatherust executor

---

### Server Files

**Main Application**:
- `server/src/main.rs` - Server entry point, router setup
- `server/src/config.rs` - Configuration loading
- `server/src/state.rs` - AppState, plugin initialization

**Routes** (NEEDS REORGANIZATION):
- `server/src/ui_routes.rs` - **1042 lines, split needed**
- `server/src/routes/api.rs` - REST API endpoints
- `server/src/routes/webhooks.rs` - Webhook endpoints

**Templates**:
- `server/templates/base.html` - Base layout
- `server/templates/pages/` - Full page templates
- `server/templates/components/` - HTMX partials

**Static Assets**:
- `server/static/css/styles.css` - Nord theme
- `server/static/js/htmx.min.js` - HTMX 2.0.3
- `server/static/js/alpine.min.js` - Alpine.js 3.14.1

---

### Database Files

**Migrations**:
- `database/migrations/001_create_core_tables.sql` - Core tables
- `database/migrations/004_create_tasks.sql` - Task system
- `database/migrations/006_create_settings.sql` - Settings

**Queries**:
- `database/src/queries/servers.rs` - Server CRUD
- `database/src/queries/tasks.rs` - Task CRUD + history
- `database/src/queries/plugins.rs` - Plugin management

---

### Plugin Files

**Docker Plugin**:
- `plugins/docker/src/lib.rs` - Main plugin + dispatcher
- `plugins/docker/src/health.rs` - Health checking (~200 lines)
- `plugins/docker/src/cleanup.rs` - Cleanup suggestions
- Reference: `weatherust/healthmon/src/main.rs` - Weatherust docker monitoring

**Updates Plugin**:
- `plugins/updates/src/lib.rs` - Main plugin
- `plugins/updates/src/detection.rs` - OS detection + update checking
- `plugins/updates/src/execution.rs` - Update execution + reboot
- Reference: `weatherust/updatemon/src/main.rs` - Weatherust update monitoring
- Reference: `weatherust/updatectl/src/` - Weatherust update execution

**Health Plugin**:
- `plugins/health/src/lib.rs` - System metrics monitoring

**Weather Plugin**:
- `plugins/weather/src/lib.rs` - OpenWeatherMap integration
- Reference: `weatherust/src/main.rs` - Weatherust weather service

**Speedtest Plugin**:
- `plugins/speedtest/src/lib.rs` - Ookla speedtest integration
- Reference: `weatherust/speedynotify/src/main.rs` - Weatherust speedtest

---

### Scheduler Files

**Built-in Scheduler**:
- `scheduler/src/lib.rs` - Cron-based task scheduler (~150 lines)

---

### Configuration Files

**Docker**:
- `Dockerfile` - Multi-stage build
- `docker-compose.yml` - Compose configuration
- `.github/workflows/docker-publish-develop.yml` - AMD64 build (5-8 min)
- `.github/workflows/docker-publish-main.yml` - Multi-arch build (15-20 min)

**Rust**:
- `Cargo.toml` - Workspace configuration
- `*/Cargo.toml` - Individual crate configs

---

## Testing Requirements

### Current State: 🔴 Minimal Coverage

**Issues**:
- No unit tests found in plugins/
- No integration tests for routes
- No UI tests
- No performance tests
- No CI test enforcement

### Target State: 🟢 70% Coverage

#### Unit Tests (Target: 70% coverage)

**Plugin Tests**:
```rust
// plugins/docker/src/lib.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_health_check() {
        let plugin = DockerPlugin::new();
        let ctx = create_test_context();

        let result = plugin.execute("docker_health", &ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_docker_health_with_unhealthy_container() {
        // Mock unhealthy container
        let plugin = DockerPlugin::new();
        let ctx = create_test_context_with_unhealthy();

        let result = plugin.execute("docker_health", &ctx).await.unwrap();
        assert!(result.notifications.len() > 0);
    }
}
```

**Notification Tests**:
```rust
// core/src/notifications.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gotify_notification() {
        let backend = GotifyBackend::new("https://gotify.example.com", "token");
        let msg = NotificationMessage {
            title: "Test".into(),
            body: "Test body".into(),
            priority: 5,
            actions: vec![],
        };

        let result = backend.send(&msg).await;
        // Assert based on mock server
    }

    #[test]
    fn test_token_masking() {
        assert_eq!(mask_token("secret"), "***");
        assert_eq!(mask_token("verylongtoken123"), "***ver...123");
    }
}
```

---

#### Integration Tests

**API Tests**:
```rust
// server/tests/integration/api_tests.rs
#[tokio::test]
async fn test_create_server() {
    let app = create_test_app().await;

    let response = app
        .request("/api/servers")
        .method("POST")
        .json(&json!({
            "name": "test-server",
            "host": "user@host",
        }))
        .send()
        .await;

    assert_eq!(response.status(), 201);
}

#[tokio::test]
async fn test_webhook_auth() {
    let app = create_test_app().await;

    // Without token
    let response = app
        .request("/api/webhooks/docker/health")
        .method("POST")
        .send()
        .await;
    assert_eq!(response.status(), 401);

    // With valid token
    let response = app
        .request("/api/webhooks/docker/health")
        .method("POST")
        .header("Authorization", "Bearer valid-token")
        .send()
        .await;
    assert_eq!(response.status(), 200);
}
```

---

#### UI Tests (Playwright)

```typescript
// server/tests/ui/dashboard.spec.ts
import { test, expect } from '@playwright/test';

test('dashboard loads correctly', async ({ page }) => {
  await page.goto('http://localhost:3000/');

  // Check for dashboard elements
  await expect(page.locator('h1')).toContainText('Dashboard');
  await expect(page.locator('.server-count')).toBeVisible();
  await expect(page.locator('.task-count')).toBeVisible();
});

test('can create new server', async ({ page }) => {
  await page.goto('http://localhost:3000/servers');

  // Click "New Server" button
  await page.click('button:has-text("New Server")');

  // Fill form
  await page.fill('input[name="name"]', 'Test Server');
  await page.fill('input[name="host"]', 'user@host');

  // Submit
  await page.click('button[type="submit"]');

  // Verify server appears in list
  await expect(page.locator('text=Test Server')).toBeVisible();
});
```

---

### CI/CD Test Integration

**Update `.github/workflows/test.yml`**:
```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run cargo test
        run: cargo test --workspace --all-features

      - name: Run cargo clippy
        run: cargo clippy --workspace --all-features -- -D warnings

      - name: Run cargo audit
        run: |
          cargo install cargo-audit
          cargo audit

      - name: Check code coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --workspace --out Xml
          # Upload to codecov or similar
```

---

## Best Practices Checklist

### Rust Code Quality ✅

- [x] Use `Result<T>` for error handling
- [x] Use `thiserror` for structured errors
- [x] Use `anyhow` for dynamic errors
- [x] Add `#[instrument]` to key functions
- [x] Use async/await with Tokio
- [ ] Add unit tests (target: 70% coverage)
- [ ] Run `cargo clippy` (enforce in CI)
- [ ] Run `cargo audit` (enforce in CI)
- [x] Use feature flags for optional components
- [x] Document public APIs with `///` comments

### Security Best Practices 🔴

- [ ] Constant-time token comparison (use `subtle`)
- [ ] Token masking in debug logs
- [ ] CSRF protection on all state-changing routes
- [ ] Authentication + session management
- [ ] File-based secret management (Docker secrets)
- [ ] Rate limiting on API endpoints
- [ ] Input validation on all forms
- [ ] SQL injection prevention (using sqlx params)
- [ ] XSS prevention (Askama escapes by default)

### Database Best Practices ⚠️

- [x] Use migrations for schema changes
- [x] Use compile-time query checking (sqlx)
- [ ] Use transactions for multi-step operations
- [x] Use connection pooling
- [ ] Add indexes for frequently queried columns
- [ ] Add foreign key constraints
- [ ] Implement soft deletes for audit trail
- [ ] Add database backups

### API Design Best Practices 🟡

- [x] RESTful endpoint naming
- [ ] API versioning (`/api/v1/`)
- [ ] Rate limiting
- [x] Error responses with context
- [ ] OpenAPI/Swagger documentation
- [x] Webhook authentication
- [ ] Request/response logging
- [ ] CORS configuration for web clients

### HTMX Best Practices ✅

- [x] Use `hx-get`/`hx-post` for AJAX requests
- [x] Use `hx-swap` for targeted updates
- [x] Use `hx-trigger` for event-driven updates
- [ ] Use `hx-confirm` for destructive actions
- [x] Use loading indicators (`hx-indicator`)
- [ ] Use `hx-push-url` for browser history
- [x] Use out-of-band swaps (`hx-swap-oob`)
- [ ] Use WebSocket/SSE for real-time (instead of polling)

### Deployment Best Practices ✅

- [x] Multi-stage Docker builds
- [x] Multi-arch support (AMD64 + ARM64)
- [x] Non-root user in container
- [x] Volume mounts for persistence
- [x] Environment variable configuration
- [ ] Health check endpoint in Dockerfile
- [x] CI/CD with GitHub Actions
- [ ] Automated security scanning
- [ ] Container image signing

---

## Quick Reference Commands

### Development

```bash
# Build project
cargo build --workspace

# Run tests
cargo test --workspace

# Run with logging
RUST_LOG=debug cargo run --bin server

# Run clippy
cargo clippy --workspace -- -D warnings

# Check security advisories
cargo install cargo-audit && cargo audit

# Format code
cargo fmt --all

# Run server
cargo run --bin server -- --config config.toml
```

### Database

```bash
# Run migrations
sqlx migrate run

# Create new migration
sqlx migrate add <name>

# Revert last migration
sqlx migrate revert
```

### Docker

```bash
# Build image
docker build -t svrctlrs:dev .

# Run container
docker-compose up -d

# View logs
docker-compose logs -f

# Shell into container
docker-compose exec svrctlrs sh
```

### Testing

```bash
# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test -- --nocapture

# Run UI tests (Playwright)
cd server/tests/ui && npx playwright test
```

---

## Summary

SvrCtlRS is a **well-architected platform** with a **solid foundation** but requires **security hardening** before production deployment. The technology stack is current with 2025 best practices, and the plugin architecture provides excellent extensibility.

**Immediate Actions**:
1. Fix security vulnerabilities (Phase 1)
2. Fix broken ntfy notifications
3. Add transaction support
4. Improve UI organization (Phase 2)

**Next Steps**:
1. Follow the development roadmap phases
2. Use this document as a reference during development
3. Update `CLAUDE.md` with significant architectural changes
4. Keep `IMMEDIATE_PRIORITIES.md` updated with current blockers

**Assessment**: B+ (85/100) - Production-Ready with Security Fixes Needed

---

**Document Version**: 1.0
**Last Updated**: 2024-11-26
**Next Review**: After Phase 1 completion
