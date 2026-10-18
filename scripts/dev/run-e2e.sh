#!/usr/bin/env bash
# Runs scripts/dev/pty-session-e2e.py against a throwaway debug server (fresh database per
# run, 3 s detach TTL) and a real SSH target, once with binary frames and once with the legacy
# JSON frames. Needs `target/debug/server` (cargo build -p server) and `ui/build` (npm run
# build) to exist, and python3 with the `websockets` package (PYTHON=... to point at one).
#
#   E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 scripts/dev/run-e2e.sh
#   scripts/dev/run-e2e.sh binary      # only the binary-frame pass
set -u
cd "$(dirname "$0")/../.."
: "${E2E_SSH_HOST:?set E2E_SSH_HOST to a host the key can log in to}"
export E2E_SSH_HOST E2E_SSH_USER="${E2E_SSH_USER:-root}" E2E_SSH_KEY="${E2E_SSH_KEY:-$HOME/.ssh/id_ed25519}"
PYTHON="${PYTHON:-python3}"
WORK="$(mktemp -d)"
trap 'kill "$SPID" 2>/dev/null; wait "$SPID" 2>/dev/null; rm -rf "$WORK"' EXIT

DATABASE_URL="sqlite:$WORK/e2e.db" ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
ENCRYPTION_KEY="$($PYTHON -c 'import secrets; print(secrets.token_hex(32))')" \
SESSION_SECURE=false SESSION_DETACH_TTL_SECS=3 SPA_DIR=ui/build RUST_LOG=info \
target/debug/server --addr 127.0.0.1:18080 > "$WORK/server.log" 2>&1 &
SPID=$!
for _ in $(seq 1 40); do curl -sf http://127.0.0.1:18080/api/v1/health >/dev/null && break; sleep 0.5; done
kill -0 "$SPID" 2>/dev/null || { echo "server died:"; tail -20 "$WORK/server.log"; exit 1; }

status=0
if [ "${1:-both}" != legacy ]; then
  echo "=== binary frames"
  $PYTHON scripts/dev/pty-session-e2e.py || status=1
fi
if [ "${1:-both}" != binary ]; then
  echo "=== legacy JSON frames"
  E2E_BINARY=0 $PYTHON scripts/dev/pty-session-e2e.py || status=1
fi
echo "--- server log errors:"; grep -i "error\|panic" "$WORK/server.log" | grep -v "Password authentication failed\|Connection test failed" | head -5
exit $status
