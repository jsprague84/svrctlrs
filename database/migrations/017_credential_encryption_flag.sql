-- Add encryption flag to credentials table
-- Tracks whether the credential value is encrypted at rest
ALTER TABLE credentials ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT 0;
