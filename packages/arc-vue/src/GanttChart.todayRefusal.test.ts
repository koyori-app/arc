/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { EMPTY_SVG_MARKUP } from './wasmContract';

const TODAY_ERROR = JSON.stringify({
  code: 'parse_error',
  error: 'parse error: today: invalid date "garbage" (expected YYYY-MM-DD or an ISO 8601 date-time)',
});

const renderSvgMock = vi.fn();
const renderSvgErrorMock = vi.fn();

vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => Promise.resolve()),
  render_svg: (...args: unknown[]) => renderSvgMock(...args),
  render_svg_error: (...args: unknown[]) => renderSvgErrorMock(...args),
  render_canvas_commands: () => '',
  empty_svg_markup: () => EMPTY_SVG_MARKUP,
}));

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-05' },
];

/** The Wasm side refuses an unreadable `today` and draws the blank chart. */
function todayOf(optionsJson?: string): string | undefined {
  return optionsJson === undefined ? undefined : JSON.parse(optionsJson).today;
}

function wasmBehavesLikeTheCrate() {
  renderSvgMock.mockImplementation((_t: string, _d: string, options?: string) =>
    todayOf(options) === 'garbage' ? EMPTY_SVG_MARKUP : '<svg>chart</svg>',
  );
  renderSvgErrorMock.mockImplementation((_t: string, _d: string, options?: string) =>
    todayOf(options) === 'garbage' ? TODAY_ERROR : undefined,
  );
}

describe('an unreadable `today`', () => {
  beforeEach(() => {
    renderSvgMock.mockReset();
    renderSvgErrorMock.mockReset();
    wasmBehavesLikeTheCrate();
  });

  it('reaches the same alert an unreadable task does, not a silent blank chart', async () => {
    const wrapper = mount(GanttChart, {
      props: { tasks, today: 'garbage', deviceTier: 'high' },
    });
    await flushPromises();

    const alert = wrapper.find('[role="alert"]');
    expect(alert.exists()).toBe(true);
    expect(alert.text()).toContain('today');
    expect(alert.text()).toContain('invalid date');
  });

  it('hands `today` to render_svg_error, which is the only channel that can report it', async () => {
    mount(GanttChart, { props: { tasks, today: 'garbage', deviceTier: 'high' } });
    await flushPromises();

    expect(renderSvgErrorMock).toHaveBeenCalled();
    const todays = renderSvgErrorMock.mock.calls.map((c) => todayOf(c[2] as string | undefined));
    expect(todays).toContain('garbage');
  });

  it('a readable date-time draws the chart and shows no alert', async () => {
    const wrapper = mount(GanttChart, {
      props: { tasks, today: '2026-06-03T09:00:00+09:00', deviceTier: 'high' },
    });
    await flushPromises();

    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.html()).toContain('chart');
  });
});
