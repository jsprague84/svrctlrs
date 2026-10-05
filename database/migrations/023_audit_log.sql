-- Audit log: who did what, from where, to which object. Entries outlive users (no FK) and
-- never hold secrets; `details` is a small JSON object (exit code, command, counts).
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
