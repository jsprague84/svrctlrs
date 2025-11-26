#!/bin/bash
# Fix migration 007 by marking it as complete if columns already exist

echo "Checking if server_id column exists..."
docker exec svrctlrs sqlite3 /app/data/svrctlrs.db "
SELECT CASE 
    WHEN EXISTS (SELECT 1 FROM pragma_table_info('tasks') WHERE name = 'server_id')
    THEN 'Columns exist - marking migration as complete'
    ELSE 'Columns do not exist - migration should run normally'
END as status;
"

echo ""
echo "If columns exist, marking migration 007 as complete..."
docker exec svrctlrs sqlite3 /app/data/svrctlrs.db "
INSERT OR IGNORE INTO _sqlx_migrations (version, description, installed_on, success, checksum)
VALUES (7, 'add server_id to tasks', datetime('now'), 1, '');
"

echo "Migration fix complete!"
