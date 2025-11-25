#!/bin/bash
# Helper script to automatically detect and configure Docker GID for the container
# This allows the Docker plugin to access the Docker socket

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$PROJECT_ROOT/.env"

echo "🔍 Detecting Docker socket GID..."

# Check if Docker socket exists
if [ ! -S /var/run/docker.sock ]; then
    echo "❌ Error: Docker socket not found at /var/run/docker.sock"
    echo "   Is Docker installed and running?"
    exit 1
fi

# Get Docker socket GID
DOCKER_GID=$(stat -c '%g' /var/run/docker.sock 2>/dev/null || stat -f '%g' /var/run/docker.sock 2>/dev/null)

if [ -z "$DOCKER_GID" ]; then
    echo "❌ Error: Could not detect Docker socket GID"
    exit 1
fi

echo "✅ Detected Docker GID: $DOCKER_GID"

# Check if .env exists
if [ ! -f "$ENV_FILE" ]; then
    echo "📝 Creating .env from env.example..."
    cp "$PROJECT_ROOT/env.example" "$ENV_FILE"
fi

# Update or add DOCKER_GID in .env
if grep -q "^DOCKER_GID=" "$ENV_FILE"; then
    # Update existing DOCKER_GID
    if [[ "$OSTYPE" == "darwin"* ]]; then
        # macOS sed syntax
        sed -i '' "s/^DOCKER_GID=.*/DOCKER_GID=$DOCKER_GID/" "$ENV_FILE"
    else
        # Linux sed syntax
        sed -i "s/^DOCKER_GID=.*/DOCKER_GID=$DOCKER_GID/" "$ENV_FILE"
    fi
    echo "✅ Updated DOCKER_GID=$DOCKER_GID in .env"
elif grep -q "^# DOCKER_GID=" "$ENV_FILE"; then
    # Uncomment and set DOCKER_GID
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/^# DOCKER_GID=.*/DOCKER_GID=$DOCKER_GID/" "$ENV_FILE"
    else
        sed -i "s/^# DOCKER_GID=.*/DOCKER_GID=$DOCKER_GID/" "$ENV_FILE"
    fi
    echo "✅ Set DOCKER_GID=$DOCKER_GID in .env"
else
    # Add DOCKER_GID to .env
    echo "" >> "$ENV_FILE"
    echo "# Auto-detected Docker GID" >> "$ENV_FILE"
    echo "DOCKER_GID=$DOCKER_GID" >> "$ENV_FILE"
    echo "✅ Added DOCKER_GID=$DOCKER_GID to .env"
fi

echo ""
echo "🎉 Docker GID configuration complete!"
echo ""
echo "Next steps:"
echo "  1. Review your .env file: cat .env"
echo "  2. Start/restart the container: docker compose up -d"
echo "  3. Check Docker plugin status in the UI"
echo ""

