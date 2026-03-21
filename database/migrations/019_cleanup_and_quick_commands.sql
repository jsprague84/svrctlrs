-- Migration 019: Database cleanup + quick commands table
--
-- Drop legacy tables from the old job-based automation system.
-- These tables were created by migrations 001-015 but are no longer used
-- on the current terminal-first architecture.
--
-- Order matters: drop leaf tables (no dependents) first,
-- then tables they reference, working up the FK dependency tree.

-- Step 1: Drop tables with no dependents (leaf nodes)
-- notification_policy_channels references notification_policies + notification_channels
DROP TABLE IF EXISTS notification_policy_channels;
-- notification_log references notification_policies + job_runs
DROP TABLE IF EXISTS notification_log;
-- step_execution_results references job_runs
DROP TABLE IF EXISTS step_execution_results;
-- server_job_results references job_runs + servers
DROP TABLE IF EXISTS server_job_results;
-- job_catalog_favorites references job_catalog + users
DROP TABLE IF EXISTS job_catalog_favorites;

-- Step 2: Drop tables referenced only by tables we just dropped
-- job_runs references job_schedules + job_templates + servers
DROP TABLE IF EXISTS job_runs;
-- notification_policies references job_types + job_templates
DROP TABLE IF EXISTS notification_policies;
-- job_catalog references job_catalog_categories
DROP TABLE IF EXISTS job_catalog;

-- Step 3: Continue up the dependency tree
-- notification_channels (referenced by notification_policy_channels, already dropped)
DROP TABLE IF EXISTS notification_channels;
-- job_schedules references job_templates + servers + notification_policies
DROP TABLE IF EXISTS job_schedules;
-- job_template_steps references job_templates + command_templates
DROP TABLE IF EXISTS job_template_steps;
-- job_catalog_categories (referenced by job_catalog, already dropped)
DROP TABLE IF EXISTS job_catalog_categories;

-- Step 4: Core job tables
-- job_templates references job_types + command_templates
DROP TABLE IF EXISTS job_templates;
-- command_templates references job_types
DROP TABLE IF EXISTS command_templates;
-- job_types (top of job hierarchy)
DROP TABLE IF EXISTS job_types;

-- Step 5: Server organization (no complex FK dependencies)
DROP TABLE IF EXISTS server_tags;
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS server_capabilities;

-- Step 6: Legacy tables from earlier migrations (may or may not exist)
DROP TABLE IF EXISTS webhooks;
DROP TABLE IF EXISTS task_history;
DROP TABLE IF EXISTS metrics;
DROP TABLE IF EXISTS tasks;
DROP TABLE IF EXISTS plugins;
DROP TABLE IF EXISTS features;

-- Phase 2: Add user_id to terminal_profiles for multi-user scoping
ALTER TABLE terminal_profiles ADD COLUMN user_id INTEGER REFERENCES users(id) ON DELETE CASCADE;

-- Phase 3: Update layout enum values to match frontend convention
UPDATE terminal_profiles SET layout = 'single' WHERE layout = '1';
UPDATE terminal_profiles SET layout = 'split-h' WHERE layout = '2h';
UPDATE terminal_profiles SET layout = 'split-v' WHERE layout = '2v';
UPDATE terminal_profiles SET layout = 'quad' WHERE layout = '4';

-- Phase 4: Create quick_commands table
CREATE TABLE IF NOT EXISTS quick_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    server_id INTEGER REFERENCES servers(id) ON DELETE SET NULL,
    category TEXT NOT NULL DEFAULT 'general',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_quick_commands_user ON quick_commands(user_id);
CREATE INDEX IF NOT EXISTS idx_quick_commands_server ON quick_commands(server_id);
CREATE INDEX IF NOT EXISTS idx_quick_commands_category ON quick_commands(user_id, category);
