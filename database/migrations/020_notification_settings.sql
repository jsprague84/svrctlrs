-- Migration 020: Seed notification settings
-- These use the existing settings key-value table

INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('notification.enabled', 'false', 'boolean', 'Enable push notifications via rstify/Gotify/ntfy'),
    ('notification.provider', 'gotify', 'string', 'Notification provider: gotify or ntfy'),
    ('notification.url', '', 'string', 'Notification server URL (e.g., https://rstify.example.com)'),
    ('notification.token', '', 'string', 'API token for notification server'),
    ('notification.topic', 'svrctlrs', 'string', 'Topic/channel for notifications (ntfy only)'),
    ('notification.health_check_enabled', 'false', 'boolean', 'Enable periodic server health checks'),
    ('notification.health_check_interval_seconds', '300', 'number', 'Interval between health checks in seconds');
