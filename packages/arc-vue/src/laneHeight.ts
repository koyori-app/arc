import type { LegendMode } from './options';
import { RUST_LAYOUT } from './wasmContract';

/**
 * What a chart in lanes is guessed to be before the Wasm has answered: one row a lane.
 * The real height depends on how the bars pack into rows, which only the Wasm knows,
 * so this is replaced by `svgRootHeight()` (or the canvas's own height) as soon as
 * there is a drawing to read it from.
 */
export function estimateLaneChartHeight(
  laneCount: number,
  legend: LegendMode = 'shown',
): number {
  return laneCount * RUST_LAYOUT.ROW_H
    + RUST_LAYOUT.HEADER_H
    + (legend === 'hidden' ? 0 : RUST_LAYOUT.LEGEND_H)
    + RUST_LAYOUT.CHART_BOTTOM_PADDING_PX;
}

/** The `height` of the root `<svg>` of a drawing, or `null` when it has none. */
export function svgRootHeight(svg: string): number | null {
  const match = /^<svg [^>]*\bheight="([\d.]+)"/.exec(svg);
  return match ? Number(match[1]) : null;
}
