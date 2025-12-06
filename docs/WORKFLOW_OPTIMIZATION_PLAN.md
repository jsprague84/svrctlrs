# Workflow Optimization Plan: Jobs, Scheduling & Notifications

**Document Version**: 1.0
**Created**: 2025-12-06
**Status**: Planning Phase
**Last Session**: Comprehensive research and plan creation

---

## Session Context (For Claude Code Continuation)

### Quick Resume Instructions

When resuming this implementation, provide Claude with:

```
Continue implementing the Workflow Optimization Plan from docs/WORKFLOW_OPTIMIZATION_PLAN.md.
Current phase: [PHASE NUMBER]
Last completed task: [TASK NAME]
```

### Implementation Status Tracking

| Phase | Status | Completion |
|-------|--------|------------|
| Phase 1: Job Catalog System | Not Started | 0% |
| Phase 2: Job Wizard UI | Not Started | 0% |
| Phase 3: Notification Enhancements | Not Started | 0% |
| Phase 4: Dashboard Quick Actions | Not Started | 0% |
| Phase 5: Advanced Mode Polish | Not Started | 0% |

### Key Files Modified (Track As You Go)

```
# Database
- [ ] database/migrations/015_job_catalog_and_workflow.sql
- [ ] database/src/models/job_catalog.rs
- [ ] database/src/queries/job_catalog.rs

# Server/Routes
- [ ] server/src/routes/ui/job_wizard.rs
- [ ] server/src/routes/ui/job_catalog.rs
- [ ] server/src/routes/ui/quick_actions.rs

# Templates
- [ ] server/templates/pages/job_wizard.html
- [ ] server/templates/components/job_catalog_*.html
- [ ] server/templates/components/quick_actions.html

# Display Models
- [ ] server/src/templates.rs (JobCatalogDisplay, etc.)

# Documentation
- [ ] docs/WORKFLOW_OPTIMIZATION_PLAN.md (this file)
- [ ] docs/JOBS.md (update with Basic/Advanced modes)
```

---

## Executive Summary

### Problem Statement

The current job creation workflow requires navigating through multiple screens:
1. Job Types (select category)
2. Command Templates (create/select command)
3. Job Templates (create job definition)
4. Job Schedules (schedule the job)
5. Notification Policies (configure alerts)

This is powerful but overwhelming for common operations.

### Solution: Two-Tier System

**Basic Mode** (Default)
- Pre-built job catalog with one-click configuration
- Single-page wizard for common operations
- Smart defaults based on server capabilities
- Target: 80% of use cases in 20% of the clicks

**Advanced Mode** (Power Users)
- Full access to all current functionality
- Custom command templates
- Composite workflows (multi-step jobs)
- Target: Complete flexibility for complex automation

### Similar Applications Researched

| Application | Key Pattern Adopted |
|-------------|---------------------|
| **Rundeck** | Job options with parameter types, error handlers |
| **AWX** | Workflow branching (success/failure paths), notification templates |
| **Cronicle** | Chain reactions, plugin parameters, separate success/failure notifications |

---

## Phase 1: Job Catalog System

### 1.1 Database Schema

**Migration**: `database/migrations/015_job_catalog_and_workflow.sql`

```sql
-- ============================================================================
-- Job Catalog: Pre-built jobs for Basic Mode
-- ============================================================================

CREATE TABLE job_catalog (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Identity
    name TEXT NOT NULL UNIQUE,            -- 'docker_cleanup', 'apt_update'
    display_name TEXT NOT NULL,           -- 'Docker Cleanup'
    description TEXT NOT NULL,

    -- Categorization
    category TEXT NOT NULL,               -- 'docker', 'system', 'backup', 'monitoring'
    subcategory TEXT,                     -- 'containers', 'images', 'networks'
    icon TEXT DEFAULT 'terminal',         -- Lucide icon name
    difficulty TEXT DEFAULT 'basic' CHECK(difficulty IN ('basic', 'intermediate', 'advanced')),

    -- Command Definition
    command TEXT NOT NULL,                -- Command with {{variable}} placeholders
    parameters JSON,                      -- Parameter schema (see below)
    required_capabilities JSON,           -- ["docker"] or ["apt"]
    os_filter JSON,                       -- {"distro": ["ubuntu", "debian"]}

    -- Execution Defaults
    default_timeout INTEGER DEFAULT 300,
    default_retry_count INTEGER DEFAULT 0,
    working_directory TEXT,
    environment JSON,                     -- {"VAR": "value"}

    -- Notification Templates
    success_title_template TEXT,          -- "{{job_name}} completed on {{server_name}}"
    success_body_template TEXT,
    failure_title_template TEXT,
    failure_body_template TEXT,
    ntfy_success_tags JSON,               -- ["white_check_mark", "docker"]
    ntfy_failure_tags JSON,               -- ["x", "warning"]

    -- Metadata
    tags JSON,                            -- ["cleanup", "maintenance", "disk-space"]
    sort_order INTEGER DEFAULT 0,
    is_system BOOLEAN DEFAULT 1,          -- System-provided (not user-created)
    enabled BOOLEAN DEFAULT 1,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_job_catalog_category ON job_catalog(category);
CREATE INDEX idx_job_catalog_difficulty ON job_catalog(difficulty);
CREATE INDEX idx_job_catalog_enabled ON job_catalog(enabled);

-- ============================================================================
-- Job Catalog Categories: Organize catalog items
-- ============================================================================

CREATE TABLE job_catalog_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,            -- 'docker', 'system', 'backup'
    display_name TEXT NOT NULL,           -- 'Docker Operations'
    description TEXT,
    icon TEXT DEFAULT 'folder',
    color TEXT,                           -- '#2496ED' (Docker blue)
    sort_order INTEGER DEFAULT 0,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- User Favorites: Pin frequently used catalog items
-- ============================================================================

CREATE TABLE job_catalog_favorites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_item_id INTEGER NOT NULL,
    sort_order INTEGER DEFAULT 0,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (catalog_item_id) REFERENCES job_catalog(id) ON DELETE CASCADE,
    UNIQUE(catalog_item_id)
);

-- ============================================================================
-- Enhanced Notification Policies: Separate success/failure templates
-- ============================================================================

ALTER TABLE notification_policies ADD COLUMN success_title_template TEXT;
ALTER TABLE notification_policies ADD COLUMN success_body_template TEXT;
ALTER TABLE notification_policies ADD COLUMN failure_title_template TEXT;
ALTER TABLE notification_policies ADD COLUMN failure_body_template TEXT;
ALTER TABLE notification_policies ADD COLUMN include_output BOOLEAN DEFAULT 0;
ALTER TABLE notification_policies ADD COLUMN output_max_lines INTEGER DEFAULT 50;
ALTER TABLE notification_policies ADD COLUMN ntfy_success_tags TEXT;  -- JSON array
ALTER TABLE notification_policies ADD COLUMN ntfy_failure_tags TEXT;  -- JSON array

-- ============================================================================
-- User Preferences: UI mode and quick action settings
-- ============================================================================

INSERT OR REPLACE INTO settings (key, value, description, value_type) VALUES
    ('ui.mode', '"basic"', 'UI mode: basic (wizard) or advanced (full)', 'string'),
    ('ui.show_quick_actions', 'true', 'Show quick actions panel on dashboard', 'boolean'),
    ('ui.default_wizard_step', '"select"', 'Default wizard step to start on', 'string');

-- ============================================================================
-- Job Template Enhancements: Link to catalog for "Run Again" feature
-- ============================================================================

ALTER TABLE job_templates ADD COLUMN catalog_item_id INTEGER REFERENCES job_catalog(id) ON DELETE SET NULL;
ALTER TABLE job_templates ADD COLUMN is_pinned BOOLEAN DEFAULT 0;

-- ============================================================================
-- Job Schedule Enhancements: Quick execution support
-- ============================================================================

ALTER TABLE job_schedules ADD COLUMN last_manual_run_at DATETIME;
ALTER TABLE job_schedules ADD COLUMN manual_run_count INTEGER DEFAULT 0;
```

### 1.2 Parameter Schema Definition

The `parameters` JSON column uses this schema:

```json
[
  {
    "name": "container_name",
    "type": "string",
    "label": "Container Name",
    "description": "Name or ID of the Docker container",
    "required": true,
    "default": "",
    "placeholder": "my-container",
    "validation": {
      "pattern": "^[a-zA-Z0-9][a-zA-Z0-9_.-]*$",
      "minLength": 1,
      "maxLength": 128
    }
  },
  {
    "name": "prune_volumes",
    "type": "boolean",
    "label": "Prune Volumes",
    "description": "Also remove unused volumes (caution: data loss)",
    "required": false,
    "default": false,
    "warning": "This will permanently delete data!"
  },
  {
    "name": "log_lines",
    "type": "number",
    "label": "Log Lines",
    "description": "Number of log lines to display",
    "required": false,
    "default": 100,
    "validation": {
      "min": 1,
      "max": 10000
    }
  },
  {
    "name": "status_filter",
    "type": "select",
    "label": "Status Filter",
    "description": "Filter containers by status",
    "required": false,
    "default": "all",
    "options": [
      {"value": "all", "label": "All Containers"},
      {"value": "running", "label": "Running Only"},
      {"value": "stopped", "label": "Stopped Only"},
      {"value": "exited", "label": "Exited Only"}
    ]
  },
  {
    "name": "packages",
    "type": "multiselect",
    "label": "Packages",
    "description": "Select packages to install",
    "required": true,
    "options": [
      {"value": "nginx", "label": "Nginx"},
      {"value": "apache2", "label": "Apache"},
      {"value": "mysql-server", "label": "MySQL"}
    ]
  }
]
```

### 1.3 Seed Data: Pre-Built Jobs

```sql
-- ============================================================================
-- SEED: Job Catalog Categories
-- ============================================================================

INSERT INTO job_catalog_categories (name, display_name, description, icon, color, sort_order) VALUES
    ('docker', 'Docker', 'Container management and cleanup', 'container', '#2496ED', 1),
    ('system', 'System', 'OS updates and maintenance', 'server', '#5E81AC', 2),
    ('monitoring', 'Monitoring', 'Health checks and diagnostics', 'activity', '#88C0D0', 3),
    ('backup', 'Backup', 'Data backup and restoration', 'archive', '#A3BE8C', 4),
    ('network', 'Network', 'Network diagnostics and configuration', 'globe', '#B48EAD', 5),
    ('security', 'Security', 'Security scans and updates', 'shield', '#BF616A', 6);

-- ============================================================================
-- SEED: Docker Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, success_title_template, failure_title_template, ntfy_success_tags, ntfy_failure_tags) VALUES
(
    'docker_cleanup',
    'Docker Cleanup',
    'Remove stopped containers, unused networks, and dangling images to free disk space',
    'docker',
    'trash-2',
    'docker system prune -f {{#if prune_volumes}}--volumes{{/if}}',
    '[{"name": "prune_volumes", "type": "boolean", "label": "Also prune volumes", "description": "Remove unused volumes (caution: data loss possible)", "default": false, "warning": "This will permanently delete volume data!"}]',
    '["docker"]',
    'basic',
    300,
    'Docker cleanup completed on {{server_name}}',
    'Docker cleanup failed on {{server_name}}',
    '["white_check_mark", "whale"]',
    '["x", "whale"]'
),
(
    'docker_container_restart',
    'Restart Container',
    'Restart a specific Docker container',
    'docker',
    'refresh-cw',
    'docker restart {{container_name}}',
    '[{"name": "container_name", "type": "string", "label": "Container Name", "description": "Name or ID of the container to restart", "required": true, "placeholder": "my-container"}]',
    '["docker"]',
    'basic',
    120,
    'Container {{container_name}} restarted on {{server_name}}',
    'Failed to restart {{container_name}} on {{server_name}}',
    '["arrows_counterclockwise", "whale"]',
    '["x", "whale"]'
),
(
    'docker_logs',
    'View Container Logs',
    'Display recent logs from a Docker container',
    'docker',
    'file-text',
    'docker logs --tail {{lines}} {{container_name}}',
    '[{"name": "container_name", "type": "string", "label": "Container Name", "required": true}, {"name": "lines", "type": "number", "label": "Lines", "description": "Number of log lines to show", "default": 100, "validation": {"min": 1, "max": 5000}}]',
    '["docker"]',
    'basic',
    60,
    NULL,
    'Failed to get logs for {{container_name}}',
    NULL,
    '["x", "whale"]'
),
(
    'docker_stats',
    'Container Stats',
    'Show resource usage statistics for all running containers',
    'docker',
    'bar-chart-2',
    'docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"',
    '[]',
    '["docker"]',
    'basic',
    30,
    NULL,
    'Failed to get Docker stats on {{server_name}}',
    NULL,
    '["x"]'
),
(
    'docker_pull_image',
    'Pull Image',
    'Pull a Docker image from registry',
    'docker',
    'download',
    'docker pull {{image_name}}{{#if tag}}:{{tag}}{{/if}}',
    '[{"name": "image_name", "type": "string", "label": "Image Name", "required": true, "placeholder": "nginx"}, {"name": "tag", "type": "string", "label": "Tag", "default": "latest", "placeholder": "latest"}]',
    '["docker"]',
    'intermediate',
    600,
    'Pulled {{image_name}} on {{server_name}}',
    'Failed to pull {{image_name}} on {{server_name}}',
    '["inbox_tray", "whale"]',
    '["x", "whale"]'
);

-- ============================================================================
-- SEED: System Maintenance Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, os_filter, difficulty, default_timeout, success_title_template, failure_title_template) VALUES
(
    'apt_update',
    'Update Package Lists (APT)',
    'Update APT package lists on Debian/Ubuntu systems',
    'system',
    'refresh-cw',
    'sudo apt-get update',
    '[]',
    '["apt"]',
    '{"distro": ["debian", "ubuntu"]}',
    'basic',
    300,
    'Package lists updated on {{server_name}}',
    'Failed to update packages on {{server_name}}'
),
(
    'apt_upgrade',
    'System Upgrade (APT)',
    'Perform full system upgrade on Debian/Ubuntu',
    'system',
    'arrow-up-circle',
    'sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y {{#if dist_upgrade}}&& sudo DEBIAN_FRONTEND=noninteractive apt-get dist-upgrade -y{{/if}}',
    '[{"name": "dist_upgrade", "type": "boolean", "label": "Include distribution upgrade", "description": "Also run dist-upgrade for major updates", "default": false}]',
    '["apt"]',
    '{"distro": ["debian", "ubuntu"]}',
    'intermediate',
    1800,
    'System upgraded on {{server_name}}',
    'Upgrade failed on {{server_name}}'
),
(
    'dnf_upgrade',
    'System Upgrade (DNF)',
    'Perform full system upgrade on Fedora/RHEL',
    'system',
    'arrow-up-circle',
    'sudo dnf upgrade -y',
    '[]',
    '["dnf"]',
    '{"distro": ["fedora", "rhel", "centos", "rocky", "alma"]}',
    'basic',
    1800,
    'System upgraded on {{server_name}}',
    'Upgrade failed on {{server_name}}'
),
(
    'reboot_server',
    'Reboot Server',
    'Safely reboot the server',
    'system',
    'power',
    'sudo shutdown -r {{delay}}',
    '[{"name": "delay", "type": "select", "label": "Delay", "default": "now", "options": [{"value": "now", "label": "Immediately"}, {"value": "+1", "label": "In 1 minute"}, {"value": "+5", "label": "In 5 minutes"}]}]',
    '[]',
    NULL,
    'advanced',
    60,
    'Reboot initiated on {{server_name}}',
    'Failed to reboot {{server_name}}'
);

-- ============================================================================
-- SEED: Monitoring Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout) VALUES
(
    'disk_usage',
    'Disk Usage Report',
    'Show disk usage for all mounted filesystems',
    'monitoring',
    'hard-drive',
    'df -h',
    '[]',
    '[]',
    'basic',
    30
),
(
    'memory_usage',
    'Memory Usage',
    'Display memory usage statistics',
    'monitoring',
    'cpu',
    'free -h',
    '[]',
    '[]',
    'basic',
    30
),
(
    'top_processes',
    'Top Processes',
    'Show processes by CPU/memory usage',
    'monitoring',
    'activity',
    'ps aux --sort=-{{sort_by}} | head -n {{count}}',
    '[{"name": "sort_by", "type": "select", "label": "Sort by", "default": "%cpu", "options": [{"value": "%cpu", "label": "CPU Usage"}, {"value": "%mem", "label": "Memory Usage"}]}, {"name": "count", "type": "number", "label": "Process count", "default": 10, "validation": {"min": 5, "max": 50}}]',
    '[]',
    'basic',
    30
),
(
    'systemd_failed',
    'Failed Services',
    'List all failed systemd services',
    'monitoring',
    'alert-triangle',
    'systemctl --failed',
    '[]',
    '["systemd"]',
    'basic',
    30
),
(
    'check_port',
    'Check Port',
    'Verify if a port is listening',
    'monitoring',
    'radio',
    'ss -tlnp | grep :{{port}} || echo "Port {{port}} is not listening"',
    '[{"name": "port", "type": "number", "label": "Port Number", "required": true, "validation": {"min": 1, "max": 65535}}]',
    '[]',
    'basic',
    30
);
```

### 1.4 Database Models

**File**: `database/src/models/job_catalog.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

/// Pre-built job from the catalog
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JobCatalogItem {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: String,
    pub difficulty: String,
    pub command: String,
    pub parameters: Option<JsonValue>,
    pub required_capabilities: Option<JsonValue>,
    pub os_filter: Option<JsonValue>,
    pub default_timeout: i64,
    pub default_retry_count: i64,
    pub working_directory: Option<String>,
    pub environment: Option<JsonValue>,
    pub success_title_template: Option<String>,
    pub success_body_template: Option<String>,
    pub failure_title_template: Option<String>,
    pub failure_body_template: Option<String>,
    pub ntfy_success_tags: Option<JsonValue>,
    pub ntfy_failure_tags: Option<JsonValue>,
    pub tags: Option<JsonValue>,
    pub sort_order: i64,
    pub is_system: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JobCatalogItem {
    /// Get required capabilities as a Vec
    pub fn get_required_capabilities(&self) -> Vec<String> {
        self.required_capabilities
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get parameter schema as parsed struct
    pub fn get_parameters(&self) -> Vec<CatalogParameter> {
        self.parameters
            .as_ref()
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Check if server has required capabilities
    pub fn is_compatible_with(&self, server_capabilities: &[String]) -> bool {
        self.get_required_capabilities()
            .iter()
            .all(|cap| server_capabilities.contains(cap))
    }
}

/// Parameter definition for catalog items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: String,  // "string", "number", "boolean", "select", "multiselect"
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<JsonValue>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub validation: Option<ParameterValidation>,
    #[serde(default)]
    pub options: Option<Vec<SelectOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterValidation {
    pub pattern: Option<String>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    #[serde(rename = "minLength")]
    pub min_length: Option<i64>,
    #[serde(rename = "maxLength")]
    pub max_length: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// Job catalog category
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct JobCatalogCategory {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub icon: String,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: DateTime<Utc>,
}
```

### 1.5 Display Models

**File**: `server/src/templates.rs` (additions)

```rust
/// Display model for job catalog items (template-safe)
#[derive(Debug, Clone)]
pub struct JobCatalogItemDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub icon: String,
    pub difficulty: String,
    pub difficulty_badge_class: String,  // "badge-success", "badge-warning", "badge-danger"
    pub command: String,
    pub parameters_json: String,         // Pre-serialized for Alpine.js
    pub parameters: Vec<CatalogParameterDisplay>,
    pub required_capabilities: Vec<String>,
    pub default_timeout: i64,
    pub is_favorite: bool,
    pub tags: Vec<String>,
}

impl From<JobCatalogItem> for JobCatalogItemDisplay {
    fn from(item: JobCatalogItem) -> Self {
        let difficulty_badge_class = match item.difficulty.as_str() {
            "basic" => "badge-success",
            "intermediate" => "badge-warning",
            "advanced" => "badge-danger",
            _ => "badge-secondary",
        }.to_string();

        let parameters = item.get_parameters();
        let parameters_json = serde_json::to_string(&item.parameters)
            .unwrap_or_else(|_| "[]".to_string());

        Self {
            id: item.id,
            name: item.name,
            display_name: item.display_name,
            description: item.description,
            category: item.category,
            subcategory: item.subcategory,
            icon: item.icon,
            difficulty: item.difficulty,
            difficulty_badge_class,
            command: item.command,
            parameters_json,
            parameters: parameters.into_iter().map(Into::into).collect(),
            required_capabilities: item.get_required_capabilities(),
            default_timeout: item.default_timeout,
            is_favorite: false, // Set by query
            tags: item.tags
                .and_then(|v| v.as_array().cloned())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CatalogParameterDisplay {
    pub name: String,
    pub param_type: String,
    pub label: String,
    pub description: String,
    pub required: bool,
    pub default_value: String,
    pub placeholder: String,
    pub warning: Option<String>,
    pub options_json: String,  // For select/multiselect
    pub has_validation: bool,
    pub validation_json: String,
}

impl From<CatalogParameter> for CatalogParameterDisplay {
    fn from(p: CatalogParameter) -> Self {
        Self {
            name: p.name,
            param_type: p.param_type,
            label: p.label,
            description: p.description.unwrap_or_default(),
            required: p.required,
            default_value: p.default
                .map(|v| match v {
                    JsonValue::String(s) => s,
                    other => other.to_string(),
                })
                .unwrap_or_default(),
            placeholder: p.placeholder.unwrap_or_default(),
            warning: p.warning,
            options_json: p.options
                .map(|o| serde_json::to_string(&o).unwrap_or_default())
                .unwrap_or_else(|| "[]".to_string()),
            has_validation: p.validation.is_some(),
            validation_json: p.validation
                .map(|v| serde_json::to_string(&v).unwrap_or_default())
                .unwrap_or_else(|| "{}".to_string()),
        }
    }
}

/// Category with item count for sidebar
#[derive(Debug, Clone)]
pub struct JobCatalogCategoryDisplay {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub item_count: i64,
    pub is_active: bool,
}
```

### 1.6 Query Functions

**File**: `database/src/queries/job_catalog.rs`

```rust
use sqlx::SqlitePool;
use crate::models::{JobCatalogItem, JobCatalogCategory};

/// List all enabled catalog items
pub async fn list_catalog_items(pool: &SqlitePool) -> Result<Vec<JobCatalogItem>, sqlx::Error> {
    sqlx::query_as!(
        JobCatalogItem,
        r#"
        SELECT * FROM job_catalog
        WHERE enabled = 1
        ORDER BY category, sort_order, display_name
        "#
    )
    .fetch_all(pool)
    .await
}

/// List catalog items by category
pub async fn list_catalog_items_by_category(
    pool: &SqlitePool,
    category: &str,
) -> Result<Vec<JobCatalogItem>, sqlx::Error> {
    sqlx::query_as!(
        JobCatalogItem,
        r#"
        SELECT * FROM job_catalog
        WHERE enabled = 1 AND category = ?
        ORDER BY sort_order, display_name
        "#,
        category
    )
    .fetch_all(pool)
    .await
}

/// List catalog items compatible with a server's capabilities
pub async fn list_compatible_catalog_items(
    pool: &SqlitePool,
    server_id: i64,
) -> Result<Vec<JobCatalogItem>, sqlx::Error> {
    // Get server capabilities
    let capabilities: Vec<String> = sqlx::query_scalar!(
        r#"
        SELECT capability FROM server_capabilities
        WHERE server_id = ? AND available = 1
        "#,
        server_id
    )
    .fetch_all(pool)
    .await?;

    // For now, return all and filter in Rust
    // Could optimize with JSON functions in SQLite if performance is an issue
    let all_items = list_catalog_items(pool).await?;

    Ok(all_items
        .into_iter()
        .filter(|item| item.is_compatible_with(&capabilities))
        .collect())
}

/// Get catalog item by ID
pub async fn get_catalog_item(
    pool: &SqlitePool,
    id: i64,
) -> Result<JobCatalogItem, sqlx::Error> {
    sqlx::query_as!(
        JobCatalogItem,
        "SELECT * FROM job_catalog WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await
}

/// List all categories with item counts
pub async fn list_categories_with_counts(
    pool: &SqlitePool,
) -> Result<Vec<(JobCatalogCategory, i64)>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            c.*,
            COUNT(j.id) as item_count
        FROM job_catalog_categories c
        LEFT JOIN job_catalog j ON j.category = c.name AND j.enabled = 1
        GROUP BY c.id
        ORDER BY c.sort_order, c.display_name
        "#
    )
    .fetch_all(pool)
    .await?;

    // Convert to tuples
    Ok(rows.into_iter().map(|r| {
        (
            JobCatalogCategory {
                id: r.id,
                name: r.name,
                display_name: r.display_name,
                description: r.description,
                icon: r.icon,
                color: r.color,
                sort_order: r.sort_order,
                created_at: r.created_at,
            },
            r.item_count
        )
    }).collect())
}

/// Toggle favorite status
pub async fn toggle_catalog_favorite(
    pool: &SqlitePool,
    catalog_item_id: i64,
) -> Result<bool, sqlx::Error> {
    // Check if exists
    let exists = sqlx::query_scalar!(
        "SELECT id FROM job_catalog_favorites WHERE catalog_item_id = ?",
        catalog_item_id
    )
    .fetch_optional(pool)
    .await?;

    if exists.is_some() {
        sqlx::query!(
            "DELETE FROM job_catalog_favorites WHERE catalog_item_id = ?",
            catalog_item_id
        )
        .execute(pool)
        .await?;
        Ok(false) // No longer favorite
    } else {
        sqlx::query!(
            "INSERT INTO job_catalog_favorites (catalog_item_id) VALUES (?)",
            catalog_item_id
        )
        .execute(pool)
        .await?;
        Ok(true) // Now favorite
    }
}

/// List user's favorite catalog items
pub async fn list_favorites(pool: &SqlitePool) -> Result<Vec<JobCatalogItem>, sqlx::Error> {
    sqlx::query_as!(
        JobCatalogItem,
        r#"
        SELECT j.* FROM job_catalog j
        INNER JOIN job_catalog_favorites f ON f.catalog_item_id = j.id
        WHERE j.enabled = 1
        ORDER BY f.sort_order, j.display_name
        "#
    )
    .fetch_all(pool)
    .await
}
```

---

## Phase 2: Job Wizard UI

### 2.1 Route Handler

**File**: `server/src/routes/ui/job_wizard.rs`

```rust
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;

use crate::{
    routes::ui::AppError,
    state::AppState,
    templates::{JobCatalogItemDisplay, JobCatalogCategoryDisplay, ServerDisplay},
};

/// Create router for job wizard
pub fn routes() -> Router<AppState> {
    Router::new()
        // Wizard pages
        .route("/jobs/wizard", get(wizard_page))
        .route("/jobs/wizard/catalog", get(catalog_grid))
        .route("/jobs/wizard/catalog/:category", get(catalog_category))
        .route("/jobs/wizard/configure/:id", get(configure_step))
        .route("/jobs/wizard/servers/:id", get(servers_step))
        .route("/jobs/wizard/schedule/:id", get(schedule_step))
        .route("/jobs/wizard/notify/:id", get(notify_step))
        .route("/jobs/wizard/preview", post(preview_job))
        .route("/jobs/wizard/create", post(create_job))
        // Quick actions
        .route("/jobs/quick-run/:catalog_id", post(quick_run_job))
        .route("/jobs/run-again/:schedule_id", post(run_again))
}

#[derive(Template)]
#[template(path = "pages/job_wizard.html")]
struct JobWizardTemplate {
    user: Option<String>,
    categories: Vec<JobCatalogCategoryDisplay>,
    favorites: Vec<JobCatalogItemDisplay>,
    current_step: String,
}

/// Main wizard page
async fn wizard_page(
    State(state): State<AppState>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let categories_with_counts = queries::job_catalog::list_categories_with_counts(db.pool()).await?;
    let categories: Vec<JobCatalogCategoryDisplay> = categories_with_counts
        .into_iter()
        .map(|(cat, count)| JobCatalogCategoryDisplay {
            id: cat.id,
            name: cat.name.clone(),
            display_name: cat.display_name,
            description: cat.description.unwrap_or_default(),
            icon: cat.icon,
            color: cat.color.unwrap_or_else(|| "#5E81AC".to_string()),
            item_count: count,
            is_active: false,
        })
        .collect();

    let favorite_items = queries::job_catalog::list_favorites(db.pool()).await?;
    let favorites: Vec<JobCatalogItemDisplay> = favorite_items
        .into_iter()
        .map(|item| {
            let mut display: JobCatalogItemDisplay = item.into();
            display.is_favorite = true;
            display
        })
        .collect();

    let template = JobWizardTemplate {
        user: None,
        categories,
        favorites,
        current_step: "select".to_string(),
    };

    Ok(Html(template.render()?))
}

#[derive(Deserialize)]
struct CatalogQuery {
    search: Option<String>,
    difficulty: Option<String>,
}

/// Catalog grid (HTMX partial)
async fn catalog_grid(
    State(state): State<AppState>,
    Query(query): Query<CatalogQuery>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let items = queries::job_catalog::list_catalog_items(db.pool()).await?;

    // Apply filters
    let filtered: Vec<JobCatalogItemDisplay> = items
        .into_iter()
        .filter(|item| {
            // Search filter
            if let Some(ref search) = query.search {
                let search_lower = search.to_lowercase();
                if !item.display_name.to_lowercase().contains(&search_lower)
                    && !item.description.to_lowercase().contains(&search_lower)
                {
                    return false;
                }
            }
            // Difficulty filter
            if let Some(ref diff) = query.difficulty {
                if diff != "all" && item.difficulty != *diff {
                    return false;
                }
            }
            true
        })
        .map(Into::into)
        .collect();

    let template = CatalogGridTemplate { items: filtered };
    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "components/job_wizard_configure.html")]
struct ConfigureStepTemplate {
    catalog_item: JobCatalogItemDisplay,
    parameters: Vec<CatalogParameterDisplay>,
}

/// Configure step - show parameter form
async fn configure_step(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;
    let item = queries::job_catalog::get_catalog_item(db.pool(), catalog_id).await?;
    let display: JobCatalogItemDisplay = item.into();
    let parameters = display.parameters.clone();

    let template = ConfigureStepTemplate {
        catalog_item: display,
        parameters,
    };

    Ok(Html(template.render()?))
}

#[derive(Template)]
#[template(path = "components/job_wizard_servers.html")]
struct ServersStepTemplate {
    catalog_item: JobCatalogItemDisplay,
    compatible_servers: Vec<ServerDisplay>,
    incompatible_servers: Vec<ServerDisplay>,
}

/// Servers step - show compatible servers
async fn servers_step(
    State(state): State<AppState>,
    Path(catalog_id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let db = state.db().await;

    let item = queries::job_catalog::get_catalog_item(db.pool(), catalog_id).await?;
    let required_caps = item.get_required_capabilities();

    let all_servers = queries::servers::list_servers_with_details(db.pool()).await?;

    let (compatible, incompatible): (Vec<_>, Vec<_>) = all_servers
        .into_iter()
        .partition(|server| {
            // Check if server has all required capabilities
            required_caps.iter().all(|cap| {
                server.capabilities.iter().any(|sc|
                    sc.capability == *cap && sc.available
                )
            })
        });

    let template = ServersStepTemplate {
        catalog_item: item.into(),
        compatible_servers: compatible.into_iter().map(Into::into).collect(),
        incompatible_servers: incompatible.into_iter().map(Into::into).collect(),
    };

    Ok(Html(template.render()?))
}
```

### 2.2 Wizard Templates

**File**: `server/templates/pages/job_wizard.html`

```html
{% extends "base.html" %}

{% block title %}Create Job - {{ settings.app_name }}{% endblock %}

{% block content %}
<div class="container" x-data="jobWizard()">
    <!-- Wizard Header -->
    <div class="wizard-header mb-4">
        <h1><i data-lucide="zap"></i> Quick Job Setup</h1>
        <p class="text-secondary">Create and schedule a job in just a few steps</p>
    </div>

    <!-- Step Indicators -->
    <div class="wizard-steps mb-4">
        <div class="step" :class="{ active: step >= 1, completed: step > 1 }">
            <span class="step-number">1</span>
            <span class="step-label">Select Job</span>
        </div>
        <div class="step-connector" :class="{ active: step > 1 }"></div>
        <div class="step" :class="{ active: step >= 2, completed: step > 2 }">
            <span class="step-number">2</span>
            <span class="step-label">Configure</span>
        </div>
        <div class="step-connector" :class="{ active: step > 2 }"></div>
        <div class="step" :class="{ active: step >= 3, completed: step > 3 }">
            <span class="step-number">3</span>
            <span class="step-label">Server</span>
        </div>
        <div class="step-connector" :class="{ active: step > 3 }"></div>
        <div class="step" :class="{ active: step >= 4, completed: step > 4 }">
            <span class="step-number">4</span>
            <span class="step-label">Schedule</span>
        </div>
        <div class="step-connector" :class="{ active: step > 4 }"></div>
        <div class="step" :class="{ active: step >= 5 }">
            <span class="step-number">5</span>
            <span class="step-label">Notify</span>
        </div>
    </div>

    <!-- Step Content -->
    <div class="wizard-content">
        <!-- Step 1: Select Job -->
        <div x-show="step === 1" x-transition>
            <div class="card">
                <div class="card-header flex-between">
                    <h3>What would you like to do?</h3>
                    <div class="flex gap-2">
                        <input type="text"
                               placeholder="Search jobs..."
                               class="form-control"
                               style="width: 200px;"
                               x-model="searchQuery"
                               @input.debounce.300ms="filterCatalog()">
                        <select class="form-select" style="width: 150px;" x-model="difficultyFilter" @change="filterCatalog()">
                            <option value="all">All Levels</option>
                            <option value="basic">Basic</option>
                            <option value="intermediate">Intermediate</option>
                            <option value="advanced">Advanced</option>
                        </select>
                    </div>
                </div>

                <!-- Favorites (if any) -->
                {% if !favorites.is_empty() %}
                <div class="mb-4">
                    <h4 class="text-secondary mb-2"><i data-lucide="star" style="width: 16px;"></i> Favorites</h4>
                    <div class="job-grid">
                        {% for item in favorites %}
                        <div class="job-card"
                             @click="selectJob({{ item.id }})"
                             :class="{ selected: selectedJobId === {{ item.id }} }">
                            <div class="job-card-icon">
                                <i data-lucide="{{ item.icon }}"></i>
                            </div>
                            <div class="job-card-content">
                                <h4>{{ item.display_name }}</h4>
                                <p>{{ item.description }}</p>
                            </div>
                            <span class="badge {{ item.difficulty_badge_class }}">{{ item.difficulty }}</span>
                        </div>
                        {% endfor %}
                    </div>
                </div>
                {% endif %}

                <!-- Categories -->
                <div class="category-tabs mb-3">
                    <button class="tab-btn" :class="{ active: selectedCategory === 'all' }" @click="selectCategory('all')">
                        All Jobs
                    </button>
                    {% for cat in categories %}
                    <button class="tab-btn" :class="{ active: selectedCategory === '{{ cat.name }}' }" @click="selectCategory('{{ cat.name }}')">
                        <i data-lucide="{{ cat.icon }}" style="width: 14px;"></i>
                        {{ cat.display_name }}
                        <span class="badge badge-secondary">{{ cat.item_count }}</span>
                    </button>
                    {% endfor %}
                </div>

                <!-- Job Grid (HTMX loaded) -->
                <div id="catalog-grid"
                     hx-get="/jobs/wizard/catalog"
                     hx-trigger="load"
                     hx-swap="innerHTML">
                    <div class="loading-spinner">Loading jobs...</div>
                </div>
            </div>

            <div class="wizard-actions mt-4">
                <a href="/job-templates" class="btn btn-secondary">
                    <i data-lucide="settings"></i> Advanced Mode
                </a>
                <button class="btn btn-primary"
                        :disabled="!selectedJobId"
                        @click="nextStep()">
                    Next: Configure <i data-lucide="arrow-right"></i>
                </button>
            </div>
        </div>

        <!-- Step 2: Configure (loaded via HTMX) -->
        <div x-show="step === 2" x-transition>
            <div id="configure-step"
                 :hx-get="'/jobs/wizard/configure/' + selectedJobId"
                 hx-trigger="revealed"
                 hx-swap="innerHTML">
            </div>

            <div class="wizard-actions mt-4">
                <button class="btn btn-secondary" @click="prevStep()">
                    <i data-lucide="arrow-left"></i> Back
                </button>
                <button class="btn btn-primary" @click="nextStep()">
                    Next: Select Server <i data-lucide="arrow-right"></i>
                </button>
            </div>
        </div>

        <!-- Step 3: Select Server -->
        <div x-show="step === 3" x-transition>
            <div id="servers-step"
                 :hx-get="'/jobs/wizard/servers/' + selectedJobId"
                 hx-trigger="revealed"
                 hx-swap="innerHTML">
            </div>

            <div class="wizard-actions mt-4">
                <button class="btn btn-secondary" @click="prevStep()">
                    <i data-lucide="arrow-left"></i> Back
                </button>
                <button class="btn btn-primary"
                        :disabled="!selectedServerId"
                        @click="nextStep()">
                    Next: Schedule <i data-lucide="arrow-right"></i>
                </button>
            </div>
        </div>

        <!-- Step 4: Schedule -->
        <div x-show="step === 4" x-transition>
            <div class="card">
                <h3>When should this run?</h3>

                <div class="schedule-options mt-4">
                    <label class="radio-card" :class="{ selected: scheduleType === 'now' }">
                        <input type="radio" name="schedule_type" value="now" x-model="scheduleType">
                        <div class="radio-card-content">
                            <i data-lucide="zap"></i>
                            <strong>Run Once Now</strong>
                            <p>Execute immediately</p>
                        </div>
                    </label>

                    <label class="radio-card" :class="{ selected: scheduleType === 'scheduled' }">
                        <input type="radio" name="schedule_type" value="scheduled" x-model="scheduleType">
                        <div class="radio-card-content">
                            <i data-lucide="calendar"></i>
                            <strong>Run on Schedule</strong>
                            <p>Recurring execution</p>
                        </div>
                    </label>
                </div>

                <!-- Schedule Builder (shown when scheduled) -->
                <div x-show="scheduleType === 'scheduled'" x-transition class="mt-4">
                    <div class="schedule-builder">
                        <div class="form-row">
                            <div class="form-group">
                                <label>Frequency</label>
                                <select class="form-select" x-model="scheduleFreq" @change="updateCron()">
                                    <option value="hourly">Every Hour</option>
                                    <option value="daily">Daily</option>
                                    <option value="weekly">Weekly</option>
                                    <option value="monthly">Monthly</option>
                                    <option value="custom">Custom Cron</option>
                                </select>
                            </div>

                            <div class="form-group" x-show="scheduleFreq === 'daily' || scheduleFreq === 'weekly' || scheduleFreq === 'monthly'">
                                <label>Time</label>
                                <input type="time" class="form-control" x-model="scheduleTime" @change="updateCron()">
                            </div>

                            <div class="form-group" x-show="scheduleFreq === 'weekly'">
                                <label>Day of Week</label>
                                <select class="form-select" x-model="scheduleDow" @change="updateCron()">
                                    <option value="0">Sunday</option>
                                    <option value="1">Monday</option>
                                    <option value="2">Tuesday</option>
                                    <option value="3">Wednesday</option>
                                    <option value="4">Thursday</option>
                                    <option value="5">Friday</option>
                                    <option value="6">Saturday</option>
                                </select>
                            </div>
                        </div>

                        <div class="form-group" x-show="scheduleFreq === 'custom'">
                            <label>Cron Expression</label>
                            <input type="text" class="form-control" x-model="cronExpression" placeholder="0 * * * *">
                            <small class="text-secondary">Format: minute hour day month weekday</small>
                        </div>

                        <div class="cron-preview mt-3 p-3" style="background: var(--bg-secondary); border-radius: 4px;">
                            <strong>Cron:</strong> <code x-text="cronExpression"></code>
                            <div class="text-secondary mt-1" x-text="cronDescription"></div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="wizard-actions mt-4">
                <button class="btn btn-secondary" @click="prevStep()">
                    <i data-lucide="arrow-left"></i> Back
                </button>
                <button class="btn btn-primary" @click="nextStep()">
                    Next: Notifications <i data-lucide="arrow-right"></i>
                </button>
            </div>
        </div>

        <!-- Step 5: Notifications -->
        <div x-show="step === 5" x-transition>
            <div class="card">
                <h3>Notify me when...</h3>

                <div class="notification-options mt-4">
                    <label class="checkbox-card">
                        <input type="checkbox" x-model="notifyOnFailure">
                        <div class="checkbox-card-content">
                            <i data-lucide="x-circle" class="text-danger"></i>
                            <strong>On Failure</strong>
                            <p>Get notified when the job fails</p>
                        </div>
                    </label>

                    <label class="checkbox-card">
                        <input type="checkbox" x-model="notifyOnSuccess">
                        <div class="checkbox-card-content">
                            <i data-lucide="check-circle" class="text-success"></i>
                            <strong>On Success</strong>
                            <p>Get notified when the job succeeds</p>
                        </div>
                    </label>
                </div>

                <div x-show="notifyOnFailure || notifyOnSuccess" class="mt-4">
                    <div class="form-group">
                        <label>Send notifications to:</label>
                        <select class="form-select" x-model="notificationChannel">
                            <option value="">Select a channel...</option>
                            <!-- Populated from channels -->
                        </select>
                    </div>

                    <div class="notification-preview card mt-3" x-show="notificationChannel">
                        <h4>Preview</h4>
                        <div id="notification-preview-content"
                             hx-post="/notifications/preview"
                             hx-trigger="change from:select[x-model='notificationChannel']"
                             hx-include="[name^='notify']">
                            <!-- Preview rendered here -->
                        </div>
                    </div>
                </div>
            </div>

            <div class="wizard-actions mt-4">
                <button class="btn btn-secondary" @click="prevStep()">
                    <i data-lucide="arrow-left"></i> Back
                </button>
                <button class="btn btn-success" @click="createJob()">
                    <i data-lucide="check"></i> Create Job
                </button>
            </div>
        </div>
    </div>
</div>

<script>
function jobWizard() {
    return {
        step: 1,
        selectedJobId: null,
        selectedServerId: null,
        selectedCategory: 'all',
        searchQuery: '',
        difficultyFilter: 'all',

        // Schedule
        scheduleType: 'now',
        scheduleFreq: 'daily',
        scheduleTime: '02:00',
        scheduleDow: '0',
        cronExpression: '0 2 * * *',
        cronDescription: 'Every day at 2:00 AM',

        // Notifications
        notifyOnFailure: true,
        notifyOnSuccess: false,
        notificationChannel: '',

        // Parameter values (populated dynamically)
        parameters: {},

        selectJob(id) {
            this.selectedJobId = id;
        },

        selectCategory(category) {
            this.selectedCategory = category;
            htmx.ajax('GET', `/jobs/wizard/catalog/${category}`, {target: '#catalog-grid'});
        },

        filterCatalog() {
            const params = new URLSearchParams();
            if (this.searchQuery) params.set('search', this.searchQuery);
            if (this.difficultyFilter !== 'all') params.set('difficulty', this.difficultyFilter);
            htmx.ajax('GET', `/jobs/wizard/catalog?${params}`, {target: '#catalog-grid'});
        },

        nextStep() {
            if (this.step < 5) {
                this.step++;
                // Trigger HTMX load for next step
                this.$nextTick(() => {
                    htmx.process(document.body);
                });
            }
        },

        prevStep() {
            if (this.step > 1) this.step--;
        },

        updateCron() {
            const [hours, minutes] = this.scheduleTime.split(':');
            switch(this.scheduleFreq) {
                case 'hourly':
                    this.cronExpression = '0 * * * *';
                    this.cronDescription = 'Every hour at minute 0';
                    break;
                case 'daily':
                    this.cronExpression = `${minutes} ${hours} * * *`;
                    this.cronDescription = `Every day at ${this.scheduleTime}`;
                    break;
                case 'weekly':
                    const days = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'];
                    this.cronExpression = `${minutes} ${hours} * * ${this.scheduleDow}`;
                    this.cronDescription = `Every ${days[this.scheduleDow]} at ${this.scheduleTime}`;
                    break;
                case 'monthly':
                    this.cronExpression = `${minutes} ${hours} 1 * *`;
                    this.cronDescription = `1st of every month at ${this.scheduleTime}`;
                    break;
            }
        },

        async createJob() {
            const payload = {
                catalog_item_id: this.selectedJobId,
                server_id: this.selectedServerId,
                parameters: this.parameters,
                schedule_type: this.scheduleType,
                cron_expression: this.scheduleType === 'scheduled' ? this.cronExpression : null,
                notify_on_success: this.notifyOnSuccess,
                notify_on_failure: this.notifyOnFailure,
                notification_channel_id: this.notificationChannel || null,
            };

            try {
                const response = await fetch('/jobs/wizard/create', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(payload),
                });

                if (response.ok) {
                    const result = await response.json();
                    window.location.href = result.redirect_url;
                } else {
                    alert('Failed to create job');
                }
            } catch (e) {
                console.error('Error creating job:', e);
                alert('Error creating job');
            }
        }
    }
}
</script>
{% endblock %}
```

---

## Phase 3: Notification Enhancements

### 3.1 Enhanced Template Variables

**File**: `core/src/notifications.rs` (additions)

```rust
use std::collections::HashMap;
use chrono::{DateTime, Local, Utc};

/// All available template variables for notifications
pub struct NotificationContext {
    // Job info
    pub job_name: String,
    pub job_display_name: String,
    pub job_type: String,

    // Server info
    pub server_name: String,
    pub server_hostname: String,

    // Execution info
    pub status: String,
    pub status_emoji: String,      // For ntfy: checkmark or X
    pub exit_code: Option<i32>,
    pub duration_seconds: i64,
    pub duration_human: String,    // "2m 34s"

    // Output
    pub output: Option<String>,
    pub output_snippet: Option<String>,  // First N lines
    pub output_tail: Option<String>,     // Last N lines
    pub error: Option<String>,
    pub error_summary: Option<String>,   // Extracted error lines

    // Multi-server (future)
    pub server_count: i64,
    pub success_count: i64,
    pub failure_count: i64,

    // Trigger info
    pub triggered_by: String,      // "schedule", "manual", "webhook"
    pub schedule_name: Option<String>,
    pub run_id: i64,
    pub run_url: String,           // Direct link to job run

    // Timestamps
    pub started_at: String,
    pub finished_at: String,
}

impl NotificationContext {
    /// Create from job run data
    pub fn from_job_run(
        run: &JobRun,
        schedule: &JobSchedule,
        template: &JobTemplate,
        server: &Server,
        base_url: &str,
    ) -> Self {
        let duration = run.duration_ms.unwrap_or(0);
        let duration_human = format_duration(duration);

        let output = run.output.clone();
        let output_snippet = output.as_ref().map(|o| {
            o.lines().take(10).collect::<Vec<_>>().join("\n")
        });
        let output_tail = output.as_ref().map(|o| {
            let lines: Vec<_> = o.lines().collect();
            lines.iter().rev().take(10).rev().cloned().collect::<Vec<_>>().join("\n")
        });

        let error_summary = run.error.as_ref().map(|e| {
            // Extract lines containing "error", "fail", "exception"
            e.lines()
                .filter(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("error") || lower.contains("fail") || lower.contains("exception")
                })
                .take(5)
                .collect::<Vec<_>>()
                .join("\n")
        });

        let status_emoji = match run.status.as_str() {
            "success" => "white_check_mark",
            "failure" => "x",
            "timeout" => "hourglass",
            "cancelled" => "stop_sign",
            _ => "question",
        };

        Self {
            job_name: template.name.clone(),
            job_display_name: template.display_name.clone(),
            job_type: template.job_type_id.to_string(), // TODO: resolve name
            server_name: server.name.clone(),
            server_hostname: server.hostname.clone().unwrap_or_default(),
            status: run.status.clone(),
            status_emoji: status_emoji.to_string(),
            exit_code: run.exit_code,
            duration_seconds: duration / 1000,
            duration_human,
            output,
            output_snippet,
            output_tail,
            error: run.error.clone(),
            error_summary,
            server_count: 1,
            success_count: if run.status == "success" { 1 } else { 0 },
            failure_count: if run.status == "failure" { 1 } else { 0 },
            triggered_by: "schedule".to_string(),
            schedule_name: Some(schedule.name.clone()),
            run_id: run.id,
            run_url: format!("{}/job-runs/{}", base_url, run.id),
            started_at: run.started_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string(),
            finished_at: run.finished_at.map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default(),
        }
    }

    /// Render a template string with this context
    pub fn render_template(&self, template: &str) -> String {
        let mut result = template.to_string();

        // Simple {{variable}} replacement
        let replacements = [
            ("{{job_name}}", &self.job_name),
            ("{{job_display_name}}", &self.job_display_name),
            ("{{server_name}}", &self.server_name),
            ("{{server_hostname}}", &self.server_hostname),
            ("{{status}}", &self.status),
            ("{{status_emoji}}", &self.status_emoji),
            ("{{duration_human}}", &self.duration_human),
            ("{{triggered_by}}", &self.triggered_by),
            ("{{run_url}}", &self.run_url),
            ("{{started_at}}", &self.started_at),
            ("{{finished_at}}", &self.finished_at),
        ];

        for (placeholder, value) in replacements {
            result = result.replace(placeholder, value);
        }

        // Optional fields
        if let Some(ref exit_code) = self.exit_code {
            result = result.replace("{{exit_code}}", &exit_code.to_string());
        }
        if let Some(ref output) = self.output_snippet {
            result = result.replace("{{output_snippet}}", output);
        }
        if let Some(ref error) = self.error_summary {
            result = result.replace("{{error_summary}}", error);
        }

        // Duration seconds
        result = result.replace("{{duration_seconds}}", &self.duration_seconds.to_string());

        result
    }
}

fn format_duration(ms: i64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}
```

### 3.2 ntfy.sh Enhanced Integration

```rust
/// Send notification via ntfy.sh with enhanced features
pub async fn send_ntfy_notification(
    channel: &NotificationChannel,
    context: &NotificationContext,
    policy: &NotificationPolicy,
) -> Result<(), NotificationError> {
    let config: NtfyConfig = serde_json::from_value(channel.config.clone())?;

    // Choose template based on status
    let (title_template, body_template, tags) = if context.status == "success" {
        (
            policy.success_title_template.as_ref().unwrap_or(&default_success_title()),
            policy.success_body_template.as_ref().unwrap_or(&default_success_body()),
            policy.ntfy_success_tags.as_ref(),
        )
    } else {
        (
            policy.failure_title_template.as_ref().unwrap_or(&default_failure_title()),
            policy.failure_body_template.as_ref().unwrap_or(&default_failure_body()),
            policy.ntfy_failure_tags.as_ref(),
        )
    };

    let title = context.render_template(title_template);
    let body = context.render_template(body_template);

    // Build ntfy tags
    let mut ntfy_tags: Vec<String> = vec![];
    if let Some(tags_json) = tags {
        if let Some(arr) = tags_json.as_array() {
            ntfy_tags.extend(arr.iter().filter_map(|v| v.as_str().map(String::from)));
        }
    }
    // Add status-based default tags if none specified
    if ntfy_tags.is_empty() {
        ntfy_tags.push(context.status_emoji.clone());
    }

    // Build request
    let client = reqwest::Client::new();
    let url = format!("{}/{}", config.url, config.topic);

    let mut request = client
        .post(&url)
        .header("Title", &title)
        .header("Tags", ntfy_tags.join(","))
        .header("Priority", channel.default_priority.to_string())
        .body(body);

    // Add click action to open job run
    request = request.header("Click", &context.run_url);

    // Add authentication if configured
    if let Some(ref token) = config.token {
        request = request.header("Authorization", format!("Bearer {}", token));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        return Err(NotificationError::SendFailed(
            response.text().await.unwrap_or_default()
        ));
    }

    Ok(())
}

fn default_success_title() -> String {
    "{{job_display_name}} completed on {{server_name}}".to_string()
}

fn default_success_body() -> String {
    "Duration: {{duration_human}}\nTriggered by: {{triggered_by}}".to_string()
}

fn default_failure_title() -> String {
    "{{job_display_name}} FAILED on {{server_name}}".to_string()
}

fn default_failure_body() -> String {
    "Exit code: {{exit_code}}\nDuration: {{duration_human}}\n\nError:\n{{error_summary}}".to_string()
}
```

---

## Phase 4: Dashboard Quick Actions

### 4.1 Quick Actions Component

**File**: `server/templates/components/quick_actions.html`

```html
<div class="quick-actions card">
    <div class="card-header flex-between">
        <h3><i data-lucide="zap"></i> Quick Actions</h3>
        <a href="/jobs/wizard" class="btn btn-sm btn-primary">
            <i data-lucide="plus"></i> New Job
        </a>
    </div>

    <div class="quick-actions-grid">
        <!-- Favorites -->
        {% for item in favorites %}
        <button class="quick-action-btn"
                hx-post="/jobs/quick-run/{{ item.catalog_item_id }}"
                hx-confirm="Run '{{ item.display_name }}' now?"
                hx-swap="none">
            <i data-lucide="{{ item.icon }}"></i>
            <span>{{ item.display_name }}</span>
        </button>
        {% endfor %}

        <!-- Recent Jobs (Run Again) -->
        {% for run in recent_runs %}
        <button class="quick-action-btn"
                hx-post="/jobs/run-again/{{ run.schedule_id }}"
                hx-confirm="Run '{{ run.job_name }}' on {{ run.server_name }} again?"
                hx-swap="none"
                title="Last run: {{ run.finished_at }}">
            <i data-lucide="repeat"></i>
            <span>{{ run.job_name }}</span>
            <small class="text-secondary">{{ run.server_name }}</small>
        </button>
        {% endfor %}
    </div>
</div>
```

---

## Phase 5: REST API Updates

### 5.1 New API Endpoints

**File**: `server/src/routes/api.rs` (additions)

```rust
// Job Catalog API
.route("/api/v1/catalog", get(list_catalog))
.route("/api/v1/catalog/:id", get(get_catalog_item))
.route("/api/v1/catalog/:id/compatible-servers", get(list_compatible_servers))
.route("/api/v1/catalog/favorites", get(list_favorites))
.route("/api/v1/catalog/favorites/:id", post(toggle_favorite))

// Quick Execution API
.route("/api/v1/jobs/quick-run", post(quick_run_job))
.route("/api/v1/jobs/run-again/:schedule_id", post(run_job_again))

// Wizard API
.route("/api/v1/wizard/create", post(create_job_from_wizard))
.route("/api/v1/wizard/preview", post(preview_job_command))
.route("/api/v1/wizard/validate", post(validate_wizard_input))
```

### 5.2 API Response Types

```rust
#[derive(Serialize)]
pub struct CatalogListResponse {
    pub items: Vec<CatalogItemSummary>,
    pub categories: Vec<CategorySummary>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct CatalogItemSummary {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    pub difficulty: String,
    pub required_capabilities: Vec<String>,
    pub is_favorite: bool,
}

#[derive(Serialize)]
pub struct WizardCreateResponse {
    pub success: bool,
    pub job_template_id: i64,
    pub job_schedule_id: Option<i64>,
    pub job_run_id: Option<i64>,  // If run immediately
    pub redirect_url: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct WizardCreateRequest {
    pub catalog_item_id: i64,
    pub server_id: i64,
    pub parameters: HashMap<String, JsonValue>,
    pub schedule_type: String,  // "now" or "scheduled"
    pub cron_expression: Option<String>,
    pub name_override: Option<String>,
    pub notify_on_success: bool,
    pub notify_on_failure: bool,
    pub notification_channel_id: Option<i64>,
}
```

---

## Implementation Checklist

### Phase 1: Job Catalog System
- [ ] Create migration `015_job_catalog_and_workflow.sql`
- [ ] Add `database/src/models/job_catalog.rs`
- [ ] Add `database/src/queries/job_catalog.rs`
- [ ] Add display models in `server/src/templates.rs`
- [ ] Run migration and seed data
- [ ] Test queries

### Phase 2: Job Wizard UI
- [ ] Create `server/src/routes/ui/job_wizard.rs`
- [ ] Create `server/templates/pages/job_wizard.html`
- [ ] Create `server/templates/components/job_wizard_*.html`
- [ ] Add CSS for wizard in `server/static/css/styles.css`
- [ ] Wire up routes in `server/src/routes.rs`
- [ ] Test full wizard flow

### Phase 3: Notification Enhancements
- [ ] Add new columns to `notification_policies`
- [ ] Extend `core/src/notifications.rs` with new context
- [ ] Add notification preview endpoint
- [ ] Update notification sending logic
- [ ] Test ntfy.sh tags and templates

### Phase 4: Dashboard Quick Actions
- [ ] Add quick actions component
- [ ] Add favorites/run-again endpoints
- [ ] Update dashboard template
- [ ] Test quick execution

### Phase 5: Documentation & Polish
- [ ] Update `docs/JOBS.md` with Basic/Advanced modes
- [ ] Update `docs/NOTIFICATIONS.md` with new variables
- [ ] Add `docs/WIZARD.md` user guide
- [ ] Update `CLAUDE.md` with new patterns

---

## Testing Plan

### Unit Tests
- [ ] Parameter schema parsing
- [ ] Template variable substitution
- [ ] Capability matching
- [ ] Cron expression building

### Integration Tests
- [ ] Wizard flow: catalog -> configure -> server -> schedule -> create
- [ ] Quick run execution
- [ ] Notification sending with new templates
- [ ] Run-again functionality

### Manual Testing
- [ ] Complete wizard flow in browser
- [ ] Verify ntfy.sh notifications display correctly
- [ ] Test with servers of varying capabilities
- [ ] Verify favorites persist correctly

---

## Future Enhancements (Post-MVP)

1. **Visual Workflow Builder** - Drag-and-drop composite job creation
2. **Conditional Execution** - Run steps based on previous step results
3. **Webhook Triggers** - Execute jobs via external webhooks
4. **Job Dependencies** - Define job chains
5. **User-Created Catalog Items** - Allow users to add to catalog
6. **Import/Export** - Share job configurations

---

*Document maintained for Claude Code session continuity. Last updated: 2025-12-06*
