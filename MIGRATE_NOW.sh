#!/bin/bash
# Quick migration script for existing deployments
# Run this on your docker-vm to migrate to database-based plugin configuration

set -e

echo "=================================="
echo "SvrCtlRS Migration Script"
echo "Database as Source of Truth"
echo "=================================="
echo ""

cd ~/docker-compose/svrctlrs || { echo "Error: Directory not found"; exit 1; }

echo "Step 1: Pulling latest image..."
docker compose pull

echo ""
echo "Step 2: Enabling plugins in database..."
docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db <<EOF
-- Enable weather and speedtest plugins
UPDATE plugins SET enabled = 1 WHERE id IN ('weather', 'speedtest');

-- Show current plugin status
SELECT '=== Plugin Status ===' as '';
SELECT id, name, enabled FROM plugins ORDER BY id;
EOF

echo ""
echo "Step 3: Cleaning up .env file..."
# Backup .env if it exists
if [ -f .env ]; then
    cp .env .env.backup
    echo "✓ Backed up .env to .env.backup"
    
    # Remove old plugin enable flags
    sed -i '/ENABLE_.*_PLUGIN/d' .env
    echo "✓ Removed old ENABLE_*_PLUGIN variables from .env"
fi

echo ""
echo "Step 4: Restarting container..."
docker compose restart svrctlrs

echo ""
echo "Step 5: Waiting for startup..."
sleep 3

echo ""
echo "Step 6: Checking logs..."
docker compose logs --tail=50 svrctlrs | grep -i "registering.*plugin"

echo ""
echo "=================================="
echo "✓ Migration Complete!"
echo "=================================="
echo ""
echo "All plugins are now managed via the database."
echo "Access the UI at: http://$(hostname -I | awk '{print $1}'):8080"
echo ""
echo "To enable/disable plugins:"
echo "  1. Go to http://your-server:8080/plugins"
echo "  2. Toggle plugins on/off"
echo "  3. No restart needed!"
echo ""
echo "To view plugin status in database:"
echo "  docker compose exec svrctlrs sqlite3 /app/data/svrctlrs.db \"SELECT * FROM plugins;\""
echo ""

