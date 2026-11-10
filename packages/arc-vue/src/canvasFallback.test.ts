import { describe, expect, it } from 'vitest';
import {
  chartHeightForRows,
  chartHeightForTaskCount,
  resolveCanvasFailure,
  skeletonLabelWidthPx,
} from './canvasFallback';
import { EMPTY_SVG_MARKUP, RUST_ERROR_CODES, RUST_LAYOUT } from './wasmContract';

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
        EMPTY_SVG_MARKUP,
      )).toEqual({ mode: 'svg', svg });
      expect(svg).toContain(`height="${height}"`);
    },
  );

  it('takes the legend band off the reserved height when the legend is hidden', () => {
    expect(chartHeightForTaskCount(4, 'hidden'))
      .toBe(chartHeightForTaskCount(4) - RUST_LAYOUT.LEGEND_H);
    expect(chartHeightForTaskCount(4, 'shown')).toBe(chartHeightForTaskCount(4));
    expect(chartHeightForTaskCount(0, 'hidden')).toBe(0);
  });

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
      EMPTY_SVG_MARKUP,
    )).toEqual({ mode: 'svg', svg });
  });

  it('shows an error when the shared task limit rejects SVG too', () => {
    expect(resolveCanvasFailure(
      { message: 'task count exceeds limit (10000)', code: RUST_ERROR_CODES.CODE_INPUT_LIMIT },
      EMPTY_SVG_MARKUP,
      EMPTY_SVG_MARKUP,
    )).toEqual({ mode: 'error', message: 'task count exceeds limit (10000)' });
  });

  it('shows an error when SVG fallback unexpectedly renders empty', () => {
    expect(resolveCanvasFailure(
      capacityFailure('canvas area exceeds limit (33554432 pixels)'),
      EMPTY_SVG_MARKUP,
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
      EMPTY_SVG_MARKUP,
    )).toEqual({ mode: 'svg', svg });
  });

  it('ignores the wording and reads the code: capacity-sounding text without a code errors', () => {
    const svg = '<svg width="800" height="16400"><g data-task-id="task-0" /></svg>';
    expect(resolveCanvasFailure(
      { message: 'canvas row count exceeds limit (407 rows / 16384px)' },
      svg,
      EMPTY_SVG_MARKUP,
    )).toEqual({
      mode: 'error',
      message: 'canvas row count exceeds limit (407 rows / 16384px)',
    });
  });
});

describe('chartHeightForRows', () => {
  it('is chartHeightForTaskCount when the crate lays out the rows', () => {
    for (const n of [0, 1, 4, 408]) {
      expect(chartHeightForRows(undefined, n)).toBe(chartHeightForTaskCount(n));
      expect(chartHeightForRows(undefined, n, 'hidden')).toBe(chartHeightForTaskCount(n, 'hidden'));
    }
  });

  it('adds every row at its own height under the header the caller set', () => {
    const rows = [
      { kind: 'group' as const },
      { kind: 'task' as const, task_id: 'a' },
      { kind: 'task' as const, task_id: 'b', height: 64 },
    ];
    expect(chartHeightForRows(rows, 2, 'hidden', { rowHeight: 32, headerHeight: 48 })).toBe(
      48 + 32 + 32 + 64 + RUST_LAYOUT.CHART_BOTTOM_PADDING_PX,
    );
    expect(chartHeightForRows(rows, 0)).toBe(0);
  });
});

describe('skeletonLabelWidthPx', () => {
  it('is the title column with titles, and the chart\'s left margin without them', () => {
    expect(skeletonLabelWidthPx()).toBe(RUST_LAYOUT.LABEL_W);
    expect(skeletonLabelWidthPx('shown')).toBe(RUST_LAYOUT.LABEL_W);
    // 図は題の列を消すと、左に最初の日の菱形のための余白を残す
    expect(skeletonLabelWidthPx('hidden')).toBe(RUST_LAYOUT.HIDDEN_LABELS_LEFT_PX);
    expect(RUST_LAYOUT.HIDDEN_LABELS_LEFT_PX).toBe(RUST_LAYOUT.BAR_H / 2);
  });

  it('follows the bar height the caller set, as the chart does', () => {
    // 棒の高さ 30 の図は、左に 15 の余白を残す（core の label_width と同じ）
    expect(skeletonLabelWidthPx('hidden', 30)).toBe(15);
    expect(skeletonLabelWidthPx('shown', 30)).toBe(RUST_LAYOUT.LABEL_W);
  });
});
