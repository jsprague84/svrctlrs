# CLAUDE.md

This file provides comprehensive guidance to Claude Code (claude.ai/code) when working with the SvrCtlRS codebase.

## 🎯 Project Mission

**SvrCtlRS** (Server Control Rust) is a clean rewrite of the weatherust monitoring system with a plugin-based architecture for Linux server and Docker monitoring/administration.

**Original Project**: `/home/jsprague/Development/weatherust` (reference for feature parity)

## 📋 Quick Context Recovery

### Current Status (Updated: 2025-01-23)

**✅ Completed:**
- Project structure & Cargo workspace
- Core plugin system (traits & types)
- Notification backends (Gotify + ntfy)
- Basic server with Axum
- Scheduler module (cron-like)
- Database layer (SQLite)
- Plugin stubs (Docker, Updates, Health)
- GitHub repository: https://github.com/jsprague84/svrctlrs

**🔄 Current Sprint: Sprint 1 - Foundation** (60% complete)
- ✅ Project structure
- ✅ Notification backends
- 🔴 Enhanced remote executor
- 🔴 Database migrations
- 🔴 Webhook framework

**📍 Next Immediate Tasks:**
1. Enhance `core/src/remote.rs` - Add connection pooling, timeouts
2. Add database migrations in `database/src/migrations/`
3. Implement basic webhook framework in `server/src/routes/webhook.rs`

### Key Documents to Read First

1. **[IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)** - Complete 6-sprint development plan
2. **[README.md](./README.md)** - Project overview and quick start
3. **Weatherust Reference**: `/home/jsprague/Development/weatherust/CLAUDE.md`

## 🏗️ Architecture Overview

### Plugin-Based System

```
svrctlrs/
├── core/              # Shared types, plugin system, notifications
├── server/            # Axum HTTP server (Dioxus UI planned)
├── scheduler/         # Built-in cron-like scheduler
├── database/          # SQLite abstraction
└── plugins/           # Monitoring plugins
    ├── docker/        # Docker health, cleanup, updates
    ├── updates/       # OS/Docker update monitoring & execution
    ├── health/        # System metrics (CPU, memory, disk)
    ├── weather/       # Weather monitoring (optional)
    └── speedtest/     # Speed test monitoring (optional)
```

### Key Design Principles

1. **Plugin Architecture** - All features are plugins implementing the `Plugin` trait
2. **Service-Specific Notifications** - Each plugin can have its own Gotify key / ntfy topic
3. **Remote Execution** - SSH-based operations via `RemoteExecutor`
4. **Dual Notifications** - Both Gotify and ntfy.sh support
5. **Webhook Triggers** - HTTP endpoints for remote-triggered actions
6. **Built-in Scheduler** - No external dependencies like Ofelia

## 🔧 Development Workflow

### Always Use Context7 MCP

When working on this project:
- Use Context7 to look up current Rust/Axum/Dioxus best practices
- Check for latest crate versions and patterns
- Research library-specific examples

Example:
```
Use Context7 to research:
- bollard (Docker API) for container monitoring
- Axum routing patterns for webhooks
- SQLite migrations with sqlx
```

### Before Starting Work

1. **Read current status** from this file (CLAUDE.md)
2. **Check IMPLEMENTATION_PLAN.md** for sprint details
3. **Review weatherust** for feature reference if porting
4. **Use Context7** for up-to-date library patterns

### Code Organization Rules

1. **Shared Code** - Always add to `core/` if used by multiple plugins
2. **Plugin Code** - Keep plugin-specific in `plugins/<name>/`
3. **No Duplication** - Check weatherust common/ for patterns to reuse
4. **Error Types** - Use structured errors from `core/src/error.rs`
5. **Notifications** - Use `NotificationManager` from core
6. **Remote Exec** - Use `RemoteExecutor` from core

## 📚 Code Patterns & Standards

### Plugin Implementation Pattern

```rust
use async_trait::async_trait;
use svrctlrs_core::{Plugin, PluginContext, PluginMetadata, PluginResult, Result, ScheduledTask};

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
            _ => Ok(PluginResult {
                success: false,
                message: format!("Unknown task: {}", task_id),
                data: None,
                metrics: None,
            }),
        }
    }
}

impl MyPlugin {
    async fn run_task(&self, _context: &PluginContext) -> Result<PluginResult> {
        // Implementation
        Ok(PluginResult {
            success: true,
            message: "Task completed".to_string(),
            data: None,
            metrics: None,
        })
    }
}
```

### Notification Pattern

```rust
use svrctlrs_core::{NotificationManager, NotificationMessage, NotificationAction};

// In plugin execute()
let manager = NotificationManager::new(client.clone(), &["docker"])?;

manager.send_for_service(
    "docker",
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

### Database Pattern

```rust
use svrctlrs_database::Database;

let db = Database::new("sqlite:data/svrctlrs.db").await?;
db.migrate().await?;

// Use db.pool() for queries
sqlx::query("SELECT * FROM servers")
    .fetch_all(db.pool())
    .await?;
```

## 🔍 Feature Parity Reference

### Porting from Weatherust

When implementing a feature that exists in weatherust:

1. **Read the source** - Check `/home/jsprague/Development/weatherust/`
2. **Understand the pattern** - How it works in the monolith
3. **Adapt to plugins** - Convert to plugin architecture
4. **Maintain compatibility** - Same environment variables, same notifications

### Environment Variable Mapping

**Weatherust → SvrCtlRS:**
- `WEATHERUST_GOTIFY_KEY` → `WEATHER_GOTIFY_KEY`
- `UPDATEMON_GOTIFY_KEY` → `UPDATES_GOTIFY_KEY`
- `HEALTHMON_GOTIFY_KEY` → `HEALTH_GOTIFY_KEY`
- `DOCKERMON_GOTIFY_KEY` → `DOCKER_GOTIFY_KEY`
- Same pattern for ntfy topics

**Server Configuration:**
- `UPDATE_SERVERS` → `SERVERS` (format: `name:user@host,name2:user@host2`)
- `UPDATE_SSH_KEY` → `SSH_KEY_PATH`
- `UPDATE_LOCAL_NAME` → Keep same

## 📊 Progress Tracking

### Sprint Overview

**Sprint 1: Foundation** (Current)
- Week 1 - Core infrastructure
- Status: 60% complete
- Blockers: None
- Next: Remote executor enhancements

**Sprint 2: Docker Plugin**
- Week 2 - Health monitoring, cleanup, updates
- Status: Not started
- Dependencies: Sprint 1 complete

**Sprint 3: Updates Plugin**
- Week 3 - OS updates, cleanup, execution
- Status: Not started

**Sprint 4: Infrastructure**
- Week 4 - Webhooks, API, CLI
- Status: Not started

**Sprint 5: Polish**
- Week 5 - Weather, Speed test, Testing
- Status: Not started

**Sprint 6: UI**
- Future - Dioxus dashboard
- Status: Not started

### Recent Commits

Check git log for recent work:
```bash
git log --oneline -10
```

Latest commit should indicate current progress.

## 🧪 Testing Strategy

### Manual Testing

Test server: `docker-vm` (SSH: `user@docker-vm`)

**Testing Workflow:**
1. Build: `cargo build --release`
2. Test locally first
3. Deploy to docker-vm
4. Run alongside weatherust for comparison
5. Validate notifications match

### Integration Testing

```bash
# Run all tests
cargo test --workspace

# Run specific plugin tests
cargo test --package svrctlrs-plugin-docker
```

## 📝 Documentation Standards

### File Headers

Every Rust file should start with:
```rust
//! Brief module description.
//!
//! Longer description of what this module does and why it exists.
```

### Function Documentation

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

## 🚨 Common Pitfalls

### Things to Avoid

1. **Don't duplicate code** - Check core/ and weatherust first
2. **Don't hard-code values** - Use environment variables
3. **Don't skip error handling** - Use Result types
4. **Don't use unwrap()** - Use proper error handling
5. **Don't skip tracing** - Add #[instrument] to key functions

### Things to Remember

1. **Use Context7** - For up-to-date library patterns
2. **Read weatherust** - For feature reference
3. **Update this file** - When making significant progress
4. **Commit frequently** - Small, focused commits
5. **Test on docker-vm** - Before considering complete

## 🔗 External References

### Documentation
- Axum: https://docs.rs/axum
- Tokio: https://docs.rs/tokio
- Bollard: https://docs.rs/bollard
- sqlx: https://docs.rs/sqlx
- Dioxus: https://dioxuslabs.com/

### Weatherust Codebase
- Location: `/home/jsprague/Development/weatherust`
- Key files:
  - `common/src/lib.rs` - Shared notification logic
  - `updatectl/src/` - Update execution & cleanup
  - `healthmon/src/` - Docker health monitoring
  - `updatemon/src/` - Update monitoring

## 🎓 Learning Resources

### New to the Project?

1. Read [README.md](./README.md)
2. Read [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)
3. Review `/home/jsprague/Development/weatherust/docs/`
4. Check recent commits: `git log --oneline -20`
5. Read this file thoroughly

### Understanding the Migration

**Why a rewrite?**
- Better architecture (plugin-based)
- No external scheduler dependency (Ofelia)
- Unified codebase (single binary)
- Modern web UI (Dioxus)
- Better separation of concerns

**What's being kept?**
- All features from weatherust
- Same notification patterns
- Same environment variables
- Same SSH-based remote execution
- Gotify + ntfy.sh support

**What's changing?**
- Plugin architecture (not multiple binaries)
- Built-in scheduler (not Ofelia)
- Web UI (not just CLI)
- Single repository structure
- Modern Rust patterns

## 💡 Quick Tips for Claude Code

### When Starting a Session

1. **Read this file first** - Get current context
2. **Check IMPLEMENTATION_PLAN.md** - Know the roadmap
3. **Review recent commits** - See latest changes
4. **Use Context7** - Research libraries as needed

### When Writing Code

1. **Check weatherust** - For feature reference
2. **Use existing patterns** - From core/
3. **Add instrumentation** - #[instrument] on functions
4. **Handle errors properly** - Structured Error types
5. **Test compilation** - `cargo check --workspace`

### When Stuck

1. **Read weatherust implementation** - How was it done before?
2. **Use Context7** - Look up library examples
3. **Check documentation** - README, IMPLEMENTATION_PLAN
4. **Review similar code** - Other plugins, core modules

## 📌 Critical Information

### Project Owner
- Name: Josh Sprague (jsprague84)
- GitHub: https://github.com/jsprague84/svrctlrs
- Reference project: weatherust

### Deployment Environment
- Primary: Docker containers
- Test server: docker-vm
- OS: Linux (Fedora/Ubuntu/Debian support)
- Container runtime: Docker

### Technology Stack
- Language: Rust (latest stable)
- Async: Tokio
- Web: Axum 0.8
- UI: Dioxus 0.7 (planned)
- Database: SQLite with sqlx
- Docker API: bollard
- Scheduler: cron crate

---

**Last Updated**: 2025-01-23
**Current Sprint**: Sprint 1 - Foundation (60% complete)
**Next Task**: Enhance RemoteExecutor with connection pooling
