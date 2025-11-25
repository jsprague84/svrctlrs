-- Extend servers table with full configuration
-- This migration extends the existing servers table

-- Add new columns to servers table
ALTER TABLE servers ADD COLUMN host TEXT;
ALTER TABLE servers ADD COLUMN port INTEGER NOT NULL DEFAULT 22;
ALTER TABLE servers ADD COLUMN username TEXT NOT NULL DEFAULT 'root';
ALTER TABLE servers ADD COLUMN ssh_key_path TEXT;
ALTER TABLE servers ADD COLUMN description TEXT;
ALTER TABLE servers ADD COLUMN tags TEXT;  -- JSON array
ALTER TABLE servers ADD COLUMN last_seen_at DATETIME;
ALTER TABLE servers ADD COLUMN os_type TEXT;
ALTER TABLE servers ADD COLUMN os_version TEXT;
ALTER TABLE servers ADD COLUMN docker_installed BOOLEAN DEFAULT 0;
ALTER TABLE servers ADD COLUMN connection_timeout INTEGER DEFAULT 30;
ALTER TABLE servers ADD COLUMN retry_attempts INTEGER DEFAULT 3;

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_servers_enabled ON servers(enabled);
CREATE INDEX IF NOT EXISTS idx_servers_last_seen ON servers(last_seen_at);

-- Migrate ssh_host to host if it exists
UPDATE servers SET host = ssh_host WHERE host IS NULL AND ssh_host IS NOT NULL;

