-- Approvals: the few destructive calls an API token may make only after its owner says yes
-- (server/src/approvals.rs). A token with `require_approval` that deletes a server, deletes a
-- credential or forgets a server's host keys gets 202 and a row here; the owner approves or
-- denies it signed in, and on approval the server makes the original call. Existing tokens
-- keep working as before (0); the create dialog and the API default new tokens to 1.
ALTER TABLE api_tokens ADD COLUMN require_approval INTEGER NOT NULL DEFAULT 0;

CREATE TABLE approvals (
    -- Random and unguessable: it is in the link sent out
    id            TEXT PRIMARY KEY,
    user_id       INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_id      INTEGER REFERENCES api_tokens(id) ON DELETE SET NULL,
    token_name    TEXT NOT NULL,
    -- server.delete | credential.delete | server.host_keys_forget
    action        TEXT NOT NULL,
    method        TEXT NOT NULL,
    path          TEXT NOT NULL,
    target_type   TEXT NOT NULL,
    target_id     INTEGER NOT NULL,
    target_name   TEXT,
    client_ip     TEXT,
    url           TEXT NOT NULL,
    -- pending | approved | denied (expiry is read from expires_at)
    status        TEXT NOT NULL DEFAULT 'pending',
    created_at    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at    DATETIME NOT NULL,
    decided_at    DATETIME,
    decided_by    INTEGER,
    -- What the approved call answered: HTTP status and JSON body
    result_status INTEGER,
    result        TEXT
);
CREATE INDEX idx_approvals_user ON approvals(user_id, created_at DESC);

-- Where approval requests are sent. `push` uses the notification provider above; `webhook`
-- POSTs JSON (an n8n workflow, say, that posts to Matrix); `matrix` posts to a room directly.
INSERT OR IGNORE INTO settings (key, value, value_type, description) VALUES
    ('approval.channel', 'push', 'string', 'Where approval requests go: push, webhook, matrix or none'),
    ('approval.webhook_url', '', 'string', 'Webhook that receives approval requests as JSON'),
    ('approval.matrix_homeserver', '', 'string', 'Matrix homeserver URL for approval requests'),
    ('approval.matrix_room', '', 'string', 'Matrix room id for approval requests'),
    ('approval.matrix_token', '', 'string', 'Access token of the Matrix account that posts approval requests'),
    ('approval.link_base', '', 'string', 'Address approval links start with (empty: the address the request came to)');
