# Jobs and Schedules User Guide

This guide explains how to create and manage jobs, job templates, and schedules in SvrCtlRS.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Understanding Jobs](#understanding-jobs)
- [Creating Job Templates](#creating-job-templates)
- [Scheduling Jobs](#scheduling-jobs)
- [Cron Schedule Syntax](#cron-schedule-syntax)
- [Job Execution](#job-execution)
- [Best Practices](#best-practices)
- [Common Use Cases](#common-use-cases)
- [Troubleshooting](#troubleshooting)

## Overview

The SvrCtlRS job system allows you to:
- Execute automated tasks on local and remote servers
- Schedule recurring operations using cron-like syntax
- Monitor execution history and results
- Group tasks by server for easier management
- Receive notifications when jobs complete or fail

### Key Concepts

**Job Type**: Defines what kind of task to perform (OS updates, Docker operations, health checks, etc.)

**Job Template**: A reusable configuration that specifies:
- Which job type to execute
- Which servers to target
- Custom parameters for the job

**Job Schedule**: Defines when a job template should run:
- One-time execution (Run Now)
- Recurring execution (cron schedule)
- Enable/disable without deleting

**Job Run**: A single execution instance of a job template, storing:
- Execution status (success, failed, timeout)
- Output and error messages
- Start/end timestamps
- Per-server results (for multi-server jobs)

## Quick Start

### 1. Create Your First Job Template

Navigate to **Job Templates** and click **Add Job Template**.

```
Name: Update Production Servers
Description: Apply OS updates to production web servers
Job Type: OS Updates
Servers: web-01, web-02, web-03
Tags: production
```

### 2. Schedule the Job

Navigate to **Job Schedules** and click **Add Schedule**.

```
Name: Weekly Production Updates
Description: Run every Sunday at 2 AM
Job Template: Update Production Servers
Schedule: 0 2 * * 0
Enabled: ✓
```

### 3. Monitor Execution

Check **Job Runs** to see execution history:
- Status indicators (success/failed/timeout)
- Execution timestamps
- Server-specific results
- Error messages and logs

## Understanding Jobs

### Job Types

SvrCtlRS includes several built-in job types:

#### OS Updates
- **Purpose**: Check for and apply operating system updates
- **Supported**: Debian, Ubuntu, Fedora, RHEL, CentOS
- **Execution**: Uses system package managers (apt, dnf, yum)
- **Output**: Lists packages updated, counts, any errors

#### Docker Operations
- **Purpose**: Manage Docker containers and images
- **Supported Operations**:
  - Pull latest images
  - Restart containers
  - Prune unused images/containers
  - Health checks
- **Requirements**: Docker installed on target server

#### Health Checks
- **Purpose**: Monitor system health metrics
- **Checks**:
  - CPU usage
  - Memory usage
  - Disk space
  - Service status
  - Docker container health

#### Custom Plugin Jobs
- Additional job types provided by plugins
- Check plugin documentation for details

### Execution Modes

**Local Execution**:
- Runs on the SvrCtlRS server itself
- No SSH required
- Fastest execution
- Best for: monitoring the SvrCtlRS host, local Docker operations

**Remote Execution (SSH)**:
- Runs on remote servers
- Requires SSH access with key-based authentication
- Executes commands over SSH connection
- Best for: managing multiple servers, distributed operations

## Creating Job Templates

### Step-by-Step Guide

#### 1. Basic Information

**Name**: Descriptive name for the job template
```
✓ Good: "Weekly Docker Image Updates"
✓ Good: "Backup Database - Production"
✗ Avoid: "Job1", "Test", "TODO"
```

**Description** (optional but recommended):
```
✓ Good: "Updates Docker images and restarts containers on production web servers every Sunday"
✗ Avoid: "Updates stuff"
```

#### 2. Select Job Type

Choose the type of operation to perform:
- **OS Updates**: System package updates
- **Docker**: Container and image management
- **Health**: System health monitoring
- **Custom**: Plugin-provided job types

#### 3. Select Servers

**All Servers**: Run on every server in the system
- Use for: System-wide health checks, universal updates

**Specific Servers**: Choose individual servers
- Use for: Targeted operations, production-only tasks

**By Tag**: Select servers with specific tags
- Use for: Environment-based operations (production, staging, development)

**Local Only**: Run only on the SvrCtlRS host
- Use for: Self-monitoring, local operations

#### 4. Configure Parameters

Job type-specific settings:

**OS Updates Parameters**:
```
Auto-approve updates: ✓ (automatically install) or ✗ (check only)
Reboot if required: ✓ (auto-reboot) or ✗ (manual reboot)
Package filter: (optional regex to filter packages)
```

**Docker Parameters**:
```
Operation: pull, restart, prune, health-check
Container filter: (regex to match container names)
Image tag: (specific tag to pull, e.g., "latest")
```

**Health Check Parameters**:
```
CPU threshold: 80 (alert if CPU > 80%)
Memory threshold: 85 (alert if memory > 85%)
Disk threshold: 90 (alert if disk > 90%)
Services to check: nginx, postgresql, redis
```

### Template Examples

#### Example 1: Daily Health Monitoring
```
Name: Daily Health Check - All Servers
Description: Monitor CPU, memory, disk on all servers
Job Type: Health Check
Servers: All Servers
Parameters:
  CPU Threshold: 80
  Memory Threshold: 85
  Disk Threshold: 90
  Services: nginx, postgresql
```

#### Example 2: Production Docker Updates
```
Name: Production Container Updates
Description: Pull latest images and restart containers
Job Type: Docker
Servers: By Tag (production)
Parameters:
  Operation: pull-and-restart
  Image Tag: stable
  Container Filter: app-.*
```

#### Example 3: Weekly OS Patching
```
Name: Weekly Security Updates
Description: Apply security patches to all servers
Job Type: OS Updates
Servers: All Servers
Parameters:
  Auto-approve: ✓
  Reboot if required: ✗
  Package Filter: .*security.*
```

## Scheduling Jobs

### Creating a Schedule

Navigate to **Job Schedules** → **Add Schedule**

#### Basic Schedule Configuration

**Name**: Descriptive name for when/why this runs
```
✓ Good: "Nightly Database Backup"
✓ Good: "Hourly Health Check"
✗ Avoid: "Schedule 1", "Cron Job"
```

**Description** (optional):
```
Example: "Runs every night at 2 AM during low-traffic hours"
```

**Job Template**: Select the template to execute

**Schedule**: Cron expression (see [Cron Schedule Syntax](#cron-schedule-syntax))

**Enabled**: Toggle to enable/disable without deleting

### Run Now vs Scheduled

**Run Now**:
- Click the "Run Now" (▶) button on any schedule
- Executes immediately, doesn't affect schedule
- Useful for: Testing, manual operations, immediate fixes

**Scheduled**:
- Runs automatically based on cron expression
- Continues until disabled or deleted
- Useful for: Routine maintenance, automated monitoring

### Schedule Examples

#### Every 5 minutes
```
Schedule: */5 * * * *
Use for: Frequent health checks, real-time monitoring
```

#### Hourly at minute 15
```
Schedule: 15 * * * *
Use for: Regular but not too frequent tasks
```

#### Daily at 2 AM
```
Schedule: 0 2 * * *
Use for: Nightly maintenance, backups
```

#### Weekly on Sunday at 3 AM
```
Schedule: 0 3 * * 0
Use for: Weekly updates, major maintenance
```

#### Monthly on the 1st at midnight
```
Schedule: 0 0 1 * *
Use for: Monthly reports, license checks
```

#### Business hours (Mon-Fri, 9 AM - 5 PM, hourly)
```
Schedule: 0 9-17 * * 1-5
Use for: Working hours monitoring
```

#### First Monday of every month at 9 AM
```
Schedule: 0 9 1-7 * 1
Use for: Monthly maintenance windows
```

## Cron Schedule Syntax

SvrCtlRS uses standard cron syntax with 5 fields:

```
┌───────────── minute (0 - 59)
│ ┌───────────── hour (0 - 23)
│ │ ┌───────────── day of month (1 - 31)
│ │ │ ┌───────────── month (1 - 12)
│ │ │ │ ┌───────────── day of week (0 - 6) (Sunday = 0)
│ │ │ │ │
│ │ │ │ │
* * * * *
```

### Special Characters

**Asterisk (*)**: Any value
```
* * * * *  = Every minute
0 * * * *  = Every hour at minute 0
```

**Comma (,)**: List of values
```
0 1,13 * * *  = 1 AM and 1 PM daily
0 0 * * 1,5   = Midnight on Monday and Friday
```

**Hyphen (-)**: Range of values
```
0 9-17 * * *  = Hourly from 9 AM to 5 PM
0 0 * * 1-5   = Midnight Monday through Friday
```

**Slash (/)**: Step values
```
*/15 * * * *  = Every 15 minutes
0 */6 * * *   = Every 6 hours
```

### Common Patterns

| Pattern | Schedule | Description |
|---------|----------|-------------|
| `* * * * *` | Every minute | Testing, high-frequency monitoring |
| `*/5 * * * *` | Every 5 minutes | Frequent health checks |
| `0 * * * *` | Every hour | Regular monitoring |
| `0 0 * * *` | Daily at midnight | Daily maintenance |
| `0 2 * * *` | Daily at 2 AM | Low-traffic maintenance |
| `0 0 * * 0` | Weekly on Sunday | Weekly updates |
| `0 0 1 * *` | Monthly on 1st | Monthly operations |
| `0 9 * * 1-5` | Weekdays at 9 AM | Business hours tasks |
| `0 */4 * * *` | Every 4 hours | Periodic checks |
| `30 3 * * 6` | Saturday 3:30 AM | Weekend maintenance |

### Testing Cron Expressions

Before saving a schedule:
1. Use an online cron calculator (search "cron calculator")
2. Verify it matches your intended schedule
3. Start with `Enabled: ✗` to test first
4. Use "Run Now" to test the job template
5. Enable once confirmed working

## Job Execution

### Execution Flow

1. **Trigger**: Schedule fires or user clicks "Run Now"
2. **Preparation**: System loads job template and server list
3. **Execution**: Job runs on each target server
   - Local jobs: Execute directly on SvrCtlRS host
   - Remote jobs: SSH to each server and execute
4. **Collection**: Gather results from all servers
5. **Storage**: Save job run with status and output
6. **Notification**: Send notifications based on policies

### Execution Status

**Success** (✓):
- Job completed without errors
- All servers executed successfully
- Expected exit codes received

**Failed** (✗):
- Job encountered errors
- One or more servers failed
- Non-zero exit code

**Timeout** (⏱):
- Job exceeded maximum execution time
- Default timeout: 300 seconds (5 minutes)
- Configurable per job type

**Running** (⟳):
- Job currently executing
- Real-time status updates
- Can be viewed in job runs list

### Multi-Server Execution

For jobs targeting multiple servers:

**Parallel Execution**:
- All servers start simultaneously
- Faster total execution time
- Independent failures (one server failing doesn't stop others)

**Results Aggregation**:
- Per-server status (success/failed)
- Individual output logs
- Combined statistics
- Summary in job run details

**Notification Behavior**:
- Can notify on individual server failures
- Can notify only if all servers fail
- Customizable per notification policy

## Best Practices

### 1. Start Simple

Begin with manual execution before scheduling:
```
1. Create job template
2. Test with "Run Now"
3. Verify output and status
4. Create schedule only after successful test
5. Start with `Enabled: ✗` initially
```

### 2. Use Descriptive Names

Make it easy to understand what jobs do:
```
✓ Good: "Weekly Docker Cleanup - Production"
✓ Good: "Hourly Health Check - Database Servers"
✗ Avoid: "Job 1", "Test", "TODO"
```

### 3. Tag Your Servers

Organize servers with tags for easier job targeting:
```
Production servers: tag "production"
Development servers: tag "development"
Database servers: tag "database"
Web servers: tag "web"
```

### 4. Schedule During Low-Traffic Hours

Run maintenance during off-peak times:
```
Updates: 2-4 AM
Backups: 1-3 AM
Cleanup: 3-5 AM
```

### 5. Set Up Notifications

Create notification policies for job failures:
```
Critical servers: Notify immediately
Development: Daily summary
Production: Individual failures
```

### 6. Monitor Job History

Regularly review job runs:
- Check for patterns in failures
- Identify slow-running jobs
- Verify schedules are working
- Clean up old job runs

### 7. Use Templates for Reusability

Create templates for common operations:
```
Base template: "OS Updates - All Servers"
Variations:
  - "OS Updates - Production Only"
  - "OS Updates - Development Only"
  - "OS Updates - Database Servers"
```

### 8. Test Before Scheduling

Always test job templates manually:
```
1. Create template
2. Click "Run Now"
3. Check job run details
4. Verify output is correct
5. Only then create schedule
```

### 9. Avoid Over-Scheduling

Don't run jobs more frequently than needed:
```
✓ Good: Health checks every 5 minutes
✗ Avoid: OS updates every minute
✗ Avoid: Docker pulls every 30 seconds
```

### 10. Document Complex Jobs

Use the description field to explain:
```
Description: "Updates all production web servers with
latest security patches. Runs during maintenance window
(2-4 AM Sunday). Does NOT auto-reboot - manual restart
required. Notifies ops-team on failure."
```

## Common Use Cases

### 1. Automated OS Updates

**Scenario**: Keep all servers updated with security patches

**Job Template**:
```
Name: Security Updates - All Servers
Job Type: OS Updates
Servers: All Servers
Parameters:
  Auto-approve: ✓
  Reboot if required: ✗
  Package Filter: .*security.*
```

**Schedule**:
```
Name: Weekly Security Patching
Schedule: 0 2 * * 0 (Sunday 2 AM)
Enabled: ✓
```

**Notification Policy**:
```
Trigger: On Failure
Severity: 4 (Error)
Message: "Security updates failed on {{server_name}}"
```

### 2. Docker Container Health

**Scenario**: Monitor Docker containers and restart unhealthy ones

**Job Template**:
```
Name: Docker Health Check & Restart
Job Type: Docker
Servers: By Tag (docker-hosts)
Parameters:
  Operation: health-check-and-restart
  Container Filter: .*
```

**Schedule**:
```
Name: Hourly Docker Health
Schedule: 0 * * * * (Every hour)
Enabled: ✓
```

### 3. Nightly Cleanup

**Scenario**: Clean up old Docker images and containers to free space

**Job Template**:
```
Name: Docker Cleanup - Prune Old Images
Job Type: Docker
Servers: All Servers
Parameters:
  Operation: prune
  Age: 30d (remove images older than 30 days)
```

**Schedule**:
```
Name: Nightly Docker Cleanup
Schedule: 0 3 * * * (Daily 3 AM)
Enabled: ✓
```

### 4. Production vs Development Schedules

**Scenario**: Different update schedules for different environments

**Production Template**:
```
Name: OS Updates - Production
Job Type: OS Updates
Servers: By Tag (production)
Parameters:
  Auto-approve: ✗ (check only, manual approval)
  Reboot if required: ✗
```

**Production Schedule**:
```
Schedule: 0 2 * * 0 (Weekly, Sunday 2 AM)
```

**Development Template**:
```
Name: OS Updates - Development
Job Type: OS Updates
Servers: By Tag (development)
Parameters:
  Auto-approve: ✓ (auto-install)
  Reboot if required: ✓ (auto-reboot)
```

**Development Schedule**:
```
Schedule: 0 2 * * * (Daily 2 AM)
```

### 5. Disaster Recovery Testing

**Scenario**: Regularly test backup restoration

**Job Template**:
```
Name: Test Backup Restoration - Database
Job Type: Custom (backup-restore plugin)
Servers: db-backup-test
Parameters:
  Backup Source: s3://backups/latest
  Test Database: test_restore
```

**Schedule**:
```
Schedule: 0 4 1 * * (Monthly, 1st at 4 AM)
```

### 6. Compliance Reporting

**Scenario**: Monthly compliance checks and reports

**Job Template**:
```
Name: Compliance Check - All Servers
Job Type: Health
Servers: All Servers
Parameters:
  Check SSL Certificates: ✓
  Check Security Updates: ✓
  Check Service Status: ✓
  Generate Report: ✓
```

**Schedule**:
```
Schedule: 0 9 1 * * (Monthly, 1st at 9 AM)
```

## Troubleshooting

### Job Won't Start

**Symptom**: Schedule is enabled but job doesn't run

**Checklist**:
1. ✓ Schedule is enabled
2. ✓ Cron expression is valid (use online calculator)
3. ✓ Job template still exists
4. ✓ At least one server is selected
5. ✓ SvrCtlRS scheduler is running (check logs)

**Solution**:
- Check system logs for scheduler errors
- Verify cron expression: `0 2 * * *` not `2 0 * * *`
- Test with "Run Now" to verify template works

### Job Fails Immediately

**Symptom**: Job starts but fails within seconds

**Common Causes**:

**SSH Connection Failures**:
```
Error: "Connection refused" or "Permission denied"
Solution:
  - Verify SSH key is configured
  - Check server is reachable
  - Test SSH manually: ssh user@server
  - Verify SSH key permissions (600)
```

**Missing Dependencies**:
```
Error: "docker: command not found"
Solution:
  - Install required software on target server
  - Verify PATH includes necessary binaries
  - Check job type matches server capabilities
```

**Permission Denied**:
```
Error: "Permission denied" or "sudo required"
Solution:
  - Verify SSH user has necessary permissions
  - Configure passwordless sudo if needed
  - Check file/directory permissions
```

### Job Times Out

**Symptom**: Job shows "timeout" status

**Causes**:
- Job takes longer than timeout period (default 5 minutes)
- Server is slow or unresponsive
- Network issues causing delays

**Solutions**:
1. Increase timeout (if available in job type settings)
2. Run during off-peak hours
3. Split large jobs into smaller operations
4. Check server performance (CPU, memory, disk I/O)

### Partial Failures (Multi-Server)

**Symptom**: Some servers succeed, others fail

**Analysis**:
1. View job run details
2. Check per-server results
3. Identify common failures
4. Review individual error messages

**Common Patterns**:

**Single Server Consistently Fails**:
```
Cause: Server-specific issue
Solution: Check that specific server's configuration
```

**Random Failures**:
```
Cause: Network instability or resource contention
Solution: Retry failed servers, check network
```

**All But One Fail**:
```
Cause: Job template misconfiguration
Solution: Review job template settings, test on working server first
```

### Missing Output/Logs

**Symptom**: Job shows success but no output visible

**Causes**:
- Job completed but produced no output
- Output was too large and truncated
- Logging not configured for job type

**Solutions**:
- Check job type supports output logging
- Review job run details page
- Check SvrCtlRS system logs
- Verify job actually performed expected actions

### Schedule Runs at Wrong Time

**Symptom**: Job runs but at unexpected times

**Common Mistakes**:

**Timezone Confusion**:
```
Issue: Cron runs in UTC but you expected local time
Solution: Convert schedule to UTC
Example: For 2 AM PST (UTC-8), use schedule 0 10 * * *
```

**Swapped Hour/Minute**:
```
Wrong: 2 0 * * * (minute 2 of hour 0 = 12:02 AM)
Right: 0 2 * * * (minute 0 of hour 2 = 2:00 AM)
```

**Day of Week**:
```
Wrong: 0 0 * * 1 (Monday in some systems, Sunday in others)
Right: 0 0 * * 0 (Sunday) or 1-5 (Mon-Fri)
```

### Jobs Pile Up (Overlapping Execution)

**Symptom**: New job starts before previous one finishes

**Causes**:
- Schedule frequency too high
- Job takes longer than schedule interval
- Server is slow or overloaded

**Solutions**:
1. Increase schedule interval
2. Optimize job to run faster
3. Configure job queue settings (if available)
4. Split job into smaller, faster operations

## Advanced Topics

### Job Execution Order

For jobs targeting multiple servers, execution order is:
1. Servers are processed in parallel (default)
2. Each server's result is independent
3. Total job status is "success" only if all servers succeed
4. Failure on one server doesn't stop others

### Retry Logic

(If supported by your SvrCtlRS configuration)

Some job types support automatic retry on failure:
- Retry count: Number of attempts
- Retry delay: Time between retries
- Retry on: Which types of failures to retry

### Job Priorities

(If supported by your SvrCtlRS configuration)

Jobs can have priorities:
- High: Execute immediately, interrupt low-priority jobs
- Normal: Standard execution queue
- Low: Execute only when resources available

### Resource Limits

(If supported by your SvrCtlRS configuration)

Jobs can be configured with resource limits:
- CPU limit: Maximum CPU usage
- Memory limit: Maximum memory usage
- Concurrent limit: Max simultaneous executions

## FAQ

### Can I run jobs manually even if disabled?

Yes! Click "Run Now" on any schedule, even if disabled. This is useful for testing or one-time execution.

### What happens if a server is offline?

The job will fail for that server. Other servers (in multi-server jobs) continue execution. Enable notifications to be alerted.

### Can I schedule jobs to run only once?

Yes, use "Run Now" for immediate one-time execution. Scheduled jobs are for recurring operations. You can disable a schedule after it runs once if needed.

### How do I stop a running job?

(Feature depends on SvrCtlRS configuration)
- Some job types support cancellation
- Check the job run details page for a "Cancel" button
- Otherwise, wait for timeout or completion

### Can I run the same template on different schedules?

Yes! Create multiple schedules pointing to the same job template:
```
Template: "OS Updates - Production"
Schedule 1: Daily health check (no auto-approve)
Schedule 2: Weekly updates (auto-approve)
```

### How long are job run results stored?

Check your SvrCtlRS configuration for retention policy. Typical defaults:
- Successful runs: 30-90 days
- Failed runs: 90-365 days (longer for debugging)

### Can I export job run history?

(Feature depends on SvrCtlRS configuration)
- Check for "Export" or "Download" buttons
- Use the API for programmatic access
- Check system logs for raw output

---

**Need Help?**
- Report issues: https://github.com/jsprague84/svrctlrs/issues
- Check system logs for detailed error messages
- Review notification policies for failure alerts
- Test jobs with "Run Now" before scheduling

