# Testing Status Report Summaries

This document explains how to test that status report summaries are working correctly.

## Prerequisites

1. SvrCtlRS server running
2. At least one notification backend configured (Gotify or ntfy)
3. Plugins enabled in database

## Test Procedure

### Step 1: Enable Summary Reports via UI

1. Navigate to **Plugins** page (http://localhost:8080/plugins)
2. Click "Configure" on each plugin (Updates, Docker, Health)
3. **Check the "Send summary" checkbox** for each plugin
4. Click "Save Configuration"

### Step 2: Manually Trigger Tasks

Instead of waiting for scheduled execution, manually trigger each task:

```bash
# Via UI: Go to Tasks page, click "Run Now" button for each task

# Or via API:
curl -X POST http://localhost:8080/api/v1/tasks/{task_id}/execute
```

### Step 3: Expected Notifications

#### Updates Plugin (with send_summary=true)
**When no updates available:**
```
Title: Updates Status: localhost
Body:  📊 System Up to Date ✓

       Server: localhost
       Package Manager: apt
       Last Checked: [timestamp]
```

#### Docker Plugin (with send_summary=true)
**When all containers healthy:**
```
Title: Docker Health Summary
Body:  📊 All containers healthy ✓

       Containers: 19 total, 19 running, 0 stopped
       Average CPU: 12.3%
       Average Memory: 45.6%

       All systems operational.
```

#### Health Plugin (with send_summary=true)
**When all metrics within thresholds:**
```
Title: System Health Summary
Body:  📊 System Status ✓

       CPU Usage: 15.2%
       Memory Usage: 45.8%
       Disk Usage: 62.1%

       All systems within normal parameters.
```

## Verification

### Database Check

Verify plugin configs have send_summary enabled:

```sql
SELECT id, name, config
FROM plugins
WHERE id IN ('updates', 'docker', 'health');
```

Expected output should show `"send_summary":true` in the config JSON.

### Log Check

Watch the logs for summary messages:

```bash
# Look for these log messages:
tail -f logs/svrctlrs.log | grep -i "summary\|health check\|updates check"
```

Expected log entries:
```
INFO health: Health summary sent
INFO docker::health: Health summary sent
INFO updates: Sending update status summary
```

## Troubleshooting

### No Summaries Received

1. **Check plugin config**: Ensure `send_summary: true` in database
2. **Check notification backend**: Verify backend is enabled and configured correctly
3. **Check logs**: Look for errors in plugin execution
4. **Restart server**: Config changes may require reload (`/api/v1/config/reload` or restart)

### Getting Alerts Instead of Summaries

This is expected behavior:
- If issues are detected (high CPU, updates available, unhealthy containers), you get an **alert**
- Summaries are ONLY sent when everything is normal
- To test summaries, ensure system is healthy first

### Summary Schedule

Default check frequencies:
- **Docker Health**: Every 5 minutes (`0 */5 * * * *`)
- **Updates Check**: Every 6 hours (`0 0 */6 * * *`)
- **Updates Report**: Daily at 9 AM (`0 0 9 * * *`) - Multi-server summary
- **Health Check**: Every 5 minutes (`0 */5 * * * *`)

## Example: Force a Summary

To guarantee a summary notification (assuming healthy system):

```bash
# 1. Enable send_summary via UI
# 2. Make sure system is healthy (no high CPU/mem/disk, no updates, all containers running)
# 3. Manually run the task:

# Via UI: Go to Tasks -> Click "Run Now" on "Docker health check"

# Or via curl:
curl -X POST http://localhost:8080/api/v1/tasks/1/execute  # Adjust task ID
```

## Success Criteria

✅ Summaries received when system is healthy
✅ Alerts received when issues detected
✅ `send_summary=false` silences summaries but keeps alerts
✅ Config changes take effect without restart

---

**Last Updated**: 2025-11-26
**Feature Status**: ✅ Fully Implemented (just needs testing)
