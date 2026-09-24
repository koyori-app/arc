import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import { parseCommandBuffer } from './bench-canvas-replay.js';

describe('parseCommandBuffer', () => {
  it('returns the parsed buffer for a normal command payload', () => {
    const buf = parseCommandBuffer(
      JSON.stringify({ viewport_width: 10, viewport_height: 5, ops: [], palette: { colors: [] } }),
    );
    assert.equal(buf.viewport_width, 10);
    assert.deepEqual(buf.ops, []);
  });

  it('throws a readable error for an {code, error} payload instead of passing it through', () => {
    assert.throws(
      () =>
        parseCommandBuffer(
          JSON.stringify({
            code: 'canvas_capacity',
            error: 'canvas row count exceeds limit (407 rows / 16384px)',
          }),
        ),
      (err) =>
        err.renderErrorCode === 'canvas_capacity' &&
        err.message.includes('canvas_capacity') &&
        err.message.includes('canvas row count exceeds limit'),
    );
  });

  it('does not mistake a buffer carrying an ops array for an error', () => {
    const buf = parseCommandBuffer(
      JSON.stringify({ viewport_width: 1, viewport_height: 1, ops: ['GroupEnd'], palette: { colors: [] } }),
    );
    assert.equal(buf.ops.length, 1);
  });
});
