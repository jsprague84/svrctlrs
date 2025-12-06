# Current State Assessment - 2024-11-26

## ✅ What's Already Built

### 1. UI Organization
- **Status**: ✅ **COMPLETE**
- UI routes already split into focused modules:
  - `routes/ui/dashboard.rs` - Dashboard page
  - `routes/ui/servers.rs` - Server management (CRUD + SSH test)
  - `routes/ui/tasks.rs` - Task management with grouping
  - `routes/ui/plugins.rs` - Plugin configuration
  - `routes/ui/settings.rs` - Settings & notifications
  - `routes/ui/auth.rs` - Authentication stubs

### 2. Server Management
- **Status**: ✅ **COMPLETE**
- Full CRUD operations for servers
- SSH connection testing works
- Database schema with:
  - Server name, host, port, username
  - SSH key path support
  - Enabled/disabled state
  - Tags field (not yet used in UI)

### 3. Task-Server Relationship
- **Status**: ✅ **COMPLETE** (Database Schema)
- **Status**: ⚠️ **INCOMPLETE** (UI Implementation)

**Database Model** (GOOD):
```rust
pub struct Task {
    pub server_id: Option<i64>,  // NULL = local plugin task
    pub server_name: Option<String>,  // Denormalized for display
    // ... other fields
}
```

**Task Execution Logic** (GOOD):
```rust
// executor.rs lines 48-55
let result = if let Some(server_id) = task.server_id {
    // Task requires SSH execution on remote server
    execute_remote_task(state, &task, server_id).await
} else {
    // Task is local plugin execution (server_id = NULL)
    execute_plugin_task(state, &task).await
};
```

**Task UI** (EXCELLENT):
- Tasks already grouped by server (Local vs Remote)
- Shows server icon (💻 Local, 📡 Remote)
- Inline schedule editing works

### 4. Remote Execution Infrastructure
- **Status**: ✅ **COMPLETE**
- SSH module (`server/src/ssh.rs`) with:
  - `test_connection()` - Tests SSH connectivity
  - `execute_command()` - Runs commands on remote servers
  - Timeout configuration
  - Proper error handling

## ❌ What's Missing

### 1. Task Creation UI
- **Status**: ❌ **MISSING**
- No way to create tasks from the UI
- No server selection dropdown when creating tasks
- Tasks can only be created via database/plugin auto-registration

### 2. Task-to-Server Assignment UI
- **Status**: ❌ **MISSING**
- Can't assign existing tasks to different servers
- Can't create tasks for remote execution
- No UI to change task's server_id

### 3. Server Grouping/Filtering
- **Status**: ⚠️ **PARTIAL**
- Server list is flat (no grouping)
- No filtering by tags/environment
- No real-time status indicators
- Tags field exists in DB but not used in UI

### 4. Bulk Operations
- **Status**: ❌ **MISSING**
- Can't select multiple servers
- Can't perform bulk restart/enable/disable
- No multi-server task deployment

### 5. Server Status Monitoring
- **Status**: ❌ **MISSING**
- No background status checker
- No visual indicators (online/offline/unknown)
- No metrics collection (CPU, memory, disk)

---

## 🎯 Priority 1: Enable Remote Task Execution

**Problem**: "Tasks are only able to run on localhost"

**Root Cause**: No UI to create tasks with `server_id` set

### Immediate Action Items

#### 1. Add Task Creation UI (Days 1-2)

**Template**: `templates/pages/tasks.html`
```html
<button hx-get="/tasks/new"
        hx-target="#task-form-container"
        hx-swap="innerHTML"
        class="btn btn-primary">
    ➕ Create Task
</button>

<div id="task-form-container"></div>
```

**New Route**: `/tasks/new` → `task_form_new()`
```rust
async fn task_form_new(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    // Load available servers and plugins
    let db = state.db().await;
    let servers = queries::servers::list_servers(db.pool()).await?;
    let plugins = queries::plugins::list_plugins(db.pool()).await?;

    let template = TaskFormTemplate {
        task: None,
        servers,
        plugins,
        error: None,
    };
    Ok(Html(template.render()?))
}
```

**Form Template**: `templates/components/task_form.html`
```html
<form hx-post="/tasks"
      hx-target="#task-list"
      hx-swap="innerHTML"
      class="card">
    <h3>{% if task.is_some() %}Edit{% else %}Create{% endif %} Task</h3>

    <!-- Basic Info -->
    <div class="form-group">
        <label>Task Name *</label>
        <input type="text" name="name" required
               value="{% if let Some(t) = task %}{{ t.name }}{% endif %}">
    </div>

    <div class="form-group">
        <label>Description</label>
        <textarea name="description">{% if let Some(t) = task %}{{ t.description }}{% endif %}</textarea>
    </div>

    <!-- Server Selection (KEY PART) -->
    <div class="form-group">
        <label>Target Server *</label>
        <select name="server_id" required>
            <option value="">-- Select Server --</option>
            <option value="local">💻 Local (Plugin Task)</option>
            {% for server in servers %}
            <option value="{{ server.id }}"
                    {% if let Some(t) = task %}{% if t.server_id == Some(server.id) %}selected{% endif %}{% endif %}>
                📡 {{ server.name }} ({{ server.host }})
            </option>
            {% endfor %}
        </select>
        <small class="text-secondary">
            Select "Local" for plugin tasks or a remote server for SSH commands
        </small>
    </div>

    <!-- Plugin Selection (for plugin tasks) -->
    <div class="form-group" id="plugin-selection">
        <label>Plugin</label>
        <select name="plugin_id">
            <option value="">-- Select Plugin --</option>
            {% for plugin in plugins %}
            <option value="{{ plugin.id }}">{{ plugin.name }}</option>
            {% endfor %}
        </select>
    </div>

    <!-- Command (for remote tasks) -->
    <div class="form-group" id="command-input">
        <label>Command</label>
        <input type="text" name="command" placeholder="e.g., ls -la /var/log">
        <small class="text-secondary">
            Shell command to run on the remote server
        </small>
    </div>

    <!-- Schedule -->
    <div class="form-group">
        <label>Schedule (Cron Expression) *</label>
        <input type="text" name="schedule" required
               placeholder="0 */5 * * * *"
               value="{% if let Some(t) = task %}{{ t.schedule }}{% endif %}">
        <small class="text-secondary">
            Examples:
            <code>0 */5 * * * *</code> (every 5 min),
            <code>0 0 2 * * *</code> (daily 2am)
        </small>
    </div>

    <!-- Timeout -->
    <div class="form-group">
        <label>Timeout (seconds)</label>
        <input type="number" name="timeout" value="300" min="1">
    </div>

    <button type="submit" class="btn btn-primary">Save Task</button>
    <button type="button" class="btn btn-secondary"
            onclick="document.getElementById('task-form-container').innerHTML = ''">
        Cancel
    </button>
</form>

<script>
// Show/hide plugin vs command inputs based on server selection
document.querySelector('[name="server_id"]').addEventListener('change', function(e) {
    const isLocal = e.target.value === 'local';
    document.getElementById('plugin-selection').style.display = isLocal ? 'block' : 'none';
    document.getElementById('command-input').style.display = isLocal ? 'none' : 'block';
});
</script>
```

**Create Handler**: `POST /tasks`
```rust
#[derive(Debug, Deserialize)]
struct CreateTaskInput {
    name: String,
    description: Option<String>,
    server_id: String,  // "local" or ID
    plugin_id: Option<String>,
    command: Option<String>,
    schedule: String,
    timeout: Option<i32>,
}

async fn task_create(
    State(state): State<AppState>,
    Form(input): Form<CreateTaskInput>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    // Parse server_id
    let (server_id, server_name, plugin_id, command) = if input.server_id == "local" {
        // Local plugin task
        (None, Some("localhost".to_string()), input.plugin_id.unwrap(), "plugin_task")
    } else {
        // Remote SSH task
        let id = input.server_id.parse::<i64>()
            .map_err(|_| anyhow::anyhow!("Invalid server ID"))?;
        let server = queries::servers::get_server(db.pool(), id).await?;
        (Some(id), Some(server.name), "ssh".to_string(), input.command.as_deref().unwrap_or(""))
    };

    // Validate cron expression
    use cron::Schedule;
    use std::str::FromStr;
    Schedule::from_str(&input.schedule)
        .map_err(|e| anyhow::anyhow!("Invalid cron expression: {}", e))?;

    // Create task
    let create_task = CreateTask {
        name: input.name,
        description: input.description,
        plugin_id,
        server_id,
        server_name,
        schedule: input.schedule,
        command: command.to_string(),
        args: None,
        timeout: input.timeout.unwrap_or(300),
    };

    let task_id = queries::tasks::create_task(db.pool(), &create_task).await?;

    // Calculate and set next_run_at
    let next_run = queries::tasks::calculate_next_run(&create_task.schedule)?;
    queries::tasks::update_task_next_run(db.pool(), task_id, next_run).await?;

    // Reload config to pick up new task
    state.reload_config().await?;

    // Return updated task list
    let tasks = get_tasks(&state).await;
    let template = TaskListTemplate { task_groups: group_tasks(tasks) };
    Ok(Html(template.render()?))
}
```

---

## 🎯 Priority 2: Multi-Server UI Enhancements

### Server Grouping & Filtering (Week 1)

#### 1. Add Server Grouping Schema
**Migration 009**: Add environment and improve tags
```sql
-- Add environment column
ALTER TABLE servers ADD COLUMN environment TEXT DEFAULT 'production';

-- Add server status tracking
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

#### 2. Implement Server Grouping in UI
**Pattern**: Copy task grouping from `routes/ui/tasks.rs`

**Apply to servers**:
```rust
// routes/ui/servers.rs
pub struct ServerGroup {
    pub environment: String,
    pub servers: Vec<Server>,
    pub online_count: usize,
    pub total_count: usize,
}

async fn servers_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let db_servers = queries::servers::list_servers(db.pool()).await?;

    // Group by environment
    let mut groups: HashMap<String, Vec<Server>> = HashMap::new();
    for server in db_servers {
        let env = server.environment.unwrap_or_else(|| "production".to_string());
        groups.entry(env).or_default().push(db_server_to_ui(server));
    }

    // Convert to sorted groups
    let server_groups: Vec<ServerGroup> = groups.into_iter()
        .map(|(env, servers)| ServerGroup {
            environment: env.clone(),
            total_count: servers.len(),
            online_count: 0, // TODO: Get from server_status table
            servers,
        })
        .collect();

    let template = ServersTemplate { user, server_groups };
    Ok(Html(template.render()?))
}
```

**Template** (accordions):
```html
<!-- templates/pages/servers.html -->
{% for group in server_groups %}
<div class="accordion-item">
    <button class="accordion-header"
            hx-get="/servers/group/{{ group.environment }}/details"
            hx-target="#group-{{ group.environment }}-content"
            hx-swap="innerHTML"
            hx-trigger="click once">
        <span>{{ group.environment }}</span>
        <span class="badge">{{ group.online_count }}/{{ group.total_count }} online</span>
    </button>

    <div id="group-{{ group.environment }}-content" class="accordion-content">
        <!-- Lazy-loaded server cards -->
    </div>
</div>
{% endfor %}
```

#### 3. Add Background Status Checker
**New file**: `server/src/status_checker.rs`

```rust
use tokio::time::{interval, Duration};
use crate::state::AppState;

pub async fn start_status_checker(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(30));

        loop {
            ticker.tick().await;

            let db = state.db().await;
            let servers = queries::servers::list_enabled_servers(db.pool())
                .await
                .unwrap_or_default();

            for server in servers {
                let state_clone = state.clone();
                tokio::spawn(async move {
                    check_server_status(&state_clone, server.id).await;
                });
            }
        }
    });
}

async fn check_server_status(state: &AppState, server_id: i64) -> Result<()> {
    let start = std::time::Instant::now();

    let db = state.db().await;
    let server = queries::servers::get_server(db.pool(), server_id).await?;

    // Test SSH connection
    let ssh_config = crate::ssh::SshConfig {
        host: server.host.unwrap_or_default(),
        port: server.port as u16,
        username: server.username,
        key_path: server.ssh_key_path,
        timeout: Duration::from_secs(10),
    };

    let (status, error) = match crate::ssh::test_connection(&ssh_config).await {
        Ok(_) => ("online", None),
        Err(e) => ("offline", Some(e.to_string())),
    };

    // Update status in database
    queries::servers::update_server_status(
        db.pool(),
        server_id,
        status,
        start.elapsed().as_millis() as i32,
        error.as_deref(),
    ).await?;

    Ok(())
}
```

**Start in main**:
```rust
// server/src/main.rs
mod status_checker;

#[tokio::main]
async fn main() -> Result<()> {
    // ... setup ...

    // Start background status checker
    tokio::spawn(status_checker::start_status_checker(state.clone()));

    // ... start server ...
}
```

#### 4. Add Server Filtering
**Template**: `templates/pages/servers.html`
```html
<div class="server-filters"
     hx-get="/servers/list"
     hx-trigger="change from:.filter-input, input changed delay:500ms from:.filter-input"
     hx-target="#server-list"
     hx-push-url="true"
     hx-include=".filter-input">

    <input type="text"
           name="search"
           class="filter-input"
           placeholder="Search servers...">

    <select name="environment" class="filter-input">
        <option value="">All Environments</option>
        <option value="production">Production</option>
        <option value="staging">Staging</option>
        <option value="development">Development</option>
    </select>

    <select name="status" class="filter-input">
        <option value="">All Statuses</option>
        <option value="online">Online</option>
        <option value="offline">Offline</option>
    </select>
</div>
```

---

## 📊 Implementation Timeline

### Week 1: Enable Remote Tasks
**Days 1-2**: Task creation UI with server selection
- Add task creation form
- Server dropdown in form
- Plugin vs command logic

**Days 3-4**: Test remote execution
- Create tasks for remote servers
- Run tasks manually from UI
- Verify SSH execution works
- Fix any bugs in remote executor

**Day 5**: Multi-server task deployment
- Create same task on multiple servers
- Bulk task creation UI

### Week 2: Server UI Enhancements
**Days 1-2**: Server grouping
- Migration 009 (environment, server_status)
- Group servers by environment
- Accordion UI with lazy loading

**Days 3-4**: Status monitoring
- Background status checker
- Real-time status indicators
- Visual feedback (🟢 online, 🔴 offline)

**Day 5**: Server filtering & sorting
- Filter by environment, status, search
- Query parameter persistence
- Bookmarkable URLs

### Week 3: Bulk Operations & Polish
**Days 1-2**: Bulk operations
- Multi-select servers (checkboxes)
- Bulk restart/enable/disable
- Progress indicators

**Days 3-5**: Dashboard enhancement
- Real-time metrics
- Server status grid
- Activity feed

---

## 🚀 Immediate Next Step

**START HERE**: Enable task creation with server selection

1. Create `templates/components/task_form.html`
2. Add route `/tasks/new` → `task_form_new()`
3. Add route `POST /tasks` → `task_create()`
4. Add "Create Task" button to tasks page
5. Test creating both:
   - Local plugin task (server_id = NULL)
   - Remote SSH task (server_id = Some(id))

This unblocks remote server functionality testing immediately.

---

**Status**: Ready to implement
**Next Review**: After task creation UI is working
