/**
 * @vitest-environment happy-dom
 */
import { defineComponent, nextTick, ref } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';

const renderSvgMock = vi.fn(() => '<svg></svg>');
const renderCanvasMock = vi.fn(() => '');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
}));

function makeTasks(count: number): GanttTask[] {
  return Array.from({ length: count }, (_, i) => ({
    id: `t${i}`,
    title: `Task ${i}`,
    progress_pct: 50,
    start: '2026-06-01',
    end: '2026-06-10',
  }));
}

type Viewport = { scroll_y: number; client_height: number };

function viewportFromArg(arg: string | undefined): Viewport | null {
  if (!arg) return null;
  return JSON.parse(arg) as Viewport;
}

/** Every render_svg / render_canvas_commands call that passed viewport_json. */
function collectViewportCalls(
  mock: { mock: { calls: unknown[][] } },
): Array<{ callIndex: number; viewport: Viewport }> {
  const out: Array<{ callIndex: number; viewport: Viewport }> = [];
  mock.mock.calls.forEach((call, i) => {
    const vp = viewportFromArg(call[3] as string | undefined);
    if (vp) out.push({ callIndex: i + 1, viewport: vp });
  });
  return out;
}

function assertNoViewportClientHeight(
  mocks: Array<{ mock: { calls: unknown[][] } }>,
  forbidden: number,
  label: string,
) {
  for (const mock of mocks) {
    for (const { callIndex, viewport } of collectViewportCalls(mock)) {
      expect(
        viewport.client_height,
        `${label}: call #${callIndex} passed stale client_height ${forbidden}`,
      ).not.toBe(forbidden);
    }
  }
}

function assertAllRecentCallsOmitViewport(
  mock: { mock: { calls: unknown[][] } },
  sinceCallIndex: number,
  label: string,
) {
  mock.mock.calls.slice(sinceCallIndex).forEach((call, offset) => {
    const idx = sinceCallIndex + offset + 1;
    expect(call[3], `${label}: call #${idx} must omit viewport_json`).toBeUndefined();
  });
}

function mockScrollMetrics(el: HTMLElement, clientHeight: number, scrollTop = 0) {
  Object.defineProperty(el, 'clientHeight', {
    configurable: true,
    get: () => clientHeight,
  });
  Object.defineProperty(el, 'scrollTop', {
    configurable: true,
    get: () => scrollTop,
    set: (v: number) => {
      scrollTop = v;
    },
  });
}

const TierFlipHarness = defineComponent({
  components: { GanttChart },
  props: {
    initialTier: { type: String as () => 'low' | 'high', required: true },
    taskCount: { type: Number, required: true },
    initialBackend: { type: String as () => 'svg' | 'canvas', default: 'svg' },
  },
  setup(props) {
    const tier = ref(props.initialTier);
    const backend = ref(props.initialBackend);
    const tasks = ref(makeTasks(props.taskCount));
    function setTier(next: 'low' | 'high') {
      tier.value = next;
    }
    function setBackend(next: 'svg' | 'canvas') {
      backend.value = next;
    }
    function setTaskCount(count: number) {
      tasks.value = makeTasks(count);
    }
    return { tier, backend, tasks, setTier, setBackend, setTaskCount };
  },
  template: `
    <div class="harness" style="height: 800px; width: 400px;">
      <GanttChart :tasks="tasks" :device-tier="tier" :backend="backend" />
    </div>
  `,
});

type HarnessVm = {
  setTier: (t: 'low' | 'high') => void;
  setBackend: (b: 'svg' | 'canvas') => void;
  setTaskCount: (n: number) => void;
};

async function mountHarness(
  initialTier: 'low' | 'high',
  taskCount = 20,
  initialBackend: 'svg' | 'canvas' = 'svg',
) {
  const wrapper = mount(TierFlipHarness, {
    props: { initialTier, taskCount, initialBackend },
    attachTo: document.body,
  });
  await flushPromises();
  await nextTick();
  await nextTick();
  return wrapper;
}

function scrollEl(wrapper: ReturnType<typeof mount>) {
  return wrapper.find('.koyori-gantt-scroll').element as HTMLElement;
}

describe('GanttChart deviceTier flip viewport sync', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockClear();
  });

  it('high→low: no render call may pass stale client_height (880) after tier flip', async () => {
    const wrapper = await mountHarness('high');
    const el = scrollEl(wrapper);
    mockScrollMetrics(el, 880);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    const callsBeforeFlip = renderSvgMock.mock.calls.length;

    (wrapper.vm as HarnessVm).setTier('low');
    mockScrollMetrics(el, 600);
    await flushPromises();
    await nextTick();
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(true);

    assertNoViewportClientHeight([renderSvgMock, renderCanvasMock], 880, 'high→low');

    const postFlipCalls = renderSvgMock.mock.calls.slice(callsBeforeFlip);
    expect(postFlipCalls.length).toBeGreaterThan(0);
    postFlipCalls.forEach((call, offset) => {
      const idx = callsBeforeFlip + offset + 1;
      const viewport = viewportFromArg(call[3] as string | undefined);
      expect(viewport, `call #${idx} must pass viewport_json (full render leaked)`).not.toBeNull();
      expect(viewport!.client_height, `call #${idx}`).toBe(600);
    });

    wrapper.unmount();
  });

  it('low→high: every call after flip omits viewport_json (no stale-height path)', async () => {
    const wrapper = await mountHarness('low');
    const el = scrollEl(wrapper);
    const callsBeforeScroll = renderSvgMock.mock.calls.length;
    mockScrollMetrics(el, 600);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(true);
    const lowTierCalls = collectViewportCalls(renderSvgMock).filter(
      (c) => c.callIndex > callsBeforeScroll,
    );
    expect(lowTierCalls.length).toBeGreaterThan(0);
    for (const { viewport } of lowTierCalls) {
      expect(viewport.client_height).toBe(600);
    }

    const callsBeforeFlip = renderSvgMock.mock.calls.length;

    (wrapper.vm as HarnessVm).setTier('high');
    mockScrollMetrics(el, 880);
    await flushPromises();
    await nextTick();
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(false);
    assertAllRecentCallsOmitViewport(renderSvgMock, callsBeforeFlip, 'low→high svg');
    assertAllRecentCallsOmitViewport(renderCanvasMock, 0, 'low→high canvas');

    wrapper.unmount();
  });

  it('tasks change while virtualized: all viewport calls keep measured client_height', async () => {
    const wrapper = await mountHarness('low', 20);
    const el = scrollEl(wrapper);
    mockScrollMetrics(el, 600);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    const callsBeforeTasksChange = renderSvgMock.mock.calls.length;

    (wrapper.vm as HarnessVm).setTaskCount(30);
    await flushPromises();
    await nextTick();
    await nextTick();

    const postChangeCalls = collectViewportCalls(renderSvgMock).filter(
      (c) => c.callIndex > callsBeforeTasksChange,
    );
    expect(postChangeCalls.length).toBeGreaterThan(0);
    for (const { callIndex, viewport } of postChangeCalls) {
      expect(viewport.client_height, `call #${callIndex}`).toBe(600);
    }

    assertNoViewportClientHeight([renderSvgMock, renderCanvasMock], 880, 'tasks-change');

    wrapper.unmount();
  });
});

/**
 * Window invariant: while requestedVirtualization is true, no WASM render call
 * may run without viewport_json. Windows where viewportReady is false:
 *   W1 = component creation until onMounted sets viewportReady (every mount)
 *   W2 = tier-flip watch body (high→low), spans a nextTick
 * Every call in mock.calls is scanned — no .at(-1) sampling.
 */
describe('GanttChart WASM viewport gate', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockClear();
  });

  it('W1 svg: initial low-tier mount never calls render_svg without viewport_json', async () => {
    const wrapper = await mountHarness('low');

    expect(renderSvgMock.mock.calls.length).toBeGreaterThan(0);
    renderSvgMock.mock.calls.forEach((call, i) => {
      expect(
        call[3],
        `W1 svg: call #${i + 1} ran without viewport_json (full-range WASM render on mount)`,
      ).toBeDefined();
    });

    wrapper.unmount();
  });

  it('W1 canvas: initial low-tier mount never calls render_canvas_commands without viewport_json', async () => {
    const wrapper = await mountHarness('low', 20, 'canvas');

    expect(renderCanvasMock.mock.calls.length).toBeGreaterThan(0);
    renderCanvasMock.mock.calls.forEach((call, i) => {
      expect(
        call[3],
        `W1 canvas: call #${i + 1} ran without viewport_json (full-range WASM render on mount)`,
      ).toBeDefined();
    });

    wrapper.unmount();
  });

  it('W2 props: tasks change inside high→low flip window never leaks a viewport-less render_svg call', async () => {
    const wrapper = await mountHarness('high');
    const callsBeforeFlip = renderSvgMock.mock.calls.length;

    // Same tick: flip opens W2, tasks change triggers svgHtml recompute inside it.
    (wrapper.vm as HarnessVm).setTier('low');
    (wrapper.vm as HarnessVm).setTaskCount(30);
    await flushPromises();
    await nextTick();
    await nextTick();

    const postFlip = renderSvgMock.mock.calls.slice(callsBeforeFlip);
    expect(postFlip.length).toBeGreaterThan(0);
    postFlip.forEach((call, offset) => {
      expect(
        call[3],
        `W2 props: call #${callsBeforeFlip + offset + 1} ran without viewport_json inside flip window`,
      ).toBeDefined();
    });

    wrapper.unmount();
  });

  it('W2 backend: backend switch inside high→low flip window never leaks a viewport-less render_canvas_commands call', async () => {
    const wrapper = await mountHarness('high');

    (wrapper.vm as HarnessVm).setTier('low');
    (wrapper.vm as HarnessVm).setBackend('canvas');
    await flushPromises();
    await nextTick();
    await nextTick();

    expect(renderCanvasMock.mock.calls.length).toBeGreaterThan(0);
    renderCanvasMock.mock.calls.forEach((call, i) => {
      expect(
        call[3],
        `W2 backend: call #${i + 1} ran without viewport_json inside flip window`,
      ).toBeDefined();
    });

    wrapper.unmount();
  });

  it('non-virtual guard: high-tier mount renders immediately and omits viewport_json', async () => {
    const wrapper = await mountHarness('high');

    expect(renderSvgMock.mock.calls.length).toBeGreaterThan(0);
    renderSvgMock.mock.calls.forEach((call, i) => {
      expect(call[3], `high tier: call #${i + 1} must omit viewport_json`).toBeUndefined();
    });

    wrapper.unmount();
  });
});
