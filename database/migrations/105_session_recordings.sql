-- Session recording (docs/SECURITY.md, "Session recordings"). Opt-in per server: shells
-- opened on a server with `record_sessions` set have their output written, encrypted, to a
-- file under RECORDINGS_DIR. Each recording belongs to the user whose shell it was; nobody
-- else can list, play, download or delete it.
ALTER TABLE servers ADD COLUMN record_sessions BOOLEAN NOT NULL DEFAULT 0;

-- `file_name` is server-generated and relative to RECORDINGS_DIR/<user_id>/. `size_bytes` is
-- the asciicast's plaintext size (what a download returns), not the encrypted file's.
-- A deleted server keeps its recordings under the name it had (`server_name`).
CREATE TABLE session_recordings (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id   INTEGER REFERENCES servers(id) ON DELETE SET NULL,
    server_name TEXT NOT NULL,
    session_id  TEXT NOT NULL,
    file_name   TEXT NOT NULL UNIQUE,
    cols        INTEGER NOT NULL,
    rows        INTEGER NOT NULL,
    started_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ended_at    DATETIME,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'recording'
                CHECK (status IN ('recording', 'complete', 'truncated', 'interrupted', 'failed')),
    end_reason  TEXT
);
CREATE INDEX idx_session_recordings_user ON session_recordings(user_id, id DESC);
CREATE INDEX idx_session_recordings_started ON session_recordings(started_at);

INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('recording.retention_days', '30', 'integer',
     'Days to keep session recordings; 0 keeps them until deleted');
