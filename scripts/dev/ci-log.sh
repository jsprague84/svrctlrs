#!/usr/bin/env bash
# A commit's CI jobs, or one job's whole log — a green job's warnings too, which ci-watch.sh
# never shows. Without a job, lists every job with its state; with one (any part of its name,
# any case), prints that job's log.
#
#   scripts/dev/ci-log.sh [sha] [job]     e.g. ci-log.sh HEAD lint · ci-log.sh 4bd032d "deploy"
set -euo pipefail
cd "$(dirname "$0")/../.."
. scripts/dev/forgejo.sh
SHA=$(git rev-parse "${1:-HEAD}")
jobs=$(jobs_of "${SHA}")
[ -n "${jobs}" ] || { echo "ci-log: no CI runs for ${SHA:0:7} (pushed? docs only?)" >&2; exit 1; }
if [ -z "${2:-}" ]; then
  awk -F'|' '{ printf "%-9s %-10s %-20s %s\n", $1, $2, $3, $4 }' <<<"${jobs}"; exit 0
fi
# the same commit pushed to two branches runs CI twice: keep each name's newest job
match=$(grep -iF -- "$2" <<<"$(cut -d'|' -f1,4 <<<"${jobs}")" | sort -t'|' -k1,1nr | awk -F'|' '!seen[$2]++' || true)
case $(grep -c . <<<"${match}") in
  0) echo "ci-log: no job of ${SHA:0:7} is named like \"$2\"" >&2; exit 1 ;;
  1) job_log "${match%%|*}" ;;
  *) echo "ci-log: \"$2\" names more than one job:" >&2; sed 's/^/  /' <<<"${match}" >&2; exit 1 ;;
esac
