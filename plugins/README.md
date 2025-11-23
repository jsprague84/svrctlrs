# SvrCtlRS Plugins

This directory contains both **core plugins** and **optional add-on plugins** for SvrCtlRS.

## Plugin Categories

### Core Plugins (Default Enabled)

Core plugins provide essential server management functionality and are included in the default build:

- **docker** - Docker container health, cleanup, and analysis
- **updates** - OS update detection, application, and cleanup
- **health** - System health monitoring (CPU, memory, disk)

### Add-on Plugins (Optional)

Add-on plugins provide nice-to-have monitoring features and are disabled by default:

- **weather** - Weather monitoring via OpenWeatherMap API
- **speedtest** - Internet speed test monitoring via Ookla CLI

## Building with Add-on Plugins

### Default Build (Core Plugins Only)

```bash
cargo build -p server
# or
cargo build --release -p server
```

This builds with docker, updates, and health plugins only.

### Enable Weather Plugin

```bash
cargo build -p server --features plugin-weather
```

### Enable Speed Test Plugin

```bash
cargo build -p server --features plugin-speedtest
```

### Enable All Plugins (Core + Add-ons)

```bash
cargo build -p server --features all-plugins
```

## Configuring Add-on Plugins

### 1. Build with Feature Flags

First, build the server with the desired add-on plugins enabled (see above).

### 2. Enable in Configuration

Add-on plugins must be explicitly enabled in configuration:

**Environment Variables:**
```bash
export ENABLE_WEATHER_PLUGIN=true
export ENABLE_SPEEDTEST_PLUGIN=true
```

**Or in config.toml:**
```toml
[plugins]
weather_enabled = true
speedtest_enabled = true
```

### 3. Configure Plugin Settings

#### Weather Plugin

```bash
# Required
export OWM_API_KEY=your_openweathermap_api_key

# Optional (with defaults)
export DEFAULT_LOCATION=Davenport,IA,US
export DEFAULT_ZIP=52801
export DEFAULT_UNITS=imperial  # or "metric"
export WEATHER_SCHEDULE="0 6 * * *"  # Default: 6 AM daily
```

#### Speed Test Plugin

```bash
# Optional thresholds
export SPEEDTEST_MIN_DOWN=100.0  # Mbps
export SPEEDTEST_MIN_UP=20.0     # Mbps
export SPEEDTEST_SERVER_ID=12345
export SPEEDTEST_SCHEDULE="0 */4 * * *"  # Default: every 4 hours
```

## Plugin Architecture

For detailed information about the plugin architecture, see:
- [Add-on Plugin Architecture](../docs/architecture/ADDON_PLUGINS.md)

### Why Optional Add-ons?

1. **Minimal Core** - Keep the base system lean and focused
2. **User Choice** - Enable only what you need
3. **Reduced Dependencies** - No unnecessary API keys or external tools
4. **Clear Separation** - Understand what's essential vs. nice-to-have
5. **Future-Proof** - Easy to add more add-ons without bloat

## Creating New Add-on Plugins

To create a new add-on plugin:

1. Create plugin crate: `plugins/myplugin/`
2. Add to workspace in root `Cargo.toml`
3. Implement `Plugin` trait from `svrctlrs-core`
4. Add feature flag in `server/Cargo.toml`:
   ```toml
   [features]
   plugin-myplugin = ["dep:svrctlrs-plugin-myplugin"]
   ```
5. Add config field in `server/src/config.rs`
6. Add conditional registration in `server/src/state.rs`

See existing add-on plugins (weather, speedtest) as examples.

## Examples

### Core Only (Minimal Build)

```bash
# Build
cargo build --release -p server

# Run - only docker, updates, health plugins available
./target/release/server
```

### With Weather Monitoring

```bash
# Build with weather add-on
cargo build --release -p server --features plugin-weather

# Configure
export ENABLE_WEATHER_PLUGIN=true
export OWM_API_KEY=your_key
export DEFAULT_ZIP=52801

# Run - weather plugin will fetch forecast at 6 AM daily
./target/release/server
```

### Full Featured (All Plugins)

```bash
# Build with everything
cargo build --release -p server --features all-plugins

# Configure
export ENABLE_WEATHER_PLUGIN=true
export ENABLE_SPEEDTEST_PLUGIN=true
export OWM_API_KEY=your_key
export DEFAULT_LOCATION=Davenport,IA,US

# Run - all plugins active
./target/release/server
```

## Notifications

All plugins (core and add-on) use the same notification system:

- **Gotify**: Service-specific keys (e.g., `WEATHER_GOTIFY_KEY`)
- **ntfy.sh**: Service-specific topics (e.g., `SPEEDY_NTFY_TOPIC`)

See the main README for notification configuration details.
