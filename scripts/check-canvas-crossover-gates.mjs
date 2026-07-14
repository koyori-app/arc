#!/usr/bin/env node
/**
 * Phase 2 canvas-vs-SVG crossover gates (cmd_284).
 *
 * Gates:
 *   L2_canvas       — canvas L2 p50 ≤ svg L2 p50 × tolerance @ gate fixture (relative)
 *   crossover       — computed N recorded (informational)
 *
 * cmd_265: no absolute time thresholds tied to fast hardware. The L2_canvas
 * gate is relative (canvas must not be slower than svg at the boundary), so it
 * measures quality — not the speed of whichever runner happens to execute it.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  evaluateGateFromL2Rows,
  formatGateActual,
  GATE_FIXTURE,
  L2_TOLERANCE,
} from './canvas-crossover-gate-lib.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const resultsDir = join(root, 'benches/results');

function main() {
  const bench = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-vs-svg-crossover.json'), 'utf8'),
  );
  const gates = [];
  let failed = false;

  const gateEval = evaluateGateFromL2Rows(bench.l2, {
    fixture: GATE_FIXTURE,
    tolerance: L2_TOLERANCE,
  });
  const l2Pass = gateEval.pass;
  gates.push({
    id: 'L2_canvas',
    fixture: GATE_FIXTURE,
    condition: `canvas L2 p50 ≤ svg L2 p50 × ${L2_TOLERANCE} @ ${GATE_FIXTURE}`,
    actual: formatGateActual(gateEval),
    pass: l2Pass,
    ...(gateEval.reason ? { note: gateEval.reason } : {}),
  });
  gates.push({
    id: 'L2_canvas_max_series',
    condition: `canvas L2 p50 max across N series (informational)`,
    actual: bench.max_canvas_l2_p50_ms,
    pass: true,
    advisory: true,
  });
  if (!l2Pass) failed = true;

  for (const [density, info] of Object.entries(bench.crossovers ?? {})) {
    gates.push({
      id: 'crossover',
      fixture: density,
      condition: 'SVG vs canvas total p50 crossover N',
      actual: info.crossover_n,
      pass: true,
      advisory: true,
      note: info.note,
    });
  }

  if (bench.engine !== 'playwright-chromium' && !String(bench.engine).startsWith('playwright-chromium')) {
    gates.push({
      id: 'L3_engine',
      condition: 'L3 measured with Playwright Chromium (CI primary)',
      actual: bench.engine,
      pass: true,
      advisory: true,
      note: 'Local fallback engine; CI workflow_dispatch uses Playwright',
    });
  }

  const report = {
    timestamp: new Date().toISOString(),
    l2_tolerance: L2_TOLERANCE,
    gate_fixture: GATE_FIXTURE,
    gates,
    crossovers: bench.crossovers,
    all_pass: !failed,
  };

  mkdirSync(resultsDir, { recursive: true });
  writeFileSync(join(resultsDir, 'canvas-crossover-gates.json'), JSON.stringify(report, null, 2));

  console.log('\n=== Canvas Crossover Gates ===');
  for (const g of gates) {
    const status = g.advisory ? 'INFO' : g.pass ? 'PASS' : 'FAIL';
    console.log(`[${status}] ${g.id}${g.fixture ? ` (${g.fixture})` : ''}: ${g.actual ?? 'n/a'} — ${g.condition}`);
    if (g.note) console.log(`         ${g.note}`);
  }
  console.log(`\nOverall: ${failed ? 'FAIL' : 'PASS'}`);

  if (failed) process.exit(1);
}

main();
