#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}" )" && pwd)
PNPM=${PNPM:-node "$ROOT_DIR/.local/lib/node_modules/pnpm/dist/pnpm.cjs"}
VERSION_OVERRIDE=${1:-}
PACKAGE_JSON="$ROOT_DIR/codex-cli/package.json"
INSTALL_NATIVE_SCRIPT="$ROOT_DIR/codex-cli/scripts/install_native_deps.py"
ORIG_VERSION=$(node -e "console.log(require('$PACKAGE_JSON').version)")
cleanup_version() {
  if [[ -n "$VERSION_OVERRIDE" ]]; then
    node -e "const fs=require('fs');const pkg=require('$PACKAGE_JSON');pkg.version='$ORIG_VERSION';fs.writeFileSync('$PACKAGE_JSON', JSON.stringify(pkg,null,2)+'\\n');"
  fi
}
trap cleanup_version EXIT
if [[ -n "$VERSION_OVERRIDE" ]]; then
  node -e "const fs=require('fs');const pkg=require('$PACKAGE_JSON');pkg.version='$VERSION_OVERRIDE';fs.writeFileSync('$PACKAGE_JSON', JSON.stringify(pkg,null,2)+'\\n');"
fi
cd "$ROOT_DIR"
$PNPM install
cd "$ROOT_DIR/codex-cli"
if command -v python3 >/dev/null 2>&1; then
  python3 "$INSTALL_NATIVE_SCRIPT" "$ROOT_DIR/codex-cli"
else
  echo "python3 is required to install native dependencies" >&2
  exit 1
fi
rm -f dist/*.tgz
$PNPM pack --pack-destination ./dist
TARBALL="openai-codex-${VERSION_OVERRIDE:-0.0.0-dev}.tgz"
mv "dist/$TARBALL" dist/codexaw.tgz
echo "Built codex-cli/dist/codexaw.tgz (version ${VERSION_OVERRIDE:-$ORIG_VERSION})"
