/**
 * Values owned by `koyori-arc-core` and mirrored here for JS.
 *
 * These are pinned copies, not a second definition: `wasmContract.test.ts`
 * reads the Rust sources back and fails if any value below drifts from them.
 * Change the Rust literal and this file goes red.
 */

/** Keys are the Rust constant names in `crates/koyori-arc-core/src/error.rs`. */
export const RUST_ERROR_CODES = {
  CODE_CANVAS_CAPACITY: 'canvas_capacity',
  CODE_INPUT_LIMIT: 'input_limit',
  CODE_PARSE_ERROR: 'parse_error',
  CODE_SERIALIZE_ERROR: 'serialize_error',
} as const;

export type RustErrorCode = (typeof RUST_ERROR_CODES)[keyof typeof RUST_ERROR_CODES];

/** The Canvas backing store is too small, but the SVG renderer may still cope. */
export const CANVAS_CAPACITY_CODE = RUST_ERROR_CODES.CODE_CANVAS_CAPACITY;

/**
 * Exact markup of `koyori-arc-core::empty_svg()`, also reachable at runtime as
 * `empty_svg_markup()`. Compared byte for byte — a chart that merely *contains*
 * `width="0"` (any 0% progress bar does) is a real chart, not this one.
 */
export const EMPTY_SVG_MARKUP =
  '<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" viewBox="0 0 0 0" role="img" '
  + 'aria-label="Empty Gantt chart" font-family="sans-serif" font-size="12">'
  + '<title>Empty Gantt chart</title><desc>No tasks to display</desc></svg>';

/** A refusal from a Wasm entry point. `code` drives control flow; `message` is for display. */
export interface RenderFailure {
  message: string;
  /** Absent when the failure did not come from Rust (e.g. malformed JSON in transit). */
  code?: string;
}

/** Parse a `{"error":…,"code":…}` payload. Unparseable input becomes a message-only failure. */
export function parseRenderError(json: string): RenderFailure {
  try {
    const value = JSON.parse(json) as { error?: unknown; code?: unknown };
    return {
      message: typeof value.error === 'string' ? value.error : json,
      code: typeof value.code === 'string' ? value.code : undefined,
    };
  } catch {
    return { message: json };
  }
}
