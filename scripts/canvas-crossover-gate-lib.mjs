/**
 * Shared relative L2 canvas gate evaluation (fail-closed).
 * Used by bench-canvas-crossover.mjs and check-canvas-crossover-gates.mjs.
 */

export const GATE_FIXTURE = '2000_dense';
export const L2_TOLERANCE = 1.15;

/** @param {unknown} value */
export function isValidMs(value) {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

/**
 * @param {{ canvasL2: unknown; svgL2: unknown; tolerance?: number }} params
 * @returns {{ pass: boolean; canvasL2: number | null; svgL2: number | null; reason?: string }}
 */
export function evaluateRelativeL2Gate({ canvasL2, svgL2, tolerance = L2_TOLERANCE }) {
  if (!isValidMs(canvasL2) || !isValidMs(svgL2)) {
    return {
      pass: false,
      canvasL2: isValidMs(canvasL2) ? canvasL2 : null,
      svgL2: isValidMs(svgL2) ? svgL2 : null,
      reason: 'missing or invalid gate metrics',
    };
  }

  const pass = canvasL2 <= svgL2 * tolerance;
  return { pass, canvasL2, svgL2 };
}

/**
 * @param {Array<{ fixture: string; canvas_l2_p50_ms?: unknown; svg_l2_p50_ms?: unknown }> | undefined} l2Rows
 * @param {string} [fixture]
 */
export function findGateRow(l2Rows, fixture = GATE_FIXTURE) {
  return l2Rows?.find((r) => r.fixture === fixture) ?? null;
}

/**
 * @param {Array<{ fixture: string; canvas_l2_p50_ms?: unknown; svg_l2_p50_ms?: unknown }> | undefined} l2Rows
 * @param {{ tolerance?: number; fixture?: string }} [opts]
 */
export function evaluateGateFromL2Rows(l2Rows, opts = {}) {
  const fixture = opts.fixture ?? GATE_FIXTURE;
  const tolerance = opts.tolerance ?? L2_TOLERANCE;
  const gateRow = findGateRow(l2Rows, fixture);
  if (!gateRow) {
    return {
      pass: false,
      fixture,
      tolerance,
      canvasL2: null,
      svgL2: null,
      reason: 'missing gate fixture row',
    };
  }

  const result = evaluateRelativeL2Gate({
    canvasL2: gateRow.canvas_l2_p50_ms,
    svgL2: gateRow.svg_l2_p50_ms,
    tolerance,
  });

  return {
    ...result,
    fixture,
    tolerance,
    gateRow,
  };
}

/** @param {{ pass: boolean; canvasL2: number | null; svgL2: number | null }} gate */
export function formatGateActual(gate) {
  return `canvas ${gate.canvasL2 ?? 'n/a'} ms vs svg ${gate.svgL2 ?? 'n/a'} ms`;
}
