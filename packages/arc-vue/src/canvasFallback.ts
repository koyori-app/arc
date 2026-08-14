import {
  CANVAS_CAPACITY_CODE,
  EMPTY_SVG_MARKUP,
  type RenderFailure,
} from './wasmContract';

export type CanvasFailureResolution =
  | { mode: 'svg'; svg: string }
  | { mode: 'error'; message: string };

export function chartHeightForTaskCount(taskCount: number): number {
  return taskCount === 0 ? 0 : taskCount * 40 + 30 + 40 + 10;
}

/**
 * Canvas capacity errors can use the independently bounded SVG renderer.
 * Decided by `code` alone — the message beside it is display text and is free
 * to be reworded on the Rust side.
 */
export function isCanvasCapacityError(failure: RenderFailure): boolean {
  return failure.code === CANVAS_CAPACITY_CODE;
}

/** True when the SVG renderer drew a chart rather than the empty placeholder. */
export function isRenderedSvg(svg: string): boolean {
  return svg.startsWith('<svg ') && svg !== EMPTY_SVG_MARKUP;
}

export function resolveCanvasFailure(
  failure: RenderFailure,
  fallbackSvg: string,
): CanvasFailureResolution {
  if (isCanvasCapacityError(failure) && isRenderedSvg(fallbackSvg)) {
    return { mode: 'svg', svg: fallbackSvg };
  }
  return { mode: 'error', message: failure.message };
}
