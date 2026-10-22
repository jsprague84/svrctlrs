#!/usr/bin/env bash
# Runs scripts/dev/pty-session-e2e.py against a throwaway debug server (fresh database per
# run, 3 s detach TTL) and a real SSH target, once with binary frames and once with the legacy
# JSON frames. Needs `target/debug/server` (cargo build -p server) and `ui/build` (npm run
# build) to exist, and python3 with the `websockets` package (PYTHON=... to point at one).
#
#   E2E_SSH_HOST=10.0.0.5 E2E_SSH_USER=ubuntu E2E_SSH_KEY=~/.ssh/id_ed25519 scripts/dev/run-e2e.sh
#   scripts/dev/run-e2e.sh binary      # only the binary-frame pass
#   scripts/dev/run-e2e.sh kept        # only the kept-shell pass (tmux on the target, a restart)
#   scripts/dev/run-e2e.sh backpressure  # burst/stall/Ctrl-C memory check (backpressure-check.py)
#   E2E_PORT=18090 scripts/dev/run-e2e.sh  # when 18080 is taken
set -u
cd "$(dirname "$0")/../.."
: "${E2E_SSH_HOST:?set E2E_SSH_HOST to a host the key can log in to}"
export E2E_SSH_HOST E2E_SSH_USER="${E2E_SSH_USER:-root}" E2E_SSH_KEY="${E2E_SSH_KEY:-$HOME/.ssh/id_ed25519}"
PYTHON="${PYTHON:-python3}"
PORT="${E2E_PORT:-18080}"
export E2E_URL="http://127.0.0.1:$PORT"
# Something else answering on the port would pass the health check and fail every request
if curl -s -o /dev/null "$E2E_URL/"; then
  echo "port $PORT is already in use; set E2E_PORT to a free one"; exit 1
fi
WORK="$(mktemp -d)"
trap 'kill "$SPID" 2>/dev/null; wait "$SPID" 2>/dev/null; rm -rf "$WORK"' EXIT
KEY="$($PYTHON -c 'import secrets; print(secrets.token_hex(32))')"

# Started again on the same database and key for the kept-shell pass's restart
start_server() {
  BACKUP_DIR="$WORK/backups" DATABASE_URL="sqlite:$WORK/e2e.db" ADMIN_USERNAME=admin ADMIN_PASSWORD=e2e-password-123456 \
  ENCRYPTION_KEY="$KEY" SESSION_SECURE=false SESSION_DETACH_TTL_SECS=3 SPA_DIR=ui/build RUST_LOG=info \
  target/debug/server --addr "127.0.0.1:$PORT" >> "$WORK/server.log" 2>&1 &
  SPID=$!
  for _ in $(seq 1 40); do curl -sf "$E2E_URL/api/v1/health" >/dev/null && break; sleep 0.5; done
  kill -0 "$SPID" 2>/dev/null || { echo "server died:"; tail -20 "$WORK/server.log"; exit 1; }
}
start_server

if [ "${1:-both}" = backpressure ]; then
  SERVER_PID=$SPID $PYTHON scripts/dev/backpressure-check.py
  exit $?
fi

status=0
# Kept shells (tmux on the target): a shell, a restart of the server, the same shell again
if [ "${1:-both}" = both ] || [ "${1:-both}" = kept ]; then
  echo "=== kept shell across a restart"
  if E2E_STATE="$WORK/kept.json" $PYTHON scripts/dev/kept-shell-e2e.py start; then
    kill -TERM "$SPID"; wait "$SPID" 2>/dev/null
    start_server
    E2E_STATE="$WORK/kept.json" $PYTHON scripts/dev/kept-shell-e2e.py resume || status=1
  else
    status=1
  fi
  [ "${1:-both}" = kept ] && { grep -i "error\|panic" "$WORK/server.log" | head -5; exit $status; }
fi
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
