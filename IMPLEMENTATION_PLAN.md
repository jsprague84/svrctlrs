# SvrCtlRS Implementation Plan
## Porting Weatherust Functionality to Plugin Architecture

### Overview

This document outlines the plan to port all weatherust functionality into the new SvrCtlRS plugin-based architecture.

### Weatherust Feature Matrix

| Feature | Type | Schedule | Dependencies | Status |
|---------|------|----------|--------------|--------|
| **Weather Monitoring** | Plugin | Daily 05:30 | OpenWeatherMap API | 🔴 To Implement |
| **Speed Test** | Plugin | Daily 02:10 | Ookla CLI | 🔴 To Implement |
| **Docker Health** | Plugin | Every 5 min | Bollard | 🔴 To Implement |
| **Update Monitoring** | Plugin | Daily 03:00 | SSH, Docker | 🔴 To Implement |
| **Update Control** | Plugin | Manual/Webhook | SSH, Docker | 🔴 To Implement |
| **Docker Cleanup** | Feature | Weekly | Docker | 🔴 To Implement |
| **OS Cleanup** | Feature | Manual | SSH | 🔴 To Implement |
| **Webhook Server** | Core | Always | Axum | 🟡 Partial |
| **Notifications** | Core | N/A | HTTP client | 🔴 To Implement |

---

## Phase 1: Core Infrastructure (Foundation)

### 1.1 Notification Backends
**Location**: `core/src/notifications.rs`

Port from `common/src/lib.rs`:
- ✅ Gotify backend implementation
- ✅ ntfy.sh backend implementation
- ✅ Service-specific key/topic resolution
- ✅ Token masking for debug output
- ✅ Retry logic with exponential backoff
- ✅ Action button support (ntfy)

**Key Features**:
```rust
pub struct GotifyBackend {
    client: Client,
    base_url: String,
    keys: HashMap<String, String>, // service -> key
}

pub struct NtfyBackend {
    client: Client,
    base_url: String,
    topics: HashMap<String, String>, // service -> topic
}
```

### 1.2 Enhanced Remote Executor
**Location**: `core/src/remote.rs`

Enhance existing `RemoteExecutor`:
- ✅ Connection pooling for SSH
- ✅ Timeout configuration
- ✅ Better error handling with context
- ✅ Support for sudo commands
- ✅ Command output streaming

### 1.3 Database Schema
**Location**: `database/src/migrations/`

Add tables:
```sql
-- Server metrics history
CREATE TABLE metrics_history (
    id INTEGER PRIMARY KEY,
    server_id INTEGER,
    plugin_id TEXT,
    metric_type TEXT,
    value REAL,
    metadata JSON,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Notification log
CREATE TABLE notification_log (
    id INTEGER PRIMARY KEY,
    service TEXT,
    backend TEXT,
    title TEXT,
    success BOOLEAN,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Webhook invocations
CREATE TABLE webhook_log (
    id INTEGER PRIMARY KEY,
    endpoint TEXT,
    server TEXT,
    action TEXT,
    success BOOLEAN,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## Phase 2: Plugin Implementations

### 2.1 Docker Plugin (High Priority)
**Location**: `plugins/docker/`

**Features**:
1. **Health Monitoring** (from healthmon)
   - Container state tracking
   - Health check status
   - CPU/Memory thresholds
   - Restart count monitoring
   - Ignore list support

2. **Docker Cleanup** (from updatectl cleanup/)
   - Dangling images analysis
   - Stopped containers cleanup
   - Unused volumes
   - Unused networks
   - Build cache analysis
   - Container logs rotation
   - Image layers analysis
   - Cleanup profiles (conservative/moderate/aggressive)

3. **Image Update Monitoring** (from updatemon)
   - Available image updates
   - Tag comparison
   - Digest comparison

**Dependencies**:
```toml
bollard = "0.18" # Docker API
```

**Module Structure**:
```
plugins/docker/
├── src/
│   ├── lib.rs              # Plugin implementation
│   ├── health.rs           # Health monitoring
│   ├── cleanup/
│   │   ├── mod.rs          # Cleanup orchestration
│   │   ├── images.rs       # Image cleanup
│   │   ├── containers.rs   # Container cleanup
│   │   ├── volumes.rs      # Volume cleanup
│   │   ├── networks.rs     # Network cleanup
│   │   ├── build_cache.rs  # Build cache
│   │   ├── logs.rs         # Log rotation
│   │   ├── layers.rs       # Layer analysis
│   │   └── profiles.rs     # Cleanup profiles
│   └── updates.rs          # Update checking
```

**Scheduled Tasks**:
- `docker_health`: Every 5 minutes
- `docker_cleanup_analysis`: Weekly (Sundays 02:00)
- `docker_update_check`: Daily (03:00)

### 2.2 Updates Plugin (High Priority)
**Location**: `plugins/updates/`

**Features**:
1. **OS Update Monitoring** (from updatemon)
   - apt-based systems (Ubuntu/Debian)
   - dnf-based systems (Fedora/RHEL)
   - Parse available updates
   - Security update detection

2. **OS Update Execution** (from updatectl)
   - apt update/upgrade
   - dnf update
   - Kernel update handling
   - Reboot detection

3. **OS Cleanup** (from updatectl)
   - Package cache cleanup
   - Old kernel removal
   - Autoremove orphaned packages

**Module Structure**:
```
plugins/updates/
├── src/
│   ├── lib.rs          # Plugin implementation
│   ├── monitor.rs      # Update monitoring
│   ├── executor.rs     # Update execution
│   ├── os_cleanup.rs   # OS cleanup
│   └── parsers/
│       ├── apt.rs      # apt parser
│       └── dnf.rs      # dnf parser
```

**Scheduled Tasks**:
- `os_update_check`: Daily (03:00)
- `docker_update_check`: Daily (03:00)

### 2.3 Health Plugin (Medium Priority)
**Location**: `plugins/health/`

**Features**:
1. **System Metrics**
   - CPU usage
   - Memory usage
   - Disk space
   - Network I/O
   - Load average

2. **Service Health**
   - systemd service monitoring
   - Process monitoring

**Dependencies**:
```toml
sysinfo = "0.32" # System metrics
```

**Module Structure**:
```
plugins/health/
├── src/
│   ├── lib.rs          # Plugin implementation
│   ├── system.rs       # System metrics
│   └── services.rs     # Service monitoring
```

**Scheduled Tasks**:
- `system_metrics`: Every 5 minutes

### 2.4 Weather Plugin (Low Priority)
**Location**: `plugins/weather/`

**Features**:
- OpenWeatherMap API integration
- Current weather + forecast
- ZIP code / city lookup
- Configurable units

**Dependencies**:
```toml
reqwest = { workspace = true }
serde_json = { workspace = true }
```

**Scheduled Tasks**:
- `weather_report`: Daily (05:30)

### 2.5 Speed Test Plugin (Low Priority)
**Location**: `plugins/speedtest/`

**Features**:
- Ookla Speedtest CLI integration
- Download/upload speed
- Threshold alerts
- Historical tracking

**Scheduled Tasks**:
- `speed_test`: Daily (02:10)

---

## Phase 3: Server Enhancements

### 3.1 Webhook Endpoints
**Location**: `server/src/routes/webhook.rs`

Port from `updatectl/src/webhook.rs`:

**Endpoints**:
```
POST /webhook/docker/cleanup
POST /webhook/docker/update
POST /webhook/os/update
POST /webhook/os/clean-cache
POST /webhook/os/autoremove
```

**Features**:
- Token-based authentication (constant-time comparison)
- Server-specific actions
- Background task execution
- Notification on completion

### 3.2 API Endpoints
**Location**: `server/src/routes/api.rs`

**Endpoints**:
```
GET  /api/health              # Server health
GET  /api/servers             # List servers
GET  /api/servers/:id         # Server details
GET  /api/servers/:id/metrics # Server metrics
GET  /api/plugins             # List plugins
GET  /api/plugins/:id         # Plugin details
GET  /api/tasks               # Scheduled tasks
GET  /api/tasks/:id/history   # Task execution history
POST /api/tasks/:id/trigger   # Manual task trigger
```

### 3.3 CLI Interface
**Location**: `server/src/main.rs`

Add subcommands (similar to updatectl):
```bash
server run                      # Start web server (default)
server list servers             # List configured servers
server list plugins             # List available plugins
server list tasks               # List scheduled tasks
server trigger <task>           # Manually trigger a task
server exec <plugin> <task>     # Execute plugin task
```

---

## Phase 4: Dioxus UI (Future)

### 4.1 Dashboard
- Server overview cards
- Recent activity feed
- Health status indicators
- Quick actions

### 4.2 Server Management
- Server list/detail pages
- Metrics visualization
- Historical data charts

### 4.3 Plugin Management
- Enable/disable plugins
- Configure plugin settings
- View plugin status

### 4.4 Task Scheduling
- View scheduled tasks
- Edit schedules
- Manual task execution

---

## Implementation Order

### Sprint 1: Foundation (Week 1)
1. ✅ Project structure (DONE)
2. 🔴 Notification backends (Gotify + ntfy)
3. 🔴 Enhanced remote executor
4. 🔴 Database migrations
5. 🔴 Basic webhook framework

### Sprint 2: Docker Plugin (Week 2)
1. 🔴 Health monitoring
2. 🔴 Cleanup analysis (all 9 modules)
3. 🔴 Update checking
4. 🔴 Integration with scheduler

### Sprint 3: Updates Plugin (Week 3)
1. 🔴 OS update monitoring
2. 🔴 OS update execution
3. 🔴 OS cleanup operations
4. 🔴 Docker update integration

### Sprint 4: Infrastructure (Week 4)
1. 🔴 Webhook endpoints
2. 🔴 API endpoints
3. 🔴 CLI subcommands
4. 🔴 Health plugin basics

### Sprint 5: Polish (Week 5)
1. 🔴 Weather plugin
2. 🔴 Speed test plugin
3. 🔴 Testing & documentation
4. 🔴 Docker images & deployment

### Sprint 6: UI (Future)
1. 🔴 Dioxus dashboard
2. 🔴 Server management UI
3. 🔴 Plugin configuration UI

---

## Migration Strategy

### Backwards Compatibility
- Keep weatherust running during development
- Test SvrCtlRS alongside weatherust
- Gradual migration of scheduled tasks

### Testing Approach
1. Unit tests for each plugin
2. Integration tests for remote execution
3. End-to-end tests with Docker
4. Manual testing on test server (docker-vm)

### Deployment Plan
1. Deploy SvrCtlRS to docker-vm
2. Run both systems in parallel for 1 week
3. Compare notifications and outputs
4. Gradually disable weatherust services
5. Full cutover once validated

---

## Success Criteria

- ✅ All weatherust features replicated
- ✅ Plugin architecture working
- ✅ Notifications identical to weatherust
- ✅ Webhooks functional
- ✅ Performance equal or better
- ✅ Database storing metrics
- ✅ CLI working for manual operations
- ✅ Documentation complete

---

## Next Immediate Steps

1. **Implement notification backends** (Sprint 1, Item 2)
2. **Test notifications** with existing services
3. **Port Docker health monitoring** (Sprint 2, Item 1)
4. **Add webhook framework** (Sprint 1, Item 5)

---

*Last Updated: 2025-01-23*
