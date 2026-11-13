/**
 * @vitest-environment happy-dom
 */
import { mount, flushPromises } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import GanttChart from './GanttChart.vue';
import type { GanttTask } from './types.ts';
import { RUST_LAYOUT } from './wasmContract';

// Wasm init that never settles keeps the chart on its loading skeleton.
vi.mock('@koyori-app/arc', () => ({
  default: vi.fn(() => new Promise(() => {})),
  render_svg: () => '',
  render_svg_error: () => undefined,
  render_canvas_commands: () => '',
  empty_svg_markup: () => '',
}));

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-05' },
];

async function skeletonLabelWidth(props: Record<string, unknown>): Promise<string> {
  const wrapper = mount(GanttChart, { props: { tasks, deviceTier: 'high', ...props } });
  await flushPromises();
  const skeleton = wrapper.find('.koyori-gantt-skeleton');
  expect(skeleton.exists()).toBe(true);
  return (skeleton.element as HTMLElement).style.getPropertyValue('--koyori-skeleton-label-w');
}

describe("the loading skeleton's label column", () => {
  it('is LABEL_W wide while the titles are shown', async () => {
    expect(await skeletonLabelWidth({})).toBe(`${RUST_LAYOUT.LABEL_W}px`);
    expect(await skeletonLabelWidth({ rowLabels: 'shown' })).toBe(`${RUST_LAYOUT.LABEL_W}px`);
  });

  it('keeps only the left margin the chart keeps once the titles are hidden', async () => {
    expect(await skeletonLabelWidth({ rowLabels: 'hidden' })).toBe(
      `${RUST_LAYOUT.HIDDEN_LABELS_LEFT_PX}px`,
    );
  });

  it('widens the title column for a bar wider than twice the column, as the chart does', async () => {
    // 棒の高さ 300 の図は、題を出しても左に 150 の余白を取る（core の label_width と同じ）
    expect(await skeletonLabelWidth({ rowLabels: 'shown', barHeight: 300, rowHeight: 300 })).toBe(
      '150px',
    );
    expect(await skeletonLabelWidth({ rowLabels: 'shown' })).toBe(`${RUST_LAYOUT.LABEL_W}px`);
  });

  it('follows the bar height the caller set, as the chart does', async () => {
    // 棒の高さ 30 の図は、題を消すと左に 15 の余白を残す（core の label_width と同じ）
    expect(await skeletonLabelWidth({ rowLabels: 'hidden', barHeight: 30 })).toBe('15px');
    expect(await skeletonLabelWidth({ rowLabels: 'shown', barHeight: 30 })).toBe(
      `${RUST_LAYOUT.LABEL_W}px`,
    );
  });
});
