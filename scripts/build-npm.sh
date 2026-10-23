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
  // wasm-pack は二つの免許本文を pkg へ写すが、files には載せない。
  // npm が files に無くても入れるのは LICENSE だけなので、GPL を選んだ
  // 受け取り手にも全文が届くよう、二本を files に足す。files が無ければ
  // pkg の全てが入るので、足さずにおく（足すと逆に中身が絞られる）。
  for (const f of ['LICENSE-MPL-2.0', 'LICENSE-GPL-2.0-or-later']) {
    if (!fs.existsSync('$PKG_DIR/' + f)) throw new Error(f + ' が pkg に無い');
    if (Array.isArray(pkg.files) && !pkg.files.includes(f)) pkg.files.push(f);
  }
  fs.writeFileSync(path, JSON.stringify(pkg, null, 2) + '\n');
  console.log('  name:', pkg.name, '  version:', pkg.version);
"

echo "==> done: $PKG_DIR"
