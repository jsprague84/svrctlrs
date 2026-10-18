-- Jump hosts: a server may be reached through another server's SSH connection (ProxyJump).
-- Deleting the jump host makes its dependants direct again rather than orphaning them.
ALTER TABLE servers ADD COLUMN via_server_id INTEGER REFERENCES servers(id) ON DELETE SET NULL;
