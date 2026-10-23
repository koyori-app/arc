/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg>chart</svg>');
const renderSvgErrorMock = vi.fn((..._args: unknown[]) => undefined);
const renderCanvasMock = vi.fn((..._args: unknown[]) => '');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: (...args: unknown[]) => renderSvgErrorMock(...args),
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-08T15:30:00Z' },
];

// render_svg(tasks, deps, options_json), render_svg_error(tasks, deps, options_json)
const OPTIONS_ARG = 2;

/** The `tz_offset_minutes` a call carried in its `options_json`. */
function tzOf(call: unknown[]): unknown {
  const optionsJson = call[OPTIONS_ARG] as string | undefined;
  return optionsJson === undefined ? undefined : JSON.parse(optionsJson).tz_offset_minutes;
}

describe('tzOffsetMinutes', () => {
  beforeEach(() => {
    renderSvgMock.mockReset();
    renderSvgMock.mockReturnValue('<svg>chart</svg>');
    renderSvgErrorMock.mockReset();
    renderCanvasMock.mockReset();
    renderCanvasMock.mockReturnValue('');
  });

  it('is handed to render_svg', async () => {
    mount(GanttChart, { props: { tasks, tzOffsetMinutes: 540, deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(tzOf(call)).toBe(540);
    }
  });

  it('is handed to the Canvas entry point', async () => {
    mount(GanttChart, {
      props: { tasks, tzOffsetMinutes: -300, backend: 'canvas', deviceTier: 'high' },
    });
    await flushPromises();

    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of renderCanvasMock.mock.calls) {
      expect(tzOf(call)).toBe(-300);
    }
  });

  it('reaches render_svg_error too, because an unusable offset is only reported there', async () => {
    renderSvgMock.mockReturnValue(EMPTY_SVG_MARKUP);
    mount(GanttChart, { props: { tasks, tzOffsetMinutes: 540, deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgErrorMock).toHaveBeenCalled();
    for (const call of renderSvgErrorMock.mock.calls) {
      expect(tzOf(call)).toBe(540);
    }
  });

  it('is left out when the caller gives none, and the browser time zone is never read', async () => {
    const intl = vi.spyOn(Intl, 'DateTimeFormat');
    mount(GanttChart, { props: { tasks, deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(call[OPTIONS_ARG]).toBeUndefined();
    }
    expect(intl).not.toHaveBeenCalled();
    intl.mockRestore();
  });
});
