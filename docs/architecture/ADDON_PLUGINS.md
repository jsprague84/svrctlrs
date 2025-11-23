# Optional Add-on Plugin Architecture

## Overview

SvrCtlRS separates plugins into two categories:

1. **Core Plugins** - Essential server management features (enabled by default)
2. **Add-on Plugins** - Optional monitoring features (disabled by default)

## Plugin Categories

### Core Plugins (Default Enabled)

Core plugins provide essential server management functionality:

- **docker** - Docker container health, cleanup, and analysis
- **updates** - OS update detection and management
- **health** - System health monitoring (CPU, memory, disk)

These plugins are included in the default build and enabled by default.

### Add-on Plugins (Optional)

Add-on plugins provide nice-to-have monitoring features:

- **weather** - Weather monitoring via OpenWeatherMap API
- **speedtest** - Internet speed test monitoring via Ookla CLI

These plugins are:
- **Disabled by default** - Must be explicitly enabled
- **Loosely coupled** - Can be added/removed without affecting core functionality
- **Optional dependencies** - External APIs and tools required
- **Future-extensible** - Easy to add more add-ons later

## Cargo Feature Flags

### Default Build

```bash
cargo build -p server
# Includes: docker, updates, health
# Excludes: weather, speedtest
```

### With Add-on Plugins

```bash
# Enable weather add-on
cargo build -p server --features plugin-weather

# Enable speed test add-on
cargo build -p server --features plugin-speedtest

# Enable all add-ons
cargo build -p server --features plugin-weather,plugin-speedtest

# Or use "all-plugins" feature
cargo build -p server --features all-plugins
```

## Configuration

Add-on plugins are configured via environment variables:

### Weather Plugin

```bash
# Required
OWM_API_KEY=your_openweathermap_api_key

# Optional (with defaults)
DEFAULT_LOCATION=Davenport,IA,US
DEFAULT_ZIP=52801
DEFAULT_UNITS=imperial  # or "metric"

# Notification keys
WEATHERUST_GOTIFY_KEY=key
WEATHERUST_NTFY_TOPIC=topic
```

### Speed Test Plugin

```bash
# Optional thresholds
SPEEDTEST_MIN_DOWN=100.0  # Mbps
SPEEDTEST_MIN_UP=20.0     # Mbps
SPEEDTEST_SERVER_ID=12345

# Notification keys
SPEEDY_GOTIFY_KEY=key
SPEEDY_NTFY_TOPIC=topic
```

## Docker Compose

The docker-compose.yml will have optional sections:

```yaml
services:
  # Core server (always present)
  svrctlrs:
    image: svrctlrs:latest
    # ...

  # Add-on: Weather monitoring (optional)
  # weather:
  #   image: svrctlrs:latest-addons
  #   environment:
  #     - OWM_API_KEY=${OWM_API_KEY}
  #   # ...

  # Add-on: Speed test (optional)
  # speedtest:
  #   image: svrctlrs:latest-addons
  #   # ...
```

## Plugin Registration

Core plugins are registered automatically:

```rust
// server/src/state.rs
#[cfg(feature = "plugin-docker")]
if self.config.plugins.docker_enabled {
    registry.register(Box::new(DockerPlugin::new()))?;
}
```

Add-on plugins require explicit configuration:

```rust
// server/src/state.rs
#[cfg(feature = "plugin-weather")]
if self.config.plugins.weather_enabled.unwrap_or(false) {
    registry.register(Box::new(WeatherPlugin::new()))?;
}
```

## File Structure

```
plugins/
├── docker/      # Core
├── updates/     # Core
├── health/      # Core
├── weather/     # Add-on (optional)
└── speedtest/   # Add-on (optional)
```

## Future Add-ons

The architecture makes it easy to add more optional plugins:

- Network monitoring (ping, traceroute)
- SSL certificate expiration checks
- Database backup verification
- Custom script execution
- Third-party service health checks

## Benefits

1. **Minimal core** - Core functionality stays lean
2. **User choice** - Users enable only what they need
3. **Reduced dependencies** - No unnecessary API keys or external tools
4. **Clear separation** - Easy to understand what's core vs. add-on
5. **Future-proof** - Easy to add more add-ons without bloat

## Implementation Checklist

- [x] Design add-on architecture
- [ ] Update Cargo.toml with feature flags
- [ ] Implement weather plugin
- [ ] Implement speedtest plugin
- [ ] Update config.rs for add-on config
- [ ] Update documentation
- [ ] Create Docker images (core and addons)
