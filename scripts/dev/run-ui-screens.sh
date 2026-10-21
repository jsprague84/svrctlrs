#!/usr/bin/env bash
# Screenshots of every screen (scripts/dev/ui-screens.py) against a throwaway debug server on
# 127.0.0.1:18082. Needs `target/debug/server`, `ui/build`, python3 with `playwright`, and a
# Chromium (CHROMIUM=/path). E2E_SSH_HOST/USER/KEY optional (one live server); E2E_OUT=dir.
set -u
cd "$(dirname "$0")/../.."
export E2E_URL=http://127.0.0.1:18082
PYTHON="${PYTHON:-python3}"
WORK="$(mktemp -d)"
trap 'kill "$SPID" 2>/dev/null; wait "$SPID" 2>/dev/null; rm -rf "$WORK"' EXIT

BACKUP_DIR="$WORK/backups" DATABASE_URL="sqlite:$WORK/screens.db" ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
ENCRYPTION_KEY="$($PYTHON -c 'import secrets; print(secrets.token_hex(32))')" \
SESSION_SECURE=false SPA_DIR=ui/build RUST_LOG=warn \
target/debug/server --addr 127.0.0.1:18082 > "$WORK/server.log" 2>&1 &
SPID=$!
for _ in $(seq 1 40); do curl -sf "$E2E_URL/api/v1/health" >/dev/null && break; sleep 0.5; done
kill -0 "$SPID" 2>/dev/null || { echo "server died:"; tail -20 "$WORK/server.log"; exit 1; }

$PYTHON scripts/dev/ui-screens.py
