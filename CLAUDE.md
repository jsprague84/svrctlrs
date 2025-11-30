# CLAUDE.md - AI Development Guide

This file provides comprehensive guidance for AI assistants working with the SvrCtlRS codebase.

**Last Updated**: 2025-11-30
**Architecture Version**: v2.0 (Job-Based System)
**Status**: ✅ Active Development - Phase 6 Complete

---

## 📈 Recent Updates

### Phase 5: Schedule Override UI (Completed 2025-11-30)
- ✅ Alpine.js integration for dynamic template defaults
- ✅ Job schedule form populates defaults from selected job template
- ✅ Client-side state management for better UX
- ✅ Automatic population of timeout, retry, and notification settings

### Phase 6: General Settings Management (Completed 2025-11-30)
- ✅ Settings management UI with inline editing
- ✅ `/settings/general` page for application-wide settings
- ✅ Database model and query layer for settings CRUD
- ✅ Support for string, number, boolean, and JSON value types
- ✅ HTMX-based inline editing (similar to job schedules pattern)

**Database Fix** (2025-11-30):
- Fixed settings table column name mismatch (`type` → `value_type`)
- Corrected sqlx model annotations
- All routes now working correctly (commit `6a5203b`)

---

## 🎯 Project Mission

**SvrCtlRS** (Server Control Rust) is a **job-based infrastructure automation platform** for managing Linux servers and Docker containers via SSH, featuring a modern HTMX web UI.

**Key Innovation**: Complete restructure from plugin-based to **job-based architecture** with:
- Built-in command templates
- Composite workflows (multi-step jobs)
- Server capability detection
- Credential management
- Tag-based organization

---

## 📋 Current Architecture

### **Job-Based System** (Migration 011 - Complete Restructure)

**Old System** (DEPRECATED):
- ❌ Plugins (hardcoded monitoring features)
- ❌ Tasks (simple scheduled commands)
- ❌ No remote execution framework
- ❌ No workflow support

**New System** (CURRENT):
- ✅ **Job Types**: Categories of work (docker, os_maintenance, backups, custom)
- ✅ **Command Templates**: Reusable commands with `{{variable}}` substitution
- ✅ **Job Templates**: User-defined jobs (simple or composite workflows)
- ✅ **Job Schedules**: Cron-scheduled job instances on specific servers
- ✅ **Job Runs**: Execution history with full output capture
- ✅ **Server Capabilities**: Auto-detected (docker, systemd, apt, dnf, etc.)
- ✅ **Credentials**: SSH keys, API tokens, managed securely
- ✅ **Tags**: Server organization (prod, staging, docker-hosts, etc.)

---

## 🏗️ Directory Structure

```
svrctlrs/
├── core/                       # Shared types, plugin system (legacy)
│   └── src/
│       ├── lib.rs             # Public API exports
│       ├── error.rs           # Error types
│       ├── plugin.rs          # Plugin trait (DEPRECATED - for old plugins)
│       ├── notifications.rs   # Notification backends (Gotify + ntfy.sh)
│       ├── remote.rs          # SSH remote execution (DEPRECATED)
│       └── types.rs           # Shared types
│
├── server/                     # Axum backend + HTMX UI
│   ├── src/
│   │   ├── main.rs            # Server entry point
│   │   ├── config.rs          # Configuration loading
│   │   ├── state.rs           # Application state
│   │   ├── routes.rs          # Route registration
│   │   ├── templates.rs       # Askama template structs + Display models
│   │   ├── ssh.rs             # SSH connection pool
│   │   ├── routes/
│   │   │   ├── api.rs         # REST API endpoints
│   │   │   ├── servers.rs     # Server management API
│   │   │   ├── webhooks.rs    # Webhook endpoints
│   │   │   └── ui/            # HTMX UI routes
│   │   │       ├── auth.rs
│   │   │       ├── credentials.rs
│   │   │       ├── dashboard.rs
│   │   │       ├── job_runs.rs
│   │   │       ├── job_schedules.rs
│   │   │       ├── job_templates.rs
│   │   │       ├── job_types.rs
│   │   │       ├── notifications.rs
│   │   │       ├── servers.rs
│   │   │       ├── settings.rs
│   │   │       └── tags.rs
│   │   └── filters.rs         # Custom Askama filters
│   │
│   ├── templates/              # Askama HTML templates
│   │   ├── base.html          # Base layout
│   │   ├── pages/             # Full page templates
│   │   │   ├── dashboard.html
│   │   │   ├── servers.html
│   │   │   ├── job_types.html
│   │   │   ├── job_templates.html
│   │   │   ├── job_schedules.html
│   │   │   ├── job_runs.html
│   │   │   └── ...
│   │   └── components/        # HTMX partials
│   │       ├── server_list.html
│   │       ├── job_type_list.html
│   │       ├── job_type_form.html
│   │       ├── job_type_view.html
│   │       └── ...
│   │
│   └── static/                 # Static assets
│       ├── css/styles.css     # Nord-inspired theme
│       └── js/                # HTMX + Alpine.js
│
├── scheduler/                  # Built-in cron scheduler
│   └── src/
│       └── lib.rs             # Cron expression evaluator
│
├── database/                   # SQLite abstraction
│   ├── src/
│   │   ├── lib.rs             # Database connection + migrations
│   │   ├── notification_service.rs  # Notification backend queries
│   │   ├── models/            # Database models
│   │   │   ├── credential.rs
│   │   │   ├── job_run.rs
│   │   │   ├── job_schedule.rs
│   │   │   ├── job_template.rs
│   │   │   ├── job_type.rs
│   │   │   ├── notification.rs
│   │   │   ├── server.rs
│   │   │   ├── setting.rs
│   │   │   ├── tag.rs
│   │   │   └── ...
│   │   └── queries/           # Database query functions
│   │       ├── credentials.rs
│   │       ├── job_runs.rs
│   │       ├── job_schedules.rs
│   │       ├── job_templates.rs
│   │       ├── job_types.rs
│   │       ├── notifications.rs
│   │       ├── servers.rs
│   │       ├── settings.rs
│   │       └── tags.rs
│   │
│   └── migrations/            # SQL migrations
│       ├── 000_initial_schema.sql
│       ├── ...
│       └── 011_complete_restructure.sql  # ← CURRENT SCHEMA
│
└── plugins/                   # OLD monitoring plugins (DEPRECATED)
    ├── docker/                # Legacy - being replaced by job types
    ├── updates/
    ├── health/
    ├── weather/
    └── speedtest/
```

---

## 💾 Database Schema (Current)

### Core Entities

1. **credentials** - SSH keys, API tokens, passwords
2. **tags** - Server organization labels
3. **servers** - Execution targets (local or remote via SSH)
4. **server_tags** - Many-to-many server ↔ tags
5. **server_capabilities** - Auto-detected capabilities per server

### Job System

6. **job_types** - Categories (docker, os_maintenance, backup, custom)
7. **command_templates** - Reusable commands with `{{variables}}`
8. **job_templates** - User-defined jobs (simple or composite)
9. **job_template_steps** - Multi-step workflow definitions
10. **job_schedules** - Cron-scheduled jobs on specific servers
11. **job_runs** - Execution history with full output
12. **server_job_results** - Per-server results for multi-server jobs

### Notifications

13. **notification_policies** - Reusable notification configs
14. **notification_channels** - Gotify/ntfy.sh backends
15. **notifications** - Sent notification history

### Settings

16. **settings** - Key-value configuration store

---

## 🔧 Technology Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Backend | Axum | 0.8 |
| Frontend | HTMX + Alpine.js | 2.0.3 + 3.14.1 |
| Templates | Askama | 0.14 |
| Database | SQLite + sqlx | Latest |
| Runtime | Tokio | Latest |
| SSH | openssh_sftp_client | Latest |
| Container | Docker | Latest |

---

## 🎨 HTMX + Askama Patterns

### Display Model Pattern (CRITICAL)

**Problem**: Askama templates cannot handle `serde_json::Value`, `HashMap`, or complex Serialize types.

**Solution**: Create "Display" models that convert database models to template-friendly types.

#### Pattern Rules

1. **Remove Serialize/Deserialize** - Display models should NOT derive these
2. **Pre-serialize JSON fields** - Convert `Option<JsonValue>` to `String`
3. **Use From trait** - Implement `From<DatabaseModel>` for automatic conversion
4. **Format timestamps** - Convert `DateTime<Utc>` to `String` with local timezone
5. **Extract computed values** - Calculate before moving fields (borrow checker)

#### Example Implementation

**Database Model** (`database/src/models.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobType {
    pub id: i64,
    pub name: String,
    pub required_capabilities: Option<JsonValue>,  // ❌ Cannot use in templates
    pub metadata: Option<JsonValue>,                // ❌ Cannot use in templates
    pub created_at: DateTime<Utc>,                  // ❌ Cannot format in templates
}

impl JobType {
    pub fn get_required_capabilities(&self) -> Vec<String> {
        // Extract from JSON
    }
}
```

**Display Model** (`server/src/templates.rs`):
```rust
use chrono::Local;

#[derive(Debug, Clone)]  // ✅ NO Serialize/Deserialize!
pub struct JobTypeDisplay {
    pub id: i64,
    pub name: String,

    // ✅ Pre-serialized JSON (String instead of JsonValue)
    pub required_capabilities_json: String,
    pub metadata_json: String,

    // ✅ Formatted timestamps (String instead of DateTime)
    pub created_at: String,

    // ✅ Computed display-only fields
    pub required_capabilities: Vec<String>,
    pub command_template_count: i64,
}

impl From<svrctlrs_database::models::JobType> for JobTypeDisplay {
    fn from(jt: svrctlrs_database::models::JobType) -> Self {
        // Extract computed values BEFORE moving fields
        let required_capabilities = jt.get_required_capabilities();

        // Pre-serialize JSON
        let metadata_json = serde_json::to_string(&jt.metadata)
            .unwrap_or_else(|_| "{}".to_string());

        // Format timestamp
        let created_at = jt.created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: jt.id,
            name: jt.name,
            metadata_json,
            created_at,
            required_capabilities,
            command_template_count: 0,  // TODO: Load via JOIN
        }
    }
}
```

**Route Handler**:
```rust
async fn job_types_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let job_types = state.db.get_all_job_types().await?;

    // ✅ Automatic From conversion
    let job_types: Vec<JobTypeDisplay> = job_types
        .into_iter()
        .map(Into::into)
        .collect();

    let template = JobTypesPageTemplate { job_types };
    Ok(Html(template.render()?))
}
```

**Template**:
```html
{% for jt in job_types %}
<div class="card">
    <h3>{{ jt.name }}</h3>
    <p>Created: {{ jt.created_at }}</p>  <!-- ✅ Formatted string -->

    {% for cap in jt.required_capabilities %}  <!-- ✅ Can iterate Vec -->
        <span class="badge">{{ cap }}</span>
    {% endfor %}

    <!-- ✅ Can use JSON in Alpine.js -->
    <div x-data='{ metadata: {{ jt.metadata_json }} }'></div>
</div>
{% endfor %}
```

#### Modules Using Display Pattern

✅ **Completed**:
- JobTypes → JobTypeDisplay
- CommandTemplates → CommandTemplateDisplay

⏳ **In Progress** (models exist, templates need updates):
- JobTemplates → JobTemplateDisplay
- JobTemplateSteps → JobTemplateStepDisplay
- JobSchedules → JobScheduleDisplay
- JobRuns → JobRunDisplay
- ServerJobResults → ServerJobResultDisplay

---

## 🔨 Development Workflows

### Working with Job Types

```rust
use svrctlrs_database::{models::CreateJobType, queries::job_types};

// Create a job type
let docker_type = CreateJobType {
    name: "docker_operations".to_string(),
    display_name: "Docker Operations".to_string(),
    description: Some("Manage Docker containers and images".to_string()),
    requires_capabilities: Some(json!(["docker"])),
    enabled: true,
    ..Default::default()
};

let id = job_types::create_job_type(&pool, &docker_type).await?;
```

### Working with Command Templates

```rust
use svrctlrs_database::{models::CreateCommandTemplate, queries::job_types};

// Create a command template with variable substitution
let template = CreateCommandTemplate {
    job_type_id: docker_type_id,
    name: "list_containers".to_string(),
    display_name: "List Containers".to_string(),
    command: "docker ps --filter 'status={{status}}'".to_string(),
    required_capabilities: Some(json!(["docker"])),
    timeout_seconds: 30,
    ..Default::default()
};

job_types::create_command_template(&pool, &template).await?;
```

### Working with Job Templates

```rust
use svrctlrs_database::{models::CreateJobTemplate, queries::job_templates};

// Simple job (single command)
let job = CreateJobTemplate {
    name: "list_running_containers".to_string(),
    display_name: "List Running Containers".to_string(),
    job_type_id: docker_type_id,
    is_composite: false,
    command_template_id: Some(list_containers_template_id),
    variables: Some(json!({"status": "running"})),
    ..Default::default()
};

let id = job_templates::create_job_template(&pool, &job).await?;
```

### Scheduling Jobs

```rust
use svrctlrs_database::{models::CreateJobSchedule, queries::job_schedules};

// Schedule job to run every hour
let schedule = CreateJobSchedule {
    name: "hourly_container_check".to_string(),
    job_template_id: job_template_id,
    server_id: server_id,
    schedule: "0 * * * *".to_string(),  // Cron expression
    enabled: true,
    ..Default::default()
};

job_schedules::create_job_schedule(&pool, &schedule).await?;
```

---

## 📝 Code Standards

### Error Handling

```rust
use anyhow::{Context, Result};

pub async fn my_function() -> Result<()> {
    let data = fetch_data()
        .await
        .context("Failed to fetch data")?;

    process_data(&data)
        .context("Failed to process data")?;

    Ok(())
}
```

### Logging

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(sensitive_data))]
pub async fn my_function(id: &str, sensitive_data: &str) -> Result<()> {
    info!(id, "Starting operation");

    match perform_operation().await {
        Ok(result) => {
            info!(id, "Operation succeeded");
            Ok(result)
        }
        Err(e) => {
            error!(id, error = %e, "Operation failed");
            Err(e)
        }
    }
}
```

---

## 🚀 CI/CD Workflows

### Two-Workflow Strategy

**Develop Branch** (`.github/workflows/docker-publish-develop.yml`):
- **Trigger**: Push to `develop`
- **Platform**: AMD64 only
- **Build Time**: ~5-8 minutes
- **Image**: `ghcr.io/jsprague84/svrctlrs:develop`
- **Purpose**: Fast iteration

**Main Branch** (`.github/workflows/docker-publish-main.yml`):
- **Trigger**: Push to `main` or version tags
- **Platforms**: AMD64 + ARM64
- **Build Time**: ~15-20 minutes
- **Images**: `latest`, `main`, `v*.*.*`
- **Purpose**: Production releases

---

## 🚨 Common Pitfalls

### Things to Avoid

1. ❌ **Don't use old plugin system** - Use job types instead
2. ❌ **Don't use core/remote.rs** - Use server/ssh.rs instead
3. ❌ **Don't skip Display models** - Required for complex types in templates
4. ❌ **Don't use unwrap()** - Use proper error handling
5. ❌ **Don't hard-code capabilities** - Check server_capabilities table
6. ❌ **Don't bypass credential management** - Use credentials table

### Things to Remember

1. ✅ **Job Types = Categories** (docker, os_maintenance, backup)
2. ✅ **Command Templates = Reusable commands** with `{{variables}}`
3. ✅ **Job Templates = User-defined jobs** (simple or composite)
4. ✅ **Job Schedules = Cron-scheduled instances** on specific servers
5. ✅ **Display Models = Template-safe types** (no Serialize/Deserialize)
6. ✅ **Check migration 011** for current schema

---

## 📚 Key Files Reference

### Database
- `database/migrations/011_complete_restructure.sql` - Current schema
- `database/src/models/` - Database models (use for DB operations)
- `database/src/queries/` - Query functions (use instead of raw SQL)

### Server
- `server/src/main.rs` - Server entry point
- `server/src/state.rs` - Application state (DB pool, SSH pool)
- `server/src/ssh.rs` - SSH connection management
- `server/src/templates.rs` - Display models (use for templates)
- `server/src/routes/ui/` - HTMX UI route handlers
- `server/templates/` - Askama HTML templates

### Configuration
- `config/example.toml` - Example configuration
- `docker-compose.yml` - Docker Compose setup
- `Dockerfile` - Multi-stage Docker build

---

## 💡 Quick Reference

### Migration Path: Old → New

| Old Concept | New Concept | Migration |
|------------|-------------|-----------|
| Plugins | Job Types | Define job type, create command templates |
| Tasks | Job Schedules | Create job template, schedule on server |
| Plugin config | Command Templates | Create template with variables |
| Remote executor (core) | SSH pool (server) | Use AppState.ssh_pool |
| Hard-coded commands | Command templates | Create reusable templates |

### Key Architecture Changes

1. **Plugins → Job Types**: Hardcoded monitoring replaced by user-defined job categories
2. **Tasks → Job Schedules**: Simple commands replaced by scheduled job template instances
3. **No workflows → Composite Jobs**: Added multi-step workflow support
4. **Static targets → Server Management**: Added SSH pool, capability detection, tags
5. **Embedded creds → Credential Store**: Centralized SSH key and token management

---

## ⚠️ Known Limitations & Workarounds

### Askama Template Comparison Errors (RESOLVED)

**Issue**: Askama 0.14 has type system limitations with reference comparisons in templates.

**Solution**: Use Alpine.js `x-init` to set form selection state client-side instead of server-side `selected` attributes.

**Implementation Pattern**:
```html
<!-- Instead of: -->
<option value="{{ id }}" {% if id == other_id %}selected{% endif %}>

<!-- Use: -->
<select x-init="$el.value = '{{ id }}'">
  <option value="{{ id }}">
```

**Applied To**:
- `job_template_form.html` - job_type_id and command_template_id selection
- `job_template_step_form.html` - job_type_id selection
- `notification_policy_form.html` - channel_id, job_type_id, and job_template checkboxes

**Benefits**:
- ✅ Avoids Askama reference type comparison errors
- ✅ Leverages existing Alpine.js dependency
- ✅ Maintains same UX (forms pre-populate correctly)
- ✅ No performance impact (Alpine.js runs on page load)

**Status**: ✅ Resolved - all forms compile and function correctly

---

## 🔗 External References

### Documentation
- Axum: https://docs.rs/axum
- HTMX: https://htmx.org/docs/
- Askama: https://docs.rs/askama
- Alpine.js: https://alpinejs.dev/
- Tokio: https://docs.rs/tokio
- sqlx: https://docs.rs/sqlx

---

## 📌 Project Information

- **Owner**: Johnathon Sprague (jsprague84)
- **GitHub**: https://github.com/jsprague84/svrctlrs
- **Original Project**: weatherust (reference for feature parity)
- **Test Environment**: docker-vm
- **Primary Use**: Infrastructure automation via SSH

---

**IMPORTANT NOTES FOR AI ASSISTANTS**:

1. **Architecture has been completely restructured** - Ignore old plugin-focused documentation
2. **Read migration 011** to understand current schema
3. **Use Display models** for ALL complex types in Askama templates
4. **Check server/src/routes/ui/** for current UI implementation patterns
5. **Old plugins/** directory is deprecated** - Do not extend old plugin system
6. **Use job types + command templates** instead of creating new plugins

**Archive**: Previous documentation saved to `CLAUDE.archive.md` (not in repo)
