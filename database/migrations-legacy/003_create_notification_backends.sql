-- Create notification_backends table for notification configuration
-- Note: The existing 'notifications' table stores notification LOG
-- This new table stores notification BACKENDS (Gotify, ntfy.sh configs)

CREATE TABLE IF NOT EXISTS notification_backends (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    type TEXT NOT NULL,  -- 'gotify', 'ntfy'
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    config TEXT NOT NULL,  -- JSON configuration
    priority INTEGER NOT NULL DEFAULT 5,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    CHECK (type IN ('gotify', 'ntfy')),
    CHECK (priority >= 1 AND priority <= 10)
);

CREATE INDEX IF NOT EXISTS idx_notification_backends_enabled ON notification_backends(enabled);

