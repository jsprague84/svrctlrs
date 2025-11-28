-- Rename plugin_id to feature_id in tasks table
-- This migration supports the architectural shift from plugins to built-in features
--
-- Features are now code-level modules (server/src/features/) rather than database-driven plugins
-- The feature_id column stores the feature type (e.g., "docker", "updates", "health", "ssh")

-- Note: SQLite doesn't support RENAME COLUMN directly in older versions,
-- so we'll use a temporary table approach for compatibility

-- Step 1: Create new tasks table with feature_id
-- Note: Temporarily omitting FK constraints to avoid issues during data migration
-- FK constraints will be added in a future migration if needed
CREATE TABLE IF NOT EXISTS tasks_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    feature_id TEXT NOT NULL,  -- Renamed from plugin_id, no FK (features are code-level)
    server_id INTEGER,  -- NULL for local, server ID for remote
    command TEXT NOT NULL,
    args TEXT,
    schedule TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    timeout INTEGER NOT NULL DEFAULT 300,
    last_run_at DATETIME,
    next_run_at DATETIME,
    success_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    -- Note: server_id FK constraint omitted for migration compatibility
    -- FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

-- Step 2: Copy all data from old table to new table
-- Handle old schema which had run_count instead of success_count/failure_count
INSERT INTO tasks_new (
    id, name, description, feature_id, server_id, command, args,
    schedule, enabled, timeout, last_run_at, next_run_at,
    success_count, failure_count, created_at, updated_at
)
SELECT
    id,
    name,
    description,
    plugin_id as feature_id,
    server_id,
    command,
    args,
    schedule,
    enabled,
    COALESCE(timeout, 300) as timeout,
    last_run_at,
    next_run_at,
    0 as success_count,  -- Old schema had run_count, start fresh with new counters
    0 as failure_count,
    created_at,
    updated_at
FROM tasks;

-- Step 3: Drop old table
DROP TABLE tasks;

-- Step 4: Rename new table to tasks
ALTER TABLE tasks_new RENAME TO tasks;

-- Step 5: Recreate indexes (if they existed)
CREATE INDEX IF NOT EXISTS idx_tasks_feature_id ON tasks(feature_id);
CREATE INDEX IF NOT EXISTS idx_tasks_server_id ON tasks(server_id);
CREATE INDEX IF NOT EXISTS idx_tasks_enabled ON tasks(enabled);
CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON tasks(next_run_at);

-- Step 6: Update task_history table to use feature_id
-- Create new table (FK constraints omitted for migration compatibility)
CREATE TABLE IF NOT EXISTS task_history_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    feature_id TEXT NOT NULL,  -- Renamed from plugin_id
    server_id INTEGER,
    success BOOLEAN NOT NULL,
    output TEXT,
    error TEXT,
    duration_ms INTEGER NOT NULL,
    executed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
    -- Note: FK constraints omitted to handle orphaned records in legacy data
    -- FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    -- FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE SET NULL
);

-- Copy data from old schema
-- Old schema had: message, stdout, stderr, error_message, timestamp, finished_at
-- New schema has: output, error, executed_at
INSERT INTO task_history_new (
    id, task_id, feature_id, server_id, success, output, error, duration_ms, executed_at
)
SELECT
    id,
    task_id,
    plugin_id as feature_id,
    server_id,
    success,
    COALESCE(stdout, message, '') as output,
    CASE
        WHEN error_message IS NOT NULL THEN error_message
        WHEN stderr IS NOT NULL AND stderr != '' THEN stderr
        ELSE NULL
    END as error,
    COALESCE(duration_ms, 0) as duration_ms,
    COALESCE(finished_at, timestamp, CURRENT_TIMESTAMP) as executed_at
FROM task_history;

-- Drop old table and rename
DROP TABLE task_history;
ALTER TABLE task_history_new RENAME TO task_history;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_task_history_task_id ON task_history(task_id);
CREATE INDEX IF NOT EXISTS idx_task_history_executed_at ON task_history(executed_at);

-- Note: We're keeping the plugins table for now to maintain backward compatibility
-- It can be dropped in a future migration once all references are removed
-- Foreign key constraints have been omitted from this migration to ensure compatibility
-- They can be re-added in a future migration if needed
