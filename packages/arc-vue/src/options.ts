/**
 * The `options_json` string the Wasm entry points take.
 *
 * `today`, the scroll window and the time zone used to be three positional
 * arguments; they are keys of one JSON object now, so a new option adds a key
 * and not a position. Only the keys that have a value are written, and when
 * none has one the result is `undefined`, which the entry points read as the
 * defaults.
 */
export interface ScrollViewportOption {
  scroll_y: number;
  client_height: number;
}

/** Whether the chart draws its own legend. Left out, the legend is drawn. */
export type LegendMode = 'shown' | 'hidden';

/**
 * Whether the chart draws one kind of its own text: `rowLabels` the task titles
 * left of the days, `percentLabels` the `NN%` on each bar. Left out, it is drawn.
 */
export type LabelMode = 'shown' | 'hidden';

/** Where a bar sits in its row. Left out, it is centred. */
export type BarAlign = 'middle' | 'bottom';

export interface ChartOptionsInput {
  today?: string | null;
  scrollViewport?: ScrollViewportOption | null;
  tzOffsetMinutes?: number | null;
  legend?: LegendMode | null;
  rowLabels?: LabelMode | null;
  percentLabels?: LabelMode | null;
  rowHeightPx?: number | null;
  headerHeightPx?: number | null;
  barHeightPx?: number | null;
  barAlign?: BarAlign | null;
}

/**
 * The object `options_json` carries. Every key is optional, and a chart drawn
 * with none of them is the chart drawn before any key existed.
 */
export interface ChartOptions {
  today?: string;
  scroll_viewport?: ScrollViewportOption;
  tz_offset_minutes?: number | string;
  legend?: LegendMode;
  row_labels?: LabelMode;
  percent_labels?: LabelMode;
  row_height_px?: number;
  header_height_px?: number;
  bar_height_px?: number;
  bar_align?: BarAlign;
}

export function buildOptionsJson(input: ChartOptionsInput): string | undefined {
  const options: ChartOptions = {};
  if (input.today != null) options.today = input.today;
  if (input.scrollViewport != null) options.scroll_viewport = input.scrollViewport;
  if (input.tzOffsetMinutes != null) {
    // JSON cannot carry NaN or Infinity (`JSON.stringify` would turn them into
    // `null`, which means "no offset"). A string keeps the unusable value
    // visible, so the entry point refuses it as it refuses any bad offset.
    options.tz_offset_minutes = Number.isFinite(input.tzOffsetMinutes)
      ? input.tzOffsetMinutes
      : String(input.tzOffsetMinutes);
  }
  // Only a given value is written. A key that is always present would turn
  // "left out" into a choice, and an old reader of the options would refuse it.
  if (input.legend != null) options.legend = input.legend;
  if (input.rowLabels != null) options.row_labels = input.rowLabels;
  if (input.percentLabels != null) options.percent_labels = input.percentLabels;
  if (input.rowHeightPx != null) options.row_height_px = input.rowHeightPx;
  if (input.headerHeightPx != null) options.header_height_px = input.headerHeightPx;
  if (input.barHeightPx != null) options.bar_height_px = input.barHeightPx;
  if (input.barAlign != null) options.bar_align = input.barAlign;
  return Object.keys(options).length === 0 ? undefined : JSON.stringify(options);
}
