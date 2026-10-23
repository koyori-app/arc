/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg data-fallback="1"></svg>');
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

// Only Rust produces this: `code` marks it a capacity error, the one canvas
// failure that is answered with an SVG (`resolveCanvasFailure`).
const capacityBuffer = JSON.stringify({
  viewport_width: 0,
  viewport_height: 0,
  ops: [],
  palette: { colors: [] },
  error: 'canvas too large',
  code: 'canvas_capacity',
});

describe('the SVG a canvas falls back to', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockReset();
    renderCanvasMock.mockReturnValue(capacityBuffer);
  });

  it('is asked to leave the legend out when the legend is hidden', async () => {
    const wrapper = mount(GanttChart, {
      props: { tasks, backend: 'canvas', legend: 'hidden', deviceTier: 'high' },
    });
    await flushPromises();

    // Positive control: the fallback is really what is on screen, so the calls
    // below are the fallback's and not a dead branch's.
    expect(wrapper.find('.koyori-gantt-svg').exists()).toBe(true);
    expect(wrapper.find('canvas').exists()).toBe(false);

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(JSON.parse(call[2] as string)).toMatchObject({ legend: 'hidden' });
    }
  });

  it('is asked for nothing about the legend when it is left out', async () => {
    mount(GanttChart, { props: { tasks, backend: 'canvas', deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      const options = JSON.parse((call[2] as string | undefined) ?? '{}') as object;
      expect(Object.keys(options)).not.toContain('legend');
    }
  });
});
