#!/usr/bin/env bash
# Run koyori-arc 3-layer render pipeline benchmarks and generate a markdown report.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
mkdir -p benches/results

DOM_THROTTLE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dom-throttle)
      DOM_THROTTLE="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      echo "Usage: $0 [--dom-throttle N]" >&2
      exit 1
      ;;
  esac
done

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

echo "==> Layer 3 native (virtualize ON)"
node scripts/bench-dom.mjs --virtualize --out layer3-dom-native.json
cp benches/results/layer3-dom-native.json benches/results/layer3-dom.json

if [[ -n "$DOM_THROTTLE" ]]; then
  echo "==> Layer 3 throttled (${DOM_THROTTLE}x CPU, virtualize ON)"
  node scripts/bench-dom.mjs --virtualize --dom-throttle "$DOM_THROTTLE" \
    --out "layer3-dom-throttle-${DOM_THROTTLE}x.json"
fi

echo "==> Check §6.6.2 gates"
GATE_ARGS=()
if [[ -n "$DOM_THROTTLE" ]]; then
  GATE_ARGS=(--dom-throttle "$DOM_THROTTLE")
fi
node scripts/check-bench-gates.mjs "${GATE_ARGS[@]}"

echo "==> Generate report"
node scripts/generate-bench-report.mjs

echo "Done. See benches/results/render-pipeline-3layer-report.md"
