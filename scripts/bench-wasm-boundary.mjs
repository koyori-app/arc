#!/usr/bin/env node
/**
 * Layer 2: Wasm/JS boundary benchmark (Node + wasm-bindgen).
 * Measures render_svg() call through the compiled Wasm module.
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const require = createRequire(import.meta.url);

const FIXTURES = [
  '100_sparse', '100_dense',
  '500_sparse', '500_dense',
  '2000_sparse', '2000_dense',
  '5000_sparse', '5000_dense',
];

const WARMUP = 3;
const ITERS = 10;

async function main() {
  const wasmPath = join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js');
  const wasmBytes = readFileSync(
    join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'),
  );
  const { initSync, render_svg } = await import(wasmPath);
  initSync(wasmBytes);

  const results = [];

  for (const name of FIXTURES) {
    const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
    const fx = JSON.parse(readFileSync(path, 'utf8'));
    const tasksJson = JSON.stringify(fx.tasks);
    const depsJson = JSON.stringify(fx.deps);

    for (let i = 0; i < WARMUP; i++) {
      render_svg(tasksJson, depsJson, fx.today);
    }

    const samples = [];
    let lastSvg = '';
    for (let i = 0; i < ITERS; i++) {
      const t0 = performance.now();
      lastSvg = render_svg(tasksJson, depsJson, fx.today);
      samples.push(performance.now() - t0);
    }

    const sorted = [...samples].sort((a, b) => a - b);
    const median = sorted[Math.floor(sorted.length / 2)];
    const byteLength = Buffer.byteLength(lastSvg, 'utf8');
    const elementCount = (lastSvg.match(/<[^/!][^>]*>/g) ?? []).length;

    results.push({
      fixture: name,
      tasks: fx.tasks.length,
      deps: fx.deps.length,
      wasm_boundary_ms_median: round(median),
      svg_bytes: byteLength,
      svg_elements: elementCount,
    });
    console.log(`${name}: wasm ${round(median)}ms, ${byteLength} bytes, ${elementCount} elements`);
  }

  const outDir = join(root, 'benches/results');
  mkdirSync(outDir, { recursive: true });
  const outPath = join(outDir, 'layer2-wasm-boundary.json');
  writeFileSync(outPath, JSON.stringify(results, null, 2));
  console.log(`\nWrote ${outPath}`);
}

function round(n) {
  return Math.round(n * 100) / 100;
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
