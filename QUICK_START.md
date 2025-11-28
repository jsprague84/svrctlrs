# SvrCtlRS Restructure - Quick Start Guide

## 🚀 Getting Started with the New System

### Prerequisites
- Rust 1.70+
- SQLite 3
- SSH access to managed servers

### First Time Setup

#### 1. Build the Application
```bash
cd /home/jsprague/Development/svrctlrs
cargo build --workspace --release
```

#### 2. Run Database Migration
```bash
# Migration runs automatically on first startup
cargo run --bin svrctlrs-server
```

The migration will:
- ✅ Drop old tables (tasks, plugins, metrics, etc.)
- ✅ Create 18 new tables
- ✅ Seed default data (settings, tags, job types, command templates)

#### 3. Access the UI
```bash
# Default: http://localhost:8080
```

---

## 📋 Initial Configuration Workflow

### Step 1: Add Credentials (Required First!)
Navigate to **Credentials** → **Add Credential**

**SSH Key Example:**
- Name: `Production SSH Key`
- Type: `SSH Key`
- Username: `sysadmin`
- Value: `/path/to/id_rsa`

### Step 2: Create Tags (Optional but Recommended)
Navigate to **Tags** → **Add Tag**

**Example Tags:**
- `prod` (color: #BF616A, icon: server)
- `staging` (color: #D08770, icon: test-tube)
- `docker-hosts` (color: #88C0D0, icon: container)

### Step 3: Add Servers
Navigate to **Servers** → **Add Server**

**Example:**
- Name: `docker-vm`
- Host: `192.168.1.100`
- Port: `22`
- Credential: Select from dropdown
- Tags: Select multiple tags
- **Click "Test Connection"** to detect capabilities

**Detected Capabilities:**
- OS Type: `ubuntu`
- OS Version: `22.04`
- Package Manager: `apt`
- Docker: `24.0.5`
- Docker Compose: `2.20.0`
- Systemd: Available

### Step 4: Configure Notification Channels
Navigate to **Notification Channels** → **Add Channel**

**Gotify Example:**
- Name: `Homelab Gotify`
- Type: `Gotify`
- URL: `https://gotify.yourdomain.com`
- Token: `AbCdEf123456`
- **Click "Test"** to verify delivery

**ntfy Example:**
- Name: `Production Alerts`
- Type: `ntfy`
- URL: `https://ntfy.sh`
- Topic: `svrctlrs-prod-alerts`

### Step 5: Create Notification Policies
Navigate to **Notification Policies** → **Add Policy**

**Example Policy:**
- Name: `Critical Failures`
- On Success: `None`
- On Failure: `Summary`
- Severity: `Error`
- Channels: Select channels
- Message Template (Failure):
```
[FAILURE] {{job_name}} failed on {{failure_count}}/{{total_servers}} servers

Failed servers:
{{#each server_results}}
- {{server_name}}: exit {{exit_code}}
{{/each}}
```

---

## 🎯 Creating Your First Job

### Example: Docker Health Check

#### Step 1: Job Template is Already Seeded!
Navigate to **Job Types** → Filter by "Docker"

**Built-in Job Types:**
- ✅ Docker: List Containers
- ✅ Docker: Container Stats
- ✅ Docker: Prune System
- ✅ Docker: List Images

#### Step 2: Create Job Template
Navigate to **Job Templates** → **Add Template**

- Name: `Docker Health Check - Production`
- Job Type: Select "Docker: Container Stats"
- Description: `Monitor container resource usage every 5 minutes`
- Parameters:
  - `cpu_warn_pct`: `80`
  - `mem_warn_pct`: `80`
- Target Type: `Tags`
- Target: Select `docker-hosts` tag
- Notification Policy: Select `Critical Failures`

#### Step 3: Schedule the Job
Navigate to **Job Schedules** → **Add Schedule**

- Name: `Docker Health - Every 5 Minutes`
- Job Template: Select "Docker Health Check - Production"
- Cron Expression: `0 */5 * * * *` (every 5 minutes)
- Timezone: `UTC`
- Enabled: ✅

#### Step 4: Test Manually
- Click **"Run Now"** button
- Navigate to **Job Runs**
- View execution results
- Check **Server Results** for per-server details
- Verify notification sent (if any failures)

---

## 💡 Common Workflows

### Multi-Step Workflow (Composite Job)

**Example: Full System Maintenance**

1. **Create Job Template** (Composite)
   - Name: `Full System Maintenance`
   - Enable Composite Mode: ✅

2. **Add Steps:**
   - Step 1: "Update Package Lists"
     - Job Type: `OS: Update Package Lists`
     - Run Condition: `Always`
     - Continue on Failure: ❌

   - Step 2: "Install Security Updates"
     - Job Type: `OS: Security Updates Only`
     - Run Condition: `On Success`
     - Continue on Failure: ❌

   - Step 3: "Clean Package Cache"
     - Job Type: `OS: Clean Package Cache`
     - Run Condition: `On Success`
     - Continue on Failure: ✅

   - Step 4: "Docker System Prune"
     - Job Type: `Docker: Prune System`
     - Run Condition: `On Success`
     - Continue on Failure: ✅

3. **Schedule Weekly**
   - Cron: `0 3 * * 0` (Sundays at 3 AM)
   - Target: All servers with `prod` tag
   - Notification Policy: Send summary on completion

---

## 🔍 Monitoring & Troubleshooting

### View Job History
Navigate to **Job Runs**

**Filters:**
- Status: Success / Failure / Running
- Job Template: Filter by specific template
- Date Range: Last 24h / 7d / 30d

**Details:**
- Click job run to view details
- See per-server results
- View stdout/stderr logs
- Check execution duration
- View extracted metrics

### Check Server Status
Navigate to **Servers**

**Status Indicators:**
- 🟢 Online: Connection successful, capabilities detected
- 🔴 Offline: Connection failed
- 🟡 Degraded: Connection succeeded but capability detection failed

**Actions:**
- **Test Connection**: Re-detect capabilities
- **View Capabilities**: See detected OS, package manager, Docker version
- **Edit**: Update credentials, tags, description

### Notification Debugging
Navigate to **Notification Channels**

**Test Delivery:**
- Click **"Test"** button on any channel
- Verify message received
- Check notification log for errors

Navigate to **Notification Policies**

**Verify Configuration:**
- Check event triggers (on_success, on_failure)
- Verify channels are linked to policy
- Review message templates

---

## 📊 Dashboard Overview

### Statistics Cards
- **Total Servers**: All configured servers
- **Active Schedules**: Enabled job schedules
- **Recent Job Runs**: Last 24 hours
- **Success Rate**: Overall job success percentage

### Quick Actions
- Add Server
- Create Job Template
- Schedule Job
- View Recent Runs

### Recent Activity
- Last 10 job runs
- Status, duration, server count
- Click to view details

---

## 🛠️ Advanced Features

### Custom Job Types

**Create a custom job:**

Navigate to **Job Types** → **Add Job Type**

- Name: `ZFS: Pool Status`
- Category: `custom`
- Description: `Check ZFS pool health`
- Parameters Schema:
```json
{
  "type": "object",
  "properties": {
    "pool_name": {
      "type": "string",
      "default": "tank"
    }
  }
}
```

**Add Command Template:**
- Target Condition: `{"capabilities": ["zfs"]}`
- Command: `zpool status {{pool_name}}`
- Timeout: 30 seconds

### Multi-OS Support

**Single job, multiple OSes:**

Job Type: `OS: Install Package`

**Command Templates:**
1. APT (Debian/Ubuntu):
   - Condition: `{"pkg_manager": "apt"}`
   - Command: `apt install -y {{package_name}}`

2. DNF (Fedora/RHEL):
   - Condition: `{"pkg_manager": "dnf"}`
   - Command: `dnf install -y {{package_name}}`

3. Pacman (Arch):
   - Condition: `{"pkg_manager": "pacman"}`
   - Command: `pacman -S --noconfirm {{package_name}}`

**Usage:**
- Create job template: "Install Nginx"
- Parameters: `{"package_name": "nginx"}`
- Target: All servers (any OS)
- Executor automatically selects correct command based on server's pkg_manager capability

---

## 🔧 Troubleshooting

### Job Execution Fails

**Check:**
1. Server status (Online?)
2. Credential validity (SSH key correct?)
3. Command template matches server capabilities
4. Timeout sufficient for job
5. Job run logs for specific error

### Notification Not Sent

**Check:**
1. Notification channel enabled?
2. Channel test succeeds?
3. Notification policy matches job (job type filter, server filter, tags)?
4. Policy trigger matches execution status (on_success vs on_failure)?
5. Notification log for delivery errors

### Scheduler Not Running Jobs

**Check:**
1. Schedule enabled?
2. Cron expression valid?
3. Next run time calculated correctly?
4. Server logs for scheduler errors
5. Job template references valid job type?

### Server Capability Detection Fails

**Check:**
1. SSH connection succeeds?
2. User has sudo/root access?
3. Commands available on server (`docker`, `systemctl`, `apt`/`dnf`/`pacman`)?
4. Test connection manually via SSH
5. Check server logs for probe script errors

---

## 📞 Getting Help

### Logs
```bash
# Check server logs
journalctl -u svrctlrs -f

# Or in Docker
docker-compose logs -f svrctlrs
```

### Database Inspection
```bash
sqlite3 /path/to/svrctlrs.db

# Useful queries:
SELECT * FROM job_schedules WHERE enabled = 1;
SELECT * FROM job_runs ORDER BY started_at DESC LIMIT 10;
SELECT * FROM servers WHERE status != 'online';
SELECT * FROM notification_log ORDER BY sent_at DESC LIMIT 10;
```

### Reset Database
```bash
# DANGER: This deletes all data!
rm /path/to/svrctlrs.db

# Restart application to re-run migration
cargo run --bin svrctlrs-server
```

---

## 🎯 Next Steps

1. ✅ Add all your servers
2. ✅ Configure notification channels
3. ✅ Create job templates for common tasks
4. ✅ Schedule regular health checks
5. ✅ Monitor job execution
6. 🚀 Explore creating custom job types!

---

**Quick Reference:**
- Database: SQLite at `/path/to/svrctlrs.db`
- Config: `config.toml` or environment variables
- Logs: Stdout/stderr with `RUST_LOG=info`
- UI: http://localhost:8080 (default)
