# CLAUDE.md

This file provides comprehensive guidance to Claude Code (claude.ai/code) when working with the SvrCtlRS codebase.

## 🎯 Project Mission

**SvrCtlRS** (Server Control Rust) is a clean rewrite of the weatherust monitoring system with a plugin-based architecture for Linux server and Docker monitoring/administration.

**Original Project**: `/home/jsprague/Development/weatherust` (reference for feature parity)

## 📋 Quick Context Recovery

### Current Status (Updated: 2025-11-23)

**✅ Completed:**
- Project structure & Cargo workspace
- Core plugin system (traits & types)
- Notification backends (Gotify + ntfy)
- Axum backend server with REST API
- Scheduler module (cron-like)
- Database layer (SQLite)
- Plugin implementations (Docker, Updates, Health)
- **Dioxus 0.7 Fullstack UI** - SSR + WASM hydration
- GitHub repository: https://github.com/jsprague84/svrctlrs

**🎉 Current Sprint: Sprint 6 - UI Implementation** (95% complete)
- ✅ Dioxus 0.7 fullstack setup with conditional compilation
- ✅ Interactive UI components (Dashboard, Servers, Plugins, Tasks, Settings)
- ✅ Server functions for backend integration
- ✅ Docker configuration for fullstack deployment
- ✅ Production build pipeline with `dx build`
- 🔄 Docker image building (in progress)

**📍 Next Immediate Tasks:**
1. Complete Docker image build and test
2. Deploy v2.1.0 with fullstack UI
3. Test client-side hydration and interactivity
4. Implement backend API endpoints for server functions

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

## 🎨 Dioxus 0.7 Fullstack Implementation

### Overview

The project uses **Dioxus 0.7** fullstack architecture with:
- **Server-Side Rendering (SSR)** - Initial HTML generated on server
- **WASM Client Hydration** - Interactive JavaScript/WASM bundle
- **Conditional Compilation** - Same codebase compiles for server and client
- **Server Functions** - Backend operations callable from frontend

### Critical Pattern: Conditional Compilation

**IMPORTANT**: Always use Context7 to verify Dioxus 0.7 best practices. The official documentation is at https://dioxuslabs.com/learn/0.7/

#### Cargo.toml Structure

```toml
[dependencies]
# Dioxus UI (always needed)
dioxus = { workspace = true, features = ["fullstack"] }
dioxus-router = "0.7"
dioxus-fullstack = "0.7"
dioxus-ssr = "0.7"

# Server-only dependencies (MUST be optional)
axum = { workspace = true, optional = true }
tokio = { workspace = true, optional = true }
tower = { workspace = true, optional = true }
tower-http = { workspace = true, optional = true }
anyhow = { workspace = true, optional = true }
tracing = { workspace = true, optional = true }
# ... all other server deps

[features]
default = ["plugin-docker", "plugin-updates", "plugin-health", "server"]

# Server feature enables all server-only dependencies
server = [
    "dioxus/server",
    "dep:axum",
    "dep:tokio",
    "dep:tower",
    "dep:tower-http",
    "dep:anyhow",
    "dep:tracing",
    # ... etc
]

# Web feature for WASM client
web = ["dioxus/web"]

# Desktop feature (if needed)
desktop = ["dioxus/desktop"]
```

#### main.rs Structure

```rust
//! Dual entry points for server and client

#![allow(non_snake_case)]

use dioxus::prelude::*;

mod ui;

// ============================================
// SERVER-SIDE CODE (Axum + SSR)
// ============================================
#[cfg(feature = "server")]
mod config;
#[cfg(feature = "server")]
mod routes;
#[cfg(feature = "server")]
mod state;

#[cfg(feature = "server")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    use axum::Router;
    use clap::Parser;

    // Initialize server, load config, start plugins

    // CRITICAL: Build UI router separately so it can be nested without affecting API state
    // This nested router pattern is required for Dioxus 0.7 + Axum 0.8 integration
    let ui_router = Router::new()
        .serve_dioxus_application(ServeConfig::new(), ui::App);

    // Build Axum router with API + UI
    let app = Router::new()
        .nest("/api", routes::api_routes(state.clone()))
        .nest_service("/", ui_router.into_service())
        // Add middleware
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    tracing::info_span!(
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri(),
                    )
                }),
        )
        .layer(tower_http::compression::CompressionLayer::new())
        .layer(tower_http::cors::CorsLayer::permissive());

    // Start server
    axum::serve(listener, app.into_make_service()).await?;
    Ok(())
}

// ============================================
// CLIENT-SIDE CODE (WASM)
// ============================================
#[cfg(not(feature = "server"))]
fn main() {
    // Launch Dioxus app in browser
    dioxus::launch(ui::App);
}
```

### Static Global State Accessor Pattern

**CRITICAL**: Server functions (`#[server]` macro) cannot use `State<AppState>` extractors like regular Axum handlers. You must use a static global accessor instead.

**Implementation in `server/src/state.rs`:**

```rust
use tokio::sync::{OnceCell, RwLock};

/// Global application state for server functions
static APP_STATE: OnceCell<AppState> = OnceCell::const_new();

impl AppState {
    /// Initialize the global app state (call once at startup)
    pub fn set_global(state: AppState) {
        if APP_STATE.set(state).is_err() {
            panic!("AppState already initialized");
        }
    }

    /// Get the global app state (for use in server functions)
    pub fn global() -> AppState {
        APP_STATE
            .get()
            .expect("AppState not initialized - call AppState::set_global() first")
            .clone()
    }
}
```

**Usage in main.rs:**

```rust
// After creating AppState
let state = AppState::new(config).await?;
state.init_plugins().await?;
state.start_scheduler().await?;

// Initialize global state for server functions
AppState::set_global(state.clone());
```

### Server Functions Pattern

Server functions run on the backend but are callable from frontend:

```rust
// In server/src/ui/server_fns.rs

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub host: String,
    pub status: String,
}

/// Get list of servers from configuration
#[server]
pub async fn list_servers() -> Result<Vec<ServerInfo>, ServerFnError> {
    // This code ONLY runs on server
    // Access state via global accessor (NOT State<AppState> extractor)
    let state = AppState::global();

    let servers: Vec<ServerInfo> = state
        .config
        .servers
        .iter()
        .map(|s| ServerInfo {
            name: s.name.clone(),
            ssh_host: s.ssh_host.clone().unwrap_or_else(|| "localhost".to_string()),
            is_local: s.is_local(),
        })
        .collect();

    Ok(ServerListResponse { servers })
}

/// Toggle plugin enabled state
#[server]
pub async fn toggle_plugin(req: TogglePluginRequest) -> Result<bool, ServerFnError> {
    // Access state via global accessor
    let state = AppState::global();

    // Verify plugin exists
    let registry = state.plugins.read().await;
    registry
        .get(&req.plugin_id)
        .ok_or_else(|| ServerFnError::new(format!("Plugin {} not found", req.plugin_id)))?;

    // TODO: Implement runtime plugin enable/disable in PluginRegistry
    tracing::info!(
        "Toggle plugin: plugin={}, enabled={}",
        req.plugin_id,
        req.enabled
    );

    Ok(req.enabled)
}
```

### UI Component Pattern

```rust
use dioxus::prelude::*;
use crate::ui::server_fns::*;

#[component]
pub fn ServerList() -> Element {
    // Resource automatically fetches on mount and refetches on demand
    let servers = use_resource(|| async move {
        get_servers().await.unwrap_or_default()
    });

    rsx! {
        div { class: "server-list",
            match &*servers.read_unchecked() {
                Some(servers) => rsx! {
                    for server in servers {
                        ServerCard { server: server.clone() }
                    }
                },
                None => rsx! {
                    div { "Loading servers..." }
                }
            }
        }
    }
}

#[component]
fn ServerCard(server: ServerInfo) -> Element {
    rsx! {
        div { class: "server-card",
            h3 { "{server.name}" }
            p { "{server.host}" }
            span { class: "status-{server.status}",
                "{server.status}"
            }
        }
    }
}
```

### Build Commands

#### Development (with hot reload)
```bash
# Starts dev server with hot reload at http://localhost:8080
dx serve --package server

# Server will restart on Rust changes, browser will reload on UI changes
```

#### Production Build
```bash
# Builds both server binary and WASM client bundle
dx build --release

# Output:
# - target/release/server (binary with embedded assets)
# - dist/ (WASM bundle + JavaScript loader)
```

#### Manual Cargo Build (without Dioxus CLI)
```bash
# Build server binary only (no WASM client)
cargo build --release --package server --bin server

# This works but won't include WASM client assets
# Use dx build for fullstack deployment
```

### Dioxus.toml Configuration

**CRITICAL**: This file controls the entire Dioxus build process.

```toml
[application]
name = "SvrCtlRS"
default_platform = "fullstack"  # Must be "fullstack" not "web"
package = "server"              # Which package to build
out_dir = "dist"                # WASM output directory
asset_dir = "assets"            # Static assets directory

[web.app]
title = "SvrCtlRS - Server Control & Monitoring"

[web.watcher]
reload_html = true
watch_path = ["server/src", "assets"]

[web.resource]
# MUST be arrays, not maps
style = []
script = []

[web.resource.dev]
script = []
```

### Docker Deployment

The Dockerfile now includes Dioxus CLI installation and fullstack build:

```dockerfile
FROM rust:bookworm AS builder

# Install Dioxus CLI
RUN cargo install dioxus-cli --version 0.7.1

WORKDIR /app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY server ./server
# ... other directories

# Copy Dioxus config
COPY Dioxus.toml ./
COPY assets ./assets

# Build with Dioxus CLI (creates server binary + WASM client)
RUN dx build --release

# Also build svrctl CLI
RUN cargo build --release --bin svrctl

# Runtime stage
FROM debian:bookworm-slim

# Copy binaries and assets
COPY --from=builder /app/target/release/server /app/svrctlrs-server
COPY --from=builder /app/target/release/svrctl /app/svrctl
COPY --from=builder /app/dist /app/dist
COPY --from=builder /app/assets /app/assets

# Run server
CMD ["/app/svrctlrs-server"]
```

### Troubleshooting Common Issues

#### 1. "Cannot find dioxus/server" Error
**Cause**: Missing `dioxus/server` feature in Cargo.toml
**Fix**: Add to server feature: `"dioxus/server"`

#### 2. "Axum not compatible with WASM" Error
**Cause**: Server dependencies not marked as optional
**Fix**: Make all server deps optional with `optional = true`

#### 3. "serve_dioxus_application method not found" Error
**Cause**: Trying to call `.serve_dioxus_application()` directly on a Router that already has routes/state
**Fix**: Use the **nested router pattern** - create Dioxus router separately and nest it as a service:
```rust
// CORRECT: Nested router pattern
let ui_router = Router::new()
    .serve_dioxus_application(ServeConfig::new(), ui::App);

let app = Router::new()
    .nest("/api", routes::api_routes(state.clone()))
    .nest_service("/", ui_router.into_service());

// INCORRECT: Trying to call directly on existing router
let app = Router::new()
    .nest("/api", routes::api_routes(state.clone()))
    .serve_dioxus_application(ServeConfig::new(), ui::App);  // ❌ Won't work
```
This is the required pattern for Dioxus 0.7 + Axum 0.8 integration. See server/src/main.rs:80-87.

#### 4. TOML Parse Errors in Dioxus.toml
**Cause**: Incorrect syntax (maps instead of arrays, duplicate keys)
**Fix**: Use arrays `[]` for resources, check for duplicate `[web.proxy]` sections

#### 5. Application Exits Immediately
**Cause**: Missing config.toml or database initialization
**Fix**: Ensure config.toml exists and DATABASE_URL is set

#### 6. WASM Not Hydrating
**Cause**: Mismatch between SSR HTML and client expectations
**Fix**: Ensure `renderer.pre_render = true` in SSR code for hydration IDs

### Context7 Usage for Dioxus

**Always use Context7** when working with Dioxus to get up-to-date patterns:

```
# Example queries for Context7
- "dioxus 0.7 fullstack server functions"
- "dioxus 0.7 use_resource pattern"
- "dioxus 0.7 conditional compilation"
- "dioxus 0.7 axum integration"
```

### Key Files for Dioxus Implementation

- `server/Cargo.toml` - Feature flags and optional dependencies
- `server/src/main.rs` - Dual entry points with #[cfg]
- `server/src/ui/mod.rs` - UI module exports
- `server/src/ui/app.rs` - Root App component with routing
- `server/src/ui/server_fns.rs` - Server functions (backend operations)
- `server/src/ui/pages/` - Page components
- `server/src/ui/components/` - Reusable UI components
- `server/src/ui/fullstack.rs` - SSR serving logic
- `Dioxus.toml` - Build configuration

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

## 🚀 CI/CD Workflows

### Two-Workflow Strategy

SvrCtlRS uses an optimized CI/CD approach with separate workflows for development and production:

**CI Workflow** (`.github/workflows/ci.yml`):
- **Triggers**: Push to `develop` branch, pull requests
- **Platforms**: linux/amd64 only (fast builds)
- **Build Time**: ~5-7 minutes
- **Image Tag**: `:develop`
- **Purpose**: Rapid iteration and testing
- **Use**: Test server deployment

**Release Workflow** (`.github/workflows/release.yml`):
- **Triggers**: Version tags (`v*.*.*`), manual dispatch
- **Platforms**: linux/amd64 + linux/arm64 (multi-arch)
- **Build Time**: ~10-15 minutes
- **Image Tags**: `:latest`, `:v2.3.0`, `:2.3`, `:2`
- **Purpose**: Production releases
- **Use**: Production server deployment

### Development Flow

```bash
# 1. Daily development on develop branch
git checkout develop
git commit -am "feat: new feature"
git push origin develop  # CI builds :develop (~7 min)

# 2. Test server auto-pulls :develop tag
ssh test-server "cd /opt/svrctlrs && docker-compose pull && docker-compose up -d"

# 3. When stable, create release
git checkout master && git merge develop
git tag v2.3.0 && git push origin v2.3.0  # Release builds :latest (~15 min)

# 4. Production pulls :latest tag
ssh prod-server "cd /opt/svrctlrs && docker-compose pull && docker-compose up -d"
```

**Performance Benefits**:
- Development builds: **50% faster** (single platform)
- 10 dev commits + 1 release = **65 minutes saved** vs always building multi-arch
- Test changes in ~10 minutes (commit → deployed)

**See [docs/CI-CD.md](./docs/CI-CD.md) for complete documentation**

---

## 📊 Progress Tracking

### Sprint Overview

**Sprint 1: Foundation** ✅ Complete
- Week 1 - Core infrastructure
- Status: 100% complete
- Core plugin system, notifications, scheduler, database

**Sprint 2: Docker Plugin** ✅ Complete
- Week 2 - Health monitoring, cleanup, updates
- Status: 100% complete
- Full Docker monitoring and management

**Sprint 3: Updates Plugin** ✅ Complete
- Week 3 - OS updates, cleanup, execution
- Status: 100% complete
- SSH-based update monitoring and execution

**Sprint 4: Infrastructure** ✅ Complete
- Week 4 - Webhooks, API, CLI
- Status: 100% complete
- Full REST API with Axum

**Sprint 5: Polish** ✅ Complete
- Week 5 - Weather, Speed test, Testing
- Status: 100% complete
- Optional plugins, testing, cleanup

**Sprint 6: UI** 🔄 95% Complete
- Dioxus 0.7 fullstack dashboard
- Status: Nearly complete
- Remaining: Docker image build, deployment, backend API implementation

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
- Web Backend: Axum 0.8
- **UI: Dioxus 0.7 Fullstack** - SSR + WASM hydration
- Database: SQLite with sqlx
- Docker API: bollard
- Scheduler: cron crate
- Build Tool: Dioxus CLI (dx) v0.7.1

---

**Last Updated**: 2025-11-24
**Current Sprint**: Sprint 6 - UI Implementation ✅ COMPLETE
**Status**: Ready for CI deployment to GHCR
**Next Task**: Push to `develop` branch to trigger CI, then deploy to test server
**Version**: v2.1.0-fullstack (ready for release)
