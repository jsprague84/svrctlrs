#!/bin/bash
# Fix task commands in database to match plugin task IDs

set -e

DB_PATH="${DATABASE_URL:-sqlite:/app/data/svrctlrs.db}"
DB_FILE="${DB_PATH#sqlite:}"

echo "Fixing task commands in database: $DB_FILE"

sqlite3 "$DB_FILE" <<EOF
-- Update task commands to match plugin task IDs
UPDATE tasks SET command = 'system_metrics' WHERE plugin_id = 'health' AND command != 'system_metrics';
UPDATE tasks SET command = 'docker_health' WHERE plugin_id = 'docker' AND command != 'docker_health';
UPDATE tasks SET command = 'speedtest_run' WHERE plugin_id = 'speedtest' AND command != 'speedtest_run';
UPDATE tasks SET command = 'weather_check' WHERE plugin_id = 'weather' AND command != 'weather_check';

-- Show updated tasks
SELECT 'Updated tasks:' as '';
SELECT id, name, plugin_id, command, enabled FROM tasks ORDER BY plugin_id, id;
EOF

echo ""
echo "Task commands updated successfully!"
echo "Restart the server for changes to take effect."

