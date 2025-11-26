-- Clean up localhost server approach
-- Migration: 008_cleanup_localhost_server.sql
--
-- This migration implements a cleaner hybrid approach:
-- - Tasks with server_id = NULL are local plugin executions
-- - Tasks with server_id = <id> are remote SSH executions
-- - Removes the "localhost" server hack from the database

-- Update all tasks that reference the localhost server to have NULL server_id
UPDATE tasks 
SET server_id = NULL, server_name = NULL
WHERE server_id = (SELECT id FROM servers WHERE name = 'localhost' LIMIT 1);

-- Also update all plugin tasks to be local (NULL server_id)
-- Plugin tasks should run locally on the SvrCtlRS host, not via SSH
UPDATE tasks 
SET server_id = NULL, server_name = NULL
WHERE plugin_id IN ('docker', 'health', 'updates', 'weather', 'speedtest');

-- Delete the localhost server (it's no longer needed)
DELETE FROM servers WHERE name = 'localhost';

