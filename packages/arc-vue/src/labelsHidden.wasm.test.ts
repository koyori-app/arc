/**
 * The mocked tests pin what arc-vue sends; this one runs the real Wasm to check
 * what comes back. Hidden row labels must narrow the SVG and the canvas by the
 * title column `RUST_LAYOUT.LABEL_W`, and leave the titles out; hidden percent
 * labels must leave the `NN%` out.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`,
 * the same as `vue-tsc` and `vite build` do.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_canvas_commands, render_svg } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import { parseCommandBuffer } from './replayCommands';
import type { GanttTask } from './types.ts';
import { RUST_LAYOUT } from './wasmContract';

const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 100, start: '2026-06-01', end: '2026-06-20' },
  { id: 'b', title: 'Bravo', progress_pct: 40, start: '2026-06-05', end: '2026-06-25' },
  { id: 'c', title: 'Charlie', progress_pct: 0, start: '2026-06-10', end: '2026-06-30' },
];
const tasksJson = JSON.stringify(tasks);

function svgWidth(svg: string): number {
  const match = /^<svg [^>]*\bwidth="([\d.]+)"/.exec(svg);
  if (!match) throw new Error(`no width on the root: ${svg.slice(0, 120)}`);
  return Number(match[1]);
}

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('hidden labels, drawn by the real Wasm', () => {
  it('leaves the titles out of the SVG and narrows it by the title column', () => {
    const shown = render_svg(tasksJson, '[]', undefined);
    const gone = render_svg(tasksJson, '[]', buildOptionsJson({ rowLabels: 'hidden' }));

    for (const t of tasks) {
      expect(shown).toContain(`>${t.title}</text>`);
      expect(gone).not.toContain(`>${t.title}</text>`);
    }
    expect(gone).toContain('>40%</text>');
    expect(svgWidth(shown) - svgWidth(gone)).toBe(RUST_LAYOUT.LABEL_W);
  });

  it('narrows the canvas by the same width', () => {
    const shown = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', undefined));
    const gone = parseCommandBuffer(
      render_canvas_commands(tasksJson, '[]', buildOptionsJson({ rowLabels: 'hidden' })),
    );
    expect(shown.viewport_width - gone.viewport_width).toBe(RUST_LAYOUT.LABEL_W);
    expect(gone.viewport_height).toBe(shown.viewport_height);
  });

  it('leaves the percent labels out of the SVG and keeps the titles', () => {
    const gone = render_svg(tasksJson, '[]', buildOptionsJson({ percentLabels: 'hidden' }));
    expect(gone).not.toContain('>40%</text>');
    expect(gone).not.toContain('>100%</text>');
    expect(gone).toContain('>Alpha</text>');
  });
});
