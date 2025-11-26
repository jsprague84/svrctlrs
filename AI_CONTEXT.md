# AI Development Context

This document provides context for AI assistants to continue development of SvrCtlRS.

## Project Overview

**SvrCtlRS** (Server Control Rust) is a modern infrastructure monitoring and automation platform built with Rust, featuring a plugin-based architecture and HTMX web UI.

### Core Technologies
- **Backend**: Axum 0.8 (async HTTP server)
- **Frontend**: HTMX 2.0.3 + Alpine.js 3.14.1 + Askama 0.12 (SSR templates)
- **Database**: SQLite with sqlx (migrations in `database/migrations/`)
- **Scheduler**: Built-in cron-like scheduler (`scheduler/` crate)
- **Notifications**: Gotify + ntfy.sh with action buttons
- **Remote Execution**: SSH via `async-ssh2-tokio`

### Architecture

```
svrctlrs/
├── core/              # Shared types, plugin system, notifications
├── server/            # Axum backend + HTMX UI
│   ├── src/
│   │   ├── main.rs         # Entry point
│   │   ├── state.rs        # AppState (DB, plugins, scheduler)
│   │   ├── ui_routes.rs    # HTMX UI handlers
│   │   ├── executor.rs     # Task execution engine
│   │   ├── ssh.rs          # SSH utilities
│   │   └── routes/         # REST API
│   ├── templates/          # Askama HTML templates
│   └── static/             # CSS, JS
├── scheduler/         # Cron-like task scheduler
├── database/          # SQLite models, queries, migrations
└── plugins/           # Monitoring plugins
    ├── docker/        # Docker monitoring (Bollard API)
    ├── updates/       # OS package updates
    ├── health/        # System health metrics
    ├── weather/       # OpenWeatherMap (optional)
    └── speedtest/     # Ookla speedtest (optional)
```

## Current Status (v1.0.0)

### ✅ Fully Implemented

1. **Database-Driven Configuration**
   - All settings stored in SQLite
   - No environment variables for app config
   - UI-first configuration approach

2. **Plugin System**
   - 5 plugins: Docker, Updates, Health, Weather, Speedtest
   - Database-backed enable/disable
   - Per-plugin configuration in DB
   - Conditional compilation (`#[cfg(feature = "plugin-*")]`)

3. **Task Execution**
   - Local plugin execution (no `server_id`)
   - Remote SSH execution (with `server_id`)
   - Cron-based scheduling
   - Task history tracking
   - Manual "Run Now" from UI

4. **Notification System**
   - Gotify backend support
   - ntfy.sh backend support
   - Database-backed configuration
   - Per-service notification routing

5. **Web UI (HTMX + Askama)**
   - Server management (`/servers`)
   - Plugin configuration (`/plugins`)
   - Task management (`/tasks`)
   - Notification settings (`/settings/notifications`)
   - Dark/light theme toggle
   - Mobile responsive

6. **Docker Deployment**
   - Multi-arch images (AMD64, ARM64)
   - GitHub Actions CI/CD
   - Docker Compose ready
   - Healthcheck support

### 🎯 Design Decisions

1. **Database as Source of Truth**
   - Eliminated most environment variables
   - All config managed via UI
   - Easier to backup/restore

2. **Hybrid Docker Monitoring**
   - Local: Bollard API via Docker socket
   - Remote: SSH + `docker` CLI commands
   - Manual GID setup for local (documented)

3. **Simple, Not Complex**
   - No auto-detection magic
   - Clear, documented manual steps
   - Prefer explicit over implicit

4. **Plugin Architecture**
   - Core plugins: docker, updates, health (always compiled)
   - Optional plugins: weather, speedtest (feature flags)
   - Easy to add new plugins

## Key Files

### Configuration
- `env.example` - Infrastructure settings only
- `docker-compose.yml` - Docker deployment
- `Dockerfile` - Multi-stage build with all plugins

### Database
- `database/migrations/*.sql` - Schema evolution
- `database/src/models/*.rs` - Rust structs for tables
- `database/src/queries/*.rs` - CRUD operations

### Server
- `server/src/main.rs` - App initialization
- `server/src/state.rs` - Global state, plugin registry
- `server/src/executor.rs` - Task execution logic
- `server/src/ui_routes.rs` - HTMX UI handlers
- `server/src/ssh.rs` - SSH connection utilities

### Plugins
- `plugins/*/src/lib.rs` - Plugin implementations
- Each plugin implements `Plugin` trait from `core/`

## Development Workflow

### Adding a New Plugin

1. Create new crate in `plugins/my-plugin/`
2. Implement `Plugin` trait:
   ```rust
   #[async_trait]
   impl Plugin for MyPlugin {
       fn metadata(&self) -> PluginMetadata { ... }
       fn scheduled_tasks(&self) -> Vec<ScheduledTask> { ... }
       async fn execute(&self, task_id: &str, context: &PluginContext) -> Result<PluginResult> { ... }
   }
   ```
3. Add to `server/Cargo.toml` as optional dependency
4. Register in `server/src/state.rs::init_plugins()`
5. Add database seed in `database/migrations/002_create_plugins.sql`
6. Add feature flag to `Dockerfile` build command

### Adding a New UI Page

1. Create template in `server/templates/pages/my_page.html`
2. Create Askama struct in `server/src/templates.rs`
3. Add route handler in `server/src/ui_routes.rs`
4. Add navigation link in `server/templates/base.html`

### Database Migrations

```bash
# Create new migration
sqlx migrate add my_migration

# Edit database/migrations/NNN_my_migration.sql
# Run migration (automatic on server start)
```

## Common Tasks

### Testing Locally

```bash
# Build and run
cargo build --release --features server,all-plugins
./target/release/server

# Or with Docker
docker compose up -d
docker compose logs -f svrctlrs
```

### Debugging

- Logs: `RUST_LOG=debug` in `.env`
- Database: `sqlite3 data/svrctlrs.db`
- SSH issues: Test manually first (`ssh user@host docker ps`)

### Publishing Release

```bash
# Update version in all Cargo.toml files
# Commit and tag
git tag -a v1.0.0 -m "Release v1.0.0"
git push origin v1.0.0

# GitHub Actions builds and publishes Docker images
```

## Known Issues / Limitations

1. **Docker Socket Permissions**
   - Requires manual GID setup for local monitoring
   - Documented in `DOCKER_MONITORING.md`
   - Alternative: Use SSH for all servers

2. **Single Notification Backend per Type**
   - Only one Gotify backend supported
   - Only one ntfy backend supported
   - Future: Support multiple backends

3. **No Authentication**
   - UI is open to anyone with access
   - Future: Add login system

4. **No HTTPS**
   - Plain HTTP only
   - Use reverse proxy (nginx, Traefik) for HTTPS

## Future Development Ideas

### High Priority

1. **Authentication System**
   - User login/logout
   - Session management (already have tower-sessions)
   - Password hashing (bcrypt/argon2)
   - Protected routes

2. **Multi-User Support**
   - User roles (admin, viewer)
   - Per-user notification preferences
   - Audit log

3. **Dashboard/Overview Page**
   - Summary of all servers
   - Recent task executions
   - System health at a glance

### Medium Priority

4. **Enhanced Docker Plugin**
   - Container logs viewer
   - Container exec (terminal)
   - Image update automation
   - Compose stack management

5. **Webhook Support**
   - Incoming webhooks for external triggers
   - Outgoing webhooks for events
   - Already have basic structure in `routes/webhooks.rs`

6. **Task Dependencies**
   - Run task B after task A succeeds
   - Conditional execution
   - Task chains

### Low Priority

7. **Kubernetes Plugin**
   - Pod monitoring
   - Deployment status
   - Resource usage

8. **Custom Plugins via WASM**
   - User-provided plugins
   - Sandboxed execution
   - Plugin marketplace

9. **Metrics Export**
   - Prometheus exporter
   - InfluxDB integration
   - Grafana dashboards

## Code Style Guidelines

1. **Use `tracing` for logging**
   - `info!`, `warn!`, `error!`, `debug!`
   - Add `#[instrument]` for function tracing

2. **Error Handling**
   - Use `anyhow::Result` for most functions
   - Use `thiserror` for custom error types
   - Context with `.context("message")`

3. **Async/Await**
   - Use `tokio` runtime
   - `#[async_trait]` for trait methods
   - Avoid blocking operations

4. **Database**
   - Use `sqlx` compile-time checked queries
   - Migrations for schema changes
   - Models in `database/src/models/`
   - Queries in `database/src/queries/`

5. **UI Templates**
   - Askama for SSR
   - HTMX for dynamic updates
   - Alpine.js for client-side state
   - Keep JavaScript minimal

## Testing

Currently minimal testing. Future improvements:

1. **Unit Tests**
   - Plugin logic
   - Database queries
   - SSH utilities

2. **Integration Tests**
   - API endpoints
   - Task execution
   - Plugin interactions

3. **E2E Tests**
   - UI workflows
   - Browser automation

## Documentation

- `README.md` - Project overview, quick start
- `QUICKSTART.md` - Detailed setup guide
- `DEVELOPMENT_PLAN.md` - Roadmap and milestones
- `DATABASE_SCHEMA.md` - Database structure
- `DOCKER_MONITORING.md` - Docker plugin setup
- `CLAUDE.md` - AI assistant instructions
- `AI_CONTEXT.md` - This file

## Deployment

### Production Checklist

- [ ] Set `TAG=latest` in `.env`
- [ ] Configure notification backends in UI
- [ ] Add remote servers in UI
- [ ] Enable desired plugins in UI
- [ ] Configure plugin settings in UI
- [ ] Set up reverse proxy for HTTPS
- [ ] Configure firewall rules
- [ ] Set up backup for `data/` volume
- [ ] Monitor logs for errors

### Docker Compose

```yaml
services:
  svrctlrs:
    image: ghcr.io/jsprague84/svrctlrs:latest
    user: "1000:987"  # Adjust GID for Docker socket
    ports:
      - "8080:8080"
    volumes:
      - svrctlrs-data:/app/data
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ~/.ssh:/home/svrctlrs/.ssh:ro
```

## Feature Planning

For comprehensive feature documentation and future roadmap:

- **[FUTURE_FEATURES.md](./FUTURE_FEATURES.md)**: Main catalog of 78 proposed features
- **[docs/features/](./docs/features/)**: Detailed specifications by category (12 categories)
- **[docs/features/TEMPLATE.md](./docs/features/TEMPLATE.md)**: Template for new feature proposals
- **[docs/architecture/plugin-development-guide.md](./docs/architecture/plugin-development-guide.md)**: Plugin development guide

**Feature Categories:**
1. Notifications & Alerting (8 features)
2. Monitoring & Metrics (9 features)
3. Docker Advanced (8 features)
4. System Administration (7 features)
5. Security & Compliance (6 features)
6. Automation & Orchestration (6 features)
7. Collaboration & Team (6 features)
8. Reporting & Analytics (6 features)
9. Integration & Extensibility (7 features)
10. Mobile & Accessibility (5 features)
11. Cost Optimization (5 features)
12. Disaster Recovery (5 features)

## Support

- GitHub Issues: https://github.com/jsprague84/svrctlrs/issues
- Discussions: https://github.com/jsprague84/svrctlrs/discussions

## License

See LICENSE file in repository root.

---

**Last Updated**: 2025-11-25 (v1.0.0 release)
**Maintainer**: jsprague84
**AI Assistant**: Claude (Anthropic)

