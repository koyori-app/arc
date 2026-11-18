/**
 * Styled dependency edges (docs/design/dependency-states.md), against the real
 * Wasm: the state's colour and the dashed `delayed` edge reach both the SVG
 * and the canvas command buffer, and the canvas replay draws the dashed path
 * dashed in the colour the palette gives (or `Dep` without one).
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 *
 * @vitest-environment happy-dom
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import { replayCommands, type CommandBuffer } from './replayCommands';
import type { GanttDep, GanttTask } from './types.ts';

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-11-02', end: '2026-11-06' },
  { id: 'b', title: 'B', progress_pct: 0, start: '2026-11-09', end: '2026-11-13' },
  { id: 'c', title: 'C', progress_pct: 0, start: '2026-11-03', end: '2026-11-07' },
];
const deps: GanttDep[] = [
  { blocker_task_id: 'a', blocked_task_id: 'b', state: 'normal' },
  { blocker_task_id: 'a', blocked_task_id: 'c', state: 'done' },
  { blocker_task_id: 'c', blocked_task_id: 'b', state: 'delayed', note: 'C 待ち' },
];
const palette = { Dep: '#0000aa', DepDone: '#aaaaaa', DepDelayed: '#dd0000' };

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('styled dependency edges, against the real Wasm', () => {
  it('draws each state in its colour, and the delayed edge dashed, in the SVG', () => {
    const svg = render_svg(JSON.stringify(tasks), JSON.stringify(deps), buildOptionsJson({ legend: 'hidden', palette }));
    const paths = [...svg.matchAll(/<path d="[^"]*"[^>]*\/>/g)].map((m) => m[0]);
    expect(paths.map((p) => /stroke="([^"]+)"/.exec(p)?.[1])).toEqual(['#0000aa', '#aaaaaa', '#dd0000']);
    expect(paths.map((p) => p.includes('stroke-dasharray="4,3"'))).toEqual([false, false, true]);
    expect(svg).toContain('>C 待ち</text>');
  });

  it('hands the canvas the same edges, the delayed one as a dashed path', () => {
    const buffer = JSON.parse(
      render_canvas_commands(JSON.stringify(tasks), JSON.stringify(deps), buildOptionsJson({ legend: 'hidden', palette })),
    ) as CommandBuffer;
    const solid = buffer.ops.flatMap((op) => (typeof op === 'object' && 'StrokePath' in op ? [op.StrokePath.color_id] : []));
    const dashed = buffer.ops.flatMap((op) =>
      typeof op === 'object' && 'StrokeDashedPath' in op ? [[op.StrokeDashedPath.color_id, op.StrokeDashedPath.dash]] : [],
    );
    // Dep は 5、DepDone は 25、DepDelayed は 26
    expect(solid).toEqual([5, 25]);
    expect(dashed).toEqual([[26, '4,3']]);
  });

  it('replays the dashed path dashed, in DepDelayed or in Dep without it', () => {
    const strokes: { color: string; dash: number[] }[] = [];
    let dash: number[] = [];
    let strokeStyle = '';
    const canvas = document.createElement('canvas');
    const ctx = new Proxy(
      {},
      {
        get: (_t, key) => {
          if (key === 'canvas') return canvas;
          if (key === 'setLineDash') return (d: number[]) => (dash = d);
          if (key === 'stroke') return () => strokes.push({ color: strokeStyle, dash });
          return () => {};
        },
        set: (_t, key, value) => {
          if (key === 'strokeStyle') strokeStyle = value as string;
          return true;
        },
      },
    ) as unknown as CanvasRenderingContext2D;
    (globalThis as { Path2D?: unknown }).Path2D ??= class {};
    const buffer = (colors: [string, string][]): CommandBuffer => ({
      viewport_width: 10,
      viewport_height: 10,
      ops: [{ StrokeDashedPath: { d: 'M 0 0 L 5 5', color_id: 26, width: 1.5, dash: '4,3' } }],
      palette: { colors: colors as CommandBuffer['palette']['colors'] },
    });
    replayCommands(ctx, buffer([['Dep', '#0000aa'], ['DepDelayed', '#dd0000']]));
    replayCommands(ctx, buffer([['Dep', '#0000aa']]));
    expect(strokes).toEqual([
      { color: '#dd0000', dash: [4, 3] },
      { color: '#0000aa', dash: [4, 3] },
    ]);
  });
});
