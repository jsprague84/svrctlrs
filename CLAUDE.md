# CLAUDE.md - AI Development Guide

This file provides comprehensive guidance for AI assistants working with the SvrCtlRS codebase.

**Last Updated**: 2025-12-01
**Architecture Version**: v2.0 (Job-Based System)
**Status**: ✅ Production Ready - Optimized Display Models & Architecture Documentation

---

## 📈 Recent Updates

### Embedded Terminal - Phase 1 & 2 COMPLETE (2025-12-01)

**Real-time terminal for testing commands on remote servers via SSH**

**Phase 1 Features**:
- ✅ **WebSocket-based terminal** using xterm.js for real-time output streaming
- ✅ **Server selection** dropdown with all configured servers
- ✅ **Command execution** via SSH or local shell
- ✅ **ANSI color support** for rich terminal output
- ✅ **Theme integration** (Tokyo Night, Nord Dark, Light themes)
- ✅ **Copy/download output** functionality
- ✅ **Command history** with up/down arrow navigation (persisted to localStorage)
- ✅ **Test button** on command template forms to quickly test commands

**Phase 2 Features**:
- ✅ **Output search** (Ctrl+F) with incremental search and case-sensitive toggle
- ✅ **Multi-line commands** (Shift+Enter for newlines, Enter to execute)
- ✅ **Auto-expanding textarea** for multi-line command editing
- ✅ **Clickable URLs** - URLs in output are automatically detected and rendered as links

**Files Created**:
- `server/src/routes/terminal.rs` - WebSocket handler (~400 lines)
- `server/static/js/terminal.js` - TerminalManager class + Alpine.js component (~590 lines)
- `server/templates/components/terminal_modal.html` - Modal UI component (~340 lines)

**Files Modified**:
- `server/templates/base.html` - Added xterm.js CDN + terminal modal include
- `server/templates/components/command_template_form.html` - Added Test button
- `server/src/routes.rs` - Added terminal module
- `server/src/main.rs` - Added terminal routes
- `server/Cargo.toml` - Added `futures-util` dependency

**Architecture**:
```
Frontend (xterm.js + Alpine.js)
    │
    ├─ WebSocket: ws://host/ws/terminal
    │
    ▼
Backend (Axum WebSocket Handler)
    │
    ├─ Local execution: tokio::process::Command
    │
    └─ Remote execution: async_ssh2_tokio
```

**Known Limitation (Phase 1)**: Non-interactive mode only. Commands requiring PTY (sudo with password, interactive editors) will fail. This is expected - PTY allocation is planned for Phase 2.

**Usage**: Click "Test" button on any command template form, or dispatch `open-terminal` event from Alpine.js.

---

### Display Model Optimization - COMPLETE (2025-12-01)

**All display models now use optimized JOINed queries** - No N+1 query patterns, no TODO comments, no deprecated code.

**Key Improvements**:
- ✅ **JobScheduleDisplay**: Now uses actual `success_count` and `failure_count` from database (previously hardcoded to 0)
- ✅ **NotificationPolicyDisplay**: Updated to support multi-channel policies via `policy_channels` field
- ✅ **Database Query Patterns**: Specialized result structs (`*WithDetails`, `*WithNames`, `*WithCounts`) used consistently
- ✅ **Template Updates**: All HTMX templates updated to use optimized data structures
- ✅ **Deprecated Code Review**: Full codebase audit completed - no deprecated elements found
- ✅ **Architecture Documentation**: Created comprehensive `ARCHITECTURE.md` documenting:
  - Application layer structure with dependency graphs
  - Frontend-backend communication patterns (HTMX examples)
  - Database architecture with optimized query patterns
  - Display model pattern with complete examples
  - Complete data flow diagrams
  - Deprecated elements review

**Files Updated**:
- `server/src/templates.rs` - Fixed From implementations for JobScheduleDisplay, NotificationPolicyDisplay
- `database/src/queries/notifications.rs` - Added `get_policy_channel_assignments()` query
- `server/src/routes/ui/notifications.rs` - Updated to populate policy_channels
- `server/templates/components/notification_policy_list.html` - Multi-channel display
- **NEW**: `ARCHITECTURE.md` - Comprehensive architecture documentation

**Result**: Clean, optimized codebase with full documentation. All Clippy warnings resolved.

### Command Template System - COMPLETE (2025-11-30)

**Phase 1: Database Schema & Models** ✅
- Database migration 012 adds `command_templates` table
- Model structs: `CommandTemplate`, `CreateCommandTemplate`, `UpdateCommandTemplate`
- Full CRUD query layer in `database/src/queries/command_templates.rs`

**Phase 2: CRUD Operations & API** ✅
- REST endpoints for command template management
- HTMX UI routes in `server/src/routes/ui/command_templates.rs`
- Template list, create, edit, delete operations

**Phase 3: Variable Substitution Engine** ✅
- Template variable parsing with `{{variable_name}}` syntax
- Runtime variable substitution in executor
- Variable extraction and validation
- Test/preview UI for templates

**Phase 4: Runtime Execution with Audit Trail** ✅
- Migration 013 adds `rendered_command` field to `job_runs` table
- Executor stores actual executed command for audit purposes
- UI displays rendered command in job run details
- Full command traceability for security and debugging

**Phase 5: Composite Job Templates** ✅ **(FULLY IMPLEMENTED)**
- Multi-step workflow execution (already in migration 011!)
- `job_template_steps` table with ordering and conditional execution
- `step_execution_results` table for per-step tracking
- Executor: `execute_composite_job()` method in `core/src/executor.rs:477-615`
- API endpoints: Full CRUD for template steps
- UI: Step list, add/edit/delete, reordering with HTMX
- Features:
  - Sequential step execution
  - `continue_on_failure` support
  - Per-step status tracking
  - Combined output aggregation
  - Overall success/failure determination

### Earlier Phases (Pre-Command Template System)

**Schedule Override UI** (Completed Earlier)
- Alpine.js integration for dynamic template defaults
- Job schedule form populates defaults from selected job template

**General Settings Management** (Completed Earlier)
- Settings management UI with inline editing
- Support for string, number, boolean, and JSON value types

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
├── core/                       # Shared types and utilities
│   └── src/
│       ├── lib.rs             # Public API exports
│       ├── error.rs           # Error types
│       ├── executor.rs        # Job execution engine
│       ├── notifications.rs   # Notification backends (Gotify + ntfy.sh)
│       ├── remote.rs          # SSH remote execution
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
| Terminal | xterm.js | 5.3.0 (CDN) |
| Templates | Askama | 0.14 |
| Database | SQLite + sqlx | Latest |
| Runtime | Tokio | Latest |
| SSH | async_ssh2_tokio | Latest |
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

✅ **Completed & Optimized**:
- JobTypes → JobTypeDisplay
- CommandTemplates → CommandTemplateDisplay
- JobSchedules → JobScheduleDisplay (using `JobScheduleWithNames` with success/failure counts)
- NotificationPolicies → NotificationPolicyDisplay (with multi-channel support)
- Servers → ServerDisplay (using `ServerWithDetails`)
- JobTemplates → JobTemplateDisplay (using `JobTemplateWithCounts`)
- JobRuns → JobRunDisplay (with server/template name resolution)

All display models use optimized JOINed queries with specialized result structs (`*WithDetails`, `*WithNames`, `*WithCounts`).

---

### Form Handling: Multi-Value Fields (Checkboxes, Multi-Select)

**CRITICAL**: When working with HTML forms that contain checkboxes or multi-select dropdowns, you MUST use `axum_extra::extract::Form` instead of `axum::Form`.

#### Why axum_extra?

- **`axum::Form`** uses `serde_urlencoded` which **CANNOT** deserialize repeated form fields (`tag_ids=1&tag_ids=2`) into `Vec<T>`
- **`axum_extra::extract::Form`** uses `serde_html_form` which **CAN** properly handle multi-value form fields

#### Dependency Setup

```toml
# server/Cargo.toml
[dependencies]
axum-extra = { version = "0.12", features = ["form"] }
```

#### Import Pattern

```rust
// ❌ DON'T use this for multi-value fields:
use axum::Form;

// ✅ DO use this instead:
use axum_extra::extract::Form;
```

#### Form Input Struct Pattern

```rust
#[derive(Deserialize, Debug)]
pub struct CreateServerInput {
    pub name: String,
    pub hostname: String,
    pub port: Option<u16>,
    pub credential_id: Option<String>,
    pub username: Option<String>,
    pub description: Option<String>,
    pub enabled: Option<String>,

    // Multi-value field (checkboxes)
    #[serde(default)]  // Empty vec when no checkboxes selected
    pub tag_ids: Vec<i64>,  // Matches HTML: <input name="tag_ids" value="1">
}
```

#### HTML Template Pattern

```html
<!-- Multiple checkboxes with same name -->
<div class="form-group">
    <label>Tags</label>
    {% for tag in tags %}
    <label>
        <input type="checkbox"
               name="tag_ids"      <!-- Same name for all checkboxes -->
               value="{{ tag.id }}"
               {% if selected_tags.contains(&tag.id) %}checked{% endif %}>
        {{ tag.name }}
    </label>
    {% endfor %}
</div>
```

#### Handler Pattern

```rust
async fn create_server(
    State(state): State<AppState>,
    Form(input): Form<CreateServerInput>,  // axum_extra::extract::Form
) -> Result<Html<String>, AppError> {
    // ... create server logic ...

    // Handle tags (Vec<i64> is guaranteed to exist, empty or not)
    for tag_id in &input.tag_ids {
        tags::add_server_tag(db.pool(), server_id, *tag_id).await?;
    }

    // ... rest of handler ...
}
```

#### Common Pitfall: 422 Unprocessable Entity

If you see `422 Unprocessable Entity` errors with 0ms latency when submitting forms with checkboxes:
- This means deserialization failed **before** reaching your handler
- Check that you're using `axum_extra::extract::Form` (not `axum::Form`)
- Check that your struct field is `Vec<T>` with `#[serde(default)]`
- Check that HTML inputs have matching `name` attributes

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

### Documentation
- **`ARCHITECTURE.md`** - Comprehensive architecture documentation
  - Application layer structure and dependency graphs
  - Frontend-backend communication patterns with HTMX examples
  - Database architecture with optimized query patterns
  - Display model pattern with complete implementation examples
  - Complete data flow diagrams
  - Code health audit (no deprecated elements)

### Database
- `database/migrations/011_complete_restructure.sql` - Current schema
- `database/src/models/` - Database models (use for DB operations)
- `database/src/queries/` - Query functions with optimized JOINed patterns

### Server
- `server/src/main.rs` - Server entry point
- `server/src/state.rs` - Application state (DB pool, SSH pool)
- `server/src/ssh.rs` - SSH connection management
- `server/src/templates.rs` - Display models (use for templates)
- `server/src/routes/terminal.rs` - WebSocket terminal handler
- `server/src/routes/ui/` - HTMX UI route handlers
- `server/templates/` - Askama HTML templates
- `server/static/js/terminal.js` - Terminal manager (xterm.js)

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

## 🎯 Command Template System

### Overview

The **Command Template System** enables creating reusable, parameterized commands for job templates. This provides a flexible way to define operations with variable substitution, validation, and testing capabilities.

### Architecture Components

**1. Command Templates** (`command_templates` table)
- Reusable command patterns with `{{variable}}` placeholders
- Parameter schemas (JSON) defining required/optional variables
- Type support: string, number, boolean, select
- Belongs to a job type for organization

**2. Job Templates** (`job_templates` table)
- References a command template via `command_template_id`
- Stores parameter values in `variables` (JSON)
- Variables merged with command template at runtime

**3. Job Runs** (`job_runs` table)
- Executes rendered command with substituted variables
- Inherits configuration from job template
- Stores rendered command in `rendered_command` field for audit trail
- Audit field shows exact command that was executed on the server

### Data Flow

```
1. Admin creates Command Template
   └─ Template: "apt-get install {{package_name}}"
   └─ Schema: [{"name": "package_name", "type": "string", "required": true}]

2. Admin creates Job Template
   └─ Selects command template
   └─ AJAX loads parameter form fields
   └─ Fills in: package_name = "nginx"
   └─ Stored as: {"package_name": "nginx"}

3. Job Run executes
   └─ Loads template + variables
   └─ Renders: "apt-get install nginx"
   └─ Executes on target server
   └─ Stores rendered command in job_runs.rendered_command for audit trail
```

### Implementation Details

**Parameter Schema (JSON)**
```json
[
  {
    "name": "package_name",
    "type": "string",
    "description": "Package to install",
    "required": true,
    "default": null,
    "options": null
  },
  {
    "name": "version",
    "type": "string",
    "description": "Package version",
    "required": false,
    "default": "latest",
    "options": null
  },
  {
    "name": "auto_restart",
    "type": "boolean",
    "description": "Restart service after install",
    "required": false,
    "default": "false",
    "options": null
  },
  {
    "name": "priority",
    "type": "select",
    "description": "Installation priority",
    "required": true,
    "default": "normal",
    "options": ["low", "normal", "high"]
  }
]
```

**Variable Extraction Pattern**
```rust
// Form fields named: var_package_name, var_version, var_auto_restart
#[derive(Deserialize)]
pub struct CreateJobTemplateInput {
    pub name: String,
    pub command_template_id: Option<i64>,
    #[serde(flatten)]
    pub extra_fields: HashMap<String, String>,  // Captures var_* fields
}

impl CreateJobTemplateInput {
    fn extract_variables(&self) -> Option<HashMap<String, String>> {
        let variables: HashMap<String, String> = self
            .extra_fields
            .iter()
            .filter_map(|(key, value)| {
                key.strip_prefix("var_")
                    .map(|var_name| (var_name.to_string(), value.clone()))
            })
            .collect();

        if variables.is_empty() { None } else { Some(variables) }
    }
}
```

**Dynamic Parameter Loading (HTMX)**
```html
<!-- Job template form -->
<select id="command_template_id"
        name="command_template_id"
        @change="if (this.value) {
            htmx.ajax('GET', '/command-templates/' + this.value + '/parameters',
                     {target:'#command-parameters', swap:'innerHTML'});
        }">
    <option value="">Select a command...</option>
    {% for ct in command_templates %}
    <option value="{{ ct.id }}">{{ ct.display_name }}</option>
    {% endfor %}
</select>

<!-- Dynamic parameter container -->
<div id="command-parameters"></div>
```

**Parameter Form Rendering**
```rust
// Handler: server/src/routes/ui/job_types.rs:1137
pub async fn get_command_template_parameters(
    State(state): State<AppState>,
    Path(template_id): Path<i64>,
    Query(query): Query<ParametersQuery>,
) -> Result<Html<String>, AppError> {
    let template = queries::get_command_template(&state.pool, template_id).await?;

    // Parse existing variables if editing
    let existing_vars: HashMap<String, String> =
        query.variables
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok())
            .unwrap_or_default();

    // Parse parameter schema
    let schema = template.get_parameter_schema();
    let parameters: Vec<ParameterDisplay> = if let Some(arr) = schema.as_array() {
        arr.iter()
            .filter_map(|v| ParameterDisplay::from_json(v, &existing_vars))
            .collect()
    } else {
        Vec::new()
    };

    // Render template: command_template_parameters.html
    let tmpl = CommandTemplateParametersTemplate { parameters };
    Ok(Html(tmpl.render()?))
}
```

### Key Files

| Component | File | Lines |
|-----------|------|-------|
| **UI Routes** | | |
| Command template management | `server/src/routes/ui/job_types.rs` | 1-1200 |
| Job template management | `server/src/routes/ui/job_templates.rs` | 1-500 |
| Parameter loader (AJAX) | `server/src/routes/ui/job_types.rs` | 1137-1189 |
| **Templates** | | |
| Command template list | `server/templates/components/command_template_list.html` | 1-100 |
| Command template form | `server/templates/components/command_template_form.html` | 1-150 |
| Command template test | `server/templates/components/command_template_test.html` | 1-82 |
| Test result display | `server/templates/components/command_template_test_result.html` | 1-48 |
| Parameter fields (AJAX) | `server/templates/components/command_template_parameters.html` | 1-72 |
| Job template form | `server/templates/components/job_template_form.html` | 70-88, 212-229 |
| **Database** | | |
| Schema migration | `database/migrations/011_complete_restructure.sql` | Lines with command_templates |
| **Display Models** | | |
| Template displays | `server/src/templates.rs` | CommandTemplateDisplay, ParameterDisplay |

### Features Completed

#### Phase 1: Schema & CRUD ✅
- Database tables (command_templates, relationships)
- CRUD operations for command templates
- UI for creating/editing/deleting templates
- Parameter schema storage (JSON)

#### Phase 2: Validation & Testing ✅
- Variable extraction using regex (`{{variable_name}}`)
- Parameter validation against schema
- Command rendering with test values
- Testing UI with live preview
- Error reporting for invalid templates

#### Phase 3: Template Execution Integration ✅
- Job template form integration
- Dynamic parameter field loading (AJAX)
- Variable storage in job_templates.variables
- Automatic form field generation from schema
- Pre-population of existing values when editing

### Usage Example

**1. Create Command Template** (`/job-types/1`)
```
Name: update_package
Display Name: Update System Package
Command: apt-get update && apt-get install -y {{package_name}}{{#if version}}={{version}}{{/if}}
Parameters:
  - package_name: string, required
  - version: string, optional
```

**2. Test Command Template**
```
Test Values:
  package_name: nginx
  version: 1.20

Rendered Command:
  apt-get update && apt-get install -y nginx=1.20

✅ Validation passed!
```

**3. Create Job Template** (`/job-templates/new`)
```
Name: weekly_nginx_update
Job Type: System Maintenance
Command Template: Update System Package
Parameters (auto-loaded):
  package_name: nginx
  version: latest

Stored in job_templates.variables:
  {"package_name": "nginx", "version": "latest"}
```

**4. Execute Job Run**
```
Job Template: weekly_nginx_update
Rendered Command: apt-get update && apt-get install -y nginx=latest
Target: server-01 (SSH)
Status: Success
```

### Development Notes

**Display Model Pattern** (Critical for Templates)
```rust
// ❌ Database model (has serde_json::Value)
pub struct CommandTemplate {
    pub parameter_schema: Option<JsonValue>,  // Cannot use in Askama
}

// ✅ Display model (template-friendly)
pub struct CommandTemplateDisplay {
    pub parameter_schema_json: String,  // Pre-serialized for display
    pub variables: Vec<String>,         // Computed from template
}

impl From<CommandTemplate> for CommandTemplateDisplay {
    fn from(ct: CommandTemplate) -> Self {
        let variables = extract_template_variables(&ct.command);
        Self {
            parameter_schema_json: ct.parameter_schema
                .map(|v| serde_json::to_string_pretty(&v).unwrap_or_default())
                .unwrap_or_default(),
            variables,
        }
    }
}
```

**Variable Naming Convention**
- Template placeholders: `{{variable_name}}`
- Form field names: `var_variable_name`
- Stored JSON: `{"variable_name": "value"}`
- Backend extracts by stripping `var_` prefix

**AJAX Pattern for Dynamic Forms**
1. User selects command template from dropdown
2. Alpine.js `@change` event triggers AJAX
3. HTMX loads `/command-templates/{id}/parameters`
4. Backend renders parameter form fields
5. Fields injected into `#command-parameters` container
6. Form submission includes all `var_*` fields
7. Backend extracts variables and stores as JSON

---

## 🖥️ Embedded Terminal System

### Overview

The **Embedded Terminal System** provides a real-time WebSocket-based terminal for testing and debugging commands on remote servers. It integrates with the command template system to allow quick testing before deploying jobs.

### Architecture Components

**1. WebSocket Handler** (`server/src/routes/terminal.rs`)
- Axum WebSocket upgrade handler
- JSON message protocol for commands and responses
- Supports local and SSH remote execution

**2. Terminal Manager** (`server/static/js/terminal.js`)
- xterm.js terminal instance management
- Theme-aware (Tokyo Night, Nord, Light)
- Command history with localStorage persistence
- Output capture for copy/download

**3. Modal Component** (`server/templates/components/terminal_modal.html`)
- Alpine.js-based modal with server selection
- Connection status indicators
- Action buttons (Clear, Copy, Download, Reconnect)

### WebSocket Protocol

**Request Messages** (Client → Server):
```json
// Execute command
{
  "type": "execute",
  "server_id": 1,
  "command": "ls -la",
  "cols": 80,
  "rows": 24
}

// Resize terminal
{
  "type": "resize",
  "cols": 120,
  "rows": 40
}

// Keep-alive ping
{
  "type": "ping"
}
```

**Response Messages** (Server → Client):
```json
// Command output
{
  "type": "output",
  "data": "file1.txt\nfile2.txt\n"
}

// Command completed
{
  "type": "exit",
  "data": "✓ Process exited with code: 0",
  "exit_code": 0
}

// Error occurred
{
  "type": "error",
  "data": "✗ Error: SSH connection failed",
  "exit_code": -1
}

// Keep-alive response
{
  "type": "pong"
}
```

### SSH Execution Flow

```
1. WebSocket receives "execute" message
   └─ server_id, command

2. Load server from database
   └─ queries::servers::get_server()

3. Load credential if assigned
   └─ queries::credentials::get_credential()

4. Determine auth method
   ├─ SSH Key: Read from cred.value (file path)
   │           Passphrase from cred.metadata["passphrase"]
   ├─ Password: Use cred.value directly
   └─ Default: Try ~/.ssh/id_rsa or ~/.ssh/id_ed25519

5. Connect via async_ssh2_tokio
   └─ Client::connect((hostname, port), username, auth_method, NoCheck)

6. Execute command
   └─ client.execute(command)

7. Stream output back via WebSocket
   └─ stdout (plain), stderr (red), exit code (green/red)
```

### Key Implementation Details

**Credential Handling** (Important for AI Assistants):
```rust
// Credentials store SSH keys as FILE PATHS, not raw key content
let auth_method = if let Some(cred) = credential {
    match cred.credential_type() {  // Method call, not field access
        Some(CredentialType::SshKey) => {
            let key_path = &cred.value;  // Path to key file
            let key_content = tokio::fs::read_to_string(key_path).await?;

            // Passphrase stored in metadata JSON
            let metadata = cred.get_metadata();
            let passphrase = metadata.get("passphrase")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());

            AuthMethod::with_key(&key_content, passphrase)
        }
        Some(CredentialType::Password) => {
            AuthMethod::with_password(&cred.value)
        }
    }
};
```

**Local vs Remote Execution**:
```rust
let result = if server.is_local {
    // Local: Use tokio::process::Command
    execute_local_command(command).await
} else {
    // Remote: Use async_ssh2_tokio
    execute_ssh_command(&server, credential.as_ref(), command).await
};
```

### Theme Integration

Terminal colors automatically match the application theme:

```javascript
// terminal.js - Theme detection
const isDark = document.documentElement.getAttribute('data-theme') !== 'light';
this.terminal.options.theme = isDark ? this.getDarkTheme() : this.getLightTheme();

// Watch for theme changes
const observer = new MutationObserver(() => {
    const isDark = document.documentElement.getAttribute('data-theme') !== 'light';
    this.terminal.options.theme = isDark ? this.getDarkTheme() : this.getLightTheme();
});
observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
```

### Opening the Terminal

**From Command Template Form**:
```html
<button type="button"
        @click="$dispatch('open-terminal', { command: document.getElementById('command').value })">
    <i data-lucide="terminal"></i> Test
</button>
```

**From Any Alpine.js Component**:
```javascript
// Open with pre-filled command
$dispatch('open-terminal', { command: 'docker ps', serverId: 1 })

// Open empty
$dispatch('open-terminal')
```

**From JavaScript**:
```javascript
// Dispatch custom event
window.dispatchEvent(new CustomEvent('open-terminal', {
    detail: { command: 'hostname', serverId: 2 }
}));
```

### Key Files Reference

| Component | File | Purpose |
|-----------|------|---------|
| WebSocket Handler | `server/src/routes/terminal.rs` | Backend WebSocket logic |
| Terminal Manager | `server/static/js/terminal.js` | xterm.js wrapper + Alpine component |
| Modal Template | `server/templates/components/terminal_modal.html` | UI with styles |
| Base Template | `server/templates/base.html` | CDN includes + modal injection |
| Command Form | `server/templates/components/command_template_form.html` | Test button integration |

### External Dependencies (CDN)

```html
<!-- xterm.js core -->
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/xterm@5.3.0/css/xterm.css" />
<script src="https://cdn.jsdelivr.net/npm/xterm@5.3.0/lib/xterm.js"></script>

<!-- xterm.js addons -->
<script src="https://cdn.jsdelivr.net/npm/xterm-addon-fit@0.8.0/lib/xterm-addon-fit.js"></script>
<script src="https://cdn.jsdelivr.net/npm/xterm-addon-search@0.13.0/lib/xterm-addon-search.js"></script>
```

### Phase 2 Roadmap (Future)

- **PTY Allocation**: Enable interactive commands (sudo, vim, etc.)
- **Session Persistence**: Keep terminal sessions alive across page reloads
- **Multiple Terminals**: Tab support for concurrent sessions
- **Real-time Streaming**: Stream output during long-running commands

---

## 📌 Project Information

- **Owner**: Johnathon Sprague (jsprague84)
- **GitHub**: https://github.com/jsprague84/svrctlrs
- **Original Project**: weatherust (reference for feature parity)
- **Test Environment**: docker-vm
- **Primary Use**: Infrastructure automation via SSH

---

**IMPORTANT NOTES FOR AI ASSISTANTS**:

1. **Architecture uses job-based system** - NO plugin system exists
2. **Read migration 011** to understand current schema
3. **Use Display models** for ALL complex types in Askama templates
4. **Check server/src/routes/ui/** for current UI implementation patterns
5. **NO plugins directory** - Plugin system completely removed
6. **Use job types + command templates** for all automation tasks

**Archive**: Previous documentation saved to `CLAUDE.archive.md` (not in repo)
