/**
 * The real Wasm draws the day scale arc-vue asks for: the caller's weekday
 * labels, a band on each weekend day, and a chart as wide as its days at the
 * width given.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import { parseCommandBuffer } from './replayCommands';
import type { GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-11-02', end: '2026-11-16' },
];
const tasksJson = JSON.stringify(tasks);
const labels = ['月', '火', '水', '木', '金', '土', '日'];

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('the day scale, drawn by the real Wasm', () => {
  const options = buildOptionsJson({
    scale: 'day',
    pxPerDay: 48,
    headerHeight: 48,
    weekendDays: [5, 6],
    weekdayLabels: labels,
    rowLabels: 'hidden',
    legend: 'hidden',
    percentLabels: 'hidden',
  });

  it('labels every day with the weekday given and bands the weekend', () => {
    const svg = render_svg(tasksJson, '[]', options);
    for (const l of labels) expect(svg).toContain(`>${l}</text>`);
    // 11/7・8、11/14・15 の四日
    expect(svg.match(/class="weekend"/g)).toHaveLength(4);
  });

  it('draws the canvas as wide as the days at the width given', () => {
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options));
    expect(buffer.error).toBeUndefined();
    // 題の列を消した左の余白（棒の高さの半分の 10px）、14 日 × 48px、右の余白 20px
    expect(buffer.viewport_width).toBe(10 + 14 * 48 + 20);
  });
});
