#!/bin/bash
# Fix plugin task IDs in the database

DB_PATH=${1:-/app/data/svrctlrs.db}

echo "Fixing plugin task IDs in database: $DB_PATH"

sqlite3 "$DB_PATH" <<EOF
-- Fix Updates plugin task ID
UPDATE tasks 
SET command = 'updates_check' 
WHERE plugin_id = 'updates' AND (command = 'execute' OR command IS NULL OR command = '');

-- Verify all task commands are correct
SELECT id, name, plugin_id, command, enabled 
FROM tasks 
ORDER BY id;
EOF

if [ $? -eq 0 ]; then
    echo "✅ Task IDs updated successfully!"
    echo "Restart the server for changes to take effect."
else
    echo "❌ Error: Failed to update task IDs."
    exit 1
fi

