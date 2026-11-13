/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { chartHeightForTaskCount } from './canvasFallback';
import { EMPTY_SVG_MARKUP, RUST_LAYOUT } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg>chart</svg>');
const renderCanvasMock = vi.fn((..._args: unknown[]) => '');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: () => undefined,
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-05' },
];

describe('what reaches the Wasm entry points', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockClear();
  });

  it('is three arguments, and the third is absent, when no option was chosen', async () => {
    mount(GanttChart, { props: { tasks, deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(call).toEqual([JSON.stringify(tasks), '[]', undefined]);
    }
  });

  it('is the same three arguments on the Canvas path', async () => {
    mount(GanttChart, { props: { tasks, backend: 'canvas', deviceTier: 'high' } });
    await flushPromises();

    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of renderCanvasMock.mock.calls) {
      expect(call).toEqual([JSON.stringify(tasks), '[]', undefined]);
    }
  });

  it('carries `today` and the offset as keys of one options object', async () => {
    mount(GanttChart, {
      props: { tasks, today: '2026-06-03', tzOffsetMinutes: 540, deviceTier: 'high' },
    });
    await flushPromises();

    for (const call of renderSvgMock.mock.calls) {
      expect(call).toHaveLength(3);
      expect(JSON.parse(call[2] as string)).toEqual({
        today: '2026-06-03',
        tz_offset_minutes: 540,
      });
    }
  });

  it('writes the same options string it wrote before `legend` existed, when `legend` is left out', async () => {
    mount(GanttChart, {
      props: { tasks, today: '2026-06-03', tzOffsetMinutes: 540, deviceTier: 'high' },
    });
    await flushPromises();

    for (const call of renderSvgMock.mock.calls) {
      expect(call[2]).toBe('{"today":"2026-06-03","tz_offset_minutes":540}');
    }
  });

  it('adds `legend` to the options of the SVG path when it is hidden', async () => {
    mount(GanttChart, { props: { tasks, legend: 'hidden', deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(JSON.parse(call[2] as string)).toEqual({ legend: 'hidden' });
    }
  });

  it('adds `legend` to the options of the Canvas path when it is hidden', async () => {
    mount(GanttChart, {
      props: { tasks, backend: 'canvas', legend: 'hidden', deviceTier: 'high' },
    });
    await flushPromises();

    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of renderCanvasMock.mock.calls) {
      expect(JSON.parse(call[2] as string)).toEqual({ legend: 'hidden' });
    }
  });

  it('adds `row_labels` and `percent_labels` to the options of both paths when they are hidden', async () => {
    mount(GanttChart, {
      props: { tasks, rowLabels: 'hidden', percentLabels: 'hidden', deviceTier: 'high' },
    });
    mount(GanttChart, {
      props: {
        tasks,
        backend: 'canvas',
        rowLabels: 'hidden',
        percentLabels: 'hidden',
        deviceTier: 'high',
      },
    });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of [...renderSvgMock.mock.calls, ...renderCanvasMock.mock.calls]) {
      expect(JSON.parse(call[2] as string)).toEqual({
        row_labels: 'hidden',
        percent_labels: 'hidden',
      });
    }
  });

  it('passes the rows as a fourth argument only when the caller lays them out', async () => {
    const rows = [{ kind: 'group' as const }, { kind: 'task' as const, task_id: 'a', height: 64 }];
    mount(GanttChart, {
      props: { tasks, rows, rowHeight: 32, barHeight: 12, barAlign: 'bottom', deviceTier: 'high' },
    });
    mount(GanttChart, {
      props: { tasks, rows, backend: 'canvas', deviceTier: 'high' },
    });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(call).toHaveLength(4);
      expect(JSON.parse(call[3] as string)).toEqual(rows);
      expect(JSON.parse(call[2] as string)).toEqual({
        row_height_px: 32,
        bar_height_px: 12,
        bar_align: 'bottom',
      });
    }
    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of renderCanvasMock.mock.calls) {
      expect(call).toHaveLength(4);
      expect(JSON.parse(call[3] as string)).toEqual(rows);
    }
  });

  it('reserves the height of the rows as laid out', async () => {
    const rows = [{ kind: 'group' as const }, { kind: 'task' as const, task_id: 'a', height: 64 }];
    const w = mount(GanttChart, {
      props: { tasks, rows, rowHeight: 32, headerHeight: 48, deviceTier: 'high' },
    });
    await flushPromises();
    const expected = 48 + 32 + 64 + RUST_LAYOUT.LEGEND_H + RUST_LAYOUT.CHART_BOTTOM_PADDING_PX;
    const height = (w.get('.koyori-gantt-inner').element as HTMLElement).style.height;
    expect(Number.parseInt(height, 10)).toBe(expected);
  });

  it('reserves the shorter height for a hidden legend, and the old one otherwise', async () => {
    const hidden = mount(GanttChart, { props: { tasks, legend: 'hidden', deviceTier: 'high' } });
    const shown = mount(GanttChart, { props: { tasks, deviceTier: 'high' } });
    await flushPromises();

    const heightOf = (w: typeof hidden) =>
      Number.parseInt((w.get('.koyori-gantt-inner').element as HTMLElement).style.height, 10);
    expect(heightOf(shown)).toBe(chartHeightForTaskCount(tasks.length));
    expect(heightOf(hidden)).toBe(chartHeightForTaskCount(tasks.length, 'hidden'));
    expect(heightOf(shown) - heightOf(hidden)).toBe(RUST_LAYOUT.LEGEND_H);
  });
});
