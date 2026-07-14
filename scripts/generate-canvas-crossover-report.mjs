#!/usr/bin/env node
/** Markdown report for canvas-vs-SVG crossover micro-bench. */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';
import { formatGateActual } from './canvas-crossover-gate-lib.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const resultsDir = join(root, 'benches/results');

function cpuInfo() {
  try {
    return execSync("lscpu | grep 'Model name' | sed 's/Model name:[[:space:]]*//'", {
      encoding: 'utf8',
    }).trim();
  } catch {
    return 'unknown';
  }
}

function resolveL2GateSummary(bench, gatesReport) {
  const gateFromGates = gatesReport.gates?.find((g) => g.id === 'L2_canvas');
  const benchGate = bench.l2_canvas_gate;

  const fixture = gatesReport.gate_fixture ?? benchGate?.fixture ?? '2000_dense';
  const tolerance = gatesReport.l2_tolerance ?? benchGate?.tolerance ?? 'n/a';
  const pass = gateFromGates?.pass ?? benchGate?.pass ?? false;
  const actual =
    gateFromGates?.actual ??
    (benchGate
      ? formatGateActual({
          pass: benchGate.pass,
          canvasL2: benchGate.canvas_l2_p50_ms ?? null,
          svgL2: benchGate.svg_l2_p50_ms ?? null,
        })
      : 'n/a');

  return { fixture, tolerance, pass, actual };
}

function main() {
  const bench = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-vs-svg-crossover.json'), 'utf8'),
  );
  const gates = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-crossover-gates.json'), 'utf8'),
  );

  const l2Gate = resolveL2GateSummary(bench, gates);

  const md = [];
  md.push('# Canvas vs SVG Crossover Micro-Bench (cmd_284)');
  md.push('');
  md.push(`- **Timestamp**: ${bench.timestamp}`);
  md.push(`- **CPU**: ${cpuInfo()}`);
  md.push(`- **Engine**: ${bench.engine}`);
  md.push(`- **DOM throttle**: ${bench.dom_throttle}x`);
  md.push(`- **N series**: ${bench.micro_counts.join(', ')}`);
  md.push(
    `- **L2 gate**: canvas L2 p50 ≤ svg L2 p50 × ${l2Gate.tolerance} @ ${l2Gate.fixture} → **${l2Gate.pass ? 'PASS' : 'FAIL'}** (${l2Gate.actual})`,
  );
  md.push('');

  md.push('## Crossover N (L2+L3 total p50)');
  md.push('');
  md.push('| Density | Crossover N | Note |');
  md.push('|---------|-------------|------|');
  for (const [density, info] of Object.entries(bench.crossovers)) {
    md.push(`| ${density} | ${info.crossover_n ?? '—'} | ${info.note} |`);
  }
  md.push('');

  md.push('## Merged totals (p50 ms)');
  md.push('');
  md.push('| Fixture | Tasks | SVG L2 | Canvas L2 | SVG L3 | Canvas L3 | SVG total | Canvas total |');
  md.push('|---------|------:|-------:|----------:|-------:|----------:|----------:|-------------:|');
  for (const row of bench.merged) {
    md.push(
      `| ${row.fixture} | ${row.tasks} | ${row.svg_l2_p50_ms} | ${row.canvas_l2_p50_ms} | ${row.svg_l3_p50_ms} | ${row.canvas_l3_p50_ms} | ${row.svg_total_p50_ms} | ${row.canvas_total_p50_ms} |`,
    );
  }
  md.push('');

  md.push('## Gates');
  md.push('');
  for (const g of gates.gates) {
    const status = g.advisory ? 'INFO' : g.pass ? 'PASS' : 'FAIL';
    md.push(`- **[${status}]** ${g.id}: ${g.condition} → ${g.actual ?? 'n/a'}`);
  }
  md.push('');
  md.push(`**Overall**: ${gates.all_pass ? 'PASS' : 'FAIL'}`);

  mkdirSync(resultsDir, { recursive: true });
  const outPath = join(resultsDir, 'canvas-vs-svg-crossover-report.md');
  writeFileSync(outPath, md.join('\n'));
  console.log(`Wrote ${outPath}`);
}

main();
