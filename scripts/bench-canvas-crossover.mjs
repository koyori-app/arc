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
import { computeMergedTotals } from './canvas-crossover-merge.mjs';
import {
  evaluateGateFromL2Rows,
  GATE_FIXTURE,
  L2_TOLERANCE,
} from './canvas-crossover-gate-lib.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');

const MICRO_COUNTS = [50, 200, 500, 1000, 2000, 5000];
const DENSITIES = ['sparse', 'dense'];
const FIXTURES = MICRO_COUNTS.flatMap((n) => DENSITIES.map((d) => `${n}_${d}`));

// L3 fixture/backend pairs we could not measure (e.g. canvas capacity
// exceeded). Logged at the end and recorded as l3_skipped in the payload.
const l3Skips = [];

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

/**
 * `parseCommandBuffer` (and `replayCommands`) for L2 and L3.
 *
 * bench-canvas-replay.js has no imports of its own, so L2 can load it without
 * @napi-rs/canvas. Do not use the same-named export of
 * packages/arc-vue/src/replayCommands.ts: it never throws on {code, error}.
 */
function importReplayModule() {
  return import(pathToFileURL(join(root, 'scripts/bench-canvas-replay.js')).href);
}

async function benchL2Node() {
  const wasmPath = join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core.js');
  const wasmBytes = readFileSync(join(root, 'crates/koyori-arc-core/pkg/koyori_arc_core_bg.wasm'));
  const { initSync, render_svg, render_canvas_commands } = await import(wasmPath);
  initSync(wasmBytes);
  const { parseCommandBuffer } = await importReplayModule();

  const results = [];
  for (const name of FIXTURES) {
    const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
    const fx = JSON.parse(readFileSync(path, 'utf8'));
    const row = benchL2Fixture(name, fx, {
      render_svg,
      render_canvas_commands,
      parseCommandBuffer,
    });
    console.log(
      row.canvas_error
        ? `${name}: L2 svg p50=${row.svg_l2_p50_ms}ms canvas SKIPPED (${row.canvas_error})`
        : `${name}: L2 svg p50=${row.svg_l2_p50_ms}ms canvas p50=${row.canvas_l2_p50_ms}ms`,
    );
    results.push(row);
  }
  return results;
}

/**
 * One L2 fixture: time svg and canvas generation in Node.
 *
 * The render functions and `parseCommandBuffer` are passed in so tests can
 * drive this without the wasm pkg.
 */
export function benchL2Fixture(name, fx, deps, opts = {}) {
  const { render_svg, render_canvas_commands, parseCommandBuffer } = deps;
  const warmup = opts.warmup ?? L2_WARMUP;
  const iters = opts.iters ?? L2_ITERS;
  const tasksJson = JSON.stringify(fx.tasks);
  const depsJson = JSON.stringify(fx.deps);

  // Probe once: capacity-exceeded fixtures return {code, error} instead
  // of a CommandBuffer. Timing that error return as "canvas" made the
  // relative gate compare a real svg render against an instant error
  // response, so a green gate guaranteed nothing. A capacity refusal is kept
  // in `canvas_error` with the canvas timing left empty, so the gate fails
  // closed; any other code throws here, as it does in L3
  // (`isExpectedRenderError`).
  let canvasError;
  try {
    canvasError = probeCanvasError(
      parseCommandBuffer,
      render_canvas_commands(tasksJson, depsJson, fx.today),
    );
  } catch (err) {
    // The run stops here before any row or log line names the fixture, so
    // put the name in the message, in the same shape as L3
    // (`${name} ${backend}: render failed (…)`). L2 probes only canvas, and
    // the parsed message already says render_canvas_commands, so there is no
    // backend part. The same error is rethrown, so `renderErrorCode` and the
    // error type stay as they were.
    if (err instanceof Error) {
      err.message = err.renderErrorCode
        ? `${name}: render failed (${err.renderErrorCode}): ${err.message}`
        : `${name}: ${err.message}`;
    }
    throw err;
  }

  for (let i = 0; i < warmup; i++) {
    render_svg(tasksJson, depsJson, fx.today);
    if (!canvasError) render_canvas_commands(tasksJson, depsJson, fx.today);
  }

  const svgSamples = [];
  const canvasSamples = [];
  let lastSvgBytes = 0;
  let lastCanvasBytes = 0;
  let lastCanvasOps = 0;

  for (let i = 0; i < iters; i++) {
    const t0 = performance.now();
    const svg = render_svg(tasksJson, depsJson, fx.today);
    svgSamples.push(performance.now() - t0);
    lastSvgBytes = Buffer.byteLength(svg, 'utf8');

    if (!canvasError) {
      const t1 = performance.now();
      const bufJson = render_canvas_commands(tasksJson, depsJson, fx.today);
      canvasSamples.push(performance.now() - t1);
      lastCanvasBytes = Buffer.byteLength(bufJson, 'utf8');
      lastCanvasOps = parseCommandBuffer(bufJson).ops?.length ?? 0;
    }
  }

  const svgStats = stats(svgSamples);
  const canvasStats = canvasError ? null : stats(canvasSamples);
  return {
    fixture: name,
    tasks: fx.tasks.length,
    deps: fx.deps.length,
    svg_l2_p50_ms: svgStats.p50,
    svg_l2_p95_ms: svgStats.p95,
    canvas_l2_p50_ms: canvasStats ? canvasStats.p50 : null,
    canvas_l2_p95_ms: canvasStats ? canvasStats.p95 : null,
    svg_bytes: lastSvgBytes,
    canvas_bytes: canvasError ? null : lastCanvasBytes,
    canvas_ops: canvasError ? null : lastCanvasOps,
    canvas_error: canvasError,
  };
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

/**
 * The canvas error code in `bufJson`, or null when it is a CommandBuffer.
 *
 * Whether a response is an error is decided in one place,
 * `parseCommandBuffer` (it throws with `renderErrorCode`); L2 and L3 both go
 * through it. Only an expected refusal (`isExpectedRenderError`) is returned
 * as a canvas answer. Anything else is rethrown instead of being reported as
 * "canvas could not be measured": a failure without `renderErrorCode`
 * (broken JSON, broken bindings), and an unexpected code such as
 * `parse_error`, which means the bench inputs or bindings are broken.
 */
function probeCanvasError(parseCommandBuffer, bufJson) {
  try {
    parseCommandBuffer(bufJson);
    return null;
  } catch (err) {
    if (isExpectedRenderError(err?.renderErrorCode)) return err.renderErrorCode;
    throw err;
  }
}

/**
 * Whether a canvas render error `code` is an expected refusal.
 *
 * Only `canvas_capacity` is: oversized fixtures exceed the canvas size limit.
 * Any other code means the bench inputs or bindings are broken, and both
 * layers fail the run on it. The layers differ only in what they do with the
 * expected refusal:
 *
 * - L2 feeds the relative gate. It records the code in `canvas_error` and
 *   leaves the canvas timing empty, so the gate fails closed
 *   (`benchL2Fixture`).
 * - L3 measures DOM cost for the fixtures canvas can draw, so it skips the
 *   fixture and records it in `l3_skipped`.
 */
export function isExpectedRenderError(code) {
  return code === 'canvas_capacity';
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
  const { replayCommands, parseCommandBuffer } = await importReplayModule();

  const results = [];
  for (const backend of ['svg', 'canvas']) {
    for (const name of FIXTURES) {
      const path = join(root, 'crates/koyori-arc-core/benches/fixtures', `${name}.json`);
      const fx = JSON.parse(readFileSync(path, 'utf8'));
      const tasksJson = JSON.stringify(fx.tasks);
      const depsJson = JSON.stringify(fx.deps);

      // Probe once: capacity-exceeded fixtures return an error payload, not
      // a CommandBuffer. Skip them instead of crashing mid-bench.
      if (backend === 'canvas') {
        try {
          parseCommandBuffer(render_canvas_commands(tasksJson, depsJson, fx.today));
        } catch (err) {
          // Skip only capacity refusals: those are expected for oversized
          // fixtures. Any other render error (input_limit / parse_error /
          // serialize_error) means the bench inputs or bindings are broken --
          // fail the run instead of silently thinning the measurement.
          if (isExpectedRenderError(err.renderErrorCode)) {
            console.warn(`${name} ${backend}: skipped (${err.renderErrorCode}): ${err.message}`);
            l3Skips.push({ fixture: name, backend, code: err.renderErrorCode, message: err.message });
            continue;
          }
          // Name the fixture and backend, as the Playwright path below does;
          // the run stops here before any row names it. The same error is
          // rethrown, so `renderErrorCode` and the error type stay as they were.
          if (err instanceof Error) {
            err.message = err.renderErrorCode
              ? `${name} ${backend}: render failed (${err.renderErrorCode}): ${err.message}`
              : `${name} ${backend}: ${err.message}`;
          }
          throw err;
        }
      }

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

        const probe = await page.evaluate(() => window.__runBench());
        if (probe.renderError) {
          // Same rule as the node path: only capacity refusals are expected.
          if (!isExpectedRenderError(probe.renderError.code)) {
            throw new Error(
              `${name} ${backend}: render failed (${probe.renderError.code}): ${probe.renderError.message}`,
            );
          }
          console.warn(
            `${name} ${backend}: skipped (${probe.renderError.code}): ${probe.renderError.message}`,
          );
          l3Skips.push({ fixture: name, backend, ...probe.renderError });
          continue;
        }

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
      if (l2.canvas_l2_p50_ms == null) continue;

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
  const measuredCanvasL2 = l2.filter((r) => r.canvas_l2_p50_ms != null);
  const maxCanvasL2 = measuredCanvasL2.length
    ? Math.max(...measuredCanvasL2.map((r) => r.canvas_l2_p50_ms))
    : null;

  // Relative L2 gate (cmd_265): fail-closed when gate metrics are missing/invalid.
  const gateEval = evaluateGateFromL2Rows(l2, {
    fixture: GATE_FIXTURE,
    tolerance: L2_TOLERANCE,
  });
  const gateCanvasL2 = gateEval.canvasL2;
  const gateSvgL2 = gateEval.svgL2;
  const l2GatePass = gateEval.pass;

  const merged = computeMergedTotals(FIXTURES, l2, l3);

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
    l3_skipped: l3Skips,
    max_canvas_l2_p50_ms: maxCanvasL2 != null ? round(maxCanvasL2) : null,
    l2_canvas_gate: {
      fixture: GATE_FIXTURE,
      tolerance: L2_TOLERANCE,
      canvas_l2_p50_ms: gateCanvasL2,
      svg_l2_p50_ms: gateSvgL2,
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
  if (l3Skips.length) {
    console.warn(`L3 skipped ${l3Skips.length} fixture/backend pair(s) (see l3_skipped in the payload):`);
    for (const s of l3Skips) console.warn(`  ${s.fixture} ${s.backend}: ${s.code}`);
  }
  console.log('Crossovers:', JSON.stringify(crossovers, null, 2));
  console.log(
    `L2 canvas gate (canvas ≤ svg ×${L2_TOLERANCE} @ ${GATE_FIXTURE}): ` +
      `canvas ${gateCanvasL2 != null ? round(gateCanvasL2) : 'n/a'}ms vs ` +
      `svg ${gateSvgL2 != null ? round(gateSvgL2) : 'n/a'}ms → ${l2GatePass ? 'PASS' : 'FAIL'}`,
  );
}

// Guarded so the module can be smoke-imported by
// canvas-crossover-gate-integration.test.mjs without running the bench.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((err) => {
    console.error(err);
    process.exit(1);
  });
}
