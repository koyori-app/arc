/**
 * @vitest-environment happy-dom
 */
import { nextTick } from 'vue';
import { mount, flushPromises } from '@vue/test-utils';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

// wasm の偽り方は GanttChart.virtualization.test.ts と同じ。canvas の描き直しを
// 見るため、render_canvas_commands は CSS で 300×120 の空の buffer を返す。
const CSS_WIDTH = 300;
const CSS_HEIGHT = 120;
const renderCanvasMock = vi.fn(() =>
  JSON.stringify({
    viewport_width: CSS_WIDTH,
    viewport_height: CSS_HEIGHT,
    ops: [],
    palette: { colors: [] },
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

/** 偽の 2D context。描き直しのたびに setTransform と clearRect が呼ばれる。 */
const fakeContext = {
  setTransform: vi.fn(),
  clearRect: vi.fn(),
};

/**
 * matchMedia が返した MediaQueryList を、返した順に覚える。部品は倍率が変わるたびに
 * 新しい query を作り直すので、どの query に listener を付け外ししたかを順に見る。
 */
type FakeQuery = {
  media: string;
  matches: boolean;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};
let queries: FakeQuery[] = [];
let pixelRatio = 1;

/** query に change の listener として付けられた handler を返す。無ければ落とす。 */
function changeHandlerOf(query: FakeQuery): () => void {
  const calls = query.addEventListener.mock.calls.filter(([type]) => type === 'change');
  expect(calls.length, `${query.media} に change の listener が付いていない`).toBe(1);
  return calls[0][1] as () => void;
}

async function settle() {
  await flushPromises();
  await nextTick();
  await nextTick();
}

async function mountCanvasChart() {
  const wrapper = mount(GanttChart, {
    props: { tasks, backend: 'canvas', deviceTier: 'high' },
    attachTo: document.body,
  });
  await settle();
  return wrapper;
}

describe('GanttChart の devicePixelRatio の追従', () => {
  beforeEach(() => {
    queries = [];
    pixelRatio = 1;
    renderCanvasMock.mockClear();
    fakeContext.setTransform.mockClear();
    fakeContext.clearRect.mockClear();
    Object.defineProperty(window, 'devicePixelRatio', {
      configurable: true,
      get: () => pixelRatio,
    });
    vi.stubGlobal(
      'matchMedia',
      vi.fn((media: string) => {
        const query: FakeQuery = {
          media,
          matches: true,
          addEventListener: vi.fn(),
          removeEventListener: vi.fn(),
        };
        queries.push(query);
        return query;
      }),
    );
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(
      fakeContext as unknown as CanvasRenderingContext2D,
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('倍率が変わると描き直し、裏打ちと setTransform が新しい倍率になる', async () => {
    const wrapper = await mountCanvasChart();
    const canvas = wrapper.find('canvas').element as HTMLCanvasElement;

    // 付けた時は倍率 1。CSS の寸法と裏打ちが同じ
    expect(canvas.width).toBe(CSS_WIDTH);
    expect(canvas.height).toBe(CSS_HEIGHT);
    expect(fakeContext.setTransform).toHaveBeenLastCalledWith(1, 0, 0, 1, 0, 0);
    expect(queries.length).toBeGreaterThan(0);

    const paintsBefore = fakeContext.setTransform.mock.calls.length;

    // 窓を倍率 2 の画面へ移した時と同じく、devicePixelRatio を替えて change を撃つ
    pixelRatio = 2;
    changeHandlerOf(queries.at(-1)!)();
    await settle();

    // 描き直された（setTransform がもう一度呼ばれた）
    expect(fakeContext.setTransform.mock.calls.length).toBeGreaterThan(paintsBefore);
    // 裏打ちは変わった後の倍率。CSS の寸法は変わらない
    expect(canvas.width).toBe(CSS_WIDTH * 2);
    expect(canvas.height).toBe(CSS_HEIGHT * 2);
    expect(canvas.style.width).toBe(`${CSS_WIDTH}px`);
    expect(canvas.style.height).toBe(`${CSS_HEIGHT}px`);
    // 描画の座標を CSS の座標に合わせる倍率も 2
    expect(fakeContext.setTransform).toHaveBeenLastCalledWith(2, 0, 0, 2, 0, 0);

    wrapper.unmount();
  });

  it('倍率が変わると前の query の listener を外し、新しい倍率の query に付け直す', async () => {
    const wrapper = await mountCanvasChart();
    const first = queries.at(-1)!;
    const handler = changeHandlerOf(first);

    pixelRatio = 2;
    handler();
    await settle();

    const second = queries.at(-1)!;
    expect(second).not.toBe(first);
    expect(second.media).toContain('2dppx');
    expect(first.removeEventListener).toHaveBeenCalledWith('change', handler);
    expect(changeHandlerOf(second)).toBe(handler);

    wrapper.unmount();
  });

  it('外す時に、change の listener を付けた時と同じ handler で外す', async () => {
    const wrapper = await mountCanvasChart();
    const query = queries.at(-1)!;
    const handler = changeHandlerOf(query);
    expect(query.removeEventListener).not.toHaveBeenCalled();

    wrapper.unmount();

    expect(query.removeEventListener).toHaveBeenCalledWith('change', handler);
  });
});
