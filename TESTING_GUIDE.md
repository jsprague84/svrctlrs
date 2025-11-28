# Phase 2 Testing Guide

## Quick Start

### 1. Pull Latest Image
```bash
# On your docker-vm server
docker pull ghcr.io/jsprague84/svrctlrs:develop

# Restart container
cd /path/to/svrctlrs
docker compose down
docker compose up -d

# Watch logs
docker compose logs -f svrctlrs
```

### 2. Verify Deployment
```bash
# Check container is running
docker compose ps

# Check for errors
docker compose logs svrctlrs | grep -i error

# Verify database migrations
docker compose exec svrctlrs ls -la /app/data/
```

## Feature Testing

### SSH Connection Testing

**Location**: http://your-server:8080/servers

**Steps**:
1. Navigate to Servers page
2. Find a configured server
3. Click "Test Connection" button
4. **Expected**: Green success message with "Connection successful!"
5. **Verify**: `last_seen_at` timestamp updated in database

**Test Failure Case**:
1. Edit a server with wrong hostname
2. Click "Test Connection"
3. **Expected**: Red error message with specific error
4. **Verify**: `last_error` field populated in database

**SQL to verify**:
```sql
SELECT name, last_seen_at, last_error 
FROM servers 
ORDER BY last_seen_at DESC;
```

### Capability Detection

**Location**: http://your-server:8080/servers

**Steps**:
1. Click "Detect Capabilities" button on a server
2. **Expected**: List of detected capabilities appears
3. **Verify**: Capabilities saved to database

**Expected Capabilities** (depending on server):
- `docker` - if Docker is installed
- `systemd` - if systemd is running
- `apt` - on Debian/Ubuntu
- `dnf` - on Fedora/RHEL
- `pacman` - on Arch
- `zfs` - if ZFS is installed
- `lvm` - if LVM is installed

**SQL to verify**:
```sql
SELECT s.name, sc.capability, sc.enabled
FROM servers s
JOIN server_capabilities sc ON s.id = sc.server_id
ORDER BY s.name, sc.capability;
```

### Job Execution Engine

**Location**: http://your-server:8080/job-schedules

**Quick Test - Create a Simple Job**:

1. **Create Job Template**:
   - Go to `/job-types`
   - Find "OS Monitoring" job type
   - Click to view details
   - Find "System Uptime" command template
   - Click "Create Job Template"
   - Name: "Test Uptime Check"
   - Select a server
   - Click "Create"

2. **Create Schedule**:
   - Go to `/job-schedules`
   - Click "Add Schedule"
   - Name: "Test Schedule"
   - Select your job template
   - Cron: `*/5 * * * *` (every 5 minutes)
   - Enable: Yes
   - Click "Create"

3. **Wait for Execution**:
   - Scheduler polls every 60 seconds
   - Job should execute within 5 minutes
   - Check `/job-runs` page for results

4. **Verify Execution**:
   ```sql
   SELECT 
     jr.id,
     jr.status,
     jr.started_at,
     jr.duration_ms,
     jr.output,
     jr.error
   FROM job_runs jr
   ORDER BY jr.started_at DESC
   LIMIT 10;
   ```

**Manual Trigger** (if available):
- Find your schedule in the list
- Click "Run Now" button
- Check job_runs table immediately

### Database Performance

**Test JOIN Optimizations**:

1. **Job Templates Page**:
   - Navigate to `/job-templates`
   - **Check**: Job Type Name column should show actual names (not empty)
   - **Check**: Page should load quickly (< 500ms)

2. **Job Schedules Page**:
   - Navigate to `/job-schedules`
   - **Check**: Template Name and Server Name columns populated
   - **Check**: No "TODO" or empty fields

3. **Job Runs Page**:
   - Navigate to `/job-runs`
   - **Check**: All name fields populated
   - **Check**: Fast page load even with many runs

**Performance Comparison**:
```sql
-- Old way (N+1 queries)
-- 1 query for schedules + N queries for template names + N queries for server names

-- New way (1 query)
SELECT 
  js.id, js.name,
  jt.name as job_template_name,
  s.name as server_name
FROM job_schedules js
INNER JOIN job_templates jt ON js.job_template_id = jt.id
LEFT JOIN servers s ON js.server_id = s.id;
```

### Notification Testing

**Location**: http://your-server:8080/settings/notifications

**Steps**:
1. Navigate to Notifications page
2. Find your configured backend (Gotify or ntfy)
3. Click "Test" button
4. **Expected**: Success message
5. **Check**: Notification received on your device

**Troubleshooting**:
- Check backend is enabled
- Verify token/credentials are correct
- Check logs: `docker compose logs svrctlrs | grep notification`

## Common Issues

### Issue: SSH Connection Fails

**Symptoms**: "Connection failed" error when testing

**Checks**:
1. Verify SSH key is mounted correctly:
   ```bash
   docker compose exec svrctlrs ls -la /home/svrctlrs/.ssh/
   ```

2. Test SSH from host:
   ```bash
   ssh -i /path/to/key user@server
   ```

3. Check credentials in database:
   ```sql
   SELECT id, name, credential_type, value 
   FROM credentials;
   ```

**Fix**: Ensure `SSH_KEY_PATH` in `.env` points to directory, not file

### Issue: Capabilities Not Detected

**Symptoms**: No capabilities shown after detection

**Checks**:
1. Check SSH connection works first
2. Verify server has the tools installed:
   ```bash
   ssh user@server 'which docker systemctl apt'
   ```

3. Check logs:
   ```bash
   docker compose logs svrctlrs | grep -i capability
   ```

**Fix**: Install missing tools on target server

### Issue: Jobs Not Executing

**Symptoms**: Schedules created but no job_runs

**Checks**:
1. Verify scheduler is running:
   ```bash
   docker compose logs svrctlrs | grep -i scheduler
   ```
   Should see: "Starting job scheduler"

2. Check schedule is enabled:
   ```sql
   SELECT id, name, enabled, next_run_at 
   FROM job_schedules;
   ```

3. Check cron expression is valid:
   - Use https://crontab.guru/ to validate

4. Check for errors:
   ```sql
   SELECT * FROM job_runs 
   WHERE status = 'failed' 
   ORDER BY started_at DESC;
   ```

**Fix**: 
- Enable schedule if disabled
- Fix cron expression
- Check server connectivity

### Issue: Database Locked

**Symptoms**: "database is locked" errors in logs

**Cause**: SQLite doesn't handle high concurrency well

**Fix**:
1. Reduce concurrent jobs:
   - Edit `server/src/state.rs`
   - Change `max_concurrent_jobs` from 10 to 5

2. Or migrate to PostgreSQL (future enhancement)

## Performance Benchmarks

### Expected Performance

**Page Load Times** (with 100 records):
- Job Templates: < 200ms
- Job Schedules: < 300ms
- Job Runs: < 500ms

**Job Execution**:
- Simple job (local): < 100ms
- Simple job (SSH): < 2s
- Composite job (3 steps): < 5s

**Scheduler**:
- Poll interval: 60s
- Max concurrent jobs: 10
- Memory usage: < 50MB

## Monitoring

### Key Metrics to Watch

1. **Job Success Rate**:
   ```sql
   SELECT 
     COUNT(*) as total,
     SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as success,
     SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed
   FROM job_runs
   WHERE started_at > datetime('now', '-1 day');
   ```

2. **Average Execution Time**:
   ```sql
   SELECT 
     AVG(duration_ms) as avg_ms,
     MIN(duration_ms) as min_ms,
     MAX(duration_ms) as max_ms
   FROM job_runs
   WHERE status = 'success'
   AND started_at > datetime('now', '-1 day');
   ```

3. **Server Health**:
   ```sql
   SELECT 
     name,
     enabled,
     last_seen_at,
     last_error
   FROM servers
   ORDER BY last_seen_at DESC;
   ```

### Log Monitoring

```bash
# Watch all logs
docker compose logs -f svrctlrs

# Watch scheduler only
docker compose logs -f svrctlrs | grep scheduler

# Watch job execution
docker compose logs -f svrctlrs | grep -i "job_run\|executing"

# Watch errors
docker compose logs -f svrctlrs | grep -i error
```

## Success Criteria

Phase 2 testing is successful if:

- ✅ SSH connection testing works for all servers
- ✅ Capability detection populates server_capabilities table
- ✅ Job scheduler executes jobs on schedule
- ✅ Job runs are recorded in database with output
- ✅ Notifications are sent on job completion
- ✅ All UI pages load without "TODO" placeholders
- ✅ No errors in logs during normal operation
- ✅ Page load times are acceptable (< 1s)

## Next Steps After Testing

Once Phase 2 testing is complete:

1. **Report Issues**: Document any bugs or unexpected behavior
2. **Performance Tuning**: Adjust scheduler interval, concurrent jobs if needed
3. **Phase 3 Planning**: Prepare for security features implementation
4. **Production Readiness**: Plan authentication, CSRF, and security hardening

## Support

**Logs Location**: Check `docker compose logs svrctlrs`
**Database**: `./data/svrctlrs.db` (SQLite)
**GitHub Actions**: https://github.com/jsprague84/svrctlrs/actions
**Docker Image**: ghcr.io/jsprague84/svrctlrs:develop

