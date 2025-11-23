# SvrCtlRS

**Server Control Rust** - A modern, plugin-based infrastructure monitoring and automation platform.

## Overview

SvrCtlRS is a complete rewrite of the weatherust monitoring system with a focus on:

- **Plugin Architecture**: Modular, extensible design for easy feature additions
- **Built-in Scheduler**: No external dependencies like Ofelia
- **Modern Web UI**: Dioxus 0.7 fullstack framework with Axum backend
- **State Management**: SQLite for persistent state and historical data
- **Remote Execution**: SSH-based operations across multiple servers
- **Dual Notifications**: Gotify and ntfy.sh support with action buttons

## Architecture

```
svrctlrs/
├── core/           # Shared types, traits, plugin system
├── server/         # Main Axum server application
├── web/            # Dioxus UI components and pages
├── scheduler/      # Built-in cron-like task scheduler
├── database/       # SQLite abstraction and migrations
└── plugins/        # Monitoring plugins
    ├── docker/     # Docker health and updates
    ├── updates/    # OS/package updates
    └── health/     # System health monitoring
```

## Technology Stack

- **Backend**: Axum 0.8 (HTTP server)
- **Frontend**: Dioxus 0.6 (Fullstack Rust UI)
- **Database**: SQLite with sqlx
- **Runtime**: Tokio async runtime
- **Notifications**: Gotify + ntfy.sh

## Features

### Current Capabilities

- ✅ **Modular Plugin System**: Easy to add new monitoring capabilities
- ✅ **Built-in Scheduler**: Schedule tasks with cron expressions
- ✅ **Web Dashboard**: Real-time monitoring interface
- ✅ **Remote Operations**: SSH-based remote command execution
- ✅ **Webhook Triggers**: HTTP endpoints for external automation
- ✅ **Notification System**: Rich notifications with action buttons

### Planned Plugins

- **Docker Plugin**: Container health, resource monitoring, image updates
- **Updates Plugin**: OS package monitoring, automated updates
- **Health Plugin**: System metrics (CPU, memory, disk, network)
- **Weather Plugin**: OpenWeatherMap integration (optional)

## Quick Start

### Development

```bash
# Clone the repository
git clone https://github.com/jsprague84/svrctlrs
cd svrctlrs

# Run the server (with hot reload)
dx serve --hot-reload

# Or run directly
cargo run --bin server
```

### Production

```bash
# Build release binary
cargo build --release --bin server

# Run server
./target/release/server
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
- **`server/`**: Main application binary, Axum server setup
- **`web/`**: Dioxus components, pages, and client-side logic
- **`scheduler/`**: Task scheduling engine
- **`database/`**: Database layer, migrations, queries
- **`plugins/`**: Individual monitoring plugins

### Adding a New Plugin

1. Create new crate: `cargo new --lib plugins/myplugin`
2. Implement `Plugin` trait from `svrctlrs-core`
3. Add to workspace in `Cargo.toml`
4. Register in `server/src/main.rs`

See [CONTRIBUTING.md](./CONTRIBUTING.md) for detailed development guide.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
