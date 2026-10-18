-- Baseline schema for databases created from 3.0.x on. A database that predates it ran the
-- legacy chain in database/migrations-legacy/ (000–024, frozen) and is marked as having this
-- version applied; see `run_migrations` in database/src/lib.rs. Like every shipped migration
-- this file is never edited: add 101_… for the next change. The `tower_sessions` table is
-- created by the session store itself.

CREATE TABLE users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_users_username ON users(username);

-- `value` (the secret) and `metadata` (JSON, e.g. {"key_passphrase": "..."}) are stored
-- AES-256-GCM encrypted; the flags say so per row (core/src/encryption.rs).
CREATE TABLE credentials (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    name               TEXT NOT NULL UNIQUE,
    credential_type    TEXT NOT NULL CHECK (credential_type IN ('ssh_key', 'api_token', 'password', 'certificate')),
    description        TEXT,
    value              TEXT NOT NULL,
    username           TEXT,
    metadata           TEXT,
    encrypted          BOOLEAN NOT NULL DEFAULT 0,
    metadata_encrypted BOOLEAN NOT NULL DEFAULT 0,
    created_at         DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at         DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_credentials_type ON credentials(credential_type);

-- An SSH target. `username` NULL means "the credential's user, else root".
CREATE TABLE servers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL UNIQUE,
    hostname      TEXT NOT NULL,
    port          INTEGER NOT NULL DEFAULT 22,
    username      TEXT,
    credential_id INTEGER REFERENCES credentials(id) ON DELETE RESTRICT,
    description   TEXT,
    enabled       BOOLEAN NOT NULL DEFAULT 1,
    last_seen_at  DATETIME,
    last_error    TEXT,
    created_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_servers_enabled ON servers(enabled);
CREATE INDEX idx_servers_hostname ON servers(hostname);

-- Trust on first use, one row per (server, key type); see docs/SECURITY.md.
CREATE TABLE server_host_keys (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id     INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    key_type      TEXT NOT NULL,
    public_key    TEXT NOT NULL,
    first_seen_at DATETIME NOT NULL DEFAULT (datetime('now')),
    last_seen_at  DATETIME NOT NULL DEFAULT (datetime('now')),
    UNIQUE (server_id, key_type)
);

-- A saved layout: `pane_configs` is a JSON array of {"server_id": n}. `user_id` NULL rows
-- predate accounts and are visible to everyone.
CREATE TABLE terminal_profiles (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT NOT NULL,
    description  TEXT,
    layout       TEXT NOT NULL DEFAULT 'single',
    pane_configs TEXT,
    is_default   INTEGER NOT NULL DEFAULT 0,
    user_id      INTEGER REFERENCES users(id) ON DELETE CASCADE,
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_terminal_profiles_name ON terminal_profiles(name);
CREATE INDEX idx_terminal_profiles_is_default ON terminal_profiles(is_default);

CREATE TABLE quick_commands (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER REFERENCES users(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    command    TEXT NOT NULL,
    server_id  INTEGER REFERENCES servers(id) ON DELETE SET NULL,
    category   TEXT NOT NULL DEFAULT 'general',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_quick_commands_user ON quick_commands(user_id);
CREATE INDEX idx_quick_commands_server ON quick_commands(server_id);
CREATE INDEX idx_quick_commands_category ON quick_commands(user_id, category);

CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    description TEXT,
    value_type  TEXT NOT NULL CHECK (value_type IN ('string', 'integer', 'boolean', 'json')),
    updated_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO settings (key, value, value_type, description) VALUES
    ('notification.enabled', 'false', 'boolean', 'Enable push notifications via rstify/Gotify/ntfy'),
    ('notification.provider', 'gotify', 'string', 'Notification provider: gotify or ntfy'),
    ('notification.url', '', 'string', 'Notification server URL (e.g., https://rstify.example.com)'),
    ('notification.token', '', 'string', 'API token for notification server'),
    ('notification.topic', 'svrctlrs', 'string', 'Topic/channel for notifications (ntfy only)'),
    ('notification.health_check_enabled', 'false', 'boolean', 'Enable periodic server health checks'),
    ('app.default_profile_id', '', 'string', 'Terminal profile ID to auto-load on app start');

-- Who did what, from where, to which object. Entries outlive users (no FK) and never hold
-- secrets; `details` is a small JSON object. Pruned after 180 days at start-up.
CREATE TABLE audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    at          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id     INTEGER,
    username    TEXT,
    client_ip   TEXT,
    action      TEXT NOT NULL,
    target_type TEXT,
    target_id   INTEGER,
    target_name TEXT,
    details     TEXT
);
CREATE INDEX idx_audit_log_at ON audit_log(at DESC);
CREATE INDEX idx_audit_log_user ON audit_log(user_id);
