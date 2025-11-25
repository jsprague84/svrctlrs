-- Create settings table for global application settings

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'string',  -- 'string', 'number', 'boolean', 'json'
    description TEXT,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CHECK (type IN ('string', 'number', 'boolean', 'json'))
);

-- Seed default settings
INSERT OR IGNORE INTO settings (key, value, type, description) VALUES
('app_name', 'SvrCtlRS', 'string', 'Application name'),
('timezone', 'America/Chicago', 'string', 'Default timezone'),
('log_level', 'info', 'string', 'Logging level'),
('retention_days', '30', 'number', 'Days to keep task history'),
('default_ssh_key', '/home/svrctlrs/.ssh/id_rsa', 'string', 'Default SSH key path'),
('webhook_secret', '', 'string', 'Webhook authentication secret'),
('enable_notifications', 'true', 'boolean', 'Enable notification system');

