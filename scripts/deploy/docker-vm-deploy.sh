#!/usr/bin/env bash
# Target-side deploy for SvrCtlRS on docker-vm. Installed at ~/docker-compose/svrctlrs/deploy.sh
# and bound to the CI deploy key in ~/.ssh/authorized_keys as a forced command:
#
#   command="/home/ubuntu/docker-compose/svrctlrs/deploy.sh",no-port-forwarding,no-agent-forwarding,no-pty,no-X11-forwarding ssh-ed25519 AAAA... svrctlrs-forgejo-deploy@epyc-ai
#
# The CI job runs `ssh ubuntu@docker-vm <commit-sha>`; sshd puts that argument in
# SSH_ORIGINAL_COMMAND and runs this script instead. It accepts nothing but a 40-hex SHA,
# waits for the registry to carry that commit's image, switches the compose stack to it, and
# refuses to call the deploy done until the running container reports the same revision and is
# healthy. Running it by hand works the same way: ./deploy.sh <sha>.
set -euo pipefail

SHA="${SSH_ORIGINAL_COMMAND:-${1:-}}"
[[ "${SHA}" =~ ^[0-9a-f]{40}$ ]] || { echo "refusing: expected a 40-character commit sha, got '${SHA}'" >&2; exit 2; }

cd "$(dirname "$(readlink -f "$0")")"
IMAGE=git.js-node.cc/jsprague/svrctlrs
TAG="${SHA:0:7}"

echo "deploy ${SHA} -> ${IMAGE}:${TAG}"

# The image is pushed by the same pipeline moments before this runs; give the registry time.
for attempt in $(seq 1 40); do
  if docker pull --quiet "${IMAGE}:${TAG}" >/dev/null 2>&1; then
    break
  fi
  [ "${attempt}" -lt 40 ] || { echo "image ${IMAGE}:${TAG} never appeared in the registry" >&2; exit 1; }
  sleep 15
done

# Persist the tag so a later manual `docker compose up -d` keeps this exact revision.
if grep -q '^TAG=' .env 2>/dev/null; then
  sed -i "s/^TAG=.*/TAG=${TAG}/" .env
else
  echo "TAG=${TAG}" >> .env
fi

docker compose up -d --remove-orphans

# Verify: the running container must carry this commit's revision label and become healthy.
for attempt in $(seq 1 24); do
  STATUS=$(docker inspect svrctlrs --format '{{.State.Health.Status}}' 2>/dev/null || echo missing)
  REVISION=$(docker inspect svrctlrs --format '{{index .Config.Labels "org.opencontainers.image.revision"}}' 2>/dev/null || true)
  if [ "${STATUS}" = healthy ] && [ "${REVISION}" = "${SHA}" ]; then
    echo "deployed: svrctlrs is healthy at revision ${SHA}"
    docker exec svrctlrs /app/svrctl health || true
    exit 0
  fi
  [ "${STATUS}" = unhealthy ] && { echo "container is unhealthy after update" >&2; docker logs --tail 50 svrctlrs >&2; exit 1; }
  sleep 5
done
echo "timed out waiting for svrctlrs to become healthy (status=${STATUS}, revision=${REVISION})" >&2
docker logs --tail 50 svrctlrs >&2
exit 1
