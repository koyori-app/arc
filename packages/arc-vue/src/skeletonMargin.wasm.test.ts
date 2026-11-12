/**
 * The loading skeleton stands in for the chart, so its label column must end
 * where the drawn chart's bars start. This runs the real Wasm and compares the
 * left edge of a bar on the chart's first day with `skeletonLabelWidthPx` for
 * the same options, rather than comparing two copies of the crate's constants.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`,
 * the same as `vue-tsc` and `vite build` do.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_svg } from '@koyori-app/arc';
import { skeletonLabelWidthPx } from './canvasFallback';
import { buildOptionsJson } from './options';
import type { GanttTask } from './types.ts';

// The first task starts on the chart's first day, so its bar starts at the left margin.
const tasks: GanttTask[] = [
  { id: 'a', title: 'Alpha', progress_pct: 40, start: '2026-06-01', end: '2026-06-10' },
  { id: 'b', title: 'Bravo', progress_pct: 0, start: '2026-06-03', end: '2026-06-12' },
];

function firstBarLeft(svg: string): number {
  const match = /<rect class="bar-bg"[^>]*\bx="([\d.]+)"/.exec(svg);
  if (!match) throw new Error(`no bar: ${svg.slice(0, 120)}`);
  return Number(match[1]);
}

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('the skeleton\'s label column, against the real Wasm', () => {
  it.each([
    ['shown', undefined],
    ['hidden', undefined],
    ['shown', 30],
    ['hidden', 30],
    ['shown', 300],
    ['hidden', 300],
  ] as const)('ends where the chart\'s bars start (titles %s, bar %s)', (rowLabels, barHeight) => {
    const options = buildOptionsJson({
      rowLabels,
      barHeightPx: barHeight,
      rowHeightPx: barHeight == null ? undefined : Math.max(barHeight, 40),
    });
    const svg = render_svg(JSON.stringify(tasks), '[]', options);
    expect(firstBarLeft(svg)).toBe(skeletonLabelWidthPx(rowLabels, barHeight));
  });
});
