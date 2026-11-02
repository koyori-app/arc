import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

// release.yml の最後の段は registry の latest を読んでから書く。別の tag の release が
// 並んで走ると読みと書きの間に割り込まれうるので、release は一つずつ順に走らせる。
// ここで縛るのは排他の設定の形だけで、割り込みそのものは試験できない。

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const workflow = readFileSync(join(repoRoot, '.github/workflows/release.yml'), 'utf8');

/** 頭の `concurrency:` の塊を、鍵と値の組にして返す（入れ子は持たない前提）。 */
function concurrencyBlock() {
  const lines = workflow.split('\n');
  const start = lines.findIndex((line) => line === 'concurrency:');
  assert.notEqual(start, -1, 'release.yml の頭に concurrency: の塊が無い');
  const block = {};
  for (const line of lines.slice(start + 1)) {
    const match = line.match(/^ {2}([a-z-]+):\s*(.*)$/);
    if (!match) break;
    block[match[1]] = match[2].trim();
  }
  return block;
}

describe('release.yml の排他', () => {
  it('群れは tag によらない固定の値（式を含まない）', () => {
    const { group } = concurrencyBlock();
    assert.ok(group, 'group が無い');
    assert.doesNotMatch(group, /\$\{\{/, `group が式を含む: ${group}`);
  });

  it('待ちの run を取り消さず積む（queue: max）', () => {
    // 既定では群れの中で待てる run は一つだけで、後から来た run が待っている run を
    // 取り消す。tag を続けて押した時に間の release が黙って消えないようにする
    assert.equal(concurrencyBlock().queue, 'max');
  });

  it('走っている publish を取り消さない（cancel-in-progress: false）', () => {
    assert.equal(concurrencyBlock()['cancel-in-progress'], 'false');
  });
});

/**
 * publish の job の段を、名と uses と run の組にして順に返す。
 * 段は `      - ` で始まり、段の中の鍵は 8 字下げ、run の本文はさらに下げて続く前提で読む。
 */
function publishSteps() {
  const body = workflow.slice(workflow.indexOf('    steps:\n'));
  return body
    .split(/\n {6}- /)
    .slice(1)
    .map((chunk) => {
      const text = `- ${chunk}`;
      // 段の頭の鍵は `- ` の後ろ、他の鍵は 8 字下げ（with: の中の鍵はさらに下がるので拾わない）
      const field = (key) =>
        text.match(new RegExp(`^(?:- | {8})${key}: (.*)$`, 'm'))?.[1]?.trim();
      return { name: field('name'), uses: field('uses'), id: field('id'), text };
    });
}

function stepIndex(steps, name) {
  const index = steps.findIndex((step) => step.name === name);
  assert.notEqual(index, -1, `段「${name}」が無い`);
  return index;
}

describe('release.yml の殻（@koyori-app/arc-vue）の publish', () => {
  const steps = publishSteps();

  it('pnpm の action は他の action と同じく版を SHA で固定する', () => {
    const setup = steps[stepIndex(steps, 'Setup pnpm')];
    assert.match(setup.uses, /^pnpm\/action-setup@[0-9a-f]{40} # /);
  });

  it('依存は lockfile のまま、lifecycle script を走らせずに入れる（id-token を持つ job のため）', () => {
    const install = steps[stepIndex(steps, 'Install workspace dependencies')];
    assert.match(install.text, /pnpm install --frozen-lockfile --ignore-scripts/);
  });

  it('殻を組んで包むのは、どの publish よりも前（落ちれば何も出ない）', () => {
    const firstPublish = Math.min(
      stepIndex(steps, 'Publish to npm'),
      stepIndex(steps, 'Publish arc-vue to npm'),
      stepIndex(steps, 'Publish to crates.io'),
    );
    assert.ok(stepIndex(steps, 'Build arc-vue') < firstPublish);
    assert.ok(stepIndex(steps, 'Pack arc-vue') < firstPublish);
  });

  it('pnpm pack で workspace:* を版へ書き換えた tarball を作り、版と workspace: の字を検める', () => {
    const pack = steps[stepIndex(steps, 'Pack arc-vue')];
    assert.equal(pack.id, 'pack_vue');
    assert.match(pack.text, /pnpm pack --pack-destination/);
    assert.match(pack.text, /GITHUB_REF_NAME#v/);
    assert.match(pack.text, /workspace:/);
  });

  it('殻は芯の npm の publish の直後に、包んだ tarball を芯と同じ dist-tag と provenance で出す', () => {
    const core = stepIndex(steps, 'Publish to npm');
    const vue = stepIndex(steps, 'Publish arc-vue to npm');
    assert.equal(vue, core + 1, '殻の publish が芯の publish の直後にない');
    assert.match(
      steps[vue].text,
      /npm publish "\$\{\{ steps\.pack_vue\.outputs\.tarball \}\}" --provenance --access public --tag "\$\{\{ steps\.verify\.outputs\.dist_tag \}\}"/,
    );
  });

  it('latest の追随は全ての publish の後の最後の段で、芯と殻の両方に撃つ', () => {
    const follow = stepIndex(steps, 'Let npm latest follow pre-releases until the first stable release');
    assert.equal(follow, steps.length - 1);
    for (const name of ['Publish to npm', 'Publish arc-vue to npm', 'Publish to crates.io']) {
      assert.ok(stepIndex(steps, name) < follow, `${name} が latest の追随より後にある`);
    }
    assert.match(steps[follow].text, /npm-latest-follow\.sh --apply @koyori-app\/arc "/);
    assert.match(steps[follow].text, /npm-latest-follow\.sh --apply @koyori-app\/arc-vue "/);
  });
});
