#!/usr/bin/env bash
# Follow a pushed commit through epyc-ai CI to production (docs/DEVELOPMENT.md, "The loop"):
# prints each job as it finishes and exits 0 once every job the push runs is green and, when
# the push deploys, production's health check reports the commit and hermes's agent interface
# (~/svrctlrs-eval/REVISION) is that commit too; 1 when a job fails (with the
# end of each failed job's log, CI_WATCH_LOG_LINES, default 60, and how to read all of it or
# re-run it); 2 on timeout (CI_WATCH_MIN, default 30).
#
#   scripts/dev/ci-watch.sh [sha]        (default HEAD)
#
# What a push runs: ci.yaml (Lint, Audit, Test, Frontend) on every branch; on master also
# docker.yaml (Build & Push, Deploy to docker-vm, Agent interface on hermes), unless
# everything the push changed is in its paths-ignore (docs/, root *.md, .forgejo/ *.md, the
# eval README, .claude/, LICENSE, ci.yaml, desktop.yaml; not skills/). Forgejo judges that
# over the whole push, so this judges it over everything since the revision production runs.
# Only "(push)" statuses count, as in the deploy gate: a PR run on the same commit does not.
#
# Needs the Forgejo token in ~/.config/forgejo/api-token. The health check is public; hermes's
# REVISION is read over ssh (AGENTS_HOST, default hermes). When hermes cannot be reached, or
# was never synced (no REVISION: the key is not set up), it says so and does not wait for it.
set -uo pipefail
cd "$(dirname "$0")/../.." || exit 1
SHA=$(git rev-parse "${1:-HEAD}")
. scripts/dev/forgejo.sh
HEALTH="${HEALTH_URL:-https://svrctlrs.js-node.app/api/v1/health}"
DEADLINE=$(($(date +%s) + ${CI_WATCH_MIN:-30} * 60))
CI_JOBS=("CI / Lint" "CI / Audit" "CI / Test" "CI / Frontend")
DEPLOY_JOBS=("Docker / Build & Push" "Docker / Deploy to docker-vm" "Docker / Agent interface on hermes")
AGENTS_HOST="${AGENTS_HOST:-hermes}"
# docker.yaml's paths-ignore, as one regex over a changed path
DOCKER_IGNORES='^([^/]+\.md|\.forgejo/.*\.md|scripts/eval/hermes/README\.md|docs/.*|\.claude/.*|LICENSE.*|\.forgejo/workflows/(ci|desktop)\.yaml)$'

live_rev() { curl -sf --max-time 10 "${HEALTH}" | jq -r '.revision // empty' 2>/dev/null || true; }
# hermes's synced revision; "none" when it was never synced, empty when hermes cannot be asked
agents_rev() {
  ssh -o BatchMode=yes -o ConnectTimeout=5 "${AGENTS_HOST}" \
    'cat svrctlrs-eval/REVISION 2>/dev/null || echo none' 2>/dev/null || true
}

git fetch -q origin
[ -n "$(git branch -r --contains "${SHA}" 2>/dev/null)" ] \
  || { echo "ci-watch: ${SHA:0:7} is on no branch of origin: push it first" >&2; exit 1; }

want=("${CI_JOBS[@]}")
deploys=""
live=$(live_rev)
if git merge-base --is-ancestor "${SHA}" origin/master; then
  if [ "${live}" = "${SHA}" ]; then
    agents=$(agents_rev)
    case "${agents}" in
      "${SHA}") h="hermes's agent interface too" ;;
      none) h="hermes never synced" ;;
      "") h="hermes not reachable from here" ;;
      *) h="hermes's agent interface is ${agents:0:7}: scripts/dev/ci-rerun.sh ${SHA:0:7} 'Agent interface'" ;;
    esac
    echo "${SHA:0:7}: already live (${HEALTH}); ${h}"; exit 0
  elif [ -n "${live}" ] && git merge-base --is-ancestor "${SHA}" "${live}" 2>/dev/null; then
    echo "${SHA:0:7}: production runs ${live:0:7}, which contains it"; exit 0
  elif [ -n "${live}" ] && git merge-base --is-ancestor "${live}" "${SHA}" 2>/dev/null; then
    changed=$(git diff --name-only "${live}" "${SHA}")
  else
    changed=$(git diff-tree --no-commit-id --name-only -r "${SHA}")
  fi
  if grep -vqE "${DOCKER_IGNORES}" <<<"${changed}"; then
    deploys=yes
    want+=("${DEPLOY_JOBS[@]}")
  else
    echo "${SHA:0:7}: only docs or CI-only workflows since ${live:0:7}: CI runs, nothing builds or deploys"
  fi
else
  echo "${SHA:0:7}: not on master: CI only, no image, no deploy"
fi

failed_logs() {
  jobs_of "${SHA}" | awk -F'|' '$2 == "failure" || $2 == "cancelled"' \
    | while IFS='|' read -r job _ _ name url; do
        echo "--- ${name}: ${url} (last ${CI_WATCH_LOG_LINES:-60} lines)"
        job_log "${job}" | tail -n "${CI_WATCH_LOG_LINES:-60}"
      done
  echo "--- all of a log: scripts/dev/ci-log.sh ${SHA:0:7} <job> · the runner's fault, not the code's: scripts/dev/ci-rerun.sh ${SHA:0:7}"
  tip=$(git rev-parse origin/master)
  if [ -n "${deploys}" ] && [ "${tip}" != "${SHA}" ]; then
    echo "--- master has moved on to ${tip:0:7}; a newer push cancels this one's image build: watch that one"
  fi
}

prev=""
while :; do
  # one line per job: "<context>|<state>"; context names contain spaces, so never split on them
  states=$(fj "commits/${SHA}/status" \
    | jq -r '.statuses[]? | select(.context | endswith(" (push)")) | "\(.context | sub(" \\(push\\)$"; ""))|\(.status)"' \
    | sort -u)
  finished=$(grep -vE '\|pending$' <<<"${states}" | grep -v '^$' || true)
  new=$(comm -13 <(echo "${prev}") <(echo "${finished}") | grep -v '^$' || true)
  [ -n "${new}" ] && sed "s/^/${SHA:0:7} /" <<<"${new}"
  prev="${finished}"
  if grep -qE '\|(failure|error|cancelled)$' <<<"${states}"; then
    echo "${SHA:0:7}: a job failed"; failed_logs; exit 1
  fi
  missing=()
  for job in "${want[@]}"; do
    grep -qxF "${job}|success" <<<"${states}" || missing+=("${job}")
  done
  if [ "${#missing[@]}" -eq 0 ]; then
    [ -z "${deploys}" ] && { echo "${SHA:0:7}: every job green (nothing to deploy)"; exit 0; }
    live=$(live_rev)
    if [ "${live}" = "${SHA}" ]; then
      agents=$(agents_rev)
      case "${agents}" in
        "${SHA}") echo "${SHA:0:7}: every job green; production and hermes's agent interface run it"; exit 0 ;;
        none) echo "${SHA:0:7}: every job green; production runs it (hermes was never synced: docs/DEPLOYMENT.md, \"Agent interface on hermes\")"; exit 0 ;;
        "") echo "${SHA:0:7}: every job green; production runs it (hermes not reachable from here: its REVISION unchecked)"; exit 0 ;;
      esac
      missing=("hermes's agent interface (${agents:0:7})")
    else
      missing=("production (runs ${live:0:7})")
    fi
  fi
  if [ "$(date +%s)" -ge "${DEADLINE}" ]; then
    printf '%s: not done after %s min; waiting on: %s\n' "${SHA:0:7}" "${CI_WATCH_MIN:-30}" "$(IFS=,; echo "${missing[*]}")"
    exit 2
  fi
  sleep 20
done
