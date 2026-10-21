#!/usr/bin/env bash
# Runs scripts/dev/pane-phone-check.py (Pixel 7 emulation, real touch events) against a
# throwaway debug server on 127.0.0.1:18081 and a real SSH target. Needs `target/debug/server`
# and `ui/build`, python3 with `playwright`, and a Chromium (CHROMIUM=/path to use your own).
#
#   E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 scripts/dev/run-phone-check.sh
set -u
cd "$(dirname "$0")/../.."
: "${E2E_SSH_HOST:?set E2E_SSH_HOST to a host the key can log in to}"
export E2E_SSH_HOST E2E_SSH_USER="${E2E_SSH_USER:-root}" E2E_SSH_KEY="${E2E_SSH_KEY:-$HOME/.ssh/id_ed25519}"
export E2E_URL=http://127.0.0.1:18081
PYTHON="${PYTHON:-python3}"
WORK="$(mktemp -d)"
trap 'kill "$SPID" 2>/dev/null; wait "$SPID" 2>/dev/null; rm -rf "$WORK"' EXIT

BACKUP_DIR="$WORK/backups" DATABASE_URL="sqlite:$WORK/phone.db" ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
ENCRYPTION_KEY="$($PYTHON -c 'import secrets; print(secrets.token_hex(32))')" \
SESSION_SECURE=false SPA_DIR=ui/build RUST_LOG=info \
target/debug/server --addr 127.0.0.1:18081 > "$WORK/server.log" 2>&1 &
SPID=$!
for _ in $(seq 1 40); do curl -sf "$E2E_URL/api/v1/health" >/dev/null && break; sleep 0.5; done
kill -0 "$SPID" 2>/dev/null || { echo "server died:"; tail -20 "$WORK/server.log"; exit 1; }

$PYTHON scripts/dev/pane-phone-check.py
status=$?
echo "--- server log errors:"; grep -i " error\|panic" "$WORK/server.log" | head -5
exit $status
