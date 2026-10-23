#!/usr/bin/env bash
# publish の前に、タグと版と系譜が揃っているかを確かめる。
# 使い方: scripts/verify-release.sh <タグ> <commit の SHA>
# 揃っていなければ理由を標準エラーに出し、exit 1 で止まる。
# 揃っていれば、npm へ出す時の dist-tag を標準出力に一行だけ出す。
# release.yml が publish の前に呼ぶ。手元でも同じ物を撃てる。
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "使い方: $0 <タグ> <commit の SHA>" >&2
  exit 2
fi

tag="$1"
sha="$2"

cd "$(dirname "$0")/.."

fail() {
  echo "::error::$1" >&2
  exit 1
}

# タグは vX.Y.Z か vX.Y.Z-前置き の形に限る。
ver="${tag#v}"
if [ "$tag" = "$ver" ]; then
  fail "タグ ${tag} が v で始まらない（vX.Y.Z の形が要る）"
fi

# crate の版とタグを照らす。npm の @koyori-app/arc も同じ版で出るため、
# ここが揃えば wasm-pack の pkg の版も揃う。
crate_ver=$(cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[] | select(.name=="koyori-arc-core") | .version')
if [ "$ver" != "$crate_ver" ]; then
  fail "タグ ${tag} と koyori-arc-core の版 ${crate_ver} が食い違う。タグか Cargo.toml を直してから release を作り直すこと"
fi

# arc-vue はこの workflow では出さないが、版を Rust の側と揃えておく。
# ずれたまま出すと、どの arc-vue がどの arc と組むのかが読めなくなる。
vue_ver=$(jq -r '.version' packages/arc-vue/package.json)
if [ "$ver" != "$vue_ver" ]; then
  fail "タグ ${tag} と @koyori-app/arc-vue の版 ${vue_ver} が食い違う。packages/arc-vue/package.json を揃えること"
fi

# release は任意の ref から作れるため、master に入った commit だけを出す。
git fetch --quiet --no-tags origin master
if ! git merge-base --is-ancestor "$sha" origin/master; then
  fail "${sha} は master の祖先ではない。master に merge されていない ref からの release は publish しない"
fi

# npm 11 は、前置き付きの版を --tag 無しで publish すると断る。
# 前置きの頭の語（0.1.0-alpha.1 なら alpha）を dist-tag にし、latest を動かさない。
case "$ver" in
  *-*)
    pre="${ver#*-}"
    dist_tag="${pre%%.*}"
    # npm は semver の範囲として読める tag を断る（npm publish の
    # "Tag name must not be a valid SemVer range"）。前置きの頭の語の字種
    # （英数とハイフン）では、範囲と読まれるのは空の字（* と読まれる）と、
    # v を頭に付けてもよい x、X、数字の一続きである。語を並べて許すのではなく、
    # この規則に当たる物だけを断る。
    semver_range_re='^v*(x|X|0|[1-9][0-9]*)$'
    if [ -z "$dist_tag" ] || [[ "$dist_tag" =~ $semver_range_re ]]; then
      fail "タグ ${tag} の前置きから選んだ dist-tag「${dist_tag}」は、npm が semver の範囲として読むので publish で断られる。前置きの頭の語を、空でも数字だけでもなく、x、X、v に数字や x が続く形でもない語にして（alpha.1 など）、タグと版を作り直すこと"
    fi
    echo "$dist_tag"
    ;;
  *)
    echo "latest"
    ;;
esac
