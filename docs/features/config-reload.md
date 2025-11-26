# Dynamic Configuration Reload

**Status**: ✅ Fully Implemented
**Last Updated**: 2025-11-26

## Overview

SvrCtlRS supports **dynamic configuration reloading** without restarting the server. This allows you to make configuration changes through the UI and apply them immediately without interrupting service.

## What Gets Reloaded

When you trigger a configuration reload, the following components are refreshed:

### 1. **Plugin Configurations**
- Plugin registry is cleared and rebuilt
- All enabled plugins are reloaded from database
- Plugin configs (send_summary, thresholds, etc.) are reapplied
- Plugins with updated configs start using new settings immediately

### 2. **Task Schedules**
- Scheduler clears all existing scheduled tasks
- Tasks are reloaded from database
- New schedules take effect immediately
- Cron expressions are revalidated

### 3. **Notification Backends** *(Implicit)*
- Notification backends are loaded from database on every use
- No explicit reload needed - always uses latest config

## How to Use

### Via Web UI (Recommended)

1. Navigate to **Settings** page (http://localhost:8080/settings)
2. Scroll to **"🔄 Configuration Reload"** card
3. Click **"Reload Configuration"** button
4. Wait for success message showing:
   - Number of plugins loaded
   - Number of scheduled tasks

**Example Success Message:**
```
✅ Configuration reloaded successfully!
Plugins loaded: 3 | Scheduled tasks: 12
```

### Via REST API

```bash
curl -X POST http://localhost:8080/api/v1/config/reload
```

**Success Response** (200 OK):
```html
<div class="alert alert-success">
    ✅ Configuration reloaded successfully!<br>
    <small class="text-secondary">
        Plugins loaded: 3 | Scheduled tasks: 12
    </small>
</div>
```

**Error Response** (500 Internal Server Error):
```html
<div class="alert alert-error">
    ❌ Configuration reload failed: [error message]
</div>
```

## Use Cases

### 1. **Plugin Configuration Changes**
**Scenario**: You update a plugin's `send_summary` setting or threshold values

**Steps**:
1. Go to Plugins page
2. Click "Configure" on a plugin
3. Change settings (e.g., enable "Send summary reports")
4. Click "Save Configuration"
5. Go to Settings → Click "Reload Configuration"
6. Plugin immediately uses new settings

**Without Reload**: Would require container restart

### 2. **Task Schedule Updates**
**Scenario**: You change when a task runs

**Steps**:
1. Go to Tasks page
2. Click on schedule to edit (e.g., change from every 5 min to every hour)
3. Save new schedule
4. Go to Settings → Click "Reload Configuration"
5. Task reschedules with new cron expression

**Without Reload**: Would require container restart

### 3. **Notification Backend Changes**
**Scenario**: You add/modify/remove notification backends

**Steps**:
1. Go to Settings → Notifications
2. Add/edit notification backend (Gotify or ntfy)
3. Save changes
4. *No reload needed* - backends are loaded dynamically

**Note**: Notification backends don't require explicit reload

### 4. **Plugin Enable/Disable**
**Scenario**: You enable/disable plugins

**Steps**:
1. Go to Plugins page
2. Toggle plugin on/off
3. Go to Settings → Click "Reload Configuration"
4. Plugin starts/stops monitoring immediately

**Without Reload**: Would require container restart

## Technical Implementation

### Code Flow

```
User clicks "Reload Configuration"
    ↓
POST /api/v1/config/reload
    ↓
AppState::reload_config()
    ↓
┌─────────────────────────────────────────┐
│ 1. Clear Plugin Registry                │
│    - registry.clear()                    │
│    - self.init_plugins().await           │
├─────────────────────────────────────────┤
│ 2. Reload Plugins from Database         │
│    - Query enabled plugins               │
│    - Load configs via from_config()      │
│    - Register in plugin registry         │
├─────────────────────────────────────────┤
│ 3. Clear Scheduler Tasks                │
│    - scheduler.clear_all_tasks().await   │
├─────────────────────────────────────────┤
│ 4. Reload Tasks from Database            │
│    - Query enabled tasks                 │
│    - Parse cron schedules                │
│    - Register task handlers              │
└─────────────────────────────────────────┘
    ↓
Return success/failure to UI
```

### Key Files

- **Backend Logic**: `server/src/state.rs` (`reload_config()` method)
- **API Endpoint**: `server/src/routes/api.rs` (`reload_config()` handler)
- **UI Button**: `server/templates/pages/settings.html`
- **Scheduler**: `scheduler/src/lib.rs` (`clear_all_tasks()` method)

### Error Handling

The reload process includes comprehensive error handling:

- **Database errors**: Caught and returned as error message
- **Plugin initialization errors**: Logged and returned
- **Schedule parsing errors**: Individual tasks skipped, continue with others
- **Scheduler errors**: Caught and reported

If reload fails:
- Previous configuration remains active
- System continues operating with old config
- Error message shows specific failure reason

## Limitations

### What Does NOT Require Reload

- **Notification backend changes** - Loaded dynamically on each use
- **Server list changes** - Used directly from context

### What CANNOT Be Reloaded

These require container restart:
- **Server address/port** (`SVRCTLRS_ADDR`)
- **Database URL** (`DATABASE_URL`)
- **Environment variables** (SSH keys, paths, etc.)
- **Static files** (CSS, JavaScript, templates)
- **Compiled code changes**

### Safety Considerations

- **No Downtime**: Reload happens while server continues running
- **Atomic Operations**: Plugin and task reloads are atomic
- **Rollback**: On failure, old config remains active
- **Concurrent Requests**: Other requests continue during reload

## Performance

- **Reload Time**: ~100-500ms depending on number of plugins/tasks
- **Impact**: Minimal - other requests continue normally
- **Logging**: All reload steps are logged at INFO level

## Troubleshooting

### Reload Button Not Working

1. **Check logs**: Look for errors in server logs
2. **Browser console**: Check for JavaScript/HTMX errors
3. **API test**: Try curl command directly
4. **Database**: Verify database is accessible

### Config Changes Not Applied

1. **Verify reload**: Check success message appears
2. **Check database**: Ensure changes were saved to database
3. **Plugin-specific**: Some configs may require task re-execution
4. **Cache**: Hard refresh browser (Ctrl+F5)

### Partial Reload

If reload partially succeeds:
- Check logs for specific errors
- Individual task schedule errors are skipped, others continue
- Plugin errors prevent that plugin from loading

## Examples

### Example 1: Enable Docker Summaries

```bash
# 1. Enable send_summary via UI
#    Plugins → Docker → Configure → Check "Send summary" → Save

# 2. Reload config
curl -X POST http://localhost:8080/api/v1/config/reload

# 3. Verify in logs
tail -f logs/svrctlrs.log | grep -i "reload\|docker"
# Expected:
# INFO reload_config: Reloading plugins...
# INFO reload_config: Reloading scheduler tasks...
# INFO reload_config: Configuration reloaded successfully
```

### Example 2: Change Task Schedule

```bash
# 1. Update task schedule via UI
#    Tasks → Click schedule → Edit to "0 0 * * * *" (hourly) → Save

# 2. Reload config via UI
#    Settings → Reload Configuration

# 3. Verify new schedule is active
# Check task list shows updated schedule
```

### Example 3: Add New Plugin

```bash
# 1. Enable plugin via UI
#    Plugins → Toggle "Health" to enabled

# 2. Reload config
curl -X POST http://localhost:8080/api/v1/config/reload

# 3. Verify plugin loaded
curl http://localhost:8080/api/v1/status
# Expected JSON includes new plugin in count
```

## See Also

- [Plugin Development Guide](../architecture/plugin-development-guide.md)
- [Task Scheduling](./task-scheduling.md)
- [Notification Backends](./notifications.md)

---

**Last Updated**: 2025-11-26
**Feature Status**: ✅ Production Ready
