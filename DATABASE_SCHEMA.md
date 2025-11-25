# SvrCtlRS Database Schema Design

## Overview

This document defines the database schema for SvrCtlRS to replace environment variable configuration with a web-based UI configuration system. The schema supports:

- Server management (SSH connections, credentials)
- Plugin configuration (enable/disable, settings)
- Notification settings (Gotify, ntfy.sh)
- Task scheduling and history
- User authentication and authorization
- Application settings

## Database: SQLite

**Rationale**: 
- Embedded database (no separate server needed)
- ACID compliant
- Good performance for single-server deployments
- Easy backup (single file)
- Already in use via `sqlx`

---

## Core Tables

### 1. `servers`

Stores remote servers to be monitored and managed.

```sql
CREATE TABLE servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    host TEXT NOT NULL,  -- hostname or IP
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL DEFAULT 'root',
    ssh_key_path TEXT,  -- Path to SSH key (optional, can use default)
    enabled BOOLEAN NOT NULL DEFAULT 1,
    description TEXT,
    tags TEXT,  -- JSON array of tags for grouping
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at DATETIME,  -- Last successful connection
    
    -- Metadata
    os_type TEXT,  -- 'debian', 'fedora', 'arch', etc.
    os_version TEXT,
    docker_installed BOOLEAN DEFAULT 0,
    
    -- Connection settings
    connection_timeout INTEGER DEFAULT 30,  -- seconds
    retry_attempts INTEGER DEFAULT 3,
    
    CONSTRAINT chk_port CHECK (port > 0 AND port <= 65535)
);

CREATE INDEX idx_servers_enabled ON servers(enabled);
CREATE INDEX idx_servers_last_seen ON servers(last_seen_at);
```

**Replaces**: `UPDATE_SERVERS` env var (comma-separated list)

**UI Integration**: 
- ✅ Already have "Add Server" form
- Need to connect to database
- Add edit/delete functionality

---

### 2. `plugins`

Stores plugin configuration and state.

```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,  -- 'docker', 'updates', 'health', 'weather', 'speedtest'
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    config JSON,  -- Plugin-specific configuration
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Seed data
INSERT INTO plugins (id, name, description, enabled, config) VALUES
('docker', 'Docker Monitor', 'Monitor Docker containers and images', 1, '{}'),
('updates', 'Updates Manager', 'Monitor and manage OS updates', 1, '{}'),
('health', 'System Health', 'Monitor system resources', 1, '{}'),
('weather', 'Weather Monitor', 'OpenWeatherMap integration', 0, '{"api_key": "", "location": ""}'),
('speedtest', 'Speed Test', 'Internet speed monitoring', 0, '{"min_down": 100, "min_up": 20}');
```

**Config Examples**:

```json
// Docker plugin
{
  "check_health": true,
  "check_updates": true,
  "alert_on_unhealthy": true,
  "alert_on_updates": false
}

// Weather plugin
{
  "api_key": "your-openweathermap-key",
  "location": "New York,US",
  "units": "imperial"
}

// Speedtest plugin
{
  "min_down": 100,
  "min_up": 20,
  "server_id": null
}
```

**Replaces**: Feature flags and plugin-specific env vars

---

### 3. `notifications`

Stores notification backend configurations.

```sql
CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,  -- 'gotify', 'ntfy'
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    config JSON NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,  -- 1-10, higher = more important
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT chk_type CHECK (type IN ('gotify', 'ntfy')),
    CONSTRAINT chk_priority CHECK (priority >= 1 AND priority <= 10)
);

CREATE INDEX idx_notifications_enabled ON notifications(enabled);
```

**Config Examples**:

```json
// Gotify
{
  "url": "http://gotify:8080/message",
  "token": "your-gotify-token",
  "priority": 5
}

// ntfy.sh
{
  "url": "https://ntfy.sh",
  "topic": "svrctlrs-alerts",
  "username": "",  // optional
  "password": ""   // optional
}
```

**Replaces**: 
- `GOTIFY_URL`, `GOTIFY_KEY`, `GOTIFY_KEY_FILE`
- `NTFY_URL`, `NTFY_TOPIC`

---

### 4. `tasks`

Stores scheduled tasks and their configuration.

```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    plugin_id TEXT NOT NULL,  -- References plugins(id)
    server_id INTEGER,  -- NULL = all servers, or specific server
    
    -- Scheduling
    schedule TEXT NOT NULL,  -- Cron expression
    enabled BOOLEAN NOT NULL DEFAULT 1,
    
    -- Task configuration
    command TEXT NOT NULL,  -- Command to execute
    args JSON,  -- Command arguments
    timeout INTEGER DEFAULT 300,  -- seconds
    
    -- Metadata
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_run_at DATETIME,
    next_run_at DATETIME,
    run_count INTEGER NOT NULL DEFAULT 0,
    
    FOREIGN KEY (plugin_id) REFERENCES plugins(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_tasks_enabled ON tasks(enabled);
CREATE INDEX idx_tasks_next_run ON tasks(next_run_at);
CREATE INDEX idx_tasks_plugin ON tasks(plugin_id);
```

**Replaces**: Ofelia labels in docker-compose.yml

**Example Tasks**:
```sql
-- Weather check daily at 5:30 AM
INSERT INTO tasks (name, plugin_id, schedule, command, args) VALUES
('Weather Check', 'weather', '0 30 5 * * *', 'check_weather', '{"zip": "52726", "units": "imperial"}');

-- Docker health check every 5 minutes
INSERT INTO tasks (name, plugin_id, schedule, command, args) VALUES
('Docker Health', 'docker', '0 */5 * * * *', 'check_health', '{}');

-- Update check daily at 3:00 AM
INSERT INTO tasks (name, plugin_id, schedule, command, args) VALUES
('Update Check', 'updates', '0 0 3 * * *', 'check_updates', '{"check_docker": true}');
```

---

### 5. `task_history`

Stores task execution history and results.

```sql
CREATE TABLE task_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    server_id INTEGER,  -- NULL if task ran locally
    
    -- Execution details
    started_at DATETIME NOT NULL,
    finished_at DATETIME,
    duration_ms INTEGER,
    status TEXT NOT NULL,  -- 'success', 'failed', 'timeout', 'cancelled'
    exit_code INTEGER,
    
    -- Output
    stdout TEXT,
    stderr TEXT,
    error_message TEXT,
    
    -- Metadata
    triggered_by TEXT DEFAULT 'schedule',  -- 'schedule', 'manual', 'webhook'
    
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE SET NULL,
    
    CONSTRAINT chk_status CHECK (status IN ('success', 'failed', 'timeout', 'cancelled'))
);

CREATE INDEX idx_task_history_task ON task_history(task_id);
CREATE INDEX idx_task_history_started ON task_history(started_at DESC);
CREATE INDEX idx_task_history_status ON task_history(status);
```

**Use Cases**:
- View task execution history
- Debug failed tasks
- Performance metrics
- Audit trail

---

### 6. `settings`

Stores global application settings (key-value store).

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'string',  -- 'string', 'number', 'boolean', 'json'
    description TEXT,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT chk_type CHECK (type IN ('string', 'number', 'boolean', 'json'))
);

-- Seed data
INSERT INTO settings (key, value, type, description) VALUES
('app_name', 'SvrCtlRS', 'string', 'Application name'),
('timezone', 'America/Chicago', 'string', 'Default timezone'),
('log_level', 'info', 'string', 'Logging level'),
('retention_days', '30', 'number', 'Days to keep task history'),
('default_ssh_key', '/home/svrctlrs/.ssh/id_rsa', 'string', 'Default SSH key path'),
('webhook_secret', '', 'string', 'Webhook authentication secret'),
('enable_notifications', 'true', 'boolean', 'Enable notification system');
```

**Replaces**: Various env vars like `TZ`, `RUST_LOG`, etc.

---

### 7. `users` (Authentication - Phase 2)

Stores user accounts for web UI authentication.

```sql
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT UNIQUE,
    password_hash TEXT NOT NULL,  -- bcrypt hash
    full_name TEXT,
    
    -- Authorization
    role TEXT NOT NULL DEFAULT 'user',  -- 'admin', 'user', 'viewer'
    enabled BOOLEAN NOT NULL DEFAULT 1,
    
    -- Session management
    last_login_at DATETIME,
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until DATETIME,
    
    -- Metadata
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CONSTRAINT chk_role CHECK (role IN ('admin', 'user', 'viewer'))
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_enabled ON users(enabled);

-- Default admin user (password: 'admin' - CHANGE IMMEDIATELY)
INSERT INTO users (username, password_hash, role) VALUES
('admin', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYqKqz8Iqey', 'admin');
```

**Roles**:
- `admin`: Full access (manage users, settings, servers, tasks)
- `user`: Can manage servers and tasks, view settings
- `viewer`: Read-only access

---

### 8. `sessions` (Authentication - Phase 2)

Stores user sessions (managed by `tower-sessions`).

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    expires_at DATETIME NOT NULL,
    data TEXT,  -- JSON session data
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
```

---

### 9. `audit_log` (Optional - Phase 3)

Stores audit trail of user actions.

```sql
CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER,
    action TEXT NOT NULL,  -- 'create', 'update', 'delete', 'login', 'logout'
    resource_type TEXT NOT NULL,  -- 'server', 'task', 'plugin', 'user', 'settings'
    resource_id TEXT,
    details JSON,  -- What changed
    ip_address TEXT,
    user_agent TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_audit_log_user ON audit_log(user_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at DESC);
CREATE INDEX idx_audit_log_resource ON audit_log(resource_type, resource_id);
```

---

## Migration Strategy

### Phase 1: Core Configuration (Immediate)

1. **Database Setup**
   - Create migration files in `database/migrations/`
   - Use `sqlx migrate` for schema management

2. **Implement Tables**:
   - ✅ `servers` - Replace `UPDATE_SERVERS` env var
   - ✅ `plugins` - Replace plugin feature flags
   - ✅ `notifications` - Replace Gotify/ntfy env vars
   - ✅ `tasks` - Replace Ofelia scheduling
   - ✅ `task_history` - New feature
   - ✅ `settings` - Replace misc env vars

3. **Update Backend**:
   - Modify `server/src/state.rs` to load from database
   - Update plugin initialization to read from DB
   - Implement CRUD API routes for each table

4. **Update UI**:
   - ✅ Server management (already have form)
   - Add plugin configuration page
   - Add notification settings page
   - Add task management page
   - Add settings page

### Phase 2: Authentication (Next Sprint)

1. **Implement Tables**:
   - ✅ `users`
   - ✅ `sessions`

2. **Backend**:
   - Implement password hashing (bcrypt)
   - Add login/logout routes
   - Add session middleware
   - Protect routes with auth guards

3. **UI**:
   - ✅ Login page (already exists)
   - Add user management page (admin only)
   - Add "change password" functionality

### Phase 3: Advanced Features (Future)

1. **Implement Tables**:
   - ✅ `audit_log`

2. **Features**:
   - Audit trail viewer
   - User activity monitoring
   - Advanced permissions
   - API keys for external integrations

---

## Environment Variable Migration

### Current (weatherust .env):

```bash
# Servers
UPDATE_SERVERS=server1:user@host1,server2:user@host2
UPDATE_SSH_KEY=/path/to/key

# Notifications
GOTIFY_URL=http://gotify:8080/message
GOTIFY_KEY=token
NTFY_URL=https://ntfy.sh
NTFY_TOPIC=alerts

# Weather
WEATHER_API_KEY=key
WEATHER_ZIP=52726
WEATHER_UNITS=imperial

# Speedtest
SPEEDTEST_MIN_DOWN=100
SPEEDTEST_MIN_UP=20

# Webhook
UPDATECTL_WEBHOOK_URL=http://webhook:8080
UPDATECTL_WEBHOOK_SECRET=secret
```

### New (SvrCtlRS database):

All configuration moves to the database and is managed via the web UI. Only essential runtime env vars remain:

```bash
# Runtime only
RUST_LOG=info
DATABASE_URL=sqlite:/app/data/svrctlrs.db
```

---

## API Routes (CRUD)

### Servers
- `GET /api/servers` - List all servers
- `POST /api/servers` - Create server
- `GET /api/servers/{id}` - Get server details
- `PUT /api/servers/{id}` - Update server
- `DELETE /api/servers/{id}` - Delete server
- `POST /api/servers/{id}/test` - Test SSH connection

### Plugins
- `GET /api/plugins` - List all plugins
- `PUT /api/plugins/{id}` - Update plugin config
- `POST /api/plugins/{id}/toggle` - Enable/disable plugin

### Notifications
- `GET /api/notifications` - List notification backends
- `POST /api/notifications` - Create notification backend
- `PUT /api/notifications/{id}` - Update notification backend
- `DELETE /api/notifications/{id}` - Delete notification backend
- `POST /api/notifications/{id}/test` - Send test notification

### Tasks
- `GET /api/tasks` - List all tasks
- `POST /api/tasks` - Create task
- `GET /api/tasks/{id}` - Get task details
- `PUT /api/tasks/{id}` - Update task
- `DELETE /api/tasks/{id}` - Delete task
- `POST /api/tasks/{id}/run` - Manually trigger task
- `GET /api/tasks/{id}/history` - Get task execution history

### Settings
- `GET /api/settings` - Get all settings
- `PUT /api/settings/{key}` - Update setting

### Users (Phase 2)
- `GET /api/users` - List users (admin only)
- `POST /api/users` - Create user (admin only)
- `PUT /api/users/{id}` - Update user
- `DELETE /api/users/{id}` - Delete user (admin only)
- `POST /api/auth/login` - Login
- `POST /api/auth/logout` - Logout
- `POST /api/auth/change-password` - Change own password

---

## Benefits of Database Approach

1. **User-Friendly**: Web UI instead of editing text files
2. **Validation**: Database constraints ensure data integrity
3. **Audit Trail**: Track who changed what and when
4. **Flexibility**: Easy to add new fields without changing env vars
5. **Multi-User**: Support multiple users with different permissions
6. **Backup**: Single SQLite file to backup
7. **History**: Track task execution history
8. **Dynamic**: Change configuration without restarting
9. **Secure**: Credentials stored securely, not in plain text env vars
10. **Scalable**: Easy to add new features (tags, groups, etc.)

---

## Next Steps

1. **Create Migration Files**: Define SQL migrations in `database/migrations/`
2. **Update Database Crate**: Add models and queries
3. **Implement CRUD Routes**: Backend API for each table
4. **Build UI Pages**: Forms for managing each entity
5. **Migrate Existing Config**: Tool to import from `config.toml` to database
6. **Testing**: Ensure all functionality works with database
7. **Documentation**: Update QUICKSTART.md and README.md

---

## File Structure

```
database/
├── migrations/
│   ├── 001_create_servers.sql
│   ├── 002_create_plugins.sql
│   ├── 003_create_notifications.sql
│   ├── 004_create_tasks.sql
│   ├── 005_create_task_history.sql
│   ├── 006_create_settings.sql
│   ├── 007_create_users.sql
│   └── 008_create_sessions.sql
└── src/
    ├── lib.rs
    ├── models/
    │   ├── mod.rs
    │   ├── server.rs
    │   ├── plugin.rs
    │   ├── notification.rs
    │   ├── task.rs
    │   └── user.rs
    └── queries/
        ├── mod.rs
        ├── servers.rs
        ├── plugins.rs
        ├── notifications.rs
        ├── tasks.rs
        └── users.rs
```

This schema provides a solid foundation for a fully-featured web-based configuration system!

