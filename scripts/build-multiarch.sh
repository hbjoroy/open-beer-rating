#!/usr/bin/env bash
# Build and push multi-arch (amd64 + arm64) container images.
# Works with both docker and podman (podman aliases docker).
#
# All Rust compilation runs natively on the build host via cargo-zigbuild.
# Only the lightweight runtime stage uses platform emulation for the
# foreign arch (requires binfmt registration — see README).
#
# Usage:
#   ./scripts/build-multiarch.sh                          # build only
#   ./scripts/build-multiarch.sh --push                   # build + push
#   IMAGE_TAG=oci.bjoroy.me/open-drink-tasting/open-drink-tasting:0.2.0 \
#     ./scripts/build-multiarch.sh --push                 # custom tag
#
# Environment variables:
#   IMAGE_TAG       Full image reference (default: open-drink-tasting:latest)
#   PLATFORMS       Space-separated platforms (default: "linux/amd64 linux/arm64")

set -euo pipefail

IMAGE_TAG="${IMAGE_TAG:-open-drink-tasting:latest}"
PLATFORMS="${PLATFORMS:-linux/amd64 linux/arm64}"
PUSH=false

for arg in "$@"; do
  case "$arg" in
    --push) PUSH=true ;;
    *) echo "Unknown argument: $arg"; exit 1 ;;
  esac
done

echo "=== Multi-arch build: $IMAGE_TAG ==="
echo "Platforms: $PLATFORMS"
echo ""

# Build each platform separately
for PLATFORM in $PLATFORMS; do
  ARCH="${PLATFORM#linux/}"
  TAG="${IMAGE_TAG}-${ARCH}"
  echo "--- Building $PLATFORM as $TAG ---"
  docker build --platform "$PLATFORM" -t "$TAG" .
  echo ""
done

# Create manifest list
echo "--- Creating manifest: $IMAGE_TAG ---"
MANIFEST_ARGS=""
for PLATFORM in $PLATFORMS; do
  ARCH="${PLATFORM#linux/}"
  MANIFEST_ARGS="$MANIFEST_ARGS ${IMAGE_TAG}-${ARCH}"
done

docker manifest rm "$IMAGE_TAG" 2>/dev/null || true
docker manifest create "$IMAGE_TAG" $MANIFEST_ARGS

if [ "$PUSH" = true ]; then
  echo "--- Pushing ---"
  for PLATFORM in $PLATFORMS; do
    ARCH="${PLATFORM#linux/}"
    docker push "${IMAGE_TAG}-${ARCH}"
  done
  docker manifest push "$IMAGE_TAG"
  echo ""
  echo "Pushed: $IMAGE_TAG (multi-arch)"
else
  echo ""
  echo "Built locally: $IMAGE_TAG (multi-arch)"
  echo "Run with --push to push to registry."
fi
