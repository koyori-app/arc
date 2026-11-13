/**
 * @vitest-environment happy-dom
 */
import { nextTick } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

// wasm の偽り方は GanttChart.pixelRatio.test.ts と同じ。canvas の buffer は、
// 棒の地（色の番号 0、BarBg）を palette の色で塗る FillRect を一つだけ持つ。
const renderCanvasMock = vi.fn(() =>
  JSON.stringify({
    viewport_width: 300,
    viewport_height: 120,
    ops: [{ FillRect: { x: 0, y: 0, w: 10, h: 10, color_id: 0, radius: 0 } }],
    palette: { colors: [['BarBg', 'var(--test-bar)']] },
  }),
);

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: () => '<svg></svg>',
  render_svg_error: () => '',
  render_canvas_commands: (...args: unknown[]) => renderCanvasMock(...args),
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = [
  { id: 't0', title: 'Task 0', progress_pct: 50, start: '2026-06-01', end: '2026-06-10' },
];
const tokenPalette = { BarBg: 'var(--test-bar)' };

/** 塗った色を順に覚える偽の 2D context。canvas は塗った要素を返す。 */
let fills: string[] = [];
function fakeContext(canvas: HTMLCanvasElement) {
  return new Proxy(
    {},
    {
      get(_, key) {
        if (key === 'canvas') return canvas;
        if (key === 'fillStyle') return fills.at(-1) ?? '';
        return () => undefined;
      },
      set(_, key, value) {
        if (key === 'fillStyle') fills.push(String(value));
        return true;
      },
    },
  ) as unknown as CanvasRenderingContext2D;
}

type FakeQuery = {
  media: string;
  matches: boolean;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};
let queries: FakeQuery[] = [];
const schemeQueries = () => queries.filter((q) => q.media === '(prefers-color-scheme: dark)');

/** 作られた MutationObserver と、それが見ておる要素を覚える。 */
let observers: { observed: Element[]; disconnect: ReturnType<typeof vi.fn> }[] = [];

async function settle() {
  await flushPromises();
  await nextTick();
  // 描き直しは requestAnimationFrame に寄せてある
  await new Promise((resolve) => setTimeout(resolve, 50));
  await flushPromises();
}

/**
 * happy-dom の getComputedStyle は、祖先に置いた custom property を子へ継がせぬ。
 * 使う側の token の今の値は `token` に置き、canvas の computed style がそれを返す体にする。
 * どこで token を替えたか（<html>・包み・style 要素）は、描き直しの合図を撃つ所で表す。
 */
let token = '#000000';
function setToken(value: string) {
  token = value;
}

async function mountChart(props: Record<string, unknown>) {
  const host = document.createElement('div');
  host.className = 'host';
  document.body.appendChild(host);
  const wrapper = mount(GanttChart, {
    props: { tasks, deviceTier: 'high', ...props },
    attachTo: host,
  });
  await settle();
  return wrapper;
}

describe('palette の var() を canvas で塗った図が、表示の切り替えに付いて来る', () => {
  beforeEach(() => {
    fills = [];
    queries = [];
    observers = [];
    renderCanvasMock.mockClear();
    token = '#111111';
    const realComputed = window.getComputedStyle.bind(window);
    vi.stubGlobal('getComputedStyle', (el: Element) => {
      const real = realComputed(el);
      return new Proxy(real, {
        get(target, key) {
          if (key === 'getPropertyValue') {
            return (name: string) => (name === '--test-bar' ? token : target.getPropertyValue(name));
          }
          return Reflect.get(target, key);
        },
      });
    });
    document.documentElement.className = '';
    document.documentElement.removeAttribute('style');
    vi.stubGlobal(
      'matchMedia',
      vi.fn((media: string) => {
        const query: FakeQuery = {
          media,
          matches: false,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        };
        queries.push(query);
        return query;
      }),
    );
    const Real = MutationObserver;
    vi.stubGlobal(
      'MutationObserver',
      class extends Real {
        record = { observed: [] as Element[], disconnect: vi.fn() };
        constructor(cb: MutationCallback) {
          super(cb);
          observers.push(this.record);
        }
        observe(target: Node, options?: MutationObserverInit) {
          this.record.observed.push(target as Element);
          super.observe(target, options);
        }
        disconnect() {
          this.record.disconnect();
          super.disconnect();
        }
      },
    );
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (
      this: HTMLCanvasElement,
    ) {
      return fakeContext(this);
    } as never);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('(a) <html> の class を替えると描き直し、その時の token の値で塗る', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    expect(fills.at(-1)).toBe('#111111');

    // 暗い表示に切り替える。token は <html> の class と一緒に替わる
    setToken('#222222');
    document.documentElement.classList.add('dark');
    await settle();
    expect(fills.at(-1)).toBe('#222222');
    wrapper.unmount();
  });

  it('(a) 図の祖先（使う側の包み）の data-* を替えても描き直す', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    const host = document.querySelector('.host') as HTMLElement;
    setToken('#333333');
    host.dataset.theme = 'dark';
    await settle();
    expect(fills.at(-1)).toBe('#333333');
    wrapper.unmount();
  });

  it('(b) prefers-color-scheme の change で描き直す', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    expect(schemeQueries()).toHaveLength(1);
    expect(fills.at(-1)).toBe('#111111');

    // 媒体の切り替えは属性を替えぬ（@media の規則で token が替わる）。change だけを撃つ
    setToken('#444444');
    const calls = schemeQueries()[0].addEventListener.mock.calls.filter(([t]) => t === 'change');
    expect(calls).toHaveLength(1);
    (calls[0][1] as () => void)();
    await settle();
    expect(fills.at(-1)).toBe('#444444');
    wrapper.unmount();
  });

  it('(c) var() を含まぬ palette と SVG の描きには見張りを付けぬ', async () => {
    const plain = await mountChart({ backend: 'canvas', palette: { BarBg: '#123456' } });
    const svg = await mountChart({ backend: 'svg', palette: tokenPalette });
    expect(schemeQueries()).toHaveLength(0);
    expect(observers).toHaveLength(0);
    plain.unmount();
    svg.unmount();
  });

  it('(d) unmount で見張りを外す', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    expect(observers).toHaveLength(1);
    expect(observers[0].observed).toContain(document.documentElement);
    const query = schemeQueries()[0];
    const handler = query.addEventListener.mock.calls[0][1];

    wrapper.unmount();
    expect(observers[0].disconnect).toHaveBeenCalled();
    expect(query.removeEventListener).toHaveBeenCalledWith('change', handler);

    // 外した後に class を替えても塗らぬ
    const before = fills.length;
    document.documentElement.classList.add('dark');
    await settle();
    expect(fills.length).toBe(before);
  });

  it('palette を新しい object で渡し直すと、同じ色でもその時の token の値で塗る', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    // 見張らぬ変化（style 要素の差し替えなど）で token が替わった体
    setToken('#555555');
    await settle();
    expect(fills.at(-1)).toBe('#111111');
    await wrapper.setProps({ palette: { ...tokenPalette } });
    await settle();
    expect(fills.at(-1)).toBe('#555555');
    wrapper.unmount();
  });

  it('公開の repaint() で描き直す', async () => {
    const wrapper = await mountChart({ backend: 'canvas', palette: tokenPalette });
    setToken('#666666');
    (wrapper.vm as unknown as { repaint: () => void }).repaint();
    await settle();
    expect(fills.at(-1)).toBe('#666666');
    wrapper.unmount();
  });
});
