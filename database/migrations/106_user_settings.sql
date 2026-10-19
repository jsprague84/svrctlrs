-- Per-user preferences, so every device a user signs in on agrees (the terminal's font,
-- cursor, scrollback, …). One row per user and key; `value` is a JSON object the UI owns.
-- The server only checks the key, the shape and the size (routes/api/users.rs).
CREATE TABLE user_settings (
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, key)
);
