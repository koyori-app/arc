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

/**
 * Gate against vacuous passes: a scan-based assertion that inspected nothing
 * proves nothing. Every helper below runs its traversal through this, so an
 * empty input is a hard failure instead of a silent green.
 *
 * All four helpers take the same `(mock, sinceCallIndex, …)` shape: the window
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

/**
 * Measured-value identity: every viewport-carrying call in the window must
 * pass exactly the measured client_height. This is the falsifiable form of
 * assertNoViewportClientHeight — a forbidden value that never appears in the
 * fixture can never fail, but "every call carries the value we measured"
 * fails on any wrong height, known or not.
 */
function assertViewportClientHeightIs(
  mock: { mock: { calls: unknown[][] } },
  sinceCallIndex: number,
  expected: number,
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
      `${label}: call #${idx} carried client_height ${viewport.client_height}, expected measured ${expected}`,
    ).toBe(expected);
  });
  assertScanned(
    scanned,
    `${label} (client-height-is window from #${sinceCallIndex + 1})`,
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

  it('assertViewportClientHeightIs fails when no scanned call carried viewport_json', () => {
    expect(() =>
      assertViewportClientHeightIs(mockWith([['t', 'd', undefined, undefined]]), 0, 600, 'gate'),
    ).toThrow(/scanned 0 calls/);
  });

  it('assertViewportClientHeightIs passes on the measured value and fails on any other', () => {
    expect(() =>
      assertViewportClientHeightIs(mockWith([['t', 'd', undefined, VP]]), 0, 600, 'gate'),
    ).not.toThrow();
    const wrong = JSON.stringify({ scroll_y: 0, client_height: 599 });
    expect(() =>
      assertViewportClientHeightIs(mockWith([['t', 'd', undefined, wrong]]), 0, 600, 'gate'),
    ).toThrow(/carried client_height 599/);
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

    // Post-flip window: every call must carry viewport_json (a full render
    // here is the leak this test exists for) and carry the re-measured 600.
    assertAllCallsCarryViewport(renderSvgMock, callsBeforeFlip, 'high→low post-flip');
    assertViewportClientHeightIs(renderSvgMock, callsBeforeFlip, 600, 'high→low post-flip');

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
    assertViewportClientHeightIs(renderSvgMock, callsBeforeScroll, 600, 'low→high pre-flip');

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
    assertViewportClientHeightIs(
      renderCanvasMock,
      callsBeforeScroll,
      600,
      'low→high canvas pre-flip',
    );

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

  it('tasks change while virtualized: every viewport call in the test carries measured client_height', async () => {
    // The clientHeight getter is stubbed at the prototype level BEFORE mount:
    // happy-dom never lays out and would otherwise report 0 to the onMounted
    // measure. Stubbing first makes window 0 a real invariant — every viewport
    // call in the whole test (mount, scroll, tasks change) must carry the
    // measured height, not merely avoid one forbidden value that this fixture
    // never produces. 640 deliberately differs from the component's internal
    // clientHeight ref default (600): a component that never syncs would carry
    // 600 and be indistinguishable from a measured 600.
    const heightSpy = vi
      .spyOn(HTMLElement.prototype, 'clientHeight', 'get')
      .mockReturnValue(640);
    try {
      const wrapper = await mountHarness('low', 20);
      scrollEl(wrapper).dispatchEvent(new Event('scroll'));
      await nextTick();

      const callsBeforeTasksChange = renderSvgMock.mock.calls.length;

      (wrapper.vm as HarnessVm).setTaskCount(30);
      await flushPromises();
      await nextTick();
      await nextTick();

      // The tasks change must itself produce render calls (otherwise the
      // window-0 scan below could be satisfied by pre-change calls alone),
      // and while virtualized none of them may run without viewport_json.
      assertAllCallsCarryViewport(
        renderSvgMock,
        callsBeforeTasksChange,
        'tasks-change post-change',
      );
      // Window 0: one falsifiable predicate over the entire test — measured
      // value identity, replacing the old forbidden-880 scan (880 never
      // appeared in this fixture, so that scan could not fail).
      assertViewportClientHeightIs(renderSvgMock, 0, 640, 'tasks-change');

      wrapper.unmount();
    } finally {
      heightSpy.mockRestore();
    }
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
