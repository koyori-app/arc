#!/usr/bin/env bash
# Run koyori-arc 3-layer render pipeline benchmarks and generate a markdown report.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p benches/results

echo "==> Export fixtures"
cargo run --release --example export_fixtures -p koyori-arc-core

echo "==> Build wasm pkg (web target)"
wasm-pack build crates/koyori-arc-core --target web --release

echo "==> Layer 1+2 (criterion, native Rust)"
cargo bench -p koyori-arc-core --bench render_pipeline 2>&1 | tee benches/results/criterion.log

echo "==> Layer 2 (Wasm/JS boundary, Node)"
node scripts/bench-wasm-boundary.mjs

echo "==> Layer 3 (Browser DOM, Playwright)"
if ! node -e "require.resolve('playwright')" 2>/dev/null; then
  echo "Installing playwright (one-time)..."
  npm install --no-save playwright@1.52.0
  npx playwright install chromium
fi
node scripts/bench-dom.mjs

echo "==> Generate report"
node scripts/generate-bench-report.mjs

echo "Done. See benches/results/render-pipeline-3layer-report.md"
