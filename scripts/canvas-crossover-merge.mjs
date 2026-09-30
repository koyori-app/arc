/**
 * Merged L2+L3 totals for the canvas-vs-SVG crossover bench.
 * Kept free of side effects so it can be tested without running the bench.
 */

function round(n) {
  return Math.round(n * 100) / 100;
}

// A total is only a number when both of its layers were measured. Filling a
// missing L3 with 0 would print an L2-only time as if it were a full total.
function total(l2Ms, l3Ms) {
  return l2Ms != null && l3Ms != null ? round(l2Ms + l3Ms) : null;
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
      svg_total_p50_ms: total(l2Row?.svg_l2_p50_ms, l3Svg?.l3_p50_ms),
      canvas_total_p50_ms: total(l2Row?.canvas_l2_p50_ms, l3Canvas?.l3_p50_ms),
      svg_l2_p50_ms: l2Row?.svg_l2_p50_ms ?? null,
      canvas_l2_p50_ms: l2Row?.canvas_l2_p50_ms ?? null,
      svg_l3_p50_ms: l3Svg?.l3_p50_ms ?? null,
      canvas_l3_p50_ms: l3Canvas?.l3_p50_ms ?? null,
    };
  });
}
