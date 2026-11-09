/**
 * The real Wasm draws the marks a task row carries: the hard deadline's
 * diamond, the state's fill and outline, the note, and a dot for a row drawn
 * as a point; and refuses a mark it cannot draw.
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
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-11-02', end: '2026-11-06' },
  { id: 'b', title: 'Beta', progress_pct: 0, start: '2026-11-05', end: '2026-11-09' },
];
const tasksJson = JSON.stringify(tasks);
const options = buildOptionsJson({ rowLabels: 'hidden', legend: 'hidden' });

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('the marks of a task row, drawn by the real Wasm', () => {
  const rows: GanttRow[] = [
    {
      kind: 'task',
      task_id: 'a',
      state: 'warn',
      hard_deadline: '2026-11-04',
      note: '目安が厳守を越えた',
    },
    { kind: 'task', task_id: 'b', shape: 'point', state: 'done', hard_deadline: '2026-11-20' },
  ];
  const rowsJson = JSON.stringify(rows);

  it('draws the diamonds, the dot and the note in the SVG', () => {
    const svg = render_svg(tasksJson, '[]', options, rowsJson);
    expect(svg.match(/class="hard-deadline"/g)).toHaveLength(2);
    expect(svg).toContain('class="bar-point"');
    expect(svg).toContain('>目安が厳守を越えた</text>');
  });

  it('gives the canvas the colours of the marks and a chart wide enough for the late deadline', () => {
    const plain = parseCommandBuffer(
      render_canvas_commands(tasksJson, '[]', options, JSON.stringify(rows.map(stripMarks))),
    );
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options, rowsJson));
    expect(buffer.error).toBeUndefined();
    const polygons = buffer.ops.filter((op) => typeof op === 'object' && 'FillPolygon' in op);
    // HardDeadline の tag は 20
    expect(polygons.filter((op) => op.FillPolygon.color_id === 20)).toHaveLength(2);
    const outlines = buffer.ops.filter((op) => typeof op === 'object' && 'StrokePath' in op);
    // WarnStroke の tag は 19
    expect(outlines.some((op) => op.StrokePath.color_id === 19)).toBe(true);
    expect(buffer.viewport_width).toBeGreaterThan(plain.viewport_width);
  });

  it('refuses a note longer than the chart takes', () => {
    const long: GanttRow[] = [{ kind: 'task', task_id: 'a', note: 'x'.repeat(201) }];
    const buffer = parseCommandBuffer(
      render_canvas_commands(tasksJson, '[]', options, JSON.stringify(long)),
    );
    expect(buffer.error).toContain('note of `a`');
  });
});

function stripMarks(row: GanttRow): GanttRow {
  return row.kind === 'task' ? { kind: 'task', task_id: row.task_id } : row;
}
