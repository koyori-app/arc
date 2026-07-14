export type CanvasFailureResolution =
  | { mode: 'svg'; svg: string }
  | { mode: 'error'; message: string };

export function chartHeightForTaskCount(taskCount: number): number {
  return taskCount === 0 ? 0 : taskCount * 40 + 30 + 40 + 10;
}

/** Canvas capacity errors can use the independently bounded SVG renderer. */
export function isCanvasCapacityError(message: string): boolean {
  return /^canvas (row count|date range|area) exceeds limit/.test(message);
}

export function resolveCanvasFailure(
  message: string,
  fallbackSvg: string,
): CanvasFailureResolution {
  if (
    isCanvasCapacityError(message)
    && fallbackSvg.includes('<svg ')
    && !fallbackSvg.includes('width="0"')
  ) {
    return { mode: 'svg', svg: fallbackSvg };
  }
  return { mode: 'error', message };
}
