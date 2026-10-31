#!/usr/bin/env bash
# One version across every manifest (CLAUDE.md "Versions"; docs/DEVELOPMENT.md, "Releasing").
#
#   scripts/ci/check-version.sh            every manifest agrees with the workspace Cargo.toml
#   scripts/ci/check-version.sh vX.Y.Z     ... and with the tag, and CHANGELOG.md has its section
#
# CI's Lint and prepush.sh run the first form on every push; the release job runs the second on
# a tag. The manifests: the workspace Cargo.toml (its crates take version.workspace),
# ui/package.json, ui/src-tauri/tauri.conf.json, ui/src-tauri/Cargo.toml, and the version
# docs/openapi.json carries. Copied from LogosOS (apps stay independent; methods are copied).
set -euo pipefail
cd "$(dirname "$0")/../.."

WANT=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -n "${WANT}" ] || { echo "no [workspace.package] version in Cargo.toml" >&2; exit 1; }
TAG="${1:-}"
if [ -n "${TAG}" ]; then
  [ "${TAG}" = "v${TAG#v}" ] || { echo "a release tag starts with v: ${TAG}" >&2; exit 1; }
  [ "${TAG#v}" = "${WANT}" ] || { echo "the tag ${TAG} is not the workspace version ${WANT}" >&2; exit 1; }
fi

bad=0
check() { # file found-version
  [ "$2" = "${WANT}" ] || { echo "$1 is ${2:-<none>}, the workspace ${WANT}" >&2; bad=1; }
}
for toml in */Cargo.toml; do
  grep -q '^version.workspace = true' "${toml}" \
    || check "${toml}" "$(sed -n 's/^version = "\(.*\)"/\1/p' "${toml}" | head -1)"
done
check ui/src-tauri/Cargo.toml "$(sed -n 's/^version = "\(.*\)"/\1/p' ui/src-tauri/Cargo.toml | head -1)"
for json in ui/package.json ui/src-tauri/tauri.conf.json; do
  check "${json}" "$(sed -n 's/^[[:space:]]*"version": "\(.*\)",/\1/p' "${json}" | head -1)"
done
check docs/openapi.json "$(jq -r '.info.version // empty' docs/openapi.json)"
if [ -n "${TAG}" ]; then
  grep -q "^## \[${WANT}\]" CHANGELOG.md || { echo "CHANGELOG.md has no \"## [${WANT}]\" section" >&2; bad=1; }
fi
[ "${bad}" = 0 ] && echo "version ${WANT}: every manifest agrees${TAG:+, the tag and CHANGELOG too}"
exit "${bad}"
