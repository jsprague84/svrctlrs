-- Add server_id to tasks table for server-centric task organization
-- Migration: 007_add_server_id_to_tasks.sql
-- 
-- NOTE: If this migration fails with "duplicate column name: server_id",
-- it means the columns were added in a previous partial migration.
-- In that case, you can either:
-- 1. Manually mark this migration as complete in _sqlx_migrations table, OR
-- 2. Run the UPDATE statements below manually to ensure data is correct
--
-- For fresh databases, this migration will run successfully.

-- Add server_id column (nullable for backward compatibility)
-- This will fail if column already exists - that's expected for partial migrations
ALTER TABLE tasks ADD COLUMN server_id INTEGER REFERENCES servers(id) ON DELETE CASCADE;

-- Add server_name for display purposes (denormalized for performance)
-- This will fail if column already exists
ALTER TABLE tasks ADD COLUMN server_name TEXT;

-- Create index (IF NOT EXISTS is supported for indexes)
CREATE INDEX IF NOT EXISTS idx_tasks_server_id ON tasks(server_id);

-- Create a "localhost" server if it doesn't exist
INSERT INTO servers (name, host, port, username, enabled, description, docker_installed, connection_timeout, retry_attempts, created_at, updated_at)
SELECT 
    'localhost',
    NULL,
    22,
    'root',
    1,
    'Local system (SvrCtlRS host)',
    1,
    30,
    3,
    datetime('now'),
    datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM servers WHERE name = 'localhost');

-- Migrate existing tasks to localhost server
-- This is safe to run multiple times
UPDATE tasks 
SET 
    server_id = COALESCE(server_id, (SELECT id FROM servers WHERE name = 'localhost' LIMIT 1)),
    server_name = COALESCE(server_name, 'localhost')
WHERE server_id IS NULL OR server_name IS NULL;
