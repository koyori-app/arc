/**
 * The real Wasm draws the colour a task row gives its bar the same way on the
 * SVG and the canvas path, and refuses a colour it cannot write.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg, render_svg_error } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import { parseCommandBuffer, type DrawOp } from './replayCommands';
import type { GanttRow, GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 50, start: '2026-11-02', end: '2026-11-06' },
  { id: 'b', title: 'Beta', progress_pct: 20, start: '2026-11-04', end: '2026-11-09' },
];
const tasksJson = JSON.stringify(tasks);
const rows: GanttRow[] = [
  { kind: 'task', task_id: 'a', color: 'var(--status-doing)', state: 'warn' },
  { kind: 'task', task_id: 'b' },
];
const rowsJson = JSON.stringify(rows);

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

function opsOf(json: string, taskId: string): DrawOp[] {
  const ops = parseCommandBuffer(json).ops;
  const start = ops.findIndex((op) => typeof op === 'object' && 'GroupStart' in op && op.GroupStart.task_id === taskId);
  const end = ops.indexOf('GroupEnd', start);
  return ops.slice(start + 1, end);
}

describe('a row colour, drawn by the real Wasm', () => {
  it('shades the bar on both paths', () => {
    const svg = render_svg(tasksJson, '[]', undefined, rowsJson);
    expect(svg).toContain('fill="var(--status-doing)" fill-opacity="0.35"');
    expect(svg).toContain('paint-order="stroke"');

    const json = render_canvas_commands(tasksJson, '[]', undefined, rowsJson);
    expect(parseCommandBuffer(json).palette.colors).toContainEqual([{ Row: 0 }, 'var(--status-doing)']);
    const ops = opsOf(json, 'a');
    expect(ops[0]).toMatchObject({ FillRectAlpha: { color_id: 256, alpha: 0.35 } });
    expect(ops[1]).toMatchObject({ FillRect: { color_id: 256 } });
    // `warn` keeps its outline.
    expect(ops[2]).toMatchObject({ StrokePath: { color_id: 19 } });
    expect(ops.some((op) => typeof op === 'object' && 'DrawHaloText' in op)).toBe(true);
  });

  it('draws the progress as a line and leaves the labels out when asked', () => {
    const options = buildOptionsJson({ rowColorProgress: 'line', rowColorPercent: 'hidden' });
    const svg = render_svg(tasksJson, '[]', options, rowsJson);
    expect(svg).toContain('class="row-progress-track"');
    expect(svg).toContain('class="row-progress"');
    expect(svg).not.toContain('paint-order');
    const ops = opsOf(render_canvas_commands(tasksJson, '[]', options, rowsJson), 'a');
    expect(ops[0]).toMatchObject({ FillRect: { color_id: 256 } });
    expect(ops.some((op) => typeof op === 'object' && 'DrawText' in op && op.DrawText.text.endsWith('%'))).toBe(false);
  });

  it('leaves a chart without a row colour as it was', () => {
    const plainRows = JSON.stringify([
      { kind: 'task', task_id: 'a' },
      { kind: 'task', task_id: 'b', color: null },
    ]);
    const options = buildOptionsJson({ rowColorProgress: 'line', rowColorPercent: 'hidden' });
    expect(render_svg(tasksJson, '[]', options, plainRows)).toBe(
      render_svg(tasksJson, '[]', undefined, plainRows),
    );
  });

  it('refuses a colour it cannot write', () => {
    for (const color of ['red', '#fff" onload="x', 'javascript:alert(1)', 'url(#a)']) {
      const bad = JSON.stringify([{ kind: 'task', task_id: 'a', color }]);
      expect(render_svg_error(tasksJson, '[]', undefined, bad)).toContain('color of `a`');
      expect(render_svg(tasksJson, '[]', undefined, bad)).not.toContain(color);
    }
  });
});
