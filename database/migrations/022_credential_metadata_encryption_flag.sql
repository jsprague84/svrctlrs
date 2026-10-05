-- Credential metadata (e.g. an SSH key passphrase) is encrypted at rest like the value.
-- The flag is separate from `encrypted` so rows written before this change — encrypted value,
-- plaintext metadata — can be recognised and re-encrypted at the next server start.
ALTER TABLE credentials ADD COLUMN metadata_encrypted BOOLEAN NOT NULL DEFAULT 0;
