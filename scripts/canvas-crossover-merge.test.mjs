import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { computeMergedTotals } from './canvas-crossover-merge.mjs';

const scriptsDir = dirname(fileURLToPath(import.meta.url));

const FIXTURES = ['50_sparse', '5000_dense'];
const L2 = [
  { fixture: '50_sparse', tasks: 50, svg_l2_p50_ms: 1, canvas_l2_p50_ms: 0.5 },
  { fixture: '5000_dense', tasks: 5000, svg_l2_p50_ms: 40, canvas_l2_p50_ms: 20 },
];
const L3 = [
  { fixture: '50_sparse', backend: 'svg', l3_p50_ms: 2 },
  { fixture: '50_sparse', backend: 'canvas', l3_p50_ms: 1 },
  { fixture: '5000_dense', backend: 'svg', l3_p50_ms: 300 },
];

describe('computeMergedTotals', () => {
  it('sums L2 and L3 when both are measured', () => {
    const [row] = computeMergedTotals(FIXTURES, L2, L3);
    assert.equal(row.svg_total_p50_ms, 3);
    assert.equal(row.canvas_total_p50_ms, 1.5);
  });

  it('leaves the canvas total empty when the canvas L3 row is missing', () => {
    const row = computeMergedTotals(FIXTURES, L2, L3)[1];
    assert.equal(row.canvas_total_p50_ms, null);
    assert.equal(row.canvas_l3_p50_ms, null);
    assert.equal(row.svg_total_p50_ms, 340);
  });

  it('leaves the svg total empty when the svg L3 row is missing', () => {
    const l3 = L3.filter((r) => !(r.fixture === '50_sparse' && r.backend === 'svg'));
    const [row] = computeMergedTotals(FIXTURES, L2, l3);
    assert.equal(row.svg_total_p50_ms, null);
    assert.equal(row.svg_l3_p50_ms, null);
    assert.equal(row.canvas_total_p50_ms, 1.5);
  });

  it('leaves both totals empty when the L2 row is missing', () => {
    const [row] = computeMergedTotals(FIXTURES, L2.slice(1), L3);
    assert.equal(row.svg_total_p50_ms, null);
    assert.equal(row.canvas_total_p50_ms, null);
  });
});

function renderReport(merged) {
  const tempRoot = mkdtempSync(join(tmpdir(), 'canvas-report-'));
  const tempScripts = join(tempRoot, 'scripts');
  mkdirSync(tempScripts, { recursive: true });
  for (const file of ['generate-canvas-crossover-report.mjs', 'canvas-crossover-gate-lib.mjs']) {
    cpSync(join(scriptsDir, file), join(tempScripts, file));
  }
  const resultsDir = join(tempRoot, 'benches/results');
  mkdirSync(resultsDir, { recursive: true });
  writeFileSync(
    join(resultsDir, 'canvas-vs-svg-crossover.json'),
    JSON.stringify({
      timestamp: '2026-01-01T00:00:00.000Z',
      engine: 'test',
      dom_throttle: 1,
      micro_counts: [50, 5000],
      crossovers: {},
      l3_skipped: [{ fixture: '5000_dense', backend: 'canvas', code: 'canvas_capacity' }],
      merged,
    }),
  );
  writeFileSync(
    join(resultsDir, 'canvas-crossover-gates.json'),
    JSON.stringify({ gates: [], all_pass: true }),
  );
  execFileSync('node', [join(tempScripts, 'generate-canvas-crossover-report.mjs')], {
    cwd: tempRoot,
    stdio: 'pipe',
  });
  return readFileSync(join(resultsDir, 'canvas-vs-svg-crossover-report.md'), 'utf8');
}

describe('crossover report merged table', () => {
  it('shows — for pairs that were not measured', () => {
    const md = renderReport(computeMergedTotals(FIXTURES, L2, L3));
    const row = md.split('\n').find((line) => line.startsWith('| 5000_dense |'));
    assert.ok(row, 'merged row for 5000_dense is present');
    const cells = row.split('|').map((c) => c.trim());
    // | Fixture | Tasks | SVG L2 | Canvas L2 | SVG L3 | Canvas L3 | SVG total | Canvas total |
    assert.equal(cells[6], '—');
    assert.equal(cells[8], '—');
    assert.equal(cells[7], '340');
    assert.doesNotMatch(md, /undefined|null|NaN/);
  });
});
