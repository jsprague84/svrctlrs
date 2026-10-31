#!/usr/bin/env bash
# The agent path and the docs ship with the feature (the dev-space agentic-coding standard §7;
# CLAUDE.md "Conventions"). Copied from rustRoast's (apps stay independent; methods are copied).
#
#   scripts/ci/agent-path.sh [BEFORE] [AFTER]     (default: the last commit)
#
# For every `feat` commit in BEFORE..AFTER that changes what a person or an agent can do
# (server/src, api/src, core/src, database/src, database/migrations, ui/src):
#   - the agent path: it touches the skill (skills/svrctlrs/), docs/cli.md, docs/agents.md or
#     an eval task (scripts/eval/hermes/tasks/), or says why not in a trailer
#     `Agent-path: <reason>`;
#   - the docs: it touches docs/ or CHANGELOG.md, or says why not in `Docs: <reason>`.
# It proves the agent path was considered, not that it is complete: the parity test
# (server/src/bin/svrctl/surface.rs, docs/agents.md) and the Hermes eval decide that.
# prepush.sh runs it over the commits not pushed yet, CI's Lint over the pushed range.
set -euo pipefail
cd "$(dirname "$0")/../.."

AFTER="${2:-HEAD}"
BEFORE="${1:-}"
if [ -z "${BEFORE}" ] || [[ "${BEFORE}" =~ ^0+$ ]] || ! git cat-file -e "${BEFORE}^{commit}" 2>/dev/null; then
  RANGE=("${AFTER}" -1)
else
  RANGE=("${BEFORE}..${AFTER}")
fi

fail=0
checked=0
for c in $(git rev-list --no-merges "${RANGE[@]}"); do
  subject=$(git log -1 --format=%s "${c}")
  [[ "${subject}" =~ ^feat(\(|:|!) ]] || continue
  files=$(git diff-tree --no-commit-id --name-only -r "${c}")
  grep -qE '^(server/src/|api/src/|core/src/|database/src/|database/migrations/|ui/src/)' <<<"${files}" || continue
  checked=$((checked + 1))
  body=$(git log -1 --format=%B "${c}")
  short="${c:0:7} ${subject:0:70}"
  if ! grep -qE '^(skills/svrctlrs/|docs/cli\.md$|docs/agents\.md$|scripts/eval/hermes/tasks/)' <<<"${files}" \
    && ! grep -qiE '^Agent-path: .{8,}' <<<"${body}"; then
    echo "::error::${short}: a feature with no agent path. Touch the skill, docs/cli.md, docs/agents.md or an eval task, or add a trailer 'Agent-path: <why none>'."
    fail=1
  fi
  if ! grep -qE '^(docs/|CHANGELOG\.md$)' <<<"${files}" && ! grep -qiE '^Docs: .{8,}' <<<"${body}"; then
    echo "::error::${short}: a feature with no docs. Touch docs/ or CHANGELOG.md, or add a trailer 'Docs: <why none>'."
    fail=1
  fi
done
echo "agent path: ${checked} feature commit(s) checked"
exit "${fail}"
