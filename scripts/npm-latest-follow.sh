#!/usr/bin/env bash
# 正式版を出すまでは、前置き付きの版を出すたびに npm の latest もその版へ動かす。
# 正式版（前置きの無い版）が latest になったら、それ以後は自ら動かさない。
#
# 使い方:
#   scripts/npm-latest-follow.sh <npm の package 名> <出した版>
#     判断だけを標準出力に一行出す。latest は動かさない。手元で確かめる時に使う。
#   scripts/npm-latest-follow.sh --apply <npm の package 名> <出した版>
#     判断が「動かす」なら npm dist-tag add で latest をその版へ動かす。
#     release.yml が npm と crates.io への publish の後に呼ぶ。
# 例: scripts/npm-latest-follow.sh @koyori-app/arc 0.1.0-alpha.1
#
# 判断の規則（今の registry の latest を見て決める。旗や設定では切り替えない）:
#   出した版が正式版        何もしない。publish が既に latest にしている
#   latest が未設定         動かす
#   latest が正式版         動かさない。正式版を出した後に前置き付きの版を出しても、
#                           latest は正式版から巻き戻らない
#   latest が前置き付き     出した版の方が新しい時だけ動かす。同じ版や古い版を出しても、
#                           latest は下がらない
# registry への問い合わせが落ちた時は、動かさずに exit 1 で止まる。
set -euo pipefail

fail() {
  echo "::error::$1" >&2
  exit 1
}

apply=false
if [ "${1:-}" = "--apply" ]; then
  apply=true
  shift
fi
if [ "$#" -ne 2 ]; then
  echo "使い方: $0 [--apply] <npm の package 名> <出した版>" >&2
  exit 2
fi

pkg="$1"
ver="$2"

semver_re='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
if ! [[ "$ver" =~ $semver_re ]]; then
  fail "版 ${ver} が X.Y.Z か X.Y.Z-前置き の形でない"
fi

case "$ver" in
  *-*) ;;
  *)
    echo "動かさない: ${ver} は正式版で、publish が既に latest にしている"
    exit 0
    ;;
esac

# 今の latest を引く。package がまだ無い（E404）時は未設定とみなす。
# それ以外の失敗で動かすと、正式版の latest を巻き戻しうるので止まる。
err_file=$(mktemp)
trap 'rm -f "$err_file"' EXIT
if latest=$(npm view "$pkg" dist-tags.latest 2>"$err_file"); then
  :
elif grep -q 'E404' "$err_file"; then
  latest=""
else
  fail "registry から ${pkg} の latest を引けなかった。latest は動かさない。npm view の出力: $(tr '\n' ' ' <"$err_file")"
fi

move() {
  echo "動かす: latest を ${ver} へ（今の latest: ${latest:-未設定}）"
  if [ "$apply" = true ]; then
    npm dist-tag add "${pkg}@${ver}" latest >&2 \
      || fail "npm dist-tag add で latest を ${ver} へ動かせなかった"
  fi
}

if [ -z "$latest" ]; then
  move
  exit 0
fi
if ! [[ "$latest" =~ $semver_re ]]; then
  fail "registry の latest「${latest}」が版の形でない。latest は動かさない"
fi
case "$latest" in
  *-*) ;;
  *)
    echo "動かさない: latest は正式版 ${latest} で、正式版が出た後は latest を動かさない"
    exit 0
    ;;
esac

# semver の優先順で比べる（前置きは点で区切った語ごとに、数字は数で、英字は字の順で、
# 数字は英字より前、語の数が多い方が後）。出した版が latest より新しい時だけ 1 を出す。
newer=$(node -e '
  const parse = (v) => {
    const [main, pre] = v.split(/-(.*)/s);
    return { main: main.split(".").map(Number), pre: pre ? pre.split(".") : [] };
  };
  const cmpId = (a, b) => {
    const an = /^[0-9]+$/.test(a), bn = /^[0-9]+$/.test(b);
    if (an && bn) return Math.sign(Number(a) - Number(b));
    if (an !== bn) return an ? -1 : 1;
    return a < b ? -1 : a > b ? 1 : 0;
  };
  const cmp = (x, y) => {
    for (let i = 0; i < 3; i++) if (x.main[i] !== y.main[i]) return Math.sign(x.main[i] - y.main[i]);
    // 前置きの無い方が新しい
    if (!x.pre.length || !y.pre.length) return Math.sign(y.pre.length - x.pre.length);
    for (let i = 0; i < Math.min(x.pre.length, y.pre.length); i++) {
      const c = cmpId(x.pre[i], y.pre[i]);
      if (c) return c;
    }
    return Math.sign(x.pre.length - y.pre.length);
  };
  process.stdout.write(cmp(parse(process.argv[1]), parse(process.argv[2])) > 0 ? "1" : "0");
' "$ver" "$latest")

if [ "$newer" = 1 ]; then
  move
else
  echo "動かさない: ${ver} は latest の ${latest} より新しくない（latest を下げない）"
fi
