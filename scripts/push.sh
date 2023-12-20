#!/bin/bash

set -eEuo pipefail

VERSION="${1:-$(cargo pkgid | cut -d@ -f2)}"

if [[ "${TRACE-0}" == "1" ]]; then
    set -x
fi

cd "$(dirname "$0")"/..

export DOCKER_BUILDKIT=1

IMAGE="ncube-ingest"
REGISTRY_PATH="docker.io/dtheodor"
FULL_IMAGE="${REGISTRY_PATH}/${IMAGE}:${VERSION}"

echo "## Pushing image ${FULL_IMAGE}"
docker tag $IMAGE:$VERSION "$FULL_IMAGE"
docker push "$FULL_IMAGE"
