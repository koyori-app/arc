/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttLane, GanttTask } from './types.ts';
import { chartHeightForTaskCount } from './canvasFallback';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg width="400" height="240">chart</svg>');
const renderCanvasMock = vi.fn((..._args: unknown[]) => '');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: () => undefined,
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-05', lane_ids: ['x'] },
  { id: 'b', title: 'B', progress_pct: 20, start: '2026-06-03', end: '2026-06-08', lane_ids: ['x', 'y'] },
];
const lanes: GanttLane[] = [
  { id: 'x', label: 'X' },
  { id: 'y', label: 'Y' },
];

const heightOf = (w: ReturnType<typeof mount>) =>
  Number.parseInt((w.get('.koyori-gantt-inner').element as HTMLElement).style.height, 10);

describe('what reaches the Wasm entry points with lanes', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockClear();
  });

  it('sends `lanes` in the options of the SVG path, and the tasks with their lane_ids', async () => {
    mount(GanttChart, { props: { tasks, lanes, deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgMock).toHaveBeenCalled();
    for (const call of renderSvgMock.mock.calls) {
      expect(call[0]).toBe(JSON.stringify(tasks));
      expect(JSON.parse(call[2] as string)).toEqual({ lanes });
    }
  });

  it('sends `lanes` on the Canvas path too', async () => {
    mount(GanttChart, { props: { tasks, lanes, backend: 'canvas', deviceTier: 'high' } });
    await flushPromises();

    expect(renderCanvasMock).toHaveBeenCalled();
    for (const call of renderCanvasMock.mock.calls) {
      expect(JSON.parse(call[2] as string)).toEqual({ lanes });
    }
  });

  it('sends the window a low tier measures along with the lanes', async () => {
    mount(GanttChart, { props: { tasks, lanes, today: '2026-06-04', deviceTier: 'low' } });
    await flushPromises();

    const last = renderSvgMock.mock.calls.at(-1)!;
    const options = JSON.parse(last[2] as string);
    expect(options.lanes).toEqual(lanes);
    expect(options.today).toBe('2026-06-04');
    expect(options.scroll_viewport).toBeDefined();
  });

  it('sends an empty list of lanes as it is', async () => {
    mount(GanttChart, { props: { tasks, lanes: [], deviceTier: 'high' } });
    await flushPromises();

    for (const call of renderSvgMock.mock.calls) {
      expect(call[2]).toBe('{"lanes":[]}');
    }
  });

  it('sends nothing about lanes, and the same three arguments as before, when there are none', async () => {
    mount(GanttChart, { props: { tasks, deviceTier: 'high' } });
    await flushPromises();

    for (const call of renderSvgMock.mock.calls) {
      expect(call).toEqual([JSON.stringify(tasks), '[]', undefined]);
    }
  });
});

describe('the room reserved for a chart in lanes', () => {
  it('is the height the drawing says it has, not one row a task', async () => {
    renderSvgMock.mockImplementation(() => '<svg width="400" height="240">chart</svg>');
    const withLanes = mount(GanttChart, { props: { tasks, lanes, deviceTier: 'high' } });
    await flushPromises();
    expect(heightOf(withLanes)).toBe(240);
    expect(240).not.toBe(chartHeightForTaskCount(tasks.length));
  });

  it('follows the drawing when it grows', async () => {
    renderSvgMock.mockImplementation(() => '<svg width="400" height="240">chart</svg>');
    const w = mount(GanttChart, { props: { tasks, lanes, deviceTier: 'high' } });
    await flushPromises();
    renderSvgMock.mockImplementation(() => '<svg width="400" height="520">chart</svg>');
    await w.setProps({ lanes: [...lanes, { id: 'z', label: 'Z' }] });
    await flushPromises();
    expect(heightOf(w)).toBe(520);
  });

  it('stays one row a task for a chart without lanes', async () => {
    renderSvgMock.mockImplementation(() => '<svg width="400" height="240">chart</svg>');
    const plain = mount(GanttChart, { props: { tasks, deviceTier: 'high' } });
    await flushPromises();
    expect(heightOf(plain)).toBe(chartHeightForTaskCount(tasks.length));
  });
});

describe('clicking a bar of a chart in lanes', () => {
  it('emits the same task from either bar of a task that is in two lanes', async () => {
    renderSvgMock.mockImplementation(
      () =>
        '<svg width="400" height="240">'
        + '<g data-task-id="a"><rect/></g>'
        + '<g data-task-id="b"><rect class="first"/></g>'
        + '<g data-task-id="b"><rect class="second"/></g>'
        + '</svg>',
    );
    const w = mount(GanttChart, { props: { tasks, lanes, deviceTier: 'high' } });
    await flushPromises();

    const bars = w.findAll('[data-task-id="b"]');
    expect(bars).toHaveLength(2);
    await bars[0]!.get('rect').trigger('click');
    await bars[1]!.get('rect').trigger('click');
    const emitted = w.emitted('taskClick')!;
    expect(emitted).toHaveLength(2);
    // Vue wraps props, so the emitted task is compared by content, and the two emits by identity.
    expect(emitted[0]![0]).toEqual(tasks[1]);
    expect(emitted[1]![0]).toBe(emitted[0]![0]);
  });
});

describe('the window of a chart in lanes that grows once it is drawn', () => {
  // happy-dom has no layout. The scroll box is as tall as what it holds, capped at 504px
  // (a viewport of 720px, 70vh), which is what a browser does for the real thing.
  const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
      configurable: true,
      get(this: HTMLElement) {
        if (!this.classList.contains('koyori-gantt-scroll')) return 0;
        const inner = this.querySelector<HTMLElement>('.koyori-gantt-inner');
        return Math.min(Number.parseInt(inner?.style.height ?? '0', 10) || 0, 504);
      },
    });
  });
  afterEach(() => {
    if (original) Object.defineProperty(HTMLElement.prototype, 'clientHeight', original);
  });

  it('measures the scroll box again after the real height replaces the guess', async () => {
    renderSvgMock.mockImplementation(() => '<svg width="400" height="900">chart</svg>');
    mount(GanttChart, { props: { tasks, lanes, deviceTier: 'low' } });
    await flushPromises();
    await flushPromises();

    const last = renderSvgMock.mock.calls.at(-1)!;
    // The guess was two lanes of one row each (160); the drawing is 900 tall and the box 504.
    expect(JSON.parse(last[2] as string).scroll_viewport.client_height).toBe(504);
  });

  it('does not measure again for a chart without lanes, whose height was known at the start', async () => {
    renderSvgMock.mockClear();
    renderSvgMock.mockImplementation(() => '<svg width="400" height="900">chart</svg>');
    mount(GanttChart, { props: { tasks, deviceTier: 'low' } });
    await flushPromises();
    await flushPromises();

    // One row a task, header, legend and padding: 2 * 40 + 30 + 40 + 10, not the drawing's 900.
    const last = renderSvgMock.mock.calls.at(-1)!;
    expect(JSON.parse(last[2] as string).scroll_viewport.client_height).toBe(160);
  });
});
