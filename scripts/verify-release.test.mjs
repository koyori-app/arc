import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, cpSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

// verify-release.sh は publish の直前に、タグと版と系譜を確かめる。release.yml は
// この script の標準出力を dist-tag として読むので、断る時は標準出力に何も出さず、
// 出口の値で止めなければならない。
// script が外へ呼ぶ cargo、jq、git は、捨てる dir に置いた stub に差し替える。
// 本物の git fetch は走らせない。

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const SHA = '0123456789abcdef0123456789abcdef01234567';

const STUBS = {
  // cargo metadata の出力は jq の stub が読み捨てる。STUB_CARGO_FAIL で落とせる
  cargo: `#!/bin/sh
if [ -n "$STUB_CARGO_FAIL" ]; then echo "cargo stub: failed" >&2; exit 101; fi
echo '{"packages":[]}'
`,
  // crate の版を引く呼び出しと、arc-vue の package.json を読む呼び出しを引数で分ける
  jq: `#!/bin/sh
case "$*" in
  *koyori-arc-core*) cat >/dev/null; echo "$STUB_CRATE_VERSION" ;;
  *packages/arc-vue/package.json*) echo "$STUB_VUE_VERSION" ;;
  *) echo "jq stub: unexpected $*" >&2; exit 3 ;;
esac
`,
  // 呼ばれた引数を log に残す。祖先かどうかは STUB_ANCESTOR で決める
  git: `#!/bin/sh
echo "$*" >> "$STUB_GIT_LOG"
case "$1" in
  fetch) exit 0 ;;
  merge-base) [ "$STUB_ANCESTOR" = yes ] && exit 0 || exit 1 ;;
esac
echo "git stub: unexpected $*" >&2
exit 3
`,
};

/**
 * script を捨てる root へ写し、stub を PATH の頭に差し込んで撃つ。
 * script は自分の dir の一つ上へ cd するので、捨てる root の中で閉じる。
 */
function runVerify(args, { crate = '0.1.0-alpha.1', vue = '0.1.0-alpha.1', ancestor = true, cargoFail = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'verify-release-'));
  mkdirSync(join(root, 'scripts'));
  cpSync(join(scriptsDir, 'verify-release.sh'), join(root, 'scripts/verify-release.sh'));
  const bin = join(root, 'bin');
  mkdirSync(bin);
  for (const [name, body] of Object.entries(STUBS)) {
    writeFileSync(join(bin, name), body);
    chmodSync(join(bin, name), 0o755);
  }
  const gitLog = join(root, 'git.log');
  writeFileSync(gitLog, '');

  const result = spawnSync('bash', [join(root, 'scripts/verify-release.sh'), ...args], {
    cwd: root,
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${bin}${delimiter}${process.env.PATH}`,
      STUB_CRATE_VERSION: crate,
      STUB_VUE_VERSION: vue,
      STUB_ANCESTOR: ancestor ? 'yes' : 'no',
      STUB_CARGO_FAIL: cargoFail ? '1' : '',
      STUB_GIT_LOG: gitLog,
    },
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    gitCalls: readFileSync(gitLog, 'utf8').split('\n').filter(Boolean),
  };
}

/** 断る時は、出口の値が expected で、標準出力は一字も出さない。 */
function assertRefused(result, expected, stderrPattern) {
  assert.equal(result.status, expected, `exit ${result.status}, stderr: ${result.stderr}`);
  assert.equal(result.stdout, '', '断る時に標準出力へ出すと、release.yml がそれを dist-tag に使う');
  if (stderrPattern) assert.match(result.stderr, stderrPattern);
}

describe('verify-release.sh', () => {
  describe('揃っている時は dist-tag を一行だけ出す', () => {
    it('前置きのある版（v0.1.0-alpha.1）は alpha', () => {
      const result = runVerify(['v0.1.0-alpha.1', SHA]);
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, 'alpha\n');
      // stub を通っている（本物の git は走っていない）ことと、master の祖先を問うていること
      assert.deepEqual(result.gitCalls, [
        'fetch --quiet --no-tags origin master',
        `merge-base --is-ancestor ${SHA} origin/master`,
      ]);
    });

    it('前置きの無い版（v0.1.0）は latest', () => {
      const result = runVerify(['v0.1.0', SHA], { crate: '0.1.0', vue: '0.1.0' });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, 'latest\n');
    });

    it('前置きが点で区切られていれば、頭の語を使う（v0.1.0-rc.2 は rc）', () => {
      const result = runVerify(['v0.1.0-rc.2', SHA], { crate: '0.1.0-rc.2', vue: '0.1.0-rc.2' });
      assert.equal(result.status, 0, result.stderr);
      assert.equal(result.stdout, 'rc\n');
    });

    // 関門は語を並べて許すのではなく、npm が断る形だけを断る。普通の前置きは
    // 一覧に無くても通ることを縛る
    for (const word of ['alpha', 'beta', 'rc', 'next', 'canary', 'pre', 'preview']) {
      it(`普通の前置き ${word} は通る`, () => {
        const version = `0.1.0-${word}.1`;
        const result = runVerify([`v${version}`, SHA], { crate: version, vue: version });
        assert.equal(result.status, 0, result.stderr);
        assert.equal(result.stdout, `${word}\n`);
      });
    }
  });

  // npm は semver の範囲として読める tag を断る（npm publish の
  // "Tag name must not be a valid SemVer range"）。publish の前に関門で止める
  describe('npm が範囲と読む dist-tag は標準出力を空にして exit 1', () => {
    for (const [label, version] of [
      ['前置きが空', '0.1.0-'],
      ['前置きの頭の語が空', '0.1.0-.1'],
      ['前置きが数字だけ', '0.1.0-1'],
      ['前置きの頭の語が 0', '0.1.0-0.3'],
      ['前置きの頭の語が x', '0.1.0-x.1'],
      ['前置きの頭の語が X', '0.1.0-X.1'],
      ['前置きの頭の語が v に数字', '0.1.0-v2.1'],
      ['前置きの頭の語が v に x', '0.1.0-vx'],
    ]) {
      it(`${label}（v${version}）`, () => {
        assertRefused(
          runVerify([`v${version}`, SHA], { crate: version, vue: version }),
          1,
          /npm が semver の範囲として読む/,
        );
      });
    }
  });

  describe('揃っていない時は標準出力を空にして exit 1', () => {
    it('タグが v で始まらない', () => {
      assertRefused(runVerify(['0.1.0-alpha.1', SHA]), 1, /v で始まらない/);
    });

    it('crate の版とタグが食い違う', () => {
      assertRefused(
        runVerify(['v0.1.0-alpha.1', SHA], { crate: '0.1.0-alpha.2' }),
        1,
        /koyori-arc-core の版 0\.1\.0-alpha\.2 が食い違う/,
      );
    });

    it('@koyori-app/arc-vue の版とタグが食い違う', () => {
      assertRefused(
        runVerify(['v0.1.0-alpha.1', SHA], { vue: '0.1.0-alpha.2' }),
        1,
        /@koyori-app\/arc-vue の版 0\.1\.0-alpha\.2 が食い違う/,
      );
    });

    it('SHA が master の祖先でない', () => {
      assertRefused(
        runVerify(['v0.1.0-alpha.1', SHA], { ancestor: false }),
        1,
        /master の祖先ではない/,
      );
    });
  });

  describe('引数の数が違う時は標準出力を空にして exit 2', () => {
    for (const args of [[], ['v0.1.0-alpha.1'], ['v0.1.0-alpha.1', SHA, 'extra']]) {
      it(`引数が ${args.length} 個`, () => {
        const result = runVerify(args);
        assertRefused(result, 2, /使い方/);
        // 版も系譜も問う前に止まる
        assert.deepEqual(result.gitCalls, []);
      });
    }
  });

  it('外へ呼ぶ命（cargo metadata）が落ちた時も、標準出力は空で 0 以外で止まる', () => {
    const result = runVerify(['v0.1.0-alpha.1', SHA], { cargoFail: true });
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, '');
  });
});
