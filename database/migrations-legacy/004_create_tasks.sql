-- Create tasks table for scheduled task management

CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    plugin_id TEXT NOT NULL,
    server_id INTEGER,  -- NULL = all servers
    
    -- Scheduling
    schedule TEXT NOT NULL,  -- Cron expression
    enabled BOOLEAN NOT NULL DEFAULT 1,
    
    -- Task configuration
    command TEXT NOT NULL,
    args TEXT,  -- JSON arguments
    timeout INTEGER DEFAULT 300,  -- seconds
    
    -- Metadata
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_run_at DATETIME,
    next_run_at DATETIME,
    run_count INTEGER NOT NULL DEFAULT 0,
    
    FOREIGN KEY (plugin_id) REFERENCES plugins(id) ON DELETE CASCADE,
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_tasks_enabled ON tasks(enabled);
CREATE INDEX IF NOT EXISTS idx_tasks_next_run ON tasks(next_run_at);
CREATE INDEX IF NOT EXISTS idx_tasks_plugin ON tasks(plugin_id);

