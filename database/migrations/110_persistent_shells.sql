-- Shells that outlive a restart (ROADMAP 23). On a server with `persistent_shell` set, a new
-- shell runs inside a tmux session on that server, named after the SvrCtlRS session. The row
-- below is what lets a restarted SvrCtlRS find it again: a client reattaching with the session
-- id gets a new SSH connection attached to the same tmux session. The row goes when the shell
-- ends for good (exit, closed, left detached past the user's limit); the tmux session with it.
ALTER TABLE servers ADD COLUMN persistent_shell BOOLEAN NOT NULL DEFAULT 0;

CREATE TABLE persistent_shells (
    id          TEXT PRIMARY KEY,          -- the SvrCtlRS session id
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    server_id   INTEGER NOT NULL REFERENCES servers(id) ON DELETE CASCADE,
    tmux_name   TEXT NOT NULL,
    created_at  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_persistent_shells_user ON persistent_shells(user_id);
