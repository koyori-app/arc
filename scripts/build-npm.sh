#!/usr/bin/env bash
set -euo pipefail

CRATE_DIR="crates/koyori-arc-core"
PKG_DIR="$CRATE_DIR/pkg"
NPM_NAME="@koyori-app/arc"

cd "$(dirname "$0")/.."

echo "==> wasm-pack build"
wasm-pack build "$CRATE_DIR" --target web "$@"

echo "==> patching package name: $NPM_NAME"
# jq が無い環境でも動くよう node で処理する
node -e "
  const fs = require('fs');
  const path = '$PKG_DIR/package.json';
  const pkg = JSON.parse(fs.readFileSync(path, 'utf8'));
  pkg.name = '$NPM_NAME';
  pkg.publishConfig = { access: 'public' };
  fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
  console.log('  name:', pkg.name, '  version:', pkg.version);
"

echo "==> done: $PKG_DIR"
