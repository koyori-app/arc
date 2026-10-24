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
