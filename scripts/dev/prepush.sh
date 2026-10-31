#!/usr/bin/env bash
# The laptop's part before a push (docs/DEVELOPMENT.md, "The loop"): seconds, not minutes.
# Formatting for the workspace, the agent-path check (scripts/ci/agent-path.sh) for the commits
# not pushed yet; svelte-check and Prettier's check when ui/ changed (ESLint is CI's and
# gates.sh's). Clippy, tests, audits and the image run on epyc-ai after the push, and
# nothing deploys unless all of it is green: follow it with scripts/dev/ci-watch.sh.
# scripts/dev/gates.sh runs every gate here, for when CI is down.
set -euo pipefail
cd "$(dirname "$0")/../.."
git fetch -q origin master
base=$(git merge-base origin/master HEAD)
nix shell nixpkgs#cargo nixpkgs#rustfmt --command cargo fmt --all --check
scripts/ci/agent-path.sh "${base}" HEAD
# ui/src-tauri is Rust outside the workspace: not the SPA
if git diff --name-only "${base}" HEAD | grep -v '^ui/src-tauri/' | grep -q '^ui/'; then
  (
    cd ui
    [ node_modules/.package-lock.json -nt package-lock.json ] || npm ci --no-audit --no-fund
    npm run check --silent && npx prettier --check . >/dev/null
  )
fi
echo "prepush ok"
