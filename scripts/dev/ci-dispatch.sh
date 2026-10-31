#!/usr/bin/env bash
# Run docker.yaml or desktop.yaml for a branch on demand (their workflow_dispatch). Off master
# and tags, docker.yaml pushes SHA-tagged images only (never :latest, never a deploy) and
# desktop.yaml keeps its packages as a workflow artifact (never a release): for trying a
# branch's image by hand, or checking every runner label after a Forgejo/runner upgrade
# (ci.yaml already runs on every branch push). On master, docker.yaml builds :latest and
# DEPLOYS (after its CI gate): for a push it skipped because only ci.yaml or docs changed, when
# production should still get the commit. Copied from LogosOS.
#
#   scripts/dev/ci-dispatch.sh [branch] [docker.yaml|desktop.yaml]   (default: this branch, docker.yaml)
set -euo pipefail
cd "$(dirname "$0")/../.."
. scripts/dev/forgejo.sh
REF="${1:-$(git branch --show-current)}"
WF="${2:-docker.yaml}"
[ "${REF}" != master ] || [ "${WF}" = docker.yaml ] \
  || { echo "ci-dispatch: ${WF} on master runs from a tag, not by hand" >&2; exit 1; }
[ "${REF}" != master ] || echo "ci-dispatch: master: this builds :latest and deploys $(git rev-parse --short origin/master)"
[ -f ".forgejo/workflows/${WF}" ] || { echo "ci-dispatch: no workflow .forgejo/workflows/${WF}" >&2; exit 1; }
git ls-remote --exit-code --heads origin "${REF}" >/dev/null \
  || { echo "ci-dispatch: ${REF} is not on origin (git push -u origin ${REF})" >&2; exit 1; }
fj_post "actions/workflows/${WF}/dispatches" "{\"ref\":\"${REF}\",\"return_run_info\":true}" \
  | jq -r '"dispatched '"${WF}"' run \(.run_number // "?") on '"${REF}"': \(.jobs // [] | join(", "))"'
echo "follow it: scripts/dev/ci-log.sh $(git rev-parse --short "origin/${REF}")"
