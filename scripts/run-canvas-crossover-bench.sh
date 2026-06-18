#!/usr/bin/env bash
# Phase 2 canvas-vs-SVG crossover micro-bench (cmd_284).
# Reuses bench-3layer build steps; manual / workflow_dispatch only.
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

echo "==> Export fixtures (includes micro N series)"
cargo run --release --example export_fixtures -p koyori-arc-core

echo "==> Build wasm pkg (web target)"
wasm-pack build crates/koyori-arc-core --target web --release

echo "==> Canvas vs SVG crossover bench"
BENCH_ARGS=()
if [[ -n "$DOM_THROTTLE" ]]; then
  BENCH_ARGS=(--dom-throttle "$DOM_THROTTLE")
fi

if ! node -e "require.resolve('playwright')" 2>/dev/null; then
  echo "Installing playwright (one-time, optional for L3)..."
  npm install --no-save playwright@1.52.0 || true
  npx playwright install chromium || echo "Playwright browser install skipped — Node fallback will be used."
fi

node scripts/bench-canvas-crossover.mjs "${BENCH_ARGS[@]}"

echo "==> Check crossover gates"
node scripts/check-canvas-crossover-gates.mjs

echo "==> Generate report"
node scripts/generate-canvas-crossover-report.mjs

echo "Done. See benches/results/canvas-vs-svg-crossover-report.md"
