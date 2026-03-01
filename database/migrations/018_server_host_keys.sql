-- Migration 018: Server SSH host keys for TOFU (Trust On First Use) verification
CREATE TABLE IF NOT EXISTS server_host_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER NOT NULL,
    key_type TEXT NOT NULL,
    public_key TEXT NOT NULL,
    first_seen_at DATETIME NOT NULL DEFAULT (datetime('now')),
    last_seen_at DATETIME NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (server_id) REFERENCES servers(id) ON DELETE CASCADE,
    UNIQUE(server_id, key_type)
);
