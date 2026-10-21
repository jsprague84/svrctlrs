-- Backups inside the app (ROADMAP 24, docs/DEPLOYMENT.md "Backups"). The server copies the
-- database into BACKUP_DIR every night with VACUUM INTO and keeps the newest `backup.keep`
-- copies; `svrctl backup` and "Back up now" write the same files. Each attempt leaves a row
-- here, so About can show the last copy and the last error. The files themselves are the
-- truth for what exists; a row only says what happened.
CREATE TABLE backup_runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    -- nightly | manual | svrctl
    trigger     TEXT NOT NULL CHECK (trigger IN ('nightly', 'manual', 'svrctl')),
    started_at  DATETIME NOT NULL,
    finished_at DATETIME NOT NULL,
    -- The copy's file name in BACKUP_DIR; NULL when the attempt failed
    file_name   TEXT,
    size_bytes  INTEGER,
    -- Older copies deleted after this one succeeded
    removed     INTEGER NOT NULL DEFAULT 0,
    error       TEXT
);
CREATE INDEX idx_backup_runs_started ON backup_runs(started_at);

INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('backup.keep', '14', 'integer',
     'Nightly database backups to keep; 0 turns nightly backups off'),
    ('backup.hour', '3', 'integer',
     'Hour of the day (UTC, 0-23) the nightly backup runs');
