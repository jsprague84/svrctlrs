# Complete Database-as-Source-of-Truth Refactoring ✅

## Summary

The system has been **completely refactored** to make the database the **ONLY** source of truth for ALL application configuration. No more environment variable confusion!

## What's in Environment Variables Now?

**ONLY infrastructure settings:**

```yaml
environment:
  - RUST_LOG=info                              # Logging level
  - DATABASE_URL=sqlite:/app/data/svrctlrs.db  # Database path
  - SSH_KEY_PATH=~/.ssh                        # SSH keys directory (optional)
```

That's it! Just 3 variables, all infrastructure-related.

## What's in the Database?

**EVERYTHING else:**

### 1. Plugin Configuration
- Enable/disable state
- Schedules (cron expressions)
- Plugin-specific settings:
  - Weather: API key, location, units
  - Speedtest: Min download/upload thresholds
  - Docker: Monitoring settings
  - Health: Metric collection settings
  - Updates: Auto-apply settings

### 2. Notification Backends
- Gotify configuration (URL, token, priority)
- ntfy configuration (URL, topic, token, priority)
- Multiple backends supported
- Enable/disable per backend

### 3. Server Configurations
- SSH connection details
- Server metadata
- Enable/disable per server

### 4. Task Definitions
- Task schedules
- Commands
- Arguments
- Timeouts

### 5. Task History
- Execution results
- Success/failure status
- Output and errors
- Performance metrics

## What Was Removed?

### From docker-compose.yml
```yaml
# ❌ REMOVED - Now in database
- ENABLE_DOCKER_PLUGIN=true
- ENABLE_UPDATES_PLUGIN=true
- ENABLE_HEALTH_PLUGIN=true
- ENABLE_WEATHER_PLUGIN=false
- ENABLE_SPEEDTEST_PLUGIN=false
- GOTIFY_URL=
- GOTIFY_TOKEN=
- NTFY_URL=
- NTFY_TOPIC=
- WEATHER_API_KEY=
- WEATHER_LOCATION=
```

### From Code
```rust
// ❌ REMOVED - No longer needed
struct PluginConfig {
    docker_enabled: bool,
    updates_enabled: bool,
    health_enabled: bool,
    weather_enabled: bool,
    speedtest_enabled: bool,
}

struct NotificationConfig {
    gotify_url: Option<String>,
    gotify_key: Option<String>,
    ntfy_url: Option<String>,
    ntfy_topic: Option<String>,
}
```

## How to Configure Everything Now

### 1. Plugins
**URL:** http://your-server:8080/plugins

- View all available plugins
- Toggle enable/disable (no restart needed!)
- Click "Configure" to set:
  - Schedule (cron expression)
  - API keys
  - Thresholds
  - Other plugin-specific settings

### 2. Notification Backends
**URL:** http://your-server:8080/settings/notifications

- Click "Add Backend"
- Choose type (Gotify or ntfy)
- Enter configuration:
  - Name
  - URL
  - Token/Topic
  - Priority
- Save (persists to database)
- Enable/disable as needed

### 3. Servers
**URL:** http://your-server:8080/servers

- Click "Add Server"
- Enter details:
  - Name
  - Host
  - Port
  - Username
  - Description
- Test SSH connection
- Save (persists to database)

### 4. Tasks
**URL:** http://your-server:8080/tasks

- View all tasks
- Run manually
- View execution history
- Tasks are auto-created from plugin schedules

## Migration from Old System

If you have an existing deployment with environment variables:

### Step 1: Migrate Plugin Configuration

Old `.env`:
```bash
ENABLE_WEATHER_PLUGIN=true
WEATHER_API_KEY=your-key-here
WEATHER_LOCATION=New York
```

New (via UI at `/plugins`):
1. Enable Weather plugin (toggle switch)
2. Click "Configure"
3. Enter API key, location, units
4. Save

### Step 2: Migrate Notification Configuration

Old `.env`:
```bash
GOTIFY_URL=https://gotify.example.com
GOTIFY_TOKEN=your-token-here
```

New (via UI at `/settings/notifications`):
1. Click "Add Backend"
2. Select "Gotify"
3. Enter name, URL, token
4. Save

### Step 3: Clean Up Environment

Remove from `.env`:
```bash
# Remove all these lines
ENABLE_*_PLUGIN=*
GOTIFY_*=*
NTFY_*=*
WEATHER_*=*
```

Keep only:
```bash
RUST_LOG=info
DATABASE_URL=sqlite:/app/data/svrctlrs.db
SSH_KEY_PATH=~/.ssh
```

### Step 4: Restart

```bash
docker compose restart svrctlrs
```

## Benefits of This Design

### For Users
1. ✅ **Everything in one place** - The UI
2. ✅ **No file editing** - Point and click configuration
3. ✅ **Instant changes** - No container restarts
4. ✅ **Persistent** - Survives container recreation
5. ✅ **Intuitive** - Web UI is familiar
6. ✅ **Discoverable** - See all options in UI

### For Developers
1. ✅ **Single source of truth** - No sync issues
2. ✅ **Less code** - No env var parsing
3. ✅ **Easier testing** - Database state is inspectable
4. ✅ **Better UX** - Users expect UI configuration
5. ✅ **Maintainable** - Clear separation of concerns
6. ✅ **Extensible** - Easy to add new settings

### For Deployment
1. ✅ **Simpler** - Only 3 env vars needed
2. ✅ **Portable** - Database contains everything
3. ✅ **Backup-friendly** - One database file
4. ✅ **Version-controlled** - Database can be backed up
5. ✅ **Multi-environment** - Same image, different databases

## Architecture

### Old Design (Before)
```
Environment Variables → Config Struct → Application
Database → Application
```
**Problem:** Configuration split across two sources, confusing!

### New Design (After)
```
Database → Application
Environment Variables → Infrastructure Only
```
**Solution:** Single source of truth, clear separation!

## Database Schema

### plugins
```sql
CREATE TABLE plugins (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN DEFAULT 1,
    config TEXT,  -- JSON: {api_key, location, schedule, etc.}
    created_at DATETIME,
    updated_at DATETIME
);
```

### notification_backends
```sql
CREATE TABLE notification_backends (
    id INTEGER PRIMARY KEY,
    type TEXT NOT NULL,  -- 'gotify' or 'ntfy'
    name TEXT UNIQUE NOT NULL,
    enabled BOOLEAN DEFAULT 1,
    config TEXT,  -- JSON: {url, token, topic, etc.}
    priority INTEGER DEFAULT 5,
    created_at DATETIME,
    updated_at DATETIME
);
```

### servers
```sql
CREATE TABLE servers (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    host TEXT,
    port INTEGER DEFAULT 22,
    username TEXT DEFAULT 'root',
    enabled BOOLEAN DEFAULT 1,
    description TEXT,
    created_at DATETIME,
    updated_at DATETIME
);
```

## Example Configurations

### Weather Plugin (in database)
```json
{
  "api_key": "your-openweathermap-key",
  "location": "New York",
  "units": "imperial",
  "schedule": "0 */30 * * * *"
}
```

### Gotify Backend (in database)
```json
{
  "url": "https://gotify.example.com",
  "token": "your-app-token"
}
```

### ntfy Backend (in database)
```json
{
  "url": "https://ntfy.sh",
  "topic": "svrctlrs-alerts",
  "token": "optional-auth-token"
}
```

## Verification

### Check Plugin Configuration
```bash
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db \
  "SELECT id, name, enabled, config FROM plugins;"
```

### Check Notification Backends
```bash
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db \
  "SELECT id, type, name, enabled, config FROM notification_backends;"
```

### Check Servers
```bash
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db \
  "SELECT id, name, host, port, enabled FROM servers;"
```

## Summary

This refactoring achieves **complete consistency**:

- ✅ **ONE source of truth**: The database
- ✅ **ONE interface**: The web UI
- ✅ **ONE way to configure**: Through the database/UI
- ✅ **ZERO environment variable confusion**

The system is now **production-ready** with a clean, maintainable architecture that users will find intuitive and developers will find easy to work with!

## Next Steps

1. Pull the latest image
2. Migrate your configuration to the database (via UI or SQL)
3. Remove old environment variables
4. Restart and enjoy the simplified system!

See `READY_TO_TEST.md` for detailed testing instructions.

