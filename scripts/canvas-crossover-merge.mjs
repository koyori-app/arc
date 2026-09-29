/**
 * Merged L2+L3 totals for the canvas-vs-SVG crossover bench.
 * Kept free of side effects so it can be tested without running the bench.
 */

function round(n) {
  return Math.round(n * 100) / 100;
}

/**
 * @param {string[]} fixtures
 * @param {Array<Record<string, any>>} l2Rows
 * @param {Array<Record<string, any>>} l3Rows
 */
export function computeMergedTotals(fixtures, l2Rows, l3Rows) {
  return fixtures.map((fx) => {
    const l2Row = l2Rows.find((r) => r.fixture === fx);
    const l3Svg = l3Rows.find((r) => r.fixture === fx && r.backend === 'svg');
    const l3Canvas = l3Rows.find((r) => r.fixture === fx && r.backend === 'canvas');
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
}
