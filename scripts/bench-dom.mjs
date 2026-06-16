#!/usr/bin/env node
/**
 * Layer 3: SVG-DOM benchmark.
 * Prefers Playwright headless Chromium; falls back to linkedom DOM parse
 * when Playwright browsers are unavailable (e.g. ubuntu26.04).
 */
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');

const FIXTURES = [
  '100_sparse', '100_dense',
  '500_sparse', '500_dense',
  '2000_sparse', '2000_dense',
  '5000_sparse', '5000_dense',
];

const WARMUP = 1;
const ITERS = 5;

async function tryPlaywright() {
  try {
    const { chromium } = await import('playwright');
    const browser = await chromium.launch({ headless: true });
    await browser.close();
    return chromium;
  } catch {
    return null;
  }
}

async function benchWithPlaywright(chromium) {
  const { readFileSync: _ } = await import('node:fs');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg } = await import(
    pathToFileURL(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js')).href
  );
  initSync(wasmBytes);

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const harnessUrl = pathToFileURL(join(root, 'scripts/bench-dom-harness.html')).href;

  const results = [];
  for (const name of FIXTURES) {
    await page.goto(`${harnessUrl}?fixture=${name}`);
    await page.waitForFunction(() => window.__benchReady === true);

    for (let i = 0; i < WARMUP; i++) await page.evaluate(() => window.__runBench());

    const samples = [];
    let meta = null;
    for (let i = 0; i < ITERS; i++) {
      const row = await page.evaluate(() => window.__runBench());
      samples.push(row.domMs);
      meta = row;
    }

    const sorted = [...samples].sort((a, b) => a - b);
    results.push(makeRow(name, meta, sorted[Math.floor(sorted.length / 2)], 'playwright-chromium'));
  }

  await browser.close();
  return results;
}

async function benchWithLinkedom() {
  const { parseHTML } = await import('linkedom');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg } = await import(
    pathToFileURL(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js')).href
  );
  initSync(wasmBytes);

  const results = [];
  for (const name of FIXTURES) {
    const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
    const fx = JSON.parse(readFileSync(path, 'utf8'));
    const tasksJson = JSON.stringify(fx.tasks);
    const depsJson = JSON.stringify(fx.deps);

    const { document } = parseHTML('<!doctype html><html><body><div id="host"></div></body></html>');
    const host = document.getElementById('host');

    for (let i = 0; i < WARMUP; i++) {
      const svg = render_svg(tasksJson, depsJson, fx.today);
      host.innerHTML = svg;
    }

    const domSamples = [];
    let meta = null;
    for (let i = 0; i < ITERS; i++) {
      const tWasm = performance.now();
      const svg = render_svg(tasksJson, depsJson, fx.today);
      const wasmMs = performance.now() - tWasm;
      const byteLength = Buffer.byteLength(svg, 'utf8');
      const elementCount = (svg.match(/<[^/!][^>]*>/g) ?? []).length;

      host.innerHTML = '';
      const tDom = performance.now();
      host.innerHTML = svg;
      // Simulate layout flush (linkedom has no paint; measures parse+tree build).
      void host.querySelectorAll('*').length;
      domSamples.push(performance.now() - tDom);

      meta = {
        wasmMs,
        byteLength,
        elementCount,
        taskCount: fx.tasks.length,
        depCount: fx.deps.length,
      };
    }

    const sorted = [...domSamples].sort((a, b) => a - b);
    results.push(makeRow(name, meta, sorted[Math.floor(sorted.length / 2)], 'linkedom-parse (Playwright unavailable)'));
  }

  return results;
}

function makeRow(name, meta, domMedian, engine) {
  const row = {
    fixture: name,
    tasks: meta.taskCount,
    deps: meta.depCount,
    wasm_in_browser_ms_median: round(meta.wasmMs),
    dom_insert_ms_median: round(domMedian),
    svg_bytes: meta.byteLength,
    svg_elements: meta.elementCount,
    dom_engine: engine,
  };
  console.log(
    `${name}: wasm ${row.wasm_in_browser_ms_median}ms, dom ${row.dom_insert_ms_median}ms (${engine}), ${row.svg_bytes} bytes`,
  );
  return row;
}

function round(n) {
  return Math.round(n * 100) / 100;
}

async function main() {
  let results;
  let engineNote;

  const chromium = await tryPlaywright();
  if (chromium) {
    engineNote = 'playwright-chromium';
    results = await benchWithPlaywright(chromium);
  } else {
    console.warn('Playwright Chromium unavailable — using linkedom DOM parse fallback.');
    try {
      await import('linkedom');
    } catch {
      console.log('Installing linkedom...');
      const { execSync } = await import('node:child_process');
      execSync('npm install --no-save linkedom@0.18.12', { cwd: root, stdio: 'inherit' });
    }
    engineNote = 'linkedom-fallback';
    results = await benchWithLinkedom();
  }

  const outDir = join(root, 'benches/results');
  mkdirSync(outDir, { recursive: true });
  const outPath = join(outDir, 'layer3-dom.json');
  writeFileSync(
    outPath,
    JSON.stringify({ engine: engineNote, results }, null, 2),
  );
  writeFileSync(join(outDir, 'layer3-dom-flat.json'), JSON.stringify(results, null, 2));
  console.log(`\nWrote ${outPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
