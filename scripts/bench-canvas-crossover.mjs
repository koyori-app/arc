#!/usr/bin/env node
/**
 * Phase 2 micro-bench: SVG vs Canvas crossover @ N={50,200,500,1000,2000,5000} × sparse/dense.
 * Reuses bench-3layer harness patterns (Playwright L3, Node L2, warmup + p50/p95).
 *
 * Usage: node scripts/bench-canvas-crossover.mjs [--dom-throttle N]
 */
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, join, extname, normalize } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');

const MICRO_COUNTS = [50, 200, 500, 1000, 2000, 5000];
const DENSITIES = ['sparse', 'dense'];
const FIXTURES = MICRO_COUNTS.flatMap((n) => DENSITIES.map((d) => `${n}_${d}`));

const L2_WARMUP = 3;
const L2_ITERS = 10;
const L3_WARMUP = 1;
const L3_ITERS = 5;

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
  const opts = { domThrottle: 1 };
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--dom-throttle') opts.domThrottle = Number(argv[++i]);
  }
  return opts;
}

function round(n) {
  return Math.round(n * 100) / 100;
}

function stats(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const p50 = sorted[Math.floor(sorted.length / 2)];
  const p95 = sorted[Math.ceil(sorted.length * 0.95) - 1];
  return { p50: round(p50), p95: round(p95), samples: sorted.map(round) };
}

async function benchL2Node() {
  const wasmPath = join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg, render_canvas_commands } = await import(wasmPath);
  initSync(wasmBytes);

  const results = [];
  for (const name of FIXTURES) {
    const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
    const fx = JSON.parse(readFileSync(path, 'utf8'));
    const tasksJson = JSON.stringify(fx.tasks);
    const depsJson = JSON.stringify(fx.deps);

    for (let i = 0; i < L2_WARMUP; i++) {
      render_svg(tasksJson, depsJson, fx.today);
      render_canvas_commands(tasksJson, depsJson, fx.today);
    }

    const svgSamples = [];
    const canvasSamples = [];
    let lastSvgBytes = 0;
    let lastCanvasBytes = 0;
    let lastCanvasOps = 0;

    for (let i = 0; i < L2_ITERS; i++) {
      const t0 = performance.now();
      const svg = render_svg(tasksJson, depsJson, fx.today);
      svgSamples.push(performance.now() - t0);
      lastSvgBytes = Buffer.byteLength(svg, 'utf8');

      const t1 = performance.now();
      const bufJson = render_canvas_commands(tasksJson, depsJson, fx.today);
      canvasSamples.push(performance.now() - t1);
      lastCanvasBytes = Buffer.byteLength(bufJson, 'utf8');
      const parsed = JSON.parse(bufJson);
      lastCanvasOps = parsed.ops?.length ?? 0;
    }

    const svgStats = stats(svgSamples);
    const canvasStats = stats(canvasSamples);
    const row = {
      fixture: name,
      tasks: fx.tasks.length,
      deps: fx.deps.length,
      svg_l2_p50_ms: svgStats.p50,
      svg_l2_p95_ms: svgStats.p95,
      canvas_l2_p50_ms: canvasStats.p50,
      canvas_l2_p95_ms: canvasStats.p95,
      svg_bytes: lastSvgBytes,
      canvas_bytes: lastCanvasBytes,
      canvas_ops: lastCanvasOps,
    };
    console.log(
      `${name}: L2 svg p50=${row.svg_l2_p50_ms}ms canvas p50=${row.canvas_l2_p50_ms}ms`,
    );
    results.push(row);
  }
  return results;
}

async function tryPlaywright() {
  try {
    const { chromium } = await import('playwright');
    const browser = await chromium.launch({ headless: true });
    await browser.close();
    return chromium;
  } catch (err) {
    console.warn(`Playwright Chromium unavailable (${err.message})`);
    return null;
  }
}

async function ensureNodeModule(spec, installCmd) {
  try {
    return await import(spec);
  } catch {
    console.log(`Installing ${spec}...`);
    const { execSync } = await import('node:child_process');
    execSync(installCmd, { cwd: root, stdio: 'inherit' });
    return import(spec);
  }
}

async function benchL3NodeFallback() {
  const wasmPath = join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg, render_canvas_commands } = await import(wasmPath);
  initSync(wasmBytes);

  const { parseHTML } = await ensureNodeModule('linkedom', 'npm install --no-save linkedom@0.18.12');
  const { createCanvas, Path2D } = await ensureNodeModule(
    '@napi-rs/canvas',
    'npm install --no-save @napi-rs/canvas@0.1.67',
  );
  if (typeof globalThis.Path2D === 'undefined') {
    globalThis.Path2D = Path2D;
  }
  const { replayCommands, parseCommandBuffer } = await import(
    pathToFileURL(join(root, 'scripts/bench-canvas-replay.js')).href,
  );

  const results = [];
  for (const backend of ['svg', 'canvas']) {
    for (const name of FIXTURES) {
      const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
      const fx = JSON.parse(readFileSync(path, 'utf8'));
      const tasksJson = JSON.stringify(fx.tasks);
      const depsJson = JSON.stringify(fx.deps);

      for (let i = 0; i < L3_WARMUP; i++) {
        if (backend === 'svg') {
          render_svg(tasksJson, depsJson, fx.today);
        } else {
          render_canvas_commands(tasksJson, depsJson, fx.today);
        }
      }

      const samples = [];
      let meta = null;
      for (let i = 0; i < L3_ITERS; i++) {
        if (backend === 'svg') {
          const tWasm = performance.now();
          const svg = render_svg(tasksJson, depsJson, fx.today);
          const wasmMs = performance.now() - tWasm;
          const byteLength = Buffer.byteLength(svg, 'utf8');
          const elementCount = (svg.match(/<[^/!][^>]*>/g) ?? []).length;
          const { document } = parseHTML('<!doctype html><html><body><div id="host"></div></body></html>');
          const host = document.getElementById('host');
          host.innerHTML = '';
          const tDom = performance.now();
          host.innerHTML = svg;
          void host.querySelectorAll('*').length;
          samples.push(performance.now() - tDom);
          meta = { wasmMs, byteLength, elementCount, canvasOps: null, taskCount: fx.tasks.length, depCount: fx.deps.length };
        } else {
          const tWasm = performance.now();
          const bufJson = render_canvas_commands(tasksJson, depsJson, fx.today);
          const wasmMs = performance.now() - tWasm;
          const buffer = parseCommandBuffer(bufJson);
          const byteLength = Buffer.byteLength(bufJson, 'utf8');
          const canvasOps = buffer.ops?.length ?? 0;
          const canvas = createCanvas(buffer.viewport_width, buffer.viewport_height);
          const ctx = canvas.getContext('2d');
          const tDom = performance.now();
          replayCommands(ctx, buffer);
          samples.push(performance.now() - tDom);
          meta = { wasmMs, byteLength, elementCount: 0, canvasOps, taskCount: fx.tasks.length, depCount: fx.deps.length };
        }
      }

      const domStats = stats(samples);
      results.push({
        fixture: name,
        backend,
        l3_p50_ms: domStats.p50,
        l3_p95_ms: domStats.p95,
        wasm_in_browser_p50_ms: round(meta.wasmMs),
        taskCount: meta.taskCount,
        depCount: meta.depCount,
        byteLength: meta.byteLength,
        elementCount: meta.elementCount,
        canvas_ops: meta.canvasOps,
      });
      console.log(
        `${name} ${backend}: L3 p50=${domStats.p50}ms wasm=${round(meta.wasmMs)}ms (node-fallback)`,
      );
    }
  }
  return { results, engine: 'node-linkedom-canvas-fallback' };
}

async function applyCpuThrottle(page, rate) {
  if (!rate || rate <= 1) return;
  const client = await page.context().newCDPSession(page);
  await client.send('Emulation.setCPUThrottlingRate', { rate });
}

async function benchL3Playwright(chromium, opts) {
  const { server, baseUrl } = await startStaticServer(root);
  const browser = await chromium.launch({ headless: true });
  const results = [];

  try {
    for (const backend of ['svg', 'canvas']) {
      const page = await browser.newPage();
      await applyCpuThrottle(page, opts.domThrottle);

      for (const name of FIXTURES) {
        const q = new URLSearchParams({
          fixture: name,
          backend,
          virtualize: '0',
        });
        await page.goto(`${baseUrl}/scripts/bench-dom-harness.html?${q.toString()}`);
        await page.waitForFunction(() => window.__benchReady === true);

        for (let i = 0; i < L3_WARMUP; i++) await page.evaluate(() => window.__runBench());

        const samples = [];
        let meta = null;
        for (let i = 0; i < L3_ITERS; i++) {
          const row = await page.evaluate(() => window.__runBench());
          samples.push(row.domMs);
          meta = row;
        }

        const domStats = stats(samples);
        results.push({
          fixture: name,
          backend,
          l3_p50_ms: domStats.p50,
          l3_p95_ms: domStats.p95,
          wasm_in_browser_p50_ms: meta.wasmMs,
          taskCount: meta.taskCount,
          depCount: meta.depCount,
          byteLength: meta.byteLength,
          elementCount: meta.elementCount,
          canvas_ops: meta.canvasOps ?? null,
        });
        console.log(
          `${name} ${backend}: L3 p50=${domStats.p50}ms wasm=${round(meta.wasmMs)}ms`,
        );
      }
      await page.close();
    }
  } finally {
    await browser.close();
    await closeServer(server);
  }

  return results;
}

function computeCrossover(l2Rows, l3Rows) {
  const crossovers = {};
  for (const density of DENSITIES) {
    let crossoverN = null;
    for (const n of MICRO_COUNTS) {
      const fx = `${n}_${density}`;
      const l2 = l2Rows.find((r) => r.fixture === fx);
      const l3Svg = l3Rows.find((r) => r.fixture === fx && r.backend === 'svg');
      const l3Canvas = l3Rows.find((r) => r.fixture === fx && r.backend === 'canvas');
      if (!l2 || !l3Svg || !l3Canvas) continue;

      const svgTotal = l2.svg_l2_p50_ms + l3Svg.l3_p50_ms;
      const canvasTotal = l2.canvas_l2_p50_ms + l3Canvas.l3_p50_ms;
      if (canvasTotal < svgTotal) {
        crossoverN = n;
        break;
      }
    }
    crossovers[density] = {
      crossover_n: crossoverN,
      note: crossoverN
        ? `Canvas faster than SVG from N=${crossoverN} (${density})`
        : `No crossover in measured range (${density})`,
    };
  }
  return crossovers;
}

async function main() {
  const opts = parseArgs(process.argv);
  mkdirSync(join(root, 'benches/results'), { recursive: true });

  console.log('==> Layer 2 (Node wasm boundary): svg vs canvas');
  const l2 = await benchL2Node();
  writeFileSync(join(root, 'benches/results/canvas-crossover-l2.json'), JSON.stringify(l2, null, 2));

  console.log('\n==> Layer 3: svg DOM vs canvas replay');
  const chromium = await tryPlaywright();
  let l3;
  let engine;
  if (chromium) {
    l3 = await benchL3Playwright(chromium, opts);
    engine = opts.domThrottle > 1 ? `playwright-chromium-${opts.domThrottle}x` : 'playwright-chromium';
  } else {
    console.warn('Using Node linkedom/canvas fallback (Playwright unavailable on this host).');
    const fallback = await benchL3NodeFallback();
    l3 = fallback.results;
    engine = fallback.engine;
  }
  writeFileSync(join(root, 'benches/results/canvas-crossover-l3.json'), JSON.stringify(l3, null, 2));

  const crossovers = computeCrossover(l2, l3);
  const maxCanvasL2 = Math.max(...l2.map((r) => r.canvas_l2_p50_ms));

  // Relative L2 gate (cmd_265): canvas L2 p50 must not exceed svg L2 p50 by
  // more than L2_TOLERANCE at the boundary fixture. No absolute time threshold.
  const GATE_FIXTURE = '2000_dense';
  const L2_TOLERANCE = 1.15;
  const gateRow = l2.find((r) => r.fixture === GATE_FIXTURE);
  const gateCanvasL2 = gateRow?.canvas_l2_p50_ms ?? maxCanvasL2;
  const gateSvgL2 = gateRow?.svg_l2_p50_ms ?? null;
  const l2GatePass = gateSvgL2 != null ? gateCanvasL2 <= gateSvgL2 * L2_TOLERANCE : true;

  const merged = FIXTURES.map((fx) => {
    const l2Row = l2.find((r) => r.fixture === fx);
    const l3Svg = l3.find((r) => r.fixture === fx && r.backend === 'svg');
    const l3Canvas = l3.find((r) => r.fixture === fx && r.backend === 'canvas');
    return {
      fixture: fx,
      tasks: l2Row?.tasks ?? 0,
      svg_total_p50_ms: round((l2Row?.svg_l2_p50_ms ?? 0) + (l3Svg?.l3_p50_ms ?? 0)),
      canvas_total_p50_ms: round((l2Row?.canvas_l2_p50_ms ?? 0) + (l3Canvas?.l3_p50_ms ?? 0)),
      svg_l2_p50_ms: l2Row?.svg_l2_p50_ms,
      canvas_l2_p50_ms: l2Row?.canvas_l2_p50_ms,
      svg_l3_p50_ms: l3Svg?.l3_p50_ms,
      canvas_l3_p50_ms: l3Canvas?.l3_p50_ms,
    };
  });

  const payload = {
    timestamp: new Date().toISOString(),
    engine,
    dom_throttle: opts.domThrottle,
    micro_counts: MICRO_COUNTS,
    l2_warmup: L2_WARMUP,
    l2_iters: L2_ITERS,
    l3_warmup: L3_WARMUP,
    l3_iters: L3_ITERS,
    crossovers,
    max_canvas_l2_p50_ms: round(maxCanvasL2),
    l2_canvas_gate: {
      fixture: GATE_FIXTURE,
      tolerance: L2_TOLERANCE,
      canvas_l2_p50_ms: gateCanvasL2 != null ? round(gateCanvasL2) : null,
      svg_l2_p50_ms: gateSvgL2 != null ? round(gateSvgL2) : null,
      pass: l2GatePass,
    },
    merged,
    l2,
    l3,
  };

  writeFileSync(
    join(root, 'benches/results/canvas-vs-svg-crossover.json'),
    JSON.stringify(payload, null, 2),
  );
  console.log('\nWrote benches/results/canvas-vs-svg-crossover.json');
  console.log('Crossovers:', JSON.stringify(crossovers, null, 2));
  console.log(
    `L2 canvas gate (canvas ≤ svg ×${L2_TOLERANCE} @ ${GATE_FIXTURE}): ` +
      `canvas ${gateCanvasL2 != null ? round(gateCanvasL2) : 'n/a'}ms vs ` +
      `svg ${gateSvgL2 != null ? round(gateSvgL2) : 'n/a'}ms → ${l2GatePass ? 'PASS' : 'FAIL'}`,
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
