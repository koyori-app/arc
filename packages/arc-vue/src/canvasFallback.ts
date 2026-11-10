import type { LegendMode } from './options';
import type { GanttRow } from './types.ts';
import {
  CANVAS_CAPACITY_CODE,
  RUST_LAYOUT,
  type RenderFailure,
} from './wasmContract';

export type CanvasFailureResolution =
  | { mode: 'svg'; svg: string }
  | { mode: 'error'; message: string };

/**
 * Reserves exactly the height the display list builds, so scrolling matches the chart.
 * A hidden legend takes no height, so the band it would occupy is left out.
 */
export function chartHeightForTaskCount(
  taskCount: number,
  legend: LegendMode = 'shown',
): number {
  if (taskCount === 0) return 0;
  return taskCount * RUST_LAYOUT.ROW_H
    + RUST_LAYOUT.HEADER_H
    + (legend === 'hidden' ? 0 : RUST_LAYOUT.LEGEND_H)
    + RUST_LAYOUT.CHART_BOTTOM_PADDING_PX;
}

/** The sizes a caller may set, in pixels; each left out is the crate's own. */
export interface RowSizes {
  rowHeight?: number;
  headerHeight?: number;
}

/**
 * `chartHeightForTaskCount` for a chart that may lay out its own rows: the
 * header, every row at its own height (or `rowHeight`), and below them the
 * legend when shown and the bottom padding. Without `rows` it is one row per
 * task, as the crate lays them out.
 */
export function chartHeightForRows(
  rows: GanttRow[] | undefined,
  taskCount: number,
  legend: LegendMode = 'shown',
  sizes: RowSizes = {},
): number {
  if (taskCount === 0) return 0;
  const rowH = sizes.rowHeight ?? RUST_LAYOUT.ROW_H;
  const rowsH = rows
    ? rows.reduce((sum, row) => sum + (row.height ?? rowH), 0)
    : taskCount * rowH;
  return rowsH
    + (sizes.headerHeight ?? RUST_LAYOUT.HEADER_H)
    + (legend === 'hidden' ? 0 : RUST_LAYOUT.LEGEND_H)
    + RUST_LAYOUT.CHART_BOTTOM_PADDING_PX;
}

/**
 * Canvas capacity errors can use the independently bounded SVG renderer.
 * Decided by `code` alone — the message beside it is display text and is free
 * to be reworded on the Rust side.
 */
export function isCanvasCapacityError(failure: RenderFailure): boolean {
  return failure.code === CANVAS_CAPACITY_CODE;
}

/**
 * True when the SVG renderer drew a chart rather than the empty placeholder.
 *
 * `emptySvgMarkup` is passed in rather than read from `wasmContract`: at runtime
 * the only authority is `empty_svg_markup()` from the `@koyori-app/arc` build
 * actually loaded. See the note beside its call in `GanttChart.vue`.
 */
export function isRenderedSvg(svg: string, emptySvgMarkup: string): boolean {
  return svg.startsWith('<svg ') && svg !== emptySvgMarkup;
}

export function resolveCanvasFailure(
  failure: RenderFailure,
  fallbackSvg: string,
  emptySvgMarkup: string,
): CanvasFailureResolution {
  if (isCanvasCapacityError(failure) && isRenderedSvg(fallbackSvg, emptySvgMarkup)) {
    return { mode: 'svg', svg: fallbackSvg };
  }
  return { mode: 'error', message: failure.message };
}

/**
 * The width of the loading skeleton's label column, which stands in for the
 * chart's title column: `LABEL_W` with the titles shown, and with them hidden
 * the left margin the chart keeps for a first-day milestone, so the skeleton's
 * bars start where the drawn chart's do. That margin is half the bar the chart
 * draws (`HIDDEN_LABELS_LEFT_PX` for the default bar).
 */
export function skeletonLabelWidthPx(rowLabels?: 'shown' | 'hidden', barHeight?: number): number {
  if (rowLabels !== 'hidden') return RUST_LAYOUT.LABEL_W;
  return barHeight == null ? RUST_LAYOUT.HIDDEN_LABELS_LEFT_PX : barHeight / 2;
}
