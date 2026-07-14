import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cpSync, mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

const scriptsDir = dirname(fileURLToPath(import.meta.url));

function runGateCheck(benchPayload) {
  const tempRoot = mkdtempSync(join(tmpdir(), 'canvas-gate-'));
  const tempScripts = join(tempRoot, 'scripts');
  mkdirSync(tempScripts, { recursive: true });
  for (const file of [
    'check-canvas-crossover-gates.mjs',
    'canvas-crossover-gate-lib.mjs',
  ]) {
    cpSync(join(scriptsDir, file), join(tempScripts, file));
  }

  const resultsDir = join(tempRoot, 'benches/results');
  mkdirSync(resultsDir, { recursive: true });
  writeFileSync(join(resultsDir, 'canvas-vs-svg-crossover.json'), JSON.stringify(benchPayload));

  let exitCode = 0;
  try {
    execFileSync('node', [join(tempScripts, 'check-canvas-crossover-gates.mjs')], {
      cwd: tempRoot,
      stdio: 'pipe',
    });
  } catch (err) {
    exitCode = err.status ?? 1;
  }

  const gates = JSON.parse(readFileSync(join(resultsDir, 'canvas-crossover-gates.json'), 'utf8'));
  return { exitCode, gates };
}

describe('check-canvas-crossover-gates integration', () => {
  const baseBench = {
    engine: 'playwright-chromium',
    dom_throttle: 1,
    micro_counts: [2000],
    crossovers: {},
    max_canvas_l2_p50_ms: 99,
    merged: [],
    l2: [
      {
        fixture: '2000_dense',
        canvas_l2_p50_ms: 20,
        svg_l2_p50_ms: 25,
      },
    ],
  };

  it('exits 0 and passes with valid gate metrics', () => {
    const { exitCode, gates } = runGateCheck(baseBench);
    assert.equal(exitCode, 0);
    assert.equal(gates.all_pass, true);
    assert.equal(gates.gates.find((g) => g.id === 'L2_canvas')?.pass, true);
  });

  it('exits 1 when svg baseline is missing', () => {
    const bench = structuredClone(baseBench);
    delete bench.l2[0].svg_l2_p50_ms;
    const { exitCode, gates } = runGateCheck(bench);
    assert.equal(exitCode, 1);
    assert.equal(gates.all_pass, false);
    assert.equal(gates.gates.find((g) => g.id === 'L2_canvas')?.pass, false);
  });

  it('exits 1 when gate fixture row is missing', () => {
    const bench = { ...baseBench, l2: [] };
    const { exitCode, gates } = runGateCheck(bench);
    assert.equal(exitCode, 1);
    assert.equal(gates.gates.find((g) => g.id === 'L2_canvas')?.pass, false);
  });

  it('exits 1 for negative svg metric', () => {
    const bench = structuredClone(baseBench);
    bench.l2[0].svg_l2_p50_ms = -1;
    const { exitCode, gates } = runGateCheck(bench);
    assert.equal(exitCode, 1);
    assert.equal(gates.gates.find((g) => g.id === 'L2_canvas')?.pass, false);
  });
});
