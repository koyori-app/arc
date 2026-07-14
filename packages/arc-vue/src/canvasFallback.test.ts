import { describe, expect, it } from 'vitest';
import { chartHeightForTaskCount, resolveCanvasFailure } from './canvasFallback';

describe('resolveCanvasFailure', () => {
  it.each([408, 1_000, 10_000])(
    'uses a non-empty SVG fallback when %i tasks exceed Canvas capacity',
    (taskCount) => {
      const height = chartHeightForTaskCount(taskCount);
      const svg = `<svg width="800" height="${height}"><g data-task-id="task-0" /></svg>`;
      expect(resolveCanvasFailure(
        'canvas row count exceeds limit (407 rows / 16384px)',
        svg,
      )).toEqual({ mode: 'svg', svg });
      expect(svg).toContain(`height="${height}"`);
    },
  );

  it('resets the scroll domain for empty content', () => {
    expect(chartHeightForTaskCount(0)).toBe(0);
    expect(chartHeightForTaskCount(408)).toBe(16_400);
  });

  it('shows an error when the shared task limit rejects SVG too', () => {
    expect(resolveCanvasFailure(
      'task count exceeds limit (10000)',
      '<svg width="0" height="0"></svg>',
    )).toEqual({ mode: 'error', message: 'task count exceeds limit (10000)' });
  });

  it('shows an error when SVG fallback unexpectedly renders empty', () => {
    expect(resolveCanvasFailure(
      'canvas area exceeds limit (33554432 pixels)',
      '<svg width="0" height="0"></svg>',
    )).toEqual({
      mode: 'error',
      message: 'canvas area exceeds limit (33554432 pixels)',
    });
  });
});
