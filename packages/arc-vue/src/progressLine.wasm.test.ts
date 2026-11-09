/**
 * The real Wasm draws the progress line in the mode arc-vue asks for: none when
 * hidden, and when `by_row`, a solid line in its own colour with a dot at each
 * bend, off the today line only for a row that is late or ahead.
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
import type { GanttRow, GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  // 遅れ
  { id: 'a', title: 'Alpha', progress_pct: 20, start: '2026-11-02', end: '2026-11-12' },
  // 済み（行の state が done）
  { id: 'b', title: 'Beta', progress_pct: 100, start: '2026-11-02', end: '2026-11-06' },
  // まだ始まらぬ
  { id: 'c', title: 'Gamma', progress_pct: 0, start: '2026-11-10', end: '2026-11-14' },
];
const tasksJson = JSON.stringify(tasks);
const today = '2026-11-07';
const rows: GanttRow[] = [
  { kind: 'task', task_id: 'a' },
  { kind: 'task', task_id: 'b', state: 'done' },
  { kind: 'task', task_id: 'c' },
];
const rowsJson = JSON.stringify(rows);

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('the progress line modes, drawn by the real Wasm', () => {
  it('draws no line when hidden', () => {
    const svg = render_svg(tasksJson, '[]', buildOptionsJson({ today, progressLine: 'hidden' }));
    expect(svg).not.toContain('<polyline');
  });

  it('bends by_row only at the late row, with one dot', () => {
    const options = buildOptionsJson({ today, progressLine: 'by_row' });
    const svg = render_svg(tasksJson, '[]', options, rowsJson);
    expect(svg.match(/class="progress-point"/g)).toHaveLength(1);
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options, rowsJson));
    expect(buffer.error).toBeUndefined();
    const lines = buffer.ops.filter((op) => typeof op === 'object' && 'StrokePolyline' in op);
    // ProgressLine の tag は 22。実線で幅 2
    const line = lines.find((op) => op.StrokePolyline.color_id === 22);
    expect(line?.StrokePolyline.dash).toBeNull();
    expect(line?.StrokePolyline.width).toBe(2);
  });

  it('bends by_row at a row at 100% that is not done, a task left open', () => {
    const options = buildOptionsJson({ today, progressLine: 'by_row' });
    const svg = render_svg(tasksJson, '[]', options);
    expect(svg.match(/class="progress-point"/g)).toHaveLength(2);
  });
});
