/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

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
});
