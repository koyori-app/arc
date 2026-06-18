#!/usr/bin/env node
/** Markdown report for canvas-vs-SVG crossover micro-bench. */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';

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

function main() {
  const bench = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-vs-svg-crossover.json'), 'utf8'),
  );
  const gates = JSON.parse(
    readFileSync(join(resultsDir, 'canvas-crossover-gates.json'), 'utf8'),
  );

  const md = [];
  md.push('# Canvas vs SVG Crossover Micro-Bench (cmd_284)');
  md.push('');
  md.push(`- **Timestamp**: ${bench.timestamp}`);
  md.push(`- **CPU**: ${cpuInfo()}`);
  md.push(`- **Engine**: ${bench.engine}`);
  md.push(`- **DOM throttle**: ${bench.dom_throttle}x`);
  md.push(`- **N series**: ${bench.micro_counts.join(', ')}`);
  md.push(`- **L2 gate**: canvas p50 < ${bench.l2_canvas_gate_ms} ms → **${bench.l2_canvas_gate_pass ? 'PASS' : 'FAIL'}** (max=${bench.max_canvas_l2_p50_ms} ms)`);
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
