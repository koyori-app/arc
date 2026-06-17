#!/usr/bin/env node
/**
 * §6.6.2 CI performance gates for Phase 1 virtualization bench.
 *
 * Gates:
 *   L3_throttled     — 2000_dense, 4× CPU, L3 p50 < 500 ms
 *   DOM_CAP            — virtualize ON: live_svg_elems ≤ DOM_CAP (500)
 *   regression_native  — virtualized native L3 ≤ cmd_263 baseline × 1.2 (advisory log on fail)
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const resultsDir = join(root, 'benches/results');

const DOM_CAP = 500;
const DOM_CAP_PHASE1_MARGIN = 4; // matches display_list_tests.rs p1_dom_cap_invariant
const L3_THROTTLED_MAX_MS = 500;
const REGRESSION_FACTOR = 1.2;
const GATE_FIXTURE = '2000_dense';

function readBench(name) {
  const raw = JSON.parse(readFileSync(join(resultsDir, name), 'utf8'));
  return {
    meta: raw,
    results: raw.results,
  };
}

function findRow(results, fixture) {
  return results.find((r) => r.fixture === fixture);
}

function parseArgs(argv) {
  const opts = { domThrottle: null };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--dom-throttle') {
      opts.domThrottle = Number(argv[++i]);
    }
  }
  return opts;
}

function main() {
  const opts = parseArgs(process.argv);
  const gates = [];
  let failed = false;

  const native = readBench('layer3-dom-native.json');
  const baseline = JSON.parse(
    readFileSync(join(resultsDir, 'cmd_263-baseline.json'), 'utf8'),
  );

  // DOM_CAP gate — strict at 2000_dense (§6.6.2); Phase1 margin for larger fixtures
  for (const row of native.results) {
    const elems = row.live_svg_elems ?? row.svg_elements;
    const tasks = row.tasks ?? 0;
    const strict = tasks <= 2000;
    const limit = strict ? DOM_CAP : DOM_CAP * DOM_CAP_PHASE1_MARGIN;
    const pass = elems <= limit;
    gates.push({
      id: 'DOM_CAP',
      fixture: row.fixture,
      condition: strict
        ? `live_svg_elems ≤ ${DOM_CAP} (strict)`
        : `live_svg_elems ≤ ${DOM_CAP * DOM_CAP_PHASE1_MARGIN} (Phase1 margin)`,
      actual: elems,
      pass,
    });
    if (!pass) failed = true;
  }

  // regression_native — advisory (design: 助言)
  const nativeRow = findRow(native.results, GATE_FIXTURE);
  const baselineMs = baseline.l3_dom_ms[GATE_FIXTURE];
  const nativeMs = nativeRow?.dom_insert_ms_median ?? Infinity;
  const regressionLimit = baselineMs * REGRESSION_FACTOR;
  const regressionPass = nativeMs <= regressionLimit;
  gates.push({
    id: 'regression_native',
    fixture: GATE_FIXTURE,
    condition: `native L3 ≤ cmd_263 ${baselineMs}ms × ${REGRESSION_FACTOR} = ${round(regressionLimit)}ms`,
    actual: nativeMs,
    pass: regressionPass,
    advisory: true,
  });
  if (!regressionPass) {
    console.warn(`[advisory] regression_native: ${nativeMs}ms > ${round(regressionLimit)}ms`);
  }

  // L3_throttled — requires throttled bench output
  const throttleFile =
    opts.domThrottle != null
      ? `layer3-dom-throttle-${opts.domThrottle}x.json`
      : null;

  if (throttleFile) {
    const throttled = readBench(throttleFile);
    const thrRow = findRow(throttled.results, GATE_FIXTURE);
    const thrMs = thrRow?.dom_insert_ms_median ?? Infinity;
    const thrPass = thrMs < L3_THROTTLED_MAX_MS;
    const throttleEmulated =
      throttled.meta.engine === 'playwright-chromium' &&
      (throttled.meta.dom_throttle ?? 1) > 1;
    gates.push({
      id: 'L3_throttled',
      fixture: GATE_FIXTURE,
      condition: `4× CPU L3 p50 < ${L3_THROTTLED_MAX_MS}ms`,
      actual: thrMs,
      dom_throttle: opts.domThrottle,
      pass: thrPass,
      advisory: !throttleEmulated,
      note: throttleEmulated
        ? null
        : 'Playwright CDP throttle unavailable; linkedom fallback does not emulate CPU slowdown',
    });
    if (!thrPass) failed = true;
    if (!throttleEmulated) {
      console.warn(`[advisory] L3_throttled: CPU throttle not emulated (${throttled.meta.engine})`);
    }
  } else {
    gates.push({
      id: 'L3_throttled',
      fixture: GATE_FIXTURE,
      condition: `4× CPU L3 p50 < ${L3_THROTTLED_MAX_MS}ms`,
      actual: null,
      pass: false,
      skipped: 'Run with --dom-throttle 4',
    });
    failed = true;
  }

  const report = {
    timestamp: new Date().toISOString(),
    dom_cap: DOM_CAP,
    gates,
    all_pass: !failed,
  };

  mkdirSync(resultsDir, { recursive: true });
  writeFileSync(join(resultsDir, 'bench-gates.json'), JSON.stringify(report, null, 2));

  console.log('\n=== §6.6.2 Bench Gates ===');
  for (const g of gates) {
    const status = g.skipped ? 'SKIP' : g.pass ? 'PASS' : 'FAIL';
    console.log(`[${status}] ${g.id} (${g.fixture}): ${g.actual ?? 'n/a'} — ${g.condition}`);
  }
  console.log(`\nOverall: ${failed ? 'FAIL' : 'PASS'}`);

  if (failed) process.exit(1);
}

function round(n) {
  return Math.round(n * 100) / 100;
}

main();
