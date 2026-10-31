#!/usr/bin/env bash
# Re-run a commit's failed jobs (or the one named), for a failure that was the runner's and not
# the code's: a network blip, a full disk. Each job re-runs with the jobs that need it. The
# deploy lives in the other workflow (docker.yaml) and gave up when CI went red, so when it
# failed it is re-run too, and waits for CI again. Then follow it with ci-watch.sh as after a
# push. Copied from LogosOS.
#
#   scripts/dev/ci-rerun.sh [sha] [job]
set -euo pipefail
cd "$(dirname "$0")/../.."
. scripts/dev/forgejo.sh
SHA=$(git rev-parse "${1:-HEAD}")
jobs=$(jobs_of "${SHA}")
if [ -n "${2:-}" ]; then
  pick=$(awk -F'|' -v want="$2" 'index(tolower($4), tolower(want))' <<<"${jobs}")
else
  pick=$(awk -F'|' '$2 == "failure" || $2 == "cancelled"' <<<"${jobs}")
fi
[ -n "${pick}" ] || { echo "ci-rerun: nothing to re-run for ${SHA:0:7}" >&2; exit 1; }
deploy=$(awk -F'|' '$4 == "Deploy to docker-vm" && ($2 == "failure" || $2 == "cancelled")' <<<"${jobs}")
while IFS='|' read -r id _ wf name _; do
  fj_post "actions/jobs/${id}/rerun" >/dev/null && echo "re-running ${wf}: ${name}"
done < <(printf '%s\n%s\n' "${pick}" "${deploy}" | grep . | sort -u)
echo "follow it: scripts/dev/ci-watch.sh ${SHA:0:7}"
