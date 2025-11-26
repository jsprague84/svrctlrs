# Status Reporting & Dynamic Config Implementation Plan

**Goal:** Achieve weatherust feature parity + add dynamic configuration reloading

---

## Phase 1: Plugin Configuration Schema

Add to plugin config JSON:
```json
{
  "schedule": "0 */5 * * * *",
  "send_summary": true,           // NEW: Always send status (even if healthy)
  "summary_schedule": "0 8 * * *", // NEW: Daily summary at 8 AM
  "notify_on_success": true,       // NEW: Notify when no issues found
  
  // Docker-specific
  "cpu_warn_pct": 80,
  "mem_warn_pct": 80,
  "ignore_containers": [],
  
  // Health-specific  
  "cpu_threshold": 80,
  "mem_threshold": 80,
  "disk_threshold": 85
}
```

---

## Phase 2: Docker Plugin Enhancements

### 2.1 Health Check Always-Send
**File:** `plugins/docker/src/health.rs`

**Current behavior:**
```rust
if !bad_containers.is_empty() {
    self.send_health_alert(notify_mgr, &bad_containers).await?;
} else {
    info!("All containers healthy");  // Silent!
}
```

**New behavior:**
```rust
if !bad_containers.is_empty() {
    self.send_health_alert(notify_mgr, &bad_containers).await?;
} else if config.send_summary {
    self.send_health_summary(notify_mgr, &health_statuses).await?;
}
```

### 2.2 Cleanup Report Always-Send
**File:** `plugins/docker/src/cleanup.rs`

Add summary notification showing:
- Unused images count and space
- Dangling volumes
- Stopped containers
- Recommendations

### 2.3 Docker Image Update Reports
**New file:** `plugins/docker/src/updates.rs`

Check for image updates:
```rust
pub async fn check_image_updates(&self) -> Result<Vec<ImageUpdate>> {
    // For each running container:
    // 1. Get current image digest
    // 2. Pull latest image (without replacing)
    // 3. Compare digests
    // 4. Report if update available
}
```

---

## Phase 3: Updates Plugin Enhancements

### 3.1 Always-Send Status
**File:** `plugins/updates/src/lib.rs`

**Current:** Only notifies when updates available  
**New:** Send summary even when no updates

```rust
if updates.is_empty() {
    if config.send_summary {
        notify_mgr.send_for_service("updates", &NotificationMessage {
            title: "System Up to Date".to_string(),
            body: format!("All packages are current. Last checked: {}", now),
            priority: 3,
            actions: vec![],
        }).await?;
    }
}
```

### 3.2 Per-Server Reports
Track update status per server in database:
```sql
CREATE TABLE server_update_status (
    server_id INTEGER,
    last_checked DATETIME,
    total_updates INTEGER,
    security_updates INTEGER,
    packages TEXT,  -- JSON array
    FOREIGN KEY (server_id) REFERENCES servers(id)
);
```

---

## Phase 4: Health Plugin Enhancements

### 4.1 Configurable Thresholds
**File:** `plugins/health/src/lib.rs`

Load from config instead of hardcoded:
```rust
pub struct HealthPlugin {
    cpu_threshold: f64,     // From config, default 80
    mem_threshold: f64,     // From config, default 80
    disk_threshold: f64,    // From config, default 85
    send_summary: bool,     // From config
}
```

### 4.2 Actual Metrics Collection
**Currently:** Placeholder that does nothing  
**New:** Collect real metrics

```rust
async fn collect_metrics(&self) -> Result<SystemMetrics> {
    SystemMetrics {
        cpu_percent: self.get_cpu_usage().await?,
        mem_percent: self.get_mem_usage().await?,
        disk_usage: self.get_disk_usage().await?,
        network_rx_bytes: self.get_network_rx().await?,
        network_tx_bytes: self.get_network_tx().await?,
    }
}
```

### 4.3 Always-Send Summary
```rust
if metrics.cpu_percent > threshold || metrics.mem_percent > threshold {
    // Send alert
} else if config.send_summary {
    // Send "all good" summary
}
```

---

## Phase 5: Dynamic Configuration Reload

### 5.1 Config Reload Endpoint
**File:** `server/src/routes/api.rs`

```rust
async fn reload_config(State(state): State<AppState>) -> Result<Json<ReloadResponse>> {
    // 1. Reload plugins from database
    // 2. Update scheduler with new schedules
    // 3. Reinitialize notification backends
    // 4. Return status
}
```

### 5.2 UI Reload Button
**File:** `server/templates/pages/settings.html`

Add button:
```html
<button hx-post="/api/v1/config/reload"
        hx-indicator="#reload-spinner"
        class="btn btn-primary">
    Reload Configuration
    <span id="reload-spinner" class="htmx-indicator">...</span>
</button>
```

### 5.3 Scheduler Hot-Reload
**File:** `scheduler/src/lib.rs`

Add methods:
```rust
impl Scheduler {
    pub async fn update_task(&mut self, task: ScheduledTask) {
        // Remove old task
        // Add new task with updated schedule
    }
    
    pub async fn reload_all_tasks(&mut self, tasks: Vec<ScheduledTask>) {
        // Clear all tasks
        // Add new tasks
    }
}
```

---

## Phase 6: Summary Schedules

### 6.1 Separate Summary Tasks
Create dedicated summary tasks:
- `docker_daily_summary` - Daily at 8 AM
- `health_daily_summary` - Daily at 8 AM
- `updates_weekly_summary` - Weekly on Monday

### 6.2 Summary Content
**Docker Summary:**
```
📊 Daily Docker Summary

Containers: 19 total, 19 running, 0 stopped
Health: All healthy ✓
Images: 45 total, 12 unused (2.3 GB reclaimable)
Volumes: 8 total, 2 unused

Last update check: 2 hours ago
Next cleanup: Sunday 2 AM
```

**Health Summary:**
```
💚 System Health Report

CPU: 15% (threshold: 80%)
Memory: 45% (threshold: 80%)
Disk: 62% (threshold: 85%)

All systems normal ✓
```

**Updates Summary:**
```
📦 Update Status

System: Up to date ✓
Last checked: 3 hours ago

Docker images: 2 updates available
- nginx: 1.25.3 → 1.25.4
- postgres: 15.4 → 15.5
```

---

## Implementation Order

1. ✅ **Docker health always-send** (1-2 hours)
2. ✅ **Updates always-send** (1 hour)
3. ✅ **Health configurable thresholds** (2 hours)
4. ✅ **Docker cleanup reports** (1 hour)
5. ✅ **Docker image update detection** (3 hours)
6. ✅ **Dynamic config reload** (2-3 hours)
7. ✅ **Summary schedules** (1 hour)
8. ✅ **UI improvements** (1 hour)

**Total estimated time:** 12-15 hours of focused work

---

## Testing Plan

1. Configure plugins with `send_summary: true`
2. Verify notifications sent even when healthy
3. Test config changes without restart
4. Verify thresholds are respected
5. Check summary schedules trigger correctly
6. Validate Docker image update detection
7. Test cleanup reports

---

## Weatherust Parity Checklist

After implementation:
- ✅ Daily summaries
- ✅ Weekly summaries  
- ✅ Status on success (not just errors)
- ✅ Docker image update detection
- ✅ Docker cleanup/maintenance reports
- ✅ Configurable health thresholds
- ✅ Per-server update tracking
- ✅ Dynamic config (bonus!)

---

**Status:** Ready to implement  
**Start Date:** 2025-11-26  
**Target Completion:** 2025-11-27

