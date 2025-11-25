# Immediate Development Priorities

Based on user feedback after v1.0.0 release.

## 🐛 Critical Issues (Fix First)

### Issue 1: ntfy Notifications Not Delivering
**Status:** Needs investigation  
**Priority:** 🔴 Critical

**Symptoms:**
- Gotify notifications working
- ntfy notifications not arriving

**Possible Causes:**
1. Topic configuration incorrect
2. URL format issue
3. Authentication failing
4. Service name mismatch

**Debug Steps:**
```bash
# Check ntfy backend config in database
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db \
  "SELECT * FROM notification_backends WHERE backend_type='ntfy';"

# Check logs for ntfy errors
docker compose logs svrctlrs | grep -i ntfy

# Test ntfy manually
curl -d "Test message" https://ntfy.sh/your-topic
```

**Fix Plan:**
1. Add debug logging to ntfy backend
2. Verify topic format (should be just topic name, not URL)
3. Check if auth token is needed
4. Test with curl to verify ntfy server is reachable

---

### Issue 2: Plugins Not Sending Status Reports
**Status:** By design, but needs improvement  
**Priority:** 🟡 High

**Current Behavior:**
- Plugins only notify on **problems** (unhealthy containers, available updates, etc.)
- No notifications when everything is OK
- Weather and Speedtest always send results (correct)

**Desired Behavior:**
- Periodic status reports even when healthy
- Summary notifications (daily/weekly)
- Configurable notification frequency

**Affected Plugins:**
- ✅ Weather: Always sends (working as expected)
- ✅ Speedtest: Always sends (working as expected)
- ❌ Docker: Only notifies on unhealthy containers
- ❌ Health: Only notifies on threshold breaches
- ❌ Updates: Only notifies when updates available

**Fix Plan:**
1. Add "send_summary" config option to each plugin
2. Add "summary_schedule" (e.g., daily at 8am)
3. Send summary even if no issues
4. Include stats in summary (containers running, disk usage, etc.)

---

### Issue 3: Missing Webhook Action Buttons
**Status:** Not implemented  
**Priority:** 🟡 High

**Current State:**
- Notifications send, but no action buttons
- ntfy supports action buttons via `actions` array
- Gotify doesn't support action buttons (limitation)

**Desired Behavior:**
- ntfy notifications include action buttons:
  - "Update Now" for Updates plugin
  - "View Details" for Docker plugin
  - "Acknowledge" for alerts

**Implementation:**
```rust
// Example ntfy action button
NotificationAction {
    action: "view".to_string(),
    label: "View Details".to_string(),
    url: "https://svrctlrs.example.com/tasks/1".to_string(),
}
```

**Fix Plan:**
1. Update `NotificationMessage` to include action buttons
2. Implement webhook endpoints for actions
3. Add action buttons to plugin notifications
4. Test with ntfy mobile app

---

## 📋 Weatherust Feature Parity Checklist

### Current Status vs Weatherust

| Feature | Weatherust | SvrCtlRS v1.0.0 | Status |
|---------|-----------|-----------------|--------|
| **Core** |
| Docker monitoring | ✅ | ✅ | Complete |
| OS updates | ✅ | ✅ | Complete |
| System health | ✅ | ✅ | Complete |
| Weather | ✅ | ✅ | Complete |
| Speedtest | ✅ | ✅ | Complete |
| **Notifications** |
| Gotify | ✅ | ✅ | Complete |
| ntfy.sh | ✅ | ⚠️ | Needs debugging |
| Action buttons | ✅ | ❌ | Not implemented |
| **Reports** |
| Daily summaries | ✅ | ❌ | Not implemented |
| Weekly summaries | ✅ | ❌ | Not implemented |
| Status on success | ✅ | ❌ | Only on errors |
| **Docker Features** |
| Container health | ✅ | ✅ | Complete |
| Container updates | ✅ | ⚠️ | Detected, not automated |
| Image cleanup | ✅ | ⚠️ | Analysis only |
| Container logs | ✅ | ❌ | Not implemented |
| **Updates Features** |
| Update detection | ✅ | ✅ | Complete |
| Auto-update | ✅ | ❌ | Not implemented |
| Update webhook | ✅ | ❌ | Not implemented |
| Reboot handling | ✅ | ❌ | Not implemented |
| **Health Features** |
| CPU monitoring | ✅ | ⚠️ | Placeholder |
| Memory monitoring | ✅ | ⚠️ | Placeholder |
| Disk monitoring | ✅ | ⚠️ | Placeholder |
| Network monitoring | ✅ | ❌ | Not implemented |
| **UI Features** |
| Web dashboard | ✅ | ✅ | Complete |
| Task management | ✅ | ✅ | Complete |
| Server management | ✅ | ✅ | Complete |
| Plugin config | ✅ | ✅ | Complete |
| Historical data | ✅ | ⚠️ | Task history only |
| Charts/graphs | ✅ | ❌ | Not implemented |

**Summary:**
- ✅ Complete: 15 features
- ⚠️ Partial: 7 features
- ❌ Missing: 10 features

---

## 🎯 Development Roadmap

### Phase 1: Fix Critical Issues (1-2 weeks)

**Goal:** Get v1.0.0 to feature parity with weatherust core functionality

#### Week 1: Notifications & Reports
1. **Fix ntfy delivery** (1 day)
   - Debug and fix ntfy backend
   - Add comprehensive logging
   - Test with multiple topics

2. **Add status reports** (2-3 days)
   - Docker plugin: Daily container summary
   - Health plugin: Daily system health report
   - Updates plugin: Daily update check summary
   - Configurable via UI

3. **Add action buttons** (2 days)
   - Implement webhook endpoints
   - Add buttons to ntfy notifications
   - Test with mobile app

#### Week 2: Plugin Enhancements
4. **Implement actual Health monitoring** (2-3 days)
   - CPU usage collection
   - Memory usage collection
   - Disk usage collection
   - Store in metrics table

5. **Add Docker automation** (2 days)
   - Auto-update containers (opt-in)
   - Image cleanup execution
   - Webhook for manual updates

---

### Phase 2: Enhanced Features (2-3 weeks)

**Goal:** Add advanced features beyond weatherust

#### Week 3-4: Dashboard & Metrics
1. **Metrics storage** (3 days)
   - Implement metrics table
   - Collect from all plugins
   - Retention policy

2. **Dashboard page** (4 days)
   - Real-time stats
   - Charts (Chart.js)
   - Activity feed
   - Quick actions

3. **Historical data** (2 days)
   - View past metrics
   - Compare over time
   - Export data

#### Week 5: Advanced Automation
4. **Container logs viewer** (2 days)
   - Stream logs from UI
   - Search and filter
   - Download logs

5. **Auto-update system** (3 days)
   - Scheduled updates
   - Reboot handling
   - Rollback on failure

---

### Phase 3: Authentication & Multi-User (v1.1.0)

**Goal:** Secure the application for production use

See `DEVELOPMENT_PLAN.md` for details.

---

## 🚀 Immediate Next Steps

### This Week:

**Day 1-2: Fix ntfy**
```bash
# 1. Add debug logging
# 2. Test manually with curl
# 3. Fix configuration issues
# 4. Verify delivery
```

**Day 3-4: Add Status Reports**
```rust
// Add to each plugin:
async fn send_summary(&self, context: &PluginContext) -> Result<()> {
    let summary = self.collect_summary().await?;
    let message = NotificationMessage {
        title: "Daily Docker Summary".to_string(),
        body: format!("Containers: {}\nHealthy: {}", summary.total, summary.healthy),
        priority: 3,
        actions: vec![],
    };
    context.notification_manager.send_for_service("docker", &message).await?;
    Ok(())
}
```

**Day 5: Add Action Buttons**
```rust
// Add to notifications:
actions: vec![
    NotificationAction {
        action: "view".to_string(),
        label: "View Details".to_string(),
        url: format!("{}/tasks/{}", base_url, task_id),
    },
    NotificationAction {
        action: "http".to_string(),
        label: "Update Now".to_string(),
        url: format!("{}/api/v1/webhooks/docker/update", base_url),
        method: Some("POST".to_string()),
    },
],
```

---

## 📝 New Feature Ideas (After Feature Parity)

Based on user request: "I also was thinking of a new feature"

**Please share your new feature idea!** We should definitely get weatherust parity first, but it's good to document ideas for future development.

Potential areas:
- 🔐 Authentication & multi-user
- 📊 Advanced metrics & analytics
- 🤖 AI-powered anomaly detection
- 📱 Mobile app
- 🔗 Integration with other services
- 🎨 Custom dashboards
- 📈 Cost optimization
- 🔔 Advanced alerting rules

---

## 🐛 Known Issues

1. **ntfy notifications not delivering** - Needs investigation
2. **No status reports when healthy** - By design, needs config option
3. **No action buttons** - Not implemented yet
4. **Health plugin is placeholder** - Needs actual metrics collection
5. **No historical metrics** - Only task history stored
6. **No charts/graphs** - Dashboard needs visualization

---

## 📚 Resources

- **Current codebase:** All plugins in `plugins/` directory
- **Weatherust reference:** `~/Development/weatherust/` (for comparison)
- **Notification code:** `core/src/notifications.rs`
- **Plugin trait:** `core/src/plugin.rs`

---

**Last Updated:** 2025-11-25 (post v1.0.0 release)
**Next Review:** After fixing critical issues

