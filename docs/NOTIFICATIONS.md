# Notification System User Guide

This guide explains how to create and configure notification policies in SvrCtlRS.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Template Variables](#template-variables)
- [Pre-Built Templates](#pre-built-templates)
- [Advanced Features](#advanced-features)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

The SvrCtlRS notification system allows you to:
- Send notifications to multiple channels (Gotify, ntfy.sh)
- Customize messages with dynamic templates
- Filter notifications by job type, server, or tags
- Control notification frequency and severity
- Use pre-built templates or create custom ones

### Key Concepts

**Notification Channel**: Where notifications are sent (Gotify server, ntfy.sh topic)
**Notification Policy**: Rules that determine when and how to send notifications
**Template Variables**: Dynamic placeholders like `{{job_name}}` that get replaced with actual values

## Quick Start

### 1. Create a Notification Channel

Navigate to **Settings → Notifications → Channels** and click **Add Channel**.

**For Gotify:**
```
Name: My Gotify Server
Type: Gotify
URL: https://gotify.example.com
Token: YOUR_GOTIFY_TOKEN
Priority: 5
```

**For ntfy.sh:**
```
Name: My ntfy Topic
Type: ntfy
Topic: my-server-alerts
Priority: 3
```

### 2. Create a Notification Policy

Navigate to **Settings → Notifications → Policies** and click **Add Policy**.

**Example: Notify on All Failures**
```
Name: Critical Failures
Description: Alert on any failed job
Channels: [Select your channel]

Trigger Conditions:
☐ On Success
☑ On Failure
☑ On Timeout

Minimum Severity: 3 (Warning)

Title Template: [{{severity}}] {{job_name}} failed on {{server_name}}
Body Template: (leave blank for default summary)
```

## Template Variables

### Basic Variables

| Variable | Description | Example Value |
|----------|-------------|---------------|
| `{{job_name}}` | Name of the job template | "System Updates" |
| `{{job_type}}` | Type of job | "os_updates" |
| `{{schedule_name}}` | Name of the schedule | "Schedule 42" |
| `{{server_name}}` | Server where job ran | "web-server-01" |
| `{{status}}` | Job execution status | "success", "failed", "timeout" |
| `{{severity}}` | Calculated severity (1-5) | "critical", "warning", "info" |

### Statistics Variables

| Variable | Description | Example Value |
|----------|-------------|---------------|
| `{{total_servers}}` | Total servers in job | "5" |
| `{{success_count}}` | Number of successful executions | "4" |
| `{{failure_count}}` | Number of failed executions | "1" |
| `{{started_at}}` | Job start time | "2025-11-30 14:30:00 UTC" |
| `{{finished_at}}` | Job completion time | "2025-11-30 14:35:00 UTC" |
| `{{duration_seconds}}` | Execution duration | "300" |

### Advanced Features

#### Accessing Metadata

If your job stores custom metadata, access it with:
```
{{metrics.package_count}}
{{metrics.updates_available}}
{{metrics.custom_field}}
```

#### Looping Over Server Results

For multi-server jobs, display individual results:
```
{{#each server_results}}
Server: {{server_name}} - Status: {{status}}
{{/each}}
```

## Pre-Built Templates

### Simple Success Notification

**Title:**
```
✓ {{job_name}} completed on {{server_name}}
```

**Body:**
```
Job completed successfully in {{duration_seconds}} seconds.
```

### Detailed Failure Notification

**Title:**
```
✗ {{job_name}} failed on {{server_name}}
```

**Body:**
```
Job: {{job_name}}
Server: {{server_name}}
Status: {{status}}
Started: {{started_at}}
Duration: {{duration_seconds}}s

Check the job run details for error information.
```

### Multi-Server Summary

**Title:**
```
[{{severity}}] {{job_name}}: {{success_count}}/{{total_servers}} succeeded
```

**Body:**
```
Job Summary:
- Total Servers: {{total_servers}}
- Successful: {{success_count}}
- Failed: {{failure_count}}
- Duration: {{duration_seconds}}s

Started: {{started_at}}
Finished: {{finished_at}}
```

### Critical Failure Alert

**Title:**
```
🚨 CRITICAL: {{job_name}} failed on {{server_name}}
```

**Body:**
```
CRITICAL FAILURE DETECTED

Job: {{job_name}}
Type: {{job_type}}
Server: {{server_name}}
Time: {{finished_at}}

Immediate attention required!
```

### Update Notification with Details

**Title:**
```
{{server_name}}: {{metrics.package_count}} packages updated
```

**Body:**
```
System Updates Applied

Server: {{server_name}}
Packages Updated: {{metrics.package_count}}
Status: {{status}}
Duration: {{duration_seconds}}s

Started: {{started_at}}
Completed: {{finished_at}}
```

### Docker Health Check

**Title:**
```
[{{severity}}] Docker health check on {{server_name}}
```

**Body:**
```
Docker Health Status

Server: {{server_name}}
Status: {{status}}
Containers Checked: {{metrics.container_count}}
Unhealthy: {{metrics.unhealthy_count}}

Check dashboard for container details.
```

### Daily Summary (Multi-Server)

**Title:**
```
📊 Daily Report: {{job_name}}
```

**Body:**
```
Daily Job Summary

Servers Processed: {{total_servers}}
✓ Successful: {{success_count}}
✗ Failed: {{failure_count}}

Total Duration: {{duration_seconds}}s
Completed: {{finished_at}}

{{#each server_results}}
- {{server_name}}: {{status}}
{{/each}}
```

## Advanced Features

### Filtering Notifications

**By Job Type:**
Only notify for OS updates and Docker jobs:
```json
["os_updates", "docker"]
```

**By Server:**
Only notify for specific servers (use server IDs):
```json
[1, 3, 5]
```

**By Tags:**
Only notify for production servers:
```json
["production", "critical"]
```

### Rate Limiting

Prevent notification spam:
```
Max Notifications Per Hour: 10
```

This limits the policy to sending a maximum of 10 notifications per hour.

### Severity Filtering

Only send notifications for important events:
```
Minimum Severity: 3
```

Severity levels:
- **1** - Info (routine operations)
- **2** - Low (minor issues)
- **3** - Warning (potential problems)
- **4** - Error (definite problems)
- **5** - Critical (urgent attention needed)

### Priority Override

Set different priorities for different channels:
```
Default Channel Priority: 3
Critical Policy Override: 5
```

## Best Practices

### 1. Start Simple

Begin with a single policy that notifies on all failures:
```
Title: [FAILED] {{job_name}} on {{server_name}}
Body: (leave blank)
```

### 2. Use Severity Filtering

Avoid alert fatigue by only notifying on important events:
```
Minimum Severity: 3  # Only warnings and above
```

### 3. Customize for Different Environments

**Production:**
- Notify on all failures
- High priority (4-5)
- Detailed messages

**Development/Testing:**
- Notify only on critical failures
- Low priority (1-2)
- Brief messages

### 4. Group Related Notifications

Create separate policies for different types of alerts:
- **Critical Failures**: Immediate attention, high priority
- **Daily Summaries**: Low priority, scheduled reports
- **Update Notifications**: Medium priority, informational

### 5. Test Your Templates

After creating a policy:
1. Trigger a test job run
2. Verify the notification looks correct
3. Adjust template variables as needed

### 6. Use Rate Limiting Wisely

For frequently-running jobs, set `max_per_hour` to prevent spam:
```
Max Per Hour: 5  # For jobs that run every 5 minutes
```

### 7. Keep Titles Concise

Mobile notifications often truncate long titles:
```
✓ Good: [FAILED] Updates on web-01
✗ Too Long: The scheduled system update job has failed on web-server-01 at 2025-11-30
```

## Troubleshooting

### Template Variables Not Replaced

**Problem:** Seeing literal `{{server_name}}` in notifications

**Solution:**
- Ensure the variable name is spelled correctly
- Check that the variable exists (see [Template Variables](#template-variables))
- Verify there are no spaces: `{{job_name}}` not `{{ job_name }}`

### No Notifications Received

**Checklist:**
1. ✓ Policy is enabled
2. ✓ Channel is configured correctly
3. ✓ Trigger conditions match the job status
4. ✓ Severity meets minimum threshold
5. ✓ Filters don't exclude the job/server
6. ✓ Rate limit not exceeded

### Notifications Too Frequent

**Solutions:**
1. Set `max_per_hour` rate limit
2. Increase `min_severity` threshold
3. Add server/job type filters
4. Adjust trigger conditions (disable success notifications)

### Missing Job Information

**Problem:** Variables show empty or unexpected values

**Cause:** Job doesn't populate that field

**Solutions:**
- Check job run details to see available data
- Use `{{metrics.field_name}}` for custom job data
- Provide default text: "Duration: {{duration_seconds}}s (or in progress)"

### Loop Not Working

**Problem:** `{{#each server_results}}` doesn't display anything

**Cause:** Only works for multi-server jobs

**Solution:**
- Check if job runs on multiple servers
- For single-server jobs, use direct variables instead

## Examples by Use Case

### 1. Production Critical Alerts

```
Name: Production Critical Failures
Channels: [PagerDuty Gotify, SMS ntfy]

Triggers: Failure, Timeout
Tag Filter: ["production"]
Min Severity: 4

Title: 🚨 {{job_name}} FAILED on {{server_name}}
Body: Status: {{status}} | Started: {{started_at}} | Duration: {{duration_seconds}}s
```

### 2. Daily Summary Reports

```
Name: Daily Job Summary
Channels: [Slack Gotify]

Triggers: Success, Failure
Min Severity: 1
Max Per Hour: 1

Title: 📊 Daily Summary: {{job_name}}
Body: ✓ {{success_count}} succeeded | ✗ {{failure_count}} failed | Servers: {{total_servers}}
```

### 3. Update Monitoring

```
Name: System Update Notifications
Channels: [Email ntfy]

Triggers: Success, Failure
Job Type Filter: ["os_updates"]
Min Severity: 2

Title: {{server_name}}: Updates {{status}}
Body: {{metrics.package_count}} packages | {{duration_seconds}}s
```

### 4. Docker Health Monitoring

```
Name: Docker Container Alerts
Channels: [Gotify Server]

Triggers: Failure
Job Type Filter: ["docker"]
Min Severity: 3

Title: [Docker] {{status}} on {{server_name}}
Body: {{metrics.unhealthy_count}} unhealthy containers detected
```

## Future Enhancements

Planned features for the notification system:

### Markdown/HTML Styling
```
**Bold Text**: {{job_name}}
*Italic*: {{server_name}}
[Links](https://example.com/job/{{job_run_id}})
```

### Conditional Sections
```
{{#if failure_count > 0}}
⚠️ {{failure_count}} servers failed!
{{/if}}
```

### More Template Variables
- `{{job_run_id}}` - Direct link to job run details
- `{{user_triggered}}` - Who triggered the job
- `{{previous_status}}` - Status of last run
- `{{error_summary}}` - Brief error description

### Template Preview
- Live preview while editing templates
- Sample data substitution
- Validation and error checking

---

**Need Help?**
- Report issues: https://github.com/jsprague84/svrctlrs/issues
- Check logs: View notification log in Settings → Notifications
- Test channel: Use "Test" button on channel details page
