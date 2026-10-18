-- First-contact confirmation for SSH host keys. A key the server presents when nothing is
-- trusted yet is recorded with trusted_at NULL and the connection is refused until the user
-- confirms its fingerprint (POST /servers/{id}/host-keys/trust). Keys stored before this
-- migration were trusted on first use; they stay trusted.
ALTER TABLE server_host_keys ADD COLUMN trusted_at DATETIME;
UPDATE server_host_keys SET trusted_at = first_seen_at;
