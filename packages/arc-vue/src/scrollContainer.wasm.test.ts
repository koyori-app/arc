/**
 * The window `scrollContainer` hands the crate, against the real Wasm: a chart
 * of 200 rows beside the caller's own list of 200 rows, both in one scroll area.
 * With the area scrolled, the crate draws only the rows inside the view, and
 * each one at the y of the same row in the list (the list has the chart's
 * header above its rows, so both count from the same top).
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, { render_svg } from '@koyori-app/arc';
import { buildOptionsJson } from './options';
import type { GanttTask } from './types.ts';
import { RUST_LAYOUT } from './wasmContract';

const ROW_H = 40;
const BAR_H = 16;
const tasks: GanttTask[] = Array.from({ length: 200 }, (_, i) => ({
  id: `t${i}`,
  title: `Task ${i}`,
  progress_pct: 30,
  start: '2026-06-01',
  end: '2026-06-10',
}));

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

/** Each drawn task's bar top, by task index. */
function barTops(svg: string): Map<number, number> {
  const tops = new Map<number, number>();
  const groups = svg.split('data-task-id="').slice(1);
  for (const g of groups) {
    const id = Number(/^t(\d+)"/.exec(g)?.[1]);
    const y = /<rect class="bar-bg"[^>]*\by="([\d.]+)"/.exec(g)?.[1];
    if (Number.isFinite(id) && y != null) tops.set(id, Number(y));
  }
  return tops;
}

/** The top of row `i` in the caller's list: the chart's header, then the rows. */
function listRowTop(i: number): number {
  return RUST_LAYOUT.HEADER_H + i * ROW_H;
}

describe('a chart scrolled by the caller\'s area, against the real Wasm', () => {
  it.each([0, 3000, 7000])(
    'draws only the rows in view, each beside the same row of the list (area scrolled %ipx)',
    (scrollTop) => {
      // 図と一覧は同じ域の中で頭を揃えて並ぶので、図の頭から見た域の頭は域の送りと同じ
      const viewHeight = 400;
      const options = buildOptionsJson({
        legend: 'hidden',
        rowLabels: 'hidden',
        rowHeightPx: ROW_H,
        barHeightPx: BAR_H,
        scrollViewport: { scroll_y: scrollTop, client_height: viewHeight },
      });
      const svg = render_svg(JSON.stringify(tasks), '[]', options);
      const tops = barTops(svg);
      // 描く行は見える分（域の高さぶんの行と、前後の余り）に絞られる
      expect(tops.size).toBeGreaterThan(0);
      expect(tops.size).toBeLessThan(40);
      for (const [i, top] of tops) {
        // 見える範囲の近くの行だけ
        expect(listRowTop(i) + ROW_H).toBeGreaterThan(scrollTop - ROW_H * 5);
        expect(listRowTop(i)).toBeLessThan(scrollTop + viewHeight + ROW_H * 5);
        // 一覧の同じ行の、真ん中に置いた棒
        expect(top).toBe(listRowTop(i) + (ROW_H - BAR_H) / 2);
      }
    },
  );

  it('draws every row without the window, so the window is what narrows it', () => {
    const options = buildOptionsJson({ legend: 'hidden', rowLabels: 'hidden', rowHeightPx: ROW_H, barHeightPx: BAR_H });
    expect(barTops(render_svg(JSON.stringify(tasks), '[]', options)).size).toBe(200);
  });
});
