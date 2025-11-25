-- Create plugins table for plugin configuration

CREATE TABLE IF NOT EXISTS plugins (
    id TEXT PRIMARY KEY,  -- 'docker', 'updates', 'health', 'weather', 'speedtest'
    name TEXT NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    config TEXT,  -- JSON configuration
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Seed default plugins
INSERT OR IGNORE INTO plugins (id, name, description, enabled, config) VALUES
('docker', 'Docker Monitor', 'Monitor Docker containers and images', 1, '{}'),
('updates', 'Updates Manager', 'Monitor and manage OS updates', 1, '{}'),
('health', 'System Health', 'Monitor system resources', 1, '{}'),
('weather', 'Weather Monitor', 'OpenWeatherMap integration', 0, '{"api_key": "", "location": "", "units": "imperial"}'),
('speedtest', 'Speed Test', 'Internet speed monitoring', 0, '{"min_down": 100, "min_up": 20}');

