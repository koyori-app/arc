import { describe, expect, it } from 'vitest';
import { chartHeightForTaskCount, resolveCanvasFailure } from './canvasFallback';
import { EMPTY_SVG_MARKUP, RUST_ERROR_CODES } from './wasmContract';

const capacityFailure = (message: string) => ({
  message,
  code: RUST_ERROR_CODES.CODE_CANVAS_CAPACITY,
});

describe('resolveCanvasFailure', () => {
  it.each([408, 1_000, 10_000])(
    'uses a non-empty SVG fallback when %i tasks exceed Canvas capacity',
    (taskCount) => {
      const height = chartHeightForTaskCount(taskCount);
      const svg = `<svg width="800" height="${height}"><g data-task-id="task-0" /></svg>`;
      expect(resolveCanvasFailure(
        capacityFailure('canvas row count exceeds limit (407 rows / 16384px)'),
        svg,
      )).toEqual({ mode: 'svg', svg });
      expect(svg).toContain(`height="${height}"`);
    },
  );

  it('resets the scroll domain for empty content', () => {
    expect(chartHeightForTaskCount(0)).toBe(0);
    expect(chartHeightForTaskCount(408)).toBe(16_400);
  });

  it('keeps a real chart whose markup happens to contain a zero-width bar', () => {
    // A 0% progress bar serializes as width="0", so substring sniffing for
    // width="0" throws away a perfectly good fallback chart.
    const svg = '<svg width="800" height="16400"><rect class="bar-bg" width="120" height="20"/>'
      + '<rect class="bar-progress bar-tier-low" width="0" height="20"/></svg>';
    expect(resolveCanvasFailure(
      capacityFailure('canvas row count exceeds limit (407 rows / 16384px)'),
      svg,
    )).toEqual({ mode: 'svg', svg });
  });

  it('shows an error when the shared task limit rejects SVG too', () => {
    expect(resolveCanvasFailure(
      { message: 'task count exceeds limit (10000)', code: RUST_ERROR_CODES.CODE_INPUT_LIMIT },
      EMPTY_SVG_MARKUP,
    )).toEqual({ mode: 'error', message: 'task count exceeds limit (10000)' });
  });

  it('shows an error when SVG fallback unexpectedly renders empty', () => {
    expect(resolveCanvasFailure(
      capacityFailure('canvas area exceeds limit (33554432 pixels)'),
      EMPTY_SVG_MARKUP,
    )).toEqual({
      mode: 'error',
      message: 'canvas area exceeds limit (33554432 pixels)',
    });
  });

  it('ignores the wording and reads the code: a reworded capacity error still falls back', () => {
    const svg = '<svg width="800" height="16400"><g data-task-id="task-0" /></svg>';
    expect(resolveCanvasFailure(
      capacityFailure('completely reworded on the Rust side'),
      svg,
    )).toEqual({ mode: 'svg', svg });
  });

  it('ignores the wording and reads the code: capacity-sounding text without a code errors', () => {
    const svg = '<svg width="800" height="16400"><g data-task-id="task-0" /></svg>';
    expect(resolveCanvasFailure(
      { message: 'canvas row count exceeds limit (407 rows / 16384px)' },
      svg,
    )).toEqual({
      mode: 'error',
      message: 'canvas row count exceeds limit (407 rows / 16384px)',
    });
  });
});
