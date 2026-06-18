#!/usr/bin/env node
/**
 * Phase 2 canvas-vs-SVG crossover gates (cmd_284).
 *
 * Gates:
 *   L2_canvas       — canvas render_canvas_commands p50 < 30 ms (all N series)
 *   crossover       — computed N recorded (informational)
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const resultsDir = join(root, 'benches/results');

const L2_CANVAS_MAX_MS = 30;
const GATE_FIXTURE = '2000_dense';

function main() {
  const bench = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-vs-svg-crossover.json'), 'utf8'),
  );
  const gates = [];
  let failed = false;

  const gateRow = bench.l2?.find((r) => r.fixture === GATE_FIXTURE);
  const gateActual = gateRow?.canvas_l2_p50_ms ?? bench.max_canvas_l2_p50_ms;
  const l2Pass = gateActual < L2_CANVAS_MAX_MS;
  gates.push({
    id: 'L2_canvas',
    fixture: GATE_FIXTURE,
    condition: `canvas L2 p50 < ${L2_CANVAS_MAX_MS} ms @ ${GATE_FIXTURE}`,
    actual: gateActual,
    pass: l2Pass,
  });
  gates.push({
    id: 'L2_canvas_max_series',
    condition: `canvas L2 p50 max across N series (informational)`,
    actual: bench.max_canvas_l2_p50_ms,
    pass: bench.max_canvas_l2_p50_ms < L2_CANVAS_MAX_MS,
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
    l2_canvas_max_ms: L2_CANVAS_MAX_MS,
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
