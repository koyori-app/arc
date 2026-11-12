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
});
