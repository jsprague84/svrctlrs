-- ============================================================================
-- Migration 015: Job Catalog and Workflow Optimization
-- Created: 2025-12-06
-- Purpose: Add pre-built job catalog for simplified job creation (Basic Mode)
-- ============================================================================

-- ============================================================================
-- PART 1: Job Catalog Tables
-- ============================================================================

-- Job Catalog Categories: Organize pre-built jobs by category
CREATE TABLE IF NOT EXISTS job_catalog_categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,            -- 'docker', 'system', 'backup', 'monitoring'
    display_name TEXT NOT NULL,           -- 'Docker Operations'
    description TEXT,
    icon TEXT DEFAULT 'folder',           -- Lucide icon name
    color TEXT,                           -- '#2496ED' (Docker blue)
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_job_catalog_categories_sort ON job_catalog_categories(sort_order);

-- Job Catalog: Pre-built jobs for Basic Mode
CREATE TABLE IF NOT EXISTS job_catalog (
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
    parameters TEXT,                      -- JSON: Parameter schema array
    required_capabilities TEXT,           -- JSON: ["docker"] or ["apt"]
    os_filter TEXT,                       -- JSON: {"distro": ["ubuntu", "debian"]}

    -- Execution Defaults
    default_timeout INTEGER DEFAULT 300,
    default_retry_count INTEGER DEFAULT 0,
    working_directory TEXT,
    environment TEXT,                     -- JSON: {"VAR": "value"}

    -- Notification Templates
    success_title_template TEXT,          -- "{{job_name}} completed on {{server_name}}"
    success_body_template TEXT,
    failure_title_template TEXT,
    failure_body_template TEXT,
    ntfy_success_tags TEXT,               -- JSON: ["white_check_mark", "docker"]
    ntfy_failure_tags TEXT,               -- JSON: ["x", "warning"]

    -- Metadata
    tags TEXT,                            -- JSON: ["cleanup", "maintenance", "disk-space"]
    sort_order INTEGER DEFAULT 0,
    is_system BOOLEAN DEFAULT 1,          -- System-provided (not user-created)
    enabled BOOLEAN DEFAULT 1,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (category) REFERENCES job_catalog_categories(name) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_job_catalog_category ON job_catalog(category);
CREATE INDEX IF NOT EXISTS idx_job_catalog_difficulty ON job_catalog(difficulty);
CREATE INDEX IF NOT EXISTS idx_job_catalog_enabled ON job_catalog(enabled);
CREATE INDEX IF NOT EXISTS idx_job_catalog_sort ON job_catalog(sort_order);

-- User Favorites: Pin frequently used catalog items
CREATE TABLE IF NOT EXISTS job_catalog_favorites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_item_id INTEGER NOT NULL,
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (catalog_item_id) REFERENCES job_catalog(id) ON DELETE CASCADE,
    UNIQUE(catalog_item_id)
);

CREATE INDEX IF NOT EXISTS idx_job_catalog_favorites_sort ON job_catalog_favorites(sort_order);

-- ============================================================================
-- PART 2: Enhanced Notification Policies
-- ============================================================================

-- Add separate success/failure templates to notification_policies
ALTER TABLE notification_policies ADD COLUMN success_title_template TEXT;
ALTER TABLE notification_policies ADD COLUMN success_body_template TEXT;
ALTER TABLE notification_policies ADD COLUMN failure_title_template TEXT;
ALTER TABLE notification_policies ADD COLUMN failure_body_template TEXT;
ALTER TABLE notification_policies ADD COLUMN include_output BOOLEAN DEFAULT 0;
ALTER TABLE notification_policies ADD COLUMN output_max_lines INTEGER DEFAULT 50;
ALTER TABLE notification_policies ADD COLUMN ntfy_success_tags TEXT;  -- JSON array
ALTER TABLE notification_policies ADD COLUMN ntfy_failure_tags TEXT;  -- JSON array

-- ============================================================================
-- PART 3: Job Template Enhancements
-- ============================================================================

-- Link job templates to catalog items for "Run Again" feature
ALTER TABLE job_templates ADD COLUMN catalog_item_id INTEGER REFERENCES job_catalog(id) ON DELETE SET NULL;
ALTER TABLE job_templates ADD COLUMN is_pinned BOOLEAN DEFAULT 0;

-- ============================================================================
-- PART 4: Job Schedule Enhancements
-- ============================================================================

-- Track manual runs for quick execution
ALTER TABLE job_schedules ADD COLUMN last_manual_run_at DATETIME;
ALTER TABLE job_schedules ADD COLUMN manual_run_count INTEGER DEFAULT 0;

-- ============================================================================
-- PART 5: User Preferences
-- ============================================================================

INSERT OR REPLACE INTO settings (key, value, description, value_type) VALUES
    ('ui.mode', '"basic"', 'UI mode: basic (wizard) or advanced (full control)', 'string'),
    ('ui.show_quick_actions', 'true', 'Show quick actions panel on dashboard', 'boolean'),
    ('ui.default_wizard_step', '"select"', 'Default wizard step to start on', 'string');

-- ============================================================================
-- PART 6: SEED DATA - Categories
-- ============================================================================

INSERT INTO job_catalog_categories (name, display_name, description, icon, color, sort_order) VALUES
    ('docker', 'Docker', 'Container management and cleanup', 'container', '#2496ED', 1),
    ('system', 'System', 'OS updates and maintenance', 'server', '#5E81AC', 2),
    ('monitoring', 'Monitoring', 'Health checks and diagnostics', 'activity', '#88C0D0', 3),
    ('backup', 'Backup', 'Data backup and restoration', 'archive', '#A3BE8C', 4),
    ('network', 'Network', 'Network diagnostics and configuration', 'globe', '#B48EAD', 5),
    ('security', 'Security', 'Security scans and updates', 'shield', '#BF616A', 6);

-- ============================================================================
-- PART 7: SEED DATA - Docker Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, success_title_template, failure_title_template, ntfy_success_tags, ntfy_failure_tags, sort_order) VALUES
(
    'docker_cleanup',
    'Docker Cleanup',
    'Remove stopped containers, unused networks, and dangling images to free disk space',
    'docker',
    'trash-2',
    'docker system prune -f{{#if prune_volumes}} --volumes{{/if}}',
    '[{"name": "prune_volumes", "type": "boolean", "label": "Also prune volumes", "description": "Remove unused volumes (caution: data loss possible)", "default": false, "warning": "This will permanently delete volume data!"}]',
    '["docker"]',
    'basic',
    300,
    '{{job_display_name}} completed on {{server_name}}',
    '{{job_display_name}} FAILED on {{server_name}}',
    '["white_check_mark", "whale"]',
    '["x", "whale"]',
    1
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
    '["x", "whale"]',
    2
),
(
    'docker_container_stop',
    'Stop Container',
    'Stop a running Docker container',
    'docker',
    'square',
    'docker stop {{container_name}}',
    '[{"name": "container_name", "type": "string", "label": "Container Name", "description": "Name or ID of the container to stop", "required": true}]',
    '["docker"]',
    'basic',
    60,
    'Container {{container_name}} stopped on {{server_name}}',
    'Failed to stop {{container_name}} on {{server_name}}',
    '["stop_sign", "whale"]',
    '["x", "whale"]',
    3
),
(
    'docker_container_start',
    'Start Container',
    'Start a stopped Docker container',
    'docker',
    'play',
    'docker start {{container_name}}',
    '[{"name": "container_name", "type": "string", "label": "Container Name", "description": "Name or ID of the container to start", "required": true}]',
    '["docker"]',
    'basic',
    60,
    'Container {{container_name}} started on {{server_name}}',
    'Failed to start {{container_name}} on {{server_name}}',
    '["arrow_forward", "whale"]',
    '["x", "whale"]',
    4
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
    '["x", "whale"]',
    5
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
    '["x"]',
    6
),
(
    'docker_list_containers',
    'List Containers',
    'List all Docker containers (running and stopped)',
    'docker',
    'list',
    'docker ps -a --format "table {{.ID}}\t{{.Image}}\t{{.Status}}\t{{.Names}}"',
    '[]',
    '["docker"]',
    'basic',
    30,
    NULL,
    NULL,
    NULL,
    NULL,
    7
),
(
    'docker_pull_image',
    'Pull Image',
    'Pull a Docker image from registry',
    'docker',
    'download',
    'docker pull {{image_name}}{{#if tag}}:{{tag}}{{else}}:latest{{/if}}',
    '[{"name": "image_name", "type": "string", "label": "Image Name", "required": true, "placeholder": "nginx"}, {"name": "tag", "type": "string", "label": "Tag", "default": "latest", "placeholder": "latest"}]',
    '["docker"]',
    'intermediate',
    600,
    'Pulled {{image_name}} on {{server_name}}',
    'Failed to pull {{image_name}} on {{server_name}}',
    '["inbox_tray", "whale"]',
    '["x", "whale"]',
    8
),
(
    'docker_compose_up',
    'Docker Compose Up',
    'Start services defined in docker-compose.yml',
    'docker',
    'layers',
    'cd {{compose_dir}} && docker compose up -d',
    '[{"name": "compose_dir", "type": "string", "label": "Compose Directory", "description": "Directory containing docker-compose.yml", "required": true, "placeholder": "/opt/myapp"}]',
    '["docker"]',
    'intermediate',
    300,
    'Docker Compose started in {{compose_dir}} on {{server_name}}',
    'Docker Compose failed in {{compose_dir}} on {{server_name}}',
    '["rocket", "whale"]',
    '["x", "whale"]',
    9
),
(
    'docker_compose_down',
    'Docker Compose Down',
    'Stop and remove containers defined in docker-compose.yml',
    'docker',
    'layers',
    'cd {{compose_dir}} && docker compose down{{#if remove_volumes}} -v{{/if}}',
    '[{"name": "compose_dir", "type": "string", "label": "Compose Directory", "required": true}, {"name": "remove_volumes", "type": "boolean", "label": "Remove Volumes", "default": false, "warning": "This will delete persistent data!"}]',
    '["docker"]',
    'intermediate',
    120,
    'Docker Compose stopped in {{compose_dir}}',
    'Docker Compose down failed in {{compose_dir}}',
    '["stop_button", "whale"]',
    '["x", "whale"]',
    10
);

-- ============================================================================
-- PART 8: SEED DATA - System Maintenance Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, os_filter, difficulty, default_timeout, success_title_template, failure_title_template, sort_order) VALUES
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
    'Failed to update packages on {{server_name}}',
    1
),
(
    'apt_upgrade',
    'System Upgrade (APT)',
    'Perform full system upgrade on Debian/Ubuntu',
    'system',
    'arrow-up-circle',
    'sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y{{#if dist_upgrade}} && sudo DEBIAN_FRONTEND=noninteractive apt-get dist-upgrade -y{{/if}}',
    '[{"name": "dist_upgrade", "type": "boolean", "label": "Include distribution upgrade", "description": "Also run dist-upgrade for major updates", "default": false}]',
    '["apt"]',
    '{"distro": ["debian", "ubuntu"]}',
    'intermediate',
    1800,
    'System upgraded on {{server_name}}',
    'Upgrade failed on {{server_name}}',
    2
),
(
    'apt_autoremove',
    'Remove Unused Packages (APT)',
    'Remove packages that were automatically installed but are no longer needed',
    'system',
    'trash',
    'sudo apt-get autoremove -y && sudo apt-get autoclean',
    '[]',
    '["apt"]',
    '{"distro": ["debian", "ubuntu"]}',
    'basic',
    300,
    'Unused packages removed on {{server_name}}',
    'Cleanup failed on {{server_name}}',
    3
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
    'Upgrade failed on {{server_name}}',
    4
),
(
    'dnf_autoremove',
    'Remove Unused Packages (DNF)',
    'Remove packages that are no longer needed',
    'system',
    'trash',
    'sudo dnf autoremove -y && sudo dnf clean all',
    '[]',
    '["dnf"]',
    '{"distro": ["fedora", "rhel", "centos", "rocky", "alma"]}',
    'basic',
    300,
    'Unused packages removed on {{server_name}}',
    'Cleanup failed on {{server_name}}',
    5
),
(
    'pacman_upgrade',
    'System Upgrade (Pacman)',
    'Perform full system upgrade on Arch Linux',
    'system',
    'arrow-up-circle',
    'sudo pacman -Syu --noconfirm',
    '[]',
    '["pacman"]',
    '{"distro": ["arch", "manjaro"]}',
    'intermediate',
    1800,
    'System upgraded on {{server_name}}',
    'Upgrade failed on {{server_name}}',
    6
),
(
    'reboot_server',
    'Reboot Server',
    'Safely reboot the server',
    'system',
    'power',
    'sudo shutdown -r {{delay}}',
    '[{"name": "delay", "type": "select", "label": "Delay", "default": "now", "options": [{"value": "now", "label": "Immediately"}, {"value": "+1", "label": "In 1 minute"}, {"value": "+5", "label": "In 5 minutes"}, {"value": "+10", "label": "In 10 minutes"}]}]',
    '[]',
    NULL,
    'advanced',
    60,
    'Reboot initiated on {{server_name}}',
    'Failed to reboot {{server_name}}',
    7
),
(
    'systemd_restart',
    'Restart Service',
    'Restart a systemd service',
    'system',
    'refresh-cw',
    'sudo systemctl restart {{service_name}}',
    '[{"name": "service_name", "type": "string", "label": "Service Name", "required": true, "placeholder": "nginx"}]',
    '["systemd"]',
    NULL,
    'basic',
    60,
    'Service {{service_name}} restarted on {{server_name}}',
    'Failed to restart {{service_name}} on {{server_name}}',
    8
),
(
    'systemd_status',
    'Service Status',
    'Check the status of a systemd service',
    'system',
    'info',
    'sudo systemctl status {{service_name}}',
    '[{"name": "service_name", "type": "string", "label": "Service Name", "required": true}]',
    '["systemd"]',
    NULL,
    'basic',
    30,
    NULL,
    NULL,
    9
);

-- ============================================================================
-- PART 9: SEED DATA - Monitoring Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, sort_order) VALUES
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
    30,
    1
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
    30,
    2
),
(
    'top_processes',
    'Top Processes',
    'Show processes by CPU or memory usage',
    'monitoring',
    'activity',
    'ps aux --sort=-{{sort_by}} | head -n {{count}}',
    '[{"name": "sort_by", "type": "select", "label": "Sort by", "default": "%cpu", "options": [{"value": "%cpu", "label": "CPU Usage"}, {"value": "%mem", "label": "Memory Usage"}, {"value": "rss", "label": "Resident Memory"}, {"value": "vsz", "label": "Virtual Memory"}]}, {"name": "count", "type": "number", "label": "Process count", "default": 10, "validation": {"min": 5, "max": 50}}]',
    '[]',
    'basic',
    30,
    3
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
    30,
    4
),
(
    'uptime_check',
    'System Uptime',
    'Show system uptime and load average',
    'monitoring',
    'clock',
    'uptime',
    '[]',
    '[]',
    'basic',
    30,
    5
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
    30,
    6
),
(
    'network_connections',
    'Network Connections',
    'Show active network connections',
    'monitoring',
    'network',
    'ss -tuln',
    '[]',
    '[]',
    'basic',
    30,
    7
),
(
    'io_stats',
    'Disk I/O Stats',
    'Show disk I/O statistics',
    'monitoring',
    'hard-drive',
    'iostat -x 1 3 2>/dev/null || (echo "iostat not available - showing vmstat" && vmstat 1 3)',
    '[]',
    '[]',
    'intermediate',
    30,
    8
);

-- ============================================================================
-- PART 10: SEED DATA - Backup Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, success_title_template, failure_title_template, sort_order) VALUES
(
    'backup_directory',
    'Backup Directory',
    'Create a compressed backup of a directory',
    'backup',
    'archive',
    'tar -czf {{backup_path}}/{{backup_name}}-$(date +%Y%m%d-%H%M%S).tar.gz -C {{source_dir}} .',
    '[{"name": "source_dir", "type": "string", "label": "Source Directory", "required": true, "placeholder": "/var/www/html"}, {"name": "backup_path", "type": "string", "label": "Backup Location", "required": true, "placeholder": "/backups"}, {"name": "backup_name", "type": "string", "label": "Backup Name", "required": true, "placeholder": "website"}]',
    '[]',
    'intermediate',
    1800,
    'Backup of {{source_dir}} completed',
    'Backup of {{source_dir}} failed',
    1
),
(
    'backup_mysql',
    'Backup MySQL Database',
    'Create a MySQL database dump',
    'backup',
    'database',
    'mysqldump -u {{db_user}} -p{{db_password}} {{db_name}} | gzip > {{backup_path}}/{{db_name}}-$(date +%Y%m%d-%H%M%S).sql.gz',
    '[{"name": "db_name", "type": "string", "label": "Database Name", "required": true}, {"name": "db_user", "type": "string", "label": "Database User", "required": true}, {"name": "db_password", "type": "string", "label": "Database Password", "required": true}, {"name": "backup_path", "type": "string", "label": "Backup Location", "required": true, "placeholder": "/backups"}]',
    '[]',
    'intermediate',
    1800,
    'MySQL backup of {{db_name}} completed',
    'MySQL backup of {{db_name}} failed',
    2
),
(
    'backup_postgres',
    'Backup PostgreSQL Database',
    'Create a PostgreSQL database dump',
    'backup',
    'database',
    'PGPASSWORD={{db_password}} pg_dump -U {{db_user}} {{db_name}} | gzip > {{backup_path}}/{{db_name}}-$(date +%Y%m%d-%H%M%S).sql.gz',
    '[{"name": "db_name", "type": "string", "label": "Database Name", "required": true}, {"name": "db_user", "type": "string", "label": "Database User", "required": true}, {"name": "db_password", "type": "string", "label": "Database Password", "required": true}, {"name": "backup_path", "type": "string", "label": "Backup Location", "required": true}]',
    '[]',
    'intermediate',
    1800,
    'PostgreSQL backup of {{db_name}} completed',
    'PostgreSQL backup of {{db_name}} failed',
    3
),
(
    'cleanup_old_backups',
    'Cleanup Old Backups',
    'Remove backup files older than specified days',
    'backup',
    'trash-2',
    'find {{backup_path}} -name "*.tar.gz" -o -name "*.sql.gz" -mtime +{{retention_days}} -delete',
    '[{"name": "backup_path", "type": "string", "label": "Backup Directory", "required": true}, {"name": "retention_days", "type": "number", "label": "Retention Days", "description": "Delete backups older than this many days", "required": true, "default": 30, "validation": {"min": 1, "max": 365}}]',
    '[]',
    'intermediate',
    300,
    'Old backups cleaned up from {{backup_path}}',
    'Backup cleanup failed in {{backup_path}}',
    4
);

-- ============================================================================
-- PART 11: SEED DATA - Network Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, sort_order) VALUES
(
    'ping_host',
    'Ping Host',
    'Ping a host to check connectivity',
    'network',
    'activity',
    'ping -c {{count}} {{host}}',
    '[{"name": "host", "type": "string", "label": "Host", "required": true, "placeholder": "google.com"}, {"name": "count", "type": "number", "label": "Ping Count", "default": 4, "validation": {"min": 1, "max": 100}}]',
    '[]',
    'basic',
    60,
    1
),
(
    'traceroute',
    'Traceroute',
    'Trace the route packets take to a network host',
    'network',
    'map-pin',
    'traceroute {{host}} 2>/dev/null || tracepath {{host}}',
    '[{"name": "host", "type": "string", "label": "Host", "required": true}]',
    '[]',
    'intermediate',
    120,
    2
),
(
    'dns_lookup',
    'DNS Lookup',
    'Perform DNS lookup for a domain',
    'network',
    'search',
    'nslookup {{domain}} {{dns_server}}',
    '[{"name": "domain", "type": "string", "label": "Domain", "required": true}, {"name": "dns_server", "type": "string", "label": "DNS Server", "placeholder": "8.8.8.8", "description": "Leave empty to use system default"}]',
    '[]',
    'basic',
    30,
    3
),
(
    'curl_check',
    'HTTP Check',
    'Check if a URL is responding',
    'network',
    'globe',
    'curl -sI -o /dev/null -w "Status: %{http_code}\nTime: %{time_total}s\n" {{url}}',
    '[{"name": "url", "type": "string", "label": "URL", "required": true, "placeholder": "https://example.com"}]',
    '[]',
    'basic',
    30,
    4
);

-- ============================================================================
-- PART 12: SEED DATA - Security Jobs
-- ============================================================================

INSERT INTO job_catalog (name, display_name, description, category, icon, command, parameters, required_capabilities, difficulty, default_timeout, success_title_template, failure_title_template, sort_order) VALUES
(
    'check_updates_security',
    'Security Updates Check',
    'Check for available security updates (Debian/Ubuntu)',
    'security',
    'shield-check',
    'apt-get update && apt-get --just-print upgrade 2>&1 | grep -i security | head -20',
    '[]',
    '["apt"]',
    'basic',
    300,
    NULL,
    NULL,
    1
),
(
    'failed_logins',
    'Failed Login Attempts',
    'Show recent failed login attempts',
    'security',
    'alert-octagon',
    'grep "Failed password" /var/log/auth.log 2>/dev/null | tail -{{lines}} || journalctl -u sshd --no-pager | grep "Failed password" | tail -{{lines}}',
    '[{"name": "lines", "type": "number", "label": "Number of entries", "default": 20, "validation": {"min": 5, "max": 100}}]',
    '[]',
    'basic',
    30,
    NULL,
    NULL,
    2
),
(
    'firewall_status',
    'Firewall Status',
    'Show firewall status and rules',
    'security',
    'shield',
    'sudo ufw status verbose 2>/dev/null || sudo iptables -L -n 2>/dev/null || sudo firewall-cmd --list-all 2>/dev/null || echo "No firewall detected"',
    '[]',
    '[]',
    'basic',
    30,
    NULL,
    NULL,
    3
),
(
    'check_listening_ports',
    'Check Listening Ports',
    'Show all ports currently listening for connections',
    'security',
    'radio',
    'sudo ss -tlnp',
    '[]',
    '[]',
    'basic',
    30,
    NULL,
    NULL,
    4
),
(
    'user_accounts',
    'List User Accounts',
    'Show user accounts with shell access',
    'security',
    'users',
    'cat /etc/passwd | grep -v nologin | grep -v false | grep -v sync',
    '[]',
    '[]',
    'basic',
    30,
    NULL,
    NULL,
    5
);

-- ============================================================================
-- Migration Complete
-- ============================================================================
