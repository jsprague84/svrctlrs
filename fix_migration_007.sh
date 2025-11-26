#!/bin/bash
# Quick fix for migration 007 - run this on docker-vm

set -e

echo "🔧 Fixing migration 007..."
echo ""

# Stop container
echo "1. Stopping container..."
docker-compose down

# Determine database path
if [ -f "./data/svrctlrs.db" ]; then
    DB_PATH="./data/svrctlrs.db"
    echo "✓ Found database at: $DB_PATH"
else
    echo "⚠️  Database not found at ./data/svrctlrs.db"
    echo "   Checking Docker volume..."
    VOLUME_PATH=$(docker volume inspect svrctlrs_data -f '{{.Mountpoint}}' 2>/dev/null || echo "")
    if [ -n "$VOLUME_PATH" ]; then
        DB_PATH="${VOLUME_PATH}/svrctlrs.db"
        echo "✓ Found database at: $DB_PATH"
    else
        echo "❌ Could not find database. Please locate it manually."
        exit 1
    fi
fi

# Check if we need sudo
if [ -w "$DB_PATH" ]; then
    SQLITE="sqlite3"
else
    SQLITE="sudo sqlite3"
    echo "   (Using sudo for database access)"
fi

# Check if columns exist
echo ""
echo "2. Checking if columns exist..."
SCHEMA=$($SQLITE "$DB_PATH" "SELECT sql FROM sqlite_master WHERE type='table' AND name='tasks';")
if echo "$SCHEMA" | grep -q "server_id"; then
    echo "✓ server_id column exists"
else
    echo "❌ server_id column does not exist - migration should run normally"
    echo "   There may be a different issue. Check container logs."
    exit 1
fi

# Mark migration as complete
echo ""
echo "3. Marking migration 007 as complete..."
$SQLITE "$DB_PATH" "INSERT OR IGNORE INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES (7, 'add server_id to tasks', datetime('now'), 1, x'', 0);"
echo "✓ Migration marked complete"

# Ensure localhost server exists
echo ""
echo "4. Ensuring localhost server exists..."
$SQLITE "$DB_PATH" "INSERT OR IGNORE INTO servers (name, host, port, username, enabled, description, docker_installed, connection_timeout, retry_attempts, created_at, updated_at) VALUES ('localhost', NULL, 22, 'root', 1, 'Local system (SvrCtlRS host)', 1, 30, 3, datetime('now'), datetime('now'));"
echo "✓ Localhost server ensured"

# Update tasks
echo ""
echo "5. Updating tasks with server_id and server_name..."
$SQLITE "$DB_PATH" "UPDATE tasks SET server_id = COALESCE(server_id, (SELECT id FROM servers WHERE name = 'localhost' LIMIT 1)), server_name = COALESCE(server_name, 'localhost') WHERE server_id IS NULL OR server_name IS NULL;"
UPDATED=$($SQLITE "$DB_PATH" "SELECT changes();")
echo "✓ Updated $UPDATED task(s)"

# Verify
echo ""
echo "6. Verifying fix..."
NULL_COUNT=$($SQLITE "$DB_PATH" "SELECT COUNT(*) FROM tasks WHERE server_id IS NULL;")
if [ "$NULL_COUNT" -eq "0" ]; then
    echo "✓ All tasks have server_id"
else
    echo "⚠️  Warning: $NULL_COUNT tasks still have NULL server_id"
fi

# Start container
echo ""
echo "7. Starting container..."
docker-compose up -d

echo ""
echo "✅ Fix complete! Check logs with: docker-compose logs -f"

