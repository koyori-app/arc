#!/usr/bin/env node
/**
 * Layer 3: SVG-DOM benchmark.
 * Prefers Playwright headless Chromium; falls back to linkedom DOM parse
 * when Playwright browsers are unavailable (e.g. ubuntu26.04).
 *
 * Usage:
 *   node scripts/bench-dom.mjs [--virtualize] [--dom-throttle N] [--out layer3-dom-native.json]
 */
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join, extname, normalize } from 'node:path';
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
const DOM_CAP = 500;

const MIME_BY_EXT = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
};

function startStaticServer(rootDir) {
  const rootNorm = normalize(rootDir);
  return new Promise((resolve, reject) => {
    const server = createServer((req, res) => {
      try {
        const urlPath = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname);
        const filePath = normalize(join(rootNorm, urlPath));
        if (!filePath.startsWith(rootNorm)) {
          res.writeHead(403);
          res.end('Forbidden');
          return;
        }
        if (!existsSync(filePath)) {
          res.writeHead(404);
          res.end('Not Found');
          return;
        }
        const body = readFileSync(filePath);
        const ext = extname(filePath).toLowerCase();
        res.writeHead(200, { 'Content-Type': MIME_BY_EXT[ext] ?? 'application/octet-stream' });
        res.end(body);
      } catch (err) {
        res.writeHead(500);
        res.end(String(err));
      }
    });
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({ server, baseUrl: `http://127.0.0.1:${port}` });
    });
    server.on('error', reject);
  });
}

function closeServer(server) {
  return new Promise((resolve, reject) => {
    server.close((err) => (err ? reject(err) : resolve()));
  });
}

function parseArgs(argv) {
  const opts = {
    virtualize: true,
    domThrottle: 1,
    out: 'layer3-dom.json',
  };
  for (let i = 2; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--virtualize') {
      opts.virtualize = true;
    } else if (arg === '--no-virtualize') {
      opts.virtualize = false;
    } else if (arg === '--dom-throttle') {
      opts.domThrottle = Number(argv[++i]);
    } else if (arg === '--out') {
      opts.out = argv[++i];
    }
  }
  return opts;
}

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

function harnessQuery(opts) {
  const q = new URLSearchParams({ fixture: 'PLACEHOLDER' });
  q.set('virtualize', opts.virtualize ? '1' : '0');
  q.set('scroll_y', '0');
  q.set('client_height', '600');
  return q;
}

async function applyCpuThrottle(page, rate) {
  if (!rate || rate <= 1) return;
  const client = await page.context().newCDPSession(page);
  await client.send('Emulation.setCPUThrottlingRate', { rate });
}

async function benchWithPlaywright(chromium, opts) {
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync } = await import(
    pathToFileURL(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js')).href
  );
  initSync(wasmBytes);

  const { server, baseUrl } = await startStaticServer(root);
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await applyCpuThrottle(page, opts.domThrottle);

    const results = [];
    for (const name of FIXTURES) {
      const q = harnessQuery(opts);
      q.set('fixture', name);
      await page.goto(`${baseUrl}/scripts/bench-dom-harness.html?${q.toString()}`);
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
      results.push(
        makeRow(name, meta, sorted[Math.floor(sorted.length / 2)], 'playwright-chromium', opts),
      );
    }

    return results;
  } finally {
    await browser.close();
    await closeServer(server);
  }
}

async function benchWithLinkedom(opts) {
  const { parseHTML } = await import('linkedom');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg } = await import(
    pathToFileURL(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js')).href
  );
  initSync(wasmBytes);

  const viewportJson = opts.virtualize
    ? JSON.stringify({ scroll_y: 0, client_height: 600 })
    : undefined;

  const results = [];
  for (const name of FIXTURES) {
    const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
    const fx = JSON.parse(readFileSync(path, 'utf8'));
    const tasksJson = JSON.stringify(fx.tasks);
    const depsJson = JSON.stringify(fx.deps);

    const { document } = parseHTML('<!doctype html><html><body><div id="host"></div></body></html>');
    const host = document.getElementById('host');

    for (let i = 0; i < WARMUP; i++) {
      const svg = render_svg(tasksJson, depsJson, fx.today, viewportJson);
      host.innerHTML = svg;
    }

    const domSamples = [];
    let meta = null;
    for (let i = 0; i < ITERS; i++) {
      const tWasm = performance.now();
      const svg = render_svg(tasksJson, depsJson, fx.today, viewportJson);
      const wasmMs = performance.now() - tWasm;
      const byteLength = Buffer.byteLength(svg, 'utf8');
      const elementCount = (svg.match(/<[^/!][^>]*>/g) ?? []).length;

      host.innerHTML = '';
      const tDom = performance.now();
      host.innerHTML = svg;
      const liveElems = host.querySelectorAll('*').length;
      void liveElems;
      domSamples.push(performance.now() - tDom);

      meta = {
        wasmMs,
        byteLength,
        elementCount,
        liveElems,
        virtualize: opts.virtualize,
        taskCount: fx.tasks.length,
        depCount: fx.deps.length,
      };
    }

    const sorted = [...domSamples].sort((a, b) => a - b);
    const note = opts.domThrottle > 1
      ? `linkedom-parse (CPU throttle ${opts.domThrottle}x not emulated)`
      : 'linkedom-parse (Playwright unavailable)';
    results.push(makeRow(name, meta, sorted[Math.floor(sorted.length / 2)], note, opts));
  }

  return results;
}

function makeRow(name, meta, domMedian, engine, opts) {
  const row = {
    fixture: name,
    tasks: meta.taskCount,
    deps: meta.depCount,
    wasm_in_browser_ms_median: round(meta.wasmMs),
    dom_insert_ms_median: round(domMedian),
    svg_bytes: meta.byteLength,
    svg_elements: meta.elementCount,
    live_svg_elems: meta.liveElems ?? meta.elementCount,
    virtualize: meta.virtualize ?? opts.virtualize,
    dom_cap_pass: opts.virtualize
      ? (meta.liveElems ?? meta.elementCount) <=
        ((meta.taskCount ?? 0) <= 2000 ? DOM_CAP : DOM_CAP * 4)
      : null,
    dom_engine: engine,
    dom_throttle: opts.domThrottle,
  };
  console.log(
    `${name}: wasm ${row.wasm_in_browser_ms_median}ms, dom ${row.dom_insert_ms_median}ms ` +
      `(${engine}, virtualize=${row.virtualize}, elems=${row.live_svg_elems})`,
  );
  return row;
}

function round(n) {
  return Math.round(n * 100) / 100;
}

async function main() {
  const opts = parseArgs(process.argv);
  let results;
  let engineNote;

  const chromium = await tryPlaywright();
  if (chromium) {
    engineNote = 'playwright-chromium';
    results = await benchWithPlaywright(chromium, opts);
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
    results = await benchWithLinkedom(opts);
  }

  const outDir = join(root, 'benches/results');
  mkdirSync(outDir, { recursive: true });
  const outPath = join(outDir, opts.out);
  const payload = {
    engine: engineNote,
    virtualize: opts.virtualize,
    dom_throttle: opts.domThrottle,
    dom_cap: DOM_CAP,
    results,
  };
  writeFileSync(outPath, JSON.stringify(payload, null, 2));
  const flatName = opts.out.replace(/\.json$/, '-flat.json');
  writeFileSync(join(outDir, flatName), JSON.stringify(results, null, 2));
  console.log(`\nWrote ${outPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
