-- Signed-in devices (ROADMAP 26, docs/SECURITY.md "Signed-in devices"). One row per browser or
-- desktop-app sign-in. The row is what keeps a sign-in alive: the auth layer accepts a session
-- only while its row exists, so signing a device out is deleting its row (and its session).
-- The session id itself is a credential and lives only in tower_sessions; this table keeps
-- its SHA-256, so a backup copy holds nothing that signs anyone in.
CREATE TABLE devices (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_hash TEXT NOT NULL UNIQUE,
    -- How it signed in: password, password+totp, password+recovery_code, oidc, or app (the
    -- desktop app's keyring session)
    method       TEXT NOT NULL,
    user_agent   TEXT,
    first_ip     TEXT,
    last_ip      TEXT,
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_devices_user ON devices(user_id);
