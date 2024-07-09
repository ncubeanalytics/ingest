#!/bin/bash

set -eEuo pipefail

if [[ "${TRACE:-0}" == "1" ]]; then
    set -x
fi

cd "$(dirname "$0")"/..

VERSION="${1:-$(cargo pkgid | cut -d@ -f2)}"

echo "Building ${VERSION}"
git stash
./scripts/build.sh $VERSION
git stash pop
echo "Tagging ${VERSION}"
git tag -a "v${VERSION}" -m "n-cube ingest ${VERSION}"
git push --tags
echo "Pushing ${VERSION}"
./scripts/push.sh $VERSION
