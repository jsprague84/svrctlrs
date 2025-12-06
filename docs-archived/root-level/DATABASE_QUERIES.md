# Database Query Functions - Complete Reference

This document provides a comprehensive overview of all database query functions for the SvrCtlRS unified schema.

## Table of Contents

1. [Credentials](#credentials)
2. [Tags](#tags)
3. [Servers](#servers)
4. [Job Types & Command Templates](#job-types--command-templates)
5. [Job Templates & Steps](#job-templates--steps)
6. [Job Schedules](#job-schedules)
7. [Job Runs](#job-runs)
8. [Notifications](#notifications)

---

## Credentials

**File:** `database/src/queries/credentials.rs`

### Core Operations

- `list_credentials(pool)` - Get all credentials
- `get_credential(pool, id)` - Get credential by ID
- `get_credential_by_name(pool, name)` - Get credential by name
- `create_credential(pool, input)` - Create new credential
- `update_credential(pool, id, input)` - Update existing credential
- `delete_credential(pool, id)` - Delete credential (checks if in use first)

### Utility Functions

- `credential_in_use(pool, id)` - Check if credential is referenced by any servers

---

## Tags

**File:** `database/src/queries/tags.rs`

### Core Operations

- `list_tags(pool)` - Get all tags
- `get_tag(pool, id)` - Get tag by ID
- `get_tags_with_counts(pool)` - Get tags with server counts for UI
- `create_tag(pool, input)` - Create new tag
- `update_tag(pool, id, input)` - Update existing tag
- `delete_tag(pool, id)` - Delete tag (CASCADE removes server_tags entries)

### Server-Tag Association

- `add_server_tag(pool, server_id, tag_id)` - Add tag to server
- `remove_server_tag(pool, server_id, tag_id)` - Remove tag from server
- `get_server_tags(pool, server_id)` - Get all tags for a server
- `set_server_tags(pool, server_id, tag_ids)` - Replace all tags for server (transactional)

---

## Servers

**File:** `database/src/queries/servers_new.rs`

### Core Operations

- `list_servers(pool)` - Get all servers
- `get_server(pool, id)` - Get server by ID
- `get_server_by_name(pool, name)` - Get server by name
- `get_servers_by_tag(pool, tag_id)` - Get all servers with specific tag
- `create_server(pool, input)` - Create new server with validation
- `update_server(pool, id, input)` - Update existing server
- `delete_server(pool, id)` - Delete server (CASCADE removes related data)
- `list_enabled_servers(pool)` - Get enabled servers only

### Status Management

- `update_server_status(pool, id, last_error)` - Update last_seen_at and error status

### Capability Detection

- `get_server_capabilities(pool, server_id)` - Get all capabilities for a server
- `set_server_capability(pool, server_id, name, available, version)` - Set/update capability
- `has_capability(pool, server_id, name)` - Check if server has capability
- `get_servers_with_capability(pool, name)` - Find servers by capability

---

## Job Types & Command Templates

**File:** `database/src/queries/job_types.rs`

### Job Type Operations

- `list_job_types(pool)` - Get all job types
- `get_job_type(pool, id)` - Get job type by ID
- `get_job_types_by_name(pool, name)` - Get job type by name
- `create_job_type(pool, input)` - Create new job type
- `update_job_type(pool, id, input)` - Update existing job type
- `delete_job_type(pool, id)` - Delete job type (checks if in use first)

### Command Template Operations

- `get_command_templates(pool, job_type_id)` - Get all command templates for a job type
- `get_command_template(pool, id)` - Get command template by ID
- `create_command_template(pool, input)` - Create new command template
- `update_command_template(pool, id, input)` - Update existing command template
- `delete_command_template(pool, id)` - Delete command template (checks if in use)

---

## Job Templates & Steps

**File:** `database/src/queries/job_templates.rs`

### Job Template Operations

- `list_job_templates(pool)` - Get all job templates
- `get_job_template(pool, id)` - Get job template by ID
- `create_job_template(pool, input)` - Create new job template (with validation)
- `update_job_template(pool, id, input)` - Update existing job template
- `delete_job_template(pool, id)` - Delete job template (checks if in use)

### Job Template Step Operations (for composite jobs)

- `get_job_template_steps(pool, template_id)` - Get all steps for a template
- `get_job_template_step(pool, id)` - Get specific step by ID
- `create_job_template_step(pool, input)` - Add step to template
- `update_job_template_step(pool, id, input)` - Update existing step
- `delete_job_template_step(pool, id)` - Remove step from template
- `reorder_job_template_steps(pool, template_id, step_orders)` - Reorder steps (transactional)

---

## Job Schedules

**File:** `database/src/queries/job_schedules.rs`

### Core Operations

- `list_job_schedules(pool)` - Get all job schedules
- `list_enabled_schedules(pool)` - Get enabled schedules only
- `get_job_schedule(pool, id)` - Get schedule by ID
- `get_schedules_by_template(pool, template_id)` - Get all schedules for a template
- `create_job_schedule(pool, input)` - Create new schedule (with cron validation)
- `update_job_schedule(pool, id, input)` - Update existing schedule
- `delete_job_schedule(pool, id)` - Delete schedule

### Scheduler Support

- `get_schedules_due(pool)` - Get schedules ready to run (enabled + next_run_at <= now)
- `update_schedule_next_run(pool, id, next_run)` - Update next_run_at timestamp
- `record_schedule_run(pool, id, status, next_run)` - Update last_run_at, status, counters, next_run_at

---

## Job Runs

**File:** `database/src/queries/job_runs.rs`

### Job Run Operations

- `list_job_runs(pool, limit, offset)` - Get recent runs with pagination
- `get_job_run(pool, id)` - Get job run by ID
- `get_job_runs_by_template(pool, template_id, limit)` - Filter by template
- `get_job_runs_by_schedule(pool, schedule_id, limit)` - Filter by schedule
- `create_job_run(pool, ...)` - Create new run (status='running')
- `update_job_run_status(pool, id, status, metadata)` - Update status during execution
- `finish_job_run(pool, id, status, exit_code, output, error)` - Mark run complete (calculates duration)
- `update_job_run_notification(pool, id, sent, error)` - Update notification tracking

### Server Job Result Operations (for multi-server jobs)

- `get_server_job_results(pool, run_id)` - Get all server results for a run
- `create_server_job_result(pool, job_run_id, server_id, metadata)` - Create result record
- `update_server_job_result(pool, id, status, exit_code, output, error)` - Update with final results

### Step Execution Operations (for composite jobs)

- `get_step_execution_results(pool, job_run_id)` - Get all step results for a run
- `create_step_execution_result(pool, job_run_id, step_order, step_name, cmd_id, metadata)` - Create step record
- `update_step_execution_result(pool, id, status, exit_code, output, error)` - Update step with results

---

## Notifications

**File:** `database/src/queries/notifications_new.rs`

### Notification Channel Operations

- `list_notification_channels(pool)` - Get all channels
- `get_notification_channel(pool, id)` - Get channel by ID
- `create_notification_channel(pool, input)` - Create new channel
- `update_notification_channel(pool, id, input)` - Update existing channel
- `delete_notification_channel(pool, id)` - Delete channel
- `update_channel_last_used(pool, id, success, error)` - Update test/usage tracking

### Notification Policy Operations

- `list_notification_policies(pool)` - Get all policies
- `get_notification_policy(pool, id)` - Get policy by ID
- `get_policy_channels(pool, policy_id)` - Get all channels linked to policy
- `create_notification_policy(pool, input)` - Create new policy
- `update_notification_policy(pool, id, input)` - Update existing policy
- `delete_notification_policy(pool, id)` - Delete policy

### Policy-Channel Association

- `add_policy_channel(pool, policy_id, channel_id, priority_override)` - Link channel to policy
- `remove_policy_channel(pool, policy_id, channel_id)` - Unlink channel from policy

### Notification Logging

- `log_notification(pool, channel_id, policy_id, job_run_id, title, body, priority, success, error, retry_count)` - Record sent notification
- `get_notification_log(pool, limit, offset)` - Get recent logs with pagination
- `get_notification_logs_for_run(pool, run_id)` - Get logs for specific job run

---

## Implementation Notes

### Error Handling

All query functions return `Result<T, Error>` from `svrctlrs_core` and use:

- `anyhow::Context` for error context
- `svrctlrs_core::Error::DatabaseError` for database-specific errors
- SQL injection prevention via sqlx bind parameters

### Transactions

The following operations use transactions for atomicity:

- `set_server_tags()` - Remove old tags + add new tags
- `reorder_job_template_steps()` - Update multiple step orders

### Logging

All functions use `#[instrument(skip(pool))]` for tracing integration.

### Type Safety

Functions use:

- `sqlx::query_as!` macro where possible for compile-time type checking
- Model validation methods (`.validate()`) before inserts
- Helper methods (`.has_changes()`) to avoid unnecessary updates

### Cascading Deletes

The following deletions cascade automatically via foreign key constraints:

- Delete server → Deletes server_tags, server_capabilities, job_schedules, job_runs
- Delete tag → Deletes server_tags entries
- Delete job_template → Deletes job_template_steps, job_schedules
- Delete job_schedule → Deletes job_runs
- Delete job_run → Deletes server_job_results, step_execution_results

### Protected Deletes

The following deletions check for usage and fail gracefully:

- Delete credential → Fails if used by servers
- Delete job_type → Fails if used by job_templates
- Delete command_template → Fails if used by job_templates or steps
- Delete job_template → Fails if used by job_schedules

---

## Usage Examples

### Creating a Server with Tags

```rust
use svrctlrs_database::queries::*;
use svrctlrs_database::models::*;

// Create server
let server_input = CreateServer {
    name: "web-server".to_string(),
    hostname: Some("192.168.1.100".to_string()),
    port: 22,
    username: Some("admin".to_string()),
    credential_id: Some(1),
    description: Some("Production web server".to_string()),
    is_local: false,
    enabled: true,
    metadata: None,
};

let server_id = create_server(&pool, &server_input).await?;

// Add tags
add_server_tag(&pool, server_id, prod_tag_id).await?;
add_server_tag(&pool, server_id, docker_tag_id).await?;

// Set capabilities
set_server_capability(&pool, server_id, "docker", true, Some("20.10.0")).await?;
set_server_capability(&pool, server_id, "apt", true, None).await?;
```

### Creating a Job Schedule

```rust
// Create job template
let template_input = CreateJobTemplate {
    name: "docker-cleanup".to_string(),
    display_name: "Docker System Cleanup".to_string(),
    description: Some("Remove unused Docker resources".to_string()),
    job_type_id: docker_job_type_id,
    is_composite: false,
    command_template_id: Some(prune_template_id),
    variables: None,
    timeout_seconds: 300,
    retry_count: 1,
    retry_delay_seconds: 60,
    notify_on_success: false,
    notify_on_failure: true,
    notification_policy_id: Some(default_policy_id),
    metadata: None,
};

let template_id = create_job_template(&pool, &template_input).await?;

// Create schedule
let schedule_input = CreateJobSchedule {
    name: "docker-cleanup-daily".to_string(),
    description: Some("Run cleanup daily at 2 AM".to_string()),
    job_template_id: template_id,
    server_id: server_id,
    schedule: "0 0 2 * * *".to_string(), // 2 AM daily
    enabled: true,
    timeout_seconds: None, // Use template default
    retry_count: None, // Use template default
    notify_on_success: None,
    notify_on_failure: None,
    notification_policy_id: None,
    metadata: None,
};

let schedule_id = create_job_schedule(&pool, &schedule_input).await?;
```

### Recording a Job Run

```rust
// Create job run
let run_id = create_job_run(
    &pool,
    schedule_id,
    template_id,
    server_id,
    0, // retry_attempt
    false, // is_retry
    None, // metadata
).await?;

// ... execute job ...

// Finish job run
finish_job_run(
    &pool,
    run_id,
    "success",
    Some(0), // exit_code
    Some("Docker system pruned successfully".to_string()),
    None, // error
).await?;

// Record notification
update_job_run_notification(&pool, run_id, true, None).await?;

// Update schedule
record_schedule_run(&pool, schedule_id, "success", Some(next_run_time)).await?;
```

---

## Migration Path

To migrate from old query functions to new ones:

1. Replace `database/src/queries/mod.rs` with `mod_new.rs`
2. Replace `database/src/queries/servers.rs` with `servers_new.rs`
3. Replace `database/src/queries/notifications.rs` with `notifications_new.rs`
4. Remove old plugin/task query files:
   - `plugins.rs`
   - `tasks.rs`
5. Update imports in server code to use new query functions

---

**Total Query Functions:** 100+

**Last Updated:** 2024-11-28
