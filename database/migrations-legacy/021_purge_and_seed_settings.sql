-- Migration 021: Purge legacy settings and seed app settings
--
-- Remove all settings from the old job system that are no longer
-- relevant to the terminal-first architecture. Preserve notification
-- settings (seeded in migration 020).

DELETE FROM settings WHERE key NOT LIKE 'notification.%';

-- Seed app-level settings for the new Settings UI
INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('app.default_profile_id', '', 'string', 'Terminal profile ID to auto-load on app start');
