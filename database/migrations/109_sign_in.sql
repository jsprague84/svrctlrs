-- Sign-in (ROADMAP 21): the login limiter's memory, identities from an OpenID Connect
-- provider linked to accounts, and the TOTP second factor with its recovery codes.

-- One row per failed sign-in attempt and key (`ip:<address>` or `user:<name>`), so a restart
-- does not reset the limiter (server/src/login_limiter.rs). Rows older than the window are
-- deleted as new failures come in.
CREATE TABLE login_failures (
    key       TEXT NOT NULL,
    -- Unix seconds
    failed_at INTEGER NOT NULL
);
CREATE INDEX idx_login_failures_key ON login_failures(key, failed_at);

-- An account's identities at an OpenID Connect provider. A sign-in with the provider finds
-- the account by (issuer, subject) and nothing else; there is no sign-up by e-mail address.
CREATE TABLE user_identities (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    issuer       TEXT NOT NULL,
    subject      TEXT NOT NULL,
    -- What the provider said when the identity was linked, for the list only
    email        TEXT,
    display_name TEXT,
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_used_at DATETIME,
    UNIQUE (issuer, subject)
);
CREATE INDEX idx_user_identities_user ON user_identities(user_id);

-- TOTP (RFC 6238, SHA-1, 6 digits, 30 s) for password sign-in. `secret` is the base32 secret,
-- AES-256-GCM encrypted like a credential. `confirmed_at` is NULL while enrolment waits for
-- the first code; `last_step` is the time step last accepted, so a code works once.
CREATE TABLE user_totp (
    user_id      INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    secret       TEXT NOT NULL,
    confirmed_at DATETIME,
    last_step    INTEGER NOT NULL DEFAULT 0,
    created_at   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- One-time recovery codes for a lost authenticator: SHA-256 of the normalised code (the codes
-- are random, so a slow hash adds nothing), each usable once.
CREATE TABLE user_recovery_codes (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    used_at   DATETIME
);
CREATE INDEX idx_user_recovery_codes_user ON user_recovery_codes(user_id);
