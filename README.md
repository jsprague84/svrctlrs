# SvrCtlRS

**Server Control Rust** - A modern, plugin-based infrastructure monitoring and automation platform with HTMX web UI.

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](https://github.com/jsprague84/svrctlrs/releases)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![HTMX](https://img.shields.io/badge/htmx-2.0-green.svg)](https://htmx.org)
[![Docker](https://img.shields.io/badge/docker-multi--arch-blue.svg)](https://github.com/jsprague84/svrctlrs/pkgs/container/svrctlrs)

## Overview

SvrCtlRS is a complete rewrite of the weatherust monitoring system with a focus on:

- **Plugin Architecture**: Modular, extensible design for easy feature additions
- **Built-in Scheduler**: No external dependencies like Ofelia
- **Modern Web UI**: HTMX + Askama for interactive, lightweight frontend
- **Axum Backend**: High-performance REST API
- **State Management**: SQLite for persistent state and historical data
- **Remote Execution**: SSH-based operations across multiple servers
- **Dual Notifications**: Gotify and ntfy.sh support with action buttons

## Architecture

```
svrctlrs/
├── core/              # Shared types, traits, plugin system
├── server/            # Axum backend + HTMX UI
│   ├── src/
│   │   ├── main.rs       # Server entry point
│   │   ├── ui_routes.rs  # HTMX UI routes
│   │   ├── routes/       # REST API routes
│   │   └── templates.rs  # Askama template structs
│   ├── templates/        # Askama HTML templates
│   └── static/           # CSS, JS (HTMX, Alpine.js)
├── scheduler/         # Built-in cron-like task scheduler
├── database/          # SQLite abstraction and migrations
└── plugins/           # Monitoring plugins
    ├── docker/        # Docker health and updates
    ├── updates/       # OS/package updates
    ├── health/        # System health monitoring
    ├── weather/       # Weather monitoring (optional)
    └── speedtest/     # Speed test monitoring (optional)
```

## Technology Stack

- **Backend**: Axum 0.8 (HTTP server + REST API)
- **Frontend**: HTMX 2.0.3 + Alpine.js 3.14.1 + Askama 0.12
- **Database**: SQLite with sqlx
- **Runtime**: Tokio async runtime
- **Notifications**: Gotify + ntfy.sh with action buttons
- **Remote Ops**: SSH with openssh_sftp_client
- **Bundle Size**: ~94KB JavaScript (vs 500KB+ with React/Dioxus)

## Features

### 🚀 Core Capabilities

- ✅ **Modular Plugin System**: Easy to add new monitoring capabilities
- ✅ **Built-in Scheduler**: Schedule tasks with cron expressions
- ✅ **Interactive Web Dashboard**: HTMX for dynamic updates without page reloads
- ✅ **Remote Operations**: SSH-based remote command execution
- ✅ **REST API**: Full HTTP API for programmatic access
- ✅ **CLI Tool**: `svrctl` command-line interface
- ✅ **Notification System**: Rich notifications with action buttons
- ✅ **Status Reports**: Periodic "all OK" summaries even when healthy
- ✅ **Database Persistence**: SQLite for historical data
- ✅ **Mobile Responsive**: Works on desktop and mobile devices

### 📦 Implemented Plugins

- ✅ **Docker Plugin**: Container health, resource monitoring, image updates
- ✅ **Updates Plugin**: OS package monitoring, automated updates, cleanup
- ✅ **Health Plugin**: System metrics (CPU, memory, disk, network)
- ✅ **Weather Plugin**: OpenWeatherMap integration (optional)
- ✅ **Speed Test Plugin**: Ookla speed test monitoring (optional)

### 📊 Status Reports

**Get regular "all systems OK" confirmations even when nothing is wrong!**

SvrCtlRS supports **status report summaries** - periodic health confirmations sent even when your infrastructure is healthy. This gives you peace of mind that monitoring is working, not just silence when things are fine.

**How It Works:**
- **Alerts**: Sent immediately when issues detected (high CPU, updates available, unhealthy containers)
- **Summaries**: Sent on schedule when everything is normal (configurable per plugin)

**Example Summary Notifications:**

```
Docker Health Summary
📊 All containers healthy ✓

Containers: 19 total, 19 running, 0 stopped
Average CPU: 12.3% | Average Memory: 45.6%
All systems operational.
```

```
System Health Summary
📊 System Status ✓

CPU Usage: 15.2%
Memory Usage: 45.8%
Disk Usage: 62.1%
All systems within normal parameters.
```

```
Updates Status: localhost
📊 System Up to Date ✓

Server: localhost
Package Manager: apt
Last Checked: 2025-11-26 14:30:00
```

**Enable Summaries:**
1. Navigate to **Plugins** page → Click **Configure** on any plugin
2. Check **"Send summary reports even when healthy"** checkbox
3. Click **Save Configuration**
4. Receive regular health confirmations according to schedule

**Default Schedules:**
- **Docker Health**: Every 5 minutes
- **System Health**: Every 5 minutes
- **Updates Check**: Every 6 hours
- **Updates Report**: Daily at 9 AM (multi-server summary)
- **Docker Cleanup**: Weekly on Sundays at 2 AM

**See [TEST_SUMMARY_REPORTS.md](./TEST_SUMMARY_REPORTS.md) for testing guide.**

## 🚀 Quick Start (Docker Compose - Recommended)

**Get up and running in 3 minutes!**

```bash
# 1. Create .env file (optional - has sensible defaults)
cp env.example .env

# 2. Start the application
docker compose up -d

# 3. Access the web UI
open http://localhost:8080

# 4. Configure through the UI:
#    - Add remote servers: /servers
#    - Enable plugins: /plugins
#    - Setup notifications: /settings/notifications
#    - View/run tasks: /tasks
```

**Note**: All configuration is managed through the web UI and stored in the database. No config files needed!

**📖 For detailed setup instructions, see [QUICKSTART.md](./QUICKSTART.md)**

### Alternative: Development Build

```bash
# Clone the repository
git clone https://github.com/jsprague84/svrctlrs
cd svrctlrs

# Copy example config
cp config/example.toml config.toml

# Build and run
cargo run --package server --features server

# Server starts at http://localhost:8080
```

### Alternative: Production Binary

```bash
# Build release binary
cargo build --release --package server --features server

# Run production server
./target/release/server --config config.toml
```

## Configuration

Configuration is managed through `config.toml`:

```toml
[server]
addr = "0.0.0.0:8080"
database_url = "sqlite:data/svrctlrs.db"

[notifications]
gotify_url = "http://gotify:8080/message"
gotify_key = "your-gotify-token"
ntfy_url = "https://ntfy.sh"
ntfy_topic = "svrctlrs-alerts"

[remote]
ssh_key_path = "/path/to/ssh/key"

[[servers]]
name = "server1"
host = "user@host1"

[[servers]]
name = "server2"
host = "user@host2"

[plugins]
docker_enabled = true
updates_enabled = true
health_enabled = true
```

## Development Workflow

### Quick Iteration on `develop` Branch

```bash
# 1. Make changes
git add .
git commit -m "feat: add new feature"
git push origin develop

# 2. GitHub Actions builds AMD64 image (~5-8 min)
#    Image: ghcr.io/jsprague84/svrctlrs:develop

# 3. Pull and test on docker-vm
docker-compose pull
docker-compose up -d
```

### Production Release on `main` Branch

```bash
# 1. Merge to main
git checkout main
git merge develop
git push origin main

# 2. GitHub Actions builds multi-arch image (~15-20 min)
#    Image: ghcr.io/jsprague84/svrctlrs:latest
#    Platforms: AMD64 + ARM64
```

See [docs/deployment/docker.md](./docs/deployment/docker.md) for complete workflow documentation.

## Project Structure

- **`core/`**: Core library with traits and types used by all plugins
- **`server/`**: Axum backend + HTMX UI
  - `src/main.rs` - Server entry point
  - `src/ui_routes.rs` - HTMX UI route handlers
  - `src/routes/` - REST API routes
  - `src/templates.rs` - Askama template structs
  - `templates/` - HTML templates (Askama)
  - `static/` - CSS, JavaScript (HTMX, Alpine.js)
- **`scheduler/`**: Task scheduling engine
- **`database/`**: Database layer, migrations, queries
- **`plugins/`**: Individual monitoring plugins

## Adding a New Plugin

1. Create new crate: `cargo new --lib plugins/myplugin`
2. Implement `Plugin` trait from `svrctlrs-core`
3. Add to workspace in `Cargo.toml`
4. Register in `server/src/state.rs`
5. Add UI components in `server/templates/`

## Documentation

### Getting Started
- **[QUICKSTART.md](./QUICKSTART.md)**: 5-minute Docker Compose setup guide
- **[.env.example](./.env.example)**: Environment variable reference
- **[config/example.toml](./config/example.toml)**: Configuration file reference

### Development & Deployment
- **[AI_CONTEXT.md](./AI_CONTEXT.md)**: Comprehensive AI development context
- **[DEVELOPMENT_PLAN.md](./DEVELOPMENT_PLAN.md)**: Current development roadmap
- **[IMMEDIATE_PRIORITIES.md](./IMMEDIATE_PRIORITIES.md)**: Critical issues and next steps
- **[FUTURE_FEATURES.md](./FUTURE_FEATURES.md)**: 📋 **Catalog of 78 proposed features**
- **[docs/features/](./docs/features/)**: Detailed feature specifications by category
- **[docs/architecture/plugin-development-guide.md](./docs/architecture/plugin-development-guide.md)**: Plugin development guide

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
