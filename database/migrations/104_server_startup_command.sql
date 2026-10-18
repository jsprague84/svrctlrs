-- Typed into every new shell on the server right after it starts (for example
-- `tmux new -A -s main`); NULL runs nothing.
ALTER TABLE servers ADD COLUMN startup_command TEXT;
