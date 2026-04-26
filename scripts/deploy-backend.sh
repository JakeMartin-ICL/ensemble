#!/bin/sh
set -e

IMAGE="ghcr.io/jakemartin-icl/ensemble-backend"
TAG=$(git rev-parse --short HEAD)

cd "$(git rev-parse --show-toplevel)"

echo "→ Building $IMAGE:$TAG"
docker build -f backend/Dockerfile -t "$IMAGE:$TAG" -t "$IMAGE:latest" .

echo "→ Pushing $IMAGE:$TAG"
docker push "$IMAGE:$TAG"
echo "→ Pushing $IMAGE:latest"
docker push "$IMAGE:latest"

echo "Done: $IMAGE:$TAG"
