-- Free-form tags per server (a JSON array of strings) for filtering the server lists.
ALTER TABLE servers ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
