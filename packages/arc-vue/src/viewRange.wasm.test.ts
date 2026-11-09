/**
 * The real Wasm draws the days a view chooses: a chart as wide as the view, a
 * bar cut at its edges with the days it runs on to, and a task outside the
 * view as its mark alone; and refuses a view it cannot draw.
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
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-11-02', end: '2026-11-18' },
  { id: 'b', title: 'Beta', progress_pct: 0, start: '2026-11-20', end: '2026-11-24' },
];
const tasksJson = JSON.stringify(tasks);

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('a view, drawn by the real Wasm', () => {
  const options = buildOptionsJson({
    view: { from: '2026-11-09', to: '2026-11-15' },
    rowLabels: 'hidden',
    legend: 'hidden',
  });

  it('cuts the bar at the edges and marks the days it runs on to', () => {
    const svg = render_svg(tasksJson, '[]', options);
    expect(svg).toContain('>← 11/2</text>');
    expect(svg).toContain('>→ 11/17</text>');
    expect(svg).toContain('>→ 11/20</text>');
  });

  it('draws the canvas as wide as the view', () => {
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options));
    expect(buffer.error).toBeUndefined();
    // 7 日 × 30px と右の余白 20px
    expect(buffer.viewport_width).toBe(7 * 30 + 20);
  });

  it('refuses a view that ends before it starts', () => {
    const backwards = buildOptionsJson({ view: { from: '2026-11-15', to: '2026-11-09' } });
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', backwards));
    expect(buffer.error).toContain('view');
  });
});
