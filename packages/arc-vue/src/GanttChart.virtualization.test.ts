/**
 * @vitest-environment happy-dom
 */
import { defineComponent, nextTick, ref } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';

const renderSvgMock = vi.fn(() => '<svg></svg>');

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_canvas_commands: vi.fn(() => ''),
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

function latestViewportJson(): { scroll_y: number; client_height: number } | null {
  const calls = renderSvgMock.mock.calls;
  for (let i = calls.length - 1; i >= 0; i -= 1) {
    const viewportArg = calls[i][3] as string | undefined;
    if (viewportArg) return JSON.parse(viewportArg) as { scroll_y: number; client_height: number };
  }
  return null;
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
  },
  setup(props) {
    const tier = ref(props.initialTier);
    const tasks = ref(makeTasks(props.taskCount));
    function setTier(next: 'low' | 'high') {
      tier.value = next;
    }
    function setTaskCount(count: number) {
      tasks.value = makeTasks(count);
    }
    return { tier, tasks, setTier, setTaskCount };
  },
  template: `
    <div class="harness" style="height: 800px; width: 400px;">
      <GanttChart :tasks="tasks" :device-tier="tier" />
    </div>
  `,
});

type HarnessVm = {
  setTier: (t: 'low' | 'high') => void;
  setTaskCount: (n: number) => void;
};

async function mountHarness(initialTier: 'low' | 'high', taskCount = 20) {
  const wrapper = mount(TierFlipHarness, {
    props: { initialTier, taskCount },
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
  });

  it('high→low: viewport client_height matches scroll container after tier flip', async () => {
    const wrapper = await mountHarness('high');
    const el = scrollEl(wrapper);
    mockScrollMetrics(el, 880);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    (wrapper.vm as HarnessVm).setTier('low');
    mockScrollMetrics(el, 600);
    await flushPromises();
    await nextTick();
    await nextTick();

    const viewport = latestViewportJson();
    expect(viewport).not.toBeNull();
    expect(viewport!.client_height).toBe(600);
    expect(viewport!.client_height).not.toBe(880);

    wrapper.unmount();
  });

  it('low→high: virtualization off so viewport_json is omitted (no stale-height bug)', async () => {
    const wrapper = await mountHarness('low');
    const el = scrollEl(wrapper);
    mockScrollMetrics(el, 600);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(true);
    expect(latestViewportJson()?.client_height).toBe(600);

    (wrapper.vm as HarnessVm).setTier('high');
    mockScrollMetrics(el, 880);
    await nextTick();
    await nextTick();

    expect(scrollEl(wrapper).classList.contains('koyori-gantt-scroll--virtual')).toBe(false);
    const lastCall = renderSvgMock.mock.calls.at(-1);
    expect(lastCall?.[3]).toBeUndefined();

    wrapper.unmount();
  });

  it('tasks change while virtualized: scroll client_height unchanged (no remeasure needed)', async () => {
    const wrapper = await mountHarness('low', 20);
    const el = scrollEl(wrapper);
    mockScrollMetrics(el, 600);
    el.dispatchEvent(new Event('scroll'));
    await nextTick();

    const beforeViewport = latestViewportJson();
    expect(beforeViewport!.client_height).toBe(600);

    (wrapper.vm as HarnessVm).setTaskCount(30);
    await nextTick();
    await nextTick();

    const afterViewport = latestViewportJson();
    expect(afterViewport!.client_height).toBe(600);

    wrapper.unmount();
  });
});
