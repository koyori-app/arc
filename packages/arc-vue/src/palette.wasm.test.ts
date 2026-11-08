/**
 * The real Wasm draws the colours a caller sets: the SVG path writes them as
 * they are, `var(--…)` included, and the canvas path hands them back in the
 * command buffer's palette for `replayCommands` to resolve.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg, render_svg_error } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import { parseCommandBuffer } from './replayCommands';
import type { GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-11-02', end: '2026-11-06' },
];
const tasksJson = JSON.stringify(tasks);
const rowsJson = JSON.stringify([{ kind: 'group' }, { kind: 'task', task_id: 'a' }]);

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('a palette, drawn by the real Wasm', () => {
  const options = buildOptionsJson({
    palette: { GroupRow: 'var(--koyori-color-surface-hover)', TierMid: '#6d28d9' },
  });

  it('writes the colours into the SVG as they are', () => {
    const svg = render_svg(tasksJson, '[]', options, rowsJson);
    expect(svg).toContain('fill="var(--koyori-color-surface-hover)"');
    expect(svg).toContain('fill="#6d28d9"');
  });

  it('hands the colours back in the canvas palette', () => {
    const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options, rowsJson));
    expect(buffer.palette.colors).toContainEqual(['GroupRow', 'var(--koyori-color-surface-hover)']);
    expect(buffer.palette.colors).toContainEqual(['TierMid', '#6d28d9']);
  });

  it('refuses a colour it cannot draw', () => {
    const bad = buildOptionsJson({ palette: { TierMid: 'red' } });
    expect(render_svg_error(tasksJson, '[]', bad)).toContain('TierMid');
  });
});
