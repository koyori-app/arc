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

/**
 * Gate against vacuous passes: a scan-based assertion that inspected nothing
 * proves nothing. Every helper below runs its traversal through this, so an
 * empty input is a hard failure instead of a silent green.
 *
 * All three helpers take the same `(mock, sinceCallIndex, …)` shape: the window
 * a scan covers is a parameter, so an assertion cannot be satisfied by calls
 * outside the window it claims to check.
 */
function assertScanned(count: number, label: string) {
  expect(
    count,
    `${label}: scanned 0 calls — vacuous assertion, nothing was actually checked`,
  ).toBeGreaterThan(0);
}

function assertNoViewportClientHeight(
  mock: { mock: { calls: unknown[][] } },
  sinceCallIndex: number,
  forbidden: number,
  label: string,
) {
  const callWindow = mock.mock.calls.slice(sinceCallIndex);
  let scanned = 0;
  callWindow.forEach((call, offset) => {
    const idx = sinceCallIndex + offset + 1;
    const viewport = viewportFromArg(call[3] as string | undefined);
    if (!viewport) return;
    scanned += 1;
    expect(
      viewport.client_height,
      `${label}: call #${idx} passed stale client_height ${forbidden}`,
    ).not.toBe(forbidden);
  });
  assertScanned(
    scanned,
    `${label} (no-stale-client-height window from #${sinceCallIndex + 1})`,
  );
}

function assertAllRecentCallsOmitViewport(
  mock: { mock: { calls: unknown[][] } },
  sinceCallIndex: number,
  label: string,
) {
  const callWindow = mock.mock.calls.slice(sinceCallIndex);
  assertScanned(callWindow.length, `${label} (omit-viewport window from #${sinceCallIndex + 1})`);
  callWindow.forEach((call, offset) => {
    const idx = sinceCallIndex + offset + 1;
    expect(call[3], `${label}: call #${idx} must omit viewport_json`).toBeUndefined();
  });
}

function assertAllCallsCarryViewport(
  mock: { mock: { calls: unknown[][] } },
  sinceCallIndex: number,
  label: string,
) {
  const callWindow = mock.mock.calls.slice(sinceCallIndex);
  assertScanned(callWindow.length, `${label} (carry-viewport window from #${sinceCallIndex + 1})`);
  callWindow.forEach((call, offset) => {
    const idx = sinceCallIndex + offset + 1;
    expect(call[3], `${label}: call #${idx} ran without viewport_json`).toBeDefined();
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

/**
 * Positive control for the scan gates themselves. Without these, a helper that
 * silently scans an empty call list keeps reporting green — the exact failure
 * that let `assertAllRecentCallsOmitViewport(renderCanvasMock, 0)` sit in the
 * svg test asserting nothing.
 */
describe('scan-assertion gates', () => {
  const VP = JSON.stringify({ scroll_y: 0, client_height: 600 });
  const mockWith = (calls: unknown[][]) => ({ mock: { calls } });

  it('assertAllRecentCallsOmitViewport fails when the scan window is empty', () => {
    expect(() =>
      assertAllRecentCallsOmitViewport(mockWith([]), 0, 'gate'),
    ).toThrow(/scanned 0 calls/);
  });

  it('assertAllCallsCarryViewport fails when the scan window is empty', () => {
    expect(() =>
      assertAllCallsCarryViewport(mockWith([]), 0, 'gate'),
    ).toThrow(/scanned 0 calls/);
  });

  it('assertNoViewportClientHeight fails when no scanned call carried viewport_json', () => {
    expect(() =>
      assertNoViewportClientHeight(mockWith([['t', 'd', undefined, undefined]]), 0, 880, 'gate'),
    ).toThrow(/scanned 0 calls/);
  });

  it('gates pass on a legitimate non-empty scan (and still catch real violations)', () => {
    expect(() =>
      assertNoViewportClientHeight(mockWith([['t', 'd', undefined, VP]]), 0, 880, 'gate'),
    ).not.toThrow();
    expect(() =>
      assertAllRecentCallsOmitViewport(mockWith([['t', 'd', undefined, undefined]]), 0, 'gate'),
    ).not.toThrow();
    expect(() =>
      assertAllCallsCarryViewport(mockWith([['t', 'd', undefined, VP]]), 0, 'gate'),
    ).not.toThrow();

    const stale = JSON.stringify({ scroll_y: 0, client_height: 880 });
    expect(() =>
      assertNoViewportClientHeight(mockWith([['t', 'd', undefined, stale]]), 0, 880, 'gate'),
    ).toThrow(/stale client_height 880/);
    expect(() =>
      assertAllRecentCallsOmitViewport(mockWith([['t', 'd', undefined, VP]]), 0, 'gate'),
    ).toThrow(/must omit viewport_json/);
    expect(() =>
      assertAllCallsCarryViewport(mockWith([['t', 'd', undefined, undefined]]), 0, 'gate'),
    ).toThrow(/ran without viewport_json/);
  });
});

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

    // Window 0: the invariant spans the whole test — no call, before or after
    // the flip, may carry the stale height. The svg harness never mounts a
    // canvas, so scanning renderCanvasMock here would inspect nothing.
    assertNoViewportClientHeight(renderSvgMock, 0, 880, 'high→low');

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

    wrapper.unmount();
  });

  // The svg harness never mounts a canvas, so the canvas leg needs its own
  // harness — asserting on renderCanvasMock from inside the svg test only ever
  // scanned an empty call list.
  it('low→high canvas: viewport carried while virtual, omitted after flip', async () => {
    const wrapper = await mountHarness('low', 20, 'canvas');
    const el = scrollEl(wrapper);
    const callsBeforeScroll = renderCanvasMock.mock.calls.length;
    mockScrollMetrics(el, 600);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(true);
    assertAllCallsCarryViewport(renderCanvasMock, 0, 'low→high canvas pre-flip');
    const lowTierCalls = collectViewportCalls(renderCanvasMock).filter(
      (c) => c.callIndex > callsBeforeScroll,
    );
    expect(lowTierCalls.length).toBeGreaterThan(0);
    for (const { callIndex, viewport } of lowTierCalls) {
      expect(viewport.client_height, `pre-flip call #${callIndex}`).toBe(600);
    }

    const callsBeforeFlip = renderCanvasMock.mock.calls.length;

    (wrapper.vm as HarnessVm).setTier('high');
    mockScrollMetrics(el, 880);
    await flushPromises();
    await nextTick();
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(false);
    // No stale-height assertion here: after the flip the window provably carries
    // zero viewport_json calls (assertAllRecentCallsOmitViewport enforces it), so
    // a client_height scan over that window could only ever inspect nothing.
    assertAllRecentCallsOmitViewport(renderCanvasMock, callsBeforeFlip, 'low→high canvas');

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

    // Window 0: covers the pre-change low-tier calls too, which no other
    // assertion in this test inspects.
    assertNoViewportClientHeight(renderSvgMock, 0, 880, 'tasks-change');

    wrapper.unmount();
  });
});

/**
 * Window invariant: while requestedVirtualization is true, no WASM render call
 * may run without viewport_json. Windows where viewportReady is false:
 *   W1 = component creation until onMounted sets viewportReady (every mount)
 *   W2 = tier-flip watch body (high→low), spans a nextTick
 * Every assertion here goes through the shared scan helpers, so the window is
 * declared per call and an empty window fails instead of passing vacuously —
 * no .at(-1) sampling and no hand-written call-count guards.
 */
describe('GanttChart WASM viewport gate', () => {
  beforeEach(() => {
    renderSvgMock.mockClear();
    renderCanvasMock.mockClear();
  });

  it('W1 svg: initial low-tier mount never calls render_svg without viewport_json', async () => {
    const wrapper = await mountHarness('low');

    assertAllCallsCarryViewport(renderSvgMock, 0, 'W1 svg mount');

    wrapper.unmount();
  });

  it('W1 canvas: initial low-tier mount never calls render_canvas_commands without viewport_json', async () => {
    const wrapper = await mountHarness('low', 20, 'canvas');

    assertAllCallsCarryViewport(renderCanvasMock, 0, 'W1 canvas mount');

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

    assertAllCallsCarryViewport(renderSvgMock, callsBeforeFlip, 'W2 props flip window');

    wrapper.unmount();
  });

  it('W2 backend: backend switch inside high→low flip window never leaks a viewport-less render_canvas_commands call', async () => {
    const wrapper = await mountHarness('high');

    (wrapper.vm as HarnessVm).setTier('low');
    (wrapper.vm as HarnessVm).setBackend('canvas');
    await flushPromises();
    await nextTick();
    await nextTick();

    assertAllCallsCarryViewport(renderCanvasMock, 0, 'W2 backend flip window');

    wrapper.unmount();
  });

  it('non-virtual guard: high-tier mount renders immediately and omits viewport_json', async () => {
    const wrapper = await mountHarness('high');

    assertAllRecentCallsOmitViewport(renderSvgMock, 0, 'non-virtual high tier');

    wrapper.unmount();
  });
});
