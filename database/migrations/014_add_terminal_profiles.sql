-- Terminal Profiles Migration
-- Adds terminal_profiles table for saving terminal configurations

-- Terminal profiles table
CREATE TABLE IF NOT EXISTS terminal_profiles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT,
    layout TEXT NOT NULL DEFAULT '2h', -- '1', '2h', '2v', '4'
    pane_configs TEXT, -- JSON: [{"server_id": 1}, {"server_id": 2}, ...]
    quick_commands TEXT, -- JSON: ["uptime", "df -h", ...]
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index on name for quick lookup
CREATE INDEX IF NOT EXISTS idx_terminal_profiles_name ON terminal_profiles(name);
CREATE INDEX IF NOT EXISTS idx_terminal_profiles_is_default ON terminal_profiles(is_default);
