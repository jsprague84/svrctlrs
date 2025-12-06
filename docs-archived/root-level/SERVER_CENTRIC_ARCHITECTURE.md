# Server-Centric Architecture Refactor

## Overview

Refactoring SvrCtlRS from plugin-centric to server-centric task organization for better multi-server management.

## Current Status: Phase 1 Complete ✅

### Completed:
- ✅ Database migration adding `server_id` and `server_name` to tasks
- ✅ Auto-creation of `localhost` server
- ✅ Migration of existing tasks to localhost
- ✅ Updated task models and queries
- ✅ Added `list_tasks_by_server()` query

### Testing Phase 1:
Before continuing with UI changes, please test the database migration:

1. **Pull and restart**:
   ```bash
   docker-compose pull
   docker-compose up -d
   ```

2. **Verify migration**:
   - Check logs for migration success
   - Go to Servers page - should see "localhost" server
   - Go to Tasks page - tasks should still work (now associated with localhost)

3. **Expected behavior**:
   - All existing tasks now belong to "localhost" server
   - Tasks page shows tasks (UI not yet updated to group by server)
   - Running tasks should work normally

## Remaining Work (Phase 2):

### UI Updates:
- [ ] Refactor Tasks page to group by server
- [ ] Add collapsible server sections
- [ ] Add inline schedule editing
- [ ] Show server status indicators

### Task Creation:
- [ ] Update task creation to require server selection
- [ ] Add "Create Tasks" dialog when adding new servers
- [ ] Template tasks for common monitoring (Health, Docker, Updates)

### Executor Updates:
- [ ] Update executor to use task's server_id for context
- [ ] Ensure SSH connections use correct server
- [ ] Handle localhost vs remote server execution

## New Architecture:

### Tasks Page (Future):
```
Tasks
├── localhost ▼
│   ├── Health Check (every 5 min) [Run Now] [Edit]
│   ├── Docker Health (every 5 min) [Run Now] [Edit]
│   ├── Updates Check (every 5 min) [Run Now] [Edit]
│   ├── Weather (daily 1:30 AM) [Run Now] [Edit]
│   └── Speedtest (daily 2:30 AM) [Run Now] [Edit]
├── web-01 (192.168.1.10) ▼
│   ├── Health Check (every 5 min) [Run Now] [Edit]
│   ├── Docker Health (every 10 min) [Run Now] [Edit]
│   └── Updates Check (every 6 hrs) [Run Now] [Edit]
└── Add Server...
```

### Benefits:
- ✅ Clear per-server visibility
- ✅ Individual schedules per server
- ✅ Easy SSH connection testing
- ✅ Flexible plugin assignment
- ✅ Task history per server
- ✅ Different monitoring for different server types

### Database Schema:
```sql
tasks:
  - id
  - name
  - plugin_id
  - server_id (FK to servers.id)
  - server_name (denormalized for display)
  - schedule (cron expression)
  - enabled
  - ...
```

## Migration Notes:

- Backward compatible: existing tasks auto-migrate to localhost
- `server_id` is nullable but should always be set in practice
- `server_name` is denormalized for query performance
- Tasks are deleted when server is deleted (CASCADE)

## Next Steps:

1. **Test Phase 1** (current state)
2. **Implement Phase 2** (UI + executor updates)
3. **Add task templates** for easy server setup
4. **Add bulk operations** (enable/disable all tasks for a server)

## Questions/Decisions:

1. **Weather/Speedtest**: These are not server-specific. Solution: Assign to one server (e.g., localhost) for scheduling purposes.

2. **Task Templates**: When adding a server, offer to create standard monitoring tasks (Health, Docker, Updates) with default schedules.

3. **Localhost**: Always exists, represents the SvrCtlRS host system.

## Commands for Testing:

```bash
# Check database
docker exec -it svrctlrs sqlite3 /app/data/svrctlrs.db "SELECT id, name, server_id, server_name FROM tasks;"

# Check servers
docker exec -it svrctlrs sqlite3 /app/data/svrctlrs.db "SELECT id, name, host FROM servers;"

# Check migration applied
docker exec -it svrctlrs sqlite3 /app/data/svrctlrs.db ".schema tasks"
```

