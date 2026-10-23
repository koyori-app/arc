/**
 * The mocked tests pin what arc-vue sends; this one runs the real Wasm to check
 * what comes back. A hidden legend must shorten the SVG and the canvas by the
 * height `chartHeightForTaskCount()` takes off, or the room reserved for the
 * chart and the chart disagree.
 *
 * Needs `scripts/build-npm.sh` to have produced `crates/koyori-arc-core/pkg`,
 * the same as `vue-tsc` and `vite build` do.
 */
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { beforeAll, describe, expect, it } from 'vitest';
import init, {
  empty_svg_markup,
  legend_spec_json,
  render_canvas_commands,
  render_svg,
} from '@koyori-app/arc';
import { chartHeightForTaskCount, resolveCanvasFailure } from './canvasFallback';
import { loadLegendSpec } from './legend';
import { buildOptionsJson } from './options';
import { parseCommandBuffer } from './replayCommands';
import type { GanttTask } from './types.ts';
import { RUST_ERROR_CODES, RUST_LAYOUT } from './wasmContract';

const tasks: GanttTask[] = [
  { id: 'a', title: 'A', progress_pct: 100, start: '2026-06-01', end: '2026-06-20' },
  { id: 'b', title: 'B', progress_pct: 40, start: '2026-06-05', end: '2026-06-25' },
  { id: 'c', title: 'C', progress_pct: 0, start: '2026-06-10', end: '2026-06-30' },
];
const tasksJson = JSON.stringify(tasks);

const hidden = buildOptionsJson({ legend: 'hidden' });

function svgHeight(svg: string): number {
  const match = /^<svg [^>]*\bheight="([\d.]+)"/.exec(svg);
  if (!match) throw new Error(`no height on the root: ${svg.slice(0, 120)}`);
  return Number(match[1]);
}

beforeAll(async () => {
  const pkgDir = dirname(createRequire(import.meta.url).resolve('@koyori-app/arc/package.json'));
  await init({ module_or_path: readFileSync(join(pkgDir, 'koyori_arc_core_bg.wasm')) });
});

describe('a hidden legend, drawn by the real Wasm', () => {
  it('leaves the legend out of the SVG and shortens it by the height reserved for it', () => {
    const shown = render_svg(tasksJson, '[]', undefined);
    const gone = render_svg(tasksJson, '[]', hidden);

    expect(shown).toContain('bar-tier-legend');
    expect(gone).not.toContain('bar-tier-legend');
    expect(gone).not.toContain('progress-line-legend');
    expect(svgHeight(shown)).toBe(chartHeightForTaskCount(tasks.length));
    expect(svgHeight(gone)).toBe(chartHeightForTaskCount(tasks.length, 'hidden'));
    expect(svgHeight(shown) - svgHeight(gone)).toBe(RUST_LAYOUT.LEGEND_H);
  });

  it('shortens the canvas by the same height, and leaves the legend out of its drawing', () => {
    const shown = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', undefined));
    const gone = parseCommandBuffer(render_canvas_commands(tasksJson, '[]', hidden));

    expect(shown.error).toBeUndefined();
    expect(gone.error).toBeUndefined();
    expect(shown.viewport_height).toBe(chartHeightForTaskCount(tasks.length));
    expect(gone.viewport_height).toBe(chartHeightForTaskCount(tasks.length, 'hidden'));
    expect(gone.ops.length).toBeLessThan(shown.ops.length);
  });

  it('keeps the SVG a canvas falls back to as short as the room reserved for it', () => {
    // `renderCanvasFallback` asks for a window of at most 600px.
    const options = buildOptionsJson({
      legend: 'hidden',
      scrollViewport: { scroll_y: 0, client_height: 600 },
    });
    const svg = render_svg(tasksJson, '[]', options);

    expect(svg).not.toContain('bar-tier-legend');
    expect(svgHeight(svg)).toBe(chartHeightForTaskCount(tasks.length, 'hidden'));
  });

  it('draws the same chart when `legend` is "shown" as when it is left out', () => {
    expect(render_svg(tasksJson, '[]', buildOptionsJson({ legend: 'shown' })))
      .toBe(render_svg(tasksJson, '[]', undefined));
  });

  it('refuses a value that is neither "shown" nor "hidden" instead of drawing something', () => {
    const svg = render_svg(tasksJson, '[]', '{"legend":"none"}');
    expect(svg).not.toContain('bar-tier-legend');
    expect(svgHeight(svg)).toBe(0);
  });
});

describe('loadLegendSpec, against the real Wasm', () => {
  it('is what legend_spec_json says, parsed', async () => {
    expect(await loadLegendSpec()).toEqual(JSON.parse(legend_spec_json()));
  });

  it('says what the legend the chart draws says: every tier label and colour, and the line', async () => {
    const spec = await loadLegendSpec();
    const svg = render_svg(tasksJson, '[]', undefined);
    const legend = svg.slice(svg.indexOf('class="bar-tier-legend"'));
    const line = svg.slice(svg.indexOf('class="progress-line-legend"'));

    expect(spec.tiers.length).toBeGreaterThan(0);
    for (const tier of spec.tiers) {
      expect(tier.min_pct).toBeLessThanOrEqual(tier.max_pct);
      expect(legend, tier.key).toContain(`fill="${tier.color}"`);
      expect(legend, tier.key).toContain(`>${tier.label}</text>`);
    }
    expect(line).toContain(`stroke="${spec.progress_line.color}"`);
    expect(line).toContain(`stroke-dasharray="${spec.progress_line.dash}"`);
    expect(line).toContain(`>${spec.progress_line.label}</text>`);
  });
});

describe('a canvas the limit refuses, drawn as the SVG it falls back to', () => {
  // A chart whose last day holds a milestone. Its label runs past the days, and
  // the canvas limit counts the label whether the chart has a legend or not, so
  // the two are refused at the same span.
  const dayMs = 24 * 60 * 60 * 1000;
  const iso = (offsetDays: number) =>
    new Date(Date.UTC(2026, 0, 1) + offsetDays * dayMs).toISOString().slice(0, 10);
  const chartOf = (span: number): GanttTask[] => [
    { id: 'long', title: 'Long', progress_pct: 10, start: iso(0), end: iso(span) },
    { id: 'ms', title: 'Milestone', progress_pct: 100, start: iso(span), end: iso(span) },
  ];
  const canvasError = (span: number, options: string | undefined) =>
    parseCommandBuffer(render_canvas_commands(JSON.stringify(chartOf(span)), '[]', options));
  // The longest span the canvas accepts, found by asking, so no limit is copied here.
  const longestAccepted = (options: string | undefined) => {
    let span = 1;
    while (!canvasError(span + 1, options).error) span += 1;
    return span;
  };

  it('is refused at the same span with and without a legend, by the capacity code that has a fallback', () => {
    const shown = longestAccepted(undefined);
    const gone = longestAccepted(hidden);
    expect(gone).toBe(shown);
    for (const options of [undefined, hidden]) {
      expect(canvasError(shown, options).error).toBeUndefined();
      expect(canvasError(shown + 1, options).code).toBe(RUST_ERROR_CODES.CODE_CANVAS_CAPACITY);
    }
  });

  it('falls back to an SVG without the legend, as tall as the room reserved for it', () => {
    const span = longestAccepted(hidden) + 1;
    const tasksOfChart = chartOf(span);
    const failure = canvasError(span, hidden);
    // What GanttChart.vue asks for when it falls back: the same legend, and a
    // window of at most 600px.
    const options = buildOptionsJson({
      legend: 'hidden',
      scrollViewport: { scroll_y: 0, client_height: 600 },
    });
    const svg = render_svg(JSON.stringify(tasksOfChart), '[]', options);
    const resolution = resolveCanvasFailure(
      { message: failure.error ?? '', code: failure.code },
      svg,
      empty_svg_markup(),
    );
    expect(resolution.mode).toBe('svg');
    expect(svg).not.toContain('bar-tier-legend');
    expect(svg).not.toContain('progress-line-legend');
    expect(svgHeight(svg)).toBe(chartHeightForTaskCount(tasksOfChart.length, 'hidden'));
  });
});
