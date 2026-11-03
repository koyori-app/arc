/**
 * @vitest-environment happy-dom
 *
 * The mocked tests pin what arc-vue sends; this one runs the real Wasm to check what comes
 * back for a chart in lanes: the height the drawing and the canvas agree on, the bars where
 * a click has to find them, the window a scroll shows, and that a chart without lanes is
 * left alone.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { mount, flushPromises } from '@vue/test-utils';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg, render_svg_error } from '@koyori-app/arc';
import GanttChart from './GanttChart.vue';
import { svgRootHeight } from './laneHeight';
import { buildOptionsJson } from './options';
import { findTaskAtPoint, parseCommandBuffer, replayCommands } from './replayCommands';
import type { GanttLane, GanttTask } from './types.ts';
import { RUST_ERROR_CODES } from './wasmContract';

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 50, start: '2026-06-01', end: '2026-06-06', lane_ids: ['x'] },
  { id: 'b', title: 'B', progress_pct: 20, start: '2026-06-03', end: '2026-06-09', lane_ids: ['x', 'y'] },
  { id: 'c', title: 'C', progress_pct: 0, start: '2026-06-02', end: '2026-06-04', lane_ids: ['y'] },
];
const lanes: GanttLane[] = [
  { id: 'x', label: 'X' },
  { id: 'y', label: 'Y' },
];
const tasksJson = JSON.stringify(tasks);
const laneOptions = buildOptionsJson({ lanes });

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
  // The canvas replay draws arrows through Path2D, which happy-dom does not have.
  globalThis.Path2D = class {
    constructor(public d: string) {}
  } as unknown as typeof Path2D;
});

/** A 2D context that takes every call and ignores it: only the hit regions are wanted. */
function nullContext(): CanvasRenderingContext2D {
  const state: Record<string, unknown> = {};
  return new Proxy(state, {
    get: (target, key: string) => (key in target ? target[key] : () => undefined),
    set: (target, key: string, value) => {
      target[key] = value;
      return true;
    },
  }) as unknown as CanvasRenderingContext2D;
}

function hitRegionsOf(options: string | undefined) {
  const buffer = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', options));
  expect(buffer.error).toBeUndefined();
  return { buffer, regions: replayCommands(nullContext(), buffer).hitRegions };
}

describe('a chart in lanes, drawn by the real Wasm', () => {
  it('is as tall on the canvas as the root of the SVG says', () => {
    const svg = render_svg(tasksJson, '[]', laneOptions);
    const { buffer } = hitRegionsOf(laneOptions);
    // Two lanes of two rows each (a and b overlap in x, b and c overlap in y), plus the
    // header (30), the legend (40) and the bottom padding (10).
    expect(svgRootHeight(svg)).toBe(2 * 2 * 40 + 30 + 40 + 10);
    expect(buffer.viewport_height).toBe(svgRootHeight(svg));
  });

  it('puts a hit region on every bar, and a task in two lanes has one in each', () => {
    const { regions } = hitRegionsOf(laneOptions);
    expect(regions.map((r) => r.taskId).sort()).toEqual(['a', 'b', 'b', 'c']);
  });

  it('finds the task under a click wherever the packing moved its bar to', () => {
    const { regions } = hitRegionsOf(laneOptions);
    // x: a on row 0, b on row 1 (30 to 110). y: c on row 0, b on row 1 (110 to 190).
    // A bar is 20 high and starts 10 below the top of its row.
    const center = (taskId: string, top: number) => {
      const r = regions.find((region) => region.taskId === taskId && Math.abs(region.y - top) < 1e-9)!;
      expect(r, `${taskId} at ${top}`).toBeDefined();
      return [r.x + r.w / 2, r.y + r.h / 2] as const;
    };
    const aAt = center('a', 40);
    const bUpper = center('b', 80);
    const cAt = center('c', 120);
    const bLower = center('b', 160);
    expect(findTaskAtPoint(regions, ...aAt)).toBe('a');
    expect(findTaskAtPoint(regions, ...bUpper)).toBe('b');
    expect(findTaskAtPoint(regions, ...cAt)).toBe('c');
    expect(findTaskAtPoint(regions, ...bLower)).toBe('b');
    // The gap between two rows of a lane belongs to no task.
    expect(findTaskAtPoint(regions, aAt[0], 30 + 40 - 2)).toBeNull();
  });

  it('keeps the SVG and the canvas in step: the same bars in the same order', () => {
    const svg = render_svg(tasksJson, '[]', laneOptions);
    const fromSvg = [...svg.matchAll(/data-task-id="([^"]+)"/g)].map((m) => m[1]);
    const { regions } = hitRegionsOf(laneOptions);
    expect(regions.map((r) => r.taskId)).toEqual(fromSvg);
  });

  it('draws only the lanes a scroll shows, found from the heights of the lanes', () => {
    // Five lanes of 5, 1, 1, 3 and 1 rows: tops at 0, 200, 240, 280, 400 and 440.
    const sizes = [5, 1, 1, 3, 1];
    const many: GanttTask[] = [];
    const manyLanes: GanttLane[] = [];
    sizes.forEach((n, l) => {
      manyLanes.push({ id: `L${l}`, label: `L${l}` });
      for (let r = 0; r < n; r += 1) {
        many.push({
          id: `L${l}r${r}`,
          title: 'T',
          progress_pct: 0,
          start: '2026-06-01',
          end: '2026-06-05',
          lane_ids: [`L${l}`],
        });
      }
    });
    const json = JSON.stringify(many);
    const options = buildOptionsJson({
      lanes: manyLanes,
      scrollViewport: { scroll_y: 330, client_height: 40 },
    });
    const svg = render_svg(json, '[]', options);
    const drawn = [...svg.matchAll(/data-task-id="([^"]+)"/g)].map((m) => m[1]).sort();
    // The band is 220 to 420 (the window less the 30 header, widened by two rows of 40).
    expect(drawn).toEqual(['L1r0', 'L2r0', 'L3r0', 'L3r1', 'L3r2', 'L4r0']);
    // Without a window every bar is drawn, and the chart is the same height.
    const all = render_svg(json, '[]', buildOptionsJson({ lanes: manyLanes }));
    expect([...all.matchAll(/data-task-id=/g)]).toHaveLength(many.length);
    expect(svgRootHeight(svg)).toBe(svgRootHeight(all));
  });

  it('refuses a lane_ids it cannot read, with the code of a parse error', () => {
    const odd = JSON.stringify([{ id: 'a', title: 'A', progress_pct: 0, start: '2026-06-01', lane_ids: 7 }]);
    const reported = JSON.parse(render_svg_error(odd, '[]', laneOptions)!);
    expect(reported.code).toBe(RUST_ERROR_CODES.CODE_PARSE_ERROR);
    expect(reported.error).toContain('lane_ids');
  });
});

describe('a chart without lanes, drawn by the real Wasm', () => {
  const plain: GanttTask[] = tasks.map(({ lane_ids: _ignored, ...rest }) => rest);

  it('is the chart drawn before, whatever lane_ids the tasks carry', () => {
    for (const options of [undefined, '{}', '{"lanes":null}']) {
      expect(render_svg(tasksJson, '[]', options)).toBe(render_svg(JSON.stringify(plain), '[]', undefined));
      expect(render_canvas_commands(tasksJson, '[]', options)).toBe(
        render_canvas_commands(JSON.stringify(plain), '[]', undefined),
      );
    }
  });

  it('still draws the dependency arrows and the progress line a lane chart leaves out', () => {
    const deps = JSON.stringify([{ blocker_task_id: 'a', blocked_task_id: 'b' }]);
    const options = buildOptionsJson({ today: '2026-06-04' });
    const withoutLanes = render_svg(tasksJson, deps, options);
    expect(withoutLanes).toContain('progress-status-line');
    expect(withoutLanes).toContain('<path');
    const inLanes = render_svg(tasksJson, deps, buildOptionsJson({ today: '2026-06-04', lanes }));
    expect(inLanes).not.toContain('progress-status-line');
    expect(inLanes).not.toContain('<path');
  });
});

describe('GanttChart in lanes, with the real Wasm', () => {
  it('reserves the height the drawing has, and emits the same task from either bar of a shared task', async () => {
    const w = mount(GanttChart, { props: { tasks, lanes, deviceTier: 'high' } });
    await flushPromises();

    const inner = w.get('.koyori-gantt-inner').element as HTMLElement;
    const svg = w.get('.koyori-gantt-svg').element.innerHTML;
    expect(Number.parseInt(inner.style.height, 10)).toBe(svgRootHeight(svg));

    const bars = w.findAll('[data-task-id="b"]');
    expect(bars).toHaveLength(2);
    await bars[0]!.trigger('click');
    await bars[1]!.trigger('click');
    const emitted = w.emitted('taskClick')!;
    expect(emitted.map((e) => (e[0] as GanttTask).id)).toEqual(['b', 'b']);
  });
});
