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
PREV_ID=$(docker inspect --type image --format='{{.Id}}' $IMAGE:latest || true)

echo "## Building image ${IMAGE}:${VERSION}"
docker build -t $IMAGE:latest .
docker tag $IMAGE:latest $IMAGE:$VERSION

NEW_ID=$(docker inspect --type image --format='{{.Id}}' $IMAGE:latest)
echo "{\"previous_id\": \"${PREV_ID}\",\"id\":\"${NEW_ID}\"}"
