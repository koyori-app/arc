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

/**
 * Keys are the Rust constant names in
 * `crates/koyori-arc-core/src/display_list/constants.rs`.
 *
 * Only the constants the JS side reproduces are listed — the crate owns many
 * more (colours, arrow geometry) that never cross the boundary. The contract
 * test checks each key below against the Rust source rather than demanding the
 * two sets match, so adding a Rust-only constant does not go red for no reason.
 *
 * The bar goes both ways: a value JS never reproduces does not belong here.
 * `CHART_RIGHT_PADDING_PX`, for one, is spent entirely inside the crate — JS
 * never computes a chart width — and pinning it would only invite a caller to
 * start reproducing it.
 *
 * Reproduced where:
 * - `ROW_H`, `HEADER_H`, `LEGEND_H`, `CHART_BOTTOM_PADDING_PX` — the chart
 *   height `chartHeightForTaskCount()` reserves before Wasm answers.
 * - `ROW_H`, `LABEL_W`, `BAR_H` — the loading skeleton's row metrics, which
 *   stand in for real rows and must line up with them.
 */
export const RUST_LAYOUT = {
  ROW_H: 40,
  HEADER_H: 30,
  LEGEND_H: 40,
  CHART_BOTTOM_PADDING_PX: 10,
  LABEL_W: 120,
  BAR_H: 20,
} as const;

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
