#!/usr/bin/env bash
# Create (or reuse) the Forgejo release for a tag and attach assets to it.
#
# Usage: scripts/ci/forgejo-release.sh <tag> [asset ...]
# Env:   TOKEN (required)  — Forgejo API token with write access to the repository
#        GITHUB_SERVER_URL, GITHUB_REPOSITORY — set by Forgejo Actions
#
# Three workflows (docker, android, desktop) run on the same tag and all call this, so it is
# idempotent: a 409 on create means another job won the race and the release is re-read; an
# asset that already exists is replaced, so a re-run uploads fresh files.
# Release notes are the matching "## [X.Y.Z]" section of CHANGELOG.md when there is one.
set -euo pipefail

TAG="${1:?usage: forgejo-release.sh <tag> [asset ...]}"
shift
: "${TOKEN:?TOKEN is required}"
API="${GITHUB_SERVER_URL%/}/api/v1/repos/${GITHUB_REPOSITORY}"
AUTH=(-H "Authorization: token ${TOKEN}")

command -v jq >/dev/null || { apt-get update -qq && apt-get install -y -qq jq; }

release_notes() {
  local version="${TAG#v}"
  [ -f CHANGELOG.md ] || { echo "SvrCtlRS ${TAG}"; return; }
  awk -v v="${version}" '
    /^## \[/ { if (found) exit; if (index($0, "[" v "]")) { found = 1; next } }
    found { print }
  ' CHANGELOG.md | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}'
}

get_release() {
  curl -fsS "${AUTH[@]}" "${API}/releases/tags/${TAG}" 2>/dev/null || true
}

RELEASE_JSON="$(get_release)"
if [ -z "${RELEASE_JSON}" ]; then
  NOTES="$(release_notes)"
  [ -n "${NOTES}" ] || NOTES="SvrCtlRS ${TAG}"
  PAYLOAD=$(jq -n --arg tag "${TAG}" --arg name "SvrCtlRS ${TAG}" --arg body "${NOTES}" \
    '{tag_name: $tag, name: $name, body: $body, draft: false, prerelease: false}')
  HTTP=$(curl -sS -o /tmp/release.json -w '%{http_code}' "${AUTH[@]}" \
    -H 'Content-Type: application/json' -X POST "${API}/releases" -d "${PAYLOAD}")
  case "${HTTP}" in
    201) RELEASE_JSON="$(cat /tmp/release.json)"; echo "created release ${TAG}" ;;
    409) RELEASE_JSON="$(get_release)"; echo "release ${TAG} already exists" ;;
    *)   echo "creating release failed (HTTP ${HTTP}): $(cat /tmp/release.json)" >&2; exit 1 ;;
  esac
else
  echo "release ${TAG} already exists"
fi

RELEASE_ID=$(jq -r '.id' <<<"${RELEASE_JSON}")
[ -n "${RELEASE_ID}" ] && [ "${RELEASE_ID}" != null ] || { echo "no release id for ${TAG}" >&2; exit 1; }

for ASSET in "$@"; do
  [ -f "${ASSET}" ] || { echo "skipping missing asset ${ASSET}" >&2; continue; }
  NAME=$(basename "${ASSET}")
  EXISTING=$(curl -fsS "${AUTH[@]}" "${API}/releases/${RELEASE_ID}/assets" \
    | jq -r --arg n "${NAME}" '.[] | select(.name == $n) | .id')
  for ID in ${EXISTING}; do
    curl -fsS "${AUTH[@]}" -X DELETE "${API}/releases/${RELEASE_ID}/assets/${ID}" >/dev/null
    echo "replaced existing asset ${NAME}"
  done
  curl -fsS "${AUTH[@]}" -X POST "${API}/releases/${RELEASE_ID}/assets?name=${NAME}" \
    -H 'Content-Type: application/octet-stream' --data-binary @"${ASSET}" >/dev/null
  echo "uploaded ${NAME} ($(du -h "${ASSET}" | cut -f1))"
done
