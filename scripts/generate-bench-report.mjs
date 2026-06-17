#!/usr/bin/env node
/**
 * Merge criterion + wasm + DOM bench outputs into a single markdown report.
 */
import { readFileSync, writeFileSync, mkdirSync, readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, '..');
const resultsDir = join(root, 'benches/results');

function readJson(name) {
  const raw = JSON.parse(readFileSync(join(resultsDir, name), 'utf8'));
  return Array.isArray(raw) ? raw : raw.results;
}

function readBenchMeta(name) {
  try {
    return JSON.parse(readFileSync(join(resultsDir, name), 'utf8'));
  } catch {
    return null;
  }
}

function parseCriterionEstimates() {
  const logPath = join(resultsDir, 'criterion.log');
  try {
    const log = readFileSync(logPath, 'utf8');
    const rows = [];
    const re =
      /(layer\d+_\w+)\/\w+\/(\d+)\/(\w+)\s*\n\s+time:\s+\[[^\]]*?([\d.]+)\s*(µs|ms)[^\]]*?\s+([\d.]+)\s*(µs|ms)[^\]]*?\s+([\d.]+)\s*(µs|ms)/g;
    let m;
    while ((m = re.exec(log)) !== null) {
      const unit = m[5] === 'µs' ? 0.001 : 1;
      rows.push({
        group: m[1],
        fixture: `${m[2]}_${m[3]}`,
        median_ms: parseFloat(m[6]) * unit,
      });
    }
    if (rows.length > 0) return rows;
  } catch {
    // fall through to merged.json
  }
  try {
    const merged = JSON.parse(readFileSync(join(resultsDir, 'merged.json'), 'utf8'));
    const rows = [];
    for (const r of merged) {
      rows.push({ group: 'layer1_rust_render', fixture: r.fixture, median_ms: r.l1_rust_ms });
      rows.push({
        group: 'layer2_render_svg_native',
        fixture: r.fixture,
        median_ms: r.l2_native_render_svg_ms,
      });
    }
    return rows;
  } catch {
    return [];
  }
}

function cpuInfo() {
  try {
    return execSync("lscpu | grep 'Model name' | sed 's/Model name:[[:space:]]*//'", { encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

function chromiumVersion() {
  try {
    return execSync('npx playwright --version 2>/dev/null || echo unknown', { encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

function rustcVersion() {
  return execSync('rustc --version', { encoding: 'utf8' }).trim();
}

function mergeRows(criterion, layer2, layer3) {
  const fixtures = [
    '100_sparse', '100_dense', '500_sparse', '500_dense',
    '2000_sparse', '2000_dense', '5000_sparse', '5000_dense',
  ];

  return fixtures.map((fx) => {
    const l1 = criterion.find((r) => r.group === 'layer1_rust_render' && r.fixture === fx);
    const l2n = criterion.find((r) => r.group === 'layer2_render_svg_native' && r.fixture === fx);
    const l2w = layer2.find((r) => r.fixture === fx);
    const l3 = layer3.find((r) => r.fixture === fx);

    const rustMs = l1?.median_ms ?? 0;
    const wasmNativeMs = l2n?.median_ms ?? 0;
    const wasmNodeMs = l2w?.wasm_boundary_ms_median ?? 0;
    const domMs = l3?.dom_insert_ms_median ?? 0;
    const total = rustMs + wasmNodeMs + domMs;

    const layers = [
      { name: 'L1 Rust render', ms: rustMs },
      { name: 'L2 Wasm boundary (Node)', ms: wasmNodeMs },
      { name: 'L3 DOM insert (browser)', ms: domMs },
    ];
    const wall = layers.reduce((a, b) => (b.ms > a.ms ? b : a), layers[0]);

    return {
      fixture: fx,
      tasks: l2w?.tasks ?? 0,
      deps: l2w?.deps ?? 0,
      l1_rust_ms: round(rustMs),
      l2_native_render_svg_ms: round(wasmNativeMs),
      l2_wasm_node_ms: round(wasmNodeMs),
      l3_dom_ms: round(domMs),
      svg_bytes: l2w?.svg_bytes ?? 0,
      svg_elements: l2w?.svg_elements ?? 0,
      total_estimated_ms: round(total),
      wall_layer: wall.name,
      wall_ms: round(wall.ms),
      wall_pct: total > 0 ? round((wall.ms / total) * 100) : 0,
    };
  });
}

function round(n) {
  return Math.round(n * 100) / 100;
}

function overallConclusion(rows) {
  const maxRow = rows.reduce((a, b) => (b.total_estimated_ms > a.total_estimated_ms ? b : a), rows[0]);
  const counts = {};
  for (const r of rows) {
    counts[r.wall_layer] = (counts[r.wall_layer] ?? 0) + 1;
  }
  const dominant = Object.entries(counts).sort((a, b) => b[1] - a[1])[0][0];
  return { maxRow, dominant };
}

function main() {
  mkdirSync(resultsDir, { recursive: true });
  const criterion = parseCriterionEstimates();
  const layer2 = readJson('layer2-wasm-boundary.json');
  const layer3NativeMeta = readBenchMeta('layer3-dom-native.json');
  const layer3Native = layer3NativeMeta?.results ?? readJson('layer3-dom.json');
  const layer3ThrottleMeta = readBenchMeta('layer3-dom-throttle-4x.json');
  const layer3Throttle = layer3ThrottleMeta?.results ?? null;
  const gatesMeta = readBenchMeta('bench-gates.json');

  const layer3Raw = layer3NativeMeta ?? { results: layer3Native, engine: 'unknown' };
  const layer3 = layer3Native;
  const domEngine = layer3Raw.engine ?? layer3[0]?.dom_engine ?? 'unknown';
  const rows = mergeRows(criterion, layer2, layer3);
  const { maxRow, dominant } = overallConclusion(rows);

  const md = [];
  md.push('# koyori-arc 描画パイプライン 3層ベンチマーク');
  md.push('');
  md.push('## 計測条件');
  md.push('');
  md.push(`| 項目 | 値 |`);
  md.push(`|------|-----|`);
  md.push(`| CPU | ${cpuInfo()} |`);
  md.push(`| Rust | ${rustcVersion()} |`);
  md.push(`| ビルド | \`cargo bench --release\`, \`wasm-pack build --release\` |`);
  md.push(`| ブラウザ/DOM | ${domEngine} |`);
  md.push(`| L3 仮想化 | ON（viewport scroll_y=0, client_height=600） |`);
  if (layer3ThrottleMeta) {
    md.push(`| CPU スロットル | native + ${layer3ThrottleMeta.dom_throttle}× CDP |`);
  }
  md.push(`| 日付 | ${new Date().toISOString().slice(0, 10)} |`);
  md.push('');
  md.push('### 3層の定義');
  md.push('');
  md.push('1. **L1 Rust 幾何計算**: `render()` — 行割当・依存矢印・イナズマ線・SVG文字列生成（criterion, release）');
  md.push('2. **L2 Wasm/JS 境界**: Node 上の `render_svg()` Wasm 呼び出し〜JS文字列受領 + SVGバイト長・要素数');
  md.push('3. **L3 ブラウザ SVG-DOM**: `innerHTML` 挿入〜2×`requestAnimationFrame` 後のレイアウト/描画完了');
  md.push('');
  md.push('### データ');
  md.push('');
  md.push('- タスク数: 100 / 500 / 2000 / 5000');
  md.push('- 依存密度: **sparse**（線形チェーン）/ **dense**（各タスク最大5先行＋クロスリンク）');
  md.push('');
  md.push('## 結果（規模 × 密度）');
  md.push('');
  md.push('| fixture | tasks | deps | L1 Rust (ms) | L2 Wasm Node (ms) | L3 DOM (ms) | SVG bytes | SVG elems | 合計 (ms) | 壁 |');
  md.push('|---------|------:|-----:|-------------:|------------------:|------------:|----------:|----------:|----------:|-----|');
  for (const r of rows) {
    md.push(
      `| ${r.fixture} | ${r.tasks} | ${r.deps} | ${r.l1_rust_ms} | ${r.l2_wasm_node_ms} | ${r.l3_dom_ms} | ${r.svg_bytes} | ${r.svg_elements} | ${r.total_estimated_ms} | **${r.wall_layer}** (${r.wall_pct}%) |`
    );
  }
  md.push('');

  if (layer3Native && layer3Throttle) {
    md.push('## Phase 1 仮想化 L3 — native / 4× 併記（§6.6.2）');
    md.push('');
    md.push('| fixture | L3 native (ms) | L3 4× throttle (ms) | live elems (native) | DOM_CAP pass |');
    md.push('|---------|---------------:|--------------------:|--------------------:|:------------:|');
    const fixtures = [
      '100_sparse', '100_dense', '500_sparse', '500_dense',
      '2000_sparse', '2000_dense', '5000_sparse', '5000_dense',
    ];
    for (const fx of fixtures) {
      const nat = layer3Native.find((r) => r.fixture === fx);
      const thr = layer3Throttle.find((r) => r.fixture === fx);
      const elems = nat?.live_svg_elems ?? nat?.svg_elements ?? '-';
      const capPass = nat?.dom_cap_pass === true ? '✓' : nat?.dom_cap_pass === false ? '✗' : '-';
      md.push(
        `| ${fx} | ${nat?.dom_insert_ms_median ?? '-'} | ${thr?.dom_insert_ms_median ?? '-'} | ${elems} | ${capPass} |`,
      );
    }
    md.push('');
  }

  if (gatesMeta) {
    md.push('## §6.6.2 CI ゲート結果');
    md.push('');
    md.push(`Overall: **${gatesMeta.all_pass ? 'PASS' : 'FAIL'}**`);
    md.push('');
    md.push('| Gate | Fixture | Actual | Condition | Result |');
    md.push('|------|---------|-------:|-----------|--------|');
    for (const g of gatesMeta.gates) {
      const result = g.skipped ? 'SKIP' : g.pass ? 'PASS' : 'FAIL';
      md.push(`| ${g.id} | ${g.fixture} | ${g.actual ?? 'n/a'} | ${g.condition} | ${result} |`);
    }
    md.push('');
  }

  md.push('## 壁の位置 — 結論');
  md.push('');
  md.push(`- **支配的な壁（全8ケース中最多）**: ${dominant}`);
  md.push(`- **最大規模ケース（${maxRow.fixture}）**: 合計 ~${maxRow.total_estimated_ms} ms — 壁は **${maxRow.wall_layer}** (${maxRow.wall_pct}%)`);
  md.push('');
  md.push('### 解釈');
  md.push('');
  if (dominant.includes('L3')) {
    md.push('- SVG文字列が大きくなるほど **ブラウザ DOM 挿入・レイアウト** が支配的。Rust/Wasm 最適化だけでは頭打ち。');
    md.push('- 対策候補: 仮想化（表示行のみ描画）、Canvas/SVG チャンク分割、インクリメンタル更新。');
  } else if (dominant.includes('L1')) {
    md.push('- **Rust 幾何計算**が支配的。依存矢印・イナズマ線のアルゴリズム改善が最優先。');
  } else {
    md.push('- **Wasm/JS 境界**が支配的。JSON 受け渡し削減（TypedArray/共有バッファ）やネイティブバイナリ連携を検討。');
  }
  md.push('');
  md.push('## 再現手順');
  md.push('');
  md.push('```bash');
  md.push('bash scripts/run-3layer-bench.sh --dom-throttle 4');
  md.push('```');
  md.push('');

  const outPath = join(resultsDir, 'render-pipeline-3layer-report.md');
  writeFileSync(outPath, md.join('\n'));
  writeFileSync(join(resultsDir, 'merged.json'), JSON.stringify(rows, null, 2));
  console.log(`Wrote ${outPath}`);
}

main();
