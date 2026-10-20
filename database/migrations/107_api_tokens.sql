-- Named API tokens for agents and scripts (Settings → Access → Agent tokens). A token acts as
-- its owner with the access its preset names (server/src/api_tokens.rs). Only a SHA-256 hash
-- of the secret is stored; `prefix` is its first characters, kept so a list can show which
-- token is which. `expires_at` NULL means it never expires; a revoked token keeps its row
-- (and its name in the audit log) until the owner removes it.
CREATE TABLE api_tokens (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    prefix       TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    preset       TEXT NOT NULL,
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at   DATETIME,
    last_used_at DATETIME,
    last_used_ip TEXT,
    revoked_at   DATETIME,
    UNIQUE (user_id, name)
);
CREATE INDEX idx_api_tokens_user ON api_tokens(user_id);

-- Which token, if any, a request that wrote an audit row came in with. No foreign key: the
-- row keeps naming the token after it is removed.
ALTER TABLE audit_log ADD COLUMN token_id INTEGER;
ALTER TABLE audit_log ADD COLUMN token_name TEXT;
