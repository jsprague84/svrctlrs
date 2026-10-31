#!/usr/bin/env bash
# CI's Lint job in CI's own image (rust-ci:latest, the tag ci.yaml uses), so a local lint uses the
# clippy CI uses: the laptop's nixpkgs clippy differs (older or newer) and they disagree (the agentic-coding
# standard §6; rustRoast 2026-10-26, two red pushes). Runs as you, so nothing it writes is
# root-owned; build cache target/ci-lint, crate downloads ~/.cache/svrctlrs-ci-cargo. Fails
# when Docker or the image is missing: no fallback.
#
#   scripts/dev/ci-lint.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
IMAGE="${CI_IMAGE:-git.js-node.cc/jsprague/rust-ci:latest}"
command -v docker >/dev/null || { echo "ci-lint: docker is needed to lint with CI's toolchain" >&2; exit 1; }
docker pull -q "$IMAGE" >/dev/null 2>&1 || docker image inspect "$IMAGE" >/dev/null 2>&1 \
  || { echo "ci-lint: cannot pull $IMAGE (docker login git.js-node.cc?)" >&2; exit 1; }
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/svrctlrs-ci-cargo"
mkdir -p "$CACHE/registry" "$CACHE/git" target/ci-lint
exec docker run --rm --user "$(id -u):$(id -g)" \
  -v "$PWD:/w" -w /w \
  -v "$CACHE/registry:/ci-cargo/registry" -v "$CACHE/git:/ci-cargo/git" \
  -e CARGO_TARGET_DIR=/w/target/ci-lint -e RUSTC_WRAPPER= -e HOME=/tmp -e RUSTUP_HOME=/usr/local/rustup \
  "$IMAGE" sh -c '
    set -e
    export CARGO_HOME_IMAGE="${CARGO_HOME:-/usr/local/cargo}"
    mkdir -p /tmp/cargo && ln -sfn /ci-cargo/registry /tmp/cargo/registry && ln -sfn /ci-cargo/git /tmp/cargo/git
    export CARGO_HOME=/tmp/cargo PATH="$CARGO_HOME_IMAGE/bin:$PATH"
    echo "CI toolchain: $(rustc -V), $(cargo clippy -V)"
    scripts/ci/check-version.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings'
