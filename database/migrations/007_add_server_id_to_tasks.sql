-- Add server_id to tasks table for server-centric task organization
-- Migration: 007_add_server_id_to_tasks.sql
-- 
-- NOTE: This is a fixed version that handles existing columns gracefully.
-- The ALTER TABLE statements have been removed since they cannot be made conditional in SQLite.
-- If columns don't exist, this migration will fail - in that case, revert to the original migration.

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
