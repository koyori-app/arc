/**
 * The mocked tests pin what arc-vue sends; this one runs the real Wasm to check
 * what comes back. A chart whose rows the caller lays out must come back as
 * tall as `chartHeightForRows` reserves, on the SVG and the canvas paths, and a
 * row list the crate refuses must be refused through `render_svg_error` too.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`,
 * the same as `vue-tsc` and `vite build` do.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg, render_svg_error } from '@koyori-app/arc';
import { chartHeightForRows } from './canvasFallback';
import { buildOptionsJson } from './options';
import { parseCommandBuffer } from './replayCommands';
import type { GanttRow, GanttTask } from './types.ts';
import { parseRenderError, RUST_ERROR_CODES } from './wasmContract';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-11-02', end: '2026-11-06' },
  { id: 'b', title: 'Bravo', progress_pct: 0, start: '2026-11-05', end: '2026-11-10' },
  { id: 'c', title: 'Charlie', progress_pct: 100, start: '2026-11-03', end: '2026-11-04' },
];
const tasksJson = JSON.stringify(tasks);
const rows: GanttRow[] = [
  { kind: 'group' },
  { kind: 'task', task_id: 'b' },
  { kind: 'task', task_id: 'a', height: 64 },
  { kind: 'group' },
  { kind: 'task', task_id: 'c' },
];
const sizes = { rowHeight: 32, headerHeight: 48 };
const options = buildOptionsJson({
  legend: 'hidden',
  rowLabels: 'hidden',
  rowHeightPx: sizes.rowHeight,
  headerHeightPx: sizes.headerHeight,
  barHeightPx: 12,
  barAlign: 'bottom',
});

function svgHeight(svg: string): number {
  const match = /^<svg [^>]*\bheight="([\d.]+)"/.exec(svg);
  if (!match) throw new Error(`no height on the root: ${svg.slice(0, 120)}`);
  return Number(match[1]);
}

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('rows the caller lays out, drawn by the real Wasm', () => {
  const reserved = chartHeightForRows(rows, tasks.length, 'hidden', sizes);

  it('comes back from the SVG path as tall as the height reserved for it', () => {
    const svg = render_svg(tasksJson, '[]', options, JSON.stringify(rows));
    expect(svg).toContain('class="group-row"');
    expect(svgHeight(svg)).toBe(reserved);
  });

  it('comes back from the canvas path as tall as the height reserved for it', () => {
    const buffer = parseCommandBuffer(
      render_canvas_commands(tasksJson, '[]', options, JSON.stringify(rows)),
    );
    expect(buffer.error).toBeUndefined();
    expect(buffer.viewport_height).toBe(reserved);
  });

  it('is refused through render_svg_error when a row names no task', () => {
    const bad = JSON.stringify([{ kind: 'task', task_id: 'zz' }]);
    const reported = render_svg_error(tasksJson, '[]', options, bad);
    expect(reported).toBeTruthy();
    const failure = parseRenderError(reported!);
    expect(failure.code).toBe(RUST_ERROR_CODES.CODE_PARSE_ERROR);
    expect(failure.message).toContain('zz');
  });
});
