#!/usr/bin/env bash
# Every CI gate (Lint, Audit, Test, Frontend) on this machine, stopping at the first failure.
# The usual loop is scripts/dev/prepush.sh, push, scripts/dev/ci-watch.sh (docs/DEVELOPMENT.md,
# "The loop"); use this when CI is down, or before pushing a migration or security code.
# Lint runs in CI's image (ci-lint.sh). The tests run with HOME pointed at an empty directory,
# as in CI, where there is no ~/.ssh: an SSH plan that finds the laptop's default key passes
# here and fails there (2026-10-19).
#
#   scripts/dev/gates.sh            the CI gates
#   scripts/dev/gates.sh --e2e      then run-e2e.sh against hermes (what CI cannot run: a real
#                                   OpenSSH, SFTP, the WebSocket), on port 18090
set -euo pipefail
cd "$(dirname "$0")/../.."
step() { printf '\n== %s\n' "$*"; }
rust() { nix shell nixpkgs#cargo nixpkgs#rustc nixpkgs#clippy nixpkgs#rustfmt nixpkgs#pkg-config nixpkgs#gcc --command "$@"; }
nohome=$(mktemp -d)
trap 'rm -rf "${nohome}"' EXIT

git fetch -q origin master
step "version";                  scripts/ci/check-version.sh
step "agent path";               scripts/ci/agent-path.sh "$(git merge-base origin/master HEAD)" HEAD
step "fmt, clippy (CI's image)"; scripts/dev/ci-lint.sh
step "cargo test (empty HOME)";  rust env HOME="${nohome}" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" cargo test --workspace
step "cargo audit";              nix shell nixpkgs#cargo nixpkgs#cargo-audit --command cargo audit
(
  cd ui
  step "ui: install";            npm ci --no-audit --no-fund
  step "ui: check";              npm run check
  step "ui: lint";               npm run lint
  step "ui: test";               npm run test
  step "ui: build";              npm run build
  step "ui: audit";              npm audit --audit-level=high
)
if [ "${1:-}" = --e2e ]; then
  step "e2e against ${E2E_SSH_HOST:-10.10.1.35}"
  rust cargo build -p server
  nix shell --impure --expr '(builtins.getFlake "nixpkgs").legacyPackages.${builtins.currentSystem}.python3.withPackages (p: [p.websockets p.playwright])' \
    --command env E2E_SSH_HOST="${E2E_SSH_HOST:-10.10.1.35}" E2E_SSH_USER="${E2E_SSH_USER:-ubuntu}" \
      E2E_SSH_KEY="${E2E_SSH_KEY:-$HOME/.ssh/id_ed25519}" E2E_PORT="${E2E_PORT:-18090}" scripts/dev/run-e2e.sh
fi
printf '\ngates ok\n'
