# Plugin Development Guide

**Version:** 1.0  
**Last Updated:** 2025-11-25

---

## Overview

This guide explains how to develop custom plugins for SvrCtlRS. Plugins extend the system's monitoring and automation capabilities.

---

## Plugin Architecture

### Core Trait

All plugins must implement the `Plugin` trait:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn version(&self) -> &str;
    fn tasks(&self) -> Vec<PluginTask>;
    
    async fn execute(
        &self,
        task_id: &str,
        context: &ExecutionContext,
    ) -> Result<PluginResult, PluginError>;
}
```

### Plugin Structure

```
plugins/
└── my_plugin/
    ├── Cargo.toml
    ├── src/
    │   └── lib.rs
    └── README.md
```

---

## Creating a Plugin

### 1. Create Plugin Crate

```bash
cd plugins
cargo new --lib my_plugin
```

### 2. Add Dependencies

```toml
[package]
name = "svrctlrs-plugin-my_plugin"
version = "0.1.0"
edition = "2021"

[dependencies]
svrctlrs-core = { path = "../../core" }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
```

### 3. Implement Plugin

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use svrctlrs_core::plugin::{Plugin, PluginTask, PluginResult, PluginError, ExecutionContext};

pub struct MyPlugin {
    config: MyPluginConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyPluginConfig {
    pub api_key: String,
    pub endpoint: String,
}

impl MyPlugin {
    pub fn new() -> Self {
        Self {
            config: MyPluginConfig {
                api_key: std::env::var("MY_PLUGIN_API_KEY").unwrap_or_default(),
                endpoint: std::env::var("MY_PLUGIN_ENDPOINT")
                    .unwrap_or_else(|_| "https://api.example.com".to_string()),
            },
        }
    }

    pub fn from_config(config: serde_json::Value) -> Result<Self, PluginError> {
        let config: MyPluginConfig = serde_json::from_value(config)
            .map_err(|e| PluginError::ConfigError(e.to_string()))?;
        Ok(Self { config })
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn id(&self) -> &str {
        "my_plugin"
    }

    fn name(&self) -> &str {
        "My Plugin"
    }

    fn description(&self) -> &str {
        "Description of what my plugin does"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn tasks(&self) -> Vec<PluginTask> {
        vec![
            PluginTask {
                id: "check".to_string(),
                name: "Run Check".to_string(),
                description: "Performs a check".to_string(),
            },
        ]
    }

    async fn execute(
        &self,
        task_id: &str,
        context: &ExecutionContext,
    ) -> Result<PluginResult, PluginError> {
        match task_id {
            "check" => self.run_check(context).await,
            _ => Err(PluginError::UnknownTask(task_id.to_string())),
        }
    }
}

impl MyPlugin {
    async fn run_check(&self, context: &ExecutionContext) -> Result<PluginResult, PluginError> {
        tracing::info!("Running check for plugin");
        
        // Your plugin logic here
        let result = "Check completed successfully";
        
        Ok(PluginResult {
            success: true,
            message: result.to_string(),
            data: None,
        })
    }
}
```

---

## Registering Plugin

### 1. Add to server/Cargo.toml

```toml
[dependencies]
svrctlrs-plugin-my_plugin = { path = "../plugins/my_plugin", optional = true }

[features]
my_plugin = ["dep:svrctlrs-plugin-my_plugin"]
all-plugins = ["weather", "speedtest", "my_plugin"]
```

### 2. Register in server/src/state.rs

```rust
#[cfg(feature = "my_plugin")]
{
    use svrctlrs_plugin_my_plugin::MyPlugin;
    let plugin = if let Some(config) = db_plugin.config {
        MyPlugin::from_config(config)?
    } else {
        MyPlugin::new()
    };
    registry.register(Box::new(plugin));
}
```

---

## Database Configuration

Plugins can store configuration in the database:

```sql
INSERT INTO plugins (id, name, description, enabled, config) VALUES
  ('my_plugin', 'My Plugin', 'Description', 1, '{"api_key": "xxx", "endpoint": "https://api.example.com"}');
```

Access via UI at `/plugins/my_plugin/config`

---

## Best Practices

### 1. Error Handling
- Use `PluginError` for all errors
- Provide descriptive error messages
- Log errors with `tracing::error!`

### 2. Configuration
- Support both environment variables and database config
- Provide sensible defaults
- Validate configuration on initialization

### 3. Logging
- Use `tracing` for structured logging
- Log at appropriate levels (info, warn, error)
- Include context in log messages

### 4. Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_plugin_execution() {
        let plugin = MyPlugin::new();
        let context = ExecutionContext::default();
        let result = plugin.execute("check", &context).await;
        assert!(result.is_ok());
    }
}
```

### 5. Documentation
- Document all configuration options
- Provide usage examples
- Include troubleshooting guide

---

## Plugin Types

### Monitoring Plugins
- Collect metrics periodically
- Report status and health
- Examples: Docker, Health, Weather

### Action Plugins
- Execute commands or operations
- Modify system state
- Examples: Updates, Backup

### Integration Plugins
- Connect to external services
- Forward data or events
- Examples: Slack, Jira

---

## Advanced Features

### Custom Metrics
```rust
pub struct PluginMetric {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
}
```

### Notifications
```rust
context.notify(Notification {
    title: "Plugin Alert",
    message: "Something happened",
    priority: Priority::High,
}).await?;
```

### State Management
```rust
// Store plugin state in database
context.set_state("last_run", serde_json::to_value(Utc::now())?).await?;

// Retrieve plugin state
let last_run: DateTime<Utc> = context.get_state("last_run").await?;
```

---

## Publishing Plugins

### Community Plugins
1. Create GitHub repository
2. Add to plugin registry
3. Submit for review
4. Publish to crates.io

### Private Plugins
1. Keep in private repository
2. Add as Git dependency
3. Build with custom features

---

## Examples

See existing plugins for reference:
- `plugins/docker/` - Monitoring plugin with Bollard API
- `plugins/weather/` - External API integration
- `plugins/speedtest/` - CLI tool wrapper
- `plugins/health/` - System metrics collection
- `plugins/updates/` - System modification plugin

---

## Support

- Documentation: [README.md](../../README.md)
- Issues: GitHub Issues
- Discussions: GitHub Discussions

---

**Maintained by:** jsprague84

