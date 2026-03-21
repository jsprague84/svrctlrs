-- Migration 019: Database cleanup + quick commands table
--
-- Phase 1: Drop legacy tables from the old job-based automation system.
-- These tables were created by migrations 001-015 but are no longer used
-- on the current terminal-first architecture (ralph/phase1-foundation branch).

-- Job execution system (011)
DROP TABLE IF EXISTS step_execution_results;
DROP TABLE IF EXISTS server_job_results;
DROP TABLE IF EXISTS job_runs;
DROP TABLE IF EXISTS job_schedules;
DROP TABLE IF EXISTS job_template_steps;
DROP TABLE IF EXISTS job_templates;
DROP TABLE IF EXISTS command_templates;
DROP TABLE IF EXISTS job_types;

-- Notification system (011) — replaced by rstify integration
DROP TABLE IF EXISTS notification_policy_channels;
DROP TABLE IF EXISTS notification_policies;
DROP TABLE IF EXISTS notification_channels;
DROP TABLE IF EXISTS notification_log;

-- Job catalog (015)
DROP TABLE IF EXISTS job_catalog_favorites;
DROP TABLE IF EXISTS job_catalog;
DROP TABLE IF EXISTS job_catalog_categories;

-- Server organization (011) — not used in terminal-first architecture
DROP TABLE IF EXISTS tags;
DROP TABLE IF EXISTS server_tags;
DROP TABLE IF EXISTS server_capabilities;

-- Legacy tables from earlier migrations (may or may not exist)
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
-- Replaces the limited quick_commands TEXT field on terminal_profiles
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
