/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg></svg>');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: () => '',
  render_canvas_commands: () => '',
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = Array.from({ length: 200 }, (_, i) => ({
  id: `t${i}`,
  title: `Task ${i}`,
  progress_pct: 50,
  start: '2026-06-01',
  end: '2026-06-10',
}));

type Viewport = { scroll_y: number; client_height: number };

/** The scroll window the last SVG render carried in its options. */
function lastViewport(): Viewport | null {
  const call = renderSvgMock.mock.calls.at(-1);
  expect(call, 'the chart rendered at least once').toBeDefined();
  const options = JSON.parse((call![2] as string) ?? '{}') as { scroll_viewport?: Viewport };
  return options.scroll_viewport ?? null;
}

/** Pins an element's top on the screen, as layout would place it. */
function placeAt(el: Element, top: () => number) {
  Object.defineProperty(el, 'getBoundingClientRect', {
    configurable: true,
    value: () => ({ top: top(), bottom: top() + 8000, left: 0, right: 800, width: 800, height: 8000, x: 0, y: top() }),
  });
}

function frames() {
  return new Promise((resolve) => setTimeout(resolve, 40));
}

afterEach(() => {
  renderSvgMock.mockClear();
  document.body.innerHTML = '';
});

describe('scrollContainer', () => {
  it('leaves a chart without it as before: a low-tier chart scrolls inside its own capped area', async () => {
    const wrapper = mount(GanttChart, { props: { tasks, deviceTier: 'low' } });
    await flushPromises();
    expect(wrapper.find('.koyori-gantt-scroll').classes()).toContain('koyori-gantt-scroll--virtual');
  });

  it('drops the chart\'s own capped area, and draws the rows inside the caller\'s view', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    Object.defineProperty(container, 'clientHeight', { configurable: true, value: 400 });
    placeAt(container, () => 100);
    let chartTop = 150;

    const wrapper = mount(GanttChart, {
      props: { tasks, deviceTier: 'low', scrollContainer: container },
      attachTo: container,
    });
    await flushPromises();
    const scroller = wrapper.find('.koyori-gantt-scroll');
    expect(scroller.classes()).not.toContain('koyori-gantt-scroll--virtual');
    placeAt(scroller.element, () => chartTop);

    // まだ送っておらぬ: 図の頭は域の頭より下にあるので、図の頭から域の高さぶん
    container.dispatchEvent(new Event('scroll'));
    await frames();
    await flushPromises();
    expect(lastViewport()).toEqual({ scroll_y: 0, client_height: 400 });

    // 域を 1050 送ると図の頭は 1050 上がる。域の頭（100）から下が見える
    chartTop = 150 - 1050;
    container.dispatchEvent(new Event('scroll'));
    await frames();
    await flushPromises();
    expect(lastViewport()).toEqual({ scroll_y: 1000, client_height: 400 });
  });

  it('reads the window as the caller\'s view', async () => {
    const wrapper = mount(GanttChart, {
      props: { tasks, deviceTier: 'low', scrollContainer: window },
      attachTo: document.body,
    });
    await flushPromises();
    placeAt(wrapper.find('.koyori-gantt-scroll').element, () => -2500);
    window.dispatchEvent(new Event('scroll'));
    await frames();
    await flushPromises();
    expect(lastViewport()).toEqual({ scroll_y: 2500, client_height: window.innerHeight });
  });

  it('stops listening once the chart is gone', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    Object.defineProperty(container, 'clientHeight', { configurable: true, value: 400 });
    placeAt(container, () => 0);
    const wrapper = mount(GanttChart, {
      props: { tasks, deviceTier: 'low', scrollContainer: container },
      attachTo: container,
    });
    await flushPromises();
    const spy = vi.spyOn(container, 'removeEventListener');
    wrapper.unmount();
    expect(spy).toHaveBeenCalledWith('scroll', expect.any(Function));
  });
});
