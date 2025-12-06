# CLAUDE.md - AI Development Guide

This file provides comprehensive guidance for AI assistants (Claude, GPT, etc.) when working with the SvrCtlRS codebase.

## 🎯 Project Mission

**SvrCtlRS** (Server Control Rust) is a plugin-based infrastructure monitoring and automation platform for Linux servers and Docker containers, featuring a modern HTMX web UI.

**Original Project**: `/home/jsprague/Development/weatherust` (reference for feature parity)

## 📋 Current Status

**Version**: v2.1.0  
**Last Updated**: 2024-11-25  
**Status**: ✅ Production Ready

### Completed Features

- ✅ Plugin architecture with core traits
- ✅ Notification backends (Gotify + ntfy.sh)
- ✅ Axum 0.8 backend with REST API
- ✅ Built-in cron-like scheduler
- ✅ SQLite database layer
- ✅ Docker, Updates, Health plugins
- ✅ **HTMX + Askama web UI** (migrated from Dioxus)
- ✅ GitHub Actions CI/CD workflows
- ✅ Docker multi-arch builds (AMD64 + ARM64)

### Technology Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Backend | Axum | 0.8 |
| Frontend | HTMX + Alpine.js | 2.0.3 + 3.14.1 |
| Templates | Askama | 0.12 |
| Database | SQLite + sqlx | Latest |
| Runtime | Tokio | Latest |
| Container | Docker | Latest |

## 🏗️ Architecture Overview

### Directory Structure

```
svrctlrs/
├── core/                    # Shared types, plugin system, notifications
│   └── src/
│       ├── lib.rs          # Public API exports
│       ├── error.rs        # Error types
│       ├── plugin.rs       # Plugin trait + registry
│       ├── notifications.rs # Gotify + ntfy.sh
│       ├── remote.rs       # SSH remote execution
│       └── types.rs        # Shared types
├── server/                  # Axum backend + HTMX UI
│   ├── src/
│   │   ├── main.rs         # Server entry point
│   │   ├── config.rs       # Configuration loading
│   │   ├── state.rs        # Application state
│   │   ├── ui_routes.rs    # HTMX UI route handlers
│   │   ├── templates.rs    # Askama template structs
│   │   └── routes/         # REST API routes
│   │       ├── api.rs      # API endpoints
│   │       └── webhooks.rs # Webhook endpoints
│   ├── templates/           # Askama HTML templates
│   │   ├── base.html       # Base layout
│   │   ├── pages/          # Full page templates
│   │   └── components/     # HTMX partials
│   └── static/              # Static assets
│       ├── css/styles.css  # Nord-inspired theme
│       └── js/             # HTMX + Alpine.js
├── scheduler/               # Built-in cron scheduler
├── database/                # SQLite abstraction
└── plugins/                 # Monitoring plugins
    ├── docker/             # Docker monitoring
    ├── updates/            # OS update monitoring
    ├── health/             # System health metrics
    ├── weather/            # Weather (optional)
    └── speedtest/          # Speed test (optional)
```

### Key Design Principles

1. **Plugin Architecture**: All features are plugins implementing the `Plugin` trait
2. **Service-Specific Notifications**: Each plugin can have its own Gotify key/ntfy topic
3. **Remote Execution**: SSH-based operations via `RemoteExecutor`
4. **Dual Notifications**: Both Gotify and ntfy.sh support
5. **Webhook Triggers**: HTTP endpoints for remote-triggered actions
6. **Built-in Scheduler**: No external dependencies
7. **HTMX for Interactivity**: Lightweight, server-driven UI updates

## 🔧 Development Patterns

### Plugin Implementation

```rust
use async_trait::async_trait;
use svrctlrs_core::{Plugin, PluginMetadata, Result, ScheduledTask};

pub struct MyPlugin {}

impl MyPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "myplugin".to_string(),
            name: "My Plugin".to_string(),
            description: "What it does".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            author: "SvrCtlRS".to_string(),
        }
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![
            ScheduledTask {
                id: "my_task".to_string(),
                schedule: "0 */5 * * * *".to_string(), // Every 5 minutes
                description: "Task description".to_string(),
                enabled: true,
            },
        ]
    }

    async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> {
        match task_id {
            "my_task" => self.run_task(context).await,
            _ => Ok(PluginResult::error(format!("Unknown task: {}", task_id))),
        }
    }
}
```

### HTMX UI Routes

```rust
use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
    Form,
};

#[derive(Template)]
#[template(path = "pages/mypage.html")]
pub struct MyPageTemplate {
    pub user: Option<User>,
    pub data: Vec<MyData>,
}

async fn my_page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let user = get_user_from_session().await;
    let data = state.get_my_data().await?;
    
    let template = MyPageTemplate { user, data };
    Ok(Html(template.render()?))
}

async fn create_item(
    State(state): State<AppState>,
    Form(input): Form<CreateItemInput>,
) -> Result<Html<String>, AppError> {
    // Validate and create
    state.create_item(input).await?;
    
    // Return updated list (HTMX will swap this in)
    let data = state.get_my_data().await?;
    let template = MyListTemplate { data };
    Ok(Html(template.render()?))
}
```

### Askama Templates

```html
<!-- templates/pages/mypage.html -->
{% extends "base.html" %}

{% block title %}My Page - SvrCtlRS{% endblock %}
{% block nav_mypage %}active{% endblock %}

{% block content %}
<h1>My Page</h1>

<button hx-get="/mypage/new" 
        hx-target="#form-container" 
        hx-swap="innerHTML"
        class="btn-primary">
    Add Item
</button>

<div id="form-container"></div>

<div id="item-list">
    {% include "components/item_list.html" %}
</div>
{% endblock %}
```

### Notification Pattern

```rust
use svrctlrs_core::{NotificationManager, NotificationMessage, NotificationAction};

let manager = NotificationManager::new(client.clone(), &["myplugin"])?;

manager.send_for_service(
    "myplugin",
    &NotificationMessage {
        title: "Alert Title".into(),
        body: "Alert details here".into(),
        priority: 4,
        actions: vec![
            NotificationAction::view("View Details", "https://..."),
            NotificationAction::http_post("Fix It", "https://webhook.../fix"),
        ],
    },
).await?;
```

### Remote Execution Pattern

```rust
use svrctlrs_core::{RemoteExecutor, Server};

let executor = RemoteExecutor::new(Some("/path/to/ssh/key".to_string()));
let server = Server::remote("myserver", "user@host");

let output = executor.execute(&server, "docker ps").await?;
```

### Display Model Pattern

**Problem**: Askama templates cannot handle complex types like `serde_json::Value`, `HashMap<String, serde_json::Value>`, or types with `Serialize`/`Deserialize` derives that contain JSON fields.

**Solution**: Create separate "Display" models that convert database models into template-friendly types using the `From` trait.

#### Pattern Rules

1. **Remove Serialize/Deserialize** - Display models should NOT derive `Serialize` or `Deserialize`
2. **Pre-serialize JSON fields** - Convert `Option<JsonValue>` to `String` during conversion
3. **Use From trait** - Implement `From<DatabaseModel>` for automatic conversion
4. **Format timestamps** - Convert `DateTime<Utc>` to formatted strings with local timezone
5. **Extract computed values** - Calculate values before moving fields (Rust borrow checker)

#### Example Implementation

**Database Model** (`database/src/models.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobType {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub execution_type: String,
    pub required_capabilities: Option<JsonValue>,  // Cannot use in templates!
    pub metadata: Option<JsonValue>,                // Cannot use in templates!
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobType {
    pub fn get_required_capabilities(&self) -> Vec<String> {
        // Helper to extract from JSON
    }
}
```

**Display Model** (`server/src/templates.rs`):
```rust
use chrono::Local;

#[derive(Debug, Clone)]  // NO Serialize/Deserialize!
pub struct JobTypeDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub execution_type: String,

    // Pre-serialized JSON fields (String instead of JsonValue)
    pub required_capabilities_json: String,
    pub metadata_json: String,

    // Formatted timestamps (String instead of DateTime)
    pub created_at: String,
    pub updated_at: String,

    // Computed display-only fields
    pub required_capabilities: Vec<String>,
    pub command_template_count: i64,
    pub job_template_count: i64,
}

impl From<svrctlrs_database::models::JobType> for JobTypeDisplay {
    fn from(jt: svrctlrs_database::models::JobType) -> Self {
        // Extract computed values BEFORE moving fields
        let required_capabilities = jt.get_required_capabilities();

        // Pre-serialize JSON fields
        let required_capabilities_json = serde_json::to_string(
            &jt.required_capabilities
        ).unwrap_or_else(|_| "null".to_string());

        let metadata_json = serde_json::to_string(&jt.metadata)
            .unwrap_or_else(|_| "{}".to_string());

        // Format timestamps with local timezone
        let created_at = jt.created_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let updated_at = jt.updated_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        Self {
            id: jt.id,
            name: jt.name,
            display_name: jt.display_name,
            description: jt.description,
            execution_type: jt.execution_type,
            required_capabilities_json,
            metadata_json,
            created_at,
            updated_at,
            required_capabilities,
            command_template_count: 0,  // TODO: Load via JOIN
            job_template_count: 0,      // TODO: Load via JOIN
        }
    }
}
```

**Route Handler** (`server/src/routes/ui/job_types.rs`):
```rust
use crate::templates::JobTypeDisplay;

async fn job_types_page(
    State(state): State<AppState>
) -> Result<Html<String>, AppError> {
    // Fetch database models
    let job_types = state.db.get_all_job_types().await?;

    // Convert to display models using From trait
    let job_types: Vec<JobTypeDisplay> = job_types
        .into_iter()
        .map(|jt| jt.into())  // Automatic From conversion
        .collect();

    let template = JobTypesPageTemplate { job_types };
    Ok(Html(template.render()?))
}
```

**Template** (`templates/components/job_type_list.html`):
```html
{% for jt in job_types %}
<div class="card">
    <h3>{{ jt.display_name }}</h3>
    <p>{{ jt.description|default("No description") }}</p>

    <!-- Can access String fields directly -->
    <p>Created: {{ jt.created_at }}</p>

    <!-- Can iterate over Vec fields -->
    {% for cap in jt.required_capabilities %}
        <span class="badge">{{ cap }}</span>
    {% endfor %}

    <!-- Can use pre-serialized JSON in Alpine.js -->
    <div x-data='{ metadata: {{ jt.metadata_json }} }'>
        <!-- JavaScript can parse the JSON string -->
    </div>
</div>
{% endfor %}
```

#### When to Use Display Models

✅ **Use Display Models When**:
- Database model has `Option<JsonValue>` fields
- Database model has `HashMap<K, V>` fields
- Templates need formatted timestamps
- Templates need computed/derived fields
- Templates iterate over nested collections

❌ **Don't Need Display Models When**:
- Model only has simple types (String, i64, bool, Option<String>)
- No JSON serialization in templates
- No computed fields needed

#### Common Issues and Solutions

**Issue 1: "cannot find function `length` in module `filters`"**
```rust
// Add to server/src/templates.rs
pub fn length<T>(value: &[T]) -> ::askama::Result<usize> {
    Ok(value.len())
}
```

**Issue 2: "field X is not found on type Y"**
- Missing computed field in Display model
- Add field and populate in From implementation

**Issue 3: "borrow of moved value"**
- Extracting computed value after moving field
- Extract computed values BEFORE moving fields in From impl

**Issue 4: HTML templates reference old field names**
- Templates expect database model field names
- Update templates to use Display model field names
- Or add fields with both names during transition

#### Modules Using Display Pattern

✅ **Completed**:
- JobTypes → JobTypeDisplay
- CommandTemplates → CommandTemplateDisplay

⏳ **In Progress** (models implemented, templates need updates):
- JobTemplates → JobTemplateDisplay
- JobTemplateSteps → JobTemplateStepDisplay
- JobSchedules → JobScheduleDisplay
- JobRuns → JobRunDisplay
- ServerJobResults → ServerJobResultDisplay

## 🎨 HTMX + Askama Implementation

### Why HTMX Over Dioxus?

The project migrated from Dioxus 0.7 to HTMX + Askama for:
- ✅ **Reliability**: No WASM build issues
- ✅ **Simplicity**: Pure HTML templates
- ✅ **Size**: 94KB vs 500KB+ bundle
- ✅ **Speed**: Faster builds (5-8 min vs 15-20 min)
- ✅ **Maintainability**: Easier to debug and extend

### HTMX Patterns

**Form Submission:**
```html
<form hx-post="/servers" 
      hx-target="#server-list" 
      hx-swap="innerHTML">
    <input type="text" name="name" required>
    <button type="submit">Save</button>
</form>
```

**Auto-refresh:**
```html
<div id="task-list" 
     hx-get="/tasks/list" 
     hx-trigger="every 5s"
     hx-swap="innerHTML">
    {% include "components/task_list.html" %}
</div>
```

**Delete with Confirmation:**
```html
<button hx-delete="/servers/{{ server.id }}"
        hx-target="#server-{{ server.id }}"
        hx-swap="outerHTML"
        hx-confirm="Delete {{ server.name }}?">
    Delete
</button>
```

### Alpine.js for Client-Side State

```html
<body x-data="{ sidebarOpen: false, theme: 'dark' }">
    <!-- Mobile menu toggle -->
    <button @click="sidebarOpen = !sidebarOpen">☰</button>
    
    <!-- Theme toggle -->
    <button @click="theme = theme === 'light' ? 'dark' : 'light'">
        <span x-show="theme === 'light'">🌙</span>
        <span x-show="theme === 'dark'">☀️</span>
    </button>
    
    <!-- Sidebar with conditional class -->
    <aside :class="{ 'open': sidebarOpen }">
        <!-- Navigation -->
    </aside>
</body>
```

## 🚀 CI/CD Workflows

### Two-Workflow Strategy

**Develop Branch** (`.github/workflows/docker-publish-develop.yml`):
- **Trigger**: Push to `develop`
- **Platform**: AMD64 only
- **Build Time**: ~5-8 minutes
- **Image**: `ghcr.io/jsprague84/svrctlrs:develop`
- **Purpose**: Fast iteration for testing

**Main Branch** (`.github/workflows/docker-publish-main.yml`):
- **Trigger**: Push to `main` or version tags
- **Platforms**: AMD64 + ARM64
- **Build Time**: ~15-20 minutes
- **Images**: `latest`, `main`, `v*.*.*`
- **Purpose**: Production releases

### Development Flow

```bash
# 1. Make changes
git add .
git commit -m "feat: new feature"
git push origin develop

# 2. GitHub Actions builds AMD64 image (~5-8 min)

# 3. Pull on docker-vm
docker-compose pull
docker-compose up -d

# 4. Test and iterate

# 5. When stable, merge to main
git checkout main
git merge develop
git push origin main  # Multi-arch build (~15-20 min)
```

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

### Documentation

```rust
/// Brief one-line description.
///
/// Longer description explaining purpose and behavior.
///
/// # Arguments
///
/// * `param1` - Description
/// * `param2` - Description
///
/// # Errors
///
/// Returns `Error::SomeVariant` if X happens.
///
/// # Examples
///
/// ```no_run
/// let result = my_function(arg1, arg2).await?;
/// ```
#[instrument(skip(sensitive_param))]
pub async fn my_function(param1: &str, sensitive_param: &str) -> Result<()> {
    // Implementation
}
```

## 🧪 Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_function() {
        let result = my_function("test").await;
        assert!(result.is_ok());
    }
}
```

### Integration Tests

```bash
# Run all tests
cargo test --workspace

# Run specific plugin tests
cargo test --package svrctlrs-plugin-docker

# Run with logging
RUST_LOG=debug cargo test --workspace -- --nocapture
```

## 🔍 Feature Parity with Weatherust

### Environment Variable Mapping

**Weatherust → SvrCtlRS:**
- `WEATHERUST_GOTIFY_KEY` → `WEATHER_GOTIFY_KEY`
- `UPDATEMON_GOTIFY_KEY` → `UPDATES_GOTIFY_KEY`
- `HEALTHMON_GOTIFY_KEY` → `HEALTH_GOTIFY_KEY`
- `DOCKERMON_GOTIFY_KEY` → `DOCKER_GOTIFY_KEY`

### Porting Checklist

When porting a feature from weatherust:
1. ✅ Read the weatherust implementation
2. ✅ Understand the notification pattern
3. ✅ Convert to plugin architecture
4. ✅ Maintain environment variable compatibility
5. ✅ Add UI components
6. ✅ Test on docker-vm

## 🚨 Common Pitfalls

### Things to Avoid

1. ❌ **Don't duplicate code** - Check `core/` first
2. ❌ **Don't hard-code values** - Use environment variables
3. ❌ **Don't skip error handling** - Use `Result` types
4. ❌ **Don't use `unwrap()`** - Use proper error handling
5. ❌ **Don't skip tracing** - Add `#[instrument]` to key functions
6. ❌ **Don't forget mobile** - Test responsive design

### Things to Remember

1. ✅ **Use MCP tools** - For up-to-date library patterns
2. ✅ **Read weatherust** - For feature reference
3. ✅ **Update this file** - When making significant progress
4. ✅ **Commit frequently** - Small, focused commits
5. ✅ **Test on docker-vm** - Before considering complete
6. ✅ **Check mobile view** - Responsive design is required

## 📚 Key Files Reference

### Core Files
- `core/src/plugin.rs` - Plugin trait and registry
- `core/src/notifications.rs` - Notification manager
- `core/src/remote.rs` - SSH remote execution
- `core/src/error.rs` - Error types

### Server Files
- `server/src/main.rs` - Server entry point
- `server/src/state.rs` - Application state
- `server/src/ui_routes.rs` - HTMX UI routes
- `server/src/templates.rs` - Askama template structs
- `server/templates/base.html` - Base layout
- `server/static/css/styles.css` - Nord theme

### Configuration
- `config/example.toml` - Example configuration
- `docker-compose.yml` - Docker Compose setup
- `Dockerfile` - Multi-stage Docker build
- `.github/workflows/` - CI/CD workflows

## 💡 Quick Tips for AI Assistants

### When Starting a Session

1. **Read this file first** - Get current context
2. **Check README.md** - Project overview
3. **Review recent commits** - See latest changes
4. **Use MCP tools** - Research libraries as needed

### When Writing Code

1. **Check weatherust** - For feature reference
2. **Use existing patterns** - From `core/`
3. **Add instrumentation** - `#[instrument]` on functions
4. **Handle errors properly** - Structured Error types
5. **Test compilation** - `cargo check --workspace`
6. **Test UI** - Check HTMX interactions work

### When Stuck

1. **Read weatherust implementation** - How was it done before?
2. **Use MCP tools** - Look up library examples
3. **Check documentation** - README, this file
4. **Review similar code** - Other plugins, core modules

## 🔗 External References

### Documentation
- Axum: https://docs.rs/axum
- HTMX: https://htmx.org/docs/
- Askama: https://docs.rs/askama
- Alpine.js: https://alpinejs.dev/
- Tokio: https://docs.rs/tokio
- Bollard: https://docs.rs/bollard
- sqlx: https://docs.rs/sqlx

### Weatherust Reference
- Location: `/home/jsprague/Development/weatherust`
- Key files:
  - `common/src/lib.rs` - Shared notification logic
  - `updatectl/src/` - Update execution & cleanup
  - `healthmon/src/` - Docker health monitoring
  - `updatemon/src/` - Update monitoring

## 📌 Project Information

### Project Owner
- Name: Josh Sprague (jsprague84)
- GitHub: https://github.com/jsprague84/svrctlrs
- Reference project: weatherust

### Deployment Environment
- Primary: Docker containers
- Test server: docker-vm
- OS: Linux (Fedora/Ubuntu/Debian support)
- Container runtime: Docker

### Current Version
- **Version**: v2.1.0
- **Status**: Production Ready
- **Last Major Change**: Migrated from Dioxus to HTMX + Askama
- **Next Steps**: Feature additions, performance metrics, historical data

---

**Last Updated**: 2024-11-25  
**Status**: ✅ Production Ready  
**Current Focus**: Feature expansion and refinement
