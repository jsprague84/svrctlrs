# shellcheck shell=bash
# Sourced by the ci-*.sh scripts: the Forgejo API for this repository, with the token in
# ~/.config/forgejo/api-token (docs/DEVELOPMENT.md, "The loop"). Not run on its own.
# Copied from LogosOS (apps stay independent; methods are copied).
TOKEN=$(cat ~/.config/forgejo/api-token)
API="https://git.js-node.cc/api/v1/repos/jsprague/svrctlrs"

# GET a path under the repository's API (e.g. "actions/runs?head_sha=…")
fj() { curl -sf -H "Authorization: token ${TOKEN}" "${API}/$1"; }
# POST, with an optional JSON body; prints the answer, fails on an HTTP error
fj_post() { curl -sf -X POST -H "Authorization: token ${TOKEN}" -H 'Content-Type: application/json' -d "${2:-{\}}" "${API}/$1"; }

# Every job of every workflow run for a commit, one per line: "<id>|<status>|<workflow>|<name>|<url>"
jobs_of() {
  local run
  for run in $(fj "actions/runs?head_sha=$1" | jq -r '.workflow_runs[]? | "\(.id)|\(.path // .workflow_id // "")"'); do
    fj "actions/runs/${run%%|*}/jobs" \
      | jq -r --arg wf "${run#*|}" '.[]? | "\(.id)|\(.status)|\($wf | sub(".*/"; "") | sub("@.*"; ""))|\(.name)|\(.html_url)"'
  done
}

# A job's log without the runner's timestamps and colour codes, so it reads in a terminal and greps
job_log() { fj "actions/jobs/$1/logs" | cut -c 30- | sed -E 's/\x1b\[[0-9;]*[A-Za-z]//g'; }
