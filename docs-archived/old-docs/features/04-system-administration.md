# System Administration Features

**Category:** System Administration  
**Current State:** Basic OS update detection, SSH command execution  
**Gap:** No service management, no file operations, limited automation  
**Priority:** Tier 1-2

---

## Features

### 1. Service Management Plugin
**Tier 1 | Effort: Low | Value: High**

Monitor and control systemd services from the UI.

**Capabilities:**
- List all services with status
- Start/stop/restart services
- Enable/disable on boot
- View service logs
- Service dependency visualization

```rust
async fn list_services(ssh: &SshSession) -> Result<Vec<Service>> {
    let output = ssh.execute("systemctl list-units --type=service --all").await?;
    parse_systemctl_output(&output)
}
```

---

### 2. Package Management Enhancements
**Tier 2 | Effort: Medium | Value: High**

One-click updates with maintenance windows and rollback.

**Capabilities:**
- One-click security updates
- Scheduled maintenance windows
- Automatic reboot handling with notification
- Package hold/pin management
- Rollback capability (apt/dnf history)

---

### 3. File Manager Plugin
**Tier 1 | Effort: Medium | Value: High**

Browse and manage remote files from the UI.

**Capabilities:**
- Browse filesystem
- Upload/download files
- Edit text files in browser
- File search
- Permission management
- Symbolic link support

**Technical:** Use SFTP over existing SSH connections

---

### 4. Backup & Restore Plugin
**Tier 2 | Effort: High | Value: High**

Automated backup jobs with verification and off-site storage.

**Capabilities:**
- Scheduled backup jobs (files, databases, configs)
- Backup verification and testing
- Off-site backup (S3, Backblaze B2, rsync)
- Backup rotation policies (7 daily, 4 weekly, 12 monthly)
- Restore testing automation

---

### 5. Log Aggregation
**Tier 2 | Effort: High | Value: Medium**

Centralized log collection and analysis.

**Capabilities:**
- Collect logs from all servers
- Real-time log streaming
- Search and filtering
- Retention policies
- Export for analysis

---

### 6. User Management Plugin
**Tier 2 | Effort: Medium | Value: Low**

Manage system users and SSH keys.

**Capabilities:**
- View/add/remove system users
- SSH key management
- Sudo access control
- Password policy enforcement

---

### 7. Cron Job Management
**Tier 2 | Effort: Low | Value: Medium**

View and manage cron jobs from UI.

**Capabilities:**
- List all cron jobs
- Add/edit/delete cron jobs
- Cron execution history
- Notification on failures

---

**Last Updated:** 2025-11-25

