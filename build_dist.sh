#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}" )" && pwd)
PNPM=${PNPM:-node "$ROOT_DIR/.local/lib/node_modules/pnpm/dist/pnpm.cjs"}
VERSION_OVERRIDE=${1:-}
PACKAGE_JSON="$ROOT_DIR/codex-cli/package.json"
CODEX_RS_DIR="$ROOT_DIR/codex-rs"
CARGO_TARGET_DIR_OVERRIDE="$CODEX_RS_DIR/target-codexaw"
ORIG_VERSION=$(node -e "console.log(require('$PACKAGE_JSON').version)")
TMP_DIR=""
cleanup() {
  if [[ -n "$VERSION_OVERRIDE" ]]; then
    node -e "const fs=require('fs');const pkg=require('$PACKAGE_JSON');pkg.version='$ORIG_VERSION';fs.writeFileSync('$PACKAGE_JSON', JSON.stringify(pkg,null,2)+'\\n');"
  fi
  if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}
trap cleanup EXIT
if [[ -n "$VERSION_OVERRIDE" ]]; then
  node -e "const fs=require('fs');const pkg=require('$PACKAGE_JSON');pkg.version='$VERSION_OVERRIDE';fs.writeFileSync('$PACKAGE_JSON', JSON.stringify(pkg,null,2)+'\\n');"
fi
cd "$ROOT_DIR"
$PNPM install
cd "$ROOT_DIR/codex-cli"

# Skip downloading prebuilt binaries from GitHub Actions (requires auth to openai/codex).
# Instead, build locally for Linux and create stub directories for other platforms.
# Users on macOS/Windows should build from source or use official releases.

BUILD_VERSION=${VERSION_OVERRIDE:-$ORIG_VERSION}
pushd "$CODEX_RS_DIR" >/dev/null
CODEX_VERSION_OVERRIDE="$BUILD_VERSION" CARGO_TARGET_DIR="$CARGO_TARGET_DIR_OVERRIDE" cargo build --release -j 2 -p codex-cli
popd >/dev/null

# Create vendor directory structure
VENDOR_DIR="$ROOT_DIR/codex-cli/vendor"
rm -rf "$VENDOR_DIR"
mkdir -p "$VENDOR_DIR"

# Linux x86_64
LINUX_VENDOR="$VENDOR_DIR/x86_64-unknown-linux-musl/codex"
mkdir -p "$LINUX_VENDOR"
cp "$CARGO_TARGET_DIR_OVERRIDE/release/codex" "$LINUX_VENDOR/codex"
chmod +x "$LINUX_VENDOR/codex"

# Create placeholder directories for other platforms (binary not included)
mkdir -p "$VENDOR_DIR/aarch64-unknown-linux-musl/codex"
mkdir -p "$VENDOR_DIR/x86_64-apple-darwin/codex"
mkdir -p "$VENDOR_DIR/aarch64-apple-darwin/codex"
mkdir -p "$VENDOR_DIR/x86_64-pc-windows-msvc/codex"
mkdir -p "$VENDOR_DIR/aarch64-pc-windows-msvc/codex"
rm -rf dist
mkdir -p dist
$PNPM pack --pack-destination ./dist
TARBALL="openai-codex-${VERSION_OVERRIDE:-0.0.0-dev}.tgz"
TMP_DIR=$(mktemp -d)
tar -xzf "dist/$TARBALL" -C "$TMP_DIR"
find "$TMP_DIR/package/vendor" -type f \( -name codex -o -name 'codex.exe' \) -exec chmod +x {} +
tar -czf dist/codexaw.tgz -C "$TMP_DIR" package
rm "dist/$TARBALL"
echo "Built codex-cli/dist/codexaw.tgz (version ${VERSION_OVERRIDE:-$ORIG_VERSION})"
