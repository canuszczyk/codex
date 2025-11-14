#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
DIST_PATH="$ROOT_DIR/codex-cli/dist/codexaw.tgz"

if [[ -n $(git status --porcelain) ]]; then
  echo "Error: working tree has uncommitted changes. Please commit or stash before releasing."
  exit 1
fi

LATEST_TAG=$(git tag --list 'codexaw-v*' | sort -V | tail -n 1)
if [[ -n "$LATEST_TAG" ]]; then
  LATEST_VERSION=${LATEST_TAG#codexaw-v}
  IFS='.' read -r MAJOR MINOR PATCH <<<"$LATEST_VERSION"
  PATCH=$((PATCH + 1))
  DEFAULT_VERSION="${MAJOR}.${MINOR}.${PATCH}"
else
  DEFAULT_VERSION="0.1.0"
fi

read -rp "Enter release version (default ${DEFAULT_VERSION}): " VERSION
VERSION=${VERSION:-$DEFAULT_VERSION}
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Error: version must be in MAJOR.MINOR.PATCH form (e.g., 0.1.0)."
  exit 1
fi

TAG="codexaw-v$VERSION"

reuse_tag=false
release_exists=false
if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null; then
  read -rp "Tag ${TAG} already exists. Reuse it? [y/N]: " reuse_response
  if [[ ! "$reuse_response" =~ ^[Yy]$ ]]; then
    echo "Aborting release; choose a new version."
    exit 1
  fi
  reuse_tag=true
  if gh release view "${TAG}" >/dev/null 2>&1; then
    release_exists=true
  fi
fi

"$ROOT_DIR/build_dist.sh" "$VERSION"

if [[ ! -f "$DIST_PATH" ]]; then
  echo "Error: expected artifact at $DIST_PATH"
  exit 1
fi

if [[ "$reuse_tag" != true ]]; then
  git tag "${TAG}"
  git push origin "${TAG}"
fi

if [[ "$reuse_tag" == true && "$release_exists" == true ]]; then
  gh release upload "${TAG}" "$DIST_PATH" --clobber
else
  gh release create "${TAG}" "$DIST_PATH" \
    --title "codexaw ${TAG}" \
    --notes "Automated release for ${TAG}"
fi

echo
echo "Release created for ${TAG}."
echo "Install with:"
echo "  npm install -g https://github.com/$(git config --get remote.origin.url | sed -n 's#.*github.com[:/]\(.*\)\.git#\1#p')/releases/download/${TAG}/codexaw.tgz"
