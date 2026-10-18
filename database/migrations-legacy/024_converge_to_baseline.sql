-- Last step of the legacy chain (000–023). Brings a database that grew through the plugin,
-- job and command-mode eras to the shape of the 100 baseline (database/migrations/) without
-- rebuilding tables, so foreign keys and data are untouched.
--
-- Retired "local" servers (no hostname; command mode could run them in the container) cannot
-- connect any more. Their rows are kept but disabled, and given a hostname so the model's
-- NOT NULL rule holds. `servers.is_local` itself stays: it is part of a CHECK constraint and
-- cannot be dropped without a rebuild; nothing reads it.
UPDATE servers SET enabled = 0 WHERE is_local = 1;
UPDATE servers SET hostname = 'localhost' WHERE hostname IS NULL OR TRIM(hostname) = '';

-- Monitoring-era facts nothing has read since 3.0.
ALTER TABLE servers DROP COLUMN os_type;
ALTER TABLE servers DROP COLUMN os_distro;
ALTER TABLE servers DROP COLUMN package_manager;
ALTER TABLE servers DROP COLUMN docker_available;
ALTER TABLE servers DROP COLUMN systemd_available;
ALTER TABLE servers DROP COLUMN metadata;

-- Quick commands have their own table since 019.
ALTER TABLE terminal_profiles DROP COLUMN quick_commands;
