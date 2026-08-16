import {
  CANVAS_CAPACITY_CODE,
  CHART_BOTTOM_PADDING_PX,
  RUST_LAYOUT,
  type RenderFailure,
} from './wasmContract';

export type CanvasFailureResolution =
  | { mode: 'svg'; svg: string }
  | { mode: 'error'; message: string };

/** Reserves exactly the height `render.rs` draws into, so scrolling matches the chart. */
export function chartHeightForTaskCount(taskCount: number): number {
  if (taskCount === 0) return 0;
  return taskCount * RUST_LAYOUT.ROW_H
    + RUST_LAYOUT.HEADER_H
    + RUST_LAYOUT.LEGEND_H
    + CHART_BOTTOM_PADDING_PX;
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
