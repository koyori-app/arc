import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, cpSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

// npm-latest-follow.sh は、正式版を出すまで前置き付きの版に latest を付いて行かせ、
// 正式版が latest になったら自ら止まる。registry へは npm で問うので、npm を捨てる
// dir の stub に差し替えて撃つ。本物の registry には問わず、latest も動かさない。

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const PKG = '@koyori-app/arc';

// npm view の振る舞いは STUB_VIEW で選ぶ: value は STUB_LATEST を出す、e404 は package が
// 無い時の失敗、fail はそれ以外の失敗。STUB_AGAIN_SET が在れば、二度目からの view は
// STUB_LATEST_AGAIN を出す（読んでから書くまでの間に latest が変わった形）。
// 呼ばれた引数は log に残す
const NPM_STUB = `#!/bin/sh
echo "$*" >> "$STUB_NPM_LOG"
case "$1" in
  view)
    n=$(grep -c '^view ' "$STUB_NPM_LOG")
    value="$STUB_LATEST"
    [ -n "$STUB_AGAIN_SET" ] && [ "$n" -ge 2 ] && value="$STUB_LATEST_AGAIN"
    case "$STUB_VIEW" in
      value) [ -n "$value" ] && echo "$value"; exit 0 ;;
      e404) echo "npm error code E404" >&2; echo "npm error 404 Not Found" >&2; exit 1 ;;
      fail) echo "npm error code ETIMEDOUT" >&2; exit 1 ;;
    esac ;;
  dist-tag)
    [ -n "$STUB_DISTTAG_FAIL" ] && { echo "npm error code E403" >&2; exit 1; }
    exit 0 ;;
esac
echo "npm stub: unexpected $*" >&2
exit 3
`;

function runFollow(args, { latest = '', latestAgain, view = 'value', distTagFail = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), 'npm-latest-follow-'));
  mkdirSync(join(root, 'scripts'));
  cpSync(join(scriptsDir, 'npm-latest-follow.sh'), join(root, 'scripts/npm-latest-follow.sh'));
  const bin = join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(join(bin, 'npm'), NPM_STUB);
  chmodSync(join(bin, 'npm'), 0o755);
  const npmLog = join(root, 'npm.log');
  writeFileSync(npmLog, '');

  const result = spawnSync('bash', [join(root, 'scripts/npm-latest-follow.sh'), ...args], {
    cwd: root,
    encoding: 'utf8',
    env: {
      ...process.env,
      PATH: `${bin}${delimiter}${process.env.PATH}`,
      STUB_VIEW: view,
      STUB_LATEST: latest,
      STUB_AGAIN_SET: latestAgain === undefined ? '' : '1',
      STUB_LATEST_AGAIN: latestAgain ?? '',
      STUB_DISTTAG_FAIL: distTagFail ? '1' : '',
      STUB_NPM_LOG: npmLog,
    },
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
    npmCalls: readFileSync(npmLog, 'utf8').split('\n').filter(Boolean),
  };
}

const VIEW_CALL = `view ${PKG} dist-tags.latest`;
const addCall = (version) => `dist-tag add ${PKG}@${version} latest`;
// --apply で動かす時は、判じる前に一度、書く直前にもう一度読む
const applyCalls = (version) => [VIEW_CALL, VIEW_CALL, addCall(version)];

describe('npm-latest-follow.sh', () => {
  describe('動かす', () => {
    it('latest が未設定なら、出した前置き付きの版へ動かす', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { latest: '' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かす: latest を 0\.1\.0-alpha\.1 へ（今の latest: 未設定）\n$/);
      assert.deepEqual(result.npmCalls, applyCalls('0.1.0-alpha.1'));
    });

    it('package がまだ無い（E404）時は未設定とみなして動かす', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { view: 'e404' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かす: /);
      assert.deepEqual(result.npmCalls, applyCalls('0.1.0-alpha.1'));
    });

    it('latest が前置き付きで、出した版の方が新しければ動かす', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.2'], { latest: '0.1.0-alpha.1' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かす: latest を 0\.1\.0-alpha\.2 へ（今の latest: 0\.1\.0-alpha\.1）\n$/);
      assert.deepEqual(result.npmCalls, applyCalls('0.1.0-alpha.2'));
    });

    // semver の優先順で新しい物（数字の語は数で、前置きの語は字の順で比べる）
    for (const [latest, version] of [
      ['0.1.0-alpha.9', '0.1.0-alpha.10'],
      ['0.1.0-alpha.5', '0.1.0-beta.1'],
      ['0.1.0-beta.2', '0.1.0-rc.1'],
      ['0.1.0-rc.9', '0.2.0-alpha.1'],
      ['0.1.0-alpha', '0.1.0-alpha.1'],
    ]) {
      it(`latest ${latest} より ${version} は新しいので動かす`, () => {
        const result = runFollow(['--apply', PKG, version], { latest });
        assert.equal(result.status, 0, result.stderr);
        assert.deepEqual(result.npmCalls, applyCalls(version));
      });
    }

    it('package が二度とも E404 なら、読み直しても変わらず動かす', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { view: 'e404' });
      assert.equal(result.status, 0, result.stderr);
      assert.deepEqual(result.npmCalls, applyCalls('0.1.0-alpha.1'));
    });

    it('--apply が無ければ、判断を出すだけで latest は動かさない', () => {
      const result = runFollow([PKG, '0.1.0-alpha.2'], { latest: '0.1.0-alpha.1' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かす: /);
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });
  });

  describe('動かさない', () => {
    it('latest が正式版なら、前置き付きの版を出しても動かさない（自ら止まる）', () => {
      const result = runFollow(['--apply', PKG, '0.2.0-alpha.1'], { latest: '0.1.0' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かさない: latest は正式版 0\.1\.0 で/);
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });

    it('出した版が正式版なら何もしない（publish が既に latest にしている）', () => {
      const result = runFollow(['--apply', PKG, '0.1.0'], { latest: '0.1.0-rc.1' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かさない: 0\.1\.0 は正式版/);
      // registry にも問わない（二重に動かさない）
      assert.deepEqual(result.npmCalls, []);
    });

    it('出した版が latest より古ければ動かさない（latest を下げない）', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { latest: '0.1.0-alpha.2' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かさない: 0\.1\.0-alpha\.1 は latest の 0\.1\.0-alpha\.2 より新しくない/);
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });

    it('出した版が latest と同じなら動かさない', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { latest: '0.1.0-alpha.1' });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /^動かさない: /);
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });
  });

  describe('止まる（latest は動かさない）', () => {
    it('registry への問い合わせが E404 以外で落ちたら exit 1', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { view: 'fail' });
      assert.equal(result.status, 1, result.stderr);
      assert.equal(result.stdout, '');
      assert.match(result.stderr, /latest を引けなかった/);
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });

    it('registry の latest が版の形でなければ exit 1', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { latest: 'not-a-version' });
      assert.equal(result.status, 1, result.stderr);
      assert.equal(result.stdout, '');
      assert.deepEqual(result.npmCalls, [VIEW_CALL]);
    });

    it('読み直しの間に latest が前置き付きの別の版へ変わったら、書かずに exit 1', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.2'], {
        latest: '0.1.0-alpha.1',
        latestAgain: '0.1.0-alpha.3',
      });
      assert.equal(result.status, 1, result.stderr);
      assert.equal(result.stdout, '');
      assert.match(result.stderr, /0\.1\.0-alpha\.1 から 0\.1\.0-alpha\.3 へ変わった/);
      assert.deepEqual(result.npmCalls, [VIEW_CALL, VIEW_CALL]);
    });

    it('読み直しの間に latest が未設定から付いたら、書かずに exit 1', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], {
        latest: '',
        latestAgain: '0.1.0',
      });
      assert.equal(result.status, 1, result.stderr);
      assert.equal(result.stdout, '');
      assert.match(result.stderr, /未設定 から 0\.1\.0 へ変わった/);
      assert.deepEqual(result.npmCalls, [VIEW_CALL, VIEW_CALL]);
    });

    it('npm dist-tag add が落ちたら exit 1', () => {
      const result = runFollow(['--apply', PKG, '0.1.0-alpha.1'], { latest: '', distTagFail: true });
      assert.equal(result.status, 1);
      assert.match(result.stderr, /latest を 0\.1\.0-alpha\.1 へ動かせなかった/);
    });

    it('出した版が版の形でなければ、registry に問う前に exit 1', () => {
      const result = runFollow(['--apply', PKG, 'v0.1.0-alpha.1']);
      assert.equal(result.status, 1);
      assert.equal(result.stdout, '');
      assert.deepEqual(result.npmCalls, []);
    });

    for (const args of [[], [PKG], ['--apply', PKG], [PKG, '0.1.0', 'extra']]) {
      it(`引数が違う（${JSON.stringify(args)}）時は exit 2`, () => {
        const result = runFollow(args);
        assert.equal(result.status, 2);
        assert.equal(result.stdout, '');
        assert.deepEqual(result.npmCalls, []);
      });
    }
  });
});
