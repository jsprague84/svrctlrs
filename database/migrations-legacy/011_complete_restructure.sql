-- Complete Database Restructure Migration
-- This is a CLEAN SLATE migration (no production data to preserve)
--
-- Drops all existing tables and creates the new unified schema that:
-- - Replaces plugins with job_types (built-in command execution framework)
-- - Replaces tasks with job_schedules and job_runs
-- - Adds credential management for SSH keys, API tokens, etc.
-- - Adds server tagging and capability detection
-- - Adds command templates for reusable commands
-- - Adds notification channels and policies
-- - Supports composite jobs (multi-step workflows)

-- ============================================================================
-- STEP 1: DROP ALL EXISTING TABLES (Clean Slate)
-- ============================================================================

DROP TABLE IF EXISTS metrics;
DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS webhooks;
DROP TABLE IF EXISTS task_history;
DROP TABLE IF EXISTS tasks;
DROP TABLE IF EXISTS plugins;
DROP TABLE IF EXISTS notification_backends;
DROP TABLE IF EXISTS servers;
DROP TABLE IF EXISTS settings;

-- ============================================================================
-- STEP 2: CREATE NEW SCHEMA
-- ============================================================================

-- ----------------------------------------------------------------------------
-- Credentials Table
-- Stores SSH keys, API tokens, database passwords, etc.
-- ----------------------------------------------------------------------------
CREATE TABLE credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    credential_type TEXT NOT NULL CHECK(credential_type IN ('ssh_key', 'api_token', 'password', 'certificate')),
    description TEXT,

    -- Credential data (encrypted in production)
    value TEXT NOT NULL,  -- SSH key path, token value, password, cert path
    username TEXT,        -- For password type
    metadata TEXT,        -- JSON: {"key_passphrase": "...", "port": 2222, etc.}

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_credentials_type ON credentials(credential_type);

-- ----------------------------------------------------------------------------
-- Tags Table
-- For organizing servers (prod, staging, docker-hosts, etc.)
-- ----------------------------------------------------------------------------
CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    color TEXT,           -- Hex color for UI (#5E81AC, #88C0D0, etc.)
    description TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ----------------------------------------------------------------------------
-- Servers Table
-- Represents execution targets (local or remote)
-- ----------------------------------------------------------------------------
CREATE TABLE servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    hostname TEXT,                    -- NULL for local, hostname/IP for remote
    port INTEGER DEFAULT 22,          -- SSH port for remote servers
    username TEXT,                    -- SSH username for remote servers
    credential_id INTEGER,            -- FK to credentials for SSH key
    description TEXT,
    is_local BOOLEAN NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT 1,

    -- Metadata
    os_type TEXT,                     -- Detected: linux, windows, macos
    os_distro TEXT,                   -- Detected: ubuntu, fedora, debian, arch, etc.
    package_manager TEXT,             -- Detected: apt, dnf, pacman, yum, etc.
    docker_available BOOLEAN DEFAULT 0,
    systemd_available BOOLEAN DEFAULT 0,
    metadata TEXT,                    -- JSON: additional detected info

    -- Status tracking
    last_seen_at DATETIME,
    last_error TEXT,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (credential_id) REFERENCES credentials(id) ON DELETE RESTRICT,
    CHECK (is_local = 1 OR (hostname IS NOT NULL AND username IS NOT NULL))
);

CREATE INDEX idx_servers_enabled ON servers(enabled);
CREATE INDEX idx_servers_hostname ON servers(hostname);

-- ----------------------------------------------------------------------------
-- Server Tags (Many-to-Many)
-- ----------------------------------------------------------------------------
CREATE TABLE server_tags (
    server_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (server_id, tag_id),
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE INDEX idx_server_tags_tag ON server_tags(tag_id);

-- ----------------------------------------------------------------------------
-- Server Capabilities
-- Tracks detected capabilities (docker, systemd, specific commands, etc.)
-- ----------------------------------------------------------------------------
CREATE TABLE server_capabilities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    capability TEXT NOT NULL,         -- docker, systemd, apt, dnf, pacman, etc.
    available BOOLEAN NOT NULL,
    version TEXT,                     -- Version if applicable
    detected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    UNIQUE(server_id, capability)
);

CREATE INDEX idx_capabilities_server ON server_capabilities(server_id);
CREATE INDEX idx_capabilities_name ON server_capabilities(capability);

-- ----------------------------------------------------------------------------
-- Job Types
-- Built-in job categories (replaces plugins)
-- ----------------------------------------------------------------------------
CREATE TABLE job_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,        -- docker, os, backup, monitoring, custom
    display_name TEXT NOT NULL,       -- "Docker Operations", "OS Maintenance"
    description TEXT,
    icon TEXT,                        -- Icon name for UI
    color TEXT,                       -- Color for UI
    requires_capabilities TEXT,       -- JSON array: ["docker"] or ["apt"]
    metadata TEXT,                    -- JSON: additional config
    enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ----------------------------------------------------------------------------
-- Command Templates
-- Reusable command patterns with variable substitution
-- ----------------------------------------------------------------------------
CREATE TABLE command_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_type_id INTEGER NOT NULL,
    name TEXT NOT NULL,               -- list_containers, update_packages, etc.
    display_name TEXT NOT NULL,       -- "List Docker Containers"
    description TEXT,

    -- Command definition
    command TEXT NOT NULL,            -- Command with {{variables}}
    required_capabilities TEXT,       -- JSON array: ["docker"] or ["apt"]
    os_filter TEXT,                   -- JSON: {"distro": ["ubuntu", "debian"]}

    -- Execution settings
    timeout_seconds INTEGER DEFAULT 300,
    working_directory TEXT,
    environment TEXT,                 -- JSON: {"VAR": "value"}

    -- Output handling
    output_format TEXT CHECK(output_format IN ('text', 'json', 'table')),
    parse_output BOOLEAN DEFAULT 0,   -- Whether to parse and structure output
    output_parser TEXT,               -- JSON: parser config if parse_output = 1

    -- Notification defaults
    notify_on_success BOOLEAN DEFAULT 0,
    notify_on_failure BOOLEAN DEFAULT 1,

    metadata TEXT,                    -- JSON: additional config
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (job_type_id) REFERENCES job_types(id) ON DELETE CASCADE,
    UNIQUE(job_type_id, name)
);

CREATE INDEX idx_command_templates_job_type ON command_templates(job_type_id);

-- ----------------------------------------------------------------------------
-- Job Templates
-- User-defined reusable jobs (can be simple or composite)
-- ----------------------------------------------------------------------------
CREATE TABLE job_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    description TEXT,
    job_type_id INTEGER NOT NULL,

    -- Template type
    is_composite BOOLEAN DEFAULT 0,   -- If true, uses job_template_steps

    -- Simple job (single command) - only used if is_composite = 0
    command_template_id INTEGER,      -- FK to command_templates
    variables TEXT,                   -- JSON: {"var": "value"} for template substitution

    -- Execution defaults
    timeout_seconds INTEGER DEFAULT 300,
    retry_count INTEGER DEFAULT 0,
    retry_delay_seconds INTEGER DEFAULT 60,

    -- Notification defaults
    notify_on_success BOOLEAN DEFAULT 0,
    notify_on_failure BOOLEAN DEFAULT 1,
    notification_policy_id INTEGER,

    metadata TEXT,                    -- JSON: additional config
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (job_type_id) REFERENCES job_types(id) ON DELETE CASCADE,
    FOREIGN KEY (command_template_id) REFERENCES command_templates(id) ON DELETE RESTRICT,
    FOREIGN KEY (notification_policy_id) REFERENCES notification_policies(id) ON DELETE SET NULL,
    CHECK (is_composite = 1 OR command_template_id IS NOT NULL)
);

CREATE INDEX idx_job_templates_type ON job_templates(job_type_id);

-- ----------------------------------------------------------------------------
-- Job Template Steps
-- For composite jobs (multi-step workflows)
-- ----------------------------------------------------------------------------
CREATE TABLE job_template_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_template_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,      -- Execution order (0, 1, 2, ...)
    name TEXT NOT NULL,               -- Step name for logging
    command_template_id INTEGER NOT NULL,
    variables TEXT,                   -- JSON: step-specific variables

    -- Step control
    continue_on_failure BOOLEAN DEFAULT 0,  -- If false, abort job on step failure
    timeout_seconds INTEGER,          -- Override template timeout

    metadata TEXT,

    FOREIGN KEY (job_template_id) REFERENCES job_templates(id) ON DELETE CASCADE,
    FOREIGN KEY (command_template_id) REFERENCES command_templates(id) ON DELETE RESTRICT,
    UNIQUE(job_template_id, step_order)
);

CREATE INDEX idx_template_steps_template ON job_template_steps(job_template_id);

-- ----------------------------------------------------------------------------
-- Job Schedules
-- Scheduled job instances (replaces tasks table)
-- ----------------------------------------------------------------------------
CREATE TABLE job_schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,

    -- Job definition
    job_template_id INTEGER NOT NULL,
    server_id INTEGER NOT NULL,       -- Which server to run on

    -- Schedule
    schedule TEXT NOT NULL,           -- Cron expression
    enabled BOOLEAN NOT NULL DEFAULT 1,

    -- Overrides for this specific schedule
    timeout_seconds INTEGER,          -- Override template timeout
    retry_count INTEGER,              -- Override template retry
    notify_on_success BOOLEAN,        -- Override template notification
    notify_on_failure BOOLEAN,
    notification_policy_id INTEGER,   -- Override template policy

    -- Tracking
    last_run_at DATETIME,
    last_run_status TEXT CHECK(last_run_status IN ('success', 'failure', 'timeout', 'skipped')),
    next_run_at DATETIME,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,

    metadata TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (job_template_id) REFERENCES job_templates(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    FOREIGN KEY (notification_policy_id) REFERENCES notification_policies(id) ON DELETE SET NULL
);

CREATE INDEX idx_job_schedules_template ON job_schedules(job_template_id);
CREATE INDEX idx_job_schedules_server ON job_schedules(server_id);
CREATE INDEX idx_job_schedules_enabled ON job_schedules(enabled);
CREATE INDEX idx_job_schedules_next_run ON job_schedules(next_run_at);

-- ----------------------------------------------------------------------------
-- Job Runs
-- Execution history for job schedules (replaces task_history)
-- ----------------------------------------------------------------------------
CREATE TABLE job_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_schedule_id INTEGER NOT NULL,
    job_template_id INTEGER NOT NULL,
    server_id INTEGER NOT NULL,

    -- Execution details
    status TEXT NOT NULL CHECK(status IN ('running', 'success', 'failure', 'timeout', 'cancelled')),
    started_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at DATETIME,
    duration_ms INTEGER,

    -- Results (for simple jobs)
    exit_code INTEGER,
    output TEXT,
    error TEXT,

    -- Retry tracking
    retry_attempt INTEGER DEFAULT 0,
    is_retry BOOLEAN DEFAULT 0,

    -- Notification tracking
    notification_sent BOOLEAN DEFAULT 0,
    notification_error TEXT,

    metadata TEXT,                    -- JSON: execution context

    FOREIGN KEY (job_schedule_id) REFERENCES job_schedules(id) ON DELETE CASCADE,
    FOREIGN KEY (job_template_id) REFERENCES job_templates(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX idx_job_runs_schedule ON job_runs(job_schedule_id);
CREATE INDEX idx_job_runs_template ON job_runs(job_template_id);
CREATE INDEX idx_job_runs_server ON job_runs(server_id);
CREATE INDEX idx_job_runs_started ON job_runs(started_at DESC);
CREATE INDEX idx_job_runs_status ON job_runs(status);

-- ----------------------------------------------------------------------------
-- Server Job Results
-- Per-server execution results (for multi-server jobs)
-- Currently unused but reserved for future multi-server support
-- ----------------------------------------------------------------------------
CREATE TABLE server_job_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_run_id INTEGER NOT NULL,
    server_id INTEGER NOT NULL,

    status TEXT NOT NULL CHECK(status IN ('success', 'failure', 'timeout', 'skipped')),
    started_at DATETIME NOT NULL,
    finished_at DATETIME,
    duration_ms INTEGER,

    exit_code INTEGER,
    output TEXT,
    error TEXT,

    metadata TEXT,

    FOREIGN KEY (job_run_id) REFERENCES job_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    UNIQUE(job_run_id, server_id)
);

CREATE INDEX idx_server_results_run ON server_job_results(job_run_id);
CREATE INDEX idx_server_results_server ON server_job_results(server_id);

-- ----------------------------------------------------------------------------
-- Step Execution Results
-- For composite jobs - tracks each step's execution
-- ----------------------------------------------------------------------------
CREATE TABLE step_execution_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_run_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,
    step_name TEXT NOT NULL,
    command_template_id INTEGER NOT NULL,

    status TEXT NOT NULL CHECK(status IN ('running', 'success', 'failure', 'timeout', 'skipped')),
    started_at DATETIME NOT NULL,
    finished_at DATETIME,
    duration_ms INTEGER,

    exit_code INTEGER,
    output TEXT,
    error TEXT,

    metadata TEXT,

    FOREIGN KEY (job_run_id) REFERENCES job_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (command_template_id) REFERENCES command_templates(id) ON DELETE CASCADE,
    UNIQUE(job_run_id, step_order)
);

CREATE INDEX idx_step_results_run ON step_execution_results(job_run_id);

-- ----------------------------------------------------------------------------
-- Notification Channels
-- Replaces notification_backends with more flexibility
-- ----------------------------------------------------------------------------
CREATE TABLE notification_channels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    channel_type TEXT NOT NULL CHECK(channel_type IN ('gotify', 'ntfy', 'email', 'slack', 'discord', 'webhook')),
    description TEXT,

    -- Configuration (encrypted in production)
    config TEXT NOT NULL,             -- JSON: type-specific config
                                      -- Gotify: {"url": "...", "token": "..."}
                                      -- ntfy: {"url": "...", "topic": "...", "token": "..."}
                                      -- email: {"smtp_host": "...", "from": "...", "to": "..."}

    -- Settings
    enabled BOOLEAN NOT NULL DEFAULT 1,
    default_priority INTEGER DEFAULT 3,

    -- Testing/validation
    last_test_at DATETIME,
    last_test_success BOOLEAN,
    last_test_error TEXT,

    metadata TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_notification_channels_type ON notification_channels(channel_type);
CREATE INDEX idx_notification_channels_enabled ON notification_channels(enabled);

-- ----------------------------------------------------------------------------
-- Notification Policies
-- Defines when/how to send notifications
-- ----------------------------------------------------------------------------
CREATE TABLE notification_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    description TEXT,

    -- Trigger conditions
    on_success BOOLEAN DEFAULT 0,
    on_failure BOOLEAN DEFAULT 1,
    on_timeout BOOLEAN DEFAULT 1,

    -- Filtering
    job_type_filter TEXT,            -- JSON array: ["docker", "os"]
    server_filter TEXT,              -- JSON array: [1, 2, 3] (server IDs)
    tag_filter TEXT,                 -- JSON array: ["prod", "critical"]

    -- Throttling
    min_severity INTEGER DEFAULT 1,  -- 1-5, only notify if severity >= this
    max_per_hour INTEGER,            -- Rate limiting

    -- Message customization
    title_template TEXT,             -- "{{job_name}} {{status}} on {{server_name}}"
    body_template TEXT,

    enabled BOOLEAN NOT NULL DEFAULT 1,
    metadata TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ----------------------------------------------------------------------------
-- Notification Policy Channels (Many-to-Many)
-- Links policies to one or more channels
-- ----------------------------------------------------------------------------
CREATE TABLE notification_policy_channels (
    policy_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    priority_override INTEGER,       -- Override channel default priority
    PRIMARY KEY (policy_id, channel_id),
    FOREIGN KEY (policy_id) REFERENCES notification_policies(id) ON DELETE CASCADE,
    FOREIGN KEY (channel_id) REFERENCES notification_channels(id) ON DELETE CASCADE
);

CREATE INDEX idx_policy_channels_channel ON notification_policy_channels(channel_id);

-- ----------------------------------------------------------------------------
-- Notification Log
-- Audit trail of sent notifications
-- ----------------------------------------------------------------------------
CREATE TABLE notification_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id INTEGER NOT NULL,
    policy_id INTEGER,
    job_run_id INTEGER,              -- Which job triggered this

    -- Message details
    title TEXT NOT NULL,
    body TEXT,
    priority INTEGER DEFAULT 3,

    -- Delivery status
    success BOOLEAN NOT NULL,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0,

    sent_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (channel_id) REFERENCES notification_channels(id) ON DELETE CASCADE,
    FOREIGN KEY (policy_id) REFERENCES notification_policies(id) ON DELETE SET NULL,
    FOREIGN KEY (job_run_id) REFERENCES job_runs(id) ON DELETE SET NULL
);

CREATE INDEX idx_notification_log_channel ON notification_log(channel_id);
CREATE INDEX idx_notification_log_sent_at ON notification_log(sent_at DESC);
CREATE INDEX idx_notification_log_job_run ON notification_log(job_run_id);

-- ----------------------------------------------------------------------------
-- Settings
-- Application-wide configuration
-- ----------------------------------------------------------------------------
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    description TEXT,
    value_type TEXT NOT NULL CHECK(value_type IN ('string', 'integer', 'boolean', 'json')),
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- STEP 3: SEED DEFAULT SETTINGS
-- ============================================================================

INSERT INTO settings (key, value, description, value_type) VALUES
    ('app.name', 'SvrCtlRS', 'Application name', 'string'),
    ('app.version', 'v2.2.0', 'Application version', 'string'),
    ('scheduler.enabled', 'true', 'Enable job scheduler', 'boolean'),
    ('scheduler.check_interval_seconds', '30', 'How often to check for scheduled jobs', 'integer'),
    ('notifications.enabled', 'true', 'Enable notifications globally', 'boolean'),
    ('ui.theme', 'nord', 'UI theme (nord, dark, light)', 'string'),
    ('ui.items_per_page', '25', 'Items per page in tables', 'integer'),
    ('jobs.default_timeout_seconds', '300', 'Default job timeout', 'integer'),
    ('jobs.max_concurrent', '5', 'Maximum concurrent job executions', 'integer'),
    ('jobs.retention_days', '30', 'Days to keep job run history', 'integer'),
    ('ssh.connection_timeout_seconds', '10', 'SSH connection timeout', 'integer'),
    ('ssh.command_timeout_seconds', '300', 'SSH command timeout', 'integer');

-- ============================================================================
-- STEP 4: SEED DEFAULT TAGS
-- ============================================================================

INSERT INTO tags (name, color, description) VALUES
    ('prod', '#BF616A', 'Production servers'),
    ('staging', '#D08770', 'Staging/testing servers'),
    ('dev', '#EBCB8B', 'Development servers'),
    ('docker-hosts', '#88C0D0', 'Servers running Docker'),
    ('critical', '#B48EAD', 'Mission-critical systems');

-- ============================================================================
-- STEP 5: SEED BUILT-IN JOB TYPES
-- ============================================================================

INSERT INTO job_types (name, display_name, description, icon, color, requires_capabilities) VALUES
    ('docker', 'Docker Operations', 'Docker container and image management', 'docker', '#2496ED', '["docker"]'),
    ('os', 'OS Maintenance', 'Operating system updates and maintenance', 'system', '#5E81AC', '[]'),
    ('monitoring', 'System Monitoring', 'System health and performance monitoring', 'chart', '#88C0D0', '[]'),
    ('backup', 'Backup & Restore', 'Data backup and recovery operations', 'archive', '#A3BE8C', '[]'),
    ('custom', 'Custom Commands', 'User-defined custom commands', 'terminal', '#8FBCBB', '[]');

-- ============================================================================
-- STEP 6: SEED COMMAND TEMPLATES
-- ============================================================================

-- Docker command templates
INSERT INTO command_templates (job_type_id, name, display_name, description, command, required_capabilities, timeout_seconds) VALUES
    (1, 'list_containers', 'List Docker Containers', 'List all running Docker containers',
     'docker ps --format "table {{.ID}}\t{{.Image}}\t{{.Status}}\t{{.Names}}"', '["docker"]', 30),

    (1, 'list_all_containers', 'List All Containers', 'List all Docker containers (including stopped)',
     'docker ps -a --format "table {{.ID}}\t{{.Image}}\t{{.Status}}\t{{.Names}}"', '["docker"]', 30),

    (1, 'container_stats', 'Container Resource Stats', 'Show resource usage statistics for all containers',
     'docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"', '["docker"]', 30),

    (1, 'prune_system', 'Docker System Prune', 'Remove unused containers, networks, images (dangling), and build cache',
     'docker system prune -f', '["docker"]', 300),

    (1, 'list_images', 'List Docker Images', 'List all Docker images',
     'docker images --format "table {{.Repository}}\t{{.Tag}}\t{{.Size}}\t{{.CreatedSince}}"', '["docker"]', 30);

-- OS Maintenance - APT (Debian/Ubuntu)
INSERT INTO command_templates (job_type_id, name, display_name, description, command, required_capabilities, os_filter, timeout_seconds, notify_on_failure) VALUES
    (2, 'apt_update', 'APT: Update Package Lists', 'Update APT package lists (Debian/Ubuntu)',
     'sudo apt-get update', '["apt"]', '{"distro": ["debian", "ubuntu"]}', 300, 1),

    (2, 'apt_upgrade', 'APT: Full System Upgrade', 'Perform full system upgrade with APT (Debian/Ubuntu)',
     'sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y', '["apt"]', '{"distro": ["debian", "ubuntu"]}', 1800, 1),

    (2, 'apt_dist_upgrade', 'APT: Distribution Upgrade', 'Perform distribution upgrade with APT (Debian/Ubuntu)',
     'sudo DEBIAN_FRONTEND=noninteractive apt-get dist-upgrade -y', '["apt"]', '{"distro": ["debian", "ubuntu"]}', 1800, 1),

    (2, 'apt_security_updates', 'APT: Security Updates Only', 'Install only security updates (Debian/Ubuntu)',
     'sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y -o Dir::Etc::SourceList=/etc/apt/sources.list.d/security.list',
     '["apt"]', '{"distro": ["debian", "ubuntu"]}', 1800, 1),

    (2, 'apt_autoremove', 'APT: Auto Remove', 'Remove unnecessary packages (Debian/Ubuntu)',
     'sudo apt-get autoremove -y', '["apt"]', '{"distro": ["debian", "ubuntu"]}', 300, 0),

    (2, 'apt_clean', 'APT: Clean Cache', 'Clean APT package cache (Debian/Ubuntu)',
     'sudo apt-get clean', '["apt"]', '{"distro": ["debian", "ubuntu"]}', 60, 0);

-- OS Maintenance - DNF (Fedora/RHEL)
INSERT INTO command_templates (job_type_id, name, display_name, description, command, required_capabilities, os_filter, timeout_seconds, notify_on_failure) VALUES
    (2, 'dnf_check_update', 'DNF: Check for Updates', 'Check for available updates with DNF (Fedora/RHEL)',
     'sudo dnf check-update', '["dnf"]', '{"distro": ["fedora", "rhel", "centos"]}', 300, 0),

    (2, 'dnf_upgrade', 'DNF: Full System Upgrade', 'Perform full system upgrade with DNF (Fedora/RHEL)',
     'sudo dnf upgrade -y', '["dnf"]', '{"distro": ["fedora", "rhel", "centos"]}', 1800, 1),

    (2, 'dnf_security_updates', 'DNF: Security Updates Only', 'Install only security updates (Fedora/RHEL)',
     'sudo dnf upgrade-minimal --security -y', '["dnf"]', '{"distro": ["fedora", "rhel", "centos"]}', 1800, 1),

    (2, 'dnf_autoremove', 'DNF: Auto Remove', 'Remove unnecessary packages (Fedora/RHEL)',
     'sudo dnf autoremove -y', '["dnf"]', '{"distro": ["fedora", "rhel", "centos"]}', 300, 0),

    (2, 'dnf_clean', 'DNF: Clean Cache', 'Clean DNF package cache (Fedora/RHEL)',
     'sudo dnf clean all', '["dnf"]', '{"distro": ["fedora", "rhel", "centos"]}', 60, 0);

-- OS Maintenance - Pacman (Arch)
INSERT INTO command_templates (job_type_id, name, display_name, description, command, required_capabilities, os_filter, timeout_seconds, notify_on_failure) VALUES
    (2, 'pacman_sync', 'Pacman: Sync Databases', 'Synchronize package databases (Arch Linux)',
     'sudo pacman -Sy', '["pacman"]', '{"distro": ["arch", "manjaro"]}', 300, 1),

    (2, 'pacman_upgrade', 'Pacman: Full System Upgrade', 'Perform full system upgrade (Arch Linux)',
     'sudo pacman -Syu --noconfirm', '["pacman"]', '{"distro": ["arch", "manjaro"]}', 1800, 1),

    (2, 'pacman_clean', 'Pacman: Clean Cache', 'Clean package cache (Arch Linux)',
     'sudo pacman -Sc --noconfirm', '["pacman"]', '{"distro": ["arch", "manjaro"]}', 60, 0);

-- OS Monitoring (distro-agnostic)
INSERT INTO command_templates (job_type_id, name, display_name, description, command, required_capabilities, timeout_seconds, notify_on_success) VALUES
    (2, 'disk_usage', 'Disk Usage Report', 'Show disk usage for all mounted filesystems',
     'df -h', '[]', 30, 0),

    (2, 'memory_usage', 'Memory Usage', 'Display memory usage statistics',
     'free -h', '[]', 30, 0),

    (2, 'system_load', 'System Load Average', 'Show system load averages and uptime',
     'uptime', '[]', 30, 0),

    (2, 'top_processes', 'Top Processes', 'Show top 10 processes by CPU usage',
     'ps aux --sort=-%cpu | head -n 11', '[]', 30, 0);

-- ============================================================================
-- STEP 7: SEED DEFAULT NOTIFICATION CHANNELS (Disabled, Requires User Config)
-- ============================================================================

INSERT INTO notification_channels (name, channel_type, description, config, enabled, default_priority) VALUES
    ('gotify-default', 'gotify', 'Default Gotify server (requires configuration)',
     '{"url": "http://localhost:8080", "token": "CHANGE_ME"}', 0, 5),

    ('ntfy-default', 'ntfy', 'Default ntfy.sh topic (requires configuration)',
     '{"url": "https://ntfy.sh", "topic": "svrctlrs-notifications", "token": null}', 0, 3);

-- ============================================================================
-- STEP 8: CREATE DEFAULT NOTIFICATION POLICY
-- ============================================================================

INSERT INTO notification_policies (name, description, on_success, on_failure, on_timeout, enabled) VALUES
    ('default-failures', 'Notify on all failures and timeouts', 0, 1, 1, 1);

-- Link default policy to channels (will be inactive until channels are configured)
INSERT INTO notification_policy_channels (policy_id, channel_id) VALUES
    (1, 1),  -- gotify-default
    (1, 2);  -- ntfy-default

-- ============================================================================
-- STEP 9: CREATE LOCALHOST SERVER
-- ============================================================================

INSERT INTO servers (name, hostname, description, is_local, enabled, os_type) VALUES
    ('localhost', NULL, 'Local server (this machine)', 1, 1, 'linux');

-- ============================================================================
-- Migration Complete
-- ============================================================================
--
-- Next Steps:
-- 1. Configure notification channels in the UI
-- 2. Add remote servers with SSH credentials
-- 3. Create job schedules using the seeded command templates
-- 4. Enable/configure notification policies as needed
--
-- The database is now ready for the new unified job execution framework!
