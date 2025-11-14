#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}" )" && pwd)
PNPM=${PNPM:-node "$ROOT_DIR/.local/lib/node_modules/pnpm/dist/pnpm.cjs"}
VERSION_OVERRIDE=${1:-}
PACKAGE_JSON="$ROOT_DIR/codex-cli/package.json"
INSTALL_NATIVE_SCRIPT="$ROOT_DIR/codex-cli/scripts/install_native_deps.py"
CODEX_RS_DIR="$ROOT_DIR/codex-rs"
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
if command -v python3 >/dev/null 2>&1; then
  python3 "$INSTALL_NATIVE_SCRIPT" --component codex "$ROOT_DIR/codex-cli"
else
  echo "python3 is required to install native dependencies" >&2
  exit 1
fi

BUILD_VERSION=${VERSION_OVERRIDE:-$ORIG_VERSION}
pushd "$CODEX_RS_DIR" >/dev/null
CODEX_VERSION_OVERRIDE="$BUILD_VERSION" cargo build --release -p codex-cli
popd >/dev/null
LINUX_VENDOR="$ROOT_DIR/codex-cli/vendor/x86_64-unknown-linux-musl/codex"
mkdir -p "$LINUX_VENDOR"
cp "$CODEX_RS_DIR/target/release/codex" "$LINUX_VENDOR/codex"
chmod +x "$LINUX_VENDOR/codex"
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
