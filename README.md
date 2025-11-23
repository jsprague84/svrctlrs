# SvrCtlRS

**Server Control Rust** - A modern, plugin-based infrastructure monitoring and automation platform with fullstack web UI.

[![Version](https://img.shields.io/badge/version-2.1.0--fullstack-blue.svg)](https://github.com/jsprague84/svrctlrs)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Dioxus](https://img.shields.io/badge/dioxus-0.7-green.svg)](https://dioxuslabs.com)

## Overview

SvrCtlRS is a complete rewrite of the weatherust monitoring system with a focus on:

- **Plugin Architecture**: Modular, extensible design for easy feature additions
- **Built-in Scheduler**: No external dependencies like Ofelia
- **Modern Web UI**: Dioxus 0.7 fullstack with SSR + WASM hydration
- **Axum Backend**: High-performance REST API
- **State Management**: SQLite for persistent state and historical data
- **Remote Execution**: SSH-based operations across multiple servers
- **Dual Notifications**: Gotify and ntfy.sh support with action buttons

## Architecture

```
svrctlrs/
├── core/              # Shared types, traits, plugin system
├── server/            # Fullstack application
│   ├── src/main.rs   # Dual entry points (server + WASM client)
│   ├── src/ui/       # Dioxus components and pages
│   ├── src/routes/   # Axum API routes
│   └── src/state.rs  # Application state management
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
- **Frontend**: Dioxus 0.7 Fullstack (SSR + WASM hydration)
- **Build Tool**: Dioxus CLI (dx) v0.7.1
- **Database**: SQLite with sqlx
- **Runtime**: Tokio async runtime
- **Notifications**: Gotify + ntfy.sh with action buttons
- **Remote Ops**: SSH with openssh_sftp_client

## Features

### 🚀 Core Capabilities

- ✅ **Modular Plugin System**: Easy to add new monitoring capabilities
- ✅ **Built-in Scheduler**: Schedule tasks with cron expressions
- ✅ **Fullstack Web Dashboard**: Dioxus 0.7 with SSR + WASM hydration
- ✅ **Remote Operations**: SSH-based remote command execution
- ✅ **REST API**: Full HTTP API for programmatic access
- ✅ **CLI Tool**: `svrctl` command-line interface
- ✅ **Notification System**: Rich notifications with action buttons
- ✅ **Database Persistence**: SQLite for historical data

### 📦 Implemented Plugins

- ✅ **Docker Plugin**: Container health, resource monitoring, image updates
- ✅ **Updates Plugin**: OS package monitoring, automated updates, cleanup
- ✅ **Health Plugin**: System metrics (CPU, memory, disk, network)
- ✅ **Weather Plugin**: OpenWeatherMap integration (optional)
- ✅ **Speed Test Plugin**: Ookla speed test monitoring (optional)

## Quick Start

### Prerequisites

```bash
# Install Dioxus CLI (required for fullstack development)
cargo install dioxus-cli --version 0.7.1

# Verify installation
dx --version
```

### Development

```bash
# Clone the repository
git clone https://github.com/jsprague84/svrctlrs
cd svrctlrs

# Copy example config
cp config.example.toml config.toml

# Run development server with hot reload
dx serve --package server

# Server starts at http://localhost:8080
# - Rust changes trigger server restart
# - UI changes hot-reload in browser
```

### Production Build

```bash
# Build fullstack release (server + WASM client)
dx build --release

# Output:
# - target/release/server (binary with embedded assets)
# - dist/ (WASM bundle + JavaScript loader)

# Run production server
./target/release/server --config config.toml
```

### Docker

```bash
# Build image
docker build -t svrctlrs:latest .

# Run container
docker run -d \
  -p 8080:8080 \
  -v ./data:/data \
  --name svrctlrs \
  svrctlrs:latest
```

## Configuration

Configuration is managed through environment variables and `.env` file:

```bash
# Server configuration
SERVER_ADDR=0.0.0.0:8080
DATABASE_URL=sqlite:data/svrctlrs.db

# Notification backends
GOTIFY_URL=http://gotify:8080/message
GOTIFY_KEY=your-gotify-token
NTFY_URL=https://ntfy.sh
NTFY_TOPIC=svrctlrs-alerts

# SSH for remote operations
SSH_KEY_PATH=/path/to/ssh/key
REMOTE_SERVERS=server1:user@host1,server2:user@host2

# Plugin configuration
ENABLE_DOCKER_PLUGIN=true
ENABLE_UPDATES_PLUGIN=true
ENABLE_HEALTH_PLUGIN=true
```

## Development

### Project Structure

- **`core/`**: Core library with traits and types used by all plugins
- **`server/`**: Fullstack application
  - `src/main.rs` - Dual entry points (server + WASM client)
  - `src/ui/` - Dioxus UI components and pages
  - `src/routes/` - Axum REST API routes
  - `src/state.rs` - Application state management
- **`scheduler/`**: Task scheduling engine
- **`database/`**: Database layer, migrations, queries
- **`plugins/`**: Individual monitoring plugins

### Adding a New Plugin

1. Create new crate: `cargo new --lib plugins/myplugin`
2. Implement `Plugin` trait from `svrctlrs-core`
3. Add to workspace in `Cargo.toml`
4. Register in `server/src/state.rs`
5. Add UI components in `server/src/ui/`

### Key Development Files

- **`CLAUDE.md`**: Comprehensive development guide for Claude Code
- **`Dioxus.toml`**: Dioxus build configuration
- **`docker-compose.yml`**: Container orchestration
- **`Dockerfile`**: Multi-stage build for fullstack deployment

See [CLAUDE.md](./CLAUDE.md) for detailed development patterns and Context7 usage.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
