-- Initial database schema
-- Creates the base servers table

CREATE TABLE IF NOT EXISTS servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    host TEXT,
    port INTEGER NOT NULL DEFAULT 22,
    username TEXT NOT NULL DEFAULT 'root',
    ssh_key_path TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    description TEXT,
    tags TEXT,  -- JSON array of strings
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at DATETIME,
    os_type TEXT,
    os_version TEXT,
    docker_installed BOOLEAN NOT NULL DEFAULT 0,
    connection_timeout INTEGER NOT NULL DEFAULT 30,
    retry_attempts INTEGER NOT NULL DEFAULT 3
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
CREATE INDEX IF NOT EXISTS idx_servers_enabled ON servers(enabled);
CREATE INDEX IF NOT EXISTS idx_servers_last_seen ON servers(last_seen_at);

