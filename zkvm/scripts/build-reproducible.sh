#!/usr/bin/env bash
set -euo pipefail

if (( $# > 1 )) || { (( $# == 1 )) && [[ ! $1 =~ ^[[:xdigit:]]{64}$ ]]; }; then
    echo "Usage: $0 [approved-image-id (64 hex characters)]" >&2
    exit 2
fi
expected_id=${1:-}

workspace=$(cd "$(dirname "$0")/.." && pwd)
export CARGO_TARGET_DIR="$workspace/target/reproducible"

if ! command -v docker >/dev/null || ! docker info >/dev/null 2>&1; then
    echo "A running Docker is required (Docker Desktop, OrbStack, Colima, or Docker Engine)." >&2
    exit 1
fi
# risc0-build exports the guest with `docker build --output`, which needs BuildKit.
if ! docker buildx version >/dev/null 2>&1; then
    cat >&2 <<'EOF'
The Docker Buildx plugin is required. Docker Desktop, OrbStack, and the Docker
Engine packages from docker.com include it. With Colima on macOS, add it once:
  brew install docker-buildx
  mkdir -p ~/.docker/cli-plugins
  ln -sfn "$(brew --prefix)/opt/docker-buildx/bin/docker-buildx" ~/.docker/cli-plugins/docker-buildx
EOF
    exit 1
fi

export DOCKER_DEFAULT_PLATFORM=linux/amd64
export DOCKER_BUILDKIT=1
export NQG_REPRODUCIBLE=1
unset RISC0_SKIP_BUILD RISC0_BUILD_DEBUG RISC0_DOCKER_CONTAINER_TAG

cargo build --release --locked --manifest-path "$workspace/Cargo.toml" -p host
image_id=$("$CARGO_TARGET_DIR/release/host" image-id)
printf '%s\n' "$image_id"
if [[ -n "$expected_id" ]]; then
    if [[ $(printf '%s' "$image_id" | tr '[:upper:]' '[:lower:]') != $(printf '%s' "$expected_id" | tr '[:upper:]' '[:lower:]') ]]; then
        echo "Image ID mismatch. Approved: $expected_id; rebuilt: $image_id" >&2
        exit 1
    fi
    echo "Image ID matches the approved value." >&2
fi
echo "Host: $CARGO_TARGET_DIR/release/host" >&2
