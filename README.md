# SvrCtlRS

**Server Control Rust** - A modern, plugin-based infrastructure monitoring and automation platform with HTMX web UI.

[![Version](https://img.shields.io/badge/version-2.1.0-blue.svg)](https://github.com/jsprague84/svrctlrs)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![HTMX](https://img.shields.io/badge/htmx-2.0-green.svg)](https://htmx.org)

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
- ✅ **Database Persistence**: SQLite for historical data
- ✅ **Mobile Responsive**: Works on desktop and mobile devices

### 📦 Implemented Plugins

- ✅ **Docker Plugin**: Container health, resource monitoring, image updates
- ✅ **Updates Plugin**: OS package monitoring, automated updates, cleanup
- ✅ **Health Plugin**: System metrics (CPU, memory, disk, network)
- ✅ **Weather Plugin**: OpenWeatherMap integration (optional)
- ✅ **Speed Test Plugin**: Ookla speed test monitoring (optional)

## 🚀 Quick Start (Docker Compose - Recommended)

**Get up and running in 5 minutes!**

```bash
# 1. Create .env file
cp env.example .env

# 2. (Optional) Auto-configure Docker plugin
./scripts/setup-docker-gid.sh

# 3. Start the application
docker compose up -d

# 4. Access the web UI
# Open http://localhost:8080

# 5. Configure everything through the UI:
#    - Plugins: http://localhost:8080/plugins
#    - Notifications: http://localhost:8080/settings/notifications
#    - Servers: http://localhost:8080/servers
#    - Tasks: http://localhost:8080/tasks
```

**Note**: All configuration is now managed through the web UI and stored in the database. No need to edit config files!

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
- **[CLAUDE.md](./CLAUDE.md)**: Comprehensive AI development guide
- **[docs/deployment/docker.md](./docs/deployment/docker.md)**: Docker build and deployment workflow
- **[docs/deployment/docker-vm.md](./docs/deployment/docker-vm.md)**: Testing on docker-vm
- **[docs/status.md](./docs/status.md)**: Current project status
- **[docs/README.md](./docs/README.md)**: Documentation index

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
