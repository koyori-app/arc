/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const renderSvgMock = vi.fn((..._args: unknown[]) => '<svg></svg>');
const renderCanvasMock = vi.fn((..._args: unknown[]) => '');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: () => '',
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
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
  renderSvgMock.mockReset();
  renderSvgMock.mockImplementation(() => '<svg></svg>');
  renderCanvasMock.mockReset();
  renderCanvasMock.mockImplementation(() => '');
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

/** A buffer only the crate produces: the canvas is too large, and the chart falls back to SVG. */
const capacityBuffer = JSON.stringify({
  viewport_width: 0,
  viewport_height: 0,
  ops: [],
  palette: { colors: [] },
  error: 'canvas too large',
  code: 'canvas_capacity',
});

/** A caller's area `height` px tall on the screen at `top`, whose height can change. */
function area(top: number, height: { value: number }) {
  const container = document.createElement('div');
  document.body.append(container);
  Object.defineProperty(container, 'clientHeight', { configurable: true, get: () => height.value });
  placeAt(container, () => top);
  return container;
}

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

  describe('when the canvas falls back to SVG', () => {
    it('keeps no capped area of its own in the caller\'s area, and draws the area\'s whole view', async () => {
      renderCanvasMock.mockImplementation(() => capacityBuffer);
      renderSvgMock.mockImplementation(() => '<svg data-fallback="1"></svg>');
      const height = { value: 900 };
      const container = area(100, height);
      let chartTop = 150;
      const wrapper = mount(GanttChart, {
        props: { tasks, deviceTier: 'low', backend: 'canvas', scrollContainer: container },
        attachTo: container,
      });
      await flushPromises();
      // 落ちた先の SVG が出ておる（試すべき道を通っておる）
      expect(wrapper.find('.koyori-gantt-svg').exists()).toBe(true);
      const scroller = wrapper.find('.koyori-gantt-scroll');
      expect(scroller.classes()).not.toContain('koyori-gantt-scroll--virtual');
      placeAt(scroller.element, () => chartTop);

      chartTop = 150 - 2050;
      container.dispatchEvent(new Event('scroll'));
      await frames();
      await flushPromises();
      // 外の域で送ると、落ちた先の窓を求め直す。高さは 600 で切らず、外の域の高さのまま
      expect(lastViewport()).toEqual({ scroll_y: 2000, client_height: 900 });
    });

    it('still scrolls inside its own capped area without the caller\'s area, as before', async () => {
      renderCanvasMock.mockImplementation(() => capacityBuffer);
      renderSvgMock.mockImplementation(() => '<svg data-fallback="1"></svg>');
      const heightSpy = vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(900);
      try {
        const wrapper = mount(GanttChart, { props: { tasks, deviceTier: 'high', backend: 'canvas' } });
        await flushPromises();
        expect(wrapper.find('.koyori-gantt-svg').exists()).toBe(true);
        expect(wrapper.find('.koyori-gantt-scroll').classes()).toContain('koyori-gantt-scroll--virtual');
        await wrapper.find('.koyori-gantt-scroll').trigger('scroll');
        await flushPromises();
        // 自分の域の中では、落ちた先は 600 までを描く
        expect(lastViewport()?.client_height).toBe(600);
      } finally {
        heightSpy.mockRestore();
      }
    });
  });

  describe('when only the caller\'s area changes size', () => {
    function stubResizeObserver() {
      const observers: { callback: () => void; observed: Element[]; disconnect: ReturnType<typeof vi.fn> }[] = [];
      class FakeResizeObserver {
        observed: Element[] = [];
        disconnect = vi.fn();
        constructor(public callback: () => void) {
          observers.push(this);
        }
        observe(el: Element) {
          this.observed.push(el);
        }
        unobserve() {}
      }
      vi.stubGlobal('ResizeObserver', FakeResizeObserver);
      return observers;
    }

    it('re-reads the window when the area grows, the window itself unchanged', async () => {
      const observers = stubResizeObserver();
      const height = { value: 400 };
      const container = area(0, height);
      const wrapper = mount(GanttChart, {
        props: { tasks, deviceTier: 'low', scrollContainer: container },
        attachTo: container,
      });
      await flushPromises();
      placeAt(wrapper.find('.koyori-gantt-scroll').element, () => 0);
      container.dispatchEvent(new Event('scroll'));
      await frames();
      await flushPromises();
      expect(lastViewport()).toEqual({ scroll_y: 0, client_height: 400 });

      // 分割の pane を広げた形: 域だけが 700 になり、window の resize は来ない
      expect(observers).toHaveLength(1);
      expect(observers[0]!.observed).toEqual([container]);
      height.value = 700;
      observers[0]!.callback();
      await frames();
      await flushPromises();
      expect(lastViewport()).toEqual({ scroll_y: 0, client_height: 700 });

      wrapper.unmount();
      expect(observers[0]!.disconnect).toHaveBeenCalled();
    });

    it('stops observing the old area when the area changes, and does not observe the window', async () => {
      const observers = stubResizeObserver();
      const first = area(0, { value: 400 });
      const wrapper = mount(GanttChart, {
        props: { tasks, deviceTier: 'low', scrollContainer: first },
        attachTo: first,
      });
      await flushPromises();
      expect(observers).toHaveLength(1);
      await wrapper.setProps({ scrollContainer: window });
      expect(observers[0]!.disconnect).toHaveBeenCalled();
      // window は resize で聞くので、観る物を作らぬ
      expect(observers).toHaveLength(1);
      wrapper.unmount();
    });

    it('does nothing more without a ResizeObserver', async () => {
      vi.stubGlobal('ResizeObserver', undefined);
      const container = area(0, { value: 400 });
      const wrapper = mount(GanttChart, {
        props: { tasks, deviceTier: 'low', scrollContainer: container },
        attachTo: container,
      });
      await flushPromises();
      expect(wrapper.find('.koyori-gantt-scroll').exists()).toBe(true);
      wrapper.unmount();
    });
  });
});
