#!/usr/bin/env bash
# Run docker.yaml or desktop.yaml for a branch on demand (their workflow_dispatch). Off master
# and tags, docker.yaml pushes SHA-tagged images only (never :latest, never a deploy) and
# desktop.yaml keeps its packages as a workflow artifact (never a release): for trying a
# branch's image by hand, or checking every runner label after a Forgejo/runner upgrade
# (ci.yaml already runs on every branch push). Copied from LogosOS.
#
#   scripts/dev/ci-dispatch.sh [branch] [docker.yaml|desktop.yaml]   (default: this branch, docker.yaml)
set -euo pipefail
cd "$(dirname "$0")/../.."
. scripts/dev/forgejo.sh
REF="${1:-$(git branch --show-current)}"
WF="${2:-docker.yaml}"
[ "${REF}" != master ] || { echo "ci-dispatch: master builds and deploys on push; push it" >&2; exit 1; }
[ -f ".forgejo/workflows/${WF}" ] || { echo "ci-dispatch: no workflow .forgejo/workflows/${WF}" >&2; exit 1; }
git ls-remote --exit-code --heads origin "${REF}" >/dev/null \
  || { echo "ci-dispatch: ${REF} is not on origin (git push -u origin ${REF})" >&2; exit 1; }
fj_post "actions/workflows/${WF}/dispatches" "{\"ref\":\"${REF}\",\"return_run_info\":true}" \
  | jq -r '"dispatched '"${WF}"' run \(.run_number // "?") on '"${REF}"': \(.jobs // [] | join(", "))"'
echo "follow it: scripts/dev/ci-log.sh $(git rev-parse --short "origin/${REF}")"
