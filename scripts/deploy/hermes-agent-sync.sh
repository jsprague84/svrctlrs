#!/usr/bin/env bash
# The door svrctlrs's agent interface goes through to hermes (ROADMAP 31; docs/agents.md,
# "Kept current"). Runs ON HERMES, where Hermes and AgentOS's Claude Code use `svrctl`.
#
#   hermes-agent-sync.sh <40-hex sha>              install what production runs (CI after the
#                                                  deploy; by hand, a repair)
#   hermes-agent-sync.sh --check <SKILL.md> [svrctl]   build and check the copies from these
#                                                  files, install nothing (prepush, CI's Lint)
#
# CI's key (repository secret HERMES_AGENT_SYNC_KEY) is bound to this file alone in
# ~/.ssh/authorized_keys on hermes:
#   restrict,command="/home/ubuntu/bin/svrctlrs-agent-sync.sh" ssh-ed25519 … svrctlrs-ci-agent-sync
# The SHA arrives in SSH_ORIGINAL_COMMAND and is checked as data, never run. NOTHING is read from
# stdin. The script refuses unless production (its public health check) reports that SHA, then
# takes `svrctl` from that commit's image in the registry (its revision label checked) and the
# skill and eval harness from the repository at that commit. So whoever holds the key can do no
# more than re-install what is already deployed. This file is copied to hermes by hand
# (docs/DEPLOYMENT.md, "Agent interface on hermes"), so CI cannot change what its own key may do.
#
# What it installs (each only where its folder already exists; the previous one kept beside it):
#   ~/.hermes/skills/devops/svrctlrs/SKILL.md          Hermes (both profiles share this folder)
#   ~/.claude/skills/svrctlrs/SKILL.md                 Claude Code on hermes (AgentOS's rhythms)
#   ~/svrctlrs-eval/bin/svrctl                         the binary the skills name (svrctl.prev)
#   ~/svrctlrs-eval/{run.sh,…,tasks/N.txt,skill/}      the eval harness, with a plain skill copy
#   ~/svrctlrs-eval/REVISION                           the SHA, written last (ci-watch.sh reads it)
# Each skill copy keeps its own local part: a "This deployment:" paragraph, or a "## Local
# deployment…" section (Hermes's own notes, with its references/ folder, which is never touched).
# It lives beside the copy in .deployment.md, taken from the installed copy the first time, and
# goes in before "## Setup". Edit .deployment.md, not SKILL.md: the next sync replaces SKILL.md.
set -Eeuo pipefail
umask 022

HEALTH="${SVRCTLRS_HEALTH:-https://svrctlrs.js-node.app/api/v1/health}"
FORGE="${SVRCTLRS_FORGE:-https://git.js-node.cc/jsprague/svrctlrs}"
IMAGE="${SVRCTLRS_IMAGE:-git.js-node.cc/jsprague/svrctlrs}"
EVAL="${SVRCTLRS_EVAL_DIR:-$HOME/svrctlrs-eval}"
IFS=: read -r -a SKILL_DIRS <<<"${SVRCTLRS_SKILL_DIRS:-$HOME/.hermes/skills/devops/svrctlrs:$HOME/.hermes/profiles/local-qwen/skills/devops/svrctlrs:$HOME/.claude/skills/svrctlrs}"

die() { printf '✗ %s\n' "$*" >&2; exit 1; }
usage() { printf 'usage: %s <40-hex sha> | --check <SKILL.md> [svrctl]\n' "$0" >&2; exit 64; }

mode=install rev="" src_skill="" src_svrctl=""
if [[ -n "${SSH_ORIGINAL_COMMAND:-}" ]]; then
  (($# == 0)) && [[ "${SSH_ORIGINAL_COMMAND}" =~ ^[0-9a-f]{40}$ ]] || usage
  rev="${SSH_ORIGINAL_COMMAND}"
elif (($# == 1)) && [[ "$1" =~ ^[0-9a-f]{40}$ ]]; then
  rev="$1"
elif (($# == 2 || $# == 3)) && [[ "$1" == --check && -f "$2" ]]; then
  mode=check src_skill="$2" src_svrctl="${3:-}"
  [ -z "${src_svrctl}" ] || [ -f "${src_svrctl}" ] || usage
else
  usage
fi
readonly mode rev

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# local_part <installed SKILL.md>: its "## Local deployment…" section (to the next "## ") or its
# "This deployment:" paragraph (to the next blank line), without trailing blank lines
local_part() {
  awk '
    /^## Local deployment/ { sec = 1; print; next }
    sec && /^## / { exit }
    sec { print; next }
    /^This deployment:/ { par = 1 }
    par && /^$/ { exit }
    par { print }
  ' "$1" | awk '{ lines[NR] = $0 } END { n = NR; while (n > 0 && lines[n] == "") n--; for (i = 1; i <= n; i++) print lines[i] }'
}

# skill_with <SKILL.md> <local part file> <out>: the skill with the local part (if any) before
# "## Setup", and checked
skill_with() {
  local src="$1" side="$2" dest="$3"
  awk -v side="${side}" '
    /^## Setup/ && !done {
      while ((getline l < side) > 0) { print l; any = 1 }
      if (any) print ""
      done = 1
    }
    { print }
  ' "${src}" > "${dest}"
  if [ -s "${side}" ]; then
    grep -qxF "$(head -n 1 "${side}")" "${dest}" || die "$(basename "$(dirname "${dest}")"): the local part did not go in"
  fi
}

# check_skill <SKILL.md>: it is this skill, and it has the one "## Setup" the local part goes before
check_skill() {
  grep -qx 'name: svrctlrs' "$1" || die "$1 is not the svrctlrs skill"
  [ "$(grep -c '^## Setup' "$1")" = 1 ] || die "$1 must have exactly one \"## Setup\" heading (the local part goes before it)"
  ! grep -q '^This deployment:\|^## Local deployment' "$1" || die "$1 carries a deployment's local part: those live on hermes, in .deployment.md"
}

if [ "${mode}" = check ]; then
  check_skill "${src_skill}"
  printf 'This deployment: SVRCTLRS_URL=https://svrctlrs.example\n' > "${work}/paragraph"
  printf '## Local deployment and retained notes\n\nRead references/local-operations.md first.\n' > "${work}/section"
  for side in paragraph section; do
    skill_with "${src_skill}" "${work}/${side}" "${work}/SKILL.${side}.md"
    [ "$(grep -c '^## Setup' "${work}/SKILL.${side}.md")" = 1 ] || die "the ${side} broke the Setup heading"
  done
  if [ -n "${src_svrctl}" ]; then
    "${src_svrctl}" --version >/dev/null || die "${src_svrctl} does not run here"
  fi
  echo "agent interface: the skill copies build and check (nothing installed)"
  exit 0
fi

command -v docker >/dev/null && command -v curl >/dev/null && command -v jq >/dev/null \
  || die "needs docker, curl and jq"

# one sync at a time
exec 9>"${XDG_RUNTIME_DIR:-/tmp}/svrctlrs-agent-sync.lock"
flock -w 300 9 || die "another sync did not finish within 5 minutes"

running="$(curl -fsS -m 10 "${HEALTH}" | jq -r '.revision // empty')" || die "production does not answer at ${HEALTH}"
[ "${running}" = "${rev}" ] || die "production runs ${running:-nothing}, not ${rev}: the agent interface goes with what is deployed"

# the skill and the eval harness from the repository at that commit (at most 64 MB, only these)
curl -fsS -m 120 "${FORGE}/archive/${rev}.tar.gz" | head -c 64000000 \
  | tar -xz -C "${work}" --no-same-owner --no-same-permissions \
    svrctlrs/skills/svrctlrs/SKILL.md svrctlrs/scripts/eval/hermes \
  || die "could not read ${rev} from ${FORGE}"
src="${work}/svrctlrs"
[ -f "${src}/skills/svrctlrs/SKILL.md" ] && [ ! -L "${src}/skills/svrctlrs/SKILL.md" ] || die "the commit has no skill"
check_skill "${src}/skills/svrctlrs/SKILL.md"

# svrctl from that commit's image: created, never run; the pull removed again unless it was here
had_image=yes
docker image inspect "${IMAGE}:${rev}" >/dev/null 2>&1 || had_image=""
docker pull -q "${IMAGE}:${rev}" >/dev/null || die "could not pull ${IMAGE}:${rev}"
label="$(docker image inspect "${IMAGE}:${rev}" --format '{{ index .Config.Labels "org.opencontainers.image.revision" }}')"
[ "${label}" = "${rev}" ] || die "the image ${IMAGE}:${rev} is revision ${label:-<none>}"
id="$(docker create "${IMAGE}:${rev}")"
docker cp -q "${id}:/app/svrctl" "${work}/svrctl.bin" || { docker rm "${id}" >/dev/null; die "the image has no /app/svrctl"; }
docker rm "${id}" >/dev/null
[ -n "${had_image}" ] || docker image rm -f "${IMAGE}:${rev}" >/dev/null 2>&1 || true
chmod 755 "${work}/svrctl.bin"
version="$("${work}/svrctl.bin" --version)" || die "svrctl does not run here"
said="$("${work}/svrctl.bin" --url "${HEALTH%/api/v1/health}" --json health | jq -r '"svrctl " + .version')" \
  || die "svrctl cannot reach production"
[ "${said}" = "${version}" ] || die "svrctl is ${version}, production says ${said}"

# no REVISION until this install is complete
rm -f "${EVAL}/REVISION"

put() { # put <file> <dest> <mode>: a rename in the destination's folder, never half a file
  install -m "$3" "$1" "$2.new" && mv -f "$2.new" "$2"
}

declare -A done_dirs=()
for dir in "${SKILL_DIRS[@]}"; do
  [ -d "${dir}" ] || { echo "  skip ${dir} (not there)"; continue; }
  real="$(readlink -f "${dir}")"
  [ -z "${done_dirs[${real}]:-}" ] || continue
  done_dirs[${real}]=1
  side="${real}/.deployment.md"
  if [ ! -f "${side}" ] && [ -f "${real}/SKILL.md" ]; then
    local_part "${real}/SKILL.md" > "${side}.new" && mv -f "${side}.new" "${side}"
    echo "  ${dir}: kept its local part in .deployment.md ($(wc -l < "${side}") lines)"
  fi
  [ -f "${side}" ] || : > "${side}"
  skill_with "${src}/skills/svrctlrs/SKILL.md" "${side}" "${work}/SKILL.out.md"
  if cmp -s "${work}/SKILL.out.md" "${real}/SKILL.md"; then
    echo "  skill: ${dir} (unchanged)"
    continue
  fi
  [ -f "${real}/SKILL.md" ] && cp -p "${real}/SKILL.md" "${real}/SKILL.md.bak"
  put "${work}/SKILL.out.md" "${real}/SKILL.md" 644
  echo "  skill: ${dir}"
done

mkdir -p "${EVAL}/bin"
if ! cmp -s "${work}/svrctl.bin" "${EVAL}/bin/svrctl"; then
  [ -f "${EVAL}/bin/svrctl" ] && cp -p "${EVAL}/bin/svrctl" "${EVAL}/bin/svrctl.prev"
  put "${work}/svrctl.bin" "${EVAL}/bin/svrctl" 755
fi
echo "  binary: ${EVAL}/bin/svrctl (${version})"

# the eval harness: only these names, as plain files
h="${src}/scripts/eval/hermes"
for f in run.sh seed.py grade.py summarize.py compose.yml ssh_config README.md; do
  [ -f "${h}/${f}" ] && [ ! -L "${h}/${f}" ] || continue
  case "${f}" in *.sh | *.py) m=755 ;; *) m=644 ;; esac
  put "${h}/${f}" "${EVAL}/${f}" "${m}"
done
mkdir -p "${EVAL}/tasks" "${EVAL}/skill"
n=0
for t in "${h}"/tasks/*.txt; do
  name="$(basename "${t}")"
  [[ "${name}" =~ ^[0-9]+\.txt$ ]] && [ -f "${t}" ] && [ ! -L "${t}" ] || continue
  put "${t}" "${EVAL}/tasks/${name}" 644
  n=$((n + 1))
done
put "${src}/skills/svrctlrs/SKILL.md" "${EVAL}/skill/SKILL.md" 644
echo "  eval: ${EVAL} (${n} tasks, plain skill in skill/)"

printf '%s\n' "${rev}" > "${EVAL}/REVISION.new" && mv -f "${EVAL}/REVISION.new" "${EVAL}/REVISION"
echo "agent interface ${rev:0:12} on hermes: skills, svrctl and the eval harness"
