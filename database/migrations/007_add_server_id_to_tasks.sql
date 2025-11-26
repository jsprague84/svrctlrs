-- Add server_id to tasks table for server-centric task organization
-- Migration: 007_add_server_id_to_tasks.sql

-- Add server_id column (nullable for backward compatibility)
ALTER TABLE tasks ADD COLUMN server_id INTEGER REFERENCES servers(id) ON DELETE CASCADE;

-- Add server_name for display purposes (denormalized for performance)
ALTER TABLE tasks ADD COLUMN server_name TEXT;

-- Create index for faster queries grouped by server
CREATE INDEX idx_tasks_server_id ON tasks(server_id);

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
UPDATE tasks 
SET 
    server_id = (SELECT id FROM servers WHERE name = 'localhost' LIMIT 1),
    server_name = 'localhost'
WHERE server_id IS NULL;

-- Make server_id NOT NULL after migration (all tasks must belong to a server)
-- Note: SQLite doesn't support ALTER COLUMN, so we'll keep it nullable but enforce in app logic

