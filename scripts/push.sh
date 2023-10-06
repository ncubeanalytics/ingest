#!/bin/bash

set -e

if [ $# -lt 1 ]; then
    echo "Usage: ${0} <version>"
    exit 1
fi

VERSION=$1

set -Euo pipefail

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
