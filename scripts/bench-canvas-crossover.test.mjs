import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { benchL2Fixture, isExpectedRenderError } from './bench-canvas-crossover.mjs';
import { parseCommandBuffer } from './bench-canvas-replay.js';
import { evaluateGateFromL2Rows } from './canvas-crossover-gate-lib.mjs';

const FIXTURE = { tasks: [{ id: 't1' }], deps: [], today: '2026-01-01' };
const OPTS = { warmup: 1, iters: 2 };
const CAPACITY_ERROR = JSON.stringify({
  code: 'canvas_capacity',
  error: 'canvas row count exceeds limit (407 rows / 16384px)',
});
const BUFFER = JSON.stringify({
  viewport_width: 10,
  viewport_height: 5,
  ops: ['GroupEnd'],
  palette: { colors: [] },
});

/** Render stubs that count calls, plus a spy around the real parser. */
function stubs(canvasPayload, parse = parseCommandBuffer) {
  const calls = { svg: 0, canvas: 0, parse: 0 };
  return {
    calls,
    deps: {
      render_svg: () => {
        calls.svg += 1;
        return '<svg></svg>';
      },
      render_canvas_commands: () => {
        calls.canvas += 1;
        return canvasPayload;
      },
      parseCommandBuffer: (json) => {
        calls.parse += 1;
        return parse(json);
      },
    },
  };
}

describe('benchL2Fixture', () => {
  it('does not time a {code, error} response and fails the gate', () => {
    const { calls, deps } = stubs(CAPACITY_ERROR);
    const row = benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS);

    assert.equal(calls.parse, 1, 'the error check goes through parseCommandBuffer');
    assert.equal(calls.canvas, 1, 'only the probe calls render_canvas_commands');
    assert.equal(row.canvas_error, 'canvas_capacity');
    assert.equal(row.canvas_l2_p50_ms, null);
    assert.equal(row.canvas_ops, null);

    const gate = evaluateGateFromL2Rows([row], { fixture: '1000_sparse' });
    assert.equal(gate.pass, false, 'a canvas error is fail-closed at the gate');
  });

  it('times a real CommandBuffer', () => {
    const { calls, deps } = stubs(BUFFER);
    const row = benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS);

    assert.ok(calls.parse >= 1, 'the probe goes through parseCommandBuffer');
    assert.equal(calls.canvas, 1 + OPTS.warmup + OPTS.iters);
    assert.equal(row.canvas_error, null);
    assert.equal(row.canvas_ops, 1);
    assert.equal(typeof row.canvas_l2_p50_ms, 'number');
  });

  it('rethrows a failure that carries no renderErrorCode', () => {
    const unexpected = new Error('bindings are broken');
    const { deps } = stubs(BUFFER, () => {
      throw unexpected;
    });
    assert.throws(() => benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS), (err) => err === unexpected);
  });

  it('still throws on a broken JSON payload instead of reporting canvas as unmeasurable', () => {
    const { deps } = stubs('{not json');
    assert.throws(() => benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS), SyntaxError);
  });

  it('throws on a render error code it cannot expect, at the probe', () => {
    for (const code of ['parse_error', 'input_limit', 'serialize_error', 'unknown']) {
      const { calls, deps } = stubs(JSON.stringify({ code, error: `${code} happened` }));
      assert.throws(
        () => benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS),
        (err) => err.renderErrorCode === code,
        code,
      );
      assert.equal(calls.canvas, 1, `${code}: stops at the probe`);
    }
  });

  it('does not throw on canvas_capacity', () => {
    const { deps } = stubs(CAPACITY_ERROR);
    assert.doesNotThrow(() => benchL2Fixture('1000_sparse', FIXTURE, deps, OPTS));
  });
});

describe('isExpectedRenderError', () => {
  it('expects only canvas_capacity', () => {
    assert.equal(isExpectedRenderError('canvas_capacity'), true);
    for (const code of ['input_limit', 'parse_error', 'serialize_error', 'unknown']) {
      assert.equal(isExpectedRenderError(code), false, code);
    }
  });
});
