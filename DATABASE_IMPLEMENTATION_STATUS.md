# Database Implementation Status

## ✅ Completed (Phase 1A)

### 1. SQL Migrations Created
- ✅ `001_extend_servers.sql` - Extends existing servers table with full configuration
- ✅ `002_create_plugins.sql` - Creates plugins table with seed data
- ✅ `003_create_notification_backends.sql` - Creates notification backends table
- ✅ `004_create_tasks.sql` - Creates tasks table for scheduling
- ✅ `005_extend_task_history.sql` - Extends existing task_history table
- ✅ `006_create_settings.sql` - Creates settings table with seed data

### 2. Database Models Created
- ✅ `models/server.rs` - Server, CreateServer, UpdateServer
- ✅ `models/plugin.rs` - Plugin, UpdatePlugin
- ✅ `models/notification.rs` - NotificationBackend, CreateNotificationBackend, UpdateNotificationBackend
- ✅ `models/task.rs` - Task, CreateTask, UpdateTask, TaskHistory
- ✅ `models/setting.rs` - Setting, UpdateSetting

**Location**: `database/src/models/`

---

## 🚧 In Progress (Phase 1B)

### 3. Database Queries (Next)
Need to create query modules for CRUD operations:

- `queries/servers.rs` - Server CRUD operations
- `queries/plugins.rs` - Plugin CRUD operations
- `queries/notifications.rs` - Notification backend CRUD operations
- `queries/tasks.rs` - Task CRUD operations
- `queries/settings.rs` - Settings CRUD operations

### 4. Update Database Lib (Next)
- Update `database/src/lib.rs` to export models
- Add migration runner to execute SQL files
- Update `Database::migrate()` to run new migrations

---

## 📋 Pending (Phase 1C)

### 5. API Routes
Create REST API endpoints in `server/src/routes/`:

**Servers** (`routes/servers.rs`):
- `GET /api/servers` - List all servers
- `POST /api/servers` - Create server
- `GET /api/servers/{id}` - Get server
- `PUT /api/servers/{id}` - Update server
- `DELETE /api/servers/{id}` - Delete server
- `POST /api/servers/{id}/test` - Test SSH connection

**Plugins** (`routes/plugins.rs`):
- `GET /api/plugins` - List all plugins
- `PUT /api/plugins/{id}` - Update plugin config
- `POST /api/plugins/{id}/toggle` - Enable/disable plugin

**Notifications** (`routes/notifications.rs`):
- `GET /api/notifications` - List notification backends
- `POST /api/notifications` - Create notification backend
- `PUT /api/notifications/{id}` - Update notification backend
- `DELETE /api/notifications/{id}` - Delete notification backend
- `POST /api/notifications/{id}/test` - Send test notification

**Tasks** (`routes/tasks.rs`):
- `GET /api/tasks` - List all tasks
- `POST /api/tasks` - Create task
- `GET /api/tasks/{id}` - Get task
- `PUT /api/tasks/{id}` - Update task
- `DELETE /api/tasks/{id}` - Delete task
- `POST /api/tasks/{id}/run` - Manually trigger task
- `GET /api/tasks/{id}/history` - Get task history

**Settings** (`routes/settings.rs`):
- `GET /api/settings` - Get all settings
- `PUT /api/settings/{key}` - Update setting

### 6. UI Pages
Create web UI for configuration:

**Servers Page** (✅ Form exists, needs backend connection):
- List all servers with status
- Add/Edit/Delete servers
- Test SSH connection button
- Show last seen time, OS info

**Plugins Page** (New):
- List all plugins with enable/disable toggles
- Configure plugin settings (JSON editor or form)
- Show plugin status and last run

**Notifications Page** (New):
- List notification backends
- Add/Edit/Delete backends
- Test notification button
- Priority configuration

**Tasks Page** (Partially exists):
- List scheduled tasks
- Add/Edit/Delete tasks
- Manual trigger button
- View execution history
- Cron expression builder

**Settings Page** (New):
- List all settings in categories
- Edit setting values
- Show setting descriptions
- Validate input based on type

---

## 🎯 Implementation Plan

### Step 1: Complete Database Layer (Current)
1. ✅ Create migration files
2. ✅ Create models
3. ⏳ Create query modules
4. ⏳ Update lib.rs to export everything
5. ⏳ Test migrations locally

### Step 2: Implement Server CRUD (Next)
1. Create `queries/servers.rs`
2. Create `routes/servers.rs` API
3. Update `ui_routes.rs` server handlers
4. Test server creation/listing
5. Add SSH connection testing

### Step 3: Implement Plugins
1. Create `queries/plugins.rs`
2. Create `routes/plugins.rs` API
3. Create `templates/pages/plugins.html`
4. Add plugin configuration UI
5. Test plugin enable/disable

### Step 4: Implement Notifications
1. Create `queries/notifications.rs`
2. Create `routes/notifications.rs` API
3. Create `templates/pages/notifications.html`
4. Add notification backend UI
5. Test notification sending

### Step 5: Implement Tasks
1. Create `queries/tasks.rs`
2. Create `routes/tasks.rs` API
3. Update `templates/pages/tasks.html`
4. Add task scheduling UI
5. Integrate with scheduler

### Step 6: Implement Settings
1. Create `queries/settings.rs`
2. Create `routes/settings.rs` API
3. Create `templates/pages/settings.html`
4. Add settings management UI

---

## 📊 Progress Tracker

| Component | Status | Progress |
|-----------|--------|----------|
| **Migrations** | ✅ Complete | 100% |
| **Models** | ✅ Complete | 100% |
| **Queries** | 🚧 In Progress | 0% |
| **API Routes** | ⏳ Pending | 0% |
| **UI Pages** | ⏳ Pending | 10% (server form exists) |
| **Testing** | ⏳ Pending | 0% |

**Overall Progress**: ~35% (Phase 1A complete)

---

## 🔄 Migration from Config File

Once database is fully implemented, we'll need a migration tool:

```bash
# Tool to import from config.toml to database
cargo run --bin svrctl -- migrate-config config.toml
```

This will:
1. Read servers from `config.toml`
2. Import them into `servers` table
3. Import notification settings
4. Import plugin configurations
5. Backup old config file

---

## 🧪 Testing Strategy

### Unit Tests
- Test each query function
- Test model serialization/deserialization
- Test database constraints

### Integration Tests
- Test full CRUD workflows
- Test API endpoints
- Test UI forms

### Manual Testing
1. Create server via UI
2. Edit server details
3. Test SSH connection
4. Enable/disable plugins
5. Configure notifications
6. Create scheduled task
7. View task history

---

## 📝 Next Immediate Steps

1. **Create query modules** (30-45 min)
   - Implement CRUD operations for each model
   - Add error handling
   - Add transaction support

2. **Update lib.rs** (10 min)
   - Export models and queries
   - Update migration runner

3. **Test locally** (15 min)
   - Run migrations
   - Test basic CRUD operations
   - Verify constraints

4. **Commit and push** (5 min)
   - Commit database layer changes
   - Push to develop branch

**Estimated Time to Complete Phase 1B**: ~1 hour

---

## 💡 Design Decisions

### Why SQLite?
- ✅ Embedded (no separate server)
- ✅ ACID compliant
- ✅ Good performance for single-server
- ✅ Easy backup (single file)
- ✅ Already in use

### Why JSON for Config?
- ✅ Flexible (plugin-specific settings)
- ✅ Easy to extend
- ✅ SQLite has good JSON support
- ✅ Can validate in application layer

### Why Separate Notification Backends Table?
- ✅ Existing `notifications` table is for LOG
- ✅ New table is for CONFIGURATION
- ✅ Allows multiple backends
- ✅ Per-backend priority settings

### Why Task History Extension?
- ✅ Existing table was basic
- ✅ Need more details for debugging
- ✅ Backward compatible (ALTER TABLE)
- ✅ Preserves existing data

---

## 🚀 Future Enhancements (Phase 2+)

### Authentication (Phase 2)
- Users table
- Sessions table
- Password hashing
- Role-based access control

### Advanced Features (Phase 3)
- Audit log
- Server groups
- Task dependencies
- Notification rules
- API keys
- Webhooks

### Performance (Phase 4)
- Query optimization
- Caching layer
- Batch operations
- Background jobs

---

**This document tracks the implementation progress of the database-backed configuration system.**

