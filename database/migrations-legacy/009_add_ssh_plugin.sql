-- Add SSH plugin for remote task execution
-- This plugin is used as a placeholder for tasks that execute SSH commands on remote servers

INSERT INTO plugins (id, name, description, enabled, config, created_at, updated_at)
SELECT
    'ssh',
    'SSH Remote Execution',
    'Virtual plugin for executing remote SSH commands on servers',
    1,
    '{}',
    datetime('now'),
    datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM plugins WHERE id = 'ssh');
