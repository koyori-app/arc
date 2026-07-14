import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  evaluateGateFromL2Rows,
  evaluateRelativeL2Gate,
  formatGateActual,
  GATE_FIXTURE,
  isValidMs,
} from './canvas-crossover-gate-lib.mjs';

describe('isValidMs', () => {
  it('accepts finite non-negative numbers', () => {
    assert.equal(isValidMs(0), true);
    assert.equal(isValidMs(12.5), true);
  });

  it('rejects missing, non-finite, and negative values', () => {
    assert.equal(isValidMs(null), false);
    assert.equal(isValidMs(undefined), false);
    assert.equal(isValidMs(NaN), false);
    assert.equal(isValidMs(Infinity), false);
    assert.equal(isValidMs(-1), false);
    assert.equal(isValidMs('10'), false);
  });
});

describe('evaluateRelativeL2Gate', () => {
  it('fails closed when svg baseline is missing', () => {
    const result = evaluateRelativeL2Gate({ canvasL2: 10, svgL2: null });
    assert.equal(result.pass, false);
    assert.match(result.reason ?? '', /invalid/i);
  });

  it('fails closed when canvas metric is missing', () => {
    const result = evaluateRelativeL2Gate({ canvasL2: undefined, svgL2: 10 });
    assert.equal(result.pass, false);
  });

  it('fails on non-finite or negative metrics', () => {
    assert.equal(evaluateRelativeL2Gate({ canvasL2: NaN, svgL2: 10 }).pass, false);
    assert.equal(evaluateRelativeL2Gate({ canvasL2: 10, svgL2: -1 }).pass, false);
  });

  it('passes when canvas is within tolerance of svg', () => {
    const result = evaluateRelativeL2Gate({ canvasL2: 11, svgL2: 10, tolerance: 1.15 });
    assert.equal(result.pass, true);
  });

  it('fails when canvas exceeds svg tolerance', () => {
    const result = evaluateRelativeL2Gate({ canvasL2: 12, svgL2: 10, tolerance: 1.15 });
    assert.equal(result.pass, false);
  });
});

describe('evaluateGateFromL2Rows', () => {
  const validRows = [
    {
      fixture: GATE_FIXTURE,
      canvas_l2_p50_ms: 20,
      svg_l2_p50_ms: 25,
    },
  ];

  it('fails when gate fixture row is missing', () => {
    assert.equal(evaluateGateFromL2Rows([]).pass, false);
    assert.equal(evaluateGateFromL2Rows(undefined).pass, false);
  });

  it('fails when baseline svg is missing on gate row', () => {
    const rows = [{ fixture: GATE_FIXTURE, canvas_l2_p50_ms: 20 }];
    assert.equal(evaluateGateFromL2Rows(rows).pass, false);
  });

  it('does not use max-series fallback for missing canvas metric', () => {
    const rows = [{ fixture: GATE_FIXTURE, svg_l2_p50_ms: 25 }];
    assert.equal(evaluateGateFromL2Rows(rows).pass, false);
  });

  it('passes with valid gate row data', () => {
    assert.equal(evaluateGateFromL2Rows(validRows).pass, true);
  });
});

describe('formatGateActual', () => {
  it('renders n/a for missing metrics', () => {
    assert.equal(
      formatGateActual({ pass: false, canvasL2: null, svgL2: 10 }),
      'canvas n/a ms vs svg 10 ms',
    );
  });
});

describe('report schema guard', () => {
  it('does not reference deprecated bench gate field names', async () => {
    const { readFileSync } = await import('node:fs');
    const { fileURLToPath } = await import('node:url');
    const { dirname, join } = await import('node:path');
    const dir = dirname(fileURLToPath(import.meta.url));
    const source = readFileSync(join(dir, 'generate-canvas-crossover-report.mjs'), 'utf8');
    assert.doesNotMatch(source, /l2_canvas_gate_ms/);
    assert.doesNotMatch(source, /l2_canvas_gate_pass/);
  });
});
